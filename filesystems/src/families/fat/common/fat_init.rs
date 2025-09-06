// Common FAT table initialization routines for FAT16 and FAT32

use std::io::{Write, Seek, SeekFrom, Result};

/// Initialize a FAT16 table with proper reserved entries
/// 
/// # Arguments
/// * `fat_data` - Mutable slice to write FAT data into
/// * `media_descriptor` - Media descriptor byte (0xF0 for removable, 0xF8 for fixed)
/// 
/// The first two FAT16 entries are reserved:
/// - FAT[0] = 0xFF00 | media_descriptor
/// - FAT[1] = 0xFFFF (end of chain marker)
pub fn init_fat16_table(fat_data: &mut [u8], media_descriptor: u8) {
    // Ensure we have at least 4 bytes for the first two entries
    assert!(fat_data.len() >= 4, "FAT16 table must be at least 4 bytes");
    
    // Clear the FAT table first
    fat_data.fill(0);
    
    // FAT[0]: 16-bit value with media descriptor in low byte, 0xFF in high byte
    let fat0_value: u16 = 0xFF00 | (media_descriptor as u16);
    fat_data[0..2].copy_from_slice(&fat0_value.to_le_bytes());
    
    // FAT[1]: End of chain marker (0xFFFF)
    fat_data[2..4].copy_from_slice(&0xFFFFu16.to_le_bytes());
}

/// Initialize a FAT32 table with proper reserved entries
/// 
/// # Arguments
/// * `fat_data` - Mutable slice to write FAT data into
/// * `media_descriptor` - Media descriptor byte (0xF0 for removable, 0xF8 for fixed)
/// * `root_cluster` - The cluster number of the root directory (usually 2)
/// 
/// The first three FAT32 entries are reserved:
/// - FAT[0] = 0x0FFFFF00 | media_descriptor
/// - FAT[1] = 0x0FFFFFFF (end of chain marker)
/// - FAT[root_cluster] = 0x0FFFFFFF (root directory end marker)
pub fn init_fat32_table(fat_data: &mut [u8], media_descriptor: u8, root_cluster: u32) {
    // Ensure we have at least 12 bytes for the first three entries
    assert!(fat_data.len() >= 12, "FAT32 table must be at least 12 bytes");
    
    // Clear the FAT table first
    fat_data.fill(0);
    
    // FAT[0]: 32-bit value with media descriptor in low byte
    // Upper 4 bits are reserved and should be ignored (we use 0x0FFFFF00)
    let fat0_value: u32 = 0x0FFFFF00 | (media_descriptor as u32);
    fat_data[0..4].copy_from_slice(&fat0_value.to_le_bytes());
    
    // FAT[1]: End of chain marker (0x0FFFFFFF)
    fat_data[4..8].copy_from_slice(&0x0FFFFFFFu32.to_le_bytes());
    
    // FAT[root_cluster]: Mark root directory cluster as end of chain
    if root_cluster >= 2 {
        let root_offset = (root_cluster * 4) as usize;
        if root_offset + 4 <= fat_data.len() {
            fat_data[root_offset..root_offset + 4].copy_from_slice(&0x0FFFFFFFu32.to_le_bytes());
        }
    }
}

/// Write FAT tables to device (handles both FAT16 and FAT32)
/// 
/// # Arguments
/// * `device` - Device to write to
/// * `fat_data` - FAT table data to write
/// * `fat_start_sector` - Starting sector for the first FAT
/// * `sectors_per_fat` - Number of sectors per FAT
/// * `num_fats` - Number of FAT copies (usually 2)
/// * `bytes_per_sector` - Bytes per sector (usually 512)
pub fn write_fat_tables<W: Write + Seek>(
    device: &mut W,
    fat_data: &[u8],
    fat_start_sector: u64,
    sectors_per_fat: u32,
    num_fats: u8,
    bytes_per_sector: u32,
) -> Result<()> {
    let fat_size_bytes = sectors_per_fat * bytes_per_sector;
    
    // Write each FAT copy
    for i in 0..num_fats {
        let fat_offset = (fat_start_sector * bytes_per_sector as u64) + 
                        (i as u64 * fat_size_bytes as u64);
        
        device.seek(SeekFrom::Start(fat_offset))?;
        
        // Write the FAT data
        if fat_data.len() >= fat_size_bytes as usize {
            device.write_all(&fat_data[..fat_size_bytes as usize])?;
        } else {
            // Write what we have and pad with zeros
            device.write_all(fat_data)?;
            let padding = vec![0u8; (fat_size_bytes as usize) - fat_data.len()];
            device.write_all(&padding)?;
        }
    }
    
    Ok(())
}

/// Calculate the appropriate cluster size for a FAT16 volume
/// 
/// Based on Microsoft's recommended cluster sizes for optimal compatibility
pub fn calculate_fat16_cluster_size(total_size_bytes: u64) -> u8 {
    let total_sectors = total_size_bytes / 512;
    
    if total_sectors <= 32_680 {
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
    } else {
        128 // 64KB clusters for <= 4GB (max for FAT16)
    }
}

/// Calculate the appropriate cluster size for a FAT32 volume
/// 
/// Based on Microsoft's recommended cluster sizes for optimal compatibility
pub fn calculate_fat32_cluster_size(total_size_bytes: u64) -> u8 {
    let total_gb = total_size_bytes / (1024 * 1024 * 1024);
    
    if total_gb <= 8 {
        8   // 4KB clusters for <= 8GB
    } else if total_gb <= 16 {
        16  // 8KB clusters for <= 16GB
    } else if total_gb <= 32 {
        32  // 16KB clusters for <= 32GB
    } else {
        64  // 32KB clusters for > 32GB
    }
}

/// Check if a cluster count is valid for FAT16
pub fn is_valid_fat16_cluster_count(cluster_count: u64) -> bool {
    cluster_count >= 4085 && cluster_count <= 65524
}

/// Check if a cluster count is valid for FAT32
pub fn is_valid_fat32_cluster_count(cluster_count: u64) -> bool {
    cluster_count >= 65525 && cluster_count <= 0x0FFFFFF5
}

/// Determine the media descriptor byte based on device type
pub fn get_media_descriptor(is_removable: bool) -> u8 {
    if is_removable {
        0xF0  // Removable media
    } else {
        0xF8  // Fixed disk
    }
}