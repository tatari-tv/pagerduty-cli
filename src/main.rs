#![deny(clippy::unwrap_used)]
#![deny(dead_code)]
#![deny(unused_variables)]

use clap::Parser;
use eyre::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

use pagerduty_cli::cli::Cli;
use pagerduty_cli::config::{AuthDiagnostic, Config};

fn setup_tracing(log_level: &str) -> Result<()> {
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pagerduty-cli")
        .join("logs");

    fs::create_dir_all(&log_dir).context("Failed to create log directory")?;

    let log_file = log_dir.join("pagerduty-cli.log");

    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .context("Failed to open log file")?;

    let filter = EnvFilter::try_new(log_level).unwrap_or_else(|_| EnvFilter::new("warn"));

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(file)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(false)
                .with_file(true)
                .with_line_number(true),
        )
        .with(filter)
        .init();

    info!(log_path = %log_file.display(), "tracing initialized");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // --example requests print a YAML skeleton and exit. They never touch the
    // PagerDuty API, so bypass config/auth before Config::load demands a token.
    if let Some(skeleton) = pagerduty_cli::example_if_requested(&cli) {
        print!("{}", skeleton);
        return Ok(());
    }

    // `pd auth` helps a new user get a token configured. It must work on a
    // fresh install where no token is set, so we dispatch it before
    // `Config::load` would fail with the missing-token error.
    if pagerduty_cli::is_auth_command(&cli) {
        let diag = AuthDiagnostic::load(&cli).context("Failed to load auth diagnostic")?;
        return pagerduty_cli::run_auth(&cli, &diag);
    }

    let config = Config::load(&cli).context("Failed to load configuration")?;

    setup_tracing(&config.log_level).context("Failed to setup tracing")?;

    pagerduty_cli::run(&cli, &config)
        .await
        .context("Command failed")?;

    Ok(())
}
