//! Compatibility adapters for serialized view data.
pub use kiosque_core::disk_cache::*;
use kiosque_core::entries::{AppEntry, FeaturedEntry, InstalledEntry};

pub async fn load_popular_cache() -> Option<(u64, Vec<AppEntry>)> {
    crate::services::services().disk.load("popular", 1).await
}
pub async fn save_popular_cache(items: &[AppEntry]) -> Result<(), String> {
    crate::services::services()
        .disk
        .save_at("popular", 1, now_secs(), &items)
        .await
}
pub async fn load_featured_cache() -> Option<(u64, Vec<FeaturedEntry>)> {
    crate::services::services().disk.load("featured", 2).await
}
pub async fn save_featured_cache(items: &[FeaturedEntry]) -> Result<(), String> {
    crate::services::services()
        .disk
        .save_at("featured", 2, now_secs(), &items)
        .await
}
pub async fn load_installed_cache() -> Option<(u64, Vec<InstalledEntry>)> {
    crate::services::services().disk.load("installed", 1).await
}
pub async fn save_installed_cache(items: &[InstalledEntry]) -> Result<(), String> {
    crate::services::services()
        .disk
        .save_at("installed", 1, now_secs(), &items)
        .await
}
pub async fn invalidate_installed_cache() {
    crate::services::services().disk.remove("installed").await;
}
