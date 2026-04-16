#![deny(clippy::unwrap_used)]
#![deny(dead_code)]
#![deny(unused_variables)]

use clap::Parser;
use eyre::{Context, Result};
use log::info;
use std::fs;
use std::path::PathBuf;

use pagerduty_cli::cli::Cli;
use pagerduty_cli::config::Config;

fn setup_logging(log_level: &str) -> Result<()> {
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pagerduty-cli")
        .join("logs");

    fs::create_dir_all(&log_dir).context("Failed to create log directory")?;

    let log_file = log_dir.join("pagerduty-cli.log");

    let target = Box::new(
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .context("Failed to open log file")?,
    );

    let level = log_level.parse::<log::LevelFilter>().unwrap_or(log::LevelFilter::Warn);

    env_logger::Builder::new()
        .filter_level(level)
        .target(env_logger::Target::Pipe(target))
        .init();

    info!("Logging initialized, writing to: {}", log_file.display());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load(&cli).context("Failed to load configuration")?;

    setup_logging(&config.log_level).context("Failed to setup logging")?;

    pagerduty_cli::run(&cli, &config).await.context("Command failed")?;

    Ok(())
}
