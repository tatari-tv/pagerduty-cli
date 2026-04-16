use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "pd",
    about = "PagerDuty incident management CLI",
    version = env!("GIT_DESCRIBE"),
    after_help = "Requires: PAGERDUTY_API_TOKEN env var, --api-token flag, or api-token in ~/.config/pagerduty-cli/pagerduty-cli.yml\nLogs: ~/.local/share/pagerduty-cli/logs/pagerduty-cli.log"
)]
pub struct Cli {
    /// Path to config file
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// PagerDuty API token (overrides env and config file)
    #[arg(long)]
    pub api_token: Option<String>,

    /// Output format
    #[arg(long, value_enum)]
    pub output: Option<OutputFormat>,

    /// Log level
    #[arg(short, long)]
    pub log_level: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Raw REST API passthrough for ad-hoc exploration
    Rest {
        /// HTTP method (GET, POST, PUT, DELETE)
        method: String,
        /// API path (e.g., /incidents/types)
        path: String,
        /// JSON request body
        #[arg(long)]
        body: Option<String>,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Auto,
    Json,
    Table,
}
