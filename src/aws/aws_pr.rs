use serde::{Deserialize, Serialize};

use crate::{
    custom_error::{CustomError, CustomResult},
    logger::Logger,
    zsh_command::ZshCommand,
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PullRequestMergeConflicts {
    mergeable: bool,
}

pub struct AwsPr {
    logger: Logger,
    zsh_command: ZshCommand,
}

impl AwsPr {
    pub fn new() -> Self {
        Self {
            logger: Logger::new(),
            zsh_command: ZshCommand::new(),
        }
    }

    pub async fn create(
        &self,
        repo: &str,
        title: Option<&str>,
        source_branch: Option<&str>,
        target: &str,
    ) -> CustomResult<String> {
        self.repo_exists(repo)?;

        self.logger.debug(format!("Creating PR in AWS: {}", repo));

        let title = match title {
            Some(t) => t.to_string(),
            None => self.get_commit_message()?,
        };

        let source = match source_branch {
            Some(s) => s.to_string(),
            None => self.get_current_branch()?,
        };

        self.check_merge_conflicts(repo, target, &source)?;

        let command = format!(
            "aws codecommit create-pull-request --title '{0}' --targets repositoryName={1},sourceReference={2},destinationReference={3}",
            title,
            repo,
            source,
            target
        );

        let output = self.zsh_command.execute(&command)?;

        let str_json = String::from_utf8(output.stdout).expect("Failed to parse stdout");
        let commit: Commit = serde_json::from_str(&str_json).expect("Failed to parse json");

        let pr_link = format!(
            "https://console.aws.amazon.com/codesuite/codecommit/repositories/{}/pull-requests/{}/details?region=us-east-1",
            repo,
            commit.pull_request.pull_request_id
        );
        self.logger.debug(format!("Created PR in AWS: {}", repo));

        Ok(pr_link)
    }

    fn check_merge_conflicts(
        &self,
        repo_name: &str,
        target_branch: &str,
        source_branch: &str,
    ) -> CustomResult<()> {
        self.logger.debug("Checking for merge conflicts");

        let command = format!(
            "aws codecommit get-merge-conflicts --repository-name {0} --destination-commit {1} --source-commit-specifier {2} --merge-option FAST_FORWARD_MERGE", repo_name, target_branch, source_branch
        );

        let output = self.zsh_command.execute(&command)?;

        let conflicts = String::from_utf8(output.stdout)
            .map_err(|err| CustomError::CommandExecution(err.to_string()))?;

        let conflicts_output: PullRequestMergeConflicts = match serde_json::from_str(&conflicts) {
            Ok(c) => c,
            Err(e) => {
                self.logger.error(format!(
                    "Can't parse merge conflicts output {:?}: {:?}",
                    e, conflicts
                ));

                return Err(CustomError::CommandExecution(
                    "Failed to parse merge conflicts".to_string(),
                ));
            }
        };

        if conflicts_output.mergeable {
            self.logger.debug("No merge conflicts found");
        } else {
            self.logger.warn(format!(
                "\n\n\nSource ({0}) and target ({1}) can't be merged due to conflicts\n\n\n",
                source_branch, target_branch
            ));
        }

        Ok(())
    }

    fn get_commit_message(&self) -> CustomResult<String> {
        self.logger.debug("Getting commit message");

        let output = self.zsh_command.execute("git log -1 --pretty=%B")?;

        let commit_message = String::from_utf8(output.stdout)
            .map_err(|err| CustomError::CommandExecution(err.to_string()))?;

        self.logger.debug("Got commit message");

        Ok(commit_message.trim().to_string())
    }

    fn get_current_branch(&self) -> CustomResult<String> {
        self.logger.debug("Getting current branch");

        let output = self.zsh_command.execute("git branch --show-current")?;

        let commit_message = String::from_utf8(output.stdout)
            .map_err(|err| CustomError::CommandExecution(err.to_string()))?;

        self.logger.debug("Got current branch");

        Ok(commit_message.trim().to_string())
    }

    fn repo_exists(&self, repo_name: &str) -> CustomResult<bool> {
        self.logger
            .debug(format!("Checking if repository '{}' exists", repo_name));
        let command = format!(
            "aws codecommit get-repository --repository-name {}",
            repo_name
        );
        let output = self.zsh_command.execute(&command);

        match output {
            Ok(_) => {
                self.logger
                    .debug(format!("Repository '{}' exists", repo_name));
                Ok(true)
            }
            Err(err) => {
                if err.to_string().contains("RepositoryDoesNotExistException") {
                    self.logger
                        .error(format!("Repository '{}' does not exist", repo_name));
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
