use serde::{Deserialize, Serialize};
use std::process::{Command, Output};

use crate::{
    aws_sso::AwsSso,
    constants,
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

        let command = "aws sso login --sso-session sso";
        self.execute_zsh_command(command)?;

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

        Ok(pr_link)
    }

    pub fn login_npm(&self) -> CustomResult<()> {
        self.logger.info("Logging in to NPM");
        let command = format!("aws codeartifact login --tool npm --repository conform5-npm-common --domain conform --domain-owner {} --region us-east-1 --profile {}", constants::DOMAIN_OWNER, constants::DEV_ROLE);
        self.execute_zsh_command(&command)?;

        self.logger.info("Logged in to NPM");

        Ok(())
    }

    //

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
}
