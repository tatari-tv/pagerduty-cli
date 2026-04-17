//! `pd change` - PagerDuty change events (read-only via REST).
//!
//! `/change_events` does NOT support the `query` parameter, so pattern
//! filtering is applied client-side against each entry's `summary`.
//!
//! NOTE: Creating change events requires the Events API v2
//! (`POST https://events.pagerduty.com/v2/change/enqueue`) with an integration
//! routing key - a different base URL and auth model from the REST client
//! used here. `create` is intentionally omitted; use `pd rest` passthrough
//! against the Events API until we add routing-key support to the client.

use crate::cli::ChangeAction;
use crate::client::{PdClient, encode_query};
use crate::config::Config;
use crate::filter;
use crate::output::print_value;
use crate::resources::service::resolve_service_id;
use eyre::Result;
use serde_json::{Value, json};
use tracing::{debug, instrument};

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
}
