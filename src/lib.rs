pub mod cache;
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
        Commands::Maintenance { action } => resources::maintenance::example_if_requested(action),
        Commands::AlertGrouping { action } => resources::grouping::example_if_requested(action),
        Commands::Change { action } => resources::change::example_if_requested(action),
        Commands::Incident { action } => match action {
            IncidentCommands::Workflow { action } => resources::incident::workflows::example_if_requested(action),
            other => resources::incident::crud::example_if_requested(other),
        },
        _ => None,
    }
}

#[instrument(skip_all, fields(command = ?cli.command))]
pub async fn run(cli: &Cli, config: &Config) -> Result<()> {
    let client = {
        let base = PdClient::new(config.api_token.clone())?;
        if cli.no_cache {
            base
        } else {
            match cache::Cache::new_for_subdomain(&config.subdomain) {
                Some(c) => base.with_cache(c),
                None => {
                    tracing::debug!("no platform cache dir available; name-to-ID cache disabled for this run");
                    base
                }
            }
        }
    };

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
            IncidentCommands::List {
                patterns,
                status,
                priority,
                team,
                since,
                until,
            } => {
                resources::incident::crud::list(
                    &client,
                    config,
                    patterns,
                    status,
                    priority,
                    team.as_deref(),
                    since.as_deref(),
                    until.as_deref(),
                )
                .await?;
            }
            IncidentCommands::Get { id } => {
                resources::incident::crud::get(&client, config, id).await?;
            }
            IncidentCommands::Create {
                title,
                service,
                priority,
                incident_type,
                body,
                from_email,
                from_file,
                example: _,
            } => {
                resources::incident::crud::create(
                    &client,
                    config,
                    title.as_deref(),
                    service.as_deref(),
                    priority.as_deref(),
                    incident_type.as_deref(),
                    body.as_deref(),
                    from_email.as_deref(),
                    from_file.as_deref(),
                )
                .await?;
            }
            IncidentCommands::Update {
                id,
                status,
                priority,
                title,
                from_email,
            } => {
                resources::incident::crud::update(
                    &client,
                    config,
                    id,
                    status.as_ref(),
                    priority.as_deref(),
                    title.as_deref(),
                    from_email.as_deref(),
                )
                .await?;
            }
            IncidentCommands::Note { action } => {
                resources::incident::note::handle(action, &client, config).await?;
            }
            IncidentCommands::Alert { action } => {
                resources::incident::alert::handle(action, &client, config).await?;
            }
            IncidentCommands::Trigger { action } => {
                resources::incident::trigger::handle(action, &client, config).await?;
            }
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
        Commands::Maintenance { action } => {
            resources::maintenance::handle(action, &client, config).await?;
        }
        Commands::AlertGrouping { action } => {
            resources::grouping::handle(action, &client, config).await?;
        }
        Commands::Orchestration { action } => {
            resources::orchestration::handle(action, &client, config).await?;
        }
        Commands::Log { action } => {
            resources::log::handle(action, &client, config).await?;
        }
        Commands::Change { action } => {
            resources::change::handle(action, &client, config).await?;
        }
        Commands::Cache { action } => {
            resources::cache::handle(action, &client, config).await?;
        }
    }

    Ok(())
}
