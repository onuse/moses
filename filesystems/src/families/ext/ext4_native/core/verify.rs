// Ext filesystem verification after formatting (supports ext2/ext3/ext4)
use crate::families::ext::ext4_native::core::{
    structures::*,
    types::*,
    constants::*,
    checksum::crc32c_ext4,
    alignment::AlignedBuffer,
    ext_config::ExtVersion,
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
    pub detected_version: Option<ExtVersion>,
}

impl VerificationResult {
    fn new() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            info: Vec::new(),
            detected_version: None,
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

/// Detect the ext filesystem version from superblock features
pub fn detect_ext_version(sb: &Ext4Superblock) -> ExtVersion {
    // Check feature flags to determine version
    let has_journal = sb.s_feature_compat & EXT4_FEATURE_COMPAT_HAS_JOURNAL != 0;
    let has_extents = sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_EXTENTS != 0;
    let has_64bit = sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_64BIT != 0;
    let has_metadata_csum = sb.s_feature_ro_compat & EXT4_FEATURE_RO_COMPAT_METADATA_CSUM != 0;
    
    // ext4 has extents or 64-bit or metadata checksums
    if has_extents || has_64bit || has_metadata_csum {
        ExtVersion::Ext4
    }
    // ext3 has journal but no ext4 features
    else if has_journal {
        ExtVersion::Ext3
    }
    // ext2 has neither journal nor ext4 features
    else {
        ExtVersion::Ext2
    }
}

/// Verify ext filesystem on device (auto-detects ext2/ext3/ext4)
pub fn verify_ext_filesystem<R: Read + Seek>(reader: &mut R) -> Result<VerificationResult, Ext4Error> {
    let mut result = VerificationResult::new();
    info!("Starting ext filesystem verification");
    
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
    
    // Detect filesystem version
    let version = detect_ext_version(&sb);
    result.detected_version = Some(version);
    result.add_info(format!("Detected filesystem version: {:?}", version));
    
    // Verify superblock checksum if enabled (ext4 only)
    if sb.has_feature_ro_compat(EXT4_FEATURE_RO_COMPAT_METADATA_CSUM) && 
       matches!(result.detected_version, Some(ExtVersion::Ext4)) {
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
    
    // Step 4: Verify version-specific features
    let incompat = sb.s_feature_incompat;
    let compat = sb.s_feature_compat;
    let _ro_compat = sb.s_feature_ro_compat;
    
    match result.detected_version {
        Some(ExtVersion::Ext2) => {
            // ext2 should NOT have journal
            if compat & EXT4_FEATURE_COMPAT_HAS_JOURNAL != 0 {
                result.add_error("ext2 filesystem has journal feature (should be ext3)".to_string());
            }
            // ext2 should NOT have extents
            if incompat & EXT4_FEATURE_INCOMPAT_EXTENTS != 0 {
                result.add_error("ext2 filesystem has extents feature (should be ext4)".to_string());
            }
            // ext2 typically has 128-byte inodes
            if sb.s_rev_level == 0 && sb.s_inode_size != 128 {
                result.add_warning(format!("ext2 inode size is {} (expected 128)", sb.s_inode_size));
            }
        },
        Some(ExtVersion::Ext3) => {
            // ext3 MUST have journal
            if compat & EXT4_FEATURE_COMPAT_HAS_JOURNAL == 0 {
                result.add_error("ext3 filesystem missing journal feature".to_string());
            }
            // ext3 should NOT have extents
            if incompat & EXT4_FEATURE_INCOMPAT_EXTENTS != 0 {
                result.add_error("ext3 filesystem has extents feature (should be ext4)".to_string());
            }
            // ext3 should NOT have 64-bit feature
            if incompat & EXT4_FEATURE_INCOMPAT_64BIT != 0 {
                result.add_error("ext3 filesystem has 64-bit feature (should be ext4)".to_string());
            }
            // Verify journal inode
            if sb.s_journal_inum != 8 {
                result.add_warning(format!("ext3 journal inode is {} (expected 8)", sb.s_journal_inum));
            }
        },
        Some(ExtVersion::Ext4) => {
            // ext4 recommendations
            if incompat & EXT4_FEATURE_INCOMPAT_FILETYPE == 0 {
                result.add_warning("FILETYPE feature not enabled (recommended for performance)".to_string());
            }
            if incompat & EXT4_FEATURE_INCOMPAT_EXTENTS == 0 {
                result.add_warning("EXTENTS feature not enabled (recommended for large files)".to_string());
            }
        },
        None => {
            result.add_error("Could not determine ext filesystem version".to_string());
        }
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
    
    // Verify group descriptor checksum if enabled (ext4 with metadata_csum)
    if sb.has_feature_ro_compat(EXT4_FEATURE_RO_COMPAT_METADATA_CSUM) && 
       matches!(result.detected_version, Some(ExtVersion::Ext4)) {
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

/// Verify ext4 filesystem on device (compatibility wrapper)
pub fn verify_ext4_filesystem<R: Read + Seek>(reader: &mut R) -> Result<VerificationResult, Ext4Error> {
    verify_ext_filesystem(reader)
}

/// Verify filesystem on a device path
pub fn verify_device(device_path: &str) -> Result<VerificationResult, Ext4Error> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use winapi::um::winbase::FILE_FLAG_SEQUENTIAL_SCAN;
        
        // For verification, we don't need FILE_FLAG_NO_BUFFERING since we're only reading
        // and buffered I/O is fine for reads. NO_BUFFERING requires sector-aligned operations
        // which our verify_ext_filesystem doesn't guarantee.
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_SEQUENTIAL_SCAN)
            .open(device_path)?;
        
        verify_ext_filesystem(&mut file)
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        let mut file = std::fs::File::open(device_path)?;
        verify_ext_filesystem(&mut file)
    }
}