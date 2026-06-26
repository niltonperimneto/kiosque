use std::fs;
use std::io::Cursor;
use std::os::raw::c_char;
use std::ffi::CStr;
use std::path::PathBuf;
use image::{DynamicImage, ImageReader};
use jxl_oxide::integration::JxlDecoder;
use jxl_encoder::{LossyConfig, PixelLayout};

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
    
    Ok(img)
}

// Load and decode a JXL image from the disk cache.
fn load_jxl_from_cache(cache_path: &PathBuf) -> Result<DynamicImage, String> {
    let jxl_bytes = fs::read(cache_path)
        .map_err(|e| format!("Failed to read cached JXL: {}", e))?;
        
    let decoder = JxlDecoder::new(Cursor::new(jxl_bytes))
        .map_err(|e| format!("Failed to initialize JXL decoder: {}", e))?;
        
    let img = DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("Failed to decode JXL image: {}", e))?;
        
    Ok(img)
}

// Internal implementation of image loading, resizing, and preparation.
async fn fetch_and_process_image_impl(
    app_id: &str,
    img_type: &str,
    index: i32,
    req_width: i32,
    req_height: i32,
) -> Result<RustImageResult, String> {
    let cache_path = cache_file_path(app_id, img_type, index);
    
    // 1. Get or download the original image
    let img = if cache_path.exists() {
        match load_jxl_from_cache(&cache_path) {
            Ok(cached_img) => cached_img,
            Err(err) => {
                eprintln!("[kiosque] Cached JXL decoding failed (error: {}), re-downloading...", err);
                let key = format!("{}:{}:{}", app_id, img_type, index);
                let fallback_key = format!("{}:{}", app_id, img_type);
                let url = crate::image_registry::get_image_url(&key)
                    .or_else(|| crate::image_registry::get_image_url(&fallback_key))
                    .ok_or_else(|| format!("No remote URL registered for key: {}", key))?;
                download_and_cache_jxl(&url, cache_path).await?
            }
        }
    } else {
        let key = format!("{}:{}:{}", app_id, img_type, index);
        let fallback_key = format!("{}:{}", app_id, img_type);
        let url = crate::image_registry::get_image_url(&key)
            .or_else(|| crate::image_registry::get_image_url(&fallback_key))
            .ok_or_else(|| format!("No remote URL registered for key: {}", key))?;
        download_and_cache_jxl(&url, cache_path).await?
    };
    
    // 2. Perform downscaling if requested and the image is larger than requested
    let processed_img = if req_width > 0 && req_height > 0 {
        let orig_w = img.width() as i32;
        let orig_h = img.height() as i32;
        
        if orig_w > req_width || orig_h > req_height {
            // Keep aspect ratio when resizing
            img.resize(
                req_width as u32,
                req_height as u32,
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            img
        }
    } else {
        img
    };
    
    // 3. Extract raw RGBA8 pixels and compile them into a heap-allocated box
    let rgba_img = processed_img.to_rgba8();
    let (w, h) = rgba_img.dimensions();
    let raw_pixels = rgba_img.into_raw();
    
    // Turn the raw pixels Vec into a boxed slice. This guarantees capacity == length.
    let boxed_slice = raw_pixels.into_boxed_slice();
    let len = boxed_slice.len() as i32;
    let data = Box::into_raw(boxed_slice) as *mut u8;
    
    Ok(RustImageResult {
        data,
        len,
        width: w as i32,
        height: h as i32,
    })
}

// ── C FFI Exports ────────────────────────────────────────────────────────────

/// Query, download, cache, and resize an image from the Rust backend.
/// Returns a `RustImageResult` struct which contains the heap-allocated pixels.
/// The caller MUST eventually call `free_rust_image` on this struct to prevent leaks.
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
