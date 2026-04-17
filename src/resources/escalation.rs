use crate::cli::EscalationAction;
use crate::client::{PdClient, encode_query};
use crate::config::Config;
use crate::filter;
use crate::output::print_value;
use crate::resources::team::{resolve_team_id, resolve_user_id};
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::Path;
use tracing::{debug, instrument};

pub fn example_if_requested(action: &EscalationAction) -> Option<&'static str> {
    match action {
        EscalationAction::Create { example: true, .. } => Some(EXAMPLE_YAML),
        _ => None,
    }
}

const EXAMPLE_YAML: &str = include_str!("../../examples/escalation.yml");

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct EscalationYaml {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    teams: Vec<String>,
    escalation_rules: Vec<RuleYaml>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct RuleYaml {
    escalation_delay_in_minutes: u64,
    targets: Vec<TargetYaml>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct TargetYaml {
    #[serde(rename = "type")]
    kind: String,
    reference: String,
}

pub async fn handle(action: &EscalationAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        EscalationAction::List { patterns, team } => list(client, config, patterns, team.as_deref()).await,
        EscalationAction::Get { name_or_id } => get(client, config, name_or_id).await,
        EscalationAction::Create {
            name,
            team,
            from_file,
            example: _,
        } => create(client, config, name.as_deref(), team.as_deref(), from_file.as_deref()).await,
        EscalationAction::Update { name_or_id, from_file } => {
            update(client, config, name_or_id, from_file.as_deref()).await
        }
        EscalationAction::Delete { name_or_id } => delete(client, config, name_or_id).await,
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, patterns: &[String], team: Option<&str>) -> Result<()> {
    debug!(patterns_len = patterns.len(), team = ?team, "escalation list");

    let mut params: Vec<String> = Vec::new();
    if let Some(t) = team {
        let team_id = resolve_team_id(client, t).await?;
        params.push(format!("team_ids[]={}", team_id));
    }
    if patterns.len() == 1 {
        params.push(format!("query={}", encode_query(&patterns[0])));
    }
    let path = if params.is_empty() {
        "/escalation_policies".to_string()
    } else {
        format!("/escalation_policies?{}", params.join("&"))
    };
    let all = client.get_all(&path, "escalation_policies").await?;

    let filtered = filter::filter_into(all, patterns, ep_name);
    let result = json!({ "escalation_policies": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, name_or_id: &str) -> Result<()> {
    let resp = resolve_escalation(client, name_or_id).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn create(
    client: &PdClient,
    config: &Config,
    name: Option<&str>,
    team: Option<&str>,
    from_file: Option<&Path>,
) -> Result<()> {
    let body = match from_file {
        Some(path) => {
            let mut yaml = load_yaml(path)?;
            if let Some(n) = name {
                yaml.name = n.to_string();
            }
            if let Some(t) = team {
                yaml.teams = vec![t.to_string()];
            }
            yaml_to_body(client, &yaml).await?
        }
        None => {
            let n = name.ok_or_else(|| eyre::eyre!("`pd escalation create` requires --name (or a --from-file)"))?;
            let mut ep = json!({
                "name": n,
                "escalation_rules": [],
            });
            if let Some(t) = team {
                let team_id = resolve_team_id(client, t).await?;
                ep["teams"] = json!([{"id": team_id, "type": "team_reference"}]);
            }
            json!({ "escalation_policy": ep })
        }
    };
    let result = client.post("/escalation_policies", body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn update(client: &PdClient, config: &Config, name_or_id: &str, from_file: Option<&Path>) -> Result<()> {
    let path = from_file.ok_or_else(|| eyre::eyre!("`pd escalation update` requires --from-file"))?;
    let id = resolve_escalation_id(client, name_or_id).await?;
    let yaml = load_yaml(path)?;
    let body = yaml_to_body(client, &yaml).await?;
    let result = client.put(&format!("/escalation_policies/{}", id), body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn delete(client: &PdClient, config: &Config, name_or_id: &str) -> Result<()> {
    let id = resolve_escalation_id(client, name_or_id).await?;
    let result = client.delete(&format!("/escalation_policies/{}", id)).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

pub async fn resolve_escalation(client: &PdClient, name_or_id: &str) -> Result<Value> {
    if let Some(resp) = client.try_get(&format!("/escalation_policies/{}", name_or_id)).await? {
        return Ok(resp);
    }
    let all = client
        .get_all(
            &format!("/escalation_policies?query={}", encode_query(name_or_id)),
            "escalation_policies",
        )
        .await?;
    let matches = filter::filter(&all, &[name_or_id.to_string()], ep_name);
    match matches.as_slice() {
        [] => eyre::bail!("Escalation policy {:?} not found (tried ID and name).", name_or_id),
        [single] => Ok(json!({ "escalation_policy": *single })),
        many => {
            let ids: Vec<&str> = many
                .iter()
                .filter_map(|e| e.get("id").and_then(|v| v.as_str()))
                .collect();
            eyre::bail!(
                "Escalation policy name {:?} matches {} entries: {}. Use the ID to disambiguate.",
                name_or_id,
                ids.len(),
                ids.join(", ")
            )
        }
    }
}

pub async fn resolve_escalation_id(client: &PdClient, name_or_id: &str) -> Result<String> {
    let resolved = resolve_escalation(client, name_or_id).await?;
    resolved
        .get("escalation_policy")
        .and_then(|p| p.get("id"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| eyre::eyre!("Resolved escalation policy missing id field"))
}

// ---------------------------------------------------------------------------
// YAML helpers
// ---------------------------------------------------------------------------

fn load_yaml(path: &Path) -> Result<EscalationYaml> {
    let content = read_path_or_stdin(path)?;
    serde_yaml::from_str::<EscalationYaml>(&content)
        .with_context(|| format!("Failed to parse escalation YAML from {}", path.display()))
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

async fn yaml_to_body(client: &PdClient, yaml: &EscalationYaml) -> Result<Value> {
    let mut rules = Vec::with_capacity(yaml.escalation_rules.len());
    for rule in &yaml.escalation_rules {
        let mut targets = Vec::with_capacity(rule.targets.len());
        for t in &rule.targets {
            let (resolved_id, pd_type) = match t.kind.as_str() {
                "user" => (resolve_user_id(client, &t.reference).await?, "user_reference"),
                "schedule" => (
                    crate::resources::schedule::resolve_schedule_id(client, &t.reference).await?,
                    "schedule_reference",
                ),
                other => eyre::bail!(
                    "Unsupported escalation target type {:?}. Expected 'user' or 'schedule'.",
                    other
                ),
            };
            targets.push(json!({"id": resolved_id, "type": pd_type}));
        }
        rules.push(json!({
            "escalation_delay_in_minutes": rule.escalation_delay_in_minutes,
            "targets": targets,
        }));
    }

    let mut ep = json!({
        "name": yaml.name,
        "escalation_rules": rules,
    });

    if let Some(desc) = &yaml.description {
        ep["description"] = json!(desc);
    }

    if !yaml.teams.is_empty() {
        let mut teams = Vec::with_capacity(yaml.teams.len());
        for t in &yaml.teams {
            let team_id = resolve_team_id(client, t).await?;
            teams.push(json!({"id": team_id, "type": "team_reference"}));
        }
        ep["teams"] = json!(teams);
    }

    Ok(json!({ "escalation_policy": ep }))
}

fn ep_name(value: &Value) -> &str {
    value.get("name").and_then(|v| v.as_str()).unwrap_or("")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn ep_name_reads_name() {
        let v = json!({"id": "PEP1", "name": "Platform"});
        assert_eq!(ep_name(&v), "Platform");
    }

    #[test]
    fn example_yaml_parses() {
        let parsed: EscalationYaml = serde_yaml::from_str(EXAMPLE_YAML).unwrap();
        assert_eq!(parsed.name, "Platform On-Call");
        assert_eq!(parsed.escalation_rules.len(), 2);
        assert_eq!(parsed.escalation_rules[0].targets[0].kind, "schedule");
    }
}
