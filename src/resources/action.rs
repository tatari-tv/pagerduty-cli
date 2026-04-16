use crate::cli::ActionAction;
use crate::client::PdClient;
use crate::config::Config;
use crate::output::print_value;
use eyre::Result;
use tracing::instrument;

pub async fn handle(action: &ActionAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        ActionAction::List { query } => list(client, config, query.as_deref()).await,
        ActionAction::Get { id } => get(client, config, id).await,
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, query: Option<&str>) -> Result<()> {
    let path = match query {
        Some(q) => format!("/incident_workflows/actions?query={}", q),
        None => "/incident_workflows/actions".to_string(),
    };
    let resp = client.get(&path).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, id: &str) -> Result<()> {
    let resp = client.get(&format!("/incident_workflows/actions/{}", id)).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}
