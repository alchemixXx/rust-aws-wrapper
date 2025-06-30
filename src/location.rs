use std::process::Output;

use crate::{
    custom_error::{CustomError, CustomResult},
    zsh_command::ZshCommand,
};

pub struct Location {
    zsh_command: ZshCommand,
}

impl Location {
    pub fn new() -> Self {
        Self {
            zsh_command: ZshCommand::new(),
        }
    }

    pub fn get_repo_name(&self) -> CustomResult<String> {
        let command = "git rev-parse --show-toplevel";
        let output: Output = self.zsh_command.execute(command)?;

        let output_str = String::from_utf8(output.stdout).map_err(|err| {
            CustomError::CommandExecution(format!("Failed to convert output to string: {}", err))
        })?;

        let result = self.get_name_from_output(&output_str)?;

        Ok(result)
    }

    fn get_name_from_output(&self, output: &str) -> CustomResult<String> {
        let repo_name = output
            .trim()
            .split('/')
            .last()
            .ok_or_else(|| {
                CustomError::CommandExecution("Failed to extract repository name".to_string())
            })?
            .to_string();

        Ok(repo_name)
    }
}
