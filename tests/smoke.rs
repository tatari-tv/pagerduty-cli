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
        .stdout(predicate::str::contains("--from-file"));
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
