use assert_cmd::Command;
use predicates::prelude::*;

fn pd() -> Command {
    Command::cargo_bin("pd").expect("binary not found")
}

// ---------------------------------------------------------------------------
// Top-level binary behavior
// ---------------------------------------------------------------------------

#[test]
fn smoke_no_args_shows_usage() {
    pd().assert().failure().stderr(predicate::str::contains("Usage"));
}

#[test]
fn smoke_help_flag() {
    pd().arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("PagerDuty incident management CLI"))
        .stdout(predicate::str::contains("incident"))
        .stdout(predicate::str::contains("trigger"))
        .stdout(predicate::str::contains("action"))
        .stdout(predicate::str::contains("priority"))
        .stdout(predicate::str::contains("rest"));
}

#[test]
fn smoke_version_flag() {
    pd().arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("pd"));
}

// ---------------------------------------------------------------------------
// Subcommand help: incident (parent for type, workflow)
// ---------------------------------------------------------------------------

#[test]
fn smoke_incident_help() {
    pd().args(["incident", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("type"))
        .stdout(predicate::str::contains("workflow"));
}

#[test]
fn smoke_incident_type_help() {
    pd().args(["incident", "type", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("update"))
        .stdout(predicate::str::contains("field"));
}

#[test]
fn smoke_incident_type_list_help() {
    pd().args(["incident", "type", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--filter"));
}

#[test]
fn smoke_incident_type_create_help() {
    pd().args(["incident", "type", "create", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--name"))
        .stdout(predicate::str::contains("--display-name"));
}

#[test]
fn smoke_incident_type_field_help() {
    pd().args(["incident", "type", "field", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("create"));
}

// ---------------------------------------------------------------------------
// Subcommand help: priority
// ---------------------------------------------------------------------------

#[test]
fn smoke_priority_help() {
    pd().args(["priority", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("verify"));
}

// ---------------------------------------------------------------------------
// Subcommand help: incident workflow
// ---------------------------------------------------------------------------

#[test]
fn smoke_incident_workflow_help() {
    pd().args(["incident", "workflow", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("update"))
        .stdout(predicate::str::contains("delete"))
        .stdout(predicate::str::contains("enable"))
        .stdout(predicate::str::contains("disable"))
        .stdout(predicate::str::contains("export"))
        .stdout(predicate::str::contains("import"));
}

#[test]
fn smoke_incident_workflow_create_help() {
    pd().args(["incident", "workflow", "create", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--name"))
        .stdout(predicate::str::contains("--from-file"))
        .stdout(predicate::str::contains("--example"));
}

#[test]
fn smoke_incident_workflow_create_example_flag() {
    pd().args(["incident", "workflow", "create", "--example"])
        .env_remove("PAGERDUTY_API_TOKEN")
        .assert()
        .success()
        .stdout(predicate::str::contains("trigger-type"));
}

#[test]
fn smoke_incident_workflow_import_help() {
    pd().args(["incident", "workflow", "import", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--id"));
}

// ---------------------------------------------------------------------------
// Subcommand help: trigger
// ---------------------------------------------------------------------------

#[test]
fn smoke_trigger_help() {
    pd().args(["trigger", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("update"))
        .stdout(predicate::str::contains("delete"))
        .stdout(predicate::str::contains("create-for-service"))
        .stdout(predicate::str::contains("remove-from-service"));
}

#[test]
fn smoke_trigger_create_help() {
    pd().args(["trigger", "create", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--workflow-id"))
        .stdout(predicate::str::contains("--type"))
        .stdout(predicate::str::contains("--condition"))
        .stdout(predicate::str::contains("--incident-types"));
}

// ---------------------------------------------------------------------------
// Subcommand help: action
// ---------------------------------------------------------------------------

#[test]
fn smoke_action_help() {
    pd().args(["action", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"));
}

// ---------------------------------------------------------------------------
// Subcommand help: user
// ---------------------------------------------------------------------------

#[test]
fn smoke_user_help() {
    pd().args(["user", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"));
}

#[test]
fn smoke_user_list_accepts_patterns() {
    // Patterns are positional, not flags; --help should not advertise a --query flag.
    pd().args(["user", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PATTERNS").or(predicate::str::contains("patterns")));
}

#[test]
fn smoke_user_list_advertises_team_filter() {
    pd().args(["user", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--team"));
}

#[test]
fn smoke_escalation_list_advertises_team_filter() {
    pd().args(["escalation", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--team"));
}

#[test]
fn smoke_service_list_advertises_team_filter() {
    pd().args(["service", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--team"));
}

// ---------------------------------------------------------------------------
// Subcommand help: team
// ---------------------------------------------------------------------------

#[test]
fn smoke_team_help() {
    pd().args(["team", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("update"))
        .stdout(predicate::str::contains("delete"))
        .stdout(predicate::str::contains("member"));
}

#[test]
fn smoke_team_member_help() {
    pd().args(["team", "member", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("add"))
        .stdout(predicate::str::contains("remove"));
}

#[test]
fn smoke_team_create_example_flag() {
    // --example prints a commented YAML skeleton and exits before API call,
    // so it works without auth.
    pd().args(["team", "create", "--example"])
        .env_remove("PAGERDUTY_API_TOKEN")
        .assert()
        .success()
        .stdout(predicate::str::contains("name:"));
}

// ---------------------------------------------------------------------------
// Subcommand help: schedule
// ---------------------------------------------------------------------------

#[test]
fn smoke_schedule_help() {
    pd().args(["schedule", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("update"))
        .stdout(predicate::str::contains("delete"))
        .stdout(predicate::str::contains("override"));
}

#[test]
fn smoke_schedule_override_help() {
    pd().args(["schedule", "override", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("delete"));
}

#[test]
fn smoke_schedule_create_example_flag() {
    pd().args(["schedule", "create", "--example"])
        .env_remove("PAGERDUTY_API_TOKEN")
        .assert()
        .success()
        .stdout(predicate::str::contains("time-zone"));
}

// ---------------------------------------------------------------------------
// Subcommand help: escalation
// ---------------------------------------------------------------------------

#[test]
fn smoke_escalation_help() {
    pd().args(["escalation", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("update"))
        .stdout(predicate::str::contains("delete"));
}

#[test]
fn smoke_escalation_create_example_flag() {
    pd().args(["escalation", "create", "--example"])
        .env_remove("PAGERDUTY_API_TOKEN")
        .assert()
        .success()
        .stdout(predicate::str::contains("escalation-rules"));
}

// ---------------------------------------------------------------------------
// Subcommand help: service
// ---------------------------------------------------------------------------

#[test]
fn smoke_service_help() {
    pd().args(["service", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("integration"));
}

#[test]
fn smoke_service_integration_help() {
    pd().args(["service", "integration", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("delete"));
}

#[test]
fn smoke_service_create_example_flag() {
    pd().args(["service", "create", "--example"])
        .env_remove("PAGERDUTY_API_TOKEN")
        .assert()
        .success()
        .stdout(predicate::str::contains("escalation-policy"));
}

#[test]
fn smoke_service_integration_create_example_flag() {
    pd().args(["service", "integration", "create", "dummy-service", "--example"])
        .env_remove("PAGERDUTY_API_TOKEN")
        .assert()
        .success()
        .stdout(predicate::str::contains("type:"));
}

// ---------------------------------------------------------------------------
// Subcommand help: oncall
// ---------------------------------------------------------------------------

#[test]
fn smoke_oncall_help() {
    pd().args(["oncall", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"));
}

#[test]
fn smoke_action_list_rejects_query_flag() {
    // The PagerDuty API returns 400 on ?query= for the actions endpoint;
    // the flag has been removed. Verify the CLI no longer accepts it.
    pd().args(["action", "list", "--query", "slack"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--query").or(predicate::str::contains("unexpected argument")));
}

// ---------------------------------------------------------------------------
// Subcommand help: rest
// ---------------------------------------------------------------------------

#[test]
fn smoke_rest_help() {
    pd().args(["rest", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("method"))
        .stdout(predicate::str::contains("path"));
}

// ---------------------------------------------------------------------------
// Global options
// ---------------------------------------------------------------------------

#[test]
fn smoke_global_options_shown() {
    pd().arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--api-token"))
        .stdout(predicate::str::contains("--output"))
        .stdout(predicate::str::contains("--log-level"))
        .stdout(predicate::str::contains("--config"));
}

#[test]
fn smoke_after_help_shown() {
    pd().arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("PAGERDUTY_API_TOKEN"));
}

// ---------------------------------------------------------------------------
// Invalid subcommands and args
// ---------------------------------------------------------------------------

#[test]
fn smoke_invalid_subcommand() {
    pd().arg("nonsense")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn smoke_incident_type_create_missing_required() {
    // create requires --name and --display-name
    pd().args(["incident", "type", "create"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--name"));
}

#[test]
fn smoke_trigger_create_missing_required() {
    // create requires --workflow-id and --type
    pd().args(["trigger", "create"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--workflow-id"));
}

#[test]
fn smoke_incident_workflow_import_missing_file() {
    // import requires a file argument
    pd().args(["incident", "workflow", "import"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("<FILE>").or(predicate::str::contains("required")));
}
