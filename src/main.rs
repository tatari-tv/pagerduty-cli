#![deny(clippy::unwrap_used)]
#![deny(dead_code)]
#![deny(unused_variables)]

use clap::Parser;
use colored::*;
use eyre::{Context, Result};
use log::info;
use std::fs;
use std::path::PathBuf;

mod cli;
mod config;

use cli::Cli;
use config::Config;

fn setup_logging() -> Result<()> {
    // Create log directory
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pagerduty-cli")
        .join("logs");

    fs::create_dir_all(&log_dir).context("Failed to create log directory")?;

    let log_file = log_dir.join("pagerduty-cli.log");

    // Setup env_logger with file output
    let target = Box::new(
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .context("Failed to open log file")?,
    );

    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(target))
        .init();

    info!("Logging initialized, writing to: {}", log_file.display());
    Ok(())
}

fn run_application(cli: &Cli, config: &Config) -> Result<()> {
    info!("Starting application");

    // Load and display configuration
    println!("{}", "✓ Configuration loaded successfully".green());
    if cli.verbose {
        println!("{}", "🔍 Verbose mode enabled".yellow());
    }
    if config.debug {
        println!("{}", "🔍 Debug mode enabled".yellow());
    }

    // Demonstrate colored output
    println!("{} Hello from {}!", "🎉".green(), "pagerduty-cli".cyan());
    println!("{} Author: {}", "👤".blue(), config.name);
    println!("{} Age: {}", "📅".blue(), config.age);

    // Log some information
    info!("Application executed successfully");

    Ok(())
}

fn main() -> Result<()> {
    // Setup logging first
    setup_logging().context("Failed to setup logging")?;

    // Parse CLI arguments
    let cli = Cli::parse();

    // Load configuration
    let config = Config::load(cli.config.as_ref()).context("Failed to load configuration")?;

    info!("Starting with config from: {:?}", cli.config);

    // Run the main application logic
    run_application(&cli, &config).context("Application failed")?;

    Ok(())
}
