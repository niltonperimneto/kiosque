use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::OnceLock;

static REGISTRY: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();

fn registry() -> &'static RwLock<HashMap<String, String>> {
    REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Register a mapping from image identifier (e.g., "org.gimp.GIMP:icon") to remote URL.
pub fn register_image_url(key: String, url: String) {
    if url.is_empty() {
        return;
    }
    if let Ok(mut map) = registry().write() {
        map.insert(key, url);
    }
}

/// Retrieve the remote URL mapped to an image identifier.
pub fn get_image_url(key: &str) -> Option<String> {
    registry().read().ok().and_then(|map| map.get(key).cloned())
}
