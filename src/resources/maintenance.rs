use crate::cli::MaintenanceAction;
use crate::client::PdClient;
use crate::config::Config;
use crate::filter;
use crate::output::print_value;
use crate::resources::service::resolve_service_id;
use crate::resources::team::resolve_team_id;
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::Path;
use tracing::{debug, instrument};

const EXAMPLE_YAML: &str = include_str!("../../examples/maintenance.yml");

pub fn example_if_requested(action: &MaintenanceAction) -> Option<&'static str> {
    match action {
        MaintenanceAction::Create { example: true, .. } => Some(EXAMPLE_YAML),
        _ => None,
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct MaintenanceYaml {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    start_time: String,
    end_time: String,
    /// Service names, slugs, or IDs to cover.
    services: Vec<String>,
}

pub async fn handle(action: &MaintenanceAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        MaintenanceAction::List {
            patterns,
            team,
            service,
        } => list(client, config, patterns, team.as_deref(), service.as_deref()).await,
        MaintenanceAction::Get { id } => get(client, config, id).await,
        MaintenanceAction::Create {
            service,
            start,
            end,
            description,
            from_file,
            example: _,
        } => {
            create(
                client,
                config,
                service,
                start.as_deref(),
                end.as_deref(),
                description.as_deref(),
                from_file.as_deref(),
            )
            .await
        }
        MaintenanceAction::Update {
            id,
            start,
            end,
            description,
        } => {
            update(
                client,
                config,
                id,
                start.as_deref(),
                end.as_deref(),
                description.as_deref(),
            )
            .await
        }
        MaintenanceAction::Delete { id } => delete(client, config, id).await,
    }
}

#[instrument(skip(client, config))]
async fn list(
    client: &PdClient,
    config: &Config,
    patterns: &[String],
    team: Option<&str>,
    service: Option<&str>,
) -> Result<()> {
    debug!(patterns_len = patterns.len(), team = ?team, service = ?service, "maintenance list");
    let mut params: Vec<String> = Vec::new();
    if let Some(t) = team {
        let team_id = resolve_team_id(client, t).await?;
        params.push(format!("team_ids[]={}", team_id));
    }
    if let Some(s) = service {
        let svc_id = resolve_service_id(client, s).await?;
        params.push(format!("service_ids[]={}", svc_id));
    }
    let base_path = if params.is_empty() {
        "/maintenance_windows".to_string()
    } else {
        format!("/maintenance_windows?{}", params.join("&"))
    };
    let all = if patterns.is_empty() {
        client.get_all(&base_path, "maintenance_windows").await?
    } else {
        client
            .query_all_patterns(&base_path, "maintenance_windows", patterns)
            .await?
    };
    let filtered = filter::filter_into(all, patterns, maintenance_name);
    let result = json!({ "maintenance_windows": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let resp = client.get(&format!("/maintenance_windows/{}", id)).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn create(
    client: &PdClient,
    config: &Config,
    service: &[String],
    start: Option<&str>,
    end: Option<&str>,
    description: Option<&str>,
    from_file: Option<&Path>,
) -> Result<()> {
    let body = match from_file {
        Some(path) => {
            let mut yaml = load_maintenance_yaml(path)?;
            if !service.is_empty() {
                yaml.services = service.to_vec();
            }
            if let Some(s) = start {
                yaml.start_time = s.to_string();
            }
            if let Some(e) = end {
                yaml.end_time = e.to_string();
            }
            if let Some(d) = description {
                yaml.description = Some(d.to_string());
            }
            maintenance_yaml_to_body(client, &yaml).await?
        }
        None => {
            if service.is_empty() {
                eyre::bail!("`pd maintenance create` requires --service (or --from-file)");
            }
            let s = start
                .ok_or_else(|| eyre::eyre!("`pd maintenance create` requires --start when not using --from-file"))?;
            let e =
                end.ok_or_else(|| eyre::eyre!("`pd maintenance create` requires --end when not using --from-file"))?;
            let service_refs = resolve_service_refs(client, service).await?;
            let mut mw = json!({
                "type": "maintenance_window",
                "start_time": s,
                "end_time": e,
                "services": service_refs,
            });
            if let Some(d) = description {
                mw["description"] = json!(d);
            }
            json!({ "maintenance_window": mw })
        }
    };
    let result = client.post("/maintenance_windows", body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn update(
    client: &PdClient,
    config: &Config,
    id: &str,
    start: Option<&str>,
    end: Option<&str>,
    description: Option<&str>,
) -> Result<()> {
    if start.is_none() && end.is_none() && description.is_none() {
        eyre::bail!("`pd maintenance update` requires at least one of --start, --end, or --description");
    }

    // PagerDuty's PUT /maintenance_windows/{id} expects the full record:
    // omitting `services` or `start_time`/`end_time` resets or rejects the
    // request. Fetch the current window and overlay only the fields the
    // caller supplied, matching the fetch-overlay-PUT pattern used in
    // src/resources/team.rs::update.
    let current = client.get(&format!("/maintenance_windows/{}", id)).await?;
    let mut mw = current.get("maintenance_window").cloned().ok_or_else(|| {
        eyre::eyre!(
            "GET /maintenance_windows/{} returned no maintenance_window envelope",
            id
        )
    })?;

    if let Some(s) = start {
        mw["start_time"] = json!(s);
    }
    if let Some(e) = end {
        mw["end_time"] = json!(e);
    }
    if let Some(d) = description {
        mw["description"] = json!(d);
    }
    let body = json!({ "maintenance_window": mw });
    let result = client.put(&format!("/maintenance_windows/{}", id), body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn delete(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let result = client.delete(&format!("/maintenance_windows/{}", id)).await?;
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

async fn maintenance_yaml_to_body(client: &PdClient, yaml: &MaintenanceYaml) -> Result<Value> {
    let service_refs = resolve_service_refs(client, &yaml.services).await?;
    let mut mw = json!({
        "type": "maintenance_window",
        "start_time": yaml.start_time,
        "end_time": yaml.end_time,
        "services": service_refs,
    });
    if let Some(d) = &yaml.description {
        mw["description"] = json!(d);
    }
    Ok(json!({ "maintenance_window": mw }))
}

fn load_maintenance_yaml(path: &Path) -> Result<MaintenanceYaml> {
    let content = read_path_or_stdin(path)?;
    serde_yaml::from_str::<MaintenanceYaml>(&content)
        .with_context(|| format!("Failed to parse maintenance window YAML from {}", path.display()))
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

fn maintenance_name(value: &Value) -> &str {
    value.get("description").and_then(|v| v.as_str()).unwrap_or("")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn maintenance_name_reads_description() {
        let v = json!({"id": "PMW1", "description": "DB migration"});
        assert_eq!(maintenance_name(&v), "DB migration");
    }

    #[test]
    fn example_yaml_parses() {
        let parsed: MaintenanceYaml = serde_yaml::from_str(EXAMPLE_YAML).unwrap();
        assert_eq!(parsed.services.len(), 1);
        assert!(parsed.start_time.starts_with("2026-"));
    }
}
