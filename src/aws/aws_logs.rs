use std::fs;
use std::time::{Duration, Instant};

use chrono::DateTime;
use dialoguer::{FuzzySelect, Input};
use serde::{Deserialize, Serialize};

use crate::{
    aws::aws_sso::AwsSso,
    custom_error::{CustomError, CustomResult},
    logger::Logger,
    zsh_command::ZshCommand,
};

/// A single log event with the fields we persist to JSON.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub timestamp: i64,
    pub message: String,
    pub log_stream_name: String,
}

/// Response shape from `aws logs filter-log-events`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FilterLogEventsResponse {
    events: Option<Vec<RawLogEvent>>,
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLogEvent {
    timestamp: Option<i64>,
    message: Option<String>,
    log_stream_name: Option<String>,
}

/// Response shape from `aws logs describe-log-groups`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DescribeLogGroupsResponse {
    log_groups: Option<Vec<LogGroup>>,
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LogGroup {
    log_group_name: Option<String>,
}

const MAX_RETRIES: u32 = 3;
const FETCH_TIMEOUT: Duration = Duration::from_secs(300);
const OUTPUT_FILE: &str = "logs.json";

pub struct AwsLogs {
    logger: Logger,
    zsh_command: ZshCommand,
}

impl AwsLogs {
    pub fn new() -> Self {
        Self {
            logger: Logger::new(),
            zsh_command: ZshCommand::new(),
        }
    }

    /// Main entry point for the `raw logs` command.
    pub fn run(&self) -> CustomResult<()> {
        // Step 1: Profile selection
        let profile = self.select_profile()?;

        // Step 2: SSO authentication
        self.authenticate(&profile)?;

        // Step 3: Log group selection
        let log_group = self.select_log_group()?;

        // Step 4: Time range prompts
        let (start_ms, end_ms) = self.prompt_time_range()?;

        // Step 5: Optional logId filter
        let log_id = self.prompt_log_id()?;

        // Step 6: Fetch logs
        let events = self.fetch_logs(&log_group, start_ms, end_ms, log_id.as_deref())?;

        // Step 7: Write to file
        self.write_output(&events)?;

        Ok(())
    }

    // ─── Profile Selection ───────────────────────────────────────────────

    fn select_profile(&self) -> CustomResult<String> {
        self.logger.debug("Reading AWS config for profile selection");

        let config_path = dirs::home_dir()
            .ok_or_else(|| {
                CustomError::CommandExecution("Failed to get home directory".to_string())
            })?
            .join(".aws/config");

        let contents = fs::read_to_string(&config_path).map_err(|err| {
            CustomError::CommandExecution(format!(
                "Could not read ~/.aws/config: {}",
                err
            ))
        })?;

        let profiles: Vec<String> = contents
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with("[profile ") && trimmed.ends_with(']') {
                    Some(
                        trimmed
                            .trim_start_matches("[profile ")
                            .trim_end_matches(']')
                            .to_string(),
                    )
                } else {
                    None
                }
            })
            .collect();

        if profiles.is_empty() {
            return Err(CustomError::CommandExecution(
                "No profiles found in ~/.aws/config".to_string(),
            ));
        }

        let selection = FuzzySelect::new()
            .with_prompt("Select an AWS profile")
            .items(&profiles)
            .default(0)
            .interact()
            .map_err(|err| {
                CustomError::CommandExecution(format!("Profile selection failed: {}", err))
            })?;

        let selected = profiles[selection].clone();
        self.logger
            .info(format!("Selected profile: {}", &selected));
        Ok(selected)
    }

    // ─── SSO Authentication ──────────────────────────────────────────────

    fn authenticate(&self, profile: &str) -> CustomResult<()> {
        self.logger
            .info(format!("Authenticating with profile '{}'", profile));

        let sso = AwsSso::new(profile.to_string());
        sso.login()?;
        sso.set_sso_credentials()?;

        self.logger.info("SSO authentication successful");
        Ok(())
    }

    // ─── Log Group Selection ─────────────────────────────────────────────

    fn select_log_group(&self) -> CustomResult<String> {
        self.logger.debug("Fetching available log groups");

        let mut log_groups: Vec<String> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut command =
                String::from("aws logs describe-log-groups --output json");
            if let Some(ref token) = next_token {
                command.push_str(&format!(" --next-token '{}'", token));
            }

            let output = self.zsh_command.execute(&command)?;
            let response: DescribeLogGroupsResponse =
                serde_json::from_slice(&output.stdout).map_err(|err| {
                    CustomError::CommandExecution(format!(
                        "Failed to parse describe-log-groups response: {}",
                        err
                    ))
                })?;

            if let Some(groups) = response.log_groups {
                for g in groups {
                    if let Some(name) = g.log_group_name {
                        log_groups.push(name);
                    }
                }
            }

            match response.next_token {
                Some(token) if !token.is_empty() => next_token = Some(token),
                _ => break,
            }
        }

        if log_groups.is_empty() {
            return Err(CustomError::CommandExecution(
                "No log groups found for the authenticated profile".to_string(),
            ));
        }

        // Extract unique environment prefixes from log group names.
        // Pattern: /aws/lambda/<env>-<rest> → env is e.g. "conform5-qa-101"
        // We detect envs by finding common prefixes that appear across multiple groups.
        let filtered_groups = self.filter_by_environment(&log_groups)?;

        let selection = FuzzySelect::new()
            .with_prompt("Select a log group")
            .items(&filtered_groups)
            .default(0)
            .interact()
            .map_err(|err| {
                CustomError::CommandExecution(format!("Log group selection failed: {}", err))
            })?;

        let selected = filtered_groups[selection].clone();
        self.logger
            .info(format!("Selected log group: {}", &selected));
        Ok(selected)
    }

    /// Extracts environment names from log groups and lets the user pick one to filter by.
    ///
    /// Environment detection strategy:
    /// For log groups like `/aws/lambda/conform5-qa-101-someFunction`, the env is
    /// the portion up to and including the version segment (e.g., `conform5-qa-101`).
    /// We split each group name by `/`, take the last segment, then extract the env
    /// prefix by matching everything up to the third `-` separated numeric segment.
    ///
    /// Falls back to showing all groups if environments can't be detected.
    fn filter_by_environment(&self, log_groups: &[String]) -> CustomResult<Vec<String>> {
        let mut envs: Vec<String> = Vec::new();

        for name in log_groups {
            if let Some(env) = self.extract_env_prefix(name) {
                if !envs.contains(&env) {
                    envs.push(env);
                }
            }
        }

        // If we found fewer than 2 envs, no point filtering — show all groups
        if envs.len() < 2 {
            return Ok(log_groups.to_vec());
        }

        envs.sort();

        let selection = FuzzySelect::new()
            .with_prompt("Select an environment")
            .items(&envs)
            .default(0)
            .interact()
            .map_err(|err| {
                CustomError::CommandExecution(format!("Environment selection failed: {}", err))
            })?;

        let selected_env = &envs[selection];
        self.logger
            .info(format!("Selected environment: {}", selected_env));

        let filtered: Vec<String> = log_groups
            .iter()
            .filter(|name| name.contains(selected_env.as_str()))
            .cloned()
            .collect();

        Ok(filtered)
    }

    /// Extracts the environment prefix from a log group name.
    ///
    /// Examples:
    ///   `/aws/lambda/conform5-qa-101-myFunction` → `conform5-qa-101`
    ///   `/aws/lambda/conform5-qa-110-otherFunc`  → `conform5-qa-110`
    ///   `/aws/ecs/conform5-prod-200-service`     → `conform5-prod-200`
    ///
    /// Strategy: take the last path segment, split by `-`, and find the prefix
    /// that ends with a numeric segment (the version number).
    fn extract_env_prefix(&self, log_group_name: &str) -> Option<String> {
        // Get the last path segment (after the final `/`)
        let segment = log_group_name.rsplit('/').next()?;

        let parts: Vec<&str> = segment.split('-').collect();
        if parts.len() < 3 {
            return None;
        }

        // Find the first numeric part (version number) and include everything up to it.
        // e.g., ["conform5", "qa", "101", "myFunction"] → "conform5-qa-101"
        let mut env_end_idx = None;
        for (i, part) in parts.iter().enumerate() {
            if i > 0 && part.chars().all(|c| c.is_ascii_digit()) {
                env_end_idx = Some(i);
                break;
            }
        }

        let end = env_end_idx?;
        Some(parts[..=end].join("-"))
    }

    // ─── Time Range Prompts ──────────────────────────────────────────────

    fn prompt_time_range(&self) -> CustomResult<(i64, i64)> {
        let start_ms = self.prompt_timestamp("Enter start time (ISO 8601, e.g. 2024-01-15T10:00:00Z)")?;

        loop {
            let end_ms =
                self.prompt_timestamp("Enter end time (ISO 8601, e.g. 2024-01-15T11:00:00Z)")?;

            if end_ms <= start_ms {
                println!("Error: End time must be after start time. Please try again.");
                continue;
            }

            return Ok((start_ms, end_ms));
        }
    }

    fn prompt_timestamp(&self, prompt: &str) -> CustomResult<i64> {
        loop {
            let input: String = Input::new()
                .with_prompt(prompt)
                .interact_text()
                .map_err(|err| {
                    CustomError::CommandExecution(format!("Input failed: {}", err))
                })?;

            match DateTime::parse_from_rfc3339(&input) {
                Ok(dt) => return Ok(dt.timestamp_millis()),
                Err(_) => {
                    println!(
                        "Invalid format. Please use ISO 8601 format, e.g. 2024-01-15T10:00:00Z"
                    );
                    continue;
                }
            }
        }
    }

    /// Prompts for an optional logId filter. Returns None if the user leaves it empty.
    fn prompt_log_id(&self) -> CustomResult<Option<String>> {
        let input: String = Input::new()
            .with_prompt("Enter logId to filter by (leave empty to skip)")
            .allow_empty(true)
            .interact_text()
            .map_err(|err| {
                CustomError::CommandExecution(format!("Input failed: {}", err))
            })?;

        let trimmed = input.trim().to_string();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            self.logger
                .info(format!("Filtering by logId = {}", &trimmed));
            Ok(Some(trimmed))
        }
    }

    // ─── Log Fetching ────────────────────────────────────────────────────

    fn fetch_logs(
        &self,
        log_group: &str,
        start_ms: i64,
        end_ms: i64,
        log_id: Option<&str>,
    ) -> CustomResult<Vec<LogEvent>> {
        self.logger.info(format!(
            "Fetching logs from '{}' between {} and {}",
            log_group, start_ms, end_ms
        ));

        let mut all_events: Vec<LogEvent> = Vec::new();
        let mut next_token: Option<String> = None;
        let start_time = Instant::now();

        loop {
            if start_time.elapsed() >= FETCH_TIMEOUT {
                self.logger
                    .warn("Fetch timeout reached (300s). Returning collected events.");
                break;
            }

            let mut command = format!(
                "aws logs filter-log-events --log-group-name '{}' --start-time {} --end-time {} --output json",
                log_group, start_ms, end_ms
            );

            if let Some(id) = log_id {
                command.push_str(&format!(
                    " --filter-pattern '{{ $.logId = \"{}\" }}'",
                    id
                ));
            }

            if let Some(ref token) = next_token {
                command.push_str(&format!(" --next-token '{}'", token));
            }

            let output = self.execute_with_retry(&command)?;

            let response: FilterLogEventsResponse =
                serde_json::from_slice(&output).map_err(|err| {
                    CustomError::CommandExecution(format!(
                        "Failed to parse filter-log-events response: {}",
                        err
                    ))
                })?;

            if let Some(events) = response.events {
                for raw in events {
                    all_events.push(LogEvent {
                        timestamp: raw.timestamp.unwrap_or(0),
                        message: raw.message.unwrap_or_default(),
                        log_stream_name: raw.log_stream_name.unwrap_or_default(),
                    });
                }
            }

            match response.next_token {
                Some(token) if !token.is_empty() => next_token = Some(token),
                _ => break,
            }
        }

        self.logger
            .info(format!("Fetched {} log events", all_events.len()));
        Ok(all_events)
    }

    fn execute_with_retry(&self, command: &str) -> CustomResult<Vec<u8>> {
        let mut last_error = None;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay = Duration::from_secs(1 << (attempt - 1)); // 1s, 2s, 4s
                self.logger.warn(format!(
                    "Retry attempt {}/{} after {}s delay",
                    attempt,
                    MAX_RETRIES,
                    delay.as_secs()
                ));
                std::thread::sleep(delay);
            }

            match self.zsh_command.execute(command) {
                Ok(output) => return Ok(output.stdout),
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            CustomError::CommandExecution("All retry attempts failed".to_string())
        }))
    }

    // ─── File Output ─────────────────────────────────────────────────────

    fn write_output(&self, events: &[LogEvent]) -> CustomResult<()> {
        let json = serde_json::to_string_pretty(events).map_err(|err| {
            CustomError::CommandExecution(format!("Failed to serialize log events: {}", err))
        })?;

        fs::write(OUTPUT_FILE, &json).map_err(|err| {
            CustomError::CommandExecution(format!("Failed to write {}: {}", OUTPUT_FILE, err))
        })?;

        let abs_path = std::env::current_dir()
            .map(|p| p.join(OUTPUT_FILE))
            .unwrap_or_else(|_| std::path::PathBuf::from(OUTPUT_FILE));

        if events.is_empty() {
            println!(
                "No events found for the given time range. Empty array written to {}",
                abs_path.display()
            );
        } else {
            println!(
                "Saved {} events to {}",
                events.len(),
                abs_path.display()
            );
        }

        Ok(())
    }
}
