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

    /// Bypass the local name-to-ID cache for this invocation. Forces every
    /// `resolve_*` helper to hit the API, even when a recent mapping exists.
    #[arg(long = "no-cache", global = true)]
    pub no_cache: bool,

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
    /// Manage incidents - list/get/create/update, notes, alerts, types, workflows, triggers
    Incident {
        #[command(subcommand)]
        action: IncidentCommands,
    },
    /// View and verify priority configuration
    Priority {
        #[command(subcommand)]
        action: PriorityAction,
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
    /// Manage maintenance windows
    Maintenance {
        #[command(subcommand)]
        action: MaintenanceAction,
    },
    /// Manage alert grouping settings
    #[command(name = "alert-grouping")]
    AlertGrouping {
        #[command(subcommand)]
        action: AlertGroupingAction,
    },
    /// View and update event orchestrations (including routers)
    Orchestration {
        #[command(subcommand)]
        action: OrchestrationAction,
    },
    /// View log entries
    Log {
        #[command(subcommand)]
        action: LogAction,
    },
    /// List and view change events
    Change {
        #[command(subcommand)]
        action: ChangeAction,
    },
    /// Manage the local name-to-ID cache (stored per PD subdomain under
    /// `~/.cache/pd/ids/`).
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
    /// Token and subdomain onboarding helpers (no API token required)
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuthAction {
    /// Show where the current API token was loaded from (or that none was found)
    Status,
}

#[derive(Subcommand, Debug)]
pub enum CacheAction {
    /// Clear cached name-to-ID mappings. With no argument, clears this
    /// subdomain's cache; with a resource type (e.g. `service`, `team`),
    /// clears only that type's subtree. `--all-accounts` wipes every
    /// subdomain's cache.
    Clear {
        /// Optional resource type to scope the clear
        resource_type: Option<String>,
        /// Wipe every subdomain's cache (not just the current one)
        #[arg(long = "all-accounts")]
        all_accounts: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum IncidentCommands {
    /// List incidents (default: triggered+acknowledged in the last 1 day)
    List {
        /// Zero or more title patterns (exact -> starts-with -> contains)
        patterns: Vec<String>,
        /// Filter by status (repeatable)
        #[arg(long, value_enum)]
        status: Vec<IncidentStatus>,
        /// Filter by priority name (e.g. P1 P2)
        #[arg(long, num_args = 1..)]
        priority: Vec<String>,
        /// Filter by owning team (tiered name match)
        #[arg(long)]
        team: Option<String>,
        /// ISO-8601 lower bound
        #[arg(long)]
        since: Option<String>,
        /// ISO-8601 upper bound
        #[arg(long)]
        until: Option<String>,
    },
    /// Get an incident by ID or incident number
    Get { id: String },
    /// Create a new incident
    Create {
        #[arg(long)]
        title: Option<String>,
        /// Service pattern (tiered match) or ID
        #[arg(long)]
        service: Option<String>,
        /// Priority name (e.g. P1)
        #[arg(long)]
        priority: Option<String>,
        /// Incident type display name or slug
        #[arg(long = "type")]
        incident_type: Option<String>,
        /// Initial incident body text
        #[arg(long)]
        body: Option<String>,
        /// Requester email (overrides config/env)
        #[arg(long = "from")]
        from_email: Option<String>,
        /// Create from a YAML incident definition ('-' for stdin)
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
        /// Print a commented YAML skeleton and exit
        #[arg(long)]
        example: bool,
    },
    /// Update an existing incident
    Update {
        id: String,
        /// New status (acknowledged or resolved)
        #[arg(long, value_enum)]
        status: Option<IncidentStatus>,
        /// Priority name (e.g. P1); empty string clears it
        #[arg(long)]
        priority: Option<String>,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// Requester email (overrides config/env)
        #[arg(long = "from")]
        from_email: Option<String>,
    },
    /// Manage notes on an incident
    Note {
        #[command(subcommand)]
        action: IncidentNoteAction,
    },
    /// View alerts attached to an incident
    Alert {
        #[command(subcommand)]
        action: IncidentAlertAction,
    },
    /// Manage incident workflow triggers
    Trigger {
        #[command(subcommand)]
        action: IncidentTriggerAction,
    },
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

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum IncidentStatus {
    Triggered,
    Acknowledged,
    Resolved,
}

#[derive(Subcommand, Debug)]
pub enum IncidentNoteAction {
    /// List notes on an incident
    List { incident_id: String },
    /// Add a note to an incident
    Add {
        incident_id: String,
        /// Note text (use '-' to read from stdin)
        text: String,
        /// Requester email (overrides config/env)
        #[arg(long = "from")]
        from_email: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum IncidentAlertAction {
    /// List alerts attached to an incident
    List { incident_id: String },
    /// Get a single alert by ID
    Get { incident_id: String, alert_id: String },
}

#[derive(Subcommand, Debug)]
pub enum IncidentTriggerAction {
    /// List workflow triggers (patterns match workflow name)
    List {
        /// Zero or more workflow-name patterns
        patterns: Vec<String>,
    },
    /// Get a workflow trigger by ID
    Get { id: String },
    /// Create a workflow trigger
    Create {
        /// Workflow ID to bind this trigger to
        #[arg(long)]
        workflow: String,
        /// Trigger type
        #[arg(long = "type", value_enum)]
        trigger_type: TriggerType,
        /// PCL condition string (for conditional type)
        #[arg(long)]
        condition: Option<String>,
        /// Comma-separated incident type IDs or display names (for incident-type triggers)
        #[arg(long = "incident-types", value_delimiter = ',')]
        incident_types: Option<Vec<String>>,
    },
    /// Update a workflow trigger
    Update {
        id: String,
        #[arg(long)]
        condition: Option<String>,
        #[arg(long = "incident-types", value_delimiter = ',')]
        incident_types: Option<Vec<String>>,
    },
    /// Delete a workflow trigger
    Delete { id: String },
    /// Associate a trigger with a service
    Bind {
        trigger_id: String,
        /// Service pattern (tiered match) or ID
        #[arg(long)]
        service: String,
    },
    /// Remove a trigger from a service
    Unbind {
        trigger_id: String,
        /// Service pattern (tiered match) or ID
        #[arg(long)]
        service: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum IncidentTypeAction {
    /// List incident types
    List {
        /// Zero or more display-name patterns (exact -> starts-with -> contains)
        patterns: Vec<String>,
        /// Filter by enabled/disabled status
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
        /// Parent incident type ID, slug, or display name (e.g. "Base Incident")
        #[arg(long)]
        parent: Option<String>,
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
        /// Zero or more name patterns (exact -> starts-with -> contains)
        patterns: Vec<String>,
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
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Create from a YAML workflow definition file
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
        /// Print a commented YAML skeleton and exit
        #[arg(long)]
        example: bool,
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
        /// Zero or more patterns matching the action's function_name
        patterns: Vec<String>,
    },
    /// Get details of a workflow action (including input/output schema)
    Get { id: String },
}

#[derive(Subcommand, Debug)]
pub enum UserAction {
    /// List users, optionally filtered by name patterns
    List {
        /// Zero or more name patterns (exact -> starts-with -> contains, OR within a tier)
        patterns: Vec<String>,
        /// Only return users on the given team (tiered name match)
        #[arg(long)]
        team: Option<String>,
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
        /// Only return policies owned by the given team (tiered name match)
        #[arg(long)]
        team: Option<String>,
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
    List {
        patterns: Vec<String>,
        /// Only return services owned by the given team (tiered name match)
        #[arg(long)]
        team: Option<String>,
    },
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

#[derive(Subcommand, Debug)]
pub enum MaintenanceAction {
    /// List maintenance windows, optionally filtered by name patterns
    List {
        /// Zero or more description patterns (exact -> starts-with -> contains)
        patterns: Vec<String>,
        /// Filter by owning team (tiered name match)
        #[arg(long)]
        team: Option<String>,
        /// Filter by affected service (tiered name match)
        #[arg(long)]
        service: Option<String>,
    },
    /// Get a maintenance window by ID
    Get { id: String },
    /// Create a new maintenance window
    Create {
        /// Service pattern (tiered match); repeatable
        #[arg(long, num_args = 1..)]
        service: Vec<String>,
        /// ISO-8601 start time
        #[arg(long)]
        start: Option<String>,
        /// ISO-8601 end time
        #[arg(long)]
        end: Option<String>,
        /// Description shown on the window
        #[arg(long)]
        description: Option<String>,
        /// Create from a YAML maintenance window definition ('-' for stdin)
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
        /// Print a commented YAML skeleton and exit
        #[arg(long)]
        example: bool,
    },
    /// Update an existing maintenance window
    Update {
        id: String,
        #[arg(long)]
        start: Option<String>,
        #[arg(long)]
        end: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete a maintenance window
    Delete { id: String },
}

#[derive(Subcommand, Debug)]
pub enum AlertGroupingAction {
    /// List alert grouping settings, optionally filtered by name patterns
    List {
        /// Zero or more name patterns
        patterns: Vec<String>,
    },
    /// Get an alert grouping setting by ID
    Get { id: String },
    /// Create an alert grouping setting
    Create {
        /// Service pattern (tiered match); repeatable
        #[arg(long, num_args = 1..)]
        service: Vec<String>,
        /// Grouping type (e.g. intelligent, content_based, time)
        #[arg(long = "type")]
        grouping_type: Option<String>,
        /// Setting name
        #[arg(long)]
        name: Option<String>,
        /// Create from a YAML alert grouping definition ('-' for stdin)
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
        /// Print a commented YAML skeleton and exit
        #[arg(long)]
        example: bool,
    },
    /// Update an alert grouping setting
    Update {
        id: String,
        /// Update from a YAML alert grouping definition ('-' for stdin)
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
    },
    /// Delete an alert grouping setting
    Delete { id: String },
}

#[derive(Subcommand, Debug)]
pub enum OrchestrationAction {
    /// List event orchestrations, optionally filtered by name patterns
    List {
        /// Zero or more name patterns
        patterns: Vec<String>,
    },
    /// Get an event orchestration by ID or name
    Get { name_or_id: String },
    /// Manage an orchestration's router rules
    Router {
        #[command(subcommand)]
        action: OrchestrationRouterAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum OrchestrationRouterAction {
    /// Get the router definition for an orchestration
    Get { orchestration: String },
    /// Update the router definition for an orchestration
    Update {
        orchestration: String,
        /// YAML or JSON file containing the router definition ('-' for stdin)
        #[arg(long = "from-file")]
        from_file: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum LogAction {
    /// List log entries (default window: last 24 hours when --since is omitted)
    List {
        /// Zero or more summary patterns (exact -> starts-with -> contains)
        patterns: Vec<String>,
        /// ISO-8601 lower bound (default: 24 hours ago)
        #[arg(long)]
        since: Option<String>,
        /// ISO-8601 upper bound
        #[arg(long)]
        until: Option<String>,
    },
    /// Get a log entry by ID
    Get { id: String },
}

#[derive(Subcommand, Debug)]
pub enum ChangeAction {
    /// List change events
    List {
        /// Zero or more summary patterns (exact -> starts-with -> contains)
        patterns: Vec<String>,
        /// Filter by affected service (tiered name match)
        #[arg(long)]
        service: Option<String>,
        /// ISO-8601 lower bound
        #[arg(long)]
        since: Option<String>,
        /// ISO-8601 upper bound
        #[arg(long)]
        until: Option<String>,
    },
    /// Get a change event by ID
    Get { id: String },
    /// Create (enqueue) a change event via the Events API v2
    Create {
        /// One-line summary of the change (stored on the event)
        #[arg(long)]
        summary: Option<String>,
        /// Service the change applies to. Drives dynamic routing-key
        /// lookup and is copied into `payload.source`.
        #[arg(long)]
        service: Option<String>,
        /// Optional link, repeatable, shape `"url|text"` (pipe-separated)
        #[arg(long, num_args = 0..)]
        links: Vec<String>,
        /// Explicit routing key override. When set, skips the dynamic
        /// lookup on --service. Can also come from PAGERDUTY_ROUTING_KEY
        /// or the config file's `routing-key` field.
        #[arg(long = "routing-key")]
        routing_key: Option<String>,
        /// Load payload fields (summary, source, custom_details, links,
        /// timestamp) from a YAML file. Routing key never comes from
        /// the file -- it's a secret.
        #[arg(long = "from-file")]
        from_file: Option<PathBuf>,
        /// Print the YAML skeleton and exit. Does not hit the API.
        #[arg(long)]
        example: bool,
    },
}
