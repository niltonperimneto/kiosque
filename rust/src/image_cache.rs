pub use kiosque_core::image_cache::*;
use std::ffi::{CStr, c_char};

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
    match crate::runtime::runtime().block_on(
        crate::services::services()
            .images
            .load(&id, req_width, req_height),
    ) {
        Ok(img) => {
            let (width, height) = img.dimensions();
            let bytes = img.as_raw().clone().into_boxed_slice();
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
