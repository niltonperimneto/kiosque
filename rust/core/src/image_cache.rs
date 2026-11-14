//! Image loading for the `image://kiosque/` QML image provider.
//!
//! QML image ids carry the source URL itself (`u/<base64url(url)>`), so no
//! process-wide id→URL registry is needed. Downloaded images are stored
//! byte-for-byte (no lossy re-encoding) under `<cache>/images/<sha256(url)>`
//! and decoded on demand. The directory is capped at [`MAX_CACHE_BYTES`];
//! the least recently used files (by mtime, refreshed on every hit) are
//! evicted first. `file://` URLs (icons from local AppStream data) are read
//! directly and never copied into the cache.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use image::{DynamicImage, ImageReader};
use sha2::{Digest, Sha256};

/// Upper bound for the on-disk image cache.
pub const MAX_CACHE_BYTES: u64 = 256 * 1024 * 1024;
/// Eviction trims down to this fraction of the cap.
const EVICT_TARGET: f64 = 0.8;

pub fn image_url(raw: &str) -> String {
    if raw.is_empty() || raw.starts_with("image://") {
        return raw.to_string();
    }
    format!(
        "image://kiosque/u/{}",
        URL_SAFE_NO_PAD.encode(raw.as_bytes())
    )
}

/// Inverse of [`image_url`]'s id part.
pub fn url_from_id(id: &str) -> Option<String> {
    let enc = id.strip_prefix("u/")?;
    let bytes = URL_SAFE_NO_PAD.decode(enc.split(['?', '#']).next()?).ok()?;
    String::from_utf8(bytes).ok()
}

fn image_cache_dir() -> PathBuf {
    let dir = crate::disk_cache::cache_dir().join("images");
    fs::create_dir_all(&dir).ok();
    dir
}

pub fn cache_path_for(dir: &Path, url: &str) -> PathBuf {
    let hash = Sha256::digest(url.as_bytes());
    let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
    dir.join(hex)
}

fn touch(path: &Path) {
    if let Ok(f) = fs::File::options().append(true).open(path) {
        let _ = f.set_modified(SystemTime::now());
    }
}

async fn download(url: &str) -> Result<Vec<u8>, String> {
    crate::flathub::client::fetch_bytes(url)
        .await
        .map_err(|e| format!("download failed: {}", e))
}

/// Return the original bytes of `url`, from the cache when possible.
async fn bytes_for(url: &str) -> Result<Vec<u8>, String> {
    if let Some(path) = url.strip_prefix("file://") {
        return fs::read(path).map_err(|e| format!("cannot read {}: {}", path, e));
    }
    if url.starts_with('/') {
        return fs::read(url).map_err(|e| format!("cannot read {}: {}", url, e));
    }
    let dir = image_cache_dir();
    let path = cache_path_for(&dir, url);
    if let Ok(bytes) = fs::read(&path) {
        touch(&path);
        return Ok(bytes);
    }
    let bytes = download(url).await?;
    let tmp = path.with_extension("part");
    if fs::write(&tmp, &bytes)
        .and_then(|_| fs::rename(&tmp, &path))
        .is_err()
    {
        let _ = fs::remove_file(&tmp);
    }
    maybe_evict(&dir);
    Ok(bytes)
}

/// Run eviction at most once a minute, off the calling thread.
fn maybe_evict(dir: &Path) {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    {
        let mut last = LAST.lock().unwrap();
        if last.is_some_and(|t| t.elapsed() < Duration::from_secs(60)) {
            return;
        }
        *last = Some(Instant::now());
    }
    let dir = dir.to_path_buf();
    std::thread::spawn(move || {
        let removed = evict(&dir, MAX_CACHE_BYTES);
        if removed > 0 {
            klog!("image cache: evicted {} files", removed);
        }
    });
}

/// Delete least-recently-used files until the directory is below
/// `EVICT_TARGET * cap`. Also removes leftovers of the old JPEG-XL cache.
/// Returns the number of files removed.
pub fn evict(dir: &Path, cap: u64) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut files = vec![];
    let mut total = 0u64;
    let mut removed = 0;
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().is_some_and(|x| x == "jxl" || x == "part") {
            if fs::remove_file(&path).is_ok() {
                removed += 1;
            }
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        total += meta.len();
        files.push((
            meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            meta.len(),
            path,
        ));
    }
    if total <= cap {
        return removed;
    }
    files.sort_by_key(|f| f.0);
    let target = (cap as f64 * EVICT_TARGET) as u64;
    for (_, len, path) in files {
        if total <= target {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total -= len;
            removed += 1;
        }
    }
    removed
}

fn decode(bytes: Vec<u8>) -> Result<DynamicImage, String> {
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("unknown image format: {}", e))?
        .decode()
        .map_err(|e| format!("decode failed: {}", e))
}

pub async fn fetch_and_process(
    id: &str,
    req_width: i32,
    req_height: i32,
) -> Result<image::RgbaImage, String> {
    let url = url_from_id(id).ok_or_else(|| format!("bad image id {}", id))?;
    let bytes = bytes_for(&url).await?;
    let img = tokio::task::spawn_blocking(move || -> Result<image::RgbaImage, String> {
        let img = decode(bytes)?;
        let fits = |w: i32, h: i32| {
            (w <= 0 || img.width() as i32 <= w) && (h <= 0 || img.height() as i32 <= h)
        };
        let img = if (req_width > 0 || req_height > 0) && !fits(req_width, req_height) {
            let w = if req_width > 0 {
                req_width as u32
            } else {
                u32::MAX
            };
            let h = if req_height > 0 {
                req_height as u32
            } else {
                u32::MAX
            };
            img.resize(w, h, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };
        Ok(img.to_rgba8())
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_roundtrip() {
        let url =
            "https://dl.flathub.org/media/org/gimp/GIMP/icons/128x128/org.gimp.GIMP.png?x=1&y=/2";
        let q = image_url(url);
        assert!(q.starts_with("image://kiosque/u/"));
        let id = q.strip_prefix("image://kiosque/").unwrap();
        assert!(!id[2..].contains('/'), "id must stay a single path segment");
        assert_eq!(url_from_id(id).unwrap(), url);
        assert_eq!(image_url(""), "");
        assert!(url_from_id("app/icon").is_none());
    }

    #[test]
    fn cache_key_is_per_url() {
        let d = Path::new("/c");
        assert_ne!(cache_path_for(d, "a"), cache_path_for(d, "b"));
        assert_eq!(cache_path_for(d, "a"), cache_path_for(d, "a"));
    }

    #[test]
    fn eviction_removes_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        let old = SystemTime::now() - Duration::from_secs(3600);
        for (i, name) in ["a", "b", "c", "d"].iter().enumerate() {
            let p = dir.path().join(name);
            fs::write(&p, vec![0u8; 100]).unwrap();
            fs::File::options()
                .append(true)
                .open(&p)
                .unwrap()
                .set_modified(old + Duration::from_secs(i as u64 * 60))
                .unwrap();
        }
        fs::write(dir.path().join("legacy.jxl"), b"x").unwrap();
        // 400 bytes, cap 300 -> trim to 240: drop "a" and "b"
        let removed = evict(dir.path(), 300);
        assert_eq!(removed, 3);
        assert!(!dir.path().join("a").exists());
        assert!(!dir.path().join("b").exists());
        assert!(dir.path().join("c").exists());
        assert!(dir.path().join("d").exists());
        assert!(!dir.path().join("legacy.jxl").exists());
    }
}
