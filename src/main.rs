mod aws;
mod aws_sso;
mod cli;
mod constants;
mod custom_error;
mod location;
mod logger;

use clap::Parser;
use cli::{Cli, Commands};
use custom_error::CustomResult;
use logger::Logger;

#[tokio::main]
async fn main() -> CustomResult<()> {
    Logger::init(logger::LogLevel::Warn);
    let logger = Logger::new();
    let cli = Cli::parse();
    let aws_cli = aws::AwsCli::new();

    match cli.command {
        Commands::CreatePr {
            name,
            source,
            target,
        } => {
            let repo_name = location::get_repo_name()?;
            let result = aws_cli
                .create_pull_request(repo_name.as_str(), name.as_deref(), &source, &target)
                .await?;
            logger.info(format!("Pull request created successfully:\n{}", result));
        }
        Commands::Login {} => {
            aws_cli.login()?;
            logger.info("Login successfully completed");
        }
    }

    Ok(())
}
