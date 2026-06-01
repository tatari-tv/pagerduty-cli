//! `pd auth` - onboarding helpers that run without requiring a valid token.
//!
//! The dispatcher in `main.rs` detects `Commands::Auth` before calling
//! `Config::load` and hands an `AuthDiagnostic` to this handler instead.
//!
//! Scope note: `pd auth status` intentionally does NOT point at the
//! PagerDuty API keys page. Per team decision (proj-pagerduty 2026-04-17),
//! API tokens will be managed via Terraform and stored in AWS Secrets
//! Manager rather than encouraging ad-hoc personal key creation.

use crate::cli::AuthAction;
use crate::config::{AuthDiagnostic, TokenSource};
use eyre::Result;

pub fn handle(action: &AuthAction, diag: &AuthDiagnostic) -> Result<()> {
    match action {
        AuthAction::Status => status(diag),
    }
}

fn status(diag: &AuthDiagnostic) -> Result<()> {
    let source_line = match &diag.token_source {
        TokenSource::CliFlag => "source:    --api-token CLI flag".to_string(),
        TokenSource::EnvVar => "source:    PAGERDUTY_API_TOKEN env var".to_string(),
        TokenSource::ConfigFile(p) => format!("source:    config file ({})", p.display()),
        TokenSource::NotFound => "source:    (none found)".to_string(),
    };

    let token_found = !matches!(diag.token_source, TokenSource::NotFound);

    println!("token:     {}", if token_found { "found" } else { "not found" });
    println!("{}", source_line);
    println!("subdomain: {}", diag.subdomain.as_deref().unwrap_or("(not configured)"));

    if !token_found {
        println!();
        println!("To configure a token:");
        println!("  export PAGERDUTY_API_TOKEN=<your-token>");
        println!("or add to ~/.config/pagerduty-cli/pagerduty-cli.yml:");
        println!("  api-token: <your-token>");
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn diag(source: TokenSource, subdomain: Option<&str>) -> AuthDiagnostic {
        AuthDiagnostic {
            subdomain: subdomain.map(str::to_string),
            token_source: source,
            config_file_path: None,
        }
    }

    #[test]
    fn status_runs_with_not_found_source() {
        // Regression: `pd auth status` must work on a fresh install where
        // no API token is configured.
        let d = diag(TokenSource::NotFound, None);
        assert!(status(&d).is_ok());
    }

    #[test]
    fn status_runs_with_env_var_source() {
        let d = diag(TokenSource::EnvVar, None);
        assert!(status(&d).is_ok());
    }

    #[test]
    fn status_runs_with_cli_flag_source() {
        let d = diag(TokenSource::CliFlag, None);
        assert!(status(&d).is_ok());
    }

    #[test]
    fn status_runs_with_config_file_source() {
        let d = AuthDiagnostic {
            subdomain: Some("example".to_string()),
            token_source: TokenSource::ConfigFile(PathBuf::from("/tmp/test.yml")),
            config_file_path: Some(PathBuf::from("/tmp/test.yml")),
        };
        assert!(status(&d).is_ok());
    }
}
