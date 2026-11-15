//! Serializable view data; independent of Qt types.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppEntry {
    pub name: String,
    pub summary: String,
    pub icon_url: String,
    pub app_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FeaturedEntry {
    pub name: String,
    pub summary: String,
    pub icon_url: String,
    pub app_id: String,
    pub description: String,
    pub developer: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InstalledEntry {
    pub name: String,
    pub app_id: String,
    pub version: String,
    pub size: String,
    pub origin: String,
    pub has_update: bool,
    pub is_checked_for_update: bool,
    pub is_runtime: bool,
}
