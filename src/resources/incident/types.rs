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
        IncidentTypeAction::List { patterns, filter } => {
            list(client, config, patterns, filter).await
        }
        IncidentTypeAction::Get { id_or_name } => get(client, config, id_or_name).await,
        IncidentTypeAction::Create {
            name,
            display_name,
            description,
            parent,
        } => {
            create(
                client,
                config,
                name,
                display_name,
                description.as_deref(),
                parent.as_deref(),
            )
            .await
        }
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
async fn list(
    client: &PdClient,
    config: &Config,
    patterns: &[String],
    filter: &TypeFilter,
) -> Result<()> {
    let all = client.get_all("/incidents/types", "incident_types").await?;
    let by_status = apply_filter(all, filter);
    // Pattern match on display_name, which is what users see in the UI. The
    // slug (name) is already ID-searchable via get.
    let filtered = crate::filter::filter_into(by_status, patterns, |v| {
        v.get("display_name").and_then(|x| x.as_str()).unwrap_or("")
    });
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

/// Resolve an incident-type identifier to its full API envelope.
/// Accepts a PagerDuty ID, slug, or display name.
///
/// Flow:
/// 1. `try_get(/incidents/types/{id_or_name})`: handles IDs and slugs in a
///    single cheap call (PagerDuty accepts both as path params). No cache
///    needed on this branch.
/// 2. Cache lookup keyed by display name only, with `try_get` verify: on
///    hit, one GET replaces the full list scan below. On 404, invalidate
///    the entry and fall through.
/// 3. Full list scan + case-insensitive display-name match. On exactly
///    one match, cache `(display_name -> id)` for next time.
///
/// The cache namespace `"incident-type"` stores ONLY display-name
/// mappings. Slugs and IDs never enter the cache, which eliminates the
/// aliasing hazard (one type's slug colliding with another's display
/// name within a shared namespace).
async fn resolve_type(client: &PdClient, id_or_name: &str) -> Result<Value> {
    if let Some(resp) = client
        .try_get(&format!("/incidents/types/{}", id_or_name))
        .await?
    {
        return Ok(resp);
    }

    if let Some(cache) = client.cache()
        && let Some(cached_id) = cache.get("incident-type", id_or_name)
    {
        match client
            .try_get(&format!("/incidents/types/{}", cached_id))
            .await?
        {
            Some(resp) => return Ok(resp),
            None => cache.invalidate_entry("incident-type", id_or_name),
        }
    }

    let all = client.get_all("/incidents/types", "incident_types").await?;
    let matches: Vec<&Value> = all
        .iter()
        .filter(|t| {
            t.get("display_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .eq_ignore_ascii_case(id_or_name)
        })
        .collect();

    match matches.as_slice() {
        [] => eyre::bail!(
            "Incident type {:?} not found (tried ID, slug, and display name).",
            id_or_name
        ),
        [single] => {
            if let Some(cache) = client.cache()
                && let Some(id) = single.get("id").and_then(|v| v.as_str())
            {
                cache.put("incident-type", id_or_name, id);
            }
            Ok(json!({ "incident_type": single }))
        }
        many => {
            let ids: Vec<&str> = many
                .iter()
                .filter_map(|t| t.get("id").and_then(|v| v.as_str()))
                .collect();
            eyre::bail!(
                "Display name {:?} matches {} incident types: {}. Use the slug or ID to disambiguate.",
                id_or_name,
                ids.len(),
                ids.join(", ")
            )
        }
    }
}

/// Extract the resolved incident-type ID from the envelope produced by
/// `resolve_type`.
fn extract_type_id(resolved: &Value) -> Result<String> {
    resolved
        .get("incident_type")
        .and_then(|t| t.get("id"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| eyre::eyre!("Resolved incident type missing id field"))
}

/// Resolve an incident type display name, slug, or ID to the API UUID.
/// Thin wrapper over `resolve_type`; all cache logic lives there.
pub async fn resolve_incident_type_id(client: &PdClient, id_or_name: &str) -> Result<String> {
    let resolved = resolve_type(client, id_or_name).await?;
    extract_type_id(&resolved)
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, id_or_name: &str) -> Result<()> {
    let resp = resolve_type(client, id_or_name).await?;
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
    parent: Option<&str>,
) -> Result<()> {
    let mut type_body = json!({
        "name": name,
        "display_name": display_name,
        "enabled": true
    });

    if let Some(desc) = description {
        type_body["description"] = json!(desc);
    }

    if let Some(parent_id_or_name) = parent {
        let parent_id = resolve_incident_type_id(client, parent_id_or_name)
            .await
            .with_context(|| {
                format!(
                    "Failed to resolve parent incident type {:?}",
                    parent_id_or_name
                )
            })?;
        type_body["parent_type"] = json!(parent_id);
    }

    let body = json!({ "incident_type": type_body });
    let result = client.post("/incidents/types", body).await?;

    // Only cache the display_name -> id mapping. Slugs are handled directly
    // by `try_get(/incidents/types/{slug})` in `resolve_type`, which is
    // already one API call; caching it saves nothing and would risk
    // colliding with another type's display_name in the shared namespace.
    if let Some(cache) = client.cache()
        && let Some(new_id) = result
            .get("incident_type")
            .and_then(|t| t.get("id"))
            .and_then(|v| v.as_str())
    {
        cache.put("incident-type", display_name, new_id);
    }

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
    // Resolve first so the user can pass an ID, slug, or display name. The
    // previous implementation called GET /incidents/types/{id_or_name}
    // directly and 404'd when id_or_name was a display name.
    let resolved = resolve_type(client, id_or_name).await?;
    let id = extract_type_id(&resolved)?;
    let raw = resolved
        .get("incident_type")
        .cloned()
        .ok_or_else(|| eyre::eyre!("Unexpected response: missing incident_type key"))?;

    let mut current: IncidentType =
        serde_json::from_value(raw).context("Failed to parse incident type from API")?;

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
    let result = client
        .put(&format!("/incidents/types/{}", id), body)
        .await?;

    // Reap every cached entry pointing at this incident type's id (the
    // current display_name plus any orphan mappings from earlier
    // out-of-band UI renames), then write the canonical
    // new_display_name -> id mapping.
    if let Some(cache) = client.cache()
        && let Some(new_display_name) = result
            .get("incident_type")
            .and_then(|t| t.get("display_name"))
            .and_then(|v| v.as_str())
    {
        cache.invalidate_by_id("incident-type", &id);
        cache.put("incident-type", new_display_name, &id);
    }

    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn field(client: &PdClient, config: &Config, action: &FieldAction) -> Result<()> {
    match action {
        FieldAction::List { type_id_or_name } => {
            let resolved = resolve_type(client, type_id_or_name).await?;
            let id = extract_type_id(&resolved)?;
            let resp = client
                .get(&format!("/incidents/types/{}/custom_fields", id))
                .await?;
            print_value(&resp, &config.output_format);
        }
        FieldAction::Create {
            type_id_or_name,
            name,
            data_type,
            field_type,
        } => {
            let resolved = resolve_type(client, type_id_or_name).await?;
            let id = extract_type_id(&resolved)?;
            let body = json!({
                "custom_field": {
                    "name": name,
                    "data_type": data_type,
                    "field_type": field_type
                }
            });
            let result = client
                .post(&format!("/incidents/types/{}/custom_fields", id), body)
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
                .filter(|t| {
                    t.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false) == want_enabled
                })
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

    #[test]
    fn test_extract_type_id_from_envelope() {
        let resolved = json!({"incident_type": {"id": "IT002", "name": "managed_incident"}});
        assert_eq!(extract_type_id(&resolved).unwrap(), "IT002");
    }

    #[test]
    fn test_extract_type_id_missing() {
        let resolved = json!({"not_incident_type": {}});
        assert!(extract_type_id(&resolved).is_err());
    }
}
