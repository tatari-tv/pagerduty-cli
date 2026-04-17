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
