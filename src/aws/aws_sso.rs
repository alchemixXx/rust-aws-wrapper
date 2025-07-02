use crate::{
    constants,
    custom_error::{CustomError, CustomResult},
    logger::Logger,
    zsh_command::ZshCommand,
};
use anyhow::Context;
use chrono::{DateTime, Utc};
use dirs::home_dir;
use glob::glob;
use serde::Deserialize;
use std::{env, fs};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SsoCacheEntry {
    start_url: Option<String>,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
struct SsoInput {
    profile: String,
}

#[derive(Deserialize)]
struct CacheFile {
    #[serde(rename = "accessToken")]
    access_token: String,
}

#[derive(Debug)]
struct ProfileInfo {
    account_id: String,
    role_name: String,
    region: String,
}

#[derive(Deserialize, Debug)]
struct RoleCredentials {
    #[serde(rename = "accessKeyId")]
    access_key_id: String,
    #[serde(rename = "secretAccessKey")]
    secret_access_key: String,
    #[serde(rename = "sessionToken")]
    session_token: String,
}

#[derive(Deserialize)]
struct SsoResponse {
    #[serde(rename = "roleCredentials")]
    role_credentials: RoleCredentials,
}

pub struct AwsSso {
    input: SsoInput,
    logger: Logger,
    zsh_command: ZshCommand,
}

impl AwsSso {
    pub fn new(profile: String) -> Self {
        Self {
            input: SsoInput { profile },
            logger: Logger::new(),
            zsh_command: ZshCommand::new(),
        }
    }

    pub fn login(&self) -> CustomResult<()> {
        self.logger.debug("Logging in to AWS with SSO tokens");

        let sso_is_valid = self.sso_token_still_valid(constants::SSO_START_URL)?;

        if sso_is_valid {
            self.logger
                .debug("SSO token is valid, no need to log in again.");
        } else {
            self.logger
                .debug("SSO token is not valid, checking for existing session...");
            let command = "aws sso login --sso-session sso";
            self.zsh_command.execute(command)?;
        }

        self.logger.debug("Logged in to AWS with SSO");

        Ok(())
    }

    pub fn set_sso_credentials(&self) -> CustomResult<()> {
        self.logger.debug("Setting AWS SSO credentials");
        let profile_info = self.get_sso_profile_info(&self.input.profile)?;
        let token = self.get_latest_sso_token()?;

        let creds = self.execute_sso_command(
            &profile_info.account_id,
            &profile_info.role_name,
            &token,
            &profile_info.region,
        )?;

        // Set them as env vars for current process
        self.set_environment_variables(&creds)?;

        self.logger.debug("AWS SSO credentials set successfully");

        Ok(())
    }

    fn get_latest_sso_token(&self) -> CustomResult<String> {
        let cache_path = dirs::home_dir()
            .context("Failed to get home directory")
            .map_err(|err| {
                CustomError::CommandExecution(format!("Failed to get home directory: {}", err))
            })?
            .join(".aws/sso/cache");

        let mut files: Vec<_> = fs::read_dir(&cache_path)
            .map_err(|err| {
                CustomError::CommandExecution(format!(
                    "Failed to read SSO cache directory: {}",
                    err
                ))
            })?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "json")
                    .unwrap_or(false)
            })
            .collect();

        if files.is_empty() {
            return Err(CustomError::CommandExecution(
                "No SSO cache files found".to_string(),
            ));
        }

        files.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());

        let latest_file = files
            .last()
            .context("Failed to get latest cache file")
            .map_err(|err| {
                CustomError::CommandExecution(format!("Failed to get latest cache file: {}", err))
            })?;
        let contents = fs::read_to_string(latest_file.path()).map_err(|err| {
            CustomError::CommandExecution(format!("Failed to read SSO cache file: {}", err))
        })?;
        let cache: CacheFile = serde_json::from_str(&contents).map_err(|err| {
            CustomError::CommandExecution(format!("Failed to parse SSO cache file: {}", err))
        })?;

        Ok(cache.access_token)
    }

    fn get_profile_block(&self, config_contents: &str, profile_name: &str) -> CustomResult<String> {
        self.logger
            .debug(format!("Fetching profile block for '{}'", profile_name));
        let profile_header = format!("[profile {}]", profile_name);
        let lines = config_contents.lines();
        let mut capture = false;
        let mut block = String::new();

        for line in lines {
            if line.trim() == profile_header {
                capture = true;
                continue;
            }

            if capture {
                if line.trim_start().starts_with("[profile") {
                    break;
                }
                block.push_str(line);
                block.push('\n');
            }
        }

        if !capture {
            self.logger.error(format!(
                "Profile '{}' not found in AWS config",
                profile_name
            ));
            Err(CustomError::CommandExecution(format!(
                "Profile '{}' not found in AWS config",
                profile_name
            )))
        } else {
            self.logger.debug(format!(
                "Profile block for '{}' fetched successfully",
                profile_name
            ));
            Ok(block)
        }
    }

    fn parse_profile_values(&self, profile_block: &str) -> CustomResult<ProfileInfo> {
        self.logger
            .debug("Parsing profile values from profile block");
        let mut account_id = None;
        let mut role_name = None;
        let mut region = None;

        for line in profile_block.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "sso_account_id" => account_id = Some(value.to_string()),
                    "sso_role_name" => role_name = Some(value.to_string()),
                    "region" => region = Some(value.to_string()),
                    _ => {}
                }
            }
        }

        match (account_id, role_name, region) {
            (Some(account_id), Some(role_name), Some(region)) => Ok(ProfileInfo {
                account_id,
                role_name,
                region,
            }),
            _ => {
                self.logger
                    .error("Missing one or more required fields in profile block");
                Err(CustomError::CommandExecution(
                    "Missing one or more required fields in profile block".to_string(),
                ))
            }
        }
    }

    fn get_sso_profile_info(&self, profile_name: &str) -> CustomResult<ProfileInfo> {
        self.logger
            .debug(format!("Fetching SSO profile info for '{}'", profile_name));
        let config_path = dirs::home_dir()
            .context("Failed to get home directory")
            .map_err(|err| {
                self.logger
                    .error(format!("Failed to get home directory: {}", err));
                CustomError::CommandExecution(format!("Failed to get home directory: {}", err))
            })?
            .join(".aws/config");

        let contents = fs::read_to_string(config_path).map_err(|err| {
            self.logger
                .error(format!("Failed to read AWS config file: {}", err));
            CustomError::CommandExecution(format!("Failed to read AWS config file: {}", err))
        })?;

        let profile_block = self.get_profile_block(&contents, profile_name)?;
        let values = self.parse_profile_values(&profile_block)?;

        Ok(values)
    }

    fn execute_sso_command(
        &self,
        account_id: &str,
        role_name: &str,
        token: &str,
        region: &str,
    ) -> CustomResult<RoleCredentials> {
        let command = format!(
            "aws sso get-role-credentials --account-id {} --role-name {} --access-token {} --region {}",
            account_id, role_name, token, region
        );

        let output = self.zsh_command.execute(&command)?;

        let resp: SsoResponse = serde_json::from_slice(&output.stdout).map_err(|err| {
            self.logger
                .error(format!("Failed to parse AWS CLI response: {}", err));
            CustomError::CommandExecution(format!("Failed to parse AWS CLI response: {}", err))
        })?;
        Ok(resp.role_credentials)
    }

    fn set_environment_variables(&self, creds: &RoleCredentials) -> CustomResult<()> {
        self.logger
            .debug("Setting environment variables for AWS credentials");
        env::set_var("AWS_ACCESS_KEY_ID", &creds.access_key_id);
        env::set_var("AWS_SECRET_ACCESS_KEY", &creds.secret_access_key);
        env::set_var("AWS_SESSION_TOKEN", &creds.session_token);

        self.logger.debug("Environment variables set successfully");

        Ok(())
    }

    fn sso_token_still_valid(&self, sso_start_url: &str) -> CustomResult<bool> {
        let cache_dir = home_dir()
            .ok_or("Failed to get home directory")
            .map_err(|err| CustomError::CommandExecution(err.to_string()))?
            .join(".aws/sso/cache");

        let pattern = cache_dir.join("*.json");
        let glob_pattern = pattern
            .to_str()
            .ok_or("Failed to convert pattern to string")
            .map_err(|err| CustomError::CommandExecution(err.to_string()))?;

        let paths =
            glob(glob_pattern).map_err(|err| CustomError::CommandExecution(err.to_string()))?;

        for entry in paths {
            let path = match entry {
                Ok(p) => p,
                Err(e) => {
                    self.logger
                        .error(format!("Skipping invalid glob entry: {}", e));
                    continue;
                }
            };

            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    self.logger
                        .error(format!("Skipping unreadable file {:?}: {}", path, e));
                    continue;
                }
            };

            let cache_entry: SsoCacheEntry = match serde_json::from_str(&content) {
                Ok(c) => c,
                Err(e) => {
                    self.logger
                        .error(format!("Skipping unparsable JSON in {:?}: {}", path, e));
                    continue;
                }
            };

            if let (Some(start_url), Some(expires_at)) =
                (cache_entry.start_url, cache_entry.expires_at)
            {
                if start_url == sso_start_url && expires_at > Utc::now() {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }
}
