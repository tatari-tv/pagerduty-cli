//! `pd log` - PagerDuty log entries (activity audit log).
//!
//! `/log_entries` does NOT support the `query` parameter, so pattern filtering
//! is applied client-side against each entry's `summary`.

use crate::cli::LogAction;
use crate::client::{PdClient, encode_query};
use crate::config::Config;
use crate::filter;
use crate::output::print_value;
use chrono::{Duration, Utc};
use eyre::Result;
use serde_json::{Value, json};
use tracing::{debug, instrument};

/// Default time window for `log list` when the caller doesn't pass `--since`.
/// Without this default the PagerDuty API returns days of entries, which
/// dumped megabytes of JSON in v0.6.5. The explicit ceiling keeps interactive
/// use responsive; pass `--since` to override.
const DEFAULT_LOG_WINDOW_HOURS: i64 = 24;

pub async fn handle(action: &LogAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        LogAction::List {
            patterns,
            since,
            until,
        } => list(client, config, patterns, since.as_deref(), until.as_deref()).await,
        LogAction::Get { id } => get(client, config, id).await,
    }
}

#[instrument(skip(client, config))]
async fn list(
    client: &PdClient,
    config: &Config,
    patterns: &[String],
    since: Option<&str>,
    until: Option<&str>,
) -> Result<()> {
    debug!(patterns_len = patterns.len(), since = ?since, until = ?until, "log list");
    let mut params: Vec<String> = Vec::new();

    // Apply a 24h default when neither --since nor --until is passed. This
    // caps what would otherwise be multiple days of log entries (megabytes
    // of JSON) in interactive use. Explicit --until alone still gets the
    // default lower bound so the window stays bounded on both ends.
    let default_since_storage;
    let effective_since = match (since, until) {
        (Some(s), _) => Some(s),
        (None, _) => {
            default_since_storage =
                (Utc::now() - Duration::hours(DEFAULT_LOG_WINDOW_HOURS)).to_rfc3339();
            Some(default_since_storage.as_str())
        }
    };
    if let Some(s) = effective_since {
        params.push(format!("since={}", encode_query(s)));
    }
    if let Some(u) = until {
        params.push(format!("until={}", encode_query(u)));
    }
    let path = if params.is_empty() {
        "/log_entries".to_string()
    } else {
        format!("/log_entries?{}", params.join("&"))
    };
    let all = client.get_all(&path, "log_entries").await?;
    let filtered = filter::filter_into(all, patterns, log_summary);
    let result = json!({ "log_entries": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let resp = client.get(&format!("/log_entries/{}", id)).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

fn log_summary(value: &Value) -> &str {
    value.get("summary").and_then(|v| v.as_str()).unwrap_or("")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn log_summary_reads_summary() {
        let v = json!({"id": "L1", "summary": "Triggered on service X"});
        assert_eq!(log_summary(&v), "Triggered on service X");
    }
}
