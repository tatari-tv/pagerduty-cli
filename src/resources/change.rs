//! `pd change` - PagerDuty change events.
//!
//! `/change_events` (REST) does NOT support the `query` parameter, so list
//! pattern filtering is applied client-side against each entry's
//! `summary`.
//!
//! Creating change events requires the Events API v2
//! (`POST https://events.pagerduty.com/v2/change/enqueue`) with an
//! integration routing key. We go through `PdClient::events_post`, which
//! switches base URL and drops the `Authorization` header; the routing
//! key travels in the body. See `create()` for the dynamic
//! service-to-routing-key discovery.

use crate::cli::ChangeAction;
use crate::client::{PdClient, encode_query};
use crate::config::Config;
use crate::filter;
use crate::output::print_value;
use crate::resources::service::resolve_service_id;
use chrono::Utc;
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::Path;
use tracing::{debug, instrument};

const EXAMPLE_YAML: &str = include_str!("../../examples/change.yml");

/// Integration type string PagerDuty uses to identify an Events API v2
/// inbound integration on a service. Confirmed against the live tatari
/// account (see design doc `2026-04-16-shakedown-v0.5.0.md`, Phase 3
/// pre-implementation verification).
const EVENTS_API_V2_INTEGRATION_TYPE: &str = "events_api_v2_inbound_integration";

pub fn example_if_requested(action: &ChangeAction) -> Option<&'static str> {
    match action {
        ChangeAction::Create { example: true, .. } => Some(EXAMPLE_YAML),
        _ => None,
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ChangeYaml {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    service: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    custom_details: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    links: Vec<LinkYaml>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct LinkYaml {
    href: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

pub async fn handle(action: &ChangeAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        ChangeAction::List {
            patterns,
            service,
            since,
            until,
        } => {
            list(
                client,
                config,
                patterns,
                service.as_deref(),
                since.as_deref(),
                until.as_deref(),
            )
            .await
        }
        ChangeAction::Get { id } => get(client, config, id).await,
        ChangeAction::Create {
            summary,
            service,
            links,
            routing_key,
            from_file,
            example: _,
        } => {
            create(
                client,
                config,
                summary.as_deref(),
                service.as_deref(),
                links,
                routing_key.as_deref(),
                from_file.as_deref(),
            )
            .await
        }
    }
}

#[instrument(skip(client, config))]
async fn list(
    client: &PdClient,
    config: &Config,
    patterns: &[String],
    service: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
) -> Result<()> {
    debug!(patterns_len = patterns.len(), service = ?service, since = ?since, until = ?until, "change list");
    let mut params: Vec<String> = Vec::new();
    if let Some(s) = service {
        let svc_id = resolve_service_id(client, s).await?;
        params.push(format!("service_ids[]={}", svc_id));
    }
    if let Some(s) = since {
        params.push(format!("since={}", encode_query(s)));
    }
    if let Some(u) = until {
        params.push(format!("until={}", encode_query(u)));
    }
    let path = if params.is_empty() {
        "/change_events".to_string()
    } else {
        format!("/change_events?{}", params.join("&"))
    };
    let all = client.get_all(&path, "change_events").await?;
    let filtered = filter::filter_into(all, patterns, change_summary);
    let result = json!({ "change_events": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let resp = client.get(&format!("/change_events/{}", id)).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn create(
    client: &PdClient,
    config: &Config,
    summary: Option<&str>,
    service: Option<&str>,
    links: &[String],
    routing_key_flag: Option<&str>,
    from_file: Option<&Path>,
) -> Result<()> {
    let yaml = match from_file {
        Some(path) => load_change_yaml(path)?,
        None => ChangeYaml::default(),
    };

    // CLI flags override file; file overrides defaults.
    let service_name = service
        .map(String::from)
        .or(yaml.service.clone())
        .ok_or_else(|| {
            eyre::eyre!(
                "`pd change create` requires --service (or `service:` in --from-file). --service also drives routing-key resolution."
            )
        })?;

    let summary_txt = summary
        .map(String::from)
        .or(yaml.summary.clone())
        .ok_or_else(|| eyre::eyre!("`pd change create` requires --summary (or `summary:` in --from-file)"))?;

    let timestamp = yaml.timestamp.clone().unwrap_or_else(|| Utc::now().to_rfc3339());

    // Precedence: --routing-key CLI > PAGERDUTY_ROUTING_KEY env / config >
    // dynamic lookup of the service's Events API v2 integration.
    let routing_key = match routing_key_flag {
        Some(k) => k.to_string(),
        None => match config.routing_key.clone() {
            Some(k) => k,
            None => discover_routing_key(client, &service_name).await?,
        },
    };

    // Source defaults to the service name; callers can override in YAML.
    let source = yaml.source.clone().unwrap_or_else(|| service_name.clone());

    // Links: CLI repeatable --links entries are "url|text" pairs. File
    // entries are already structured. Merge: CLI entries first, then file.
    let mut link_values: Vec<Value> = Vec::new();
    for raw in links {
        let (href, text) = parse_link(raw)?;
        let mut l = json!({ "href": href });
        if let Some(t) = text {
            l["text"] = json!(t);
        }
        link_values.push(l);
    }
    for l in &yaml.links {
        let mut v = json!({ "href": l.href });
        if let Some(t) = &l.text {
            v["text"] = json!(t);
        }
        link_values.push(v);
    }

    let mut payload = json!({
        "summary": summary_txt,
        "source": source,
        "timestamp": timestamp,
    });
    if let Some(details) = &yaml.custom_details {
        payload["custom_details"] = details.clone();
    }

    let mut body = json!({
        "routing_key": routing_key,
        "payload": payload,
    });
    if !link_values.is_empty() {
        body["links"] = Value::Array(link_values);
    }

    let result = client.events_post("/v2/change/enqueue", body).await?;
    print_value(&result, &config.output_format);
    Ok(())
}

async fn discover_routing_key(client: &PdClient, service_name: &str) -> Result<String> {
    let id = resolve_service_id(client, service_name).await?;
    // `resolve_service` only fetches the bare service record without
    // integrations; issue a targeted GET with `include[]=integrations`
    // here so the extra bytes stay localized to change-create.
    let resp = client.get(&format!("/services/{}?include[]=integrations", id)).await?;
    let integrations = resp
        .get("service")
        .and_then(|s| s.get("integrations"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let v2_key = integrations.iter().find_map(|i| {
        let is_v2 = i
            .get("type")
            .and_then(|v| v.as_str())
            .map(|t| t == EVENTS_API_V2_INTEGRATION_TYPE)
            .unwrap_or(false);
        if is_v2 {
            i.get("integration_key").and_then(|v| v.as_str()).map(String::from)
        } else {
            None
        }
    });

    v2_key.ok_or_else(|| {
        eyre::eyre!(
            "Service {:?} has no Events API v2 integration. \
             Create one (`pd service integration create {:?} \
             --type events_api_v2_inbound_integration`) or pass \
             --routing-key explicitly.",
            service_name,
            service_name
        )
    })
}

fn parse_link(raw: &str) -> Result<(String, Option<String>)> {
    let parts: Vec<&str> = raw.splitn(2, '|').collect();
    match parts.as_slice() {
        [href] => Ok((href.to_string(), None)),
        [href, text] => Ok((href.to_string(), Some(text.to_string()))),
        _ => eyre::bail!("Invalid --links entry {:?}. Expected `url` or `url|text`.", raw),
    }
}

fn load_change_yaml(path: &Path) -> Result<ChangeYaml> {
    let content = read_path_or_stdin(path)?;
    serde_yaml::from_str::<ChangeYaml>(&content)
        .with_context(|| format!("Failed to parse change YAML from {}", path.display()))
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

fn change_summary(value: &Value) -> &str {
    value.get("summary").and_then(|v| v.as_str()).unwrap_or("")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn change_summary_reads_summary() {
        let v = json!({"id": "C1", "summary": "Deployed v1.2.3 to prod"});
        assert_eq!(change_summary(&v), "Deployed v1.2.3 to prod");
    }

    #[test]
    fn parse_link_plain_href() {
        let (href, text) = parse_link("https://example.com/deploy/42").unwrap();
        assert_eq!(href, "https://example.com/deploy/42");
        assert_eq!(text, None);
    }

    #[test]
    fn parse_link_href_and_text() {
        let (href, text) = parse_link("https://example.com/deploy/42|Deploy #42").unwrap();
        assert_eq!(href, "https://example.com/deploy/42");
        assert_eq!(text.as_deref(), Some("Deploy #42"));
    }

    #[test]
    fn example_yaml_parses() {
        let parsed: ChangeYaml = serde_yaml::from_str(EXAMPLE_YAML).unwrap();
        assert!(parsed.summary.is_some());
    }
}
