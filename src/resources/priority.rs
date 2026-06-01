use crate::cli::PriorityAction;
use crate::client::PdClient;
use crate::config::Config;
use crate::output::print_value;
use colored::*;
use eyre::{Context, ContextCompat, Result, bail};
use serde::{Deserialize, Serialize};
use tracing::instrument;

const EXPECTED_PRIORITIES: &[&str] = &["P1", "P2", "P3", "P4"];

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Priority {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub order: Option<u32>,
}

pub async fn handle(action: &PriorityAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        PriorityAction::List => list(client, config).await,
        PriorityAction::Verify => verify(client).await,
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config) -> Result<()> {
    let resp = client.get("/priorities").await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client))]
async fn verify(client: &PdClient) -> Result<()> {
    let resp = client.get("/priorities").await?;

    let raw = resp
        .get("priorities")
        .context("Missing priorities key in response")?
        .clone();
    let priorities: Vec<Priority> = serde_json::from_value(raw).context("Failed to parse priorities")?;

    let names: Vec<&str> = priorities.iter().map(|p| p.name.as_str()).collect();

    let mut all_pass = true;
    for expected in EXPECTED_PRIORITIES {
        if names.contains(expected) {
            println!("{} {}", "✓".green(), expected);
        } else {
            println!("{} {} - MISSING", "✗".red(), expected);
            all_pass = false;
        }
    }

    println!();

    if all_pass {
        println!("{}", "All expected priorities present".green());
        Ok(())
    } else {
        bail!("Priority verification failed: missing one or more expected priorities (P1-P4)");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_priority_deserializes() {
        let val = json!({
            "id": "P123",
            "name": "P1",
            "description": "Critical",
            "color": "red",
            "order": 1
        });
        let p: Priority = serde_json::from_value(val).unwrap();
        assert_eq!(p.name, "P1");
        assert_eq!(p.id, "P123");
    }

    #[test]
    fn test_priority_description_optional() {
        let val = json!({"id": "P1", "name": "P1"});
        let p: Priority = serde_json::from_value(val).unwrap();
        assert!(p.description.is_none());
        assert!(p.color.is_none());
    }

    #[test]
    fn test_expected_priorities_count() {
        assert_eq!(EXPECTED_PRIORITIES.len(), 4);
        assert_eq!(EXPECTED_PRIORITIES[0], "P1");
        assert_eq!(EXPECTED_PRIORITIES[3], "P4");
    }
}
