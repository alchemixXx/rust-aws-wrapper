mod aws;
mod cli;
mod constants;
mod custom_error;
mod location;
mod logger;
mod zsh_command;

use clap::Parser;
use cli::{Cli, Commands};
use custom_error::CustomResult;
use logger::Logger;

#[tokio::main]
async fn main() -> CustomResult<()> {
    Logger::init(logger::LogLevel::Trace);
    let logger = Logger::new();
    let cli = Cli::parse();
    let aws_cli = aws::aws_cli::AwsCli::new();

    match cli.command {
        Commands::CreatePr {
            name,
            source,
            target,
            abort,
        } => {
            let locator = location::Location::new();
            let repo_name = locator.get_repo_name()?;
            logger.info(format!("Repository name: {}", repo_name));
            let result = aws_cli
                .create_pull_request(
                    repo_name.as_str(),
                    name.as_deref(),
                    source.as_deref(),
                    &target,
                    abort,
                )
                .await?;
            logger.info(format!("Pull request created successfully:\n{}", result));
        }
        Commands::Login {} => {
            aws_cli.login()?;
            logger.info("Login successfully completed");
        }
        Commands::LoginNpm {} => {
            logger.info("Logging in to NPM");
            aws_cli.login_npm()?;
            logger.info("Logged in to NPM successfully");
        }
        Commands::LoginPip {} => {
            logger.info("Logging in to PIP");
            aws_cli.login_pip()?;
            logger.info("Logged in to PIP successfully");
        }
        Commands::Morning {} => {
            logger.info("Good morning!");
            aws_cli.login()?;
            aws_cli.login_npm()?;
            logger.info("Have a great day!");
        }
    }

    Ok(())
}
