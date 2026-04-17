//! `pd cache` command handler. Delegates to the `crate::cache` backend.
//! Kept in a separate module (not inlined into `lib.rs`) so the command-
//! surface stays colocated with the other resource handlers.

use crate::cache;
use crate::cli::CacheAction;
use crate::client::PdClient;
use crate::config::Config;
use eyre::Result;
use tracing::{debug, instrument};

#[instrument(skip(client, config))]
pub async fn handle(action: &CacheAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        CacheAction::Clear {
            resource_type,
            all_accounts,
        } => clear(client, config, resource_type.as_deref(), *all_accounts),
    }
}

fn clear(client: &PdClient, config: &Config, resource_type: Option<&str>, all_accounts: bool) -> Result<()> {
    debug!(resource_type = ?resource_type, all_accounts, subdomain = %config.subdomain, "cache clear");

    if all_accounts {
        cache::invalidate_all_accounts();
        return Ok(());
    }

    // A client without an attached cache means `--no-cache` is set or the
    // platform has no cache dir. Either way there's nothing to purge.
    let Some(c) = client.cache() else {
        debug!("no cache attached; clear is a no-op");
        return Ok(());
    };

    match resource_type {
        Some(t) => c.invalidate_type(t),
        None => c.invalidate_subdomain(),
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::cache::Cache;
    use crate::cli::OutputFormat;
    use tempfile::TempDir;

    fn make_config() -> Config {
        Config {
            api_token: "test-token".to_string(),
            from_email: None,
            subdomain: "tatari".to_string(),
            output_format: OutputFormat::Json,
            log_level: "warn".to_string(),
            routing_key: None,
        }
    }

    /// `pd cache clear` with no arg nukes the whole subdomain subtree, and
    /// leaves unrelated subdomains intact. This is the actual command
    /// surface, not just `--help`.
    #[tokio::test]
    async fn cache_clear_subdomain_removes_entries() {
        let tmp = TempDir::new().unwrap();
        let prod_root = tmp.path().join("ids").join("tatari");
        let staging_root = tmp.path().join("ids").join("tatari-staging");

        let prod_cache = Cache::with_root(prod_root.clone());
        let staging_cache = Cache::with_root(staging_root.clone());
        prod_cache.put("service", "Platform", "PPROD");
        prod_cache.put("team", "SRE", "TSRE");
        staging_cache.put("service", "Platform", "PSTAGING");
        assert_eq!(prod_cache.get("service", "Platform").as_deref(), Some("PPROD"));

        let client = PdClient::new("test-token".to_string()).unwrap().with_cache(prod_cache);
        let action = CacheAction::Clear {
            resource_type: None,
            all_accounts: false,
        };
        handle(&action, &client, &make_config()).await.unwrap();

        // Prod cache entries gone, staging still there.
        assert!(!prod_root.exists());
        assert_eq!(
            staging_cache.get("service", "Platform").as_deref(),
            Some("PSTAGING"),
            "clearing current subdomain must not touch other subdomains"
        );
    }

    /// `pd cache clear <type>` scopes to a single resource type and
    /// leaves the rest of the subdomain intact.
    #[tokio::test]
    async fn cache_clear_type_removes_only_that_type() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("ids").join("tatari");
        let cache = Cache::with_root(root.clone());
        cache.put("service", "Foo", "PFOO");
        cache.put("team", "Infra", "TINF");

        let client = PdClient::new("test-token".to_string())
            .unwrap()
            .with_cache(cache.clone());
        let action = CacheAction::Clear {
            resource_type: Some("service".to_string()),
            all_accounts: false,
        };
        handle(&action, &client, &make_config()).await.unwrap();

        assert_eq!(cache.get("service", "Foo"), None);
        assert_eq!(
            cache.get("team", "Infra").as_deref(),
            Some("TINF"),
            "clearing one type must not touch others"
        );
    }
}
