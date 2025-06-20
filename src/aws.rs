use serde::{Deserialize, Serialize};
use std::process::{Command, Output};

use chrono::{DateTime, Utc};
use dirs::home_dir;
use glob::glob;
use std::fs;

use crate::{
    aws_sso::AwsSso,
    constants,
    custom_error::{CustomError, CustomResult},
    logger::Logger,
};

#[derive(Debug, Deserialize)]
struct SsoCacheEntry {
    startUrl: Option<String>,
    expiresAt: Option<DateTime<Utc>>,
    _accessToken: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Commit {
    pull_request: PullRequest,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequest {
    pull_request_id: String,
}

pub struct AwsCli {
    logger: Logger,
}

impl AwsCli {
    pub fn new() -> Self {
        Self {
            logger: Logger::new(),
        }
    }

    fn execute_zsh_command(&self, command: &str) -> CustomResult<Output> {
        let output = Command::new("zsh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|err| CustomError::CommandExecution(err.to_string()))?;

        if !output.status.success() {
            self.logger
                .error(format!("Failed to execute command: {}", command));
            self.logger.error(format!(
                "Error: {}",
                String::from_utf8_lossy(&output.stderr)
            ));

            return Err(CustomError::CommandExecution(
                "Failed to execute command".to_string(),
            ));
        }

        Ok(output)
    }

    pub fn login(&self) -> CustomResult<()> {
        self.logger.info("Logging in to AWS");

        let sso_is_valid = self.sso_token_still_valid(constants::SSO_START_URL)?;

        if sso_is_valid {
            self.logger
                .info("SSO token is valid, no need to log in again.");
        } else {
            self.logger
                .info("SSO token is not valid, checking for existing session...");
            let command = "aws sso login --sso-session sso";
            self.execute_zsh_command(command)?;
        }

        self.logger.info("Logged in to AWS");

        Ok(())
    }

    fn change_role(&self, role: &str) -> CustomResult<()> {
        self.logger.info(format!("Changing AWS role to '{}'", role));
        AwsSso::new(role.to_string()).set_sso_credentials()?;
        self.logger.info(format!("Changed AWS role to '{}'", role));

        Ok(())
    }

    pub async fn create_pull_request(
        &self,
        repo: &str,
        title: Option<&str>,
        source: &str,
        target: &str,
    ) -> CustomResult<String> {
        self.change_role(constants::DEV_ROLE)?;
        self.repo_exists(repo)?;

        self.logger.info(format!("Creating PR in AWS: {}", repo));

        let title = match title {
            Some(t) => t.to_string(),
            None => self.get_commit_message()?,
        };

        let command = format!(
            "aws codecommit create-pull-request --title '{0}' --targets repositoryName={1},sourceReference={2},destinationReference={3}",
            title,
            repo,
            source,
            target
        );

        let output = self.execute_zsh_command(&command)?;

        let str_json = String::from_utf8(output.stdout).expect("Failed to parse stdout");
        let commit: Commit = serde_json::from_str(&str_json).expect("Failed to parse json");

        let pr_link = format!(
            "https://console.aws.amazon.com/codesuite/codecommit/repositories/{}/pull-requests/{}/details?region=us-east-1",
            repo,
            commit.pull_request.pull_request_id
        );
        self.logger.info(format!("Created PR in AWS: {}", repo));
        println!("Pull Request Link: {}", pr_link);

        Ok(pr_link)
    }

    pub fn login_npm(&self) -> CustomResult<()> {
        self.logger.info("Logging in to NPM");
        let command = format!("aws codeartifact login --tool npm --repository conform5-npm-common --domain conform --domain-owner {} --region us-east-1 --profile {}", constants::DOMAIN_OWNER, constants::DEV_ROLE);
        self.execute_zsh_command(&command)?;

        self.logger.info("Logged in to NPM");

        Ok(())
    }

    pub fn login_pip(&self) -> CustomResult<()> {
        self.logger.info("Logging in to NPM");
        let command = format!("aws codeartifact login --tool pip --repository conform5-python-common --domain conform5-python --domain-owner {} --region us-east-1 --profile {}", constants::DOMAIN_OWNER, constants::DEV_ROLE);
        self.execute_zsh_command(&command)?;

        self.logger.info("Logged in to NPM");

        Ok(())
    }
    fn get_commit_message(&self) -> CustomResult<String> {
        self.logger.info("Getting commit message");

        let output = self.execute_zsh_command("git log -1 --pretty=%B")?;

        let commit_message = String::from_utf8(output.stdout)
            .map_err(|err| CustomError::CommandExecution(err.to_string()))?;

        self.logger.info("Got commit message");

        Ok(commit_message.trim().to_string())
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
                (cache_entry.startUrl, cache_entry.expiresAt)
            {
                if start_url == sso_start_url && expires_at > Utc::now() {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn repo_exists(&self, repo_name: &str) -> CustomResult<bool> {
        self.logger
            .info(format!("Checking if repository '{}' exists", repo_name));
        let command = format!(
            "aws codecommit get-repository --repository-name {}",
            repo_name
        );
        let output = self.execute_zsh_command(&command);

        match output {
            Ok(_) => {
                self.logger
                    .info(format!("Repository '{}' exists", repo_name));
                Ok(true)
            }
            Err(err) => {
                if err.to_string().contains("RepositoryDoesNotExistException") {
                    self.logger
                        .info(format!("Repository '{}' does not exist", repo_name));
                    Ok(false)
                } else {
                    self.logger.error(format!(
                        "Error checking repository '{}': {}",
                        repo_name, err
                    ));
                    Err(err)
                }
            }
        }
    }
}
