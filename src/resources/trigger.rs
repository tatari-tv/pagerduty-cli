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
    let resp = client.get("/incident_workflows/triggers").await?;
    print_value(&resp, &config.output_format);
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

    let mut trigger = json!({
        "trigger_type": type_str,
        "workflow": {
            "id": workflow_id,
            "type": "workflow"
        }
    });

    if let Some(cond) = condition {
        trigger["condition"] = json!(cond);
    }
    if let Some(types) = incident_types {
        trigger["incident_types"] = json!(types);
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
    // Fetch current trigger
    let resp = client.get(&format!("/incident_workflows/triggers/{}", id)).await?;
    let mut trigger = resp
        .get("trigger")
        .cloned()
        .ok_or_else(|| eyre::eyre!("Unexpected response: missing trigger key"))?;

    if let Some(cond) = condition {
        trigger["condition"] = json!(cond);
    }
    if let Some(types) = incident_types {
        trigger["incident_types"] = json!(types);
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
    // delete may return empty body; print if present
    if result != json!(null) {
        print_value(&result, &crate::cli::OutputFormat::Json);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_type_str() {
        // Verify the string mapping is correct
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
