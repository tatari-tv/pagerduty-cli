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
    /// Manage incident types
    #[command(name = "incident-type")]
    IncidentType {
        #[command(subcommand)]
        action: IncidentTypeAction,
    },
    /// View and verify priority configuration
    Priority {
        #[command(subcommand)]
        action: PriorityAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum IncidentTypeAction {
    /// List incident types
    List {
        /// Filter by status
        #[arg(long, value_enum, default_value = "all")]
        filter: TypeFilter,
    },
    /// Get an incident type by ID or name
    Get { id_or_name: String },
    /// Create a new incident type
    Create {
        #[arg(long)]
        name: String,
        #[arg(long = "display-name")]
        display_name: String,
        #[arg(long)]
        description: Option<String>,
    },
    /// Update an existing incident type
    Update {
        id_or_name: String,
        #[arg(long = "display-name")]
        display_name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Set enabled state (true or false)
        #[arg(long)]
        enabled: Option<bool>,
    },
    /// Manage custom fields on an incident type
    Field {
        #[command(subcommand)]
        action: FieldAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum FieldAction {
    /// List custom fields for an incident type
    List { type_id_or_name: String },
    /// Create a custom field on an incident type
    Create {
        type_id_or_name: String,
        #[arg(long)]
        name: String,
        #[arg(long = "data-type")]
        data_type: String,
        #[arg(long = "field-type")]
        field_type: String,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum TypeFilter {
    Enabled,
    Disabled,
    All,
}

#[derive(Subcommand, Debug)]
pub enum PriorityAction {
    /// List all priority levels
    List,
    /// Verify P1-P4 priorities match Tatari severity matrix
    Verify,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Auto,
    Json,
    Table,
}
