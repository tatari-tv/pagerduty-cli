use crate::cli::{Cli, OutputFormat};
use eyre::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Default)]
#[serde(default, rename_all = "kebab-case")]
struct ConfigFile {
    api_token: Option<String>,
    subdomain: Option<String>,
    output_format: Option<String>,
    log_level: Option<String>,
}

#[derive(Debug)]
pub struct Config {
    pub api_token: String,
    pub subdomain: String,
    pub output_format: OutputFormat,
    pub log_level: String,
}

impl Config {
    pub fn load(cli: &Cli) -> Result<Self> {
        let file = load_config_file(cli.config.as_ref())?;

        // Resolution order: CLI flag > env var > config file
        let api_token = cli
            .api_token
            .clone()
            .or_else(|| std::env::var("PAGERDUTY_API_TOKEN").ok())
            .or(file.api_token)
            .ok_or_else(|| {
                eyre::eyre!(
                    "No API token found. Set PAGERDUTY_API_TOKEN, use --api-token, or add api-token to ~/.config/pagerduty-cli/pagerduty-cli.yml"
                )
            })?;

        let subdomain = file.subdomain.unwrap_or_else(|| "tatari".to_string());

        let output_format = cli.output.clone().unwrap_or({
            match file.output_format.as_deref() {
                Some("json") => OutputFormat::Json,
                Some("table") => OutputFormat::Table,
                _ => OutputFormat::Auto,
            }
        });

        let log_level = cli
            .log_level
            .clone()
            .or(file.log_level)
            .unwrap_or_else(|| "warn".to_string());

        Ok(Self {
            api_token,
            subdomain,
            output_format,
            log_level,
        })
    }
}

fn load_config_file(path: Option<&PathBuf>) -> Result<ConfigFile> {
    if let Some(p) = path {
        let content = fs::read_to_string(p).with_context(|| format!("Failed to read config file: {}", p.display()))?;
        return serde_yaml::from_str(&content).with_context(|| format!("Failed to parse config file: {}", p.display()));
    }

    if let Some(config_dir) = dirs::config_dir() {
        let project_config = config_dir.join("pagerduty-cli").join("pagerduty-cli.yml");
        if project_config.exists() {
            let content = fs::read_to_string(&project_config)
                .with_context(|| format!("Failed to read {}", project_config.display()))?;
            return serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse {}", project_config.display()));
        }
    }

    Ok(ConfigFile::default())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(unused_variables)]
mod tests {
    use super::*;
    use crate::cli::Commands;
    use std::sync::Mutex;

    // Serialize all env-var-touching tests to prevent parallel races
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn make_cli(api_token: Option<&str>) -> Cli {
        Cli {
            config: None,
            api_token: api_token.map(|s| s.to_string()),
            output: None,
            log_level: None,
            command: Commands::Rest {
                method: "GET".to_string(),
                path: "/test".to_string(),
                body: None,
            },
        }
    }

    #[test]
    fn test_config_from_cli_token() {
        let guard = ENV_LOCK.lock().unwrap();
        let cli = make_cli(Some("cli-token"));
        // SAFETY: serialized by ENV_LOCK; no concurrent env mutation
        unsafe { std::env::remove_var("PAGERDUTY_API_TOKEN") };
        let config = Config::load(&cli).unwrap();
        assert_eq!(config.api_token, "cli-token");
    }

    #[test]
    fn test_config_from_env_token() {
        let guard = ENV_LOCK.lock().unwrap();
        let cli = make_cli(None);
        // SAFETY: serialized by ENV_LOCK; no concurrent env mutation
        unsafe { std::env::set_var("PAGERDUTY_API_TOKEN", "env-token") };
        let config = Config::load(&cli).unwrap();
        assert_eq!(config.api_token, "env-token");
        unsafe { std::env::remove_var("PAGERDUTY_API_TOKEN") };
    }

    #[test]
    fn test_config_missing_token_errors() {
        let guard = ENV_LOCK.lock().unwrap();
        let cli = make_cli(None);
        // SAFETY: serialized by ENV_LOCK; no concurrent env mutation
        unsafe { std::env::remove_var("PAGERDUTY_API_TOKEN") };
        let result = Config::load(&cli);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_defaults() {
        let guard = ENV_LOCK.lock().unwrap();
        let cli = make_cli(Some("token"));
        // SAFETY: serialized by ENV_LOCK; no concurrent env mutation
        unsafe { std::env::remove_var("PAGERDUTY_API_TOKEN") };
        let config = Config::load(&cli).unwrap();
        assert_eq!(config.subdomain, "tatari");
        assert_eq!(config.log_level, "warn");
    }
}
