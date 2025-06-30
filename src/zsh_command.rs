use std::process::{Command, Output};

use crate::{
    custom_error::{CustomError, CustomResult},
    logger::Logger,
};

pub struct ZshCommand;

impl ZshCommand {
    pub fn new() -> Self {
        Self {}
    }
    pub fn execute(&self, command: &str) -> CustomResult<Output> {
        let logger = Logger::new();
        let output = Command::new("zsh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|err| CustomError::CommandExecution(err.to_string()))?;

        if !output.status.success() {
            logger.error(format!("Failed to execute command: {}", command));
            logger.error(format!(
                "Error: {}",
                String::from_utf8_lossy(&output.stderr)
            ));

            return Err(CustomError::CommandExecution(
                "Failed to execute command".to_string(),
            ));
        }

        Ok(output)
    }
}
