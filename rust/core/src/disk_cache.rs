//! Small on-disk caches used for instant cold starts (stale-while-revalidate).
//!
//! File layout: 4-byte magic `KQC\0`, little-endian `u32` format version, a
//! little-endian `u32` schema version supplied by the caller, then a
//! [postcard] payload of `(timestamp, value)`. Files with a different magic or
//! version are ignored (and later overwritten) instead of being misparsed.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde::de::DeserializeOwned;

const MAGIC: &[u8; 4] = b"KQC\0";
const FORMAT_VERSION: u32 = 1;

pub fn cache_dir() -> PathBuf {
    let mut path = dirs::cache_dir().unwrap_or_else(|| std::env::temp_dir().join("kiosque-cache"));
    path.push("kiosque");
    fs::create_dir_all(&path).ok();
    path
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

pub fn encode<T: Serialize>(schema: u32, timestamp: u64, value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(256);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&schema.to_le_bytes());
    let payload = postcard::to_stdvec(&(timestamp, value)).map_err(|e| e.to_string())?;
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

pub fn decode<T: DeserializeOwned>(schema: u32, bytes: &[u8]) -> Option<(u64, T)> {
    if bytes.len() < 12 || &bytes[..4] != MAGIC {
        return None;
    }
    let format = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let found_schema = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    if format != FORMAT_VERSION || found_schema != schema {
        return None;
    }
    postcard::from_bytes(&bytes[12..]).ok()
}

/// Independent disk cache. Tests supply temporary directories and timestamps.
#[derive(Clone)]
pub struct DiskCache {
    dir: PathBuf,
}

impl DiskCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path(&self, name: &str) -> Option<PathBuf> {
        (!name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .then(|| self.dir.join(format!("{}.bin", name)))
    }

    pub async fn save_at<T: Serialize>(
        &self,
        name: &str,
        schema: u32,
        timestamp: u64,
        value: &T,
    ) -> Result<(), String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = self.path(name).ok_or("invalid cache name")?;
        let bytes = encode(schema, timestamp, value)?;
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(|e| e.to_string())?;
        let tmp = path.with_extension(format!(
            "{}.{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let result = async {
            tokio::fs::write(&tmp, bytes)
                .await
                .map_err(|e| e.to_string())?;
            tokio::fs::rename(&tmp, &path)
                .await
                .map_err(|e| e.to_string())
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&tmp).await;
        }
        result
    }

    pub async fn load<T: DeserializeOwned>(&self, name: &str, schema: u32) -> Option<(u64, T)> {
        let bytes = tokio::fs::read(self.path(name)?).await.ok()?;
        decode(schema, &bytes)
    }

    pub async fn remove(&self, name: &str) {
        if let Some(path) = self.path(name) {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
}

pub async fn save<T: Serialize>(name: &str, schema: u32, value: &T) -> Result<(), String> {
    DiskCache::new(cache_dir())
        .save_at(name, schema, now_secs(), value)
        .await
}
pub async fn load<T: DeserializeOwned>(name: &str, schema: u32) -> Option<(u64, T)> {
    DiskCache::new(cache_dir()).load(name, schema).await
}
pub async fn remove(name: &str) {
    DiskCache::new(cache_dir()).remove(name).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, serde::Deserialize, Debug, PartialEq)]
    struct Item {
        a: String,
        b: u32,
    }

    #[test]
    fn roundtrip() {
        let v = vec![Item {
            a: "x".into(),
            b: 1,
        }];
        let bytes = encode(3, 42, &v).unwrap();
        let (ts, back): (u64, Vec<Item>) = decode(3, &bytes).unwrap();
        assert_eq!(ts, 42);
        assert_eq!(back, v);
    }

    #[test]
    fn rejects_other_versions_and_garbage() {
        let bytes = encode(
            3,
            42,
            &vec![Item {
                a: "x".into(),
                b: 1,
            }],
        )
        .unwrap();
        assert!(decode::<Vec<Item>>(4, &bytes).is_none(), "schema mismatch");
        assert!(
            decode::<Vec<Item>>(3, &bytes[..10]).is_none(),
            "truncated header"
        );
        // the pre-versioning format started directly with a u64 timestamp
        let mut legacy = 1_700_000_000u64.to_le_bytes().to_vec();
        legacy.extend_from_slice(&[0, 0, 0, 0]);
        assert!(decode::<Vec<Item>>(3, &legacy).is_none());
        let mut corrupt = bytes.clone();
        corrupt.truncate(bytes.len() - 1);
        assert!(decode::<Vec<Item>>(3, &corrupt).is_none());
    }
}
