//! Transitional adapter for the C++ `image://kiosque/` provider.
//!
//! Provider ids carry the source URL (`u/<base64url(url)>`). Every image is
//! produced by the sanitizing core pipeline ([`kiosque_core::image_cache`]),
//! so the provider only ever decodes PNGs written by Rust. Removed together
//! with the C++ shell when QML switches to `RemoteImage`.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
pub use kiosque_core::image_cache::*;
use std::ffi::{CStr, c_char};

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

/// Decode the sanitized PNG for a provider id, fitted to the request.
pub async fn fetch_and_process(
    id: &str,
    req_width: i32,
    req_height: i32,
) -> Result<image::RgbaImage, String> {
    let url = url_from_id(id).ok_or_else(|| format!("bad image id {}", id))?;
    let longest = req_width.max(req_height);
    let kind = if longest > 0 && longest <= 512 {
        ImageKind::Icon(longest as u32)
    } else {
        ImageKind::Screenshot
    };
    let path = loader().ensure(&url, kind).await?;
    tokio::task::spawn_blocking(move || -> Result<image::RgbaImage, String> {
        let img = image::open(&path).map_err(|e| format!("decode failed: {e}"))?;
        let (w, h) = (
            if req_width > 0 {
                req_width as u32
            } else {
                u32::MAX
            },
            if req_height > 0 {
                req_height as u32
            } else {
                u32::MAX
            },
        );
        let img = if img.width() > w || img.height() > h {
            img.resize(w, h, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };
        Ok(img.to_rgba8())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_roundtrip() {
        let url =
            "https://dl.flathub.org/media/org/gimp/GIMP/icons/128x128/org.gimp.GIMP.png?x=1&y=/2";
        let q = image_url(url);
        let id = q.strip_prefix("image://kiosque/").unwrap();
        assert!(!id[2..].contains('/'), "id must stay a single path segment");
        assert_eq!(url_from_id(id).unwrap(), url);
        assert_eq!(image_url(""), "");
        assert!(url_from_id("app/icon").is_none());
    }
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

// ── C FFI ───────────────────────────────────────────────────────────────────

/// Load the image for a provider id (`u/<base64url(url)>`), downscaled to fit
/// `req_width`×`req_height` when those are positive. Blocks the calling
/// thread; the C++ async image provider calls it from its own thread pool.
/// The result must be released with [`free_rust_image`].
///
/// # Safety
/// `id` must be a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fetch_image_from_rust(
    id: *const c_char,
    req_width: i32,
    req_height: i32,
) -> RustImageResult {
    if id.is_null() {
        return RustImageResult::default();
    }
    let id = unsafe { CStr::from_ptr(id) }.to_string_lossy().into_owned();
    match crate::runtime::runtime().block_on(fetch_and_process(&id, req_width, req_height)) {
        Ok(img) => {
            let (width, height) = img.dimensions();
            let bytes = img.into_raw().into_boxed_slice();
            let Ok(len) = i32::try_from(bytes.len()) else {
                return RustImageResult::default();
            };
            RustImageResult {
                data: Box::into_raw(bytes) as *mut u8,
                len,
                width: width as i32,
                height: height as i32,
            }
        }
        Err(e) => {
            kerr!("image {}: {}", url_from_id(&id).unwrap_or(id), e);
            RustImageResult::default()
        }
    }
}

/// Free a buffer returned by [`fetch_image_from_rust`].
///
/// # Safety
/// `res` must come from `fetch_image_from_rust` and be freed only once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_rust_image(res: RustImageResult) {
    if !res.data.is_null() && res.len > 0 {
        unsafe {
            let slice = std::slice::from_raw_parts_mut(res.data, res.len as usize);
            drop(Box::from_raw(slice as *mut [u8]));
        }
    }
}
