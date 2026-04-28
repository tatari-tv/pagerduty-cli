use crate::cli::ActionAction;
use crate::client::PdClient;
use crate::config::Config;
use crate::output::print_value;
use eyre::Result;
use tracing::instrument;

pub async fn handle(action: &ActionAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        ActionAction::List { patterns } => list(client, config, patterns).await,
        ActionAction::Get { id } => get(client, config, id).await,
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, patterns: &[String]) -> Result<()> {
    let all = client
        .get_all_no_offset("/incident_workflows/actions", "actions")
        .await?;
    // Match on function_name (the table's primary display column). The action
    // catalog is ~335KB; local filtering is fine since the endpoint has no
    // supported query parameter (see v0.2.1 shakedown).
    let filtered = crate::filter::filter_into(all, patterns, |v| {
        v.get("function_name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
    });
    let result = serde_json::json!({ "actions": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let resp = client
        .get(&format!("/incident_workflows/actions/{}", id))
        .await?;
    print_value(&resp, &config.output_format);
    Ok(())
}
