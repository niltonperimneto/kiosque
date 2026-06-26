use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bridge::featured_model::FeaturedEntry;
use crate::bridge::app_list_model::AppEntry;
use crate::bridge::installed_model::InstalledEntry;

// ── Cache Dir Helper ────────────────────────────────────────────────────────

pub fn cache_dir() -> PathBuf {
    let mut path = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("~/.cache"));
    path.push("kiosque");
    fs::create_dir_all(&path).ok();
    path
}

// ── Custom Binary Serialization Primitives ──────────────────────────────────

fn write_u32(bytes: &mut Vec<u8>, val: u32) {
    bytes.extend_from_slice(&val.to_le_bytes());
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Option<u32> {
    if *cursor + 4 > bytes.len() {
        return None;
    }
    let val = u32::from_le_bytes(bytes[*cursor..*cursor + 4].try_into().ok()?);
    *cursor += 4;
    Some(val)
}

fn write_u64(bytes: &mut Vec<u8>, val: u64) {
    bytes.extend_from_slice(&val.to_le_bytes());
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
    if *cursor + 8 > bytes.len() {
        return None;
    }
    let val = u64::from_le_bytes(bytes[*cursor..*cursor + 8].try_into().ok()?);
    *cursor += 8;
    Some(val)
}

fn write_string(bytes: &mut Vec<u8>, s: &str) {
    write_u32(bytes, s.len() as u32);
    bytes.extend_from_slice(s.as_bytes());
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Option<String> {
    let len = read_u32(bytes, cursor)? as usize;
    if *cursor + len > bytes.len() {
        return None;
    }
    let s = std::str::from_utf8(&bytes[*cursor..*cursor + len]).ok()?.to_string();
    *cursor += len;
    Some(s)
}

// ── Featured Cache ──────────────────────────────────────────────────────────

pub async fn save_featured_cache(items: &[FeaturedEntry]) -> Result<(), String> {
    let mut bytes = Vec::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    write_u64(&mut bytes, now);
    write_u32(&mut bytes, items.len() as u32);
    
    for item in items {
        write_string(&mut bytes, &item.name);
        write_string(&mut bytes, &item.summary);
        write_string(&mut bytes, &item.icon_url);
        write_string(&mut bytes, &item.app_id);
        write_string(&mut bytes, &item.description);
        write_string(&mut bytes, &item.developer);
    }
    
    let path = cache_dir().join("featured.bin");
    tokio::fs::write(&path, bytes).await.map_err(|e| format!("Failed to write featured cache: {}", e))?;
    Ok(())
}

pub async fn load_featured_cache() -> Option<(u64, Vec<FeaturedEntry>)> {
    let path = cache_dir().join("featured.bin");
    let bytes = tokio::fs::read(&path).await.ok()?;
    let mut cursor = 0;
    
    let timestamp = read_u64(&bytes, &mut cursor)?;
    let len = read_u32(&bytes, &mut cursor)? as usize;
    let mut items = Vec::with_capacity(len);
    
    for _ in 0..len {
        items.push(FeaturedEntry {
            name: read_string(&bytes, &mut cursor)?,
            summary: read_string(&bytes, &mut cursor)?,
            icon_url: read_string(&bytes, &mut cursor)?,
            app_id: read_string(&bytes, &mut cursor)?,
            description: read_string(&bytes, &mut cursor)?,
            developer: read_string(&bytes, &mut cursor)?,
        });
    }
    
    Some((timestamp, items))
}

// ── Popular Cache ───────────────────────────────────────────────────────────

pub async fn save_popular_cache(items: &[AppEntry]) -> Result<(), String> {
    let mut bytes = Vec::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    write_u64(&mut bytes, now);
    write_u32(&mut bytes, items.len() as u32);
    
    for item in items {
        write_string(&mut bytes, &item.name);
        write_string(&mut bytes, &item.summary);
        write_string(&mut bytes, &item.icon_url);
        write_string(&mut bytes, &item.app_id);
    }
    
    let path = cache_dir().join("popular.bin");
    tokio::fs::write(&path, bytes).await.map_err(|e| format!("Failed to write popular cache: {}", e))?;
    Ok(())
}

pub async fn load_popular_cache() -> Option<(u64, Vec<AppEntry>)> {
    let path = cache_dir().join("popular.bin");
    let bytes = tokio::fs::read(&path).await.ok()?;
    let mut cursor = 0;
    
    let timestamp = read_u64(&bytes, &mut cursor)?;
    let len = read_u32(&bytes, &mut cursor)? as usize;
    let mut items = Vec::with_capacity(len);
    
    for _ in 0..len {
        items.push(AppEntry {
            name: read_string(&bytes, &mut cursor)?,
            summary: read_string(&bytes, &mut cursor)?,
            icon_url: read_string(&bytes, &mut cursor)?,
            app_id: read_string(&bytes, &mut cursor)?,
        });
    }
    
    Some((timestamp, items))
}

// ── Installed Cache ─────────────────────────────────────────────────────────

pub async fn save_installed_cache(items: &[InstalledEntry]) -> Result<(), String> {
    let mut bytes = Vec::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    write_u64(&mut bytes, now);
    write_u32(&mut bytes, items.len() as u32);
    
    for item in items {
        write_string(&mut bytes, &item.name);
        write_string(&mut bytes, &item.app_id);
        write_string(&mut bytes, &item.version);
        write_string(&mut bytes, &item.size);
        write_string(&mut bytes, &item.origin);
        bytes.push(item.has_update as u8);
        bytes.push(item.is_checked_for_update as u8);
        bytes.push(item.is_runtime as u8);
    }
    
    let path = cache_dir().join("installed.bin");
    tokio::fs::write(&path, bytes).await.map_err(|e| format!("Failed to write installed cache: {}", e))?;
    Ok(())
}

pub async fn load_installed_cache() -> Option<(u64, Vec<InstalledEntry>)> {
    let path = cache_dir().join("installed.bin");
    let bytes = tokio::fs::read(&path).await.ok()?;
    let mut cursor = 0;
    
    let timestamp = read_u64(&bytes, &mut cursor)?;
    let len = read_u32(&bytes, &mut cursor)? as usize;
    let mut items = Vec::with_capacity(len);
    
    for _ in 0..len {
        let name = read_string(&bytes, &mut cursor)?;
        let app_id = read_string(&bytes, &mut cursor)?;
        let version = read_string(&bytes, &mut cursor)?;
        let size = read_string(&bytes, &mut cursor)?;
        let origin = read_string(&bytes, &mut cursor)?;
        
        if cursor + 3 > bytes.len() {
            return None;
        }
        let has_update = bytes[cursor] != 0;
        let is_checked_for_update = bytes[cursor + 1] != 0;
        let is_runtime = bytes[cursor + 2] != 0;
        cursor += 3;
        
        items.push(InstalledEntry {
            name,
            app_id,
            version,
            size,
            origin,
            has_update,
            is_checked_for_update,
            is_runtime,
        });
    }
    
    Some((timestamp, items))
}

pub async fn invalidate_installed_cache() {
    let path = cache_dir().join("installed.bin");
    let _ = tokio::fs::remove_file(path).await;
}
