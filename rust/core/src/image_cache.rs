//! Image sanitizing cache.
//!
//! QML never decodes network data. Every image is fetched here, checked
//! (HTTPS only, body ≤ [`MAX_BODY_BYTES`], PNG/JPEG/WebP only, decoder limits
//! of [`MAX_DIMENSION`]² pixels and [`MAX_DECODE_ALLOC`] bytes), resized for
//! its [`ImageKind`] and re-encoded as an RGBA PNG under
//! `<cache>/img/v2/<sha256(url)>-<kind>.png`. QML only ever receives
//! `file://` URLs of PNG files written by this module.
//!
//! Local icons from installed AppStream data (`file://` or absolute paths) go
//! through the same decode/re-encode path. The directory is capped at
//! [`MAX_CACHE_BYTES`]; least recently used files (by mtime, refreshed on each
//! hit) are evicted first.

use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use futures_util::future::BoxFuture;
use image::{ExtendedColorType, ImageEncoder, ImageFormat, ImageReader, Limits};
use sha2::{Digest, Sha256};
use tokio::sync::{OnceCell, Semaphore};

/// Upper bound for the on-disk image cache.
pub const MAX_CACHE_BYTES: u64 = 256 * 1024 * 1024;
/// Eviction trims down to this fraction of the cap.
const EVICT_TARGET: f64 = 0.8;
/// Largest accepted download or local source file.
pub const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
/// Largest accepted width or height of a source image.
pub const MAX_DIMENSION: u32 = 8192;
/// Decoder allocation limit.
pub const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;
/// Concurrent fetch/transcode jobs.
pub const MAX_CONCURRENT: usize = 6;

/// What an image is used for; decides its output size and cache key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageKind {
    /// Square icon, longest side at most `px` (clamped to 16…512).
    Icon(u32),
    /// Screenshot, fitted into 1920×1920.
    Screenshot,
    /// User avatar, fitted into 256×256.
    Avatar,
}

impl ImageKind {
    /// Parse the QML-facing form: `icon`, `icon:<px>`, `screenshot`, `avatar`.
    pub fn parse(kind: &str) -> Option<Self> {
        match kind {
            "screenshot" => Some(Self::Screenshot),
            "avatar" => Some(Self::Avatar),
            "icon" => Some(Self::Icon(128)),
            _ => kind
                .strip_prefix("icon:")
                .and_then(|px| px.parse().ok())
                .map(Self::Icon),
        }
        .map(Self::normalized)
    }

    fn normalized(self) -> Self {
        match self {
            Self::Icon(px) => Self::Icon(px.clamp(16, 512)),
            k => k,
        }
    }

    pub fn tag(self) -> String {
        match self.normalized() {
            Self::Icon(px) => format!("icon{px}"),
            Self::Screenshot => "screenshot".into(),
            Self::Avatar => "avatar".into(),
        }
    }

    fn bounds(self) -> (u32, u32) {
        match self.normalized() {
            Self::Icon(px) => (px, px),
            Self::Screenshot => (1920, 1920),
            Self::Avatar => (256, 256),
        }
    }
}

/// `<cache>/img/v2`, created on demand.
pub fn default_dir() -> PathBuf {
    crate::disk_cache::cache_dir().join("img").join("v2")
}

pub fn png_path(dir: &Path, url: &str, kind: ImageKind) -> PathBuf {
    let hash = Sha256::digest(url.as_bytes());
    let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
    dir.join(format!("{hex}-{}.png", kind.tag()))
}

fn touch(path: &Path) {
    if let Ok(f) = fs::File::options().append(true).open(path) {
        let _ = f.set_modified(SystemTime::now());
    }
}

/// Where an image comes from, after the scheme policy has been applied.
#[derive(Debug, PartialEq)]
enum Source {
    Remote(String),
    Local(PathBuf),
}

fn classify(url: &str, allow_http_loopback: bool) -> Result<Source, String> {
    if url.starts_with("https://") {
        return Ok(Source::Remote(url.to_string()));
    }
    if let Some(path) = url.strip_prefix("file://") {
        return Ok(Source::Local(PathBuf::from(path)));
    }
    if url.starts_with('/') {
        return Ok(Source::Local(PathBuf::from(url)));
    }
    if allow_http_loopback
        && (url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:"))
    {
        return Ok(Source::Remote(url.to_string()));
    }
    Err(format!(
        "refusing image source {url:?}: only https is allowed"
    ))
}

/// Decode `bytes` under the module's limits and re-encode as an RGBA PNG
/// fitted into `kind`'s bounds.
pub fn transcode(bytes: &[u8], kind: ImageKind) -> Result<Vec<u8>, String> {
    if bytes.len() > MAX_BODY_BYTES {
        return Err(format!("image too large ({} bytes)", bytes.len()));
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("unreadable image: {e}"))?;
    match reader.format() {
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP) => {}
        Some(f) => return Err(format!("unsupported image format {f:?}")),
        None => return Err("unknown image format".into()),
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let img = reader.decode().map_err(|e| format!("decode failed: {e}"))?;
    let (bw, bh) = kind.bounds();
    let img = if img.width() > bw || img.height() > bh {
        img.resize(bw, bh, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&rgba, rgba.width(), rgba.height(), ExtendedColorType::Rgba8)
        .map_err(|e| format!("encode failed: {e}"))?;
    Ok(out)
}

/// Write `data` to `path` through a unique temporary file and a rename, so
/// readers never observe a partial PNG.
fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.{}.part",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let res = fs::write(&tmp, data).and_then(|_| fs::rename(&tmp, path));
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    res
}

/// Fetches the raw bytes of a remote image. Production uses
/// [`HttpClient::fetch_bytes_capped`](crate::flathub::client::HttpClient::fetch_bytes_capped).
pub type Fetcher = Arc<dyn Fn(String) -> BoxFuture<'static, Result<Vec<u8>, String>> + Send + Sync>;

type Slot = Arc<OnceCell<Result<PathBuf, String>>>;

/// Deduplicating, concurrency-limited fetch → transcode → cache pipeline.
pub struct ImageLoader {
    dir: PathBuf,
    fetch: Fetcher,
    permits: Semaphore,
    inflight: Mutex<HashMap<PathBuf, Slot>>,
    allow_http_loopback: bool,
}

impl ImageLoader {
    /// Loader over `dir` using the shared HTTP client.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let http = crate::flathub::client::HttpClient::default();
        Self::with_fetcher(
            dir,
            Arc::new(move |url| {
                let http = http.clone();
                Box::pin(async move {
                    http.fetch_bytes_capped(&url, MAX_BODY_BYTES)
                        .await
                        .map_err(|e| format!("download failed: {e}"))
                })
            }),
        )
    }

    pub fn with_fetcher(dir: impl Into<PathBuf>, fetch: Fetcher) -> Self {
        Self {
            dir: dir.into(),
            fetch,
            permits: Semaphore::new(MAX_CONCURRENT),
            inflight: Mutex::new(HashMap::new()),
            allow_http_loopback: false,
        }
    }

    /// Test fixtures serve plain HTTP on loopback; production never sets this.
    pub fn allow_http_loopback(mut self) -> Self {
        self.allow_http_loopback = true;
        self
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The cached PNG for `url`/`kind`, if it has been produced already.
    /// Synchronous, so a view can show a hit without waiting a frame.
    pub fn cached(&self, url: &str, kind: ImageKind) -> Option<PathBuf> {
        if url.is_empty() {
            return None;
        }
        let path = png_path(&self.dir, url, kind);
        if path.is_file() {
            touch(&path);
            Some(path)
        } else {
            None
        }
    }

    /// Produce (or reuse) the sanitized PNG for `url`/`kind`. Concurrent
    /// callers for the same key share one job; at most [`MAX_CONCURRENT`]
    /// jobs run at once.
    pub async fn ensure(&self, url: &str, kind: ImageKind) -> Result<PathBuf, String> {
        if url.is_empty() {
            return Err("empty image url".into());
        }
        let source = classify(url, self.allow_http_loopback)?;
        let path = png_path(&self.dir, url, kind);
        if path.is_file() {
            touch(&path);
            return Ok(path);
        }
        let slot = self
            .inflight
            .lock()
            .unwrap()
            .entry(path.clone())
            .or_default()
            .clone();
        let result = slot
            .get_or_init(|| self.produce(source, kind, path.clone()))
            .await
            .clone();
        self.inflight.lock().unwrap().remove(&path);
        result
    }

    async fn produce(
        &self,
        source: Source,
        kind: ImageKind,
        path: PathBuf,
    ) -> Result<PathBuf, String> {
        let _permit = self.permits.acquire().await.map_err(|e| e.to_string())?;
        if path.is_file() {
            return Ok(path);
        }
        let bytes = match source {
            Source::Remote(url) => (self.fetch)(url).await?,
            Source::Local(file) => tokio::task::spawn_blocking(move || read_local(&file))
                .await
                .map_err(|e| e.to_string())??,
        };
        let out = path.clone();
        tokio::task::spawn_blocking(move || -> Result<(), String> {
            let png = transcode(&bytes, kind)?;
            write_atomic(&out, &png).map_err(|e| format!("cannot write {}: {e}", out.display()))
        })
        .await
        .map_err(|e| e.to_string())??;
        maybe_evict(&self.dir);
        Ok(path)
    }
}

fn read_local(path: &Path) -> Result<Vec<u8>, String> {
    let meta = fs::metadata(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if meta.len() > MAX_BODY_BYTES as u64 {
        return Err(format!("image too large ({} bytes)", meta.len()));
    }
    fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// The process-wide loader over [`default_dir`]. Creating it also removes
/// the unsanitized byte cache of earlier versions.
pub fn loader() -> &'static ImageLoader {
    static LOADER: OnceLock<ImageLoader> = OnceLock::new();
    LOADER.get_or_init(|| {
        let legacy = crate::disk_cache::cache_dir().join("images");
        if legacy.is_dir() {
            std::thread::spawn(move || {
                let _ = fs::remove_dir_all(legacy);
            });
        }
        ImageLoader::new(default_dir())
    })
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
/// `EVICT_TARGET * cap`. Stale `.part` files are always removed.
/// Returns the number of files removed.
pub fn evict(dir: &Path, cap: u64) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut files = vec![];
    let mut total = 0u64;
    let mut removed = 0;
    let stale = SystemTime::now() - Duration::from_secs(3600);
    for e in entries.flatten() {
        let path = e.path();
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if path.extension().is_some_and(|x| x == "part") {
            if modified < stale && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
            continue;
        }
        total += meta.len();
        files.push((modified, meta.len(), path));
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
    use std::sync::atomic::AtomicUsize;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 0, 0, 255]));
        let mut out = Vec::new();
        image::codecs::png::PngEncoder::new(&mut out)
            .write_image(&img, w, h, ExtendedColorType::Rgba8)
            .unwrap();
        out
    }

    fn counting_fetcher(body: Vec<u8>, hits: Arc<AtomicUsize>, delay: Duration) -> Fetcher {
        Arc::new(move |_url| {
            let body = body.clone();
            let hits = hits.clone();
            Box::pin(async move {
                hits.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(delay).await;
                Ok(body)
            })
        })
    }

    #[test]
    fn kinds_parse_and_clamp() {
        assert_eq!(ImageKind::parse("icon:64"), Some(ImageKind::Icon(64)));
        assert_eq!(ImageKind::parse("icon:4096"), Some(ImageKind::Icon(512)));
        assert_eq!(ImageKind::parse("screenshot"), Some(ImageKind::Screenshot));
        assert_eq!(ImageKind::parse("avatar"), Some(ImageKind::Avatar));
        assert_eq!(ImageKind::parse("icon:x"), None);
        let d = Path::new("/c");
        assert_ne!(
            png_path(d, "a", ImageKind::Icon(64)),
            png_path(d, "a", ImageKind::Screenshot)
        );
        assert!(
            png_path(d, "a", ImageKind::Avatar)
                .to_string_lossy()
                .ends_with("-avatar.png")
        );
    }

    #[test]
    fn only_https_and_local_sources() {
        assert!(classify("https://dl.flathub.org/x.png", false).is_ok());
        assert_eq!(
            classify("file:///usr/share/icons/a.png", false),
            Ok(Source::Local("/usr/share/icons/a.png".into()))
        );
        for bad in [
            "http://dl.flathub.org/x.png",
            "image://kiosque/u/abc",
            "qrc:/qml/images/logo.svg",
            "data:image/png;base64,AAAA",
            "ftp://x/y.png",
            "http://127.0.0.1:8080/a.png",
        ] {
            assert!(classify(bad, false).is_err(), "{bad}");
        }
        assert!(classify("http://127.0.0.1:8080/a.png", true).is_ok());
        assert!(classify("http://example.org/a.png", true).is_err());
    }

    #[test]
    fn transcode_resizes_and_emits_png() {
        let out = transcode(&png(1024, 512), ImageKind::Icon(64)).unwrap();
        let img = image::load_from_memory_with_format(&out, ImageFormat::Png).unwrap();
        assert_eq!((img.width(), img.height()), (64, 32));
        let small = transcode(&png(10, 10), ImageKind::Screenshot).unwrap();
        let img = image::load_from_memory(&small).unwrap();
        assert_eq!((img.width(), img.height()), (10, 10));
    }

    #[test]
    fn transcode_rejects_hostile_input() {
        assert!(transcode(b"not an image at all", ImageKind::Avatar).is_err());
        // Truncated PNG: valid signature, broken body.
        let mut truncated = png(32, 32);
        truncated.truncate(40);
        assert!(transcode(&truncated, ImageKind::Avatar).is_err());
        // GIF is decodable in principle but not an accepted format.
        assert!(transcode(b"GIF89a\x01\x00\x01\x00\x00\x00\x00;", ImageKind::Avatar).is_err());
        // SVG text is never handed to a decoder.
        assert!(
            transcode(
                b"<svg xmlns='http://www.w3.org/2000/svg'/>",
                ImageKind::Avatar
            )
            .is_err()
        );
        // A header claiming 65535×65535 must hit the dimension limit before allocating.
        let mut bomb = png(1, 1);
        bomb[16..20].copy_from_slice(&65535u32.to_be_bytes());
        bomb[20..24].copy_from_slice(&65535u32.to_be_bytes());
        assert!(transcode(&bomb, ImageKind::Avatar).is_err());
        assert!(transcode(&vec![0u8; MAX_BODY_BYTES + 1], ImageKind::Avatar).is_err());
    }

    #[tokio::test]
    async fn ensure_dedupes_and_caches() {
        let dir = tempfile::tempdir().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let loader = Arc::new(ImageLoader::with_fetcher(
            dir.path(),
            counting_fetcher(png(300, 300), hits.clone(), Duration::from_millis(50)),
        ));
        let url = "https://fixture.invalid/icon.png";
        assert!(loader.cached(url, ImageKind::Icon(64)).is_none());
        let tasks: Vec<_> = (0..8)
            .map(|_| {
                let loader = loader.clone();
                tokio::spawn(async move { loader.ensure(url, ImageKind::Icon(64)).await })
            })
            .collect();
        let mut paths = vec![];
        for t in tasks {
            paths.push(t.await.unwrap().unwrap());
        }
        assert_eq!(hits.load(Ordering::SeqCst), 1, "one download for one key");
        assert!(paths.iter().all(|p| p == &paths[0]));
        assert!(paths[0].starts_with(dir.path()));
        assert_eq!(paths[0].extension().unwrap(), "png");
        assert_eq!(
            loader.cached(url, ImageKind::Icon(64)),
            Some(paths[0].clone())
        );
        let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().flatten().collect();
        assert_eq!(entries.len(), 1, "no temporary files left behind");
        // A second kind is a second job.
        loader.ensure(url, ImageKind::Screenshot).await.unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn ensure_limits_concurrency() {
        let dir = tempfile::tempdir().unwrap();
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let body = png(8, 8);
        let (a, p) = (active.clone(), peak.clone());
        let fetch: Fetcher = Arc::new(move |_| {
            let (a, p, body) = (a.clone(), p.clone(), body.clone());
            Box::pin(async move {
                let now = a.fetch_add(1, Ordering::SeqCst) + 1;
                p.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(30)).await;
                a.fetch_sub(1, Ordering::SeqCst);
                Ok(body)
            })
        });
        let loader = Arc::new(ImageLoader::with_fetcher(dir.path(), fetch));
        let tasks: Vec<_> = (0..20)
            .map(|i| {
                let loader = loader.clone();
                tokio::spawn(async move {
                    loader
                        .ensure(
                            &format!("https://fixture.invalid/{i}.png"),
                            ImageKind::Avatar,
                        )
                        .await
                })
            })
            .collect();
        for t in tasks {
            t.await.unwrap().unwrap();
        }
        assert!(peak.load(Ordering::SeqCst) <= MAX_CONCURRENT);
    }

    #[tokio::test]
    async fn corrupt_or_refused_sources_leave_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let loader = ImageLoader::with_fetcher(
            dir.path(),
            counting_fetcher(
                b"\x89PNG\r\n\x1a\ngarbage".to_vec(),
                hits.clone(),
                Duration::ZERO,
            ),
        );
        assert!(
            loader
                .ensure("https://fixture.invalid/bad.png", ImageKind::Avatar)
                .await
                .is_err()
        );
        assert!(
            loader
                .ensure("http://insecure.invalid/a.png", ImageKind::Avatar)
                .await
                .is_err()
        );
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "refused scheme never fetched"
        );
        assert_eq!(fs::read_dir(dir.path()).map(|d| d.count()).unwrap_or(0), 0);
        // Failures are not cached: a later attempt runs again.
        let _ = loader
            .ensure("https://fixture.invalid/bad.png", ImageKind::Avatar)
            .await;
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn local_files_are_transcoded() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.png");
        fs::write(&src, png(600, 600)).unwrap();
        let loader = ImageLoader::with_fetcher(
            dir.path().join("out"),
            Arc::new(|_| Box::pin(async { Err("no network in this test".to_string()) })),
        );
        let url = format!("file://{}", src.display());
        let out = loader.ensure(&url, ImageKind::Icon(128)).await.unwrap();
        assert!(out.starts_with(dir.path().join("out")));
        let img = image::open(&out).unwrap();
        assert_eq!(img.width(), 128);
    }

    #[test]
    fn eviction_removes_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        let old = SystemTime::now() - Duration::from_secs(3600 * 3);
        for (i, name) in ["a.png", "b.png", "c.png", "d.png"].iter().enumerate() {
            let p = dir.path().join(name);
            fs::write(&p, vec![0u8; 100]).unwrap();
            fs::File::options()
                .append(true)
                .open(&p)
                .unwrap()
                .set_modified(old + Duration::from_secs(i as u64 * 60))
                .unwrap();
        }
        let part = dir.path().join("e.png.1.0.part");
        fs::write(&part, b"x").unwrap();
        fs::File::options()
            .append(true)
            .open(&part)
            .unwrap()
            .set_modified(old)
            .unwrap();
        // 400 bytes, cap 300 -> trim to 240: drop "a" and "b", plus the stale part file.
        let removed = evict(dir.path(), 300);
        assert_eq!(removed, 3);
        assert!(!dir.path().join("a.png").exists());
        assert!(!dir.path().join("b.png").exists());
        assert!(dir.path().join("c.png").exists());
        assert!(dir.path().join("d.png").exists());
        assert!(!part.exists());
    }
}
