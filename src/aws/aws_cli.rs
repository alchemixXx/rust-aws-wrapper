use serde::{Deserialize, Serialize};

use crate::{
    aws::aws_pr::AwsPr, constants, custom_error::CustomResult, logger::Logger,
    zsh_command::ZshCommand,
};

use super::aws_sso::AwsSso;

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
    zsh_command: ZshCommand,
}

impl AwsCli {
    pub fn new() -> Self {
        Self {
            logger: Logger::new(),
            zsh_command: ZshCommand::new(),
        }
    }

    pub fn login(&self) -> CustomResult<()> {
        self.logger.info("Logging in to AWS");
        AwsSso::new(constants::DEV_ROLE.to_string()).login()?;
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
        source_branch: Option<&str>,
        target: &str,
    ) -> CustomResult<String> {
        self.logger
            .info(format!("Creating pull request in AWS: {}", repo));
        self.change_role(constants::DEV_ROLE)?;
        let pr_link = AwsPr::new()
            .create(repo, title, source_branch, target)
            .await?;

        self.logger
            .info(format!("Created PR in AWS: {} : '{}'", repo, pr_link));
        println!("Pull Request Link: {}", pr_link);

        Ok(pr_link)
    }

    pub fn login_npm(&self) -> CustomResult<()> {
        self.logger.info("Logging in to NPM");
        let command = format!("aws codeartifact login --tool npm --repository conform5-npm-common --domain conform --domain-owner {} --region us-east-1 --profile {}", constants::DOMAIN_OWNER, constants::DEV_ROLE);
        self.zsh_command.execute(&command)?;

        self.logger.info("Logged in to NPM");

        Ok(())
    }

    pub fn login_pip(&self) -> CustomResult<()> {
        self.logger.info("Logging in to NPM");
        let command = format!("aws codeartifact login --tool pip --repository conform5-python-common --domain conform5-python --domain-owner {} --region us-east-1 --profile {}", constants::DOMAIN_OWNER, constants::DEV_ROLE);
        self.zsh_command.execute(&command)?;

        self.logger.info("Logged in to NPM");

        Ok(())
    }
}
