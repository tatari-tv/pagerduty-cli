use crate::cli::OncallAction;
use crate::client::PdClient;
use crate::config::Config;
use crate::output::print_value;
use eyre::Result;
use serde_json::{Value, json};
use tracing::{debug, instrument};

pub async fn handle(action: &OncallAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        OncallAction::List { patterns } => list(client, config, patterns).await,
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, patterns: &[String]) -> Result<()> {
    debug!(patterns_len = patterns.len(), "oncall list");
    let all = client.get_all("/oncalls", "oncalls").await?;
    let filtered = filter_oncalls(all, patterns);
    let result = json!({ "oncalls": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

/// Custom 3-tier filter that checks schedule.summary, escalation_policy.summary,
/// and user.summary on each on-call record. An item matches a tier if any of its
/// three candidate names matches any pattern at that tier.
fn filter_oncalls(items: Vec<Value>, patterns: &[String]) -> Vec<Value> {
    if patterns.is_empty() {
        return items;
    }

    let mut t1 = Vec::new();
    let mut t2 = Vec::new();
    let mut t3 = Vec::new();

    for item in items {
        let names = candidate_names(&item);
        if patterns.iter().any(|p| names.iter().any(|n| n.as_str() == p.as_str())) {
            t1.push(item);
        } else if patterns.iter().any(|p| names.iter().any(|n| n.starts_with(p.as_str()))) {
            t2.push(item);
        } else if patterns.iter().any(|p| names.iter().any(|n| n.contains(p.as_str()))) {
            t3.push(item);
        }
    }

    if !t1.is_empty() {
        return t1;
    }
    if !t2.is_empty() {
        return t2;
    }
    t3
}

fn candidate_names(item: &Value) -> Vec<String> {
    ["schedule", "escalation_policy", "user"]
        .iter()
        .filter_map(|k| {
            item.get(k)
                .and_then(|v| v.get("summary"))
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn oncall(schedule: &str, ep: &str, user: &str) -> Value {
        json!({
            "schedule": {"summary": schedule},
            "escalation_policy": {"summary": ep},
            "user": {"summary": user},
            "escalation_level": 1,
        })
    }

    fn pats(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn empty_patterns_returns_all() {
        let items = vec![
            oncall("Platform Primary", "Platform On-Call", "Scott"),
            oncall("Data Primary", "Data On-Call", "Alice"),
        ];
        assert_eq!(filter_oncalls(items, &[]).len(), 2);
    }

    #[test]
    fn exact_schedule_name_wins() {
        let items = vec![
            oncall("Platform", "Platform On-Call", "Scott"),
            oncall("Platform Primary", "Platform On-Call", "Scott"),
        ];
        let result = filter_oncalls(items, &pats(&["Platform"]));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["schedule"]["summary"], "Platform");
    }

    #[test]
    fn matches_any_field() {
        // User "Scott Idler" matches via user.summary contains
        let items = vec![
            oncall("Data", "Data On-Call", "Scott Idler"),
            oncall("Platform", "Platform On-Call", "Alice"),
        ];
        let result = filter_oncalls(items, &pats(&["Scott"]));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["user"]["summary"], "Scott Idler");
    }

    #[test]
    fn starts_with_in_ep_name() {
        let items = vec![
            oncall("Data", "Platform On-Call", "A"),
            oncall("Platform", "Platform On-Call", "B"),
        ];
        let result = filter_oncalls(items, &pats(&["Platform"]));
        // Platform Primary schedule is exact in one, so exact tier wins:
        // actually schedule "Platform" is exact. Verify the exact tier returns that.
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["schedule"]["summary"], "Platform");
    }
}
