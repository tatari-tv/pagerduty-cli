use crate::cli::IncidentNoteAction;
use crate::client::PdClient;
use crate::config::Config;
use crate::output::print_value;
use crate::resources::incident::crud::resolve_from_email;
use eyre::{Context, Result};
use serde_json::json;
use std::io::Read;
use tracing::instrument;

pub async fn handle(action: &IncidentNoteAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        IncidentNoteAction::List { incident_id } => list(client, config, incident_id).await,
        IncidentNoteAction::Create {
            incident_id,
            text,
            from_email,
        } => create(client, config, incident_id, text, from_email.as_deref()).await,
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, incident_id: &str) -> Result<()> {
    let resp = client.get(&format!("/incidents/{}/notes", incident_id)).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn create(
    client: &PdClient,
    config: &Config,
    incident_id: &str,
    text: &str,
    from_email_override: Option<&str>,
) -> Result<()> {
    let from = resolve_from_email(config, from_email_override)?;
    let content = if text == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("Failed to read note text from stdin")?;
        buf
    } else {
        text.to_string()
    };
    let body = json!({ "note": { "content": content } });
    let result = client
        .post_with_from(&format!("/incidents/{}/notes", incident_id), body, &from)
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}
