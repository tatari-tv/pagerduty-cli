use crate::cli::ActionAction;
use crate::client::PdClient;
use crate::config::Config;
use crate::output::print_value;
use eyre::Result;
use tracing::instrument;

pub async fn handle(action: &ActionAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        ActionAction::List => list(client, config).await,
        ActionAction::Get { id } => get(client, config, id).await,
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config) -> Result<()> {
    let all = client
        .get_all_no_offset("/incident_workflows/actions", "actions")
        .await?;
    let result = serde_json::json!({ "actions": all });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let resp = client.get(&format!("/incident_workflows/actions/{}", id)).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}
