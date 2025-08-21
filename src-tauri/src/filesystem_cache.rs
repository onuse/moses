// Filesystem detection cache to avoid redundant analysis
use std::collections::HashMap;
use std::sync::RwLock;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFilesystemInfo {
    pub filesystem: String,
    pub partition_table: Option<String>,
    pub partitions: Vec<PartitionInfo>,
    pub detected_at: std::time::SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionInfo {
    pub number: u32,
    pub filesystem: Option<String>,
    pub size: u64,
    pub start_offset: u64,
}

// Global cache for filesystem detection results
pub static FILESYSTEM_CACHE: Lazy<RwLock<HashMap<String, CachedFilesystemInfo>>> = 
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Store filesystem analysis results in cache
pub fn cache_filesystem_info(device_id: &str, info: CachedFilesystemInfo) {
    log::info!("Caching filesystem info for device {}: {:?}", device_id, info.filesystem);
    
    if let Ok(mut cache) = FILESYSTEM_CACHE.write() {
        cache.insert(device_id.to_string(), info);
    }
}

/// Get cached filesystem info for a device
pub fn get_cached_filesystem_info(device_id: &str) -> Option<CachedFilesystemInfo> {
    if let Ok(cache) = FILESYSTEM_CACHE.read() {
        cache.get(device_id).cloned()
    } else {
        None
    }
}

/// Clear cached info for a specific device (e.g., after formatting)
#[allow(dead_code)] // Will be used when format operations are hooked up
pub fn invalidate_device_cache(device_id: &str) {
    log::info!("Invalidating filesystem cache for device {}", device_id);
    
    if let Ok(mut cache) = FILESYSTEM_CACHE.write() {
        cache.remove(device_id);
    }
}

/// Clear all cached filesystem info
#[allow(dead_code)] // Will be used for cache management UI
pub fn clear_filesystem_cache() {
    log::info!("Clearing all filesystem cache");
    
    if let Ok(mut cache) = FILESYSTEM_CACHE.write() {
        cache.clear();
    }
}

/// Check if cached info is still fresh (within 5 minutes)
pub fn is_cache_fresh(info: &CachedFilesystemInfo) -> bool {
    if let Ok(elapsed) = info.detected_at.elapsed() {
        // Consider cache fresh if less than 5 minutes old
        elapsed.as_secs() < 300
    } else {
        false
    }
}