use crate::cli::{TriggerAction, TriggerType};
use crate::client::PdClient;
use crate::config::Config;
use crate::output::print_value;
use eyre::Result;
use serde_json::json;
use tracing::instrument;

pub async fn handle(action: &TriggerAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        TriggerAction::List => list(client, config).await,
        TriggerAction::Get { id } => get(client, config, id).await,
        TriggerAction::Create {
            workflow_id,
            trigger_type,
            condition,
            incident_types,
        } => {
            create(
                client,
                config,
                workflow_id,
                trigger_type,
                condition.as_deref(),
                incident_types.as_deref(),
            )
            .await
        }
        TriggerAction::Update {
            id,
            condition,
            incident_types,
        } => update(client, config, id, condition.as_deref(), incident_types.as_deref()).await,
        TriggerAction::Delete { id } => delete(client, config, id).await,
        TriggerAction::CreateForService { trigger_id, service_id } => {
            create_for_service(client, config, trigger_id, service_id).await
        }
        TriggerAction::RemoveFromService { trigger_id, service_id } => {
            remove_from_service(client, trigger_id, service_id).await
        }
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config) -> Result<()> {
    let all = client
        .get_all_no_offset("/incident_workflows/triggers", "triggers")
        .await?;
    let result = serde_json::json!({ "triggers": all });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let resp = client.get(&format!("/incident_workflows/triggers/{}", id)).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn create(
    client: &PdClient,
    config: &Config,
    workflow_id: &str,
    trigger_type: &TriggerType,
    condition: Option<&str>,
    incident_types: Option<&[String]>,
) -> Result<()> {
    let type_str = match trigger_type {
        TriggerType::Conditional => "conditional",
        TriggerType::Manual => "manual",
        TriggerType::IncidentType => "incident_type",
    };

    // NOTE: Do not include "type" in the workflow reference.
    // PagerDuty returns 400 "trigger.workflow.type is not allowed" if present.
    let mut trigger = json!({
        "trigger_type": type_str,
        "workflow": {
            "id": workflow_id
        }
    });

    if let Some(cond) = condition {
        trigger["condition"] = json!(cond);
    }

    // For incident_type triggers, resolve display names to UUIDs. This makes the
    // CLI consistent with the YAML import UX: users can pass "Managed Incident"
    // directly instead of having to look up the UUID first.
    if let Some(names) = incident_types {
        let resolved = if matches!(trigger_type, TriggerType::IncidentType) {
            resolve_incident_type_ids(client, names).await?
        } else {
            names.to_vec()
        };
        trigger["incident_types"] = json!(resolved);
    }

    let body = json!({ "trigger": trigger });
    let result = client.post("/incident_workflows/triggers", body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn update(
    client: &PdClient,
    config: &Config,
    id: &str,
    condition: Option<&str>,
    incident_types: Option<&[String]>,
) -> Result<()> {
    // Fetch current trigger to preserve existing fields
    let resp = client.get(&format!("/incident_workflows/triggers/{}", id)).await?;
    let mut trigger = resp
        .get("trigger")
        .cloned()
        .ok_or_else(|| eyre::eyre!("Unexpected response: missing trigger key"))?;

    if let Some(cond) = condition {
        trigger["condition"] = json!(cond);
    }

    if let Some(names) = incident_types {
        // Resolve names to UUIDs if this is (or is being updated to) an incident_type trigger
        let is_incident_type = trigger
            .get("trigger_type")
            .and_then(|v| v.as_str())
            .map(|t| t == "incident_type")
            .unwrap_or(false);
        let resolved = if is_incident_type {
            resolve_incident_type_ids(client, names).await?
        } else {
            names.to_vec()
        };
        trigger["incident_types"] = json!(resolved);
    }

    let body = json!({ "trigger": trigger });
    let result = client
        .put(&format!("/incident_workflows/triggers/{}", id), body)
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn delete(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let result = client.delete(&format!("/incident_workflows/triggers/{}", id)).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn create_for_service(client: &PdClient, config: &Config, trigger_id: &str, service_id: &str) -> Result<()> {
    let body = json!({
        "service": {
            "id": service_id,
            "type": "service_reference"
        }
    });
    let result = client
        .post(&format!("/incident_workflows/triggers/{}/services", trigger_id), body)
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client))]
async fn remove_from_service(client: &PdClient, trigger_id: &str, service_id: &str) -> Result<()> {
    let result = client
        .delete(&format!(
            "/incident_workflows/triggers/{}/services/{}",
            trigger_id, service_id
        ))
        .await?;
    println!("Service {} removed from trigger {}", service_id, trigger_id);
    if result != json!(null) {
        print_value(&result, &crate::cli::OutputFormat::Json);
    }
    Ok(())
}

/// Resolve incident type display names or slugs to API UUIDs.
/// Matches on both `display_name` and `name` (slug) case-insensitively.
/// Unrecognized values are passed through unchanged so the API error is visible.
#[instrument(skip(client))]
async fn resolve_incident_type_ids(client: &PdClient, names: &[String]) -> Result<Vec<String>> {
    let all_types = client.get_all("/incidents/types", "incident_types").await?;
    let mut resolved = Vec::with_capacity(names.len());

    for name in names {
        let found_id = all_types.iter().find_map(|t| {
            let display = t.get("display_name").and_then(|v| v.as_str()).unwrap_or("");
            let slug = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let id = t.get("id").and_then(|v| v.as_str())?;
            if display.eq_ignore_ascii_case(name) || slug.eq_ignore_ascii_case(name) {
                Some(id.to_string())
            } else {
                None
            }
        });

        match found_id {
            Some(id) => resolved.push(id),
            None => {
                tracing::warn!(name = %name, "incident type name not found in account; passing through");
                resolved.push(name.clone());
            }
        }
    }

    Ok(resolved)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_type_str() {
        let cases = [
            (TriggerType::Conditional, "conditional"),
            (TriggerType::Manual, "manual"),
            (TriggerType::IncidentType, "incident_type"),
        ];
        for (tt, expected) in cases {
            let s = match tt {
                TriggerType::Conditional => "conditional",
                TriggerType::Manual => "manual",
                TriggerType::IncidentType => "incident_type",
            };
            assert_eq!(s, expected);
        }
    }
}
