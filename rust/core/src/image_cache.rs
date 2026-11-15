//! URL-keyed image loading, original-byte disk caching, and decoded-frame reuse.
use crate::flathub::client::HttpClient;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use image::{DynamicImage, ImageReader};
use moka::future::Cache;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
    time::{Duration, SystemTime},
};

pub const MAX_CACHE_BYTES: u64 = 256 * 1024 * 1024;
const EVICT_TARGET: f64 = 0.8;
type DownloadLocks = Arc<Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>>;

pub fn image_url(raw: &str) -> String {
    if raw.is_empty() || raw.starts_with("image://") {
        return raw.to_string();
    }
    format!(
        "image://kiosque/u/{}",
        URL_SAFE_NO_PAD.encode(raw.as_bytes())
    )
}
pub fn url_from_id(id: &str) -> Option<String> {
    let enc = id.strip_prefix("u/")?;
    String::from_utf8(URL_SAFE_NO_PAD.decode(enc.split(['?', '#']).next()?).ok()?).ok()
}
pub fn cache_path_for(dir: &Path, url: &str) -> PathBuf {
    let hash = Sha256::digest(url.as_bytes());
    let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
    dir.join(hex)
}

#[derive(Clone)]
pub struct ImageLoader {
    http: HttpClient,
    dir: PathBuf,
    decoded: Cache<String, Arc<image::RgbaImage>>,
    downloads: DownloadLocks,
}
impl ImageLoader {
    pub fn new(http: HttpClient, dir: impl Into<PathBuf>) -> Self {
        Self::with_cache_policy(http, dir, 96 * 1024 * 1024, Duration::from_secs(600))
    }
    pub fn with_cache_policy(
        http: HttpClient,
        dir: impl Into<PathBuf>,
        capacity: u64,
        ttl: Duration,
    ) -> Self {
        Self {
            http,
            dir: dir.into(),
            downloads: Arc::new(Mutex::new(HashMap::new())),
            decoded: Cache::builder()
                .max_capacity(capacity)
                .time_to_live(ttl)
                .weigher(|_: &String, frame: &Arc<image::RgbaImage>| {
                    frame.as_raw().len().min(u32::MAX as usize) as u32
                })
                .build(),
        }
    }
    fn download_lock(&self, url: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.downloads.lock().unwrap();
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(url).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(url.into(), Arc::downgrade(&lock));
        lock
    }
    async fn source_for(&self, url: &str) -> Result<DynamicImage, String> {
        if let Some(path) = local_path(url) {
            let bytes = tokio::fs::read(path)
                .await
                .map_err(|e| format!("cannot read image: {e}"))?;
            return decode_async(bytes).await;
        }
        let lock = self.download_lock(url);
        let _guard = lock.lock().await;
        let path = cache_path_for(&self.dir, url);
        if let Ok(bytes) = tokio::fs::read(&path).await {
            if let Ok(image) = decode_async(bytes).await {
                if let Ok(file) = fs::File::options().append(true).open(&path) {
                    let _ = file.set_modified(SystemTime::now());
                }
                return Ok(image);
            }
            let _ = tokio::fs::remove_file(&path).await;
        }
        let bytes = self
            .http
            .fetch_bytes(url)
            .await
            .map_err(|e| e.to_string())?;
        let decoded = decode_async(bytes.clone()).await?;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = path.with_extension(format!("{}.{}.part", std::process::id(), serial));
        if tokio::fs::create_dir_all(&self.dir).await.is_ok()
            && tokio::fs::write(&tmp, &bytes).await.is_ok()
        {
            let _ = tokio::fs::rename(&tmp, &path).await;
        }
        let _ = tokio::fs::remove_file(tmp).await;
        let dir = self.dir.clone();
        let _ = tokio::task::spawn_blocking(move || evict(&dir, MAX_CACHE_BYTES)).await;
        Ok(decoded)
    }
    pub async fn load(
        &self,
        id: &str,
        width: i32,
        height: i32,
    ) -> Result<Arc<image::RgbaImage>, String> {
        let url = url_from_id(id).ok_or_else(|| "invalid image id".to_string())?;
        let local_version = local_path(&url)
            .and_then(|path| fs::metadata(path).ok())
            .map(|m| (m.len(), m.modified().ok()));
        let key = format!("{}:{url}:{width}:{height}:{local_version:?}", url.len());
        self.decoded
            .try_get_with(key, async {
                let source = self.source_for(&url).await?;
                tokio::task::spawn_blocking(move || resize(source, width, height).map(Arc::new))
                    .await
                    .map_err(|e| e.to_string())?
            })
            .await
            .map_err(|error: Arc<String>| error.to_string())
    }
}
fn local_path(url: &str) -> Option<PathBuf> {
    if url.starts_with("file://") {
        return reqwest::Url::parse(url).ok()?.to_file_path().ok();
    }
    url.starts_with('/').then(|| PathBuf::from(url))
}
async fn decode_async(bytes: Vec<u8>) -> Result<DynamicImage, String> {
    tokio::task::spawn_blocking(move || decode(bytes))
        .await
        .map_err(|e| e.to_string())?
}

fn decode(bytes: Vec<u8>) -> Result<DynamicImage, String> {
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())
}
fn resize(img: DynamicImage, width: i32, height: i32) -> Result<image::RgbaImage, String> {
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    let ratio = (if width > 0 {
        width as f64 / w as f64
    } else {
        f64::INFINITY
    })
    .min(if height > 0 {
        height as f64 / h as f64
    } else {
        f64::INFINITY
    });
    if ratio >= 1.0 {
        return Ok(rgba);
    }
    let target_w = (w as f64 * ratio).round().max(1.0) as u32;
    let target_h = (h as f64 * ratio).round().max(1.0) as u32;
    use fast_image_resize as fir;
    let source = fir::images::Image::from_vec_u8(w, h, rgba.into_raw(), fir::PixelType::U8x4)
        .map_err(|e| e.to_string())?;
    let mut target = fir::images::Image::new(target_w, target_h, fir::PixelType::U8x4);
    let options = fir::ResizeOptions::new()
        .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::CatmullRom));
    fir::Resizer::new()
        .resize(&source, &mut target, &options)
        .map_err(|e| e.to_string())?;
    image::RgbaImage::from_raw(target_w, target_h, target.into_vec())
        .ok_or_else(|| "invalid resized image".into())
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
