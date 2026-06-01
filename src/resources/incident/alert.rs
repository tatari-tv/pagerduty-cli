use crate::cli::IncidentAlertAction;
use crate::client::PdClient;
use crate::config::Config;
use crate::output::print_value;
use eyre::Result;
use tracing::instrument;

pub async fn handle(action: &IncidentAlertAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        IncidentAlertAction::List { incident_id } => list(client, config, incident_id).await,
        IncidentAlertAction::Get { incident_id, alert_id } => get(client, config, incident_id, alert_id).await,
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, incident_id: &str) -> Result<()> {
    let all = client
        .get_all(&format!("/incidents/{}/alerts", incident_id), "alerts")
        .await?;
    let result = serde_json::json!({ "alerts": all });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, incident_id: &str, alert_id: &str) -> Result<()> {
    let resp = client
        .get(&format!("/incidents/{}/alerts/{}", incident_id, alert_id))
        .await?;
    print_value(&resp, &config.output_format);
    Ok(())
}
