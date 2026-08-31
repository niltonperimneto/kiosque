use std::fs;
use std::io::Cursor;
use std::os::raw::c_char;
use std::ffi::CStr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;
use moka::future::Cache;
use image::{DynamicImage, ImageReader};
use jxl_oxide::integration::JxlDecoder;
use jxl_encoder::{LossyConfig, PixelLayout};

// A fully decoded and resized RGBA8 frame, ready to be copied across the FFI
// boundary. Shared via `Arc` so the in-memory cache can hold it cheaply.
struct DecodedImage {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

// In-memory LRU of decoded+resized frames, keyed by request parameters.
// Bounded by total byte weight so a handful of large screenshots cannot grow
// memory without limit. Hits skip all disk I/O, JXL decoding, and resizing.
static MEM_CACHE: OnceLock<Cache<String, Arc<DecodedImage>>> = OnceLock::new();

fn mem_cache() -> &'static Cache<String, Arc<DecodedImage>> {
    MEM_CACHE.get_or_init(|| {
        Cache::builder()
            // 96 MiB of decoded pixels; weigher below measures each entry.
            .max_capacity(96 * 1024 * 1024)
            .weigher(|_k, v: &Arc<DecodedImage>| v.data.len().min(u32::MAX as usize) as u32)
            .time_to_live(Duration::from_secs(600))
            .build()
    })
}

#[repr(C)]
pub struct RustImageResult {
    pub data: *mut u8,
    pub len: i32,
    pub width: i32,
    pub height: i32,
}

impl Default for RustImageResult {
    fn default() -> Self {
        Self {
            data: std::ptr::null_mut(),
            len: 0,
            width: 0,
            height: 0,
        }
    }
}

// Ensure the image cache directory exists.
fn image_cache_dir() -> PathBuf {
    let dir = crate::disk_cache::cache_dir().join("images");
    fs::create_dir_all(&dir).ok();
    dir
}

// Generate the JXL cache file path for an app ID, image type, and index.
fn cache_file_path(app_id: &str, img_type: &str, index: i32) -> PathBuf {
    // Sanitize app_id to prevent path traversal
    let safe_app_id = app_id.replace(|c: char| !c.is_alphanumeric() && c != '.' && c != '-', "_");
    let safe_img_type = img_type.replace(|c: char| !c.is_alphanumeric(), "_");
    let filename = format!("{}_{}_{}.jxl", safe_app_id, safe_img_type, index);
    image_cache_dir().join(filename)
}

// Download image, decode PNG/JPEG, encode to JXL, save cache, and return the image.
async fn download_and_cache_jxl(
    url: &str,
    cache_path: PathBuf,
) -> Result<DynamicImage, String> {
    eprintln!("[kiosque] Image cache miss. Downloading from: {}", url);
    
    // Download image bytes using the shared reqwest client
    let client = crate::flathub::client::shared_client();
    let resp = client.get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to download image: {}", e))?;
        
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP error downloading image: {}", status.as_u16()));
    }
    
    let bytes = resp.bytes()
        .await
        .map_err(|e| format!("Failed to read image bytes: {}", e))?;
        
    // Decode downloaded PNG/JPEG using the image crate
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("Failed to guess image format: {}", e))?;
        
    let img = reader.decode()
        .map_err(|e| format!("Failed to decode image: {}", e))?;
        
    // Convert to RGBA8 for encoding
    let rgba_img = img.to_rgba8();
    let (width, height) = rgba_img.dimensions();
    
    // Encode to JPEG-XL
    eprintln!("[kiosque] Re-encoding downloaded image to JXL: {}x{}", width, height);
    let jxl_bytes = LossyConfig::new(1.0)
        .encode(&rgba_img, width, height, PixelLayout::Rgba8)
        .map_err(|e| format!("Failed to encode JXL: {}", e))?;
        
    // Save to disk cache
    if let Err(e) = fs::write(&cache_path, jxl_bytes) {
        eprintln!("[kiosque] WARNING: Failed to write JXL cache file: {}", e);
    }

    // Reuse the already-allocated RGBA buffer instead of returning the separate
    // `img` (which would keep a second full-resolution buffer alive and force a
    // redundant RGBA conversion downstream).
    Ok(DynamicImage::ImageRgba8(rgba_img))
}

// Decode JXL bytes into a DynamicImage.
fn decode_jxl(jxl_bytes: Vec<u8>) -> Result<DynamicImage, String> {
    let decoder = JxlDecoder::new(Cursor::new(jxl_bytes))
        .map_err(|e| format!("Failed to initialize JXL decoder: {}", e))?;

    DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("Failed to decode JXL image: {}", e))
}

// Load and decode a JXL image from the disk cache. Returns NotFound as an error.
fn load_jxl_from_cache(cache_path: &PathBuf) -> Result<DynamicImage, String> {
    let jxl_bytes = fs::read(cache_path)
        .map_err(|e| format!("Failed to read cached JXL: {}", e))?;
    decode_jxl(jxl_bytes)
}

static IN_FLIGHT: OnceLock<Mutex<HashMap<PathBuf, Arc<TokioMutex<()>>>>> = OnceLock::new();

fn in_flight_locks() -> &'static Mutex<HashMap<PathBuf, Arc<TokioMutex<()>>>> {
    IN_FLIGHT.get_or_init(|| Mutex::new(HashMap::new()))
}

async fn download_under_lock(
    url: String,
    cache_path: PathBuf,
) -> Result<DynamicImage, String> {
    let lock = {
        let mut map = in_flight_locks().lock().unwrap();
        map.entry(cache_path.clone()).or_insert_with(|| Arc::new(TokioMutex::new(()))).clone()
    };

    let _guard = lock.lock().await;

    // Check again if it was written while we waited (single read; missing file
    // returns an Err and falls through to the download).
    let result = match load_jxl_from_cache(&cache_path) {
        Ok(img) => Ok(img),
        Err(_) => download_and_cache_jxl(&url, cache_path.clone()).await,
    };

    // Drop the per-path entry from the map once no other task is waiting on it,
    // so the map does not grow unbounded over the process lifetime. strong_count
    // of 2 means only the map and our local `lock` hold a reference.
    {
        let mut map = in_flight_locks().lock().unwrap();
        if Arc::strong_count(&lock) <= 2 {
            map.remove(&cache_path);
        }
    }

    result
}

// Resolve the remote URL registered for an image, trying the indexed key first
// then the type-only fallback key.
fn registered_url(app_id: &str, img_type: &str, index: i32) -> Result<String, String> {
    let key = format!("{}:{}:{}", app_id, img_type, index);
    let fallback_key = format!("{}:{}", app_id, img_type);
    crate::image_registry::get_image_url(&key)
        .or_else(|| crate::image_registry::get_image_url(&fallback_key))
        .ok_or_else(|| format!("No remote URL registered for key: {}", key))
}

// Decode (from disk or network) and resize an image, returning a shared,
// cacheable RGBA8 frame. The in-memory cache short-circuits repeat requests for
// the same parameters, avoiding all disk I/O, JXL decoding, and resizing.
async fn get_decoded_image(
    app_id: &str,
    img_type: &str,
    index: i32,
    req_width: i32,
    req_height: i32,
) -> Result<Arc<DecodedImage>, String> {
    let mem_key = format!("{}/{}/{}/{}x{}", app_id, img_type, index, req_width, req_height);
    if let Some(cached) = mem_cache().get(&mem_key).await {
        return Ok(cached);
    }

    let cache_path = cache_file_path(app_id, img_type, index);

    // 1. Get or download the original image (single read; missing/corrupt cache
    //    falls through to a download).
    let img = match load_jxl_from_cache(&cache_path) {
        Ok(cached_img) => cached_img,
        Err(_) => {
            let url = registered_url(app_id, img_type, index)?;
            download_under_lock(url, cache_path).await?
        }
    };

    // 2. Determine target dimensions (aspect-preserving when only one axis is
    //    given), then extract RGBA8 pixels consuming the decoded buffer.
    let orig_w = img.width();
    let orig_h = img.height();
    let rgba_img = img.into_rgba8();

    let (target_w, target_h) = match (req_width, req_height) {
        (w, h) if w > 0 && h > 0 => {
            // Fit within the requested box, preserving aspect ratio. The C++
            // provider validates against the dimensions we return, so the
            // target need not match the request exactly.
            let ratio = (w as f32 / orig_w as f32).min(h as f32 / orig_h as f32);
            (
                (orig_w as f32 * ratio).round().max(1.0) as u32,
                (orig_h as f32 * ratio).round().max(1.0) as u32,
            )
        }
        (w, _) if w > 0 => {
            let ratio = w as f32 / orig_w as f32;
            (w as u32, (orig_h as f32 * ratio).round().max(1.0) as u32)
        }
        (_, h) if h > 0 => {
            let ratio = h as f32 / orig_h as f32;
            ((orig_w as f32 * ratio).round().max(1.0) as u32, h as u32)
        }
        _ => (orig_w, orig_h),
    };

    // 3. Downscale only when the source is larger than the target. SIMD
    //    convolution (AVX2/SSE4/NEON) makes this far cheaper than the scalar
    //    `image` crate resize.
    let (data, width, height) = if orig_w > target_w || orig_h > target_h {
        resize_rgba(rgba_img, target_w, target_h)?
    } else {
        (rgba_img.into_raw(), orig_w, orig_h)
    };

    let decoded = Arc::new(DecodedImage { data, width, height });
    mem_cache().insert(mem_key, decoded.clone()).await;
    Ok(decoded)
}

// SIMD-accelerated RGBA8 downscale via `fast_image_resize`. CatmullRom is a
// high-quality cubic filter; alpha is filtered straight (matching the previous
// `image` crate behaviour) rather than premultiplied.
fn resize_rgba(
    src: image::RgbaImage,
    target_w: u32,
    target_h: u32,
) -> Result<(Vec<u8>, u32, u32), String> {
    use fast_image_resize as fir;

    let (sw, sh) = src.dimensions();
    let src_image = fir::images::Image::from_vec_u8(sw, sh, src.into_raw(), fir::PixelType::U8x4)
        .map_err(|e| format!("Failed to wrap source image for resize: {}", e))?;

    let mut dst_image = fir::images::Image::new(target_w, target_h, fir::PixelType::U8x4);
    let opts = fir::ResizeOptions::new()
        .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::CatmullRom));

    fir::Resizer::new()
        .resize(&src_image, &mut dst_image, &opts)
        .map_err(|e| format!("Failed to resize image: {}", e))?;

    Ok((dst_image.into_vec(), target_w, target_h))
}

// Internal implementation of image loading, resizing, and preparation. Produces
// a freshly heap-allocated RGBA8 buffer owned by the caller across the FFI.
async fn fetch_and_process_image_impl(
    app_id: &str,
    img_type: &str,
    index: i32,
    req_width: i32,
    req_height: i32,
) -> Result<RustImageResult, String> {
    let decoded = get_decoded_image(app_id, img_type, index, req_width, req_height).await?;

    // Copy the cached pixels into a boxed slice owned by the caller (guarantees
    // capacity == length so `free_rust_image` can reconstruct it exactly).
    let boxed_slice: Box<[u8]> = decoded.data.as_slice().into();
    let len = boxed_slice.len() as i32;
    let data = Box::into_raw(boxed_slice) as *mut u8;

    Ok(RustImageResult {
        data,
        len,
        width: decoded.width as i32,
        height: decoded.height as i32,
    })
}

// ── C FFI Exports ────────────────────────────────────────────────────────────

/// Query, download, cache, and resize an image from the Rust backend.
/// Returns a `RustImageResult` struct which contains the heap-allocated pixels.
/// The caller MUST eventually call `free_rust_image` on this struct to prevent leaks.
///
/// # Safety
/// `app_id` and `img_type` must be non-null pointers to valid, NUL-terminated C
/// strings that remain valid for the duration of the call. The returned
/// `RustImageResult` owns a heap allocation that must be released exactly once
/// with [`free_rust_image`]; it must not be freed by any other allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fetch_image_from_rust(
    app_id: *const c_char,
    img_type: *const c_char,
    index: i32,
    req_width: i32,
    req_height: i32,
) -> RustImageResult {
    let app_id_str = unsafe { CStr::from_ptr(app_id).to_str().unwrap_or("") };
    let img_type_str = unsafe { CStr::from_ptr(img_type).to_str().unwrap_or("") };
    
    // We run the async fetch on the global tokio runtime blocking pool, blocking this thread
    // until it's ready. Since QQuickImageProvider works asynchronously on its own Qt threadpool,
    // blocking here is perfectly fine and does not stall the main GUI thread.
    let runtime = crate::runtime::runtime();
    let result = runtime.block_on(async {
        fetch_and_process_image_impl(app_id_str, img_type_str, index, req_width, req_height).await
    });
    
    match result {
        Ok(res) => res,
        Err(e) => {
            eprintln!("[kiosque] ERROR: Image fetching failed for {}/{}: {}", app_id_str, img_type_str, e);
            RustImageResult::default()
        }
    }
}

/// Frees a heap-allocated image buffer returned by `fetch_image_from_rust`.
///
/// # Safety
/// `res` must be a `RustImageResult` previously returned by
/// [`fetch_image_from_rust`] and not already freed. Its `data`/`len` fields must
/// be unmodified so the original allocation can be reconstructed exactly. Passing
/// it more than once is a double-free; a default (null) result is safe and ignored.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_rust_image(res: RustImageResult) {
    if !res.data.is_null() && res.len > 0 {
        // Reconstruct the boxed slice from the raw pointer to trigger normal drop/free.
        unsafe {
            let slice = std::slice::from_raw_parts_mut(res.data, res.len as usize);
            let _ = Box::from_raw(slice as *mut [u8]);
        }
    }
}
