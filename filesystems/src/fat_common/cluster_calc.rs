// Cluster size and FAT size calculation for FAT filesystems
// Ensures correct cluster counts for FAT16 vs FAT32

use moses_core::MosesError;
use super::constants::*;

/// Parameters calculated for FAT filesystem
#[derive(Debug, Clone)]
pub struct FatParams {
    pub sectors_per_cluster: u8,
    pub sectors_per_fat: u32,
    pub root_entries: u16,  // 0 for FAT32
    pub total_clusters: u32,
    pub fat_type: FatType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FatType {
    Fat16,
    Fat32,
}

/// Calculate FAT16 parameters
/// Ensures cluster count is between 4085 and 65524
pub fn calculate_fat16_params(total_sectors: u64) -> Result<FatParams, MosesError> {
    // Microsoft's recommended cluster sizes for FAT16
    let sectors_per_cluster = if total_sectors <= 32_680 {
        2   // 1KB clusters for <= 16MB
    } else if total_sectors <= 262_144 {
        4   // 2KB clusters for <= 128MB
    } else if total_sectors <= 524_288 {
        8   // 4KB clusters for <= 256MB
    } else if total_sectors <= 1_048_576 {
        16  // 8KB clusters for <= 512MB
    } else if total_sectors <= 2_097_152 {
        32  // 16KB clusters for <= 1GB
    } else if total_sectors <= 4_194_304 {
        64  // 32KB clusters for <= 2GB
    } else if total_sectors <= 8_388_608 {
        128 // 64KB clusters for <= 4GB (maximum for FAT16)
    } else {
        return Err(MosesError::Other("Volume too large for FAT16 (max 4GB)".to_string()));
    };
    
    let root_entries = 512u16;  // Standard for FAT16
    let reserved_sectors = 1u16;
    
    // Calculate data area
    let root_dir_sectors = (root_entries * 32 + 511) / 512;
    
    // Initial estimate without FAT size
    let data_start_estimate = reserved_sectors + root_dir_sectors;
    let usable_sectors = total_sectors.saturating_sub(data_start_estimate as u64);
    let total_clusters = usable_sectors / sectors_per_cluster as u64;
    
    // Validate cluster count for FAT16
    if total_clusters < FAT16_MIN_CLUSTERS as u64 {
        return Err(MosesError::Other(format!(
            "Volume too small for FAT16 (only {} clusters, need at least {})",
            total_clusters, FAT16_MIN_CLUSTERS
        )));
    }
    if total_clusters > FAT16_MAX_CLUSTERS as u64 {
        return Err(MosesError::Other(format!(
            "Too many clusters for FAT16 ({}, max {})",
            total_clusters, FAT16_MAX_CLUSTERS
        )));
    }
    
    // Calculate FAT size (2 bytes per cluster)
    let fat_entries = total_clusters + 2;  // +2 for reserved entries
    let fat_bytes = fat_entries * 2;
    let sectors_per_fat = ((fat_bytes + 511) / 512) as u32;
    
    // Recalculate with actual FAT size
    let data_start = reserved_sectors as u32 + (2 * sectors_per_fat) + root_dir_sectors as u32;
    let data_sectors = total_sectors as u32 - data_start;
    let final_clusters = data_sectors / sectors_per_cluster as u32;
    
    // Final validation
    if final_clusters < FAT16_MIN_CLUSTERS || final_clusters > FAT16_MAX_CLUSTERS {
        return Err(MosesError::Other(format!(
            "Invalid cluster count after FAT calculation: {}",
            final_clusters
        )));
    }
    
    Ok(FatParams {
        sectors_per_cluster,
        sectors_per_fat,
        root_entries,
        total_clusters: final_clusters,
        fat_type: FatType::Fat16,
    })
}

/// Calculate FAT32 parameters
/// Ensures cluster count is >= 65525
pub fn calculate_fat32_params(total_sectors: u64) -> Result<FatParams, MosesError> {
    // For FAT32, we need at least 65525 clusters
    // Start with smaller cluster sizes to maximize cluster count
    let mut sectors_per_cluster = if total_sectors <= 532_480 {
        1   // 512B clusters for <= 260MB (minimum to get 65525 clusters)
    } else if total_sectors <= 16_777_216 {
        8   // 4KB clusters for <= 8GB
    } else if total_sectors <= 33_554_432 {
        16  // 8KB clusters for <= 16GB
    } else if total_sectors <= 67_108_864 {
        32  // 16KB clusters for <= 32GB
    } else if total_sectors <= 0xFFFFFFFF {
        64  // 32KB clusters for <= 2TB
    } else {
        128 // 64KB clusters for > 2TB
    };
    
    // FAT32 typically uses 32 reserved sectors
    let reserved_sectors = 32u16;
    
    // Calculate clusters
    let mut total_clusters = total_sectors / sectors_per_cluster as u64;
    
    // Adjust cluster size if we have too few clusters
    while total_clusters < FAT32_MIN_CLUSTERS as u64 && sectors_per_cluster > 1 {
        sectors_per_cluster /= 2;
        total_clusters = total_sectors / sectors_per_cluster as u64;
    }
    
    if total_clusters < FAT32_MIN_CLUSTERS as u64 {
        return Err(MosesError::Other(format!(
            "Volume too small for FAT32 (only {} clusters, need at least {})",
            total_clusters, FAT32_MIN_CLUSTERS
        )));
    }
    
    // Calculate FAT size (4 bytes per cluster, but only 28 bits used)
    let fat_entries = total_clusters + 2;  // +2 for reserved entries
    let fat_bytes = fat_entries * 4;
    let sectors_per_fat = ((fat_bytes + 511) / 512) as u32;
    
    // Recalculate with actual FAT size
    let data_start = reserved_sectors as u32 + (2 * sectors_per_fat);
    let data_sectors = total_sectors as u32 - data_start;
    let final_clusters = data_sectors / sectors_per_cluster as u32;
    
    // Final validation
    if final_clusters < FAT32_MIN_CLUSTERS {
        return Err(MosesError::Other(format!(
            "Invalid cluster count after FAT calculation: {} (need at least {})",
            final_clusters, FAT32_MIN_CLUSTERS
        )));
    }
    
    // Check for maximum cluster count (2^28 - 1 for FAT32)
    if final_clusters > 0x0FFFFFFF {
        return Err(MosesError::Other(format!(
            "Too many clusters for FAT32: {} (max 268435455)",
            final_clusters
        )));
    }
    
    Ok(FatParams {
        sectors_per_cluster,
        sectors_per_fat,
        root_entries: 0,  // FAT32 has no fixed root directory
        total_clusters: final_clusters,
        fat_type: FatType::Fat32,
    })
}

/// Automatically choose between FAT16 and FAT32 based on volume size
pub fn calculate_fat_params_auto(total_sectors: u64) -> Result<FatParams, MosesError> {
    // Try FAT16 first (for volumes up to 4GB)
    if total_sectors <= 8_388_608 {  // 4GB with 512-byte sectors
        match calculate_fat16_params(total_sectors) {
            Ok(params) => return Ok(params),
            Err(_) => {
                // Fall through to FAT32
            }
        }
    }
    
    // Use FAT32 for larger volumes or if FAT16 failed
    calculate_fat32_params(total_sectors)
}