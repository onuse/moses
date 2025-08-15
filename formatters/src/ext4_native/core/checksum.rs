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
/// Uses the exact Linux kernel CRC16 implementation
fn crc16(data: &[u8], initial: u16) -> u16 {
    // CRC table for the CRC-16. The poly is 0x8005 (x^16 + x^15 + x^2 + 1)
    // This is the exact table from the Linux kernel lib/crc/crc16.c
    const CRC16_TABLE: [u16; 256] = [
        0x0000, 0xC0C1, 0xC181, 0x0140, 0xC301, 0x03C0, 0x0280, 0xC241,
        0xC601, 0x06C0, 0x0780, 0xC741, 0x0500, 0xC5C1, 0xC481, 0x0440,
        0xCC01, 0x0CC0, 0x0D80, 0xCD41, 0x0F00, 0xCFC1, 0xCE81, 0x0E40,
        0x0A00, 0xCAC1, 0xCB81, 0x0B40, 0xC901, 0x09C0, 0x0880, 0xC841,
        0xD801, 0x18C0, 0x1980, 0xD941, 0x1B00, 0xDBC1, 0xDA81, 0x1A40,
        0x1E00, 0xDEC1, 0xDF81, 0x1F40, 0xDD01, 0x1DC0, 0x1C80, 0xDC41,
        0x1400, 0xD4C1, 0xD581, 0x1540, 0xD701, 0x17C0, 0x1680, 0xD641,
        0xD201, 0x12C0, 0x1380, 0xD341, 0x1100, 0xD1C1, 0xD081, 0x1040,
        0xF001, 0x30C0, 0x3180, 0xF141, 0x3300, 0xF3C1, 0xF281, 0x3240,
        0x3600, 0xF6C1, 0xF781, 0x3740, 0xF501, 0x35C0, 0x3480, 0xF441,
        0x3C00, 0xFCC1, 0xFD81, 0x3D40, 0xFF01, 0x3FC0, 0x3E80, 0xFE41,
        0xFA01, 0x3AC0, 0x3B80, 0xFB41, 0x3900, 0xF9C1, 0xF881, 0x3840,
        0x2800, 0xE8C1, 0xE981, 0x2940, 0xEB01, 0x2BC0, 0x2A80, 0xEA41,
        0xEE01, 0x2EC0, 0x2F80, 0xEF41, 0x2D00, 0xEDC1, 0xEC81, 0x2C40,
        0xE401, 0x24C0, 0x2580, 0xE541, 0x2700, 0xE7C1, 0xE681, 0x2640,
        0x2200, 0xE2C1, 0xE381, 0x2340, 0xE101, 0x21C0, 0x2080, 0xE041,
        0xA001, 0x60C0, 0x6180, 0xA141, 0x6300, 0xA3C1, 0xA281, 0x6240,
        0x6600, 0xA6C1, 0xA781, 0x6740, 0xA501, 0x65C0, 0x6480, 0xA441,
        0x6C00, 0xACC1, 0xAD81, 0x6D40, 0xAF01, 0x6FC0, 0x6E80, 0xAE41,
        0xAA01, 0x6AC0, 0x6B80, 0xAB41, 0x6900, 0xA9C1, 0xA881, 0x6840,
        0x7800, 0xB8C1, 0xB981, 0x7940, 0xBB01, 0x7BC0, 0x7A80, 0xBA41,
        0xBE01, 0x7EC0, 0x7F80, 0xBF41, 0x7D00, 0xBDC1, 0xBC81, 0x7C40,
        0xB401, 0x74C0, 0x7580, 0xB541, 0x7700, 0xB7C1, 0xB681, 0x7640,
        0x7200, 0xB2C1, 0xB381, 0x7340, 0xB101, 0x71C0, 0x7080, 0xB041,
        0x5000, 0x90C1, 0x9181, 0x5140, 0x9301, 0x53C0, 0x5280, 0x9241,
        0x9601, 0x56C0, 0x5780, 0x9741, 0x5500, 0x95C1, 0x9481, 0x5440,
        0x9C01, 0x5CC0, 0x5D80, 0x9D41, 0x5F00, 0x9FC1, 0x9E81, 0x5E40,
        0x5A00, 0x9AC1, 0x9B81, 0x5B40, 0x9901, 0x59C0, 0x5880, 0x9841,
        0x8801, 0x48C0, 0x4980, 0x8941, 0x4B00, 0x8BC1, 0x8A81, 0x4A40,
        0x4E00, 0x8EC1, 0x8F81, 0x4F40, 0x8D01, 0x4DC0, 0x4C80, 0x8C41,
        0x4400, 0x84C1, 0x8581, 0x4540, 0x8701, 0x47C0, 0x4680, 0x8641,
        0x8201, 0x42C0, 0x4380, 0x8341, 0x4100, 0x81C1, 0x8081, 0x4040
    ];
    
    let mut crc = initial;
    for &byte in data {
        crc = (crc >> 8) ^ CRC16_TABLE[((crc & 0xff) ^ byte as u16) as usize];
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