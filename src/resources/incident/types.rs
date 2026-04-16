use crate::cli::{FieldAction, IncidentTypeAction, TypeFilter};
use crate::client::PdClient;
use crate::config::Config;
use crate::output::print_value;
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::instrument;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct IncidentType {
    pub id: Option<String>,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub parent: Option<ApiReference>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ApiReference {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
}

pub async fn handle(action: &IncidentTypeAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        IncidentTypeAction::List { filter } => list(client, config, filter).await,
        IncidentTypeAction::Get { id_or_name } => get(client, config, id_or_name).await,
        IncidentTypeAction::Create {
            name,
            display_name,
            description,
        } => create(client, config, name, display_name, description.as_deref()).await,
        IncidentTypeAction::Update {
            id_or_name,
            display_name,
            description,
            enabled,
        } => {
            update(
                client,
                config,
                id_or_name,
                display_name.as_deref(),
                description.as_deref(),
                *enabled,
            )
            .await
        }
        IncidentTypeAction::Field { action } => field(client, config, action).await,
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, filter: &TypeFilter) -> Result<()> {
    let all = client.get_all("/incidents/types", "incident_types").await?;
    let filtered = apply_filter(all, filter);
    let count = filtered.len();
    let result = json!({
        "incident_types": filtered,
        "limit": count,
        "offset": 0,
        "more": false
    });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, id_or_name: &str) -> Result<()> {
    let resp = client.get(&format!("/incidents/types/{}", id_or_name)).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn create(
    client: &PdClient,
    config: &Config,
    name: &str,
    display_name: &str,
    description: Option<&str>,
) -> Result<()> {
    let mut type_body = json!({
        "name": name,
        "display_name": display_name,
        "enabled": true
    });

    if let Some(desc) = description {
        type_body["description"] = json!(desc);
    }

    let body = json!({ "incident_type": type_body });
    let result = client.post("/incidents/types", body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn update(
    client: &PdClient,
    config: &Config,
    id_or_name: &str,
    display_name: Option<&str>,
    description: Option<&str>,
    enabled: Option<bool>,
) -> Result<()> {
    // Fetch current state, apply changes, PUT full object
    let resp = client.get(&format!("/incidents/types/{}", id_or_name)).await?;

    let raw = resp
        .get("incident_type")
        .cloned()
        .ok_or_else(|| eyre::eyre!("Unexpected response: missing incident_type key"))?;

    let mut current: IncidentType = serde_json::from_value(raw).context("Failed to parse incident type from API")?;

    if let Some(dn) = display_name {
        current.display_name = dn.to_string();
    }
    if let Some(desc) = description {
        current.description = Some(desc.to_string());
    }
    if let Some(en) = enabled {
        current.enabled = en;
    }

    let body = json!({ "incident_type": current });
    let result = client.put(&format!("/incidents/types/{}", id_or_name), body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn field(client: &PdClient, config: &Config, action: &FieldAction) -> Result<()> {
    match action {
        FieldAction::List { type_id_or_name } => {
            let resp = client
                .get(&format!("/incidents/types/{}/custom_fields", type_id_or_name))
                .await?;
            print_value(&resp, &config.output_format);
        }
        FieldAction::Create {
            type_id_or_name,
            name,
            data_type,
            field_type,
        } => {
            let body = json!({
                "custom_field": {
                    "name": name,
                    "data_type": data_type,
                    "field_type": field_type
                }
            });
            let result = client
                .post(&format!("/incidents/types/{}/custom_fields", type_id_or_name), body)
                .await?;
            print_value(&result, &config.output_format);
        }
    }
    Ok(())
}

fn apply_filter(types: Vec<Value>, filter: &TypeFilter) -> Vec<Value> {
    match filter {
        TypeFilter::All => types,
        TypeFilter::Enabled | TypeFilter::Disabled => {
            let want_enabled = matches!(filter, TypeFilter::Enabled);
            types
                .into_iter()
                .filter(|t| t.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false) == want_enabled)
                .collect()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_apply_filter_all() {
        let types = vec![
            json!({"name": "a", "enabled": true}),
            json!({"name": "b", "enabled": false}),
        ];
        let result = apply_filter(types, &TypeFilter::All);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_apply_filter_enabled() {
        let types = vec![
            json!({"name": "a", "enabled": true}),
            json!({"name": "b", "enabled": false}),
            json!({"name": "c", "enabled": true}),
        ];
        let result = apply_filter(types, &TypeFilter::Enabled);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|t| t["enabled"] == true));
    }

    #[test]
    fn test_apply_filter_disabled() {
        let types = vec![
            json!({"name": "a", "enabled": true}),
            json!({"name": "b", "enabled": false}),
        ];
        let result = apply_filter(types, &TypeFilter::Disabled);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], "b");
    }

    #[test]
    fn test_incident_type_roundtrip() {
        let it = IncidentType {
            id: Some("abc".to_string()),
            name: "managed_incident".to_string(),
            display_name: "Managed Incident".to_string(),
            description: Some("The default type".to_string()),
            enabled: true,
            parent: None,
        };
        let json = serde_json::to_value(&it).unwrap();
        let roundtrip: IncidentType = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip.name, "managed_incident");
        assert_eq!(roundtrip.display_name, "Managed Incident");
        assert!(roundtrip.enabled);
    }
}
