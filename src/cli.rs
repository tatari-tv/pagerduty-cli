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
    /// Manage incident workflows (WF1-WF5)
    #[command(name = "incident-workflow")]
    IncidentWorkflow {
        #[command(subcommand)]
        action: IncidentWorkflowAction,
    },
    /// Manage workflow triggers
    Trigger {
        #[command(subcommand)]
        action: TriggerAction,
    },
    /// Discover available workflow actions and their input schemas
    Action {
        #[command(subcommand)]
        action: ActionAction,
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

#[derive(Subcommand, Debug)]
pub enum IncidentWorkflowAction {
    /// List incident workflows
    List {
        /// Filter by query string
        #[arg(long)]
        query: Option<String>,
    },
    /// Get an incident workflow by ID
    Get {
        id: String,
        /// Include steps in response
        #[arg(long)]
        include_steps: bool,
    },
    /// Create a new incident workflow
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
        /// Create from a YAML workflow definition file
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
    },
    /// Update an existing incident workflow
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete an incident workflow
    Delete { id: String },
    /// Enable an incident workflow
    Enable { id: String },
    /// Disable an incident workflow
    Disable { id: String },
    /// Export a workflow to YAML (including trigger)
    Export { id: String },
    /// Import a workflow from YAML definition (create or update workflow + trigger)
    Import {
        /// Path to YAML workflow definition file
        file: PathBuf,
        /// Use workflow ID instead of name for lookup (required if duplicate names exist)
        #[arg(long)]
        id: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum TriggerAction {
    /// List workflow triggers
    List,
    /// Get a workflow trigger by ID
    Get { id: String },
    /// Create a workflow trigger
    Create {
        /// Workflow ID to bind this trigger to
        #[arg(long = "workflow-id")]
        workflow_id: String,
        /// Trigger type
        #[arg(long = "type", value_enum)]
        trigger_type: TriggerType,
        /// PCL condition string (for conditional type)
        #[arg(long)]
        condition: Option<String>,
        /// Comma-separated incident type IDs (for incident_type type)
        #[arg(long = "incident-types", value_delimiter = ',')]
        incident_types: Option<Vec<String>>,
    },
    /// Update a workflow trigger
    Update {
        id: String,
        /// PCL condition string
        #[arg(long)]
        condition: Option<String>,
        /// Comma-separated incident type IDs
        #[arg(long = "incident-types", value_delimiter = ',')]
        incident_types: Option<Vec<String>>,
    },
    /// Delete a workflow trigger
    Delete { id: String },
    /// Associate a trigger with a service
    #[command(name = "create-for-service")]
    CreateForService {
        /// Trigger ID
        trigger_id: String,
        /// Service ID to associate
        #[arg(long = "service-id")]
        service_id: String,
    },
    /// Remove a trigger from a service
    #[command(name = "remove-from-service")]
    RemoveFromService {
        /// Trigger ID
        trigger_id: String,
        /// Service ID to remove
        #[arg(long = "service-id")]
        service_id: String,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum TriggerType {
    Conditional,
    Manual,
    #[value(name = "incident-type")]
    IncidentType,
}

#[derive(Subcommand, Debug)]
pub enum ActionAction {
    /// List available workflow actions
    List {
        /// Filter by query string
        #[arg(long)]
        query: Option<String>,
    },
    /// Get details of a workflow action (including input/output schema)
    Get { id: String },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Auto,
    Json,
    Table,
}
