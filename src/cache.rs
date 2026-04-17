//! Name-to-ID cache for PagerDuty resource lookups.
//!
//! Amortizes `resolve_*_id` latency for tight scripted loops like
//! `for s in $(...); do pd service get "$s"; done`. Without the cache, every
//! iteration re-issues `try_get(/type/name)` + `get_all(/type?query=name)`.
//! With the cache, the first hit stores `(name -> id)` on disk, and
//! subsequent hits are file reads.
//!
//! Layout (one file per entry):
//!   `~/.cache/pd/ids/<subdomain>/<type>/<sha256(name)>.json`
//!
//! - Subdomain namespacing prevents staging/prod cross-contamination on the
//!   same laptop. A `tatari-staging` invocation and a `tatari` invocation
//!   never share IDs.
//! - Full `sha256(name)` in the filename eliminates the hash-collision
//!   failure mode (the cost is 64 extra filename bytes per entry, which is
//!   negligible on any modern filesystem).
//! - One file per entry eliminates the read-modify-rewrite race: two
//!   processes caching different names never touch the same file; two
//!   processes caching the same name produce the same content.
//! - Atomic writes: write to `<file>.tmp`, `sync_all`, rename.
//! - No negative caching. Failed lookups are not persisted. See the
//!   "Known Cache Limitations" block in `read_cached` below for the
//!   rationale.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

const CACHE_VERSION: u32 = 1;
const TTL_SECS: u64 = 300;

/// One cache entry, one file. The `name` field is stored explicitly so a
/// hand-edit or a theoretical hash collision is detectable at read time.
#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    version: u32,
    name: String,
    id: String,
    ts: u64,
}

/// Cache backend. Construct once via `Cache::new_for_subdomain` and pass it
/// into `PdClient::new_with_cache`; resolvers use it through the client.
#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    /// Create a cache rooted at `<cache_dir>/pd/ids/<subdomain>/`.
    /// Returns `None` if `dirs::cache_dir()` is unavailable (non-standard
    /// platform with no XDG cache home). Callers must treat `None` as
    /// "cache disabled" and fall through to the API.
    pub fn new_for_subdomain(subdomain: &str) -> Option<Self> {
        let base = dirs::cache_dir()?;
        let root = base.join("pd").join("ids").join(subdomain);
        Some(Self { root })
    }

    /// Entry path for a given (type, name) pair.
    fn path_for(&self, resource_type: &str, name: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        let hash = hex(hasher.finalize().as_slice());
        self.root.join(resource_type).join(format!("{}.json", hash))
    }

    /// Return the cached ID for `(resource_type, name)` or `None` on miss,
    /// expiry, parse failure, or name-mismatch.
    ///
    /// =======================================================================
    /// KNOWN LIMITATION: Rename-without-delete staleness
    /// =======================================================================
    ///
    /// When a PagerDuty resource is RENAMED via the UI (not deleted, not
    /// created), its ID stays stable but its name changes. The cache has no
    /// way to observe this rename through normal lifecycle hooks
    /// (create / update / delete on a given `pd` invocation) because the
    /// rename happened out-of-band.
    ///
    /// The failure mode: cache says "Web" -> "P0001". A user renames "Web"
    /// to "Web-Legacy" in the UI. The next `pd service get Web` returns the
    /// cached "P0001" without hitting the API. Because P0001 still exists
    /// as a valid service (now named "Web-Legacy"), any subsequent
    /// PUT/DELETE lands on the wrong service conceptually.
    ///
    /// Why we accept this:
    ///   1. The 5-minute TTL bounds staleness to a 5-minute window.
    ///   2. Verifying the rename would require a GET against the cached ID
    ///      on every cache hit, which defeats the purpose of the cache.
    ///   3. UI renames are rare compared to reads.
    ///
    /// If you suspect stale data, run `pd cache clear <type>` or pass
    /// `--no-cache` to bypass the cache for a single invocation.
    ///
    /// Related: we deliberately do NOT cache negative results. A cached
    /// "not found" with a 5-minute TTL would poison CI pipelines where an
    /// external system (Terraform, another `pd` session, the PD UI)
    /// creates the resource between two `pd` invocations. Do not add
    /// negative caching without revisiting this decision.
    ///
    /// =======================================================================
    pub fn get(&self, resource_type: &str, name: &str) -> Option<String> {
        let path = self.path_for(resource_type, name);
        let bytes = fs::read(&path).ok()?;
        let entry: Entry = serde_json::from_slice(&bytes).ok()?;

        // Hash collision defense AND version bump protection: a stored entry
        // whose `version` doesn't match OR whose `name` doesn't match the
        // requested name is treated as a miss. The former catches schema
        // changes; the latter catches (implausible but theoretically
        // possible) sha256 collisions and hand-edits.
        if entry.version != CACHE_VERSION || entry.name != name {
            debug!(path = %path.display(), "cache entry version/name mismatch, ignoring");
            return None;
        }

        let age = now_secs().saturating_sub(entry.ts);
        if age > TTL_SECS {
            debug!(path = %path.display(), age, "cache entry expired");
            return None;
        }

        debug!(path = %path.display(), age, resource_type, name, id = %entry.id, "cache hit");
        Some(entry.id)
    }

    /// Store `(resource_type, name) -> id`. Atomic: writes to `<file>.tmp`
    /// and renames into place. Errors are logged and swallowed; a failed
    /// cache write must not bubble up and fail the user's command.
    pub fn put(&self, resource_type: &str, name: &str, id: &str) {
        let path = self.path_for(resource_type, name);
        if let Some(parent) = path.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            warn!(error = %e, path = %parent.display(), "cache dir create failed");
            return;
        }

        let entry = Entry {
            version: CACHE_VERSION,
            name: name.to_string(),
            id: id.to_string(),
            ts: now_secs(),
        };

        let bytes = match serde_json::to_vec(&entry) {
            Ok(b) => b,
            Err(e) => {
                warn!(error = %e, "cache entry serialization failed");
                return;
            }
        };

        let tmp = path.with_extension("json.tmp");
        match fs::File::create(&tmp) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(&bytes).and_then(|_| f.sync_all()) {
                    warn!(error = %e, path = %tmp.display(), "cache tmp write failed");
                    let _ = fs::remove_file(&tmp);
                    return;
                }
            }
            Err(e) => {
                warn!(error = %e, path = %tmp.display(), "cache tmp create failed");
                return;
            }
        }

        if let Err(e) = fs::rename(&tmp, &path) {
            warn!(error = %e, from = %tmp.display(), to = %path.display(), "cache rename failed");
            let _ = fs::remove_file(&tmp);
        }
    }

    /// Delete one entry. Errors are logged and swallowed.
    pub fn invalidate_entry(&self, resource_type: &str, name: &str) {
        let path = self.path_for(resource_type, name);
        match fs::remove_file(&path) {
            Ok(()) => debug!(path = %path.display(), "cache entry invalidated"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => warn!(error = %e, path = %path.display(), "cache invalidate_entry failed"),
        }
    }

    /// Delete the whole resource-type subtree for this subdomain.
    pub fn invalidate_type(&self, resource_type: &str) {
        let path = self.root.join(resource_type);
        match fs::remove_dir_all(&path) {
            Ok(()) => debug!(path = %path.display(), "cache type subtree invalidated"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => warn!(error = %e, path = %path.display(), "cache invalidate_type failed"),
        }
    }

    /// Delete this subdomain's whole cache subtree.
    pub fn invalidate_subdomain(&self) {
        match fs::remove_dir_all(&self.root) {
            Ok(()) => debug!(path = %self.root.display(), "cache subdomain subtree invalidated"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => warn!(error = %e, path = %self.root.display(), "cache invalidate_subdomain failed"),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_root(root: PathBuf) -> Self {
        Self { root }
    }
}

/// Delete every subdomain's cache subtree (used by `pd cache clear
/// --all-accounts`). Best-effort; errors are logged and swallowed.
pub fn invalidate_all_accounts() {
    let Some(base) = dirs::cache_dir() else {
        return;
    };
    let root = base.join("pd").join("ids");
    match fs::remove_dir_all(&root) {
        Ok(()) => debug!(path = %root.display(), "all-accounts cache invalidated"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => warn!(error = %e, path = %root.display(), "cache invalidate_all_accounts failed"),
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn cache_in(dir: &TempDir) -> Cache {
        Cache::with_root(dir.path().join("ids").join("tatari"))
    }

    #[test]
    fn put_then_get_returns_id() {
        let tmp = TempDir::new().unwrap();
        let c = cache_in(&tmp);
        c.put("service", "Platform API", "PSVC1");
        assert_eq!(c.get("service", "Platform API").as_deref(), Some("PSVC1"));
    }

    #[test]
    fn get_miss_returns_none() {
        let tmp = TempDir::new().unwrap();
        let c = cache_in(&tmp);
        assert_eq!(c.get("service", "DoesNotExist"), None);
    }

    #[test]
    fn get_returns_none_on_expired_entry() {
        let tmp = TempDir::new().unwrap();
        let c = cache_in(&tmp);
        // Write an entry with an old ts directly, then read.
        let path = c.path_for("service", "Stale");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let entry = Entry {
            version: CACHE_VERSION,
            name: "Stale".to_string(),
            id: "PSTALE".to_string(),
            ts: now_secs().saturating_sub(TTL_SECS + 1),
        };
        fs::write(&path, serde_json::to_vec(&entry).unwrap()).unwrap();
        assert_eq!(c.get("service", "Stale"), None);
    }

    #[test]
    fn get_returns_none_on_name_mismatch() {
        let tmp = TempDir::new().unwrap();
        let c = cache_in(&tmp);
        // Write an entry whose stored `name` disagrees with the filename.
        // In practice this only happens on hand-edit or a (2^256 odds)
        // hash collision, but the defense is cheap.
        let path = c.path_for("service", "Requested");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let entry = Entry {
            version: CACHE_VERSION,
            name: "Different".to_string(),
            id: "PWRONG".to_string(),
            ts: now_secs(),
        };
        fs::write(&path, serde_json::to_vec(&entry).unwrap()).unwrap();
        assert_eq!(c.get("service", "Requested"), None);
    }

    #[test]
    fn get_returns_none_on_version_mismatch() {
        let tmp = TempDir::new().unwrap();
        let c = cache_in(&tmp);
        let path = c.path_for("service", "OldSchema");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // Raw JSON with an unknown version field. Deserialize succeeds;
        // the read path rejects on version != 1.
        let blob = br#"{"version": 99, "name": "OldSchema", "id": "POLD", "ts": 0}"#;
        fs::write(&path, blob).unwrap();
        assert_eq!(c.get("service", "OldSchema"), None);
    }

    #[test]
    fn invalidate_entry_removes_file() {
        let tmp = TempDir::new().unwrap();
        let c = cache_in(&tmp);
        c.put("service", "Foo", "PFOO");
        assert!(c.get("service", "Foo").is_some());
        c.invalidate_entry("service", "Foo");
        assert!(c.get("service", "Foo").is_none());
    }

    #[test]
    fn invalidate_type_removes_subtree() {
        let tmp = TempDir::new().unwrap();
        let c = cache_in(&tmp);
        c.put("service", "Foo", "PFOO");
        c.put("service", "Bar", "PBAR");
        c.put("team", "Infra", "PINF");
        c.invalidate_type("service");
        assert!(c.get("service", "Foo").is_none());
        assert!(c.get("service", "Bar").is_none());
        assert_eq!(c.get("team", "Infra").as_deref(), Some("PINF"));
    }

    #[test]
    fn invalidate_subdomain_removes_everything() {
        let tmp = TempDir::new().unwrap();
        let c = cache_in(&tmp);
        c.put("service", "Foo", "PFOO");
        c.put("team", "Infra", "PINF");
        c.invalidate_subdomain();
        assert!(c.get("service", "Foo").is_none());
        assert!(c.get("team", "Infra").is_none());
    }

    #[test]
    fn subdomain_namespacing_isolates_accounts() {
        let tmp = TempDir::new().unwrap();
        let prod = Cache::with_root(tmp.path().join("ids").join("tatari"));
        let staging = Cache::with_root(tmp.path().join("ids").join("tatari-staging"));
        prod.put("service", "Platform", "PPROD");
        staging.put("service", "Platform", "PSTAGING");
        assert_eq!(prod.get("service", "Platform").as_deref(), Some("PPROD"));
        assert_eq!(staging.get("service", "Platform").as_deref(), Some("PSTAGING"));
    }
}
