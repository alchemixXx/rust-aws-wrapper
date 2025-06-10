mod aws;
mod cli;
mod constants;
mod custom_error;
mod location;
mod logger;

use clap::Parser;
use cli::{Cli, Commands};
use custom_error::CustomResult;
mod aws_sso;

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
                .create_pull_request(repo_name.as_str(), name.as_deref(), &source, &target)
                .await?;
            println!("Pull request created successfully:\n{}", result);
        }
        Commands::Login {} => {
            aws_cli.login()?;
            println!("Login successful:\n");
        }
        Commands::ChangeRole { role } => {
            // aws_cli.change_role(&role)?;
            aws_cli.change_role(role.as_str())?;
            println!("Role changed successfully:\n");
        }
        Commands::BecomeDev {} => {
            aws_cli.change_role(constants::DEV_ROLE)?;
            println!("Switched to dev role successfully:\n");
        }
    }

    Ok(())
}
