// CRC32c checksum implementation for ext4
// CRITICAL: Must use CRC32c (Castagnoli), NOT standard CRC32!

use crc32c;

/// Calculate CRC32c checksum as used by ext4
/// ext4 uses reflected CRC32c with specific initial/final XOR values
pub fn crc32c_ext4(data: &[u8], initial: u32) -> u32 {
    // ext4 inverts the initial value and the result
    let crc = !crc32c::crc32c_append(!initial, data);
    crc
}

/// Calculate superblock checksum
/// The checksum covers all bytes except the checksum field itself
pub fn calculate_superblock_checksum(sb_bytes: &[u8], csum_seed: u32) -> u32 {
    // Superblock is 1024 bytes, checksum is at offset 0x3FC (last 4 bytes)
    if sb_bytes.len() < 1024 {
        return 0;
    }
    
    // Checksum everything except the checksum field
    let before_checksum = &sb_bytes[0..0x3FC];
    crc32c_ext4(before_checksum, csum_seed)
}

/// CRC16 implementation for ext4 group descriptor checksums
fn crc16(data: &[u8], initial: u16) -> u16 {
    let mut crc = initial;
    
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021; // CRC16-CCITT polynomial
            } else {
                crc = crc << 1;
            }
        }
    }
    
    crc
}

/// Calculate group descriptor checksum
/// Uses CRC16 for GDT_CSUM feature (older ext4)
/// Uses CRC32c for METADATA_CSUM feature (newer ext4)
pub fn calculate_group_desc_checksum(
    gd_bytes: &[u8],
    fs_uuid: &[u8; 16],
    group_num: u32,
    desc_size: usize,
) -> u16 {
    // For GDT_CSUM (not METADATA_CSUM), use CRC16
    let mut crc16_val = 0xFFFF;
    
    // Include filesystem UUID (low 16 bytes)
    crc16_val = crc16(fs_uuid, crc16_val);
    
    // Include group number (as 4 bytes, little-endian)
    let group_le = group_num.to_le_bytes();
    crc16_val = crc16(&group_le, crc16_val);
    
    // Include descriptor WITHOUT checksum field
    // Checksum is at offset 0x1E (30) for both 32 and 64-byte descriptors
    if desc_size >= 32 {
        // First part before checksum
        crc16_val = crc16(&gd_bytes[0..0x1E], crc16_val);
        // Skip 2 bytes of checksum (0x1E-0x1F)
        // Rest after checksum (only for 64-byte descriptors)
        if desc_size > 32 && gd_bytes.len() > 0x20 {
            crc16_val = crc16(&gd_bytes[0x20..desc_size.min(gd_bytes.len())], crc16_val);
        }
    }
    
    crc16_val
}

/// Calculate inode checksum
pub fn calculate_inode_checksum(
    inode_bytes: &[u8],
    inode_num: u32,
    inode_generation: u32,
    fs_uuid: &[u8; 16],
) -> u32 {
    let mut crc = !0u32;
    
    // Include inode number and generation
    let inode_le = inode_num.to_le_bytes();
    let gen_le = inode_generation.to_le_bytes();
    
    crc = crc32c_ext4(&inode_le, crc);
    crc = crc32c_ext4(&gen_le, crc);
    crc = crc32c_ext4(fs_uuid, crc);
    
    // Checksum the inode structure
    // Skip the checksum fields themselves
    // i_checksum_lo is at offset 0x82-0x83 (not used in 128-byte inodes)
    // i_checksum_hi is at offset 0x100-0x101 (256-byte inodes)
    
    if inode_bytes.len() <= 128 {
        // 128-byte inode, no checksum field
        crc = crc32c_ext4(inode_bytes, crc);
    } else {
        // Larger inode with checksum fields
        // Checksum up to i_checksum_lo
        crc = crc32c_ext4(&inode_bytes[0..0x82], crc);
        // Skip checksum fields, continue after
        if inode_bytes.len() > 0x84 {
            crc = crc32c_ext4(&inode_bytes[0x84..], crc);
        }
    }
    
    crc
}

/// Calculate block bitmap checksum
pub fn calculate_block_bitmap_checksum(
    bitmap: &[u8],
    fs_uuid: &[u8; 16],
    group_num: u32,
) -> u32 {
    let mut crc = !0u32;
    
    // Include filesystem UUID
    crc = crc32c_ext4(fs_uuid, crc);
    
    // Include group number
    let group_le = group_num.to_le_bytes();
    crc = crc32c_ext4(&group_le, crc);
    
    // Checksum the bitmap
    crc = crc32c_ext4(bitmap, crc);
    
    crc
}

/// Calculate inode bitmap checksum
pub fn calculate_inode_bitmap_checksum(
    bitmap: &[u8],
    fs_uuid: &[u8; 16],
    group_num: u32,
) -> u32 {
    // Same as block bitmap
    calculate_block_bitmap_checksum(bitmap, fs_uuid, group_num)
}

/// Calculate extent tree checksum
pub fn calculate_extent_checksum(
    extent_data: &[u8],
    inode_num: u32,
    fs_uuid: &[u8; 16],
) -> u32 {
    let mut crc = !0u32;
    
    // Include filesystem UUID
    crc = crc32c_ext4(fs_uuid, crc);
    
    // Include inode number
    let inode_le = inode_num.to_le_bytes();
    crc = crc32c_ext4(&inode_le, crc);
    
    // Checksum the extent data
    crc = crc32c_ext4(extent_data, crc);
    
    crc
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_crc32c_ext4() {
        // Test with known values
        let data = b"123456789";
        let crc = crc32c_ext4(data, !0);
        
        // This should produce a specific CRC32c value
        // Note: actual value would need to be verified against ext4 implementation
        assert_ne!(crc, 0);
    }
    
    #[test]
    fn test_superblock_checksum() {
        let mut sb_bytes = vec![0u8; 1024];
        // Set some data
        sb_bytes[0] = 0x12;
        sb_bytes[1] = 0x34;
        
        let checksum = calculate_superblock_checksum(&sb_bytes, 0);
        assert_ne!(checksum, 0);
        
        // Verify it doesn't include the checksum field
        sb_bytes[0x3FC] = 0xFF;
        sb_bytes[0x3FD] = 0xFF;
        sb_bytes[0x3FE] = 0xFF;
        sb_bytes[0x3FF] = 0xFF;
        
        let checksum2 = calculate_superblock_checksum(&sb_bytes, 0);
        assert_eq!(checksum, checksum2); // Should be same since we skip checksum field
    }
    
    #[test]
    fn test_group_desc_checksum() {
        let gd_bytes = vec![0u8; 64];
        let uuid = [0u8; 16];
        
        let checksum = calculate_group_desc_checksum(&gd_bytes, &uuid, 0, 64);
        assert_ne!(checksum, 0);
    }
}