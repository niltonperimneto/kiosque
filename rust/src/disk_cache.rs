use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::bridge::featured_model::FeaturedEntry;
use crate::bridge::app_list_model::AppEntry;
use crate::bridge::installed_model::InstalledEntry;

// ── Cache Dir Helper ────────────────────────────────────────────────────────

pub fn cache_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let mut path = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("~/.cache"));
        path.push("kiosque");
        fs::create_dir_all(&path).ok();

        // Best-effort removal of the legacy hand-rolled binary cache files
        // (superseded by the versioned JSON envelopes below).
        for legacy in ["featured.bin", "popular.bin", "installed.bin"] {
            let _ = fs::remove_file(path.join(legacy));
        }

        path
    })
    .clone()
}

// ── Generic versioned cache envelope ─────────────────────────────────────────

/// Bumped whenever the on-disk schema changes; mismatched files are treated as a
/// cache miss and transparently re-fetched.
const CACHE_VERSION: u8 = 1;

/// How long a persisted list stays fresh before the caller should revalidate.
pub const DISK_CACHE_TTL_SECS: u64 = 5 * 60; // 5 minutes

#[derive(serde::Deserialize)]
struct CacheEnvelope<T> {
    version: u8,
    timestamp: u64,
    items: Vec<T>,
}

/// Borrowing twin of [`CacheEnvelope`] used on the write path so we can serialize
/// directly from a `&[T]` without cloning into a `Vec`.
#[derive(Serialize)]
struct CacheEnvelopeRef<'a, T> {
    version: u8,
    timestamp: u64,
    items: &'a [T],
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn cache_path(name: &str) -> PathBuf {
    cache_dir().join(format!("{}.json", name))
}

/// Atomically replace `path` with `bytes` (write a sibling temp file, then rename)
/// so a crash mid-write can never leave a truncated cache file behind.
async fn write_atomic(path: &Path, bytes: Vec<u8>) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, path).await
}

/// Serialize `items` into a versioned, timestamped envelope and persist it.
pub async fn save_cache<T: Serialize>(name: &str, items: &[T]) -> Result<(), String> {
    let envelope = CacheEnvelopeRef {
        version: CACHE_VERSION,
        timestamp: now_secs(),
        items,
    };
    let bytes = serde_json::to_vec(&envelope)
        .map_err(|e| format!("Failed to serialize {} cache: {}", name, e))?;
    write_atomic(&cache_path(name), bytes)
        .await
        .map_err(|e| format!("Failed to write {} cache: {}", name, e))
}

/// Load a persisted list, returning `(items, is_expired)`. Returns `None` when the
/// file is missing, unreadable, the wrong version, or malformed — callers treat
/// that as a plain cache miss.
pub async fn load_cache<T: DeserializeOwned>(name: &str) -> Option<(Vec<T>, bool)> {
    let bytes = tokio::fs::read(cache_path(name)).await.ok()?;
    let envelope: CacheEnvelope<T> = serde_json::from_slice(&bytes).ok()?;
    if envelope.version != CACHE_VERSION {
        return None;
    }
    let is_expired = now_secs().saturating_sub(envelope.timestamp) > DISK_CACHE_TTL_SECS;
    Some((envelope.items, is_expired))
}

// ── Typed wrappers ───────────────────────────────────────────────────────────

pub async fn save_featured_cache(items: &[FeaturedEntry]) -> Result<(), String> {
    // "featured_v2": FeaturedEntry gained screenshot/branding fields; the old
    // cache would deserialize with empty defaults and pin a screenshot-less
    // hero until expiry, so it is simply left behind.
    save_cache("featured_v2", items).await
}

pub async fn load_featured_cache() -> Option<(Vec<FeaturedEntry>, bool)> {
    load_cache("featured_v2").await
}

pub async fn save_popular_cache(items: &[AppEntry]) -> Result<(), String> {
    save_cache("popular", items).await
}

pub async fn load_popular_cache() -> Option<(Vec<AppEntry>, bool)> {
    load_cache("popular").await
}

pub async fn save_installed_cache(items: &[InstalledEntry]) -> Result<(), String> {
    save_cache("installed", items).await
}

pub async fn load_installed_cache() -> Option<(Vec<InstalledEntry>, bool)> {
    load_cache("installed").await
}

pub async fn invalidate_installed_cache() {
    let _ = tokio::fs::remove_file(cache_path("installed")).await;
}
