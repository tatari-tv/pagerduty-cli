use crate::cli::{IncidentCommands, IncidentStatus};
use crate::client::{PdClient, encode_query};
use crate::config::Config;
use crate::filter;
use crate::output::print_value;
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::Path;
use tracing::{debug, instrument};

const INCIDENT_EXAMPLE_YAML: &str = include_str!("../../../examples/incident.yml");

pub fn example_if_requested(action: &IncidentCommands) -> Option<&'static str> {
    match action {
        IncidentCommands::Create { example: true, .. } => Some(INCIDENT_EXAMPLE_YAML),
        _ => None,
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct IncidentYaml {
    title: String,
    service: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    incident_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    escalation_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    urgency: Option<String>,
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

#[instrument(skip(client, config))]
#[allow(clippy::too_many_arguments)]
pub async fn list(
    client: &PdClient,
    config: &Config,
    patterns: &[String],
    statuses: &[IncidentStatus],
    priorities: &[String],
    team: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
) -> Result<()> {
    debug!(
        patterns_len = patterns.len(),
        statuses_len = statuses.len(),
        priorities_len = priorities.len(),
        team = ?team,
        "incident list"
    );

    let mut params: Vec<String> = Vec::new();

    // Design: default to triggered+acknowledged when neither --status nor --since is provided.
    let effective_statuses: Vec<&str> = if !statuses.is_empty() {
        statuses.iter().map(status_str).collect()
    } else if since.is_none() {
        vec!["triggered", "acknowledged"]
    } else {
        Vec::new()
    };
    for s in &effective_statuses {
        params.push(format!("statuses[]={}", s));
    }

    if let Some(t) = team {
        let team_id = crate::resources::team::resolve_team_id(client, t).await?;
        params.push(format!("team_ids[]={}", team_id));
    }

    if !priorities.is_empty() {
        let resolved = resolve_priority_ids(client, priorities).await?;
        for pid in resolved {
            params.push(format!("priority_ids[]={}", pid));
        }
    }

    if let Some(s) = since {
        params.push(format!("since={}", encode_query(s)));
    }
    if let Some(u) = until {
        params.push(format!("until={}", encode_query(u)));
    }

    let path = if params.is_empty() {
        "/incidents".to_string()
    } else {
        format!("/incidents?{}", params.join("&"))
    };
    let all = client.get_all(&path, "incidents").await?;

    // /incidents does not support the `query` parameter; positional patterns
    // are applied client-side against each incident's `title`.
    let filtered = filter::filter_into(all, patterns, incident_title);
    let result = json!({ "incidents": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// get
// ---------------------------------------------------------------------------

#[instrument(skip(client, config))]
pub async fn get(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let resp = client.get(&format!("/incidents/{}", id)).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// create
// ---------------------------------------------------------------------------

#[instrument(skip(client, config, from_file))]
#[allow(clippy::too_many_arguments)]
pub async fn create(
    client: &PdClient,
    config: &Config,
    title: Option<&str>,
    service: Option<&str>,
    priority: Option<&str>,
    incident_type: Option<&str>,
    body: Option<&str>,
    from_email_override: Option<&str>,
    from_file: Option<&Path>,
) -> Result<()> {
    let from = resolve_from_email(config, from_email_override)?;

    let payload = match from_file {
        Some(path) => {
            let mut yaml = load_incident_yaml(path)?;
            if let Some(t) = title {
                yaml.title = t.to_string();
            }
            if let Some(s) = service {
                yaml.service = s.to_string();
            }
            if let Some(p) = priority {
                yaml.priority = Some(p.to_string());
            }
            if let Some(t) = incident_type {
                yaml.incident_type = Some(t.to_string());
            }
            if let Some(b) = body {
                yaml.body = Some(b.to_string());
            }
            incident_yaml_to_body(client, &yaml).await?
        }
        None => {
            let t = title.ok_or_else(|| {
                eyre::eyre!("`pd incident create` requires --title or --from-file")
            })?;
            let s = service.ok_or_else(|| {
                eyre::eyre!("`pd incident create` requires --service when not using --from-file")
            })?;
            let service_id = crate::resources::service::resolve_service_id(client, s).await?;
            let mut incident = json!({
                "type": "incident",
                "title": t,
                "service": { "id": service_id, "type": "service_reference" },
            });
            if let Some(p) = priority {
                let pid = resolve_priority_id(client, p).await?;
                incident["priority"] = json!({ "id": pid, "type": "priority_reference" });
            }
            if let Some(tname) = incident_type {
                let tid =
                    crate::resources::incident::types::resolve_incident_type_id(client, tname)
                        .await?;
                incident["incident_type"] = json!({ "id": tid, "type": "incident_type_reference" });
            }
            if let Some(b) = body {
                incident["body"] = json!({ "type": "incident_body", "details": b });
            }
            json!({ "incident": incident })
        }
    };

    let result = client.post_with_from("/incidents", payload, &from).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// update
// ---------------------------------------------------------------------------

#[instrument(skip(client, config))]
pub async fn update(
    client: &PdClient,
    config: &Config,
    id: &str,
    status: Option<&IncidentStatus>,
    priority: Option<&str>,
    title: Option<&str>,
    from_email_override: Option<&str>,
) -> Result<()> {
    if status.is_none() && priority.is_none() && title.is_none() {
        eyre::bail!(
            "`pd incident update` requires at least one of --status, --priority, or --title"
        );
    }

    let from = resolve_from_email(config, from_email_override)?;

    let mut incident = json!({ "type": "incident_reference" });
    if let Some(s) = status {
        incident["status"] = json!(status_str(s));
    }
    if let Some(t) = title {
        incident["title"] = json!(t);
    }
    if let Some(p) = priority {
        if p.is_empty() {
            incident["priority"] = Value::Null;
        } else {
            let pid = resolve_priority_id(client, p).await?;
            incident["priority"] = json!({ "id": pid, "type": "priority_reference" });
        }
    }

    let body = json!({ "incident": incident });
    let result = client
        .put_with_from(&format!("/incidents/{}", id), body, &from)
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

pub fn status_str(s: &IncidentStatus) -> &'static str {
    match s {
        IncidentStatus::Triggered => "triggered",
        IncidentStatus::Acknowledged => "acknowledged",
        IncidentStatus::Resolved => "resolved",
    }
}

pub fn resolve_from_email(config: &Config, override_email: Option<&str>) -> Result<String> {
    if let Some(e) = override_email {
        return Ok(e.to_string());
    }
    config.from_email.clone().ok_or_else(|| {
        eyre::eyre!(
            "Missing requester email. Set PAGERDUTY_FROM_EMAIL, pass --from <email>, or add from-email to ~/.config/pagerduty-cli/pagerduty-cli.yml"
        )
    })
}

async fn resolve_priority_id(client: &PdClient, name_or_id: &str) -> Result<String> {
    let priorities = client.get("/priorities").await?;
    let list = priorities
        .get("priorities")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let matched = list.iter().find(|p| {
        let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
        id == name_or_id || name.eq_ignore_ascii_case(name_or_id)
    });
    matched
        .and_then(|p| p.get("id").and_then(|v| v.as_str()).map(String::from))
        .ok_or_else(|| eyre::eyre!("Priority {:?} not found", name_or_id))
}

async fn resolve_priority_ids(client: &PdClient, names_or_ids: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(names_or_ids.len());
    for n in names_or_ids {
        out.push(resolve_priority_id(client, n).await?);
    }
    Ok(out)
}

fn incident_title(value: &Value) -> &str {
    value.get("title").and_then(|v| v.as_str()).unwrap_or("")
}

fn load_incident_yaml(path: &Path) -> Result<IncidentYaml> {
    let content = read_path_or_stdin(path)?;
    serde_yaml::from_str::<IncidentYaml>(&content)
        .with_context(|| format!("Failed to parse incident YAML from {}", path.display()))
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

async fn incident_yaml_to_body(client: &PdClient, yaml: &IncidentYaml) -> Result<Value> {
    let service_id = crate::resources::service::resolve_service_id(client, &yaml.service).await?;
    let mut incident = json!({
        "type": "incident",
        "title": yaml.title,
        "service": { "id": service_id, "type": "service_reference" },
    });
    if let Some(p) = &yaml.priority {
        let pid = resolve_priority_id(client, p).await?;
        incident["priority"] = json!({ "id": pid, "type": "priority_reference" });
    }
    if let Some(t) = &yaml.incident_type {
        let tid = crate::resources::incident::types::resolve_incident_type_id(client, t).await?;
        incident["incident_type"] = json!({ "id": tid, "type": "incident_type_reference" });
    }
    if let Some(b) = &yaml.body {
        incident["body"] = json!({ "type": "incident_body", "details": b });
    }
    if let Some(u) = &yaml.urgency {
        incident["urgency"] = json!(u);
    }
    if let Some(ep) = &yaml.escalation_policy {
        let ep_id = crate::resources::escalation::resolve_escalation_id(client, ep).await?;
        incident["escalation_policy"] =
            json!({ "id": ep_id, "type": "escalation_policy_reference" });
    }
    Ok(json!({ "incident": incident }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn incident_title_reads_title_field() {
        let v = json!({"id": "P1", "title": "Database down"});
        assert_eq!(incident_title(&v), "Database down");
    }

    #[test]
    fn status_str_values() {
        assert_eq!(status_str(&IncidentStatus::Triggered), "triggered");
        assert_eq!(status_str(&IncidentStatus::Acknowledged), "acknowledged");
        assert_eq!(status_str(&IncidentStatus::Resolved), "resolved");
    }

    #[test]
    fn resolve_from_email_prefers_override() {
        let config = Config {
            api_token: "x".to_string(),
            from_email: Some("config@example.com".to_string()),
            subdomain: None,
            output_format: crate::cli::OutputFormat::Auto,
            log_level: "warn".to_string(),
            routing_key: None,
        };
        let resolved = resolve_from_email(&config, Some("override@example.com")).unwrap();
        assert_eq!(resolved, "override@example.com");
    }

    #[test]
    fn resolve_from_email_errors_when_missing() {
        let config = Config {
            api_token: "x".to_string(),
            from_email: None,
            subdomain: None,
            output_format: crate::cli::OutputFormat::Auto,
            log_level: "warn".to_string(),
            routing_key: None,
        };
        assert!(resolve_from_email(&config, None).is_err());
    }

    #[test]
    fn example_yaml_parses() {
        let parsed: IncidentYaml = serde_yaml::from_str(INCIDENT_EXAMPLE_YAML).unwrap();
        assert_eq!(parsed.title, "Database connection pool exhausted");
        assert_eq!(parsed.service, "Platform API");
    }
}
