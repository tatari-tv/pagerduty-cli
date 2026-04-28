use crate::cli::{Cli, OutputFormat};
use eyre::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default, rename_all = "kebab-case")]
pub(crate) struct ConfigFile {
    pub(crate) api_token: Option<String>,
    pub(crate) from_email: Option<String>,
    pub(crate) subdomain: Option<String>,
    pub(crate) output_format: Option<String>,
    pub(crate) log_level: Option<String>,
    pub(crate) routing_key: Option<String>,
}

#[derive(Debug)]
pub struct Config {
    pub api_token: String,
    pub from_email: Option<String>,
    pub subdomain: Option<String>,
    pub output_format: OutputFormat,
    pub log_level: String,
    /// Escape-hatch Events API v2 routing key. When set at any layer
    /// (`--routing-key` CLI flag, `PAGERDUTY_ROUTING_KEY` env, or
    /// `routing-key` config field), `pd change create` skips the
    /// dynamic service-to-integration-key lookup and sends the event
    /// with this key. Default: None (dynamic lookup runs).
    pub routing_key: Option<String>,
}

/// Where an API token was (or wasn't) resolved from. Used by `pd auth status`
/// and to explain config-load failures without leaking token values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSource {
    CliFlag,
    EnvVar,
    ConfigFile(PathBuf),
    NotFound,
}

/// Diagnostic view of the resolved config for auth-related commands. Does
/// not require an API token to construct, so `pd auth status` works on a
/// fresh install.
#[derive(Debug)]
pub struct AuthDiagnostic {
    pub subdomain: Option<String>,
    pub token_source: TokenSource,
    pub config_file_path: Option<PathBuf>,
}

impl AuthDiagnostic {
    pub fn load(cli: &Cli) -> Result<Self> {
        let (file, path) = load_config_file_with_path(cli.config.as_ref())?;
        let subdomain = file.subdomain.clone();

        let token_source = if cli.api_token.is_some() {
            TokenSource::CliFlag
        } else if std::env::var("PAGERDUTY_API_TOKEN").is_ok() {
            TokenSource::EnvVar
        } else if file.api_token.is_some() {
            // Token resolved from the loaded config file. Path is only
            // reported when we actually read a file (vs. using defaults).
            match &path {
                Some(p) => TokenSource::ConfigFile(p.clone()),
                None => TokenSource::NotFound,
            }
        } else {
            TokenSource::NotFound
        };

        Ok(Self {
            subdomain,
            token_source,
            config_file_path: path,
        })
    }
}

impl Config {
    pub fn load(cli: &Cli) -> Result<Self> {
        let (file, _) = load_config_file_with_path(cli.config.as_ref())?;

        let subdomain = file.subdomain.clone();

        // Resolution order: CLI flag > env var > config file
        let api_token = cli
            .api_token
            .clone()
            .or_else(|| std::env::var("PAGERDUTY_API_TOKEN").ok())
            .or(file.api_token)
            .ok_or_else(|| eyre::eyre!("{}", no_token_error_message()))?;

        let from_email = std::env::var("PAGERDUTY_FROM_EMAIL")
            .ok()
            .or(file.from_email);

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

        let routing_key = std::env::var("PAGERDUTY_ROUTING_KEY")
            .ok()
            .or(file.routing_key);

        Ok(Self {
            api_token,
            from_email,
            subdomain,
            output_format,
            log_level,
            routing_key,
        })
    }
}

/// Rendered error shown when no API token can be resolved. Policy-neutral:
/// does NOT link to the PagerDuty API keys page or walk the user through
/// personal key creation. Per team decision (proj-pagerduty 2026-04-17), PD
/// tokens are managed via Terraform + AWS Secrets Manager, not ad-hoc by
/// individual users; this message just explains how to wire up a token
/// once the user has one.
pub fn no_token_error_message() -> String {
    "No PagerDuty API token found.\n\
     \n\
     To configure a token:\n\
     \x20\x20   export PAGERDUTY_API_TOKEN=<your-token>\n\
     \n\
     or add to ~/.config/pagerduty-cli/pagerduty-cli.yml:\n\
     \x20\x20   api-token: <your-token>\n\
     \n\
     Run `pd auth status` to verify detection."
        .to_string()
}

fn xdg_config_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        let path = PathBuf::from(dir);
        if path.is_absolute() {
            return Some(path);
        }
    }
    dirs::home_dir().map(|h| h.join(".config"))
}

pub(crate) fn load_config_file_with_path(
    path: Option<&PathBuf>,
) -> Result<(ConfigFile, Option<PathBuf>)> {
    if let Some(p) = path {
        let content = fs::read_to_string(p)
            .with_context(|| format!("Failed to read config file: {}", p.display()))?;
        let file: ConfigFile = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", p.display()))?;
        return Ok((file, Some(p.clone())));
    }

    if let Some(config_dir) = xdg_config_dir() {
        let project_config = config_dir.join("pagerduty-cli").join("pagerduty-cli.yml");
        if project_config.exists() {
            let content = fs::read_to_string(&project_config)
                .with_context(|| format!("Failed to read {}", project_config.display()))?;
            let file: ConfigFile = serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse {}", project_config.display()))?;
            return Ok((file, Some(project_config)));
        }
    }

    Ok((ConfigFile::default(), None))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(unused_variables)]
mod tests {
    use super::*;
    use crate::cli::Commands;
    use std::sync::Mutex;
    use tempfile::TempDir;

    fn isolate_xdg_config() -> TempDir {
        let dir = TempDir::new().unwrap();
        unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()) };
        dir
    }

    // Serialize all env-var-touching tests to prevent parallel races
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn make_cli(api_token: Option<&str>) -> Cli {
        Cli {
            config: None,
            api_token: api_token.map(|s| s.to_string()),
            output: None,
            log_level: None,
            no_cache: false,
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
        let _xdg = isolate_xdg_config();
        let result = Config::load(&cli);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_missing_token_error_is_policy_neutral() {
        // Regression: per proj-pagerduty discussion 2026-04-17, the error
        // must NOT link to the PagerDuty API keys page or walk users
        // through ad-hoc personal key creation. PD tokens are managed via
        // Terraform + AWS Secrets Manager. The error should only explain
        // how to wire a token up once the user has one.
        let guard = ENV_LOCK.lock().unwrap();
        let cli = make_cli(None);
        // SAFETY: serialized by ENV_LOCK; no concurrent env mutation
        unsafe { std::env::remove_var("PAGERDUTY_API_TOKEN") };
        let _xdg = isolate_xdg_config();
        let err = Config::load(&cli).expect_err("expected missing-token error");
        let msg = format!("{}", err);
        assert!(
            msg.contains("PAGERDUTY_API_TOKEN"),
            "error message lacks env var: {}",
            msg
        );
        assert!(
            msg.contains("api-token:"),
            "error message lacks config example: {}",
            msg
        );
        assert!(
            msg.contains("pd auth status"),
            "error message lacks pointer to pd auth status: {}",
            msg
        );
        assert!(
            !msg.contains("pagerduty.com/api_keys"),
            "error message should not link to API keys page: {}",
            msg
        );
    }

    #[test]
    fn test_config_defaults() {
        let guard = ENV_LOCK.lock().unwrap();
        let cli = make_cli(Some("token"));
        // SAFETY: serialized by ENV_LOCK; no concurrent env mutation
        unsafe { std::env::remove_var("PAGERDUTY_API_TOKEN") };
        let config = Config::load(&cli).unwrap();
        assert_eq!(config.subdomain, None);
        assert_eq!(config.log_level, "warn");
    }

    #[test]
    fn test_auth_diagnostic_detects_cli_flag() {
        let guard = ENV_LOCK.lock().unwrap();
        let cli = make_cli(Some("cli-token"));
        // SAFETY: serialized by ENV_LOCK; no concurrent env mutation
        unsafe { std::env::remove_var("PAGERDUTY_API_TOKEN") };
        let diag = AuthDiagnostic::load(&cli).unwrap();
        assert_eq!(diag.token_source, TokenSource::CliFlag);
        assert_eq!(diag.subdomain, None);
    }

    #[test]
    fn test_auth_diagnostic_detects_env_var() {
        let guard = ENV_LOCK.lock().unwrap();
        let cli = make_cli(None);
        // SAFETY: serialized by ENV_LOCK; no concurrent env mutation
        unsafe { std::env::set_var("PAGERDUTY_API_TOKEN", "env-token") };
        let diag = AuthDiagnostic::load(&cli).unwrap();
        assert_eq!(diag.token_source, TokenSource::EnvVar);
        unsafe { std::env::remove_var("PAGERDUTY_API_TOKEN") };
    }

    #[test]
    fn test_auth_diagnostic_detects_not_found() {
        let guard = ENV_LOCK.lock().unwrap();
        let cli = make_cli(None);
        // SAFETY: serialized by ENV_LOCK; no concurrent env mutation
        unsafe { std::env::remove_var("PAGERDUTY_API_TOKEN") };
        let _xdg = isolate_xdg_config();
        let diag = AuthDiagnostic::load(&cli).unwrap();
        assert_eq!(diag.token_source, TokenSource::NotFound);
    }

    /// The sample `pagerduty-cli.yml` in the repo root is what users copy to
    /// `~/.config/pagerduty-cli/pagerduty-cli.yml`. It must parse cleanly and
    /// resolve to the documented defaults. This test guards against the
    /// previous bug where the sample contained fake fields (`name`, `age`,
    /// `debug`) that the code never read.
    #[test]
    fn test_sample_config_file_parses() {
        let guard = ENV_LOCK.lock().unwrap();
        let sample_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("pagerduty-cli.yml");
        let cli = Cli {
            config: Some(sample_path),
            api_token: None,
            output: None,
            log_level: None,
            no_cache: false,
            command: Commands::Rest {
                method: "GET".to_string(),
                path: "/test".to_string(),
                body: None,
            },
        };
        // SAFETY: serialized by ENV_LOCK; no concurrent env mutation.
        // Sample leaves api-token commented out, so the env var must satisfy it.
        unsafe { std::env::set_var("PAGERDUTY_API_TOKEN", "sample-test-token") };
        let config = Config::load(&cli).expect("sample config must load");
        assert_eq!(config.api_token, "sample-test-token");
        assert_eq!(config.subdomain, None);
        assert_eq!(config.log_level, "warn");
        assert!(matches!(config.output_format, OutputFormat::Auto));
        unsafe { std::env::remove_var("PAGERDUTY_API_TOKEN") };
    }
}
