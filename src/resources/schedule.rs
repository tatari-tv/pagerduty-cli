use crate::cli::{ScheduleAction, ScheduleOverrideAction};
use crate::client::{PdClient, encode_query};
use crate::config::Config;
use crate::filter;
use crate::output::print_value;
use crate::resources::team::resolve_user_id;
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::Path;
use tracing::{debug, instrument};

/// Returns the skeleton YAML if the action requests `--example`, otherwise `None`.
pub fn example_if_requested(action: &ScheduleAction) -> Option<&'static str> {
    match action {
        ScheduleAction::Create { example: true, .. } => Some(EXAMPLE_YAML),
        _ => None,
    }
}

const EXAMPLE_YAML: &str = include_str!("../../examples/schedule.yml");

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ScheduleYaml {
    name: String,
    time_zone: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default)]
    schedule_layers: Vec<LayerYaml>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    teams: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct LayerYaml {
    name: String,
    users: Vec<String>,
    start: String,
    rotation_turn_length_seconds: u64,
    rotation_virtual_start: String,
}

pub async fn handle(action: &ScheduleAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        ScheduleAction::List { patterns } => list(client, config, patterns).await,
        ScheduleAction::Get { name_or_id } => get(client, config, name_or_id).await,
        ScheduleAction::Create {
            name,
            timezone,
            from_file,
            example: _,
        } => {
            create(
                client,
                config,
                name.as_deref(),
                timezone.as_deref(),
                from_file.as_deref(),
            )
            .await
        }
        ScheduleAction::Update {
            name_or_id,
            from_file,
        } => update(client, config, name_or_id, from_file.as_deref()).await,
        ScheduleAction::Delete { name_or_id } => delete(client, config, name_or_id).await,
        ScheduleAction::Override { action } => override_handler(client, config, action).await,
    }
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, patterns: &[String]) -> Result<()> {
    debug!(patterns_len = patterns.len(), "schedule list");
    let all = if patterns.is_empty() {
        client.get_all("/schedules", "schedules").await?
    } else {
        client
            .query_all_patterns("/schedules", "schedules", patterns)
            .await?
    };
    let filtered = filter::filter_into(all, patterns, schedule_name);
    let result = json!({ "schedules": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, name_or_id: &str) -> Result<()> {
    let resp = resolve_schedule(client, name_or_id).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn create(
    client: &PdClient,
    config: &Config,
    name: Option<&str>,
    timezone: Option<&str>,
    from_file: Option<&Path>,
) -> Result<()> {
    let body = match from_file {
        Some(path) => {
            let mut yaml = load_schedule_yaml(path)?;
            if let Some(n) = name {
                yaml.name = n.to_string();
            }
            if let Some(tz) = timezone {
                yaml.time_zone = tz.to_string();
            }
            schedule_yaml_to_body(&yaml, client).await?
        }
        None => {
            let n = name.ok_or_else(|| {
                eyre::eyre!("`pd schedule create` requires --name and --timezone (or --from-file with full definition)")
            })?;
            let tz = timezone.ok_or_else(|| {
                eyre::eyre!("`pd schedule create` requires --timezone when not using --from-file")
            })?;
            json!({
                "schedule": {
                    "name": n,
                    "time_zone": tz,
                    "schedule_layers": []
                }
            })
        }
    };
    let result = client.post("/schedules", body).await?;

    if let Some(cache) = client.cache()
        && let Some(new_id) = result
            .get("schedule")
            .and_then(|s| s.get("id"))
            .and_then(|v| v.as_str())
        && let Some(new_name) = result
            .get("schedule")
            .and_then(|s| s.get("name"))
            .and_then(|v| v.as_str())
    {
        cache.put("schedule", new_name, new_id);
    }

    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn update(
    client: &PdClient,
    config: &Config,
    name_or_id: &str,
    from_file: Option<&Path>,
) -> Result<()> {
    let path = from_file.ok_or_else(|| eyre::eyre!("`pd schedule update` requires --from-file"))?;
    let resolved = resolve_schedule(client, name_or_id).await?;
    let id = resolved
        .get("schedule")
        .and_then(|s| s.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Resolved schedule missing id field"))?
        .to_string();
    let yaml = load_schedule_yaml(path)?;
    let body = schedule_yaml_to_body(&yaml, client).await?;
    let result = client.put(&format!("/schedules/{}", id), body).await?;

    if let Some(cache) = client.cache()
        && let Some(new_name) = result
            .get("schedule")
            .and_then(|s| s.get("name"))
            .and_then(|v| v.as_str())
    {
        cache.invalidate_by_id("schedule", &id);
        cache.put("schedule", new_name, &id);
    }

    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn delete(client: &PdClient, config: &Config, name_or_id: &str) -> Result<()> {
    let resolved = resolve_schedule(client, name_or_id).await?;
    let id = resolved
        .get("schedule")
        .and_then(|s| s.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Resolved schedule missing id field"))?
        .to_string();
    let result = client.delete(&format!("/schedules/{}", id)).await?;
    if let Some(cache) = client.cache() {
        cache.invalidate_entry("schedule", name_or_id);
    }
    print_value(&result, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// Override subresource
// ---------------------------------------------------------------------------

#[instrument(skip(client, config))]
async fn override_handler(
    client: &PdClient,
    config: &Config,
    action: &ScheduleOverrideAction,
) -> Result<()> {
    match action {
        ScheduleOverrideAction::List {
            schedule,
            since,
            until,
        } => override_list(client, config, schedule, since.as_deref(), until.as_deref()).await,
        ScheduleOverrideAction::Create {
            schedule,
            user,
            start,
            end,
        } => override_create(client, config, schedule, user, start, end).await,
        ScheduleOverrideAction::Delete {
            schedule,
            override_id,
        } => override_delete(client, config, schedule, override_id).await,
    }
}

#[instrument(skip(client, config))]
async fn override_list(
    client: &PdClient,
    config: &Config,
    schedule: &str,
    since: Option<&str>,
    until: Option<&str>,
) -> Result<()> {
    let schedule_id = resolve_schedule_id(client, schedule).await?;
    let mut path = format!("/schedules/{}/overrides", schedule_id);
    let mut params: Vec<String> = Vec::new();
    if let Some(s) = since {
        params.push(format!("since={}", encode_query(s)));
    }
    if let Some(u) = until {
        params.push(format!("until={}", encode_query(u)));
    }
    if !params.is_empty() {
        path.push('?');
        path.push_str(&params.join("&"));
    }
    let resp = client.get(&path).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn override_create(
    client: &PdClient,
    config: &Config,
    schedule: &str,
    user: &str,
    start: &str,
    end: &str,
) -> Result<()> {
    let schedule_id = resolve_schedule_id(client, schedule).await?;
    let user_id = resolve_user_id(client, user).await?;
    let body = json!({
        "override": {
            "start": start,
            "end": end,
            "user": { "id": user_id, "type": "user_reference" }
        }
    });
    let result = client
        .post(&format!("/schedules/{}/overrides", schedule_id), body)
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn override_delete(
    client: &PdClient,
    config: &Config,
    schedule: &str,
    override_id: &str,
) -> Result<()> {
    let schedule_id = resolve_schedule_id(client, schedule).await?;
    let result = client
        .delete(&format!(
            "/schedules/{}/overrides/{}",
            schedule_id, override_id
        ))
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// Resolution helpers
// ---------------------------------------------------------------------------

/// Resolve a schedule identifier (ID or name) to the `{"schedule": {...}}`
/// envelope. See `resolve_service` for the cache + 404-recovery rationale.
pub async fn resolve_schedule(client: &PdClient, name_or_id: &str) -> Result<Value> {
    if let Some(resp) = client
        .try_get(&format!("/schedules/{}", name_or_id))
        .await?
    {
        return Ok(resp);
    }

    if let Some(cache) = client.cache()
        && let Some(cached_id) = cache.get("schedule", name_or_id)
    {
        match client.try_get(&format!("/schedules/{}", cached_id)).await? {
            Some(resp) => return Ok(resp),
            None => cache.invalidate_entry("schedule", name_or_id),
        }
    }

    let all = client
        .get_all(
            &format!("/schedules?query={}", encode_query(name_or_id)),
            "schedules",
        )
        .await?;
    let matches = filter::filter(&all, &[name_or_id.to_string()], schedule_name);
    match matches.as_slice() {
        [] => eyre::bail!("Schedule {:?} not found (tried ID and name).", name_or_id),
        [single] => {
            if let Some(cache) = client.cache()
                && let Some(id) = single.get("id").and_then(|v| v.as_str())
                && id != name_or_id
            {
                cache.put("schedule", name_or_id, id);
            }
            Ok(json!({ "schedule": *single }))
        }
        many => {
            let ids: Vec<&str> = many
                .iter()
                .filter_map(|s| s.get("id").and_then(|v| v.as_str()))
                .collect();
            eyre::bail!(
                "Schedule name {:?} matches {} schedules: {}. Use the ID to disambiguate.",
                name_or_id,
                ids.len(),
                ids.join(", ")
            )
        }
    }
}

pub async fn resolve_schedule_id(client: &PdClient, name_or_id: &str) -> Result<String> {
    let resolved = resolve_schedule(client, name_or_id).await?;
    resolved
        .get("schedule")
        .and_then(|s| s.get("id"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| eyre::eyre!("Resolved schedule missing id field"))
}

// ---------------------------------------------------------------------------
// YAML helpers
// ---------------------------------------------------------------------------

fn load_schedule_yaml(path: &Path) -> Result<ScheduleYaml> {
    let content = read_path_or_stdin(path)?;
    serde_yaml::from_str::<ScheduleYaml>(&content)
        .with_context(|| format!("Failed to parse schedule YAML from {}", path.display()))
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

/// Convert the YAML definition into the PD API body, resolving each user
/// identifier (email or ID) to a PD user reference.
async fn schedule_yaml_to_body(yaml: &ScheduleYaml, client: &PdClient) -> Result<Value> {
    let mut layers = Vec::with_capacity(yaml.schedule_layers.len());
    for layer in &yaml.schedule_layers {
        let mut users = Vec::with_capacity(layer.users.len());
        for u in &layer.users {
            let id = resolve_user_id(client, u).await?;
            users.push(json!({ "user": { "id": id, "type": "user_reference" } }));
        }
        layers.push(json!({
            "name": layer.name,
            "start": layer.start,
            "rotation_turn_length_seconds": layer.rotation_turn_length_seconds,
            "rotation_virtual_start": layer.rotation_virtual_start,
            "users": users,
        }));
    }

    let mut schedule = json!({
        "name": yaml.name,
        "time_zone": yaml.time_zone,
        "schedule_layers": layers,
    });

    if let Some(desc) = &yaml.description {
        schedule["description"] = json!(desc);
    }

    if !yaml.teams.is_empty() {
        // Design: team references accept ID or name via tiered match. For the
        // schedule definition we pass through whatever the YAML gives (the PD
        // API accepts IDs). Resolving names would require a team list lookup
        // that is not justified here until a user hits the need.
        let teams: Vec<Value> = yaml
            .teams
            .iter()
            .map(|t| json!({ "id": t, "type": "team_reference" }))
            .collect();
        schedule["teams"] = json!(teams);
    }

    Ok(json!({ "schedule": schedule }))
}

fn schedule_name(value: &Value) -> &str {
    value.get("name").and_then(|v| v.as_str()).unwrap_or("")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn schedule_name_reads_name() {
        let v = json!({"id": "PS1", "name": "Platform Primary"});
        assert_eq!(schedule_name(&v), "Platform Primary");
    }

    #[test]
    fn example_yaml_parses() {
        let parsed: ScheduleYaml = serde_yaml::from_str(EXAMPLE_YAML).unwrap();
        assert_eq!(parsed.name, "Platform Primary");
        assert_eq!(parsed.time_zone, "America/Los_Angeles");
        assert_eq!(parsed.schedule_layers.len(), 1);
        assert_eq!(
            parsed.schedule_layers[0].rotation_turn_length_seconds,
            604800
        );
    }
}
