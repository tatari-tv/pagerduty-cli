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
    /// Manage incidents - types and workflows
    Incident {
        #[command(subcommand)]
        action: IncidentCommands,
    },
    /// View and verify priority configuration
    Priority {
        #[command(subcommand)]
        action: PriorityAction,
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
    /// Look up PagerDuty users
    User {
        #[command(subcommand)]
        action: UserAction,
    },
    /// Manage teams and their membership
    Team {
        #[command(subcommand)]
        action: TeamAction,
    },
    /// Manage on-call schedules and overrides
    Schedule {
        #[command(subcommand)]
        action: ScheduleAction,
    },
    /// Manage escalation policies
    Escalation {
        #[command(subcommand)]
        action: EscalationAction,
    },
    /// Manage services and their integrations
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
    /// Show who is currently on call
    Oncall {
        #[command(subcommand)]
        action: OncallAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum IncidentCommands {
    /// Manage incident types
    Type {
        #[command(subcommand)]
        action: IncidentTypeAction,
    },
    /// Manage incident workflows
    Workflow {
        #[command(subcommand)]
        action: IncidentWorkflowAction,
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
    /// Export a workflow to YAML (including trigger).
    ///
    /// If the listed workflow has no steps and no triggers, automatically
    /// falls back to searching the global trigger list for a workflow with
    /// the same name (PagerDuty sometimes stores the real workflow under a
    /// different ID than the one returned by `incident-workflow list`).
    /// Pass `--real-id` to skip the fallback and export a specific ID.
    Export {
        id: String,
        /// Explicit workflow ID to export, bypassing the stub/shadow fallback.
        #[arg(long = "real-id")]
        real_id: Option<String>,
    },
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
    List,
    /// Get details of a workflow action (including input/output schema)
    Get { id: String },
}

#[derive(Subcommand, Debug)]
pub enum UserAction {
    /// List users, optionally filtered by name patterns
    List {
        /// Zero or more name patterns (exact -> starts-with -> contains, OR within a tier)
        patterns: Vec<String>,
    },
    /// Get one user by PagerDuty ID or email
    Get { email_or_id: String },
}

#[derive(Subcommand, Debug)]
pub enum TeamAction {
    /// List teams, optionally filtered by name patterns
    List {
        /// Zero or more name patterns (exact -> starts-with -> contains)
        patterns: Vec<String>,
    },
    /// Get one team by ID, slug, or display name
    Get { name_or_id: String },
    /// Create a new team
    Create {
        /// Team name (human-readable)
        #[arg(long)]
        name: Option<String>,
        /// Team description
        #[arg(long)]
        description: Option<String>,
        /// Create from a YAML team definition file ('-' for stdin)
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
        /// Print a commented YAML skeleton and exit
        #[arg(long)]
        example: bool,
    },
    /// Update an existing team
    Update {
        /// Team ID, slug, or display name
        name_or_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Update from a YAML team definition file ('-' for stdin)
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
    },
    /// Delete a team
    Delete {
        /// Team ID, slug, or display name
        name_or_id: String,
    },
    /// Manage team membership
    Member {
        #[command(subcommand)]
        action: TeamMemberAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum TeamMemberAction {
    /// List team members, optionally filtered by name patterns
    List {
        /// Team ID, slug, or display name
        team: String,
        /// Zero or more name patterns
        patterns: Vec<String>,
    },
    /// Add a user to the team
    Add {
        /// Team ID, slug, or display name
        team: String,
        /// User ID or email
        user: String,
        /// Team role for this user
        #[arg(long, value_enum, default_value = "responder")]
        role: TeamMemberRole,
    },
    /// Remove a user from the team
    Remove {
        /// Team ID, slug, or display name
        team: String,
        /// User ID or email
        user: String,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum TeamMemberRole {
    Observer,
    Responder,
    Manager,
}

#[derive(Subcommand, Debug)]
pub enum ScheduleAction {
    /// List schedules, optionally filtered by name patterns
    List {
        /// Zero or more name patterns
        patterns: Vec<String>,
    },
    /// Get one schedule by ID or name
    Get { name_or_id: String },
    /// Create a new schedule (most users want --from-file; see --example)
    Create {
        /// Schedule name
        #[arg(long)]
        name: Option<String>,
        /// IANA time zone (e.g., America/Los_Angeles)
        #[arg(long)]
        timezone: Option<String>,
        /// Create from a YAML schedule definition file ('-' for stdin)
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
        /// Print a commented YAML skeleton and exit
        #[arg(long)]
        example: bool,
    },
    /// Update an existing schedule
    Update {
        name_or_id: String,
        /// Update from a YAML schedule definition file ('-' for stdin)
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
    },
    /// Delete a schedule
    Delete { name_or_id: String },
    /// Manage schedule overrides
    Override {
        #[command(subcommand)]
        action: ScheduleOverrideAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ScheduleOverrideAction {
    /// List overrides for a schedule
    List {
        schedule: String,
        /// ISO-8601 lower bound (e.g., 2026-01-01T00:00:00Z)
        #[arg(long)]
        since: Option<String>,
        /// ISO-8601 upper bound
        #[arg(long)]
        until: Option<String>,
    },
    /// Create an override on a schedule
    Create {
        schedule: String,
        /// User ID or email taking the override
        #[arg(long)]
        user: String,
        /// ISO-8601 start time
        #[arg(long)]
        start: String,
        /// ISO-8601 end time
        #[arg(long)]
        end: String,
    },
    /// Delete an override by its ID
    Delete { schedule: String, override_id: String },
}

#[derive(Subcommand, Debug)]
pub enum EscalationAction {
    /// List escalation policies, optionally filtered by name patterns
    List {
        /// Zero or more name patterns
        patterns: Vec<String>,
    },
    /// Get one escalation policy by ID or name
    Get { name_or_id: String },
    /// Create a new escalation policy (real configs need --from-file; see --example)
    Create {
        /// Policy name
        #[arg(long)]
        name: Option<String>,
        /// Team pattern the policy belongs to (resolved via tiered match)
        #[arg(long)]
        team: Option<String>,
        /// Create from a YAML definition ('-' for stdin)
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
        /// Print a commented YAML skeleton and exit
        #[arg(long)]
        example: bool,
    },
    /// Update an existing escalation policy
    Update {
        name_or_id: String,
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
    },
    /// Delete an escalation policy
    Delete { name_or_id: String },
}

#[derive(Subcommand, Debug)]
pub enum ServiceAction {
    /// List services, optionally filtered by name patterns
    List { patterns: Vec<String> },
    /// Get one service by ID or name
    Get { name_or_id: String },
    /// Create a new service
    Create {
        #[arg(long)]
        name: Option<String>,
        /// Escalation policy pattern (tiered match)
        #[arg(long)]
        escalation: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
        #[arg(long)]
        example: bool,
    },
    /// Update a service
    Update {
        name_or_id: String,
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
    },
    /// Delete a service
    Delete { name_or_id: String },
    /// Manage service integrations
    Integration {
        #[command(subcommand)]
        action: ServiceIntegrationAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ServiceIntegrationAction {
    /// List integrations on a service
    List { service: String, patterns: Vec<String> },
    /// Get one integration
    Get { service: String, integration_id: String },
    /// Create an integration on a service
    Create {
        service: String,
        /// Integration type (e.g. generic_events_api_inbound_integration, events_api_v2_inbound_integration)
        #[arg(long = "type")]
        integration_type: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
        #[arg(long)]
        example: bool,
    },
    /// Delete an integration
    Delete { service: String, integration_id: String },
}

#[derive(Subcommand, Debug)]
pub enum OncallAction {
    /// List current on-calls, optionally filtered by schedule/EP/user name
    List {
        /// Zero or more name patterns (match schedule, escalation policy, or user name)
        patterns: Vec<String>,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Auto,
    Json,
    Table,
}
