use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    // /// AWS Profile to use
    // #[arg(short, long)]
    // profile: Option<String>,

    // /// AWS Region to use
    // #[arg(short, long)]
    // region: Option<String>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a pull request
    CreatePr {
        /// Title of the pull request
        #[arg(short, long)]
        name: String,

        /// Source branch
        #[arg(short, long)]
        source: String,

        /// Target branch (default: main)
        #[arg(short, long, default_value = "main")]
        target: String,
    },
    Login {},
    /// Change AWS role
    ChangeRole {
        /// Role to change to
        #[arg(
            short,
            long,
            default_value = "conform5-edetek-dev-01.conform5-batch-dev"
        )]
        role: String,
    },
}
