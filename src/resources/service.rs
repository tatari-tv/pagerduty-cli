use crate::cli::{ServiceAction, ServiceIntegrationAction};
use crate::client::{PdClient, encode_query};
use crate::config::Config;
use crate::filter;
use crate::output::print_value;
use crate::resources::escalation::resolve_escalation_id;
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::Path;
use tracing::{debug, instrument};

const SERVICE_EXAMPLE_YAML: &str = include_str!("../../examples/service.yml");
const INTEGRATION_EXAMPLE_YAML: &str = include_str!("../../examples/integration.yml");

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ServiceYaml {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    escalation_policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auto_resolve_timeout: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    acknowledgement_timeout: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    alert_creation: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    teams: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct IntegrationYaml {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(rename = "type")]
    integration_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vendor: Option<String>,
}

pub fn example_if_requested(action: &ServiceAction) -> Option<&'static str> {
    match action {
        ServiceAction::Create { example: true, .. } => Some(SERVICE_EXAMPLE_YAML),
        ServiceAction::Integration {
            action: ServiceIntegrationAction::Create { example: true, .. },
        } => Some(INTEGRATION_EXAMPLE_YAML),
        _ => None,
    }
}

pub async fn handle(action: &ServiceAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        ServiceAction::List { patterns, team } => list(client, config, patterns, team.as_deref()).await,
        ServiceAction::Get { name_or_id } => get(client, config, name_or_id).await,
        ServiceAction::Create {
            name,
            escalation,
            description,
            from_file,
            example: _,
        } => {
            create(
                client,
                config,
                name.as_deref(),
                escalation.as_deref(),
                description.as_deref(),
                from_file.as_deref(),
            )
            .await
        }
        ServiceAction::Update { name_or_id, from_file } => {
            update(client, config, name_or_id, from_file.as_deref()).await
        }
        ServiceAction::Delete { name_or_id } => delete(client, config, name_or_id).await,
        ServiceAction::Integration { action } => integration(client, config, action).await,
    }
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, patterns: &[String], team: Option<&str>) -> Result<()> {
    debug!(patterns_len = patterns.len(), team = ?team, "service list");

    let mut params: Vec<String> = Vec::new();
    if let Some(t) = team {
        let team_id = crate::resources::team::resolve_team_id(client, t).await?;
        params.push(format!("team_ids[]={}", team_id));
    }
    let base_path = if params.is_empty() {
        "/services".to_string()
    } else {
        format!("/services?{}", params.join("&"))
    };
    let all = if patterns.is_empty() {
        client.get_all(&base_path, "services").await?
    } else {
        client.query_all_patterns(&base_path, "services", patterns).await?
    };

    let filtered = filter::filter_into(all, patterns, service_name);
    let result = json!({ "services": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, name_or_id: &str) -> Result<()> {
    let resp = resolve_service(client, name_or_id).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn create(
    client: &PdClient,
    config: &Config,
    name: Option<&str>,
    escalation: Option<&str>,
    description: Option<&str>,
    from_file: Option<&Path>,
) -> Result<()> {
    let body = match from_file {
        Some(path) => {
            let mut yaml = load_service_yaml(path)?;
            if let Some(n) = name {
                yaml.name = n.to_string();
            }
            if let Some(e) = escalation {
                yaml.escalation_policy = e.to_string();
            }
            if let Some(d) = description {
                yaml.description = Some(d.to_string());
            }
            service_yaml_to_body(client, &yaml).await?
        }
        None => {
            let n = name.ok_or_else(|| eyre::eyre!("`pd service create` requires --name or --from-file"))?;
            let e = escalation
                .ok_or_else(|| eyre::eyre!("`pd service create` requires --escalation when not using --from-file"))?;
            let ep_id = resolve_escalation_id(client, e).await?;
            let mut svc = json!({
                "name": n,
                "escalation_policy": { "id": ep_id, "type": "escalation_policy_reference" },
            });
            if let Some(d) = description {
                svc["description"] = json!(d);
            }
            json!({ "service": svc })
        }
    };
    let result = client.post("/services", body).await?;

    if let Some(cache) = client.cache()
        && let Some(new_id) = result.get("service").and_then(|s| s.get("id")).and_then(|v| v.as_str())
        && let Some(new_name) = result
            .get("service")
            .and_then(|s| s.get("name"))
            .and_then(|v| v.as_str())
    {
        cache.put("service", new_name, new_id);
    }

    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn update(client: &PdClient, config: &Config, name_or_id: &str, from_file: Option<&Path>) -> Result<()> {
    let path = from_file.ok_or_else(|| eyre::eyre!("`pd service update` requires --from-file"))?;
    let id = resolve_service_id(client, name_or_id).await?;
    let yaml = load_service_yaml(path)?;
    let body = service_yaml_to_body(client, &yaml).await?;
    let result = client.put(&format!("/services/{}", id), body).await?;

    if let Some(cache) = client.cache()
        && let Some(new_name) = result
            .get("service")
            .and_then(|s| s.get("name"))
            .and_then(|v| v.as_str())
    {
        if new_name != name_or_id {
            cache.invalidate_entry("service", name_or_id);
        }
        cache.put("service", new_name, &id);
    }

    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn delete(client: &PdClient, config: &Config, name_or_id: &str) -> Result<()> {
    let id = resolve_service_id(client, name_or_id).await?;
    let result = client.delete(&format!("/services/{}", id)).await?;
    if let Some(cache) = client.cache() {
        cache.invalidate_entry("service", name_or_id);
    }
    print_value(&result, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// Integration subresource
// ---------------------------------------------------------------------------

#[instrument(skip(client, config))]
async fn integration(client: &PdClient, config: &Config, action: &ServiceIntegrationAction) -> Result<()> {
    match action {
        ServiceIntegrationAction::List { service, patterns } => {
            integration_list(client, config, service, patterns).await
        }
        ServiceIntegrationAction::Get {
            service,
            integration_id,
        } => integration_get(client, config, service, integration_id).await,
        ServiceIntegrationAction::Create {
            service,
            integration_type,
            name,
            from_file,
            example: _,
        } => {
            integration_create(
                client,
                config,
                service,
                integration_type.as_deref(),
                name.as_deref(),
                from_file.as_deref(),
            )
            .await
        }
        ServiceIntegrationAction::Delete {
            service,
            integration_id,
        } => integration_delete(client, config, service, integration_id).await,
    }
}

#[instrument(skip(client, config))]
async fn integration_list(client: &PdClient, config: &Config, service: &str, patterns: &[String]) -> Result<()> {
    let service_id = resolve_service_id(client, service).await?;
    // GET /services/{id} with include[]=integrations is the documented path;
    // there is no standalone list endpoint on integrations.
    let resp = client
        .get(&format!("/services/{}?include[]=integrations", service_id))
        .await?;
    let integrations = resp
        .get("service")
        .and_then(|s| s.get("integrations"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let filtered = filter::filter_into(integrations, patterns, integration_name);
    let result = json!({ "integrations": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn integration_get(client: &PdClient, config: &Config, service: &str, integration_id: &str) -> Result<()> {
    let service_id = resolve_service_id(client, service).await?;
    let resp = client
        .get(&format!("/services/{}/integrations/{}", service_id, integration_id))
        .await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn integration_create(
    client: &PdClient,
    config: &Config,
    service: &str,
    integration_type: Option<&str>,
    name: Option<&str>,
    from_file: Option<&Path>,
) -> Result<()> {
    let service_id = resolve_service_id(client, service).await?;
    let body = match from_file {
        Some(path) => {
            let mut yaml = load_integration_yaml(path)?;
            if let Some(t) = integration_type {
                yaml.integration_type = t.to_string();
            }
            if let Some(n) = name {
                yaml.name = Some(n.to_string());
            }
            integration_yaml_to_body(&yaml)
        }
        None => {
            let t = integration_type
                .ok_or_else(|| eyre::eyre!("`pd service integration create` requires --type (or --from-file)"))?;
            let mut integration = json!({ "type": t });
            if let Some(n) = name {
                integration["name"] = json!(n);
            }
            json!({ "integration": integration })
        }
    };
    let result = client
        .post(&format!("/services/{}/integrations", service_id), body)
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn integration_delete(client: &PdClient, config: &Config, service: &str, integration_id: &str) -> Result<()> {
    let service_id = resolve_service_id(client, service).await?;
    let result = client
        .delete(&format!("/services/{}/integrations/{}", service_id, integration_id))
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// Resolution + YAML
// ---------------------------------------------------------------------------

/// Resolve a service identifier (ID or name) to the full
/// `{"service": {...}}` envelope. Cache integration lives here so that any
/// caller reaching for the full record - not just `resolve_service_id` -
/// benefits from a warm cache. The flow:
///
/// 1. `try_get(/services/{name_or_id})`: cheapest path. Works when the
///    caller passed an ID, and returns early.
/// 2. Not an ID. If the cache has a `(name -> id)` entry, try
///    `try_get(/services/{cached_id})`. A 200 is the full record. A 404
///    means the cached ID is stale (deleted or renamed-away); invalidate
///    the entry and fall through.
/// 3. Name-based `?query=` list. On single match, cache `(name -> id)`
///    and return.
///
/// This gives us the 404-on-cached-id recovery the design doc calls for
/// without a separate verify GET: the `try_get` at step 2 IS the
/// verification and the returned body when present.
pub async fn resolve_service(client: &PdClient, name_or_id: &str) -> Result<Value> {
    if let Some(resp) = client.try_get(&format!("/services/{}", name_or_id)).await? {
        return Ok(resp);
    }

    if let Some(cache) = client.cache()
        && let Some(cached_id) = cache.get("service", name_or_id)
    {
        match client.try_get(&format!("/services/{}", cached_id)).await? {
            Some(resp) => return Ok(resp),
            None => {
                // Cached ID 404'd. Invalidate and fall through to name list.
                cache.invalidate_entry("service", name_or_id);
            }
        }
    }

    let all = client
        .get_all(&format!("/services?query={}", encode_query(name_or_id)), "services")
        .await?;
    let matches = filter::filter(&all, &[name_or_id.to_string()], service_name);
    match matches.as_slice() {
        [] => eyre::bail!("Service {:?} not found (tried ID and name).", name_or_id),
        [single] => {
            if let Some(cache) = client.cache()
                && let Some(id) = single.get("id").and_then(|v| v.as_str())
                && id != name_or_id
            {
                cache.put("service", name_or_id, id);
            }
            Ok(json!({ "service": *single }))
        }
        many => {
            let ids: Vec<&str> = many
                .iter()
                .filter_map(|s| s.get("id").and_then(|v| v.as_str()))
                .collect();
            eyre::bail!(
                "Service name {:?} matches {} services: {}. Use the ID to disambiguate.",
                name_or_id,
                ids.len(),
                ids.join(", ")
            )
        }
    }
}

pub async fn resolve_service_id(client: &PdClient, name_or_id: &str) -> Result<String> {
    let resolved = resolve_service(client, name_or_id).await?;
    resolved
        .get("service")
        .and_then(|s| s.get("id"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| eyre::eyre!("Resolved service missing id field"))
}

fn load_service_yaml(path: &Path) -> Result<ServiceYaml> {
    let content = read_path_or_stdin(path)?;
    serde_yaml::from_str::<ServiceYaml>(&content)
        .with_context(|| format!("Failed to parse service YAML from {}", path.display()))
}

fn load_integration_yaml(path: &Path) -> Result<IntegrationYaml> {
    let content = read_path_or_stdin(path)?;
    serde_yaml::from_str::<IntegrationYaml>(&content)
        .with_context(|| format!("Failed to parse integration YAML from {}", path.display()))
}

fn read_path_or_stdin(path: &Path) -> Result<String> {
    if path == Path::new("-") {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("Failed to read stdin")?;
        Ok(buf)
    } else {
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))
    }
}

async fn service_yaml_to_body(client: &PdClient, yaml: &ServiceYaml) -> Result<Value> {
    let ep_id = resolve_escalation_id(client, &yaml.escalation_policy).await?;
    let mut svc = json!({
        "name": yaml.name,
        "escalation_policy": { "id": ep_id, "type": "escalation_policy_reference" },
    });
    if let Some(desc) = &yaml.description {
        svc["description"] = json!(desc);
    }
    if let Some(t) = yaml.auto_resolve_timeout {
        svc["auto_resolve_timeout"] = json!(t);
    }
    if let Some(t) = yaml.acknowledgement_timeout {
        svc["acknowledgement_timeout"] = json!(t);
    }
    if let Some(ac) = &yaml.alert_creation {
        svc["alert_creation"] = json!(ac);
    }
    if !yaml.teams.is_empty() {
        let mut teams = Vec::with_capacity(yaml.teams.len());
        for t in &yaml.teams {
            let team_id = crate::resources::team::resolve_team_id(client, t).await?;
            teams.push(json!({"id": team_id, "type": "team_reference"}));
        }
        svc["teams"] = json!(teams);
    }
    Ok(json!({ "service": svc }))
}

fn integration_yaml_to_body(yaml: &IntegrationYaml) -> Value {
    let mut integration = json!({ "type": yaml.integration_type });
    if let Some(n) = &yaml.name {
        integration["name"] = json!(n);
    }
    if let Some(v) = &yaml.vendor {
        integration["vendor"] = json!({ "id": v, "type": "vendor_reference" });
    }
    json!({ "integration": integration })
}

fn service_name(value: &Value) -> &str {
    value.get("name").and_then(|v| v.as_str()).unwrap_or("")
}

fn integration_name(value: &Value) -> &str {
    value
        .get("name")
        .or_else(|| value.get("summary"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn service_name_reads_name() {
        let v = json!({"id": "PS1", "name": "Platform API"});
        assert_eq!(service_name(&v), "Platform API");
    }

    #[test]
    fn integration_name_falls_back_to_summary() {
        let v = json!({"id": "PI1", "summary": "Datadog"});
        assert_eq!(integration_name(&v), "Datadog");
    }

    #[test]
    fn service_example_yaml_parses() {
        let parsed: ServiceYaml = serde_yaml::from_str(SERVICE_EXAMPLE_YAML).unwrap();
        assert_eq!(parsed.name, "Platform API");
        assert_eq!(parsed.escalation_policy, "Platform On-Call");
    }

    #[test]
    fn integration_example_yaml_parses() {
        let parsed: IntegrationYaml = serde_yaml::from_str(INTEGRATION_EXAMPLE_YAML).unwrap();
        assert_eq!(parsed.integration_type, "events_api_v2_inbound_integration");
    }
}
