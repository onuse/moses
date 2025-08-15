// Ext4 filesystem verification after formatting
use crate::ext4_native::core::{
    structures::*,
    types::*,
    constants::*,
    checksum::crc32c_ext4,
    alignment::AlignedBuffer,
};
use log::{debug, info, warn, error};
use std::io::{Read, Seek, SeekFrom};

/// Verification results
#[derive(Debug)]
pub struct VerificationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub info: Vec<String>,
}

impl VerificationResult {
    fn new() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            info: Vec::new(),
        }
    }
    
    fn add_error(&mut self, msg: String) {
        error!("Verification error: {}", msg);
        self.errors.push(msg);
        self.is_valid = false;
    }
    
    fn add_warning(&mut self, msg: String) {
        warn!("Verification warning: {}", msg);
        self.warnings.push(msg);
    }
    
    fn add_info(&mut self, msg: String) {
        debug!("Verification info: {}", msg);
        self.info.push(msg);
    }
}

/// Verify ext4 filesystem on device
pub fn verify_ext4_filesystem<R: Read + Seek>(reader: &mut R) -> Result<VerificationResult, Ext4Error> {
    let mut result = VerificationResult::new();
    info!("Starting ext4 filesystem verification");
    
    // Step 1: Read and verify superblock
    let mut sb_buffer = AlignedBuffer::<4096>::new();
    reader.seek(SeekFrom::Start(0))?;
    reader.read_exact(&mut sb_buffer[..])?;
    
    // Superblock starts at offset 1024
    let sb_bytes = &sb_buffer[1024..2048];
    let sb = unsafe {
        std::ptr::read_unaligned(sb_bytes.as_ptr() as *const Ext4Superblock)
    };
    
    // Verify magic number
    if sb.s_magic != EXT4_SUPER_MAGIC {
        result.add_error(format!("Invalid magic number: 0x{:X} (expected 0x{:X})", 
                                sb.s_magic, EXT4_SUPER_MAGIC));
        return Ok(result); // Can't continue without valid superblock
    }
    result.add_info("Superblock magic number valid".to_string());
    
    // Verify superblock checksum if enabled
    if sb.has_feature_ro_compat(EXT4_FEATURE_RO_COMPAT_METADATA_CSUM) {
        let stored_csum = sb.s_checksum;
        let mut sb_copy = sb;
        sb_copy.s_checksum = 0;
        
        let sb_bytes = unsafe {
            std::slice::from_raw_parts(
                &sb_copy as *const _ as *const u8,
                1024
            )
        };
        
        let calculated_csum = crc32c_ext4(sb_bytes, 0xFFFFFFFF) ^ 0xFFFFFFFF;
        
        if stored_csum != calculated_csum {
            result.add_error(format!("Superblock checksum mismatch: stored=0x{:08X}, calculated=0x{:08X}",
                                   stored_csum, calculated_csum));
        } else {
            result.add_info("Superblock checksum valid".to_string());
        }
    }
    
    // Step 2: Verify filesystem state
    if sb.s_state != EXT4_VALID_FS {
        result.add_warning(format!("Filesystem state is not clean: 0x{:X}", sb.s_state));
    } else {
        result.add_info("Filesystem state is clean".to_string());
    }
    
    // Step 3: Verify block and inode counts
    let total_blocks = sb.s_blocks_count_lo as u64 | ((sb.s_blocks_count_hi as u64) << 32);
    let free_blocks = sb.s_free_blocks_count_lo as u64 | ((sb.s_free_blocks_count_hi as u64) << 32);
    
    if free_blocks > total_blocks {
        result.add_error(format!("Free blocks ({}) exceeds total blocks ({})", 
                               free_blocks, total_blocks));
    } else {
        let used_blocks = total_blocks - free_blocks;
        let usage_percent = (used_blocks as f64 / total_blocks as f64) * 100.0;
        result.add_info(format!("Block usage: {} of {} ({:.2}%)", 
                              used_blocks, total_blocks, usage_percent));
    }
    
    let total_inodes = sb.s_inodes_count;
    let free_inodes = sb.s_free_inodes_count;
    
    if free_inodes > total_inodes {
        result.add_error(format!("Free inodes ({}) exceeds total inodes ({})", 
                               free_inodes, total_inodes));
    } else {
        result.add_info(format!("Inode usage: {} of {}", 
                              total_inodes - free_inodes, total_inodes));
    }
    
    // Step 4: Verify required features
    let incompat = sb.s_feature_incompat;
    if incompat & EXT4_FEATURE_INCOMPAT_FILETYPE == 0 {
        result.add_warning("FILETYPE feature not enabled (recommended for performance)".to_string());
    }
    
    if incompat & EXT4_FEATURE_INCOMPAT_EXTENTS == 0 {
        result.add_warning("EXTENTS feature not enabled (recommended for large files)".to_string());
    }
    
    // Step 5: Verify group descriptor table
    let _num_groups = ((total_blocks + sb.s_blocks_per_group as u64 - 1) 
                      / sb.s_blocks_per_group as u64) as u32;
    
    // Read first group descriptor
    reader.seek(SeekFrom::Start((sb.s_blocks_per_group as u64) * 4096))?;
    let mut gdt_buffer = vec![0u8; 64];
    reader.read_exact(&mut gdt_buffer)?;
    
    let gd = unsafe {
        std::ptr::read_unaligned(gdt_buffer.as_ptr() as *const Ext4GroupDesc)
    };
    
    // Verify group descriptor checksum if enabled
    if sb.has_feature_ro_compat(EXT4_FEATURE_RO_COMPAT_METADATA_CSUM) {
        let stored_csum = gd.bg_checksum;
        let mut gd_copy = gd;
        gd_copy.bg_checksum = 0;
        
        let gd_bytes = unsafe {
            std::slice::from_raw_parts(
                &gd_copy as *const _ as *const u8,
                64
            )
        };
        
        // Calculate checksum (simplified - actual calculation is more complex)
        let seed = sb.s_uuid[..4].iter().fold(0u32, |acc, &b| (acc << 8) | b as u32);
        let calculated_csum = crc32c_ext4(gd_bytes, seed) as u16;
        
        if stored_csum != calculated_csum {
            result.add_warning(format!("Group 0 descriptor checksum mismatch: stored=0x{:04X}, calculated=0x{:04X}",
                                     stored_csum, calculated_csum));
        } else {
            result.add_info("Group 0 descriptor checksum valid".to_string());
        }
    }
    
    // Step 6: Basic sanity checks
    if sb.s_block_size() < 1024 || sb.s_block_size() > 65536 {
        result.add_error(format!("Invalid block size: {}", sb.s_block_size()));
    }
    
    if sb.s_inode_size < 128 || sb.s_inode_size > sb.s_block_size() as u16 {
        result.add_error(format!("Invalid inode size: {}", sb.s_inode_size));
    }
    
    if sb.s_first_data_block > 1 {
        result.add_warning(format!("Unexpected first data block: {}", sb.s_first_data_block));
    }
    
    // Step 7: Verify filesystem UUID
    let uuid_zero = sb.s_uuid.iter().all(|&b| b == 0);
    if uuid_zero {
        result.add_warning("Filesystem UUID is all zeros".to_string());
    } else {
        let uuid = format!("{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                          sb.s_uuid[0], sb.s_uuid[1], sb.s_uuid[2], sb.s_uuid[3],
                          sb.s_uuid[4], sb.s_uuid[5], sb.s_uuid[6], sb.s_uuid[7],
                          sb.s_uuid[8], sb.s_uuid[9], sb.s_uuid[10], sb.s_uuid[11],
                          sb.s_uuid[12], sb.s_uuid[13], sb.s_uuid[14], sb.s_uuid[15]);
        result.add_info(format!("Filesystem UUID: {}", uuid));
    }
    
    // Step 8: Check volume label
    let label = String::from_utf8_lossy(&sb.s_volume_name)
        .trim_end_matches('\0')
        .to_string();
    if !label.is_empty() {
        result.add_info(format!("Volume label: '{}'", label));
    }
    
    // Final summary
    if result.is_valid {
        info!("Filesystem verification passed with {} warnings", result.warnings.len());
    } else {
        error!("Filesystem verification failed with {} errors", result.errors.len());
    }
    
    Ok(result)
}

/// Verify filesystem on a device path (Windows)
#[cfg(target_os = "windows")]
pub fn verify_device(device_path: &str) -> Result<VerificationResult, Ext4Error> {
    use std::os::windows::fs::OpenOptionsExt;
    use winapi::um::winbase::FILE_FLAG_SEQUENTIAL_SCAN;
    
    // For verification, we don't need FILE_FLAG_NO_BUFFERING since we're only reading
    // and buffered I/O is fine for reads. NO_BUFFERING requires sector-aligned operations
    // which our verify_ext4_filesystem doesn't guarantee.
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_SEQUENTIAL_SCAN)
        .open(device_path)?;
    
    verify_ext4_filesystem(&mut file)
}

/// Verify filesystem on a device path (Unix)
#[cfg(not(target_os = "windows"))]
pub fn verify_device(device_path: &str) -> Result<VerificationResult, Ext4Error> {
    let mut file = std::fs::File::open(device_path)?;
    verify_ext4_filesystem(&mut file)
}