use crate::cli::IncidentWorkflowAction;
use crate::client::{PdClient, encode_query};
use crate::config::Config;
use crate::output::print_value;
use eyre::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use tracing::instrument;

// ---------------------------------------------------------------------------
// API structs (snake_case for PagerDuty REST API)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize)]
pub struct IncidentWorkflow {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub is_enabled: bool,
    pub steps: Option<Vec<Step>>,
    pub team: Option<TeamReference>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Step {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub action_configuration: ActionConfiguration,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ActionConfiguration {
    pub action_id: String,
    pub description: Option<String>,
    pub inputs: Option<Vec<ActionInput>>,
    pub outputs: Option<Vec<ActionOutput>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ActionInput {
    pub name: String,
    pub value: String,
    pub parameter_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ActionOutput {
    pub name: String,
    pub value: Option<String>,
    pub parameter_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TeamReference {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
}

// ---------------------------------------------------------------------------
// YAML definition structs (kebab-case for workflow YAML files)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct WorkflowDefinition {
    pub workflow: WorkflowYaml,
    pub trigger: Option<TriggerYaml>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct WorkflowYaml {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub is_enabled: bool,
    #[serde(default)]
    pub steps: Vec<StepYaml>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct StepYaml {
    pub name: String,
    pub description: Option<String>,
    pub action_id: String,
    #[serde(default)]
    pub inputs: Vec<InputYaml>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct InputYaml {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct TriggerYaml {
    pub trigger_type: String,
    pub condition: Option<String>,
    pub incident_types: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Handler dispatch
// ---------------------------------------------------------------------------

pub async fn handle(action: &IncidentWorkflowAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        IncidentWorkflowAction::List { query } => list(client, config, query.as_deref()).await,
        IncidentWorkflowAction::Get { id, include_steps } => get(client, config, id, *include_steps).await,
        IncidentWorkflowAction::Create {
            name,
            description,
            from_file,
        } => {
            if let Some(path) = from_file {
                create_from_file(client, config, path).await
            } else {
                create(client, config, name, description.as_deref()).await
            }
        }
        IncidentWorkflowAction::Update { id, name, description } => {
            update(client, config, id, name.as_deref(), description.as_deref()).await
        }
        IncidentWorkflowAction::Delete { id } => delete(client, config, id).await,
        IncidentWorkflowAction::Enable { id } => enable(client, config, id).await,
        IncidentWorkflowAction::Disable { id } => disable(client, config, id).await,
        IncidentWorkflowAction::Export { id } => export(client, id).await,
        IncidentWorkflowAction::Import { file, id } => import(client, config, file, id.as_deref()).await,
    }
}

// ---------------------------------------------------------------------------
// CRUD operations
// ---------------------------------------------------------------------------

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, query: Option<&str>) -> Result<()> {
    let path = match query {
        Some(q) => format!("/incident_workflows?query={}", encode_query(q)),
        None => "/incident_workflows".to_string(),
    };
    let all = client.get_all(&path, "incident_workflows").await?;
    let result = serde_json::json!({ "incident_workflows": all });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, id: &str, include_steps: bool) -> Result<()> {
    let path = if include_steps {
        format!("/incident_workflows/{}?include[]=steps", id)
    } else {
        format!("/incident_workflows/{}", id)
    };
    let resp = client.get(&path).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn create(client: &PdClient, config: &Config, name: &str, description: Option<&str>) -> Result<()> {
    let mut wf = json!({ "name": name });
    if let Some(desc) = description {
        wf["description"] = json!(desc);
    }
    let body = json!({ "incident_workflow": wf });
    let result = client.post("/incident_workflows", body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn create_from_file(client: &PdClient, config: &Config, path: &Path) -> Result<()> {
    let def = load_definition(path)?;
    let body = definition_to_api_body(&def);
    let result = client.post("/incident_workflows", body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn update(
    client: &PdClient,
    config: &Config,
    id: &str,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<()> {
    let resp = client
        .get(&format!("/incident_workflows/{}?include[]=steps", id))
        .await?;
    let raw = resp
        .get("incident_workflow")
        .cloned()
        .ok_or_else(|| eyre::eyre!("Unexpected response: missing incident_workflow key"))?;

    let mut wf: Value = raw;
    if let Some(n) = name {
        wf["name"] = json!(n);
    }
    if let Some(d) = description {
        wf["description"] = json!(d);
    }

    let body = json!({ "incident_workflow": wf });
    let result = client.put(&format!("/incident_workflows/{}", id), body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn delete(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let result = client.delete(&format!("/incident_workflows/{}", id)).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn enable(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let body = json!({ "incident_workflow": { "is_enabled": true } });
    let result = client.put(&format!("/incident_workflows/{}", id), body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn disable(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let body = json!({ "incident_workflow": { "is_enabled": false } });
    let result = client.put(&format!("/incident_workflows/{}", id), body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// Export: dump workflow + trigger to YAML
// ---------------------------------------------------------------------------

#[instrument(skip(client))]
async fn export(client: &PdClient, id: &str) -> Result<()> {
    // Fetch workflow with steps
    let resp = client
        .get(&format!("/incident_workflows/{}?include[]=steps", id))
        .await?;
    let wf_raw = resp
        .get("incident_workflow")
        .cloned()
        .ok_or_else(|| eyre::eyre!("Unexpected response: missing incident_workflow key"))?;
    let wf: IncidentWorkflow = serde_json::from_value(wf_raw).context("Failed to parse workflow")?;

    // Fetch ALL triggers (paginated) and find the one for this workflow
    let all_triggers = client.get_all("/incident_workflows/triggers", "triggers").await?;
    let triggers_resp = serde_json::json!({ "triggers": all_triggers });
    let trigger_yaml = find_trigger_for_workflow(&triggers_resp, id);

    let def = api_to_definition(&wf, trigger_yaml);
    let yaml = serde_yaml::to_string(&def).context("Failed to serialize to YAML")?;
    println!("{}", yaml);
    Ok(())
}

// ---------------------------------------------------------------------------
// Import: three-step atomic (create/update workflow, create/update trigger, enable)
// ---------------------------------------------------------------------------

#[instrument(skip(client, config))]
async fn import(client: &PdClient, config: &Config, path: &Path, explicit_id: Option<&str>) -> Result<()> {
    let def = load_definition(path)?;
    let want_enabled = def.workflow.is_enabled;

    // Step 1: Create or update workflow (always disabled initially)
    let workflow_id = if let Some(id) = explicit_id {
        update_workflow_from_definition(client, id, &def).await?
    } else {
        upsert_workflow_by_name(client, &def).await?
    };

    println!("Workflow ID: {}", workflow_id);

    // Step 2: Create or update trigger (if trigger section present)
    if let Some(ref trigger) = def.trigger {
        match upsert_trigger(client, &workflow_id, trigger).await {
            Ok(trigger_id) => println!("Trigger ID: {}", trigger_id),
            Err(e) => {
                eprintln!("Warning: trigger creation/update failed: {}", e);
                eprintln!(
                    "Workflow {} exists but is disabled. Fix trigger and retry.",
                    workflow_id
                );
                return Err(e);
            }
        }
    }

    // Step 3: Enable if requested and trigger succeeded
    if want_enabled {
        let body = json!({ "incident_workflow": { "is_enabled": true } });
        let result = client
            .put(&format!("/incident_workflows/{}", workflow_id), body)
            .await?;
        print_value(&result, &config.output_format);
        println!("Workflow enabled.");
    } else {
        println!("Workflow imported (disabled).");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Import helpers
// ---------------------------------------------------------------------------

#[instrument(skip(client, def), fields(name = %def.workflow.name))]
async fn upsert_workflow_by_name(client: &PdClient, def: &WorkflowDefinition) -> Result<String> {
    // Look up by name - paginate in case the query returns multiple pages
    let path = format!("/incident_workflows?query={}", encode_query(&def.workflow.name));
    let all_workflows = client.get_all(&path, "incident_workflows").await?;

    let matches: Vec<&Value> = all_workflows
        .iter()
        .filter(|w| w.get("name").and_then(|n| n.as_str()) == Some(&def.workflow.name))
        .collect();

    match matches.len() {
        0 => {
            // Create new workflow (disabled)
            let body = definition_to_api_body_disabled(def);
            let result = client.post("/incident_workflows", body).await?;
            extract_workflow_id(&result)
        }
        1 => {
            // Update existing
            let id = matches[0]
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| eyre::eyre!("Workflow match missing id field"))?;
            update_workflow_from_definition(client, id, def).await
        }
        n => {
            bail!(
                "Found {} workflows named '{}'. Use --id to specify which one to update.",
                n,
                def.workflow.name
            );
        }
    }
}

#[instrument(skip(client, def))]
async fn update_workflow_from_definition(client: &PdClient, id: &str, def: &WorkflowDefinition) -> Result<String> {
    // NOTE: This is a blind PUT from the YAML definition without fetching the current state first.
    // The YAML is the source of truth for all mutable workflow fields. Whether PD replaces the
    // steps array entirely or merges it is an open question that must be verified against the live
    // API. If PD merges, duplicate steps will appear on repeated imports; if PD replaces, this is
    // safe. See the design doc Open Questions section.
    let body = definition_to_api_body_disabled(def);
    let result = client.put(&format!("/incident_workflows/{}", id), body).await?;
    extract_workflow_id(&result)
}

#[instrument(skip(client, trigger))]
async fn upsert_trigger(client: &PdClient, workflow_id: &str, trigger: &TriggerYaml) -> Result<String> {
    // IMPORTANT: GET /incident_workflows/triggers only returns is_disabled=false triggers.
    // During import, step 1 always disables the workflow, which makes its triggers
    // is_disabled=true and invisible in the global list. Using get_all() on the global
    // list would therefore miss existing triggers on re-import and create duplicates.
    //
    // Fix: read trigger IDs from the workflow GET response's embedded "triggers" array,
    // which is present regardless of disabled state. Use the first trigger ID for PUT.
    let wf_resp = client.get(&format!("/incident_workflows/{}", workflow_id)).await?;
    let existing_trigger_id: Option<String> = wf_resp
        .get("incident_workflow")
        .and_then(|w| w.get("triggers"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|t| t.get("id").and_then(|id| id.as_str()))
        .map(|s| s.to_string());

    // The API accepts incident type display names directly; no UUID resolution needed.
    let trigger_body = build_trigger_body(workflow_id, trigger, None);

    if let Some(trigger_id) = existing_trigger_id {
        let result = client
            .put(&format!("/incident_workflows/triggers/{}", trigger_id), trigger_body)
            .await?;
        extract_trigger_id(&result)
    } else {
        let result = client.post("/incident_workflows/triggers", trigger_body).await?;
        extract_trigger_id(&result)
    }
}

// Note: UUID resolution for incident_type trigger names was removed. Live testing confirmed
// the PagerDuty API accepts display names (e.g. "Managed Incident") directly in the
// incident_types array without requiring UUIDs.

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

fn load_definition(path: &Path) -> Result<WorkflowDefinition> {
    let content = fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    serde_yaml::from_str(&content).with_context(|| format!("Failed to parse YAML from {}", path.display()))
}

fn definition_to_api_body(def: &WorkflowDefinition) -> Value {
    let steps: Vec<Value> = def
        .workflow
        .steps
        .iter()
        .map(|s| {
            let inputs: Vec<Value> = s
                .inputs
                .iter()
                .map(|i| json!({ "name": i.name, "value": i.value }))
                .collect();
            let mut step = json!({
                "name": s.name,
                "action_configuration": {
                    "action_id": s.action_id,
                    "inputs": inputs
                }
            });
            if let Some(ref desc) = s.description {
                step["description"] = json!(desc);
            }
            step
        })
        .collect();

    let mut wf = json!({
        "name": def.workflow.name,
        "is_enabled": def.workflow.is_enabled,
        "steps": steps
    });

    if let Some(ref desc) = def.workflow.description {
        wf["description"] = json!(desc);
    }

    json!({ "incident_workflow": wf })
}

fn definition_to_api_body_disabled(def: &WorkflowDefinition) -> Value {
    let mut body = definition_to_api_body(def);
    if let Some(wf) = body.get_mut("incident_workflow") {
        wf["is_enabled"] = json!(false);
    }
    body
}

/// Build the JSON body for a trigger create/update.
/// `resolved_types` overrides `trigger.incident_types` with UUID-resolved values.
/// Pass `None` to use whatever is in `trigger.incident_types` as-is.
fn build_trigger_body(workflow_id: &str, trigger: &TriggerYaml, resolved_types: Option<&[String]>) -> Value {
    // NOTE: The trigger body must NOT include "type" in the workflow reference.
    // The PagerDuty API returns 400 "trigger.workflow.type is not allowed" if present.
    let mut t = json!({
        "trigger_type": trigger.trigger_type,
        "workflow": {
            "id": workflow_id
        }
    });

    if let Some(ref cond) = trigger.condition {
        t["condition"] = json!(cond);
    }

    let types = resolved_types.or(trigger.incident_types.as_deref());
    if let Some(types) = types {
        t["incident_types"] = json!(types);
    }

    json!({ "trigger": t })
}

fn api_to_definition(wf: &IncidentWorkflow, trigger: Option<TriggerYaml>) -> WorkflowDefinition {
    let steps = wf
        .steps
        .as_ref()
        .map(|ss| {
            ss.iter()
                .map(|s| StepYaml {
                    name: s.name.clone(),
                    description: s.description.clone(),
                    action_id: s.action_configuration.action_id.clone(),
                    inputs: s
                        .action_configuration
                        .inputs
                        .as_ref()
                        .map(|inputs| {
                            inputs
                                .iter()
                                .map(|i| InputYaml {
                                    name: i.name.clone(),
                                    value: i.value.clone(),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    WorkflowDefinition {
        workflow: WorkflowYaml {
            name: wf.name.clone(),
            description: wf.description.clone(),
            is_enabled: wf.is_enabled,
            steps,
        },
        trigger,
    }
}

fn find_trigger_for_workflow(triggers_resp: &Value, workflow_id: &str) -> Option<TriggerYaml> {
    triggers_resp
        .get("triggers")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|t| t.get("workflow").and_then(|w| w.get("id")).and_then(|id| id.as_str()) == Some(workflow_id))
        })
        .map(|t| TriggerYaml {
            trigger_type: t
                .get("trigger_type")
                .and_then(|v| v.as_str())
                .unwrap_or("conditional")
                .to_string(),
            condition: t.get("condition").and_then(|v| v.as_str()).map(|s| s.to_string()),
            incident_types: t
                .get("incident_types")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()),
        })
}

fn extract_workflow_id(resp: &Value) -> Result<String> {
    resp.get("incident_workflow")
        .and_then(|w| w.get("id"))
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| eyre::eyre!("Response missing incident_workflow.id"))
}

fn extract_trigger_id(resp: &Value) -> Result<String> {
    resp.get("trigger")
        .and_then(|t| t.get("id"))
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| eyre::eyre!("Response missing trigger.id"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_roundtrip() {
        let wf = IncidentWorkflow {
            id: Some("WF123".to_string()),
            name: "Test Workflow".to_string(),
            description: Some("A test workflow".to_string()),
            is_enabled: true,
            steps: Some(vec![Step {
                id: Some("S1".to_string()),
                name: "Step 1".to_string(),
                description: None,
                action_configuration: ActionConfiguration {
                    action_id: "pagerduty.slack.create-dedicated-channel".to_string(),
                    description: None,
                    inputs: Some(vec![ActionInput {
                        name: "channel_name".to_string(),
                        value: "incident-{{incident.id}}".to_string(),
                        parameter_type: None,
                    }]),
                    outputs: None,
                },
            }]),
            team: None,
        };
        let json = serde_json::to_value(&wf).unwrap();
        let roundtrip: IncidentWorkflow = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip.name, "Test Workflow");
        assert!(roundtrip.is_enabled);
        assert_eq!(roundtrip.steps.unwrap().len(), 1);
    }

    #[test]
    fn test_workflow_definition_yaml_roundtrip() {
        let yaml = r#"
workflow:
  name: Auto-Manage P1
  description: Automatically set Managed Incident for all P1s
  is-enabled: false
  steps:
    - name: Set Incident Type
      action-id: pagerduty.incident-management.update-incident-type
      inputs:
        - name: incident_type
          value: Managed Incident
trigger:
  trigger-type: conditional
  condition: "incident.priority matches 'P1'"
"#;
        let def: WorkflowDefinition = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.workflow.name, "Auto-Manage P1");
        assert!(!def.workflow.is_enabled);
        assert_eq!(def.workflow.steps.len(), 1);
        assert_eq!(
            def.workflow.steps[0].action_id,
            "pagerduty.incident-management.update-incident-type"
        );

        let trigger = def.trigger.as_ref().unwrap();
        assert_eq!(trigger.trigger_type, "conditional");
        assert_eq!(trigger.condition.as_deref(), Some("incident.priority matches 'P1'"));
    }

    #[test]
    fn test_definition_to_api_body() {
        let def = WorkflowDefinition {
            workflow: WorkflowYaml {
                name: "Test WF".to_string(),
                description: Some("desc".to_string()),
                is_enabled: true,
                steps: vec![StepYaml {
                    name: "Step 1".to_string(),
                    description: None,
                    action_id: "pagerduty.slack.send-message".to_string(),
                    inputs: vec![InputYaml {
                        name: "message".to_string(),
                        value: "hello".to_string(),
                    }],
                }],
            },
            trigger: None,
        };
        let body = definition_to_api_body(&def);
        let wf = body.get("incident_workflow").unwrap();
        assert_eq!(wf["name"], "Test WF");
        assert_eq!(wf["is_enabled"], true);
        assert_eq!(wf["steps"].as_array().unwrap().len(), 1);
        assert_eq!(
            wf["steps"][0]["action_configuration"]["action_id"],
            "pagerduty.slack.send-message"
        );
    }

    #[test]
    fn test_definition_to_api_body_disabled() {
        let def = WorkflowDefinition {
            workflow: WorkflowYaml {
                name: "Test".to_string(),
                description: None,
                is_enabled: true,
                steps: vec![],
            },
            trigger: None,
        };
        let body = definition_to_api_body_disabled(&def);
        let wf = body.get("incident_workflow").unwrap();
        assert_eq!(wf["is_enabled"], false);
    }

    #[test]
    fn test_build_trigger_body_conditional() {
        let trigger = TriggerYaml {
            trigger_type: "conditional".to_string(),
            condition: Some("incident.priority matches 'P1'".to_string()),
            incident_types: None,
        };
        let body = build_trigger_body("WF123", &trigger, None);
        let t = body.get("trigger").unwrap();
        assert_eq!(t["trigger_type"], "conditional");
        assert_eq!(t["condition"], "incident.priority matches 'P1'");
        assert_eq!(t["workflow"]["id"], "WF123");
    }

    #[test]
    fn test_build_trigger_body_incident_type() {
        let trigger = TriggerYaml {
            trigger_type: "incident_type".to_string(),
            condition: None,
            incident_types: Some(vec!["TYPE1".to_string(), "TYPE2".to_string()]),
        };
        let body = build_trigger_body("WF456", &trigger, None);
        let t = body.get("trigger").unwrap();
        assert_eq!(t["trigger_type"], "incident_type");
        assert_eq!(t["incident_types"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_api_to_definition_roundtrip() {
        let wf = IncidentWorkflow {
            id: Some("WF1".to_string()),
            name: "My Workflow".to_string(),
            description: Some("A workflow".to_string()),
            is_enabled: false,
            steps: Some(vec![Step {
                id: Some("S1".to_string()),
                name: "Send Message".to_string(),
                description: None,
                action_configuration: ActionConfiguration {
                    action_id: "pagerduty.slack.send-message".to_string(),
                    description: None,
                    inputs: Some(vec![ActionInput {
                        name: "message".to_string(),
                        value: "hello".to_string(),
                        parameter_type: None,
                    }]),
                    outputs: None,
                },
            }]),
            team: None,
        };

        let trigger = TriggerYaml {
            trigger_type: "conditional".to_string(),
            condition: Some("incident.priority matches 'P1'".to_string()),
            incident_types: None,
        };

        let def = api_to_definition(&wf, Some(trigger));
        assert_eq!(def.workflow.name, "My Workflow");
        assert_eq!(def.workflow.steps.len(), 1);
        assert_eq!(def.workflow.steps[0].action_id, "pagerduty.slack.send-message");
        assert!(def.trigger.is_some());
    }

    #[test]
    fn test_find_trigger_for_workflow_found() {
        let resp = json!({
            "triggers": [
                {
                    "id": "T1",
                    "trigger_type": "conditional",
                    "condition": "incident.priority matches 'P1'",
                    "workflow": { "id": "WF1", "type": "workflow" }
                },
                {
                    "id": "T2",
                    "trigger_type": "incident_type",
                    "incident_types": ["TYPE1"],
                    "workflow": { "id": "WF2", "type": "workflow" }
                }
            ]
        });

        let result = find_trigger_for_workflow(&resp, "WF1");
        assert!(result.is_some());
        let t = result.unwrap();
        assert_eq!(t.trigger_type, "conditional");
        assert_eq!(t.condition.as_deref(), Some("incident.priority matches 'P1'"));
    }

    #[test]
    fn test_find_trigger_for_workflow_not_found() {
        let resp = json!({
            "triggers": [
                {
                    "id": "T1",
                    "trigger_type": "conditional",
                    "workflow": { "id": "WF99", "type": "workflow" }
                }
            ]
        });
        assert!(find_trigger_for_workflow(&resp, "WF1").is_none());
    }

    #[test]
    fn test_extract_workflow_id_success() {
        let resp = json!({ "incident_workflow": { "id": "WF123", "name": "test" } });
        assert_eq!(extract_workflow_id(&resp).unwrap(), "WF123");
    }

    #[test]
    fn test_extract_workflow_id_missing() {
        let resp = json!({ "other": "data" });
        assert!(extract_workflow_id(&resp).is_err());
    }

    #[test]
    fn test_complex_workflow_yaml() {
        let yaml = r#"
workflow:
  name: Managed Incident Response
  description: >-
    Full response for all managed incidents P1-P4
  is-enabled: false
  steps:
    - name: Create Slack Channel
      action-id: pagerduty.slack.create-dedicated-channel
      inputs:
        - name: channel_name
          value: "incident-{{incident.created_at | date: \"%Y%m%d-%H%M\"}}"
    - name: Set Channel Topic
      action-id: pagerduty.slack.set-channel-topic
      inputs:
        - name: topic
          value: "P{{incident.priority.name}} | {{incident.title}}"
    - name: Post Status Card
      action-id: pagerduty.slack.send-message
      inputs:
        - name: channel
          value: "{{steps.create_slack_channel.channel_id}}"
        - name: message
          value: ":rotating_light: Managed Incident Declared"
trigger:
  trigger-type: incident_type
  incident-types:
    - Managed Incident
"#;
        let def: WorkflowDefinition = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.workflow.name, "Managed Incident Response");
        assert_eq!(def.workflow.steps.len(), 3);
        assert_eq!(
            def.workflow.steps[0].action_id,
            "pagerduty.slack.create-dedicated-channel"
        );
        assert_eq!(def.workflow.steps[2].inputs.len(), 2);

        let trigger = def.trigger.unwrap();
        assert_eq!(trigger.trigger_type, "incident_type");
        assert_eq!(trigger.incident_types.unwrap(), vec!["Managed Incident"]);
    }

    // -----------------------------------------------------------------------
    // Workflow YAML file validation tests
    // -----------------------------------------------------------------------

    fn load_workflow_file(filename: &str) -> WorkflowDefinition {
        let path = format!("{}/workflows/{}", env!("CARGO_MANIFEST_DIR"), filename);
        let content = fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));
        serde_yaml::from_str(&content).unwrap_or_else(|e| panic!("Failed to parse {}: {}", path, e))
    }

    #[test]
    fn test_wf1_managed_incident_response_yaml() {
        let def = load_workflow_file("wf1-managed-incident-response.yml");
        assert_eq!(def.workflow.name, "Managed Incident Response");
        assert!(!def.workflow.is_enabled);
        assert_eq!(def.workflow.steps.len(), 8);

        // Verify step order and action IDs
        assert_eq!(def.workflow.steps[0].name, "Create Slack Channel");
        assert_eq!(
            def.workflow.steps[0].action_id,
            "pagerduty.slack.create-dedicated-channel"
        );
        assert_eq!(def.workflow.steps[1].name, "Set Channel Topic");
        assert_eq!(def.workflow.steps[2].name, "Post Status Card");
        assert_eq!(def.workflow.steps[3].name, "Create Jira Issue");
        assert_eq!(def.workflow.steps[4].name, "Add Bookmarks");
        assert_eq!(def.workflow.steps[5].name, "Post to #incidents");
        assert_eq!(def.workflow.steps[6].name, "Delay Before Status Prompt");
        assert_eq!(def.workflow.steps[6].action_id, "pagerduty.logic.delay");
        assert_eq!(def.workflow.steps[7].name, "Status Update Prompt");

        // Verify trigger
        let trigger = def.trigger.as_ref().unwrap();
        assert_eq!(trigger.trigger_type, "incident_type");
        assert_eq!(trigger.incident_types.as_ref().unwrap(), &["Managed Incident"]);

        // Verify it produces a valid API body
        let body = definition_to_api_body(&def);
        let wf = body.get("incident_workflow").unwrap();
        assert_eq!(wf["steps"].as_array().unwrap().len(), 8);
    }

    #[test]
    fn test_wf2_incident_visibility_yaml() {
        let def = load_workflow_file("wf2-incident-visibility.yml");
        assert_eq!(def.workflow.name, "Incident Visibility");
        assert!(!def.workflow.is_enabled);
        assert_eq!(def.workflow.steps.len(), 1);

        assert_eq!(def.workflow.steps[0].name, "Post to #incidents");
        assert_eq!(def.workflow.steps[0].action_id, "pagerduty.slack.send-message");

        let trigger = def.trigger.as_ref().unwrap();
        assert_eq!(trigger.trigger_type, "conditional");
        assert!(trigger.condition.is_some());
    }

    #[test]
    fn test_wf3_auto_manage_p1_yaml() {
        let def = load_workflow_file("wf3-auto-manage-p1.yml");
        assert_eq!(def.workflow.name, "Auto-Manage P1");
        assert!(!def.workflow.is_enabled);
        assert_eq!(def.workflow.steps.len(), 1);

        assert_eq!(def.workflow.steps[0].name, "Set Incident Type");
        assert_eq!(
            def.workflow.steps[0].action_id,
            "pagerduty.incident-management.update-incident-type"
        );

        let trigger = def.trigger.as_ref().unwrap();
        assert_eq!(trigger.trigger_type, "conditional");
        assert_eq!(trigger.condition.as_deref(), Some("incident.priority matches 'P1'"));
    }

    #[test]
    fn test_wf4a_auto_manage_p1_escalation_yaml() {
        let def = load_workflow_file("wf4a-auto-manage-p1-escalation.yml");
        assert_eq!(def.workflow.name, "Auto-Manage on P1 Escalation");
        assert!(!def.workflow.is_enabled);
        assert_eq!(def.workflow.steps.len(), 1);

        assert_eq!(
            def.workflow.steps[0].action_id,
            "pagerduty.incident-management.update-incident-type"
        );

        let trigger = def.trigger.as_ref().unwrap();
        assert_eq!(trigger.trigger_type, "conditional");
        // Must match on P1 AND not already managed
        let cond = trigger.condition.as_deref().unwrap();
        assert!(cond.contains("P1"));
        assert!(cond.contains("Managed Incident"));
    }

    #[test]
    fn test_wf4b_priority_changed_yaml() {
        let def = load_workflow_file("wf4b-priority-changed.yml");
        assert_eq!(def.workflow.name, "Managed Incident - Priority Changed");
        assert!(!def.workflow.is_enabled);
        assert_eq!(def.workflow.steps.len(), 3);

        assert_eq!(def.workflow.steps[0].name, "Post Cadence Update");
        assert_eq!(def.workflow.steps[1].name, "Update #incidents");
        assert_eq!(def.workflow.steps[2].name, "Update Channel Topic");

        let trigger = def.trigger.as_ref().unwrap();
        assert_eq!(trigger.trigger_type, "conditional");
        let cond = trigger.condition.as_deref().unwrap();
        assert!(cond.contains("Managed Incident"));
    }

    #[test]
    fn test_all_workflow_files_produce_valid_api_bodies() {
        let files = [
            "wf1-managed-incident-response.yml",
            "wf2-incident-visibility.yml",
            "wf3-auto-manage-p1.yml",
            "wf4a-auto-manage-p1-escalation.yml",
            "wf4b-priority-changed.yml",
        ];
        for file in files {
            let def = load_workflow_file(file);
            let body = definition_to_api_body(&def);
            let wf = body.get("incident_workflow").unwrap();
            assert!(wf.get("name").is_some(), "{} missing name", file);
            assert!(wf.get("steps").is_some(), "{} missing steps", file);

            // Verify trigger builds correctly if present
            if let Some(ref trigger) = def.trigger {
                let trigger_body = build_trigger_body("FAKE_WF_ID", trigger, None);
                let t = trigger_body.get("trigger").unwrap();
                assert!(t.get("trigger_type").is_some(), "{} trigger missing type", file);
                assert_eq!(t["workflow"]["id"], "FAKE_WF_ID");
            }
        }
    }
}
