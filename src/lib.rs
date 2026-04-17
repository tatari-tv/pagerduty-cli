pub mod cli;
pub mod client;
pub mod config;
pub mod filter;
pub mod output;
pub mod resources;

use cli::{Cli, Commands, IncidentCommands};
use client::PdClient;
use config::Config;
use eyre::Result;
use serde_json::Value;
use tracing::instrument;

/// Returns the YAML skeleton to print if the invoked command is a
/// `--example` request that should bypass API/auth setup, `None` otherwise.
pub fn example_if_requested(cli: &Cli) -> Option<&'static str> {
    match &cli.command {
        Commands::Team { action } => resources::team::example_if_requested(action),
        Commands::Schedule { action } => resources::schedule::example_if_requested(action),
        Commands::Escalation { action } => resources::escalation::example_if_requested(action),
        Commands::Service { action } => resources::service::example_if_requested(action),
        Commands::Incident {
            action: IncidentCommands::Workflow { action },
        } => resources::incident::workflows::example_if_requested(action),
        _ => None,
    }
}

#[instrument(skip_all, fields(command = ?cli.command))]
pub async fn run(cli: &Cli, config: &Config) -> Result<()> {
    let client = PdClient::new(config.api_token.clone())?;

    match &cli.command {
        Commands::Rest { method, path, body } => {
            let body_value = body
                .as_deref()
                .map(serde_json::from_str::<Value>)
                .transpose()
                .map_err(|e| eyre::eyre!("Invalid JSON body: {}", e))?;

            let result = client.raw(method, path, body_value).await?;
            output::print_value(&result, &config.output_format);
        }
        Commands::Incident { action } => match action {
            IncidentCommands::Type { action } => {
                resources::incident::types::handle(action, &client, config).await?;
            }
            IncidentCommands::Workflow { action } => {
                resources::incident::workflows::handle(action, &client, config).await?;
            }
        },
        Commands::Priority { action } => {
            resources::priority::handle(action, &client, config).await?;
        }
        Commands::Trigger { action } => {
            resources::trigger::handle(action, &client, config).await?;
        }
        Commands::Action { action } => {
            resources::action::handle(action, &client, config).await?;
        }
        Commands::User { action } => {
            resources::user::handle(action, &client, config).await?;
        }
        Commands::Team { action } => {
            resources::team::handle(action, &client, config).await?;
        }
        Commands::Schedule { action } => {
            resources::schedule::handle(action, &client, config).await?;
        }
        Commands::Escalation { action } => {
            resources::escalation::handle(action, &client, config).await?;
        }
        Commands::Service { action } => {
            resources::service::handle(action, &client, config).await?;
        }
        Commands::Oncall { action } => {
            resources::oncall::handle(action, &client, config).await?;
        }
    }

    Ok(())
}
