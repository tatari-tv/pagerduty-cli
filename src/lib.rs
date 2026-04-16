pub mod cli;
pub mod client;
pub mod config;
pub mod output;

use cli::{Cli, Commands};
use client::PdClient;
use config::Config;
use eyre::Result;
use log::debug;
use serde_json::Value;

pub async fn run(cli: &Cli, config: &Config) -> Result<()> {
    debug!("run: command={:?}", cli.command);

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
    }

    Ok(())
}
