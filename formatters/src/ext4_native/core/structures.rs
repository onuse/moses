// EXT4 filesystem structures - Phase 1: Superblock
// CRITICAL: These structures must match the ext4 specification EXACTLY

use static_assertions::assert_eq_size;
use crate::ext4_native::core::{constants::*, checksum, types::*};
use std::io;

/// EXT4 Superblock structure (1024 bytes)
/// Located at byte offset 1024 from the beginning of the device
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4Superblock {
    /* 0x000 */ pub s_inodes_count: u32,         // Total inodes count
    /* 0x004 */ pub s_blocks_count_lo: u32,      // Blocks count (low 32 bits)
    /* 0x008 */ pub s_r_blocks_count_lo: u32,    // Reserved blocks (low 32 bits)
    /* 0x00C */ pub s_free_blocks_count_lo: u32, // Free blocks (low 32 bits)
    /* 0x010 */ pub s_free_inodes_count: u32,    // Free inodes count
    /* 0x014 */ pub s_first_data_block: u32,     // First data block
    /* 0x018 */ pub s_log_block_size: u32,       // Block size = 1024 << s_log_block_size
    /* 0x01C */ pub s_log_cluster_size: u32,     // Cluster size
    /* 0x020 */ pub s_blocks_per_group: u32,     // Blocks per group
    /* 0x024 */ pub s_clusters_per_group: u32,   // Clusters per group
    /* 0x028 */ pub s_inodes_per_group: u32,     // Inodes per group
    /* 0x02C */ pub s_mtime: u32,                // Mount time
    /* 0x030 */ pub s_wtime: u32,                // Write time
    /* 0x034 */ pub s_mnt_count: u16,            // Mount count
    /* 0x036 */ pub s_max_mnt_count: u16,        // Max mount count
    /* 0x038 */ pub s_magic: u16,                // Magic (0xEF53)
    /* 0x03A */ pub s_state: u16,                // Filesystem state
    /* 0x03C */ pub s_errors: u16,               // Error handling
    /* 0x03E */ pub s_minor_rev_level: u16,      // Minor revision
    /* 0x040 */ pub s_lastcheck: u32,            // Last check time
    /* 0x044 */ pub s_checkinterval: u32,        // Check interval
    /* 0x048 */ pub s_creator_os: u32,           // Creator OS
    /* 0x04C */ pub s_rev_level: u32,            // Revision level
    /* 0x050 */ pub s_def_resuid: u16,           // Default UID for reserved blocks
    /* 0x052 */ pub s_def_resgid: u16,           // Default GID for reserved blocks
    
    // -- Dynamic revision fields (only valid if s_rev_level > 0) --
    /* 0x054 */ pub s_first_ino: u32,            // First non-reserved inode
    /* 0x058 */ pub s_inode_size: u16,           // Inode size
    /* 0x05A */ pub s_block_group_nr: u16,       // Block group number of this superblock
    /* 0x05C */ pub s_feature_compat: u32,       // Compatible features
    /* 0x060 */ pub s_feature_incompat: u32,     // Incompatible features
    /* 0x064 */ pub s_feature_ro_compat: u32,    // Read-only compatible features
    /* 0x068 */ pub s_uuid: [u8; 16],            // Filesystem UUID
    /* 0x078 */ pub s_volume_name: [u8; 16],     // Volume name
    /* 0x088 */ pub s_last_mounted: [u8; 64],    // Last mount path
    /* 0x0C8 */ pub s_algorithm_usage_bitmap: u32, // Compression algorithms used
    
    // -- Performance hints --
    /* 0x0CC */ pub s_prealloc_blocks: u8,       // Number of blocks to preallocate
    /* 0x0CD */ pub s_prealloc_dir_blocks: u8,   // Number of blocks to preallocate for dirs
    /* 0x0CE */ pub s_reserved_gdt_blocks: u16,  // Number of reserved GDT entries for growth
    
    // -- Journaling support --
    /* 0x0D0 */ pub s_journal_uuid: [u8; 16],    // UUID of journal superblock
    /* 0x0E0 */ pub s_journal_inum: u32,         // Inode number of journal file
    /* 0x0E4 */ pub s_journal_dev: u32,          // Device number of journal file
    /* 0x0E8 */ pub s_last_orphan: u32,          // Head of orphan inode list
    /* 0x0EC */ pub s_hash_seed: [u32; 4],       // HTREE hash seed
    /* 0x0FC */ pub s_def_hash_version: u8,      // Default hash version
    /* 0x0FD */ pub s_jnl_backup_type: u8,       // Journal backup type
    /* 0x0FE */ pub s_desc_size: u16,            // Size of group descriptors
    /* 0x100 */ pub s_default_mount_opts: u32,   // Default mount options
    /* 0x104 */ pub s_first_meta_bg: u32,        // First metablock block group
    /* 0x108 */ pub s_mkfs_time: u32,            // When filesystem was created
    /* 0x10C */ pub s_jnl_blocks: [u32; 17],     // Backup of journal inode
    
    // -- 64-bit support --
    /* 0x150 */ pub s_blocks_count_hi: u32,      // Blocks count (high 32 bits)
    /* 0x154 */ pub s_r_blocks_count_hi: u32,    // Reserved blocks (high 32 bits)
    /* 0x158 */ pub s_free_blocks_count_hi: u32, // Free blocks (high 32 bits)
    /* 0x15C */ pub s_min_extra_isize: u16,      // Minimum extra inode size
    /* 0x15E */ pub s_want_extra_isize: u16,     // Desired extra inode size
    /* 0x160 */ pub s_flags: u32,                // Miscellaneous flags
    /* 0x164 */ pub s_raid_stride: u16,          // RAID stride
    /* 0x166 */ pub s_mmp_interval: u16,         // MMP check interval
    /* 0x168 */ pub s_mmp_block: u64,            // Block for multi-mount protection
    /* 0x170 */ pub s_raid_stripe_width: u32,    // Blocks on all data disks
    /* 0x174 */ pub s_log_groups_per_flex: u8,   // FLEX_BG group size
    /* 0x175 */ pub s_checksum_type: u8,         // Metadata checksum type
    /* 0x176 */ pub s_reserved_pad: u16,         // Padding
    /* 0x178 */ pub s_kbytes_written: u64,       // Kilobytes written
    /* 0x180 */ pub s_snapshot_inum: u32,        // Inode number of active snapshot
    /* 0x184 */ pub s_snapshot_id: u32,          // Sequential ID of active snapshot
    /* 0x188 */ pub s_snapshot_r_blocks_count: u64, // Reserved blocks for active snapshot
    /* 0x190 */ pub s_snapshot_list: u32,        // Head of snapshot list
    /* 0x194 */ pub s_error_count: u32,          // Number of filesystem errors
    /* 0x198 */ pub s_first_error_time: u32,     // First error time
    /* 0x19C */ pub s_first_error_ino: u32,      // Inode involved in first error
    /* 0x1A0 */ pub s_first_error_block: u64,    // Block involved in first error
    /* 0x1A8 */ pub s_first_error_func: [u8; 32], // Function where error happened
    /* 0x1C8 */ pub s_first_error_line: u32,     // Line number where error happened
    /* 0x1CC */ pub s_last_error_time: u32,      // Most recent error time
    /* 0x1D0 */ pub s_last_error_ino: u32,       // Inode involved in last error
    /* 0x1D4 */ pub s_last_error_line: u32,      // Line number of last error
    /* 0x1D8 */ pub s_last_error_block: u64,     // Block involved in last error
    /* 0x1E0 */ pub s_last_error_func: [u8; 32], // Function where last error happened
    /* 0x200 */ pub s_mount_opts: [u8; 64],      // Mount options
    /* 0x240 */ pub s_usr_quota_inum: u32,       // Inode for tracking user quota
    /* 0x244 */ pub s_grp_quota_inum: u32,       // Inode for tracking group quota
    /* 0x248 */ pub s_overhead_blocks: u32,      // Overhead blocks in filesystem
    /* 0x24C */ pub s_backup_bgs: [u32; 2],      // Backup block groups for sparse_super2
    /* 0x254 */ pub s_encrypt_algos: [u8; 4],    // Encryption algorithms in use
    /* 0x258 */ pub s_encrypt_pw_salt: [u8; 16], // Salt for string2key
    /* 0x268 */ pub s_lpf_ino: u32,              // Inode number of lost+found
    /* 0x26C */ pub s_prj_quota_inum: u32,       // Inode for tracking project quota
    /* 0x270 */ pub s_checksum_seed: u32,        // Checksum seed
    /* 0x274 */ pub s_reserved: [u32; 98],       // Reserved for future use
    /* 0x3FC */ pub s_checksum: u32,             // Superblock checksum
}

// CRITICAL: Verify size at compile time
assert_eq_size!(Ext4Superblock, [u8; 1024]);

impl Ext4Superblock {
    /// Create a new zeroed superblock
    pub fn new() -> Self {
        unsafe { std::mem::zeroed() }
    }
    
    /// Initialize with minimal valid values for a new filesystem
    pub fn init_minimal(&mut self, params: &FilesystemParams, layout: &FilesystemLayout) {
        // Get current time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        
        // CRITICAL: Magic number must be exactly this
        self.s_magic = EXT4_SUPER_MAGIC;
        
        // Basic sizes and counts
        self.s_blocks_count_lo = (layout.total_blocks & 0xFFFFFFFF) as u32;
        self.s_blocks_count_hi = ((layout.total_blocks >> 32) & 0xFFFFFFFF) as u32;
        self.s_inodes_count = layout.num_groups * layout.inodes_per_group;
        
        // Block configuration
        self.s_log_block_size = match params.block_size {
            1024 => 0,
            2048 => 1,
            4096 => 2,
            _ => 2, // Default to 4096
        };
        self.s_log_cluster_size = self.s_log_block_size; // Same as block size
        self.s_blocks_per_group = layout.blocks_per_group;
        self.s_clusters_per_group = layout.blocks_per_group;
        self.s_inodes_per_group = layout.inodes_per_group;
        
        // First data block depends on block size
        self.s_first_data_block = if params.block_size == 1024 { 1 } else { 0 };
        
        // Calculate free blocks and inodes
        let metadata_blocks = self.calculate_metadata_blocks(layout);
        let free_blocks = layout.total_blocks.saturating_sub(metadata_blocks);
        self.s_free_blocks_count_lo = (free_blocks & 0xFFFFFFFF) as u32;
        self.s_free_blocks_count_hi = ((free_blocks >> 32) & 0xFFFFFFFF) as u32;
        self.s_free_inodes_count = self.s_inodes_count - EXT4_FIRST_INO;
        
        // Reserved blocks (default 5%)
        let reserved_blocks = (layout.total_blocks * params.reserved_percent as u64) / 100;
        self.s_r_blocks_count_lo = (reserved_blocks & 0xFFFFFFFF) as u32;
        self.s_r_blocks_count_hi = ((reserved_blocks >> 32) & 0xFFFFFFFF) as u32;
        
        // Filesystem state and behavior
        self.s_state = EXT4_VALID_FS;
        self.s_errors = EXT4_DEFAULT_ERRORS;
        self.s_minor_rev_level = 0;
        self.s_creator_os = EXT4_OS_LINUX;
        self.s_rev_level = EXT4_DYNAMIC_REV;
        self.s_def_resuid = 0;
        self.s_def_resgid = 0;
        
        // Timestamps
        self.s_mkfs_time = now;
        self.s_wtime = now;
        self.s_lastcheck = now;
        self.s_mtime = 0;
        
        // Mount counts
        self.s_mnt_count = 0;
        self.s_max_mnt_count = 0xFFFF; // Effectively disable mount count checking
        self.s_checkinterval = 0; // Disable interval checking
        
        // Dynamic revision fields
        self.s_first_ino = EXT4_FIRST_INO;
        self.s_inode_size = params.inode_size;
        self.s_block_group_nr = 0; // This is superblock in group 0
        
        // Feature flags - minimal set for basic ext4
        self.s_feature_compat = 0;
        self.s_feature_incompat = EXT4_FEATURE_INCOMPAT_FILETYPE | 
                                  EXT4_FEATURE_INCOMPAT_EXTENTS;
        if params.enable_64bit {
            self.s_feature_incompat |= EXT4_FEATURE_INCOMPAT_64BIT;
        }
        
        self.s_feature_ro_compat = EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER |
                                   EXT4_FEATURE_RO_COMPAT_LARGE_FILE;
        if params.enable_checksums {
            self.s_feature_ro_compat |= EXT4_FEATURE_RO_COMPAT_GDT_CSUM;
        }
        
        // UUID generation
        self.s_uuid = Self::generate_uuid();
        
        // Volume label
        if let Some(ref label) = params.label {
            let label_bytes = label.as_bytes();
            let len = label_bytes.len().min(16);
            self.s_volume_name[..len].copy_from_slice(&label_bytes[..len]);
        }
        
        // Hash seed for directory indexing
        self.s_hash_seed[0] = 0x67452301;
        self.s_hash_seed[1] = 0xEFCDAB89;
        self.s_hash_seed[2] = 0x98BADCFE;
        self.s_hash_seed[3] = 0x10325476;
        self.s_def_hash_version = EXT4_DEFAULT_HASH_VERSION;
        
        // Descriptor size
        self.s_desc_size = if params.enable_64bit { 64 } else { 32 };
        
        // Reserved GDT blocks for future growth
        // Set to 0 since we don't have resize_inode feature
        self.s_reserved_gdt_blocks = 0;
        
        // Inode extra size for extended attributes
        self.s_min_extra_isize = 32;
        self.s_want_extra_isize = 32;
        
        // Flex block groups
        self.s_log_groups_per_flex = 4; // 16 groups per flex group
        
        // Checksum configuration
        if params.enable_checksums {
            self.s_checksum_type = 1; // CRC32c
            self.s_checksum_seed = self.generate_checksum_seed();
        }
        
        // Lost+found inode (will be created later)
        self.s_lpf_ino = 11; // Standard lost+found inode number
    }
    
    /// Generate a UUID for the filesystem
    fn generate_uuid() -> [u8; 16] {
        let mut uuid = [0u8; 16];
        
        // Simple UUID v4 generation
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        
        // Fill with timestamp-based randomness
        for i in 0..16 {
            uuid[i] = ((now >> (i * 8)) & 0xFF) as u8;
        }
        
        // Set version (4) and variant bits
        uuid[6] = (uuid[6] & 0x0F) | 0x40; // Version 4
        uuid[8] = (uuid[8] & 0x3F) | 0x80; // Variant 10
        
        uuid
    }
    
    /// Generate checksum seed
    fn generate_checksum_seed(&self) -> u32 {
        // Use UUID to generate seed
        let mut seed = 0u32;
        for i in 0..4 {
            seed ^= u32::from_le_bytes([
                self.s_uuid[i*4],
                self.s_uuid[i*4 + 1],
                self.s_uuid[i*4 + 2],
                self.s_uuid[i*4 + 3],
            ]);
        }
        seed
    }
    
    /// Calculate total metadata blocks
    fn calculate_metadata_blocks(&self, layout: &FilesystemLayout) -> u64 {
        let mut metadata = 0u64;
        
        for group in 0..layout.num_groups {
            metadata += layout.metadata_blocks_per_group(group) as u64;
        }
        
        metadata
    }
    
    /// Calculate and set the superblock checksum
    pub fn update_checksum(&mut self) {
        // Only if checksums are enabled
        if self.s_feature_ro_compat & EXT4_FEATURE_RO_COMPAT_GDT_CSUM == 0 {
            return;
        }
        
        // Calculate checksum seed
        let seed = if self.s_feature_ro_compat & EXT4_FEATURE_RO_COMPAT_METADATA_CSUM != 0 {
            self.s_checksum_seed
        } else {
            // Use UUID as seed
            let uuid_crc = checksum::crc32c_ext4(&self.s_uuid, !0);
            uuid_crc
        };
        
        // Calculate checksum
        let sb_bytes = unsafe {
            std::slice::from_raw_parts(
                self as *const _ as *const u8,
                1024
            )
        };
        
        self.s_checksum = checksum::calculate_superblock_checksum(sb_bytes, seed);
    }
    
    /// Write superblock to a byte buffer
    pub fn write_to_buffer(&self, buffer: &mut [u8]) -> io::Result<()> {
        if buffer.len() < 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Buffer too small for superblock"
            ));
        }
        
        // Convert to bytes (structures are already little-endian in memory)
        let sb_bytes = unsafe {
            std::slice::from_raw_parts(
                self as *const _ as *const u8,
                1024
            )
        };
        
        buffer[..1024].copy_from_slice(sb_bytes);
        Ok(())
    }
    
    /// Validate superblock fields
    pub fn validate(&self) -> Result<(), String> {
        // Check magic
        if self.s_magic != EXT4_SUPER_MAGIC {
            return Err(format!("Invalid magic: 0x{:04X}", self.s_magic));
        }
        
        // Check revision
        if self.s_rev_level != EXT4_DYNAMIC_REV && self.s_rev_level != EXT4_GOOD_OLD_REV {
            return Err(format!("Invalid revision: {}", self.s_rev_level));
        }
        
        // Check state
        if self.s_state != EXT4_VALID_FS {
            return Err(format!("Invalid state: {}", self.s_state));
        }
        
        // Check block size
        if self.s_log_block_size > 6 {
            return Err(format!("Invalid block size: {}", self.s_log_block_size));
        }
        
        // Check required features for ext4
        if self.s_feature_incompat & EXT4_FEATURE_INCOMPAT_FILETYPE == 0 {
            return Err("Missing required FILETYPE feature".to_string());
        }
        
        Ok(())
    }
}

impl Default for Ext4Superblock {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Phase 2: Block Group Structures
// ============================================================================

/// Block Group Descriptor (32 bytes for 32-bit, 64 bytes for 64-bit)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4GroupDesc {
    /* 0x00 */ pub bg_block_bitmap_lo: u32,      // Block bitmap block (low 32 bits)
    /* 0x04 */ pub bg_inode_bitmap_lo: u32,      // Inode bitmap block (low 32 bits)
    /* 0x08 */ pub bg_inode_table_lo: u32,       // Inode table block (low 32 bits)
    /* 0x0C */ pub bg_free_blocks_count_lo: u16, // Free blocks count (low 16 bits)
    /* 0x0E */ pub bg_free_inodes_count_lo: u16, // Free inodes count (low 16 bits)
    /* 0x10 */ pub bg_used_dirs_count_lo: u16,   // Used directories count (low 16 bits)
    /* 0x12 */ pub bg_flags: u16,                // Block group flags
    /* 0x14 */ pub bg_exclude_bitmap_lo: u32,    // Exclude bitmap for snapshots
    /* 0x18 */ pub bg_block_bitmap_csum_lo: u16, // Block bitmap checksum (low 16 bits)
    /* 0x1A */ pub bg_inode_bitmap_csum_lo: u16, // Inode bitmap checksum (low 16 bits)
    /* 0x1C */ pub bg_itable_unused_lo: u16,     // Unused inodes count (low 16 bits)
    /* 0x1E */ pub bg_checksum: u16,             // Group descriptor checksum
    // 64-bit fields (only if s_desc_size >= 64)
    /* 0x20 */ pub bg_block_bitmap_hi: u32,      // Block bitmap block (high 32 bits)
    /* 0x24 */ pub bg_inode_bitmap_hi: u32,      // Inode bitmap block (high 32 bits)
    /* 0x28 */ pub bg_inode_table_hi: u32,       // Inode table block (high 32 bits)
    /* 0x2C */ pub bg_free_blocks_count_hi: u16, // Free blocks count (high 16 bits)
    /* 0x2E */ pub bg_free_inodes_count_hi: u16, // Free inodes count (high 16 bits)
    /* 0x30 */ pub bg_used_dirs_count_hi: u16,   // Used directories count (high 16 bits)
    /* 0x32 */ pub bg_itable_unused_hi: u16,     // Unused inodes count (high 16 bits)
    /* 0x34 */ pub bg_exclude_bitmap_hi: u32,    // Exclude bitmap (high 32 bits)
    /* 0x38 */ pub bg_block_bitmap_csum_hi: u16, // Block bitmap checksum (high 16 bits)
    /* 0x3A */ pub bg_inode_bitmap_csum_hi: u16, // Inode bitmap checksum (high 16 bits)
    /* 0x3C */ pub bg_reserved: u32,             // Reserved for future use
}

// Verify sizes
assert_eq_size!(Ext4GroupDesc, [u8; 64]);

impl Ext4GroupDesc {
    /// Create a new zeroed group descriptor
    pub fn new() -> Self {
        unsafe { std::mem::zeroed() }
    }
    
    /// Initialize for a block group
    pub fn init(&mut self, group: u32, layout: &FilesystemLayout, params: &FilesystemParams) {
        let blocks_per_group = layout.blocks_per_group;
        let group_start = group as u64 * blocks_per_group as u64;
        
        // Calculate block numbers for this group's metadata
        // In the first group, we skip the superblock and its padding
        let mut current_block = if group == 0 {
            // First group: skip boot block (if 1K blocks) and superblock
            if params.block_size == 1024 {
                2 // Boot block + superblock block
            } else {
                1 // Just superblock block (superblock is in block 0 for 4K blocks)
            }
        } else {
            group_start
        };
        
        // Group descriptor table blocks
        let gdt_blocks = layout.gdt_blocks();
        current_block += gdt_blocks as u64;
        
        // Reserved GDT blocks (for online resize)
        current_block += layout.reserved_gdt_blocks as u64;
        
        // Block bitmap
        self.bg_block_bitmap_lo = current_block as u32;
        self.bg_block_bitmap_hi = (current_block >> 32) as u32;
        current_block += 1;
        
        // Inode bitmap
        self.bg_inode_bitmap_lo = current_block as u32;
        self.bg_inode_bitmap_hi = (current_block >> 32) as u32;
        current_block += 1;
        
        // Inode table
        self.bg_inode_table_lo = current_block as u32;
        self.bg_inode_table_hi = (current_block >> 32) as u32;
        let inode_table_blocks = layout.inode_table_blocks();
        current_block += inode_table_blocks as u64;
        
        // Calculate free blocks and inodes
        let metadata_blocks = (current_block - group_start) as u32;
        let total_blocks_in_group = if group == layout.num_groups - 1 {
            // Last group might be smaller
            ((layout.total_blocks - group_start) as u32).min(blocks_per_group)
        } else {
            blocks_per_group
        };
        
        self.bg_free_blocks_count_lo = (total_blocks_in_group - metadata_blocks) as u16;
        self.bg_free_blocks_count_hi = ((total_blocks_in_group - metadata_blocks) >> 16) as u16;
        
        // All inodes are free initially except reserved ones in group 0
        if group == 0 {
            self.bg_free_inodes_count_lo = (layout.inodes_per_group - EXT4_FIRST_INO) as u16;
            self.bg_free_inodes_count_hi = 0;
            self.bg_used_dirs_count_lo = 1; // Root directory
        } else {
            self.bg_free_inodes_count_lo = layout.inodes_per_group as u16;
            self.bg_free_inodes_count_hi = (layout.inodes_per_group >> 16) as u16;
            self.bg_used_dirs_count_lo = 0;
        }
        
        // Flags
        self.bg_flags = 0; // Will be set later for uninit groups
        
        // Unused inodes (for lazy initialization)
        self.bg_itable_unused_lo = if group == 0 {
            (layout.inodes_per_group - EXT4_FIRST_INO) as u16
        } else {
            layout.inodes_per_group as u16
        };
        self.bg_itable_unused_hi = 0;
    }
    
    /// Calculate and set checksum
    pub fn update_checksum(&mut self, group: u32, sb: &Ext4Superblock) {
        if sb.s_feature_ro_compat & EXT4_FEATURE_RO_COMPAT_GDT_CSUM == 0 {
            return;
        }
        
        // Clear checksum field before calculation
        self.bg_checksum = 0;
        
        // Get descriptor bytes
        let desc_size = if sb.s_desc_size >= 64 { 64 } else { 32 };
        let desc_bytes = unsafe {
            std::slice::from_raw_parts(
                self as *const _ as *const u8,
                desc_size
            )
        };
        
        // Calculate checksum using the proper method
        // GDT_CSUM uses CRC16, METADATA_CSUM uses CRC32c
        let checksum = checksum::calculate_group_desc_checksum(
            desc_bytes,
            &sb.s_uuid,
            group,
            desc_size
        );
        
        self.bg_checksum = checksum;
    }
}

impl Default for Ext4GroupDesc {
    fn default() -> Self {
        Self::new()
    }
}

/// Inode structure (256 bytes for ext4 with extended attributes)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4Inode {
    /* 0x00 */ pub i_mode: u16,         // File mode
    /* 0x02 */ pub i_uid: u16,          // Low 16 bits of Owner UID
    /* 0x04 */ pub i_size_lo: u32,      // Size in bytes (low 32 bits)
    /* 0x08 */ pub i_atime: u32,        // Access time
    /* 0x0C */ pub i_ctime: u32,        // Inode change time
    /* 0x10 */ pub i_mtime: u32,        // Modification time
    /* 0x14 */ pub i_dtime: u32,        // Deletion time
    /* 0x18 */ pub i_gid: u16,          // Low 16 bits of Group ID
    /* 0x1A */ pub i_links_count: u16,  // Links count
    /* 0x1C */ pub i_blocks_lo: u32,    // Blocks count (low 32 bits)
    /* 0x20 */ pub i_flags: u32,        // File flags
    /* 0x24 */ pub i_osd1: u32,         // OS dependent 1
    /* 0x28 */ pub i_block: [u32; 15],  // Pointers to blocks or extent tree
    /* 0x64 */ pub i_generation: u32,   // File version
    /* 0x68 */ pub i_file_acl_lo: u32,  // File ACL (low 32 bits)
    /* 0x6C */ pub i_size_high: u32,    // Size in bytes (high 32 bits)
    /* 0x70 */ pub i_obso_faddr: u32,   // Obsoleted fragment address
    /* 0x74 */ pub i_osd2: [u8; 12],    // OS dependent 2
    /* 0x80 */ pub i_extra_isize: u16,  // Extra inode size
    /* 0x82 */ pub i_checksum_hi: u16,  // Checksum (high 16 bits)
    /* 0x84 */ pub i_ctime_extra: u32,  // Extra change time (nanoseconds)
    /* 0x88 */ pub i_mtime_extra: u32,  // Extra modification time (nanoseconds)
    /* 0x8C */ pub i_atime_extra: u32,  // Extra access time (nanoseconds)
    /* 0x90 */ pub i_crtime: u32,       // File creation time
    /* 0x94 */ pub i_crtime_extra: u32, // Extra creation time (nanoseconds)
    /* 0x98 */ pub i_version_hi: u32,   // High 32 bits of version
    /* 0x9C */ pub i_projid: u32,       // Project ID
    /* 0xA0 */ pub i_reserved: [u8; 96], // Reserved space to reach 256 bytes
}

// Verify size (standard ext4 inode with extra space)
assert_eq_size!(Ext4Inode, [u8; 256]);

impl Ext4Inode {
    /// Create a new zeroed inode
    pub fn new() -> Self {
        unsafe { std::mem::zeroed() }
    }
    
    /// Initialize as lost+found directory inode
    pub fn init_lost_found_dir(&mut self, params: &FilesystemParams) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        
        self.i_mode = S_IFDIR | 0o700;  // Directory with mode 700
        self.i_uid = 0;
        self.i_gid = 0;
        self.i_size_lo = params.block_size;
        self.i_size_high = 0;
        self.i_atime = now;
        self.i_ctime = now;
        self.i_mtime = now;
        self.i_crtime = now;
        self.i_links_count = 2;  // . and parent
        self.i_blocks_lo = (params.block_size / 512) as u32;  // In 512-byte sectors
        self.i_flags = EXT4_EXTENTS_FL;
        self.i_generation = 0;
        self.i_extra_isize = 32;
        
        // Initialize extent tree
        self.init_extent_tree();
    }
    
    /// Initialize as root directory inode
    pub fn init_root_dir(&mut self, params: &FilesystemParams) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        
        // Directory mode: drwxr-xr-x (755)
        self.i_mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH;
        
        // Root owned
        self.i_uid = 0;
        self.i_gid = 0;
        
        // Size is one block for directory entries
        self.i_size_lo = params.block_size;
        self.i_size_high = 0;
        
        // Timestamps
        self.i_atime = now;
        self.i_ctime = now;
        self.i_mtime = now;
        self.i_crtime = now;
        
        // Two links: . and parent's reference
        self.i_links_count = 2;
        
        // One block allocated
        self.i_blocks_lo = (params.block_size / 512) as u32; // In 512-byte sectors
        
        // Enable extents
        self.i_flags = EXT4_EXTENTS_FL;
        
        // Generation number
        self.i_generation = 0;
        
        // Extra inode size for extended attributes
        self.i_extra_isize = 32;
        
        // Initialize extent header in i_block
        self.init_extent_tree();
    }
    
    /// Initialize extent tree for root directory
    fn init_extent_tree(&mut self) {
        // Extent header is stored at the beginning of i_block
        // We'll implement this properly when we add extent support
        let header = &mut self.i_block[0..3];
        
        // Magic number for extent header (0xF30A)
        header[0] = 0x0AF30000; // Magic in first 16 bits, entries=0 in last 16 bits
        header[1] = 0x00010000; // Max entries=1 in first 16 bits, depth=0 in last 16 bits  
        header[2] = 0x00000000; // Generation
        
        // We'll add the actual extent in Phase 3 when we allocate the directory block
    }
    
    /// Calculate inode checksum
    pub fn update_checksum(&mut self, inode_num: u32, sb: &Ext4Superblock) {
        if sb.s_feature_ro_compat & EXT4_FEATURE_RO_COMPAT_METADATA_CSUM == 0 {
            return;
        }
        
        // Will implement full checksum calculation later
        self.i_checksum_hi = 0;
    }
}

impl Default for Ext4Inode {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Phase 3: Directory and Extent Structures
// ============================================================================

/// Directory entry structure (variable length)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4DirEntry2 {
    pub inode: u32,        // Inode number
    pub rec_len: u16,      // Directory entry length
    pub name_len: u8,      // Name length
    pub file_type: u8,     // File type
    // name follows immediately after this structure
}

impl Ext4DirEntry2 {
    /// Calculate the minimum size needed for a directory entry
    pub fn size_needed(name_len: usize) -> usize {
        // Base structure size + name length, rounded up to 4 bytes
        let base_size = std::mem::size_of::<Self>();
        let total = base_size + name_len;
        (total + 3) & !3  // Round up to 4-byte boundary
    }
    
    /// Create a directory entry
    pub fn new(inode: u32, name: &str, file_type: u8) -> (Self, Vec<u8>) {
        let name_bytes = name.as_bytes();
        let entry = Self {
            inode,
            rec_len: 0,  // Will be set by caller based on available space
            name_len: name_bytes.len() as u8,
            file_type,
        };
        (entry, name_bytes.to_vec())
    }
}

/// Extent header - starts the extent tree
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4ExtentHeader {
    pub eh_magic: u16,       // Magic number (0xF30A)
    pub eh_entries: u16,     // Number of valid entries
    pub eh_max: u16,         // Maximum entries
    pub eh_depth: u16,       // Tree depth (0 for leaf)
    pub eh_generation: u32,  // Generation for consistency
}

assert_eq_size!(Ext4ExtentHeader, [u8; 12]);

impl Ext4ExtentHeader {
    pub fn new_leaf(max_entries: u16) -> Self {
        Self {
            eh_magic: EXT4_EXTENT_MAGIC,
            eh_entries: 0,
            eh_max: max_entries,
            eh_depth: 0,  // Leaf node
            eh_generation: 0,
        }
    }
}

/// Extent - points to data blocks
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4Extent {
    pub ee_block: u32,       // First logical block
    pub ee_len: u16,         // Number of blocks
    pub ee_start_hi: u16,    // High 16 bits of physical block
    pub ee_start_lo: u32,    // Low 32 bits of physical block
}

assert_eq_size!(Ext4Extent, [u8; 12]);

impl Ext4Extent {
    pub fn new(logical_block: u32, physical_block: u64, length: u16) -> Self {
        Self {
            ee_block: logical_block,
            ee_len: length,
            ee_start_hi: (physical_block >> 32) as u16,
            ee_start_lo: (physical_block & 0xFFFFFFFF) as u32,
        }
    }
    
    pub fn physical_block(&self) -> u64 {
        ((self.ee_start_hi as u64) << 32) | (self.ee_start_lo as u64)
    }
}

/// Create root directory data block with lost+found entry
pub fn create_root_directory_block(block_size: u32) -> Vec<u8> {
    let mut data = vec![0u8; block_size as usize];
    let mut offset = 0;
    
    // Entry 1: "." pointing to root inode (2)
    let (dot_entry, dot_name) = Ext4DirEntry2::new(EXT4_ROOT_INO, ".", EXT4_FT_DIR);
    
    // Entry 2: ".." also pointing to root inode (2) since root is its own parent
    let (dotdot_entry, dotdot_name) = Ext4DirEntry2::new(EXT4_ROOT_INO, "..", EXT4_FT_DIR);
    
    // Entry 3: "lost+found" pointing to inode 11
    let (lf_entry, lf_name) = Ext4DirEntry2::new(EXT4_FIRST_INO as u32, "lost+found", EXT4_FT_DIR);
    
    // Calculate record lengths
    // First entry is fixed size
    let mut dot_entry_mut = dot_entry;
    dot_entry_mut.rec_len = 12;  // Standard size for "." entry
    
    // Second entry is fixed size
    let mut dotdot_entry_mut = dotdot_entry;
    dotdot_entry_mut.rec_len = 12;  // Standard size for ".." entry
    
    // Third entry takes the rest of the block
    let mut lf_entry_mut = lf_entry;
    lf_entry_mut.rec_len = (block_size as u16) - 24;
    
    // Write "." entry
    unsafe {
        let entry_bytes = std::slice::from_raw_parts(
            &dot_entry_mut as *const _ as *const u8,
            std::mem::size_of::<Ext4DirEntry2>()
        );
        data[offset..offset + 8].copy_from_slice(entry_bytes);
    }
    data[offset + 8..offset + 9].copy_from_slice(&dot_name);
    offset = 12;
    
    // Write ".." entry
    unsafe {
        let entry_bytes = std::slice::from_raw_parts(
            &dotdot_entry_mut as *const _ as *const u8,
            std::mem::size_of::<Ext4DirEntry2>()
        );
        data[offset..offset + 8].copy_from_slice(entry_bytes);
    }
    data[offset + 8..offset + 10].copy_from_slice(&dotdot_name);
    offset = 24;
    
    // Write "lost+found" entry
    unsafe {
        let entry_bytes = std::slice::from_raw_parts(
            &lf_entry_mut as *const _ as *const u8,
            std::mem::size_of::<Ext4DirEntry2>()
        );
        data[offset..offset + 8].copy_from_slice(entry_bytes);
    }
    data[offset + 8..offset + 8 + lf_name.len()].copy_from_slice(&lf_name);
    
    data
}

/// Create lost+found directory data block
pub fn create_lost_found_directory_block(block_size: u32) -> Vec<u8> {
    let mut data = vec![0u8; block_size as usize];
    let mut offset = 0;
    
    // Entry 1: "." pointing to lost+found inode (11)
    let (dot_entry, dot_name) = Ext4DirEntry2::new(EXT4_FIRST_INO as u32, ".", EXT4_FT_DIR);
    
    // Entry 2: ".." pointing to root inode (2)
    let (dotdot_entry, dotdot_name) = Ext4DirEntry2::new(EXT4_ROOT_INO, "..", EXT4_FT_DIR);
    
    // Calculate record lengths
    let mut dot_entry_mut = dot_entry;
    dot_entry_mut.rec_len = 12;
    
    let mut dotdot_entry_mut = dotdot_entry;
    dotdot_entry_mut.rec_len = (block_size as u16) - 12;
    
    // Write "." entry
    unsafe {
        let entry_bytes = std::slice::from_raw_parts(
            &dot_entry_mut as *const _ as *const u8,
            std::mem::size_of::<Ext4DirEntry2>()
        );
        data[offset..offset + 8].copy_from_slice(entry_bytes);
    }
    data[offset + 8..offset + 9].copy_from_slice(&dot_name);
    offset = 12;
    
    // Write ".." entry
    unsafe {
        let entry_bytes = std::slice::from_raw_parts(
            &dotdot_entry_mut as *const _ as *const u8,
            std::mem::size_of::<Ext4DirEntry2>()
        );
        data[offset..offset + 8].copy_from_slice(entry_bytes);
    }
    data[offset + 8..offset + 10].copy_from_slice(&dotdot_name);
    
    data
}

/// Update root inode with extent pointing to directory block
pub fn update_root_inode_extents(inode: &mut Ext4Inode, dir_block: u64) {
    // Clear the i_block array first
    inode.i_block = [0; 15];
    
    // Create extent header at the beginning of i_block
    let header = Ext4ExtentHeader::new_leaf(4);  // Max 4 extents in inode
    
    // Create extent pointing to directory block
    let extent = Ext4Extent::new(0, dir_block, 1);  // Logical block 0, 1 block
    
    // Write header to i_block (first 3 u32s)
    unsafe {
        let header_bytes = std::slice::from_raw_parts(
            &header as *const _ as *const u8,
            12
        );
        let header_u32s = std::slice::from_raw_parts(
            header_bytes.as_ptr() as *const u32,
            3
        );
        inode.i_block[0] = header_u32s[0];
        inode.i_block[1] = header_u32s[1];
        inode.i_block[2] = header_u32s[2];
    }
    
    // Update header to have 1 entry
    let mut header_mut = header;
    header_mut.eh_entries = 1;
    
    // Rewrite updated header
    unsafe {
        let header_bytes = std::slice::from_raw_parts(
            &header_mut as *const _ as *const u8,
            12
        );
        let header_u32s = std::slice::from_raw_parts(
            header_bytes.as_ptr() as *const u32,
            3
        );
        inode.i_block[0] = header_u32s[0];
        inode.i_block[1] = header_u32s[1];
        inode.i_block[2] = header_u32s[2];
    }
    
    // Write extent immediately after header (next 3 u32s)
    unsafe {
        let extent_bytes = std::slice::from_raw_parts(
            &extent as *const _ as *const u8,
            12
        );
        let extent_u32s = std::slice::from_raw_parts(
            extent_bytes.as_ptr() as *const u32,
            3
        );
        inode.i_block[3] = extent_u32s[0];
        inode.i_block[4] = extent_u32s[1];
        inode.i_block[5] = extent_u32s[2];
    }
}