// FAT16 parameter validation
// Ensures we never create invalid FAT16 filesystems

use moses_core::MosesError;

/// Validates FAT16 parameters and returns corrected values if needed
pub fn validate_and_fix_fat16_params(
    size_bytes: u64,
) -> Result<(u8, u16, u16, String), MosesError> {
    let total_sectors = size_bytes / 512;
    
    // FAT16 absolute limits
    const MIN_FAT16_SECTORS: u64 = 4085 * 2;  // Minimum ~4MB
    const MAX_FAT16_SECTORS: u64 = 8_388_608; // Maximum 4GB
    
    if total_sectors < MIN_FAT16_SECTORS {
        return Err(MosesError::Other(format!(
            "Device too small for FAT16. Minimum size is {} MB, device has {} MB",
            MIN_FAT16_SECTORS * 512 / 1024 / 1024,
            size_bytes / 1024 / 1024
        )));
    }
    
    if total_sectors > MAX_FAT16_SECTORS {
        return Err(MosesError::Other(format!(
            "Device too large for FAT16. Maximum size is 4 GB, device has {} GB",
            size_bytes as f64 / 1024.0 / 1024.0 / 1024.0
        )));
    }
    
    // Try different cluster sizes to find a valid configuration
    let cluster_sizes = [2, 4, 8, 16, 32, 64, 128];
    let mut valid_config = None;
    let mut validation_notes = Vec::new();
    
    for &sectors_per_cluster in &cluster_sizes {
        // Calculate with this cluster size
        let reserved_sectors = 1u16;
        let num_fats = 2u8;
        let root_entries = 512u16;
        let root_dir_sectors = (root_entries * 32 + 511) / 512;
        
        // Iteratively calculate FAT size
        let mut sectors_per_fat = 1u16;
        loop {
            // Calculate data area
            let data_start = reserved_sectors + (num_fats as u16 * sectors_per_fat) + root_dir_sectors;
            if data_start as u64 >= total_sectors {
                break; // No space for data
            }
            
            let data_sectors = total_sectors - data_start as u64;
            let total_clusters = data_sectors / sectors_per_cluster as u64;
            
            // Check if this is valid FAT16
            if total_clusters >= 4085 && total_clusters <= 65524 {
                // Calculate required FAT size for these clusters
                let required_fat_entries = total_clusters + 2;
                let required_fat_bytes = required_fat_entries * 2;
                let required_sectors_per_fat = ((required_fat_bytes + 511) / 512) as u16;
                
                if required_sectors_per_fat == sectors_per_fat {
                    // Found valid configuration!
                    valid_config = Some((
                        sectors_per_cluster,
                        sectors_per_fat,
                        root_entries,
                        total_clusters
                    ));
                    
                    validation_notes.push(format!(
                        "Valid FAT16: {} clusters with {}KB cluster size",
                        total_clusters,
                        (sectors_per_cluster as u32) * 512 / 1024
                    ));
                    break;
                } else if required_sectors_per_fat < sectors_per_fat {
                    // FAT is too big, but configuration is valid
                    valid_config = Some((
                        sectors_per_cluster,
                        sectors_per_fat,
                        root_entries,
                        total_clusters
                    ));
                    break;
                } else {
                    // Need bigger FAT
                    sectors_per_fat = required_sectors_per_fat;
                    continue;
                }
            }
            
            break; // Not valid with this cluster size
        }
        
        if valid_config.is_some() {
            break;
        }
    }
    
    match valid_config {
        Some((sectors_per_cluster, sectors_per_fat, root_entries, total_clusters)) => {
            // Additional validation checks
            if total_clusters < 4085 {
                validation_notes.push(format!(
                    "WARNING: {} clusters is borderline FAT12/FAT16. Some systems may interpret as FAT12.",
                    total_clusters
                ));
            }
            
            if sectors_per_cluster > 64 {
                validation_notes.push(
                    "WARNING: Cluster size >32KB may have compatibility issues with older systems.".to_string()
                );
            }
            
            let notes = if validation_notes.is_empty() {
                "Valid FAT16 configuration".to_string()
            } else {
                validation_notes.join("; ")
            };
            
            Ok((sectors_per_cluster, sectors_per_fat, root_entries, notes))
        }
        None => {
            Err(MosesError::Other(format!(
                "Cannot create valid FAT16 on {} MB device. No cluster size produces valid cluster count (4085-65524)",
                size_bytes / 1024 / 1024
            )))
        }
    }
}

/// Check if existing FAT16 parameters are valid
pub fn validate_fat16_params(
    total_sectors: u64,
    sectors_per_cluster: u8,
    sectors_per_fat: u16,
    root_entries: u16,
    num_fats: u8,
) -> Result<(), String> {
    // Basic sanity checks
    if sectors_per_cluster == 0 || (sectors_per_cluster & (sectors_per_cluster - 1)) != 0 {
        return Err(format!("Invalid sectors per cluster: {} (must be power of 2)", sectors_per_cluster));
    }
    
    if sectors_per_cluster > 128 {
        return Err(format!("Sectors per cluster {} exceeds FAT16 maximum of 128", sectors_per_cluster));
    }
    
    if num_fats < 1 || num_fats > 2 {
        return Err(format!("Invalid number of FATs: {} (must be 1 or 2)", num_fats));
    }
    
    if root_entries == 0 || root_entries % 16 != 0 {
        return Err(format!("Invalid root entries: {} (must be multiple of 16)", root_entries));
    }
    
    // Calculate cluster count
    let reserved_sectors = 1u16;
    let root_dir_sectors = (root_entries * 32 + 511) / 512;
    let data_start = reserved_sectors + (num_fats as u16 * sectors_per_fat) + root_dir_sectors;
    
    if data_start as u64 >= total_sectors {
        return Err("No space for data area after metadata".to_string());
    }
    
    let data_sectors = total_sectors - data_start as u64;
    let total_clusters = data_sectors / sectors_per_cluster as u64;
    
    // FAT16 cluster count must be in valid range
    if total_clusters < 4085 {
        return Err(format!(
            "Cluster count {} is too small for FAT16 (minimum 4085). This would be FAT12.",
            total_clusters
        ));
    }
    
    if total_clusters > 65524 {
        return Err(format!(
            "Cluster count {} is too large for FAT16 (maximum 65524). This would require FAT32.",
            total_clusters
        ));
    }
    
    // Verify FAT size is adequate
    let required_fat_entries = total_clusters + 2;
    let required_fat_bytes = required_fat_entries * 2;
    let required_sectors_per_fat = ((required_fat_bytes + 511) / 512) as u16;
    
    if sectors_per_fat < required_sectors_per_fat {
        return Err(format!(
            "FAT size {} sectors is too small. Need at least {} sectors for {} clusters",
            sectors_per_fat, required_sectors_per_fat, total_clusters
        ));
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_too_small_for_fat16() {
        // 2MB is too small
        let result = validate_and_fix_fat16_params(2 * 1024 * 1024);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_too_large_for_fat16() {
        // 5GB is too large
        let result = validate_and_fix_fat16_params(5 * 1024 * 1024 * 1024);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_valid_sizes() {
        // Test various valid sizes
        let sizes = [
            16 * 1024 * 1024,    // 16MB
            128 * 1024 * 1024,   // 128MB
            512 * 1024 * 1024,   // 512MB
            2048 * 1024 * 1024,  // 2GB
        ];
        
        for size in sizes {
            let result = validate_and_fix_fat16_params(size);
            assert!(result.is_ok(), "Size {} MB should be valid", size / 1024 / 1024);
            
            if let Ok((spc, spf, re, _)) = result {
                // Verify the parameters are valid
                let total_sectors = size / 512;
                let check = validate_fat16_params(total_sectors, spc, spf, re, 2);
                assert!(check.is_ok(), "Generated params should be valid for {} MB", size / 1024 / 1024);
            }
        }
    }
    
    #[test]
    fn test_borderline_sizes() {
        // Test edge cases that might produce invalid cluster counts
        
        // Just under 4GB
        let size = (4096 - 1) * 1024 * 1024;
        let result = validate_and_fix_fat16_params(size);
        assert!(result.is_ok());
        
        // Very small but valid
        let size = 8 * 1024 * 1024; // 8MB
        let result = validate_and_fix_fat16_params(size);
        assert!(result.is_ok());
    }
}