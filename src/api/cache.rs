use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: u32 = 1;
const DEFAULT_TTL_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone)]
pub struct CachePolicy {
    pub enabled: bool,
    pub refresh: bool,
    pub ttl_secs: u64,
    pub cache_dir: PathBuf,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            refresh: false,
            ttl_secs: DEFAULT_TTL_SECS,
            cache_dir: default_cache_dir(),
        }
    }
}

impl CachePolicy {
    pub fn new(enabled: bool, refresh: bool, ttl_secs: u64, cache_dir: Option<PathBuf>) -> Self {
        Self {
            enabled,
            refresh,
            ttl_secs,
            cache_dir: cache_dir.unwrap_or_else(default_cache_dir),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheFreshness {
    Fresh,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheLookup {
    Miss,
    Hit {
        payload: String,
        freshness: CacheFreshness,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEnvelope {
    schema_version: u32,
    created_at_unix: u64,
    payload: String,
}

pub fn default_cache_dir() -> PathBuf {
    if let Ok(xdg_cache_home) = std::env::var("XDG_CACHE_HOME") {
        let trimmed = xdg_cache_home.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("mapto3d");
        }
    }

    PathBuf::from(".mapto3d-cache")
}

pub fn normalized_payload(payload: &str) -> String {
    payload.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn make_cache_key(namespace: &str, payload: &str) -> String {
    let normalized = normalized_payload(payload);
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    hasher.update(b":");
    hasher.update(normalized.as_bytes());
    let digest = hasher.finalize();

    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{:02x}", byte);
    }
    hex
}

pub fn lookup(policy: &CachePolicy, namespace: &str, payload: &str) -> Result<CacheLookup> {
    if !policy.enabled {
        return Ok(CacheLookup::Miss);
    }

    let path = cache_path(&policy.cache_dir, namespace, payload);
    if !path.exists() {
        return Ok(CacheLookup::Miss);
    }

    let raw = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!(
                "Warning: failed to read cache entry {}: {}",
                path.display(),
                err
            );
            return Ok(CacheLookup::Miss);
        }
    };

    let envelope: CacheEnvelope = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!(
                "Warning: failed to parse cache entry {}: {}",
                path.display(),
                err
            );
            return Ok(CacheLookup::Miss);
        }
    };

    if envelope.schema_version != SCHEMA_VERSION {
        return Ok(CacheLookup::Miss);
    }

    let freshness = freshness_at(envelope.created_at_unix, unix_now(), policy.ttl_secs);
    Ok(CacheLookup::Hit {
        payload: envelope.payload,
        freshness,
    })
}

pub fn store(
    policy: &CachePolicy,
    namespace: &str,
    request_payload: &str,
    response_payload: &str,
) -> Result<()> {
    if !policy.enabled {
        return Ok(());
    }

    let path = cache_path(&policy.cache_dir, namespace, request_payload);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create cache directory {}",
                parent.to_string_lossy()
            )
        })?;
    }

    let envelope = CacheEnvelope {
        schema_version: SCHEMA_VERSION,
        created_at_unix: unix_now(),
        payload: response_payload.to_string(),
    };

    let serialized = serde_json::to_string(&envelope).context("Failed to serialize cache entry")?;
    fs::write(&path, serialized)
        .with_context(|| format!("Failed to write cache entry {}", path.display()))?;
    Ok(())
}

pub fn get_or_fetch_payload<F>(
    policy: &CachePolicy,
    namespace: &str,
    request_payload: &str,
    fetcher: F,
) -> Result<String>
where
    F: FnOnce() -> Result<String>,
{
    if !policy.enabled {
        return fetcher();
    }

    let cached = lookup(policy, namespace, request_payload)?;
    if !policy.refresh
        && let CacheLookup::Hit {
            payload,
            freshness: CacheFreshness::Fresh,
        } = &cached
    {
        return Ok(payload.clone());
    }

    match fetcher() {
        Ok(payload) => {
            if let Err(err) = store(policy, namespace, request_payload, &payload) {
                eprintln!("Warning: failed to update cache for {}: {}", namespace, err);
            }
            Ok(payload)
        }
        Err(network_err) => match cached {
            CacheLookup::Hit { payload, freshness } => {
                let cache_kind = match freshness {
                    CacheFreshness::Fresh => "fresh",
                    CacheFreshness::Stale => "stale",
                };
                eprintln!(
                    "Warning: network request failed ({}); using {} cache for {}",
                    network_err, cache_kind, namespace
                );
                Ok(payload)
            }
            CacheLookup::Miss => Err(network_err),
        },
    }
}

fn cache_path(cache_dir: &Path, namespace: &str, payload: &str) -> PathBuf {
    let key = make_cache_key(namespace, payload);
    cache_dir.join(namespace).join(format!("{key}.json"))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn freshness_at(created_at_unix: u64, now_unix: u64, ttl_secs: u64) -> CacheFreshness {
    let age = now_unix.saturating_sub(created_at_unix);
    if age <= ttl_secs {
        CacheFreshness::Fresh
    } else {
        CacheFreshness::Stale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_cache_key_is_deterministic_after_normalization() {
        let key_a = make_cache_key("overpass", "line one\nline\t two");
        let key_b = make_cache_key("overpass", "line one line two");
        assert_eq!(key_a, key_b);
    }

    #[test]
    fn test_freshness_resolution() {
        let now = 1_000;
        assert_eq!(freshness_at(950, now, 60), CacheFreshness::Fresh);
        assert_eq!(freshness_at(900, now, 60), CacheFreshness::Stale);
    }

    #[test]
    fn test_lookup_detects_stale_and_fresh_entries() {
        let temp = tempfile::tempdir().unwrap();
        let cache_dir = temp.path().to_path_buf();
        let policy = CachePolicy::new(true, false, 5, Some(cache_dir.clone()));
        let request = "q=test";

        let stale_entry = CacheEnvelope {
            schema_version: SCHEMA_VERSION,
            created_at_unix: unix_now().saturating_sub(10),
            payload: "stale".to_string(),
        };
        let stale_path = cache_path(&cache_dir, "nominatim", request);
        fs::create_dir_all(stale_path.parent().unwrap()).unwrap();
        fs::write(&stale_path, serde_json::to_string(&stale_entry).unwrap()).unwrap();

        let stale_lookup = lookup(&policy, "nominatim", request).unwrap();
        assert_eq!(
            stale_lookup,
            CacheLookup::Hit {
                payload: "stale".to_string(),
                freshness: CacheFreshness::Stale
            }
        );

        let fresh_entry = CacheEnvelope {
            schema_version: SCHEMA_VERSION,
            created_at_unix: unix_now(),
            payload: "fresh".to_string(),
        };
        fs::write(&stale_path, serde_json::to_string(&fresh_entry).unwrap()).unwrap();

        let fresh_lookup = lookup(&policy, "nominatim", request).unwrap();
        assert_eq!(
            fresh_lookup,
            CacheLookup::Hit {
                payload: "fresh".to_string(),
                freshness: CacheFreshness::Fresh
            }
        );
    }

    #[test]
    fn test_get_or_fetch_returns_stale_payload_on_fetch_error() {
        let temp = tempfile::tempdir().unwrap();
        let cache_dir = temp.path().to_path_buf();
        let policy = CachePolicy::new(true, false, 1, Some(cache_dir.clone()));
        let request = "data=query";

        let stale_entry = CacheEnvelope {
            schema_version: SCHEMA_VERSION,
            created_at_unix: unix_now().saturating_sub(10),
            payload: "cached".to_string(),
        };
        let stale_path = cache_path(&cache_dir, "overpass", request);
        fs::create_dir_all(stale_path.parent().unwrap()).unwrap();
        fs::write(&stale_path, serde_json::to_string(&stale_entry).unwrap()).unwrap();

        let payload = get_or_fetch_payload(&policy, "overpass", request, || {
            anyhow::bail!("network down")
        })
        .unwrap();

        assert_eq!(payload, "cached");
    }
}
