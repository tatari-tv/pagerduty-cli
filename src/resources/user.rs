use crate::cli::UserAction;
use crate::client::PdClient;
use crate::config::Config;
use crate::filter;
use crate::output::print_value;
use eyre::Result;
use serde_json::{Value, json};
use tracing::{debug, instrument};

pub async fn handle(action: &UserAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        UserAction::List { patterns } => list(client, config, patterns).await,
        UserAction::Get { email_or_id } => get(client, config, email_or_id).await,
    }
}

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, patterns: &[String]) -> Result<()> {
    debug!(patterns_len = patterns.len(), "user list");

    // Single-pattern case can use the API `query` parameter to reduce payload.
    // Multi-pattern still needs a full fetch because PD's query param is a single string.
    let all = if patterns.len() == 1 {
        let q = crate::client::encode_query(&patterns[0]);
        client.get_all(&format!("/users?query={}", q), "users").await?
    } else {
        client.get_all("/users", "users").await?
    };

    let filtered = filter::filter_into(all, patterns, user_name);
    let result = json!({ "users": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, email_or_id: &str) -> Result<()> {
    // A PagerDuty user ID is an opaque short string; treat anything containing '@'
    // as an email to scan-match, and otherwise try a direct GET first with fallback.
    if email_or_id.contains('@') {
        let resolved = resolve_by_email(client, email_or_id).await?;
        print_value(&json!({ "user": resolved }), &config.output_format);
        return Ok(());
    }

    if let Some(resp) = client.try_get(&format!("/users/{}", email_or_id)).await? {
        print_value(&resp, &config.output_format);
        return Ok(());
    }

    // Not a PD ID; fall back to email scan so users can pass a local-part or name.
    let resolved = resolve_by_email(client, email_or_id).await?;
    print_value(&json!({ "user": resolved }), &config.output_format);
    Ok(())
}

async fn resolve_by_email(client: &PdClient, email: &str) -> Result<Value> {
    let q = crate::client::encode_query(email);
    let all = client.get_all(&format!("/users?query={}", q), "users").await?;
    let matches: Vec<&Value> = all
        .iter()
        .filter(|u| {
            u.get("email")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .eq_ignore_ascii_case(email)
        })
        .collect();
    match matches.as_slice() {
        [single] => Ok((*single).clone()),
        [] => eyre::bail!("No user found matching {:?}", email),
        many => {
            let ids: Vec<&str> = many
                .iter()
                .filter_map(|u| u.get("id").and_then(|v| v.as_str()))
                .collect();
            eyre::bail!(
                "Multiple users match {:?}: {}. Use the PagerDuty ID to disambiguate.",
                email,
                ids.join(", ")
            )
        }
    }
}

fn user_name(value: &Value) -> &str {
    value.get("name").and_then(|v| v.as_str()).unwrap_or("")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn user_name_extracts_name_field() {
        let v = json!({"name": "Scott Idler", "email": "scott.idler@tatari.tv"});
        assert_eq!(user_name(&v), "Scott Idler");
    }

    #[test]
    fn user_name_returns_empty_when_missing() {
        let v = json!({"email": "scott.idler@tatari.tv"});
        assert_eq!(user_name(&v), "");
    }
}
