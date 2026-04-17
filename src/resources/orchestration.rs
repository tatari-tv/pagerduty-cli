//! `pd orchestration` - PagerDuty Event Orchestrations.
//!
//! `/event_orchestrations` does NOT support the `query` parameter, so pattern
//! filtering is applied client-side. The `router` subresource manages the
//! routing rules attached to an orchestration.

use crate::cli::{OrchestrationAction, OrchestrationRouterAction};
use crate::client::PdClient;
use crate::config::Config;
use crate::filter;
use crate::output::print_value;
use eyre::{Context, Result};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::Path;
use tracing::{debug, instrument};

pub async fn handle(action: &OrchestrationAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        OrchestrationAction::List { patterns } => list(client, config, patterns).await,
        OrchestrationAction::Get { name_or_id } => get(client, config, name_or_id).await,
        OrchestrationAction::Router { action } => router(client, config, action).await,
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, patterns: &[String]) -> Result<()> {
    debug!(patterns_len = patterns.len(), "orchestration list");
    let all = client.get_all("/event_orchestrations", "orchestrations").await?;
    let filtered = filter::filter_into(all, patterns, orch_name);
    let result = json!({ "orchestrations": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, name_or_id: &str) -> Result<()> {
    let resp = resolve_orchestration(client, name_or_id).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn router(client: &PdClient, config: &Config, action: &OrchestrationRouterAction) -> Result<()> {
    match action {
        OrchestrationRouterAction::Get { orchestration } => router_get(client, config, orchestration).await,
        OrchestrationRouterAction::Update {
            orchestration,
            from_file,
        } => router_update(client, config, orchestration, from_file).await,
    }
}

#[instrument(skip(client, config))]
async fn router_get(client: &PdClient, config: &Config, orchestration: &str) -> Result<()> {
    let id = resolve_orchestration_id(client, orchestration).await?;
    let resp = client.get(&format!("/event_orchestrations/{}/router", id)).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn router_update(client: &PdClient, config: &Config, orchestration: &str, from_file: &Path) -> Result<()> {
    let id = resolve_orchestration_id(client, orchestration).await?;
    let body = load_router_body(from_file)?;
    let result = client
        .put(&format!("/event_orchestrations/{}/router", id), body)
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

pub async fn resolve_orchestration(client: &PdClient, name_or_id: &str) -> Result<Value> {
    if let Some(resp) = client.try_get(&format!("/event_orchestrations/{}", name_or_id)).await? {
        return Ok(resp);
    }
    let all = client.get_all("/event_orchestrations", "orchestrations").await?;
    let matches = filter::filter(&all, &[name_or_id.to_string()], orch_name);
    match matches.as_slice() {
        [] => eyre::bail!("Orchestration {:?} not found (tried ID and name).", name_or_id),
        [single] => Ok(json!({ "orchestration": *single })),
        many => {
            let ids: Vec<&str> = many
                .iter()
                .filter_map(|v| v.get("id").and_then(|x| x.as_str()))
                .collect();
            eyre::bail!(
                "Orchestration name {:?} matches {} records: {}. Use the ID to disambiguate.",
                name_or_id,
                ids.len(),
                ids.join(", ")
            )
        }
    }
}

pub async fn resolve_orchestration_id(client: &PdClient, name_or_id: &str) -> Result<String> {
    if let Some(cache) = client.cache()
        && let Some(id) = cache.get("orchestration", name_or_id)
    {
        return Ok(id);
    }
    let resolved = resolve_orchestration(client, name_or_id).await?;
    let id = resolved
        .get("orchestration")
        .and_then(|o| o.get("id"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| eyre::eyre!("Resolved orchestration missing id field"))?;
    if let Some(cache) = client.cache()
        && id != name_or_id
    {
        cache.put("orchestration", name_or_id, &id);
    }
    Ok(id)
}

/// Load a router definition from YAML or JSON. `-` reads stdin. The body is
/// passed through verbatim to PagerDuty; the schema is the orchestration
/// router payload documented in the PD API reference.
fn load_router_body(path: &Path) -> Result<Value> {
    let content = read_path_or_stdin(path)?;
    let trimmed = content.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        serde_json::from_str::<Value>(&content)
            .with_context(|| format!("Failed to parse router JSON from {}", path.display()))
    } else {
        serde_yaml::from_str::<Value>(&content)
            .with_context(|| format!("Failed to parse router YAML from {}", path.display()))
    }
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

fn orch_name(value: &Value) -> &str {
    value.get("name").and_then(|v| v.as_str()).unwrap_or("")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn orch_name_reads_name() {
        let v = json!({"id": "O1", "name": "Platform Router"});
        assert_eq!(orch_name(&v), "Platform Router");
    }
}
