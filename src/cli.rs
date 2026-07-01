use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a pull request
    CreatePr {
        /// Title of the pull request
        #[arg(short, long)]
        name: Option<String>,

        /// Source branch
        #[arg(short, long)]
        source: Option<String>,

        /// Target branch
        #[arg(short, long)]
        target: String,

        /// Abort on merge conflicts
        #[arg(short, long, default_value_t = false)]
        abort: bool,
    },
    Login {},
    LoginNpm {},
    LoginPip {},
    Morning {},
    /// Fetch CloudWatch log events and save to logs.json
    Logs {},
    /// Fetch CloudWatch log events (provide AWS credentials directly, no SSO)
    LogsRaw {},
}
