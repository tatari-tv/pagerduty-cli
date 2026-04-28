//! `pd alert-grouping` - PagerDuty Alert Grouping Settings.
//!
//! The endpoint is `/alert_grouping_settings` and does NOT support the `query`
//! parameter, so pattern filtering is applied client-side against each
//! setting's `name` field.

use crate::cli::AlertGroupingAction;
use crate::client::PdClient;
use crate::config::Config;
use crate::filter;
use crate::output::print_value;
use crate::resources::service::resolve_service_id;
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::Path;
use tracing::{debug, instrument};

const EXAMPLE_YAML: &str = include_str!("../../examples/alert-grouping.yml");

pub fn example_if_requested(action: &AlertGroupingAction) -> Option<&'static str> {
    match action {
        AlertGroupingAction::Create { example: true, .. } => Some(EXAMPLE_YAML),
        _ => None,
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct GroupingYaml {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(rename = "type")]
    grouping_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    /// Services the setting applies to (names, slugs, or IDs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    services: Vec<String>,
    /// Free-form `config` block passed through verbatim. Shape depends on
    /// the grouping `type` (intelligent vs content_based vs time); consult
    /// the PagerDuty Alert Grouping Settings API for the expected fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    config: Option<Value>,
}

pub async fn handle(
    action: &AlertGroupingAction,
    client: &PdClient,
    config: &Config,
) -> Result<()> {
    match action {
        AlertGroupingAction::List { patterns } => list(client, config, patterns).await,
        AlertGroupingAction::Get { id } => get(client, config, id).await,
        AlertGroupingAction::Create {
            service,
            grouping_type,
            name,
            from_file,
            example: _,
        } => {
            create(
                client,
                config,
                service,
                grouping_type.as_deref(),
                name.as_deref(),
                from_file.as_deref(),
            )
            .await
        }
        AlertGroupingAction::Update { id, from_file } => {
            update(client, config, id, from_file.as_deref()).await
        }
        AlertGroupingAction::Delete { id } => delete(client, config, id).await,
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, patterns: &[String]) -> Result<()> {
    debug!(patterns_len = patterns.len(), "alert-grouping list");
    let all = client
        .get_all_cursor("/alert_grouping_settings", "alert_grouping_settings")
        .await?;
    let filtered = filter::filter_into(all, patterns, grouping_name);
    let result = json!({ "alert_grouping_settings": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let resp = client
        .get(&format!("/alert_grouping_settings/{}", id))
        .await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn create(
    client: &PdClient,
    config: &Config,
    service: &[String],
    grouping_type: Option<&str>,
    name: Option<&str>,
    from_file: Option<&Path>,
) -> Result<()> {
    let body = match from_file {
        Some(path) => {
            let mut yaml = load_grouping_yaml(path)?;
            if !service.is_empty() {
                yaml.services = service.to_vec();
            }
            if let Some(t) = grouping_type {
                yaml.grouping_type = t.to_string();
            }
            if let Some(n) = name {
                yaml.name = Some(n.to_string());
            }
            grouping_yaml_to_body(client, &yaml).await?
        }
        None => {
            if service.is_empty() {
                eyre::bail!("`pd alert-grouping create` requires --service (or --from-file)");
            }
            let t = grouping_type.ok_or_else(|| {
                eyre::eyre!("`pd alert-grouping create` requires --type when not using --from-file")
            })?;
            let service_refs = resolve_service_refs(client, service).await?;
            let mut setting = json!({
                "type": t,
                "services": service_refs,
            });
            if let Some(n) = name {
                setting["name"] = json!(n);
            }
            json!({ "alert_grouping_setting": setting })
        }
    };
    let result = client.post("/alert_grouping_settings", body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn update(
    client: &PdClient,
    config: &Config,
    id: &str,
    from_file: Option<&Path>,
) -> Result<()> {
    let path =
        from_file.ok_or_else(|| eyre::eyre!("`pd alert-grouping update` requires --from-file"))?;
    let yaml = load_grouping_yaml(path)?;
    let body = grouping_yaml_to_body(client, &yaml).await?;
    let result = client
        .put(&format!("/alert_grouping_settings/{}", id), body)
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn delete(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let result = client
        .delete(&format!("/alert_grouping_settings/{}", id))
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

async fn resolve_service_refs(client: &PdClient, services: &[String]) -> Result<Vec<Value>> {
    let mut refs = Vec::with_capacity(services.len());
    for s in services {
        let id = resolve_service_id(client, s).await?;
        refs.push(json!({ "id": id, "type": "service_reference" }));
    }
    Ok(refs)
}

async fn grouping_yaml_to_body(client: &PdClient, yaml: &GroupingYaml) -> Result<Value> {
    let mut setting = json!({ "type": yaml.grouping_type });
    if let Some(n) = &yaml.name {
        setting["name"] = json!(n);
    }
    if let Some(d) = &yaml.description {
        setting["description"] = json!(d);
    }
    if !yaml.services.is_empty() {
        let refs = resolve_service_refs(client, &yaml.services).await?;
        setting["services"] = json!(refs);
    }
    if let Some(cfg) = &yaml.config {
        setting["config"] = cfg.clone();
    }
    Ok(json!({ "alert_grouping_setting": setting }))
}

fn load_grouping_yaml(path: &Path) -> Result<GroupingYaml> {
    let content = read_path_or_stdin(path)?;
    serde_yaml::from_str::<GroupingYaml>(&content).with_context(|| {
        format!(
            "Failed to parse alert grouping YAML from {}",
            path.display()
        )
    })
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

fn grouping_name(value: &Value) -> &str {
    value
        .get("name")
        .or_else(|| value.get("description"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn grouping_name_reads_name() {
        let v = json!({"id": "AG1", "name": "Platform intelligent"});
        assert_eq!(grouping_name(&v), "Platform intelligent");
    }

    #[test]
    fn grouping_name_falls_back_to_description() {
        let v = json!({"id": "AG1", "description": "Fallback"});
        assert_eq!(grouping_name(&v), "Fallback");
    }

    #[test]
    fn example_yaml_parses() {
        let parsed: GroupingYaml = serde_yaml::from_str(EXAMPLE_YAML).unwrap();
        assert_eq!(parsed.grouping_type, "intelligent");
        assert!(!parsed.services.is_empty());
    }
}
