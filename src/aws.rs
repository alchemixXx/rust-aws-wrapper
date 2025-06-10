use serde::{Deserialize, Serialize};
use std::process::{Command, Output};

use crate::{
    aws_sso::AwsSso,
    custom_error::{CustomError, CustomResult},
    logger::Logger,
};

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

pub struct AwsCli {}

impl AwsCli {
    pub fn new() -> Self {
        Self {}
    }

    fn execute_zsh_command(&self, command: &str) -> CustomResult<Output> {
        let logger = Logger::new();
        let output = Command::new("zsh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|err| CustomError::CommandExecution(err.to_string()))?;

        if !output.status.success() {
            logger.error(format!("Failed to execute command: {}", command).as_str());
            logger.error(format!("Error: {}", String::from_utf8_lossy(&output.stderr)).as_str());

            return Err(CustomError::CommandExecution(
                "Failed to execute command".to_string(),
            ));
        }

        Ok(output)
    }

    pub fn login(self) -> CustomResult<()> {
        let logger = Logger::new();
        logger.info(format!("Logging in to AWS").as_str());
        let command = format!("aws sso login --sso-session sso",);

        self.execute_zsh_command(&command)?;

        Ok(())
    }

    /*
    NOT WORKING YET
     */
    pub fn change_role(self, role: &str) -> CustomResult<()> {
        // aws sso login --sso-session sso
        let logger = Logger::new();
        logger.info(format!("Changing AWS role to '{}'", role).as_str());
        AwsSso::new(role.to_string()).set_sso_credentials()?;
        logger.info(format!("Changed AWS role to '{}'", role).as_str());

        Ok(())
    }

    pub async fn create_pull_request(
        &self,
        repo: &str,
        title: Option<&str>,
        source: &str,
        target: &str,
    ) -> CustomResult<String> {
        let logger = Logger::new();
        logger.info(format!("Creating PR in AWS: {}", repo).as_str());

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

        Ok(pr_link)
    }

    fn get_commit_message(&self) -> CustomResult<String> {
        let logger = Logger::new();
        logger.info("Getting commit message");

        let output = self.execute_zsh_command("git log -1 --pretty=%B")?;

        let commit_message = String::from_utf8(output.stdout)
            .map_err(|err| CustomError::CommandExecution(err.to_string()))?;

        Ok(commit_message.trim().to_string())
    }
}
