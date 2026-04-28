use crate::cli::{IncidentTriggerAction, TriggerType};
use crate::client::PdClient;
use crate::config::Config;
use crate::output::print_value;
use eyre::Result;
use serde_json::json;
use tracing::instrument;

pub async fn handle(
    action: &IncidentTriggerAction,
    client: &PdClient,
    config: &Config,
) -> Result<()> {
    match action {
        IncidentTriggerAction::List { patterns } => list(client, config, patterns).await,
        IncidentTriggerAction::Get { id } => get(client, config, id).await,
        IncidentTriggerAction::Create {
            workflow,
            trigger_type,
            condition,
            incident_types,
        } => {
            create(
                client,
                config,
                workflow,
                trigger_type,
                condition.as_deref(),
                incident_types.as_deref(),
            )
            .await
        }
        IncidentTriggerAction::Update {
            id,
            condition,
            incident_types,
        } => {
            update(
                client,
                config,
                id,
                condition.as_deref(),
                incident_types.as_deref(),
            )
            .await
        }
        IncidentTriggerAction::Delete { id } => delete(client, config, id).await,
        IncidentTriggerAction::Bind {
            trigger_id,
            service,
        } => bind(client, config, trigger_id, service).await,
        IncidentTriggerAction::Unbind {
            trigger_id,
            service,
        } => unbind(client, trigger_id, service).await,
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, patterns: &[String]) -> Result<()> {
    let all = client
        .get_all_no_offset("/incident_workflows/triggers", "triggers")
        .await?;
    let filtered = crate::filter::filter_into(all, patterns, |v| {
        v.get("workflow")
            .and_then(|w| w.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
    });
    let result = json!({ "triggers": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let resp = client
        .get(&format!("/incident_workflows/triggers/{}", id))
        .await?;
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
    let type_str = trigger_type_str(trigger_type);

    // NOTE: Do not include "type" in the workflow reference.
    // PagerDuty returns 400 "trigger.workflow.type is not allowed" if present.
    let mut trigger = json!({
        "trigger_type": type_str,
        "workflow": { "id": workflow_id }
    });

    if let Some(cond) = condition {
        trigger["condition"] = json!(cond);
    }

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
    let resp = client
        .get(&format!("/incident_workflows/triggers/{}", id))
        .await?;
    let mut trigger = resp
        .get("trigger")
        .cloned()
        .ok_or_else(|| eyre::eyre!("Unexpected response: missing trigger key"))?;

    if let Some(cond) = condition {
        trigger["condition"] = json!(cond);
    }

    if let Some(names) = incident_types {
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
    let result = client
        .delete(&format!("/incident_workflows/triggers/{}", id))
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn bind(client: &PdClient, config: &Config, trigger_id: &str, service: &str) -> Result<()> {
    let service_id = crate::resources::service::resolve_service_id(client, service).await?;
    let body = json!({
        "service": {
            "id": service_id,
            "type": "service_reference"
        }
    });
    let result = client
        .post(
            &format!("/incident_workflows/triggers/{}/services", trigger_id),
            body,
        )
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client))]
async fn unbind(client: &PdClient, trigger_id: &str, service: &str) -> Result<()> {
    let service_id = crate::resources::service::resolve_service_id(client, service).await?;
    let result = client
        .delete(&format!(
            "/incident_workflows/triggers/{}/services/{}",
            trigger_id, service_id
        ))
        .await?;
    println!("Service {} unbound from trigger {}", service_id, trigger_id);
    if result != json!(null) {
        print_value(&result, &crate::cli::OutputFormat::Json);
    }
    Ok(())
}

fn trigger_type_str(t: &TriggerType) -> &'static str {
    match t {
        TriggerType::Conditional => "conditional",
        TriggerType::Manual => "manual",
        TriggerType::IncidentType => "incident_type",
    }
}

/// Resolve incident type display names or slugs to API UUIDs.
/// Mirrors the old top-level trigger helper; kept here so the submodule is
/// self-contained. Unrecognized values pass through unchanged.
#[instrument(skip(client))]
async fn resolve_incident_type_ids(client: &PdClient, names: &[String]) -> Result<Vec<String>> {
    let all_types = client.get_all("/incidents/types", "incident_types").await?;
    let mut resolved = Vec::with_capacity(names.len());

    for name in names {
        let found_id = all_types.iter().find_map(|t| {
            let display = t.get("display_name").and_then(|v| v.as_str()).unwrap_or("");
            let slug = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let id: &str = t.get("id").and_then(|v| v.as_str())?;
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
    fn trigger_type_str_values() {
        assert_eq!(trigger_type_str(&TriggerType::Conditional), "conditional");
        assert_eq!(trigger_type_str(&TriggerType::Manual), "manual");
        assert_eq!(
            trigger_type_str(&TriggerType::IncidentType),
            "incident_type"
        );
    }
}
