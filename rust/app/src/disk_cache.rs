//! Popular and featured collection caches.
use kiosque_core::disk_cache::now_secs;
use kiosque_core::entries::{AppEntry, FEATURED_CACHE_SCHEMA, FeaturedEntry, POPULAR_CACHE_SCHEMA};

pub async fn load_popular_cache() -> Option<(u64, Vec<AppEntry>)> {
    crate::services::services()
        .disk
        .load("popular", POPULAR_CACHE_SCHEMA)
        .await
}
pub async fn save_popular_cache(items: &[AppEntry]) -> Result<(), String> {
    crate::services::services()
        .disk
        .save_at("popular", POPULAR_CACHE_SCHEMA, now_secs(), &items)
        .await
}
pub async fn load_featured_cache() -> Option<(u64, Vec<FeaturedEntry>)> {
    crate::services::services()
        .disk
        .load("featured", FEATURED_CACHE_SCHEMA)
        .await
}
pub async fn save_featured_cache(items: &[FeaturedEntry]) -> Result<(), String> {
    crate::services::services()
        .disk
        .save_at("featured", FEATURED_CACHE_SCHEMA, now_secs(), &items)
        .await
}
