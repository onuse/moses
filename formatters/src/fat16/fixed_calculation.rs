// Corrected FAT16 parameter calculation
// This properly calculates cluster count with iterative FAT size adjustment

use moses_core::MosesError;

pub fn calculate_fat16_params_correct(size_bytes: u64) -> Result<(u8, u16, u16), MosesError> {
    let total_sectors = size_bytes / 512;
    
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
        return Err(MosesError::Other("Volume too large for FAT16 (max 4GB with 64KB clusters)".to_string()));
    };
    
    // Standard FAT16 values
    let root_entries = 512u16;
    let reserved_sectors = 1u16;
    let num_fats = 2u8;
    
    // Calculate root directory sectors
    let root_dir_sectors = (root_entries * 32 + 511) / 512;
    
    // Iteratively calculate FAT size
    // We need to find the right FAT size such that cluster count is valid
    let mut sectors_per_fat = 1u16;
    
    loop {
        // Calculate data sectors with current FAT size
        let fat_sectors = num_fats as u64 * sectors_per_fat as u64;
        let system_sectors = reserved_sectors as u64 + fat_sectors + root_dir_sectors as u64;
        
        if system_sectors >= total_sectors {
            return Err(MosesError::Other("Not enough space for FAT16 filesystem structures".to_string()));
        }
        
        let data_sectors = total_sectors - system_sectors;
        let total_clusters = data_sectors / sectors_per_cluster as u64;
        
        // Check if we're in valid FAT16 range
        if total_clusters < 4085 {
            return Err(MosesError::Other(format!(
                "Too few clusters for FAT16: {} (minimum 4085)",
                total_clusters
            )));
        }
        if total_clusters > 65524 {
            return Err(MosesError::Other(format!(
                "Too many clusters for FAT16: {} (maximum 65524)",
                total_clusters
            )));
        }
        
        // Calculate required FAT size for this cluster count
        // Each FAT16 entry is 2 bytes, plus 2 reserved entries
        let required_fat_entries = total_clusters + 2;
        let required_fat_bytes = required_fat_entries * 2;
        let required_sectors_per_fat = ((required_fat_bytes + 511) / 512) as u16;
        
        // If our estimate matches requirement, we're done
        if sectors_per_fat >= required_sectors_per_fat {
            // Double-check the final cluster count
            let final_data_sectors = total_sectors - (reserved_sectors as u64 + 
                (num_fats as u64 * sectors_per_fat as u64) + root_dir_sectors as u64);
            let final_clusters = final_data_sectors / sectors_per_cluster as u64;
            
            println!("FAT16 calculation complete:");
            println!("  Total sectors: {}", total_sectors);
            println!("  Reserved sectors: {}", reserved_sectors);
            println!("  FAT sectors: {} (per FAT: {})", num_fats as u64 * sectors_per_fat as u64, sectors_per_fat);
            println!("  Root dir sectors: {}", root_dir_sectors);
            println!("  Data sectors: {}", final_data_sectors);
            println!("  Sectors per cluster: {}", sectors_per_cluster);
            println!("  Total clusters: {}", final_clusters);
            
            return Ok((sectors_per_cluster, sectors_per_fat, root_entries));
        }
        
        // Adjust FAT size and try again
        sectors_per_fat = required_sectors_per_fat;
        
        // Safety check to prevent infinite loop
        if sectors_per_fat > 256 {
            return Err(MosesError::Other("FAT size calculation failed".to_string()));
        }
    }
}