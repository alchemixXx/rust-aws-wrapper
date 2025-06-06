mod aws;
mod cli;
mod custom_error;
mod location;
mod logger;

use clap::Parser;
use cli::{Cli, Commands};
use custom_error::CustomResult;

#[tokio::main]
async fn main() -> CustomResult<()> {
    crate::logger::Logger::init(logger::LogLevel::Debug);
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
                .create_pull_request(repo_name.as_str(), &name, &source, &target)
                .await?;
            println!("Pull request created successfully:\n{}", result);
        }
        Commands::Login {} => {
            aws_cli.login()?;
            println!("Login successful:\n");
        }
        Commands::ChangeRole { role } => {
            aws_cli.change_role(&role)?;
            println!("Role changed successfully:\n");
        }
    }

    Ok(())
}
