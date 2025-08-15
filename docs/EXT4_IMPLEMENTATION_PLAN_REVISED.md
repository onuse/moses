# EXT4 Native Windows Implementation Plan (Revised)

## Critical Issues Addressed in This Revision

### ⚠️ Issues That Would Cause Immediate Failure:
1. **Wrong CRC Algorithm** - ext4 uses CRC32c (Castagnoli), NOT standard CRC32
2. **Alignment Requirements** - Structures must be properly aligned, not packed
3. **Endianness** - ALL values must be little-endian
4. **Block Group 0 Special Cases** - Different layout than other groups
5. **Windows Sector Alignment** - Buffers must be 512-byte aligned
6. **Directory Entry Complexity** - rec_len rules are non-obvious
7. **Extent Tree Requirements** - Even empty inodes need valid headers

## Implementation Philosophy

**"Make it work, make it right, make it fast"** - in that order.

We will build incrementally with validation at every step. No moving forward until current step passes validation.

## Phase 0: Foundation (2 weeks)

### 0.1 Development Environment Setup
```toml
# Cargo.toml dependencies
[dependencies]
crc32c = "0.6"  # NOT crc32fast! Must be CRC32c (Castagnoli)
byteorder = "1.4"  # For explicit endianness control
static_assertions = "1.1"  # Compile-time structure validation
hex = "0.4"  # For debugging
memmap2 = "0.5"  # For memory-mapped I/O testing

[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = [
    "fileapi", "winnt", "handleapi", "ioapiset",
    "winioctl", "errhandlingapi", "sysinfoapi"
]}

[dev-dependencies]
pretty_assertions = "1.3"  # Better test output
hexdump = "0.1"  # For debugging
```

### 0.2 Critical Infrastructure
```rust
// alignment.rs - CRITICAL: Get this wrong and nothing works
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AlignedBuffer<const N: usize> {
    #[repr(align(512))]  // Windows sector alignment
    data: [u8; N],
}

// endian.rs - CRITICAL: Everything must be little-endian
use byteorder::{LittleEndian, WriteBytesExt, ReadBytesExt};

pub trait Ext4Endian {
    fn write_le<W: Write>(&self, writer: &mut W) -> io::Result<()>;
    fn read_le<R: Read>(reader: &mut R) -> io::Result<Self>;
}

// crc32c.rs - CRITICAL: Must be CRC32c, not CRC32!
pub fn crc32c_ext4(data: &[u8], initial: u32) -> u32 {
    // ext4 uses reflected CRC32c
    !crc32c::crc32c_append(!initial, data)
}
```

### 0.3 Structure Validation Framework
```rust
// Every structure MUST be validated at compile time
use static_assertions::{assert_eq_size, assert_eq_align};

// Superblock MUST be exactly 1024 bytes
assert_eq_size!(Ext4Superblock, [u8; 1024]);

// Group descriptor MUST be 32 or 64 bytes
assert_eq_size!(Ext4GroupDesc32, [u8; 32]);
assert_eq_size!(Ext4GroupDesc64, [u8; 64]);

// Inode MUST be power of 2, minimum 128
assert_eq_size!(Ext4Inode256, [u8; 256]);
```

### 0.4 Comparison Tools
```rust
// Essential for validation against mkfs.ext4
pub struct Ext4Comparator {
    pub fn compare_with_mkfs(&self, our_path: &str, mkfs_path: &str) -> ComparisonReport {
        // Byte-by-byte comparison of critical regions
        let mut report = ComparisonReport::new();
        
        // Compare superblock (offset 1024)
        report.superblock = self.compare_bytes(our_path, mkfs_path, 1024, 1024);
        
        // Compare GDT (after superblock)
        report.gdt = self.compare_bytes(our_path, mkfs_path, 2048, gdt_size);
        
        report
    }
}
```

### Phase 0 Validation
- [ ] CRC32c produces correct test vectors
- [ ] All structures have correct size
- [ ] All structures have correct alignment
- [ ] Endianness conversions work correctly
- [ ] Windows sector-aligned I/O works

## Phase 1: Minimal Valid Superblock (2 weeks)

### 1.1 Superblock Structure (EXACT)
```rust
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4Superblock {
    // CRITICAL: Order and size must be EXACT
    /* 0x000 */ pub s_inodes_count: u32,         // Total inodes count
    /* 0x004 */ pub s_blocks_count_lo: u32,      // Blocks count low
    /* 0x008 */ pub s_r_blocks_count_lo: u32,    // Reserved blocks low
    /* 0x00C */ pub s_free_blocks_count_lo: u32, // Free blocks low
    /* 0x010 */ pub s_free_inodes_count: u32,    // Free inodes count
    /* 0x014 */ pub s_first_data_block: u32,     // First data block
    /* 0x018 */ pub s_log_block_size: u32,       // Block size
    /* 0x01C */ pub s_log_cluster_size: u32,     // Cluster size
    /* 0x020 */ pub s_blocks_per_group: u32,     // Blocks per group
    /* 0x024 */ pub s_clusters_per_group: u32,   // Clusters per group
    /* 0x028 */ pub s_inodes_per_group: u32,     // Inodes per group
    /* 0x02C */ pub s_mtime: u32,                // Mount time
    /* 0x030 */ pub s_wtime: u32,                // Write time
    /* 0x034 */ pub s_mnt_count: u16,            // Mount count
    /* 0x036 */ pub s_max_mnt_count: u16,        // Maximal mount count
    /* 0x038 */ pub s_magic: u16,                // Magic (0xEF53)
    /* 0x03A */ pub s_state: u16,                // File system state
    /* 0x03C */ pub s_errors: u16,               // Behaviour on errors
    /* 0x03E */ pub s_minor_rev_level: u16,      // Minor revision
    /* 0x040 */ pub s_lastcheck: u32,            // Last check time
    /* 0x044 */ pub s_checkinterval: u32,        // Check interval
    /* 0x048 */ pub s_creator_os: u32,           // Creator OS
    /* 0x04C */ pub s_rev_level: u32,            // Revision level
    /* 0x050 */ pub s_def_resuid: u16,           // Default uid
    /* 0x052 */ pub s_def_resgid: u16,           // Default gid
    
    // -- EXT4 dynamic rev fields start here --
    /* 0x054 */ pub s_first_ino: u32,            // First non-reserved inode
    /* 0x058 */ pub s_inode_size: u16,           // Size of inode
    /* 0x05A */ pub s_block_group_nr: u16,       // Block group # 
    /* 0x05C */ pub s_feature_compat: u32,       // Compatible features
    /* 0x060 */ pub s_feature_incompat: u32,     // Incompatible features
    /* 0x064 */ pub s_feature_ro_compat: u32,    // RO-compatible features
    /* 0x068 */ pub s_uuid: [u8; 16],            // 128-bit UUID
    /* 0x078 */ pub s_volume_name: [u8; 16],     // Volume name
    /* 0x088 */ pub s_last_mounted: [u8; 64],    // Last mounted path
    /* 0x0C8 */ pub s_algorithm_usage_bitmap: u32,// Compression algorithms
    
    // -- Performance hints --
    /* 0x0CC */ pub s_prealloc_blocks: u8,       // # blocks to preallocate
    /* 0x0CD */ pub s_prealloc_dir_blocks: u8,   // # blocks for dirs
    /* 0x0CE */ pub s_reserved_gdt_blocks: u16,  // Per-group desc for growth
    
    // -- Journaling support --
    /* 0x0D0 */ pub s_journal_uuid: [u8; 16],    // UUID of journal 
    /* 0x0E0 */ pub s_journal_inum: u32,         // Inode number of journal
    /* 0x0E4 */ pub s_journal_dev: u32,          // Device number
    /* 0x0E8 */ pub s_last_orphan: u32,          // Head of orphan list
    /* 0x0EC */ pub s_hash_seed: [u32; 4],       // HTREE hash seed
    /* 0x0FC */ pub s_def_hash_version: u8,      // Default hash version
    /* 0x0FD */ pub s_jnl_backup_type: u8,       // Journal backup type
    /* 0x0FE */ pub s_desc_size: u16,            // Size of group descriptor
    /* 0x100 */ pub s_default_mount_opts: u32,   // Default mount options
    /* 0x104 */ pub s_first_meta_bg: u32,        // First metablock group
    /* 0x108 */ pub s_mkfs_time: u32,            // When filesystem created
    /* 0x10C */ pub s_jnl_blocks: [u32; 17],     // Backup of journal inode
    
    // -- 64-bit support --
    /* 0x150 */ pub s_blocks_count_hi: u32,      // Blocks count high
    /* 0x154 */ pub s_r_blocks_count_hi: u32,    // Reserved blocks high
    /* 0x158 */ pub s_free_blocks_count_hi: u32, // Free blocks high
    /* 0x15C */ pub s_min_extra_isize: u16,      // All inodes have this
    /* 0x15E */ pub s_want_extra_isize: u16,     // New inodes should have
    /* 0x160 */ pub s_flags: u32,                // Miscellaneous flags
    /* 0x164 */ pub s_raid_stride: u16,          // RAID stride
    /* 0x166 */ pub s_mmp_interval: u16,         // MMP check interval
    /* 0x168 */ pub s_mmp_block: u64,            // Block for MMP
    /* 0x170 */ pub s_raid_stripe_width: u32,    // Blocks on all disks
    /* 0x174 */ pub s_log_groups_per_flex: u8,   // FLEX_BG group size
    /* 0x175 */ pub s_checksum_type: u8,         // Metadata checksum type
    /* 0x176 */ pub s_reserved_pad: u16,         // Padding
    /* 0x178 */ pub s_kbytes_written: u64,       // KB written lifetime
    /* 0x180 */ pub s_snapshot_inum: u32,        // Inode of snapshot
    /* 0x184 */ pub s_snapshot_id: u32,          // Sequential ID
    /* 0x188 */ pub s_snapshot_r_blocks_count: u64, // Reserved blocks
    /* 0x190 */ pub s_snapshot_list: u32,        // Head of snapshot list
    /* 0x194 */ pub s_error_count: u32,          // Number of fs errors
    /* 0x198 */ pub s_first_error_time: u32,     // First error time
    /* 0x19C */ pub s_first_error_ino: u32,      // Inode in first error
    /* 0x1A0 */ pub s_first_error_block: u64,    // Block in first error
    /* 0x1A8 */ pub s_first_error_func: [u8; 32],// Function name
    /* 0x1C8 */ pub s_first_error_line: u32,     // Line number
    /* 0x1CC */ pub s_last_error_time: u32,      // Last error time
    /* 0x1D0 */ pub s_last_error_ino: u32,       // Last error inode
    /* 0x1D4 */ pub s_last_error_line: u32,      // Last error line
    /* 0x1D8 */ pub s_last_error_block: u64,     // Last error block
    /* 0x1E0 */ pub s_last_error_func: [u8; 32], // Last error function
    /* 0x200 */ pub s_mount_opts: [u8; 64],      // Mount options
    /* 0x240 */ pub s_usr_quota_inum: u32,       // User quota inode
    /* 0x244 */ pub s_grp_quota_inum: u32,       // Group quota inode
    /* 0x248 */ pub s_overhead_blocks: u32,      // Overhead blocks
    /* 0x24C */ pub s_backup_bgs: [u32; 2],      // Backup bg for sparse_super2
    /* 0x254 */ pub s_encrypt_algos: [u8; 4],    // Encryption algorithms
    /* 0x258 */ pub s_encrypt_pw_salt: [u8; 16], // Salt for encryption
    /* 0x268 */ pub s_lpf_ino: u32,              // Lost+found inode
    /* 0x26C */ pub s_prj_quota_inum: u32,       // Project quota inode
    /* 0x270 */ pub s_checksum_seed: u32,        // Checksum seed
    /* 0x274 */ pub s_reserved: [u32; 98],       // Padding to 1024 bytes
    /* 0x3FC */ pub s_checksum: u32,             // Superblock checksum
}

// CRITICAL: Verify size at compile time
static_assertions::assert_eq_size!(Ext4Superblock, [u8; 1024]);
```

### 1.2 Superblock Creation Rules
```rust
impl Ext4Superblock {
    pub fn new_minimal(size_bytes: u64) -> Result<Self> {
        let mut sb = Self::zeroed();
        
        // CRITICAL: Magic must be exactly this
        sb.s_magic = 0xEF53;
        
        // CRITICAL: State must be valid
        sb.s_state = 1; // EXT4_VALID_FS
        
        // CRITICAL: Calculate blocks correctly
        let block_size = 4096u64;
        let total_blocks = size_bytes / block_size;
        
        // CRITICAL: First data block depends on block size!
        sb.s_first_data_block = if block_size == 1024 { 1 } else { 0 };
        
        // CRITICAL: Block size is stored as log2(block_size) - 10
        sb.s_log_block_size = 2; // 4096 = 1024 << 2
        
        // CRITICAL: Must have valid revision
        sb.s_rev_level = 1; // EXT4_DYNAMIC_REV
        
        // CRITICAL: Minimum valid values
        sb.s_first_ino = 11;
        sb.s_inode_size = 256;
        sb.s_desc_size = 64;
        
        // CRITICAL: Creator OS (0 = Linux)
        sb.s_creator_os = 0;
        
        // CRITICAL: Required features for basic ext4
        sb.s_feature_compat = 0;
        sb.s_feature_incompat = INCOMPAT_FILETYPE | INCOMPAT_EXTENTS;
        sb.s_feature_ro_compat = RO_COMPAT_SPARSE_SUPER | RO_COMPAT_LARGE_FILE;
        
        Ok(sb)
    }
}
```

### 1.3 Superblock Checksum (CRITICAL)
```rust
// THIS IS THE #1 CAUSE OF FAILURES - GET IT EXACTLY RIGHT
impl Ext4Superblock {
    pub fn calculate_checksum(&self, csum_seed: u32) -> u32 {
        // Convert to bytes
        let sb_bytes = unsafe {
            std::slice::from_raw_parts(
                self as *const _ as *const u8,
                1024
            )
        };
        
        // CRITICAL: Checksum covers everything EXCEPT the checksum field
        // Checksum field is at offset 0x3FC (1020)
        let before_checksum = &sb_bytes[0..1020];
        
        // CRITICAL: Initial value depends on feature flags
        let initial = if self.s_feature_ro_compat & RO_COMPAT_METADATA_CSUM != 0 {
            // With metadata_csum, use checksum seed
            csum_seed
        } else {
            // Without metadata_csum, use filesystem UUID
            let uuid_crc = crc32c_ext4(&self.s_uuid, !0);
            uuid_crc
        };
        
        // Calculate CRC32c
        crc32c_ext4(before_checksum, initial)
    }
}
```

### Phase 1 Validation
```bash
# Test 1: Superblock recognized
hexdump -C test.img | grep "53 ef"  # Should see magic at offset 0x438

# Test 2: Basic structure valid
dumpe2fs test.img 2>&1 | head -20  # Should show filesystem info

# Test 3: e2fsck recognizes it
e2fsck -fn test.img 2>&1 | head -5  # Will complain but should recognize ext4
```

## Phase 2: Block Group 0 (3 weeks)

### 2.1 Block Group Layout (CRITICAL)
```rust
// CRITICAL: Block group 0 is special!
pub struct BlockGroup0Layout {
    // Block 0: [Unused/Boot sector - 1024 bytes]
    // Block 0: [Superblock - 1024 bytes at offset 1024]
    // Block 1+: Group Descriptor Table (size varies)
    // Block N: Block bitmap
    // Block N+1: Inode bitmap  
    // Block N+2: Inode table (multiple blocks)
    // Remaining: Data blocks
}

impl BlockGroup0Layout {
    pub fn calculate(sb: &Ext4Superblock) -> Self {
        // CRITICAL: GDT size calculation
        let groups_count = (sb.blocks_count() + sb.s_blocks_per_group - 1) 
                          / sb.s_blocks_per_group;
        let gdt_blocks = (groups_count * sb.s_desc_size as u32 + 
                         sb.block_size() - 1) / sb.block_size();
        
        // CRITICAL: Reserved GDT blocks for resize
        let reserved_gdt = sb.s_reserved_gdt_blocks as u32;
        
        Self {
            superblock_block: 0,  // At offset 1024 in block 0
            gdt_start_block: 1,
            gdt_blocks: gdt_blocks + reserved_gdt,
            block_bitmap_block: 1 + gdt_blocks + reserved_gdt,
            inode_bitmap_block: 2 + gdt_blocks + reserved_gdt,
            inode_table_block: 3 + gdt_blocks + reserved_gdt,
        }
    }
}
```

### 2.2 Group Descriptor (EXACT)
```rust
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4GroupDesc64 {
    /* 0x00 */ pub bg_block_bitmap_lo: u32,      // Block bitmap block low
    /* 0x04 */ pub bg_inode_bitmap_lo: u32,      // Inode bitmap block low
    /* 0x08 */ pub bg_inode_table_lo: u32,       // Inode table block low
    /* 0x0C */ pub bg_free_blocks_count_lo: u16, // Free blocks count low
    /* 0x0E */ pub bg_free_inodes_count_lo: u16, // Free inodes count low
    /* 0x10 */ pub bg_used_dirs_count_lo: u16,   // Used dirs count low
    /* 0x12 */ pub bg_flags: u16,                // Flags
    /* 0x14 */ pub bg_exclude_bitmap_lo: u32,    // Exclude bitmap low
    /* 0x18 */ pub bg_block_bitmap_csum_lo: u16, // Block bitmap checksum low
    /* 0x1A */ pub bg_inode_bitmap_csum_lo: u16, // Inode bitmap checksum low
    /* 0x1C */ pub bg_itable_unused_lo: u16,     // Unused inode count low
    /* 0x1E */ pub bg_checksum: u16,             // Group descriptor checksum
    /* 0x20 */ pub bg_block_bitmap_hi: u32,      // Block bitmap block high
    /* 0x24 */ pub bg_inode_bitmap_hi: u32,      // Inode bitmap block high
    /* 0x28 */ pub bg_inode_table_hi: u32,       // Inode table block high
    /* 0x2C */ pub bg_free_blocks_count_hi: u16, // Free blocks count high
    /* 0x2E */ pub bg_free_inodes_count_hi: u16, // Free inodes count high
    /* 0x30 */ pub bg_used_dirs_count_hi: u16,   // Used dirs count high
    /* 0x32 */ pub bg_itable_unused_hi: u16,     // Unused inode count high
    /* 0x34 */ pub bg_exclude_bitmap_hi: u32,    // Exclude bitmap high
    /* 0x38 */ pub bg_block_bitmap_csum_hi: u16, // Block bitmap checksum high
    /* 0x3A */ pub bg_inode_bitmap_csum_hi: u16, // Inode bitmap checksum high
    /* 0x3C */ pub bg_reserved: u32,             // Reserved for future
}

static_assertions::assert_eq_size!(Ext4GroupDesc64, [u8; 64]);
```

### 2.3 Group Descriptor Checksum (CRITICAL)
```rust
impl Ext4GroupDesc64 {
    pub fn calculate_checksum(&self, fs_uuid: &[u8; 16], group_num: u32) -> u16 {
        // CRITICAL: Different calculation than superblock!
        let mut crc = !0u32;
        
        // Include filesystem UUID
        crc = crc32c_ext4(fs_uuid, crc);
        
        // Include group number (little-endian!)
        let group_le = group_num.to_le_bytes();
        crc = crc32c_ext4(&group_le, crc);
        
        // Include descriptor WITHOUT checksum field
        let gd_bytes = unsafe {
            std::slice::from_raw_parts(self as *const _ as *const u8, 64)
        };
        
        // Checksum is at offset 0x1E (30)
        crc = crc32c_ext4(&gd_bytes[0..30], crc);
        crc = crc32c_ext4(&gd_bytes[32..], crc);
        
        (crc & 0xFFFF) as u16
    }
}
```

### 2.4 Bitmap Initialization (CRITICAL)
```rust
pub struct BitmapInitializer {
    pub fn init_block_bitmap(layout: &BlockGroup0Layout) -> [u8; 4096] {
        let mut bitmap = [0u8; 4096];
        
        // CRITICAL: Mark system blocks as used
        // Bits are in LSB order within each byte!
        let mut mark_used = |block: u32| {
            let byte = (block / 8) as usize;
            let bit = (block % 8) as u8;
            bitmap[byte] |= 1 << bit;
        };
        
        // Superblock + GDT
        for i in 0..=layout.gdt_blocks {
            mark_used(i);
        }
        
        // Bitmaps and inode table
        mark_used(layout.block_bitmap_block);
        mark_used(layout.inode_bitmap_block);
        
        let inode_table_blocks = (sb.s_inodes_per_group * sb.s_inode_size as u32) 
                                / sb.block_size();
        for i in 0..inode_table_blocks {
            mark_used(layout.inode_table_block + i);
        }
        
        bitmap
    }
    
    pub fn init_inode_bitmap(sb: &Ext4Superblock) -> [u8; 4096] {
        let mut bitmap = [0u8; 4096];
        
        // CRITICAL: Inodes 1-10 are reserved
        // Inode 0 doesn't exist (used to indicate deleted)
        for i in 0..11 {
            let byte = (i / 8) as usize;
            let bit = (i % 8) as u8;
            bitmap[byte] |= 1 << bit;
        }
        
        bitmap
    }
}
```

## Phase 3: Root Directory (3 weeks)

### 3.1 Inode Structure (EXACT)
```rust
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4Inode {
    /* 0x00 */ pub i_mode: u16,              // File mode
    /* 0x02 */ pub i_uid: u16,               // Owner UID low
    /* 0x04 */ pub i_size_lo: u32,           // Size in bytes low
    /* 0x08 */ pub i_atime: u32,             // Access time
    /* 0x0C */ pub i_ctime: u32,             // Inode change time
    /* 0x10 */ pub i_mtime: u32,             // Modification time
    /* 0x14 */ pub i_dtime: u32,             // Deletion time
    /* 0x18 */ pub i_gid: u16,               // Group ID low
    /* 0x1A */ pub i_links_count: u16,       // Links count
    /* 0x1C */ pub i_blocks_lo: u32,         // Blocks count low
    /* 0x20 */ pub i_flags: u32,             // File flags
    /* 0x24 */ pub i_osd1: u32,              // OS dependent
    /* 0x28 */ pub i_block: [u32; 15],       // Block pointers
    /* 0x64 */ pub i_generation: u32,        // File version
    /* 0x68 */ pub i_file_acl_lo: u32,       // File ACL low
    /* 0x6C */ pub i_size_high: u32,         // Size high
    /* 0x70 */ pub i_obso_faddr: u32,        // Obsolete
    /* 0x74 */ pub i_osd2: [u8; 12],         // OS dependent 2
    /* 0x80 */ pub i_extra_isize: u16,       // Extra inode size
    /* 0x82 */ pub i_checksum_hi: u16,       // Checksum high
    /* 0x84 */ pub i_ctime_extra: u32,       // Extra change time
    /* 0x88 */ pub i_mtime_extra: u32,       // Extra modify time
    /* 0x8C */ pub i_atime_extra: u32,       // Extra access time
    /* 0x90 */ pub i_crtime: u32,            // File creation time
    /* 0x94 */ pub i_crtime_extra: u32,      // Extra creation time
    /* 0x98 */ pub i_version_hi: u32,        // Version high
    /* 0x9C */ pub i_projid: u32,            // Project ID
    /* 0xA0 */ pub padding: [u8; 96],        // Padding to 256 bytes
}

static_assertions::assert_eq_size!(Ext4Inode, [u8; 256]);
```

### 3.2 Extent Tree (CRITICAL)
```rust
// CRITICAL: This is the most complex part!
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4ExtentHeader {
    pub eh_magic: u16,     // 0xF30A
    pub eh_entries: u16,   // Number of valid entries
    pub eh_max: u16,       // Maximum entries 
    pub eh_depth: u16,     // Tree depth (0 = leaf)
    pub eh_generation: u32,// Generation (unused)
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4Extent {
    pub ee_block: u32,     // First logical block
    pub ee_len: u16,       // Number of blocks
    pub ee_start_hi: u16,  // Physical block high
    pub ee_start_lo: u32,  // Physical block low
}

impl Ext4Inode {
    pub fn init_extent_tree(&mut self, data_block: u64) {
        // CRITICAL: i_block array holds extent tree
        unsafe {
            let header = self.i_block.as_mut_ptr() as *mut Ext4ExtentHeader;
            (*header).eh_magic = 0xF30A_u16.to_le();
            (*header).eh_entries = 1_u16.to_le();
            (*header).eh_max = 4_u16.to_le();  // 4 extents fit in inode
            (*header).eh_depth = 0_u16.to_le();
            (*header).eh_generation = 0_u32.to_le();
            
            // First extent follows header
            let extent = (self.i_block.as_mut_ptr() as *mut u8)
                .add(size_of::<Ext4ExtentHeader>()) as *mut Ext4Extent;
            (*extent).ee_block = 0_u32.to_le();  // Logical block 0
            (*extent).ee_len = 1_u16.to_le();    // 1 block
            (*extent).ee_start_hi = ((data_block >> 32) as u16).to_le();
            (*extent).ee_start_lo = (data_block as u32).to_le();
        }
        
        // CRITICAL: Set extent flag
        self.i_flags |= 0x80000;  // EXT4_EXTENTS_FL
    }
}
```

### 3.3 Directory Entries (CRITICAL COMPLEXITY)
```rust
// CRITICAL: Directory entry layout is tricky!
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4DirEntry2 {
    pub inode: u32,      // Inode number
    pub rec_len: u16,    // Directory entry length
    pub name_len: u8,    // Name length
    pub file_type: u8,   // File type
    // Name follows here (variable length)
}

pub struct DirectoryBlock {
    data: [u8; 4096],
}

impl DirectoryBlock {
    pub fn new_root() -> Self {
        let mut block = Self { data: [0; 4096] };
        let mut offset = 0;
        
        // CRITICAL: "." entry
        {
            let entry = unsafe {
                &mut *(block.data.as_mut_ptr().add(offset) as *mut Ext4DirEntry2)
            };
            entry.inode = 2_u32.to_le();  // Root inode
            entry.rec_len = 12_u16.to_le(); // 8 + 4 (name with padding)
            entry.name_len = 1;
            entry.file_type = 2; // EXT4_FT_DIR
            
            // Name "."
            block.data[offset + 8] = b'.';
            offset += 12;
        }
        
        // CRITICAL: ".." entry
        {
            let entry = unsafe {
                &mut *(block.data.as_mut_ptr().add(offset) as *mut Ext4DirEntry2)
            };
            entry.inode = 2_u32.to_le();  // Parent is also root
            entry.name_len = 2;
            entry.file_type = 2; // EXT4_FT_DIR
            
            // CRITICAL: rec_len extends to end of block!
            entry.rec_len = (4096 - offset as u16).to_le();
            
            // Name ".."
            block.data[offset + 8] = b'.';
            block.data[offset + 9] = b'.';
        }
        
        block
    }
}
```

## Phase 4: Multi-Group Support (2 weeks)

### 4.1 Backup Superblocks (CRITICAL)
```rust
pub fn get_backup_sb_groups(total_groups: u32) -> Vec<u32> {
    let mut groups = vec![];
    
    // CRITICAL: Sparse_super means specific groups only
    groups.push(0); // Primary
    
    if total_groups > 1 {
        groups.push(1); // Always has backup
    }
    
    // Powers of 3, 5, and 7
    let mut n = 3;
    while n < total_groups {
        groups.push(n);
        n *= 3;
    }
    
    n = 5;
    while n < total_groups {
        groups.push(n);
        n *= 5;
    }
    
    n = 7;
    while n < total_groups {
        groups.push(n);
        n *= 7;
    }
    
    groups.sort();
    groups.dedup();
    groups
}
```

## Phase 5: Complete Checksums (1 week)

### 5.1 All Checksum Types
```rust
pub enum Ext4ChecksumType {
    Superblock,      // Uses seed or UUID
    GroupDesc,       // Uses UUID + group number
    Inode,           // Uses inode number + generation
    DirEntry,        // Uses parent inode + name
    Extent,          // Uses inode number
    BlockBitmap,     // Uses group number
    InodeBitmap,     // Uses group number
}
```

## Windows-Specific Implementation

### Critical Windows Issues
```rust
// CRITICAL: Windows requires sector-aligned I/O
#[cfg(target_os = "windows")]
pub struct WindowsDiskWriter {
    handle: HANDLE,
    sector_size: u32,
}

impl WindowsDiskWriter {
    pub fn open(path: &str) -> Result<Self> {
        // CRITICAL: Need these exact flags
        let handle = unsafe {
            CreateFileW(
                path_to_wide(path).as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH,
                null_mut(),
            )
        };
        
        // CRITICAL: Get sector size for alignment
        let mut bytes_per_sector = 0u32;
        unsafe {
            GetDiskFreeSpaceW(
                null(),
                null_mut(),
                &mut bytes_per_sector,
                null_mut(),
                null_mut(),
            );
        }
        
        Ok(Self { handle, sector_size: bytes_per_sector })
    }
    
    pub fn write_aligned(&mut self, offset: u64, data: &[u8]) -> Result<()> {
        // CRITICAL: Both offset and size must be sector-aligned!
        if offset % self.sector_size as u64 != 0 {
            return Err("Offset not sector-aligned");
        }
        
        // CRITICAL: Create aligned buffer
        let aligned_size = (data.len() + self.sector_size as usize - 1) 
                          / self.sector_size as usize 
                          * self.sector_size as usize;
        
        let mut aligned = AlignedBuffer::<65536>::new();
        aligned.data[..data.len()].copy_from_slice(data);
        
        // Write
        unsafe {
            SetFilePointerEx(self.handle, offset as i64, null_mut(), FILE_BEGIN);
            WriteFile(
                self.handle,
                aligned.data.as_ptr() as *const _,
                aligned_size as u32,
                null_mut(),
                null_mut(),
            );
        }
        
        Ok(())
    }
}
```

## Validation at Every Step

### Continuous Validation Pipeline
```bash
#!/bin/bash
# Run after EVERY change

# 1. Binary inspection
hexdump -C test.img | head -100 > hex.txt
diff hex.txt reference_hex.txt

# 2. Structure check
dumpe2fs test.img 2>&1 > dump.txt
grep -E "error|invalid|bad" dump.txt && exit 1

# 3. e2fsck validation
e2fsck -fn test.img 2>&1 > fsck.txt
grep -v "clean" fsck.txt | grep -E "error|invalid|bad" && exit 1

# 4. Mount test (if possible)
sudo mount -o loop test.img /mnt/test || exit 1
ls -la /mnt/test || exit 1
sudo umount /mnt/test || exit 1

echo "All validations passed!"
```

## Timeline (Realistic)

### Month 1
- Week 1-2: Phase 0 (Foundation)
- Week 3-4: Phase 1 (Superblock)

### Month 2  
- Week 5-6: Phase 2 (Block Group 0)
- Week 7-8: Phase 3 part 1 (Inode structure)

### Month 3
- Week 9-10: Phase 3 part 2 (Directory entries)
- Week 11-12: Phase 4 (Multi-group)

### Month 4
- Week 13: Phase 5 (Checksums)
- Week 14: Integration testing
- Week 15: Bug fixes
- Week 16: Documentation

Total: **4 months for production-ready implementation**

## Critical Success Factors

1. **Get CRC32c right** - Use the correct algorithm!
2. **Respect alignment** - Both structure and I/O
3. **Handle endianness** - Everything is little-endian
4. **Validate continuously** - Don't accumulate errors
5. **Compare with mkfs.ext4** - Byte-for-byte where possible
6. **Test on real devices** - Not just image files
7. **Handle Windows quirks** - Sector alignment is critical

## What We're NOT Doing (Scope Control)

- ❌ Journal implementation (adds 2+ months)
- ❌ Extended attributes (adds 1 month)
- ❌ Encryption support (adds 2+ months)
- ❌ Resize capability (adds 1 month)
- ❌ Repair capability (adds 3+ months)
- ❌ Mount/read capability (different project)

## Definition of Done

✅ Creates ext4 filesystem that:
1. Passes `e2fsck -fn` with zero errors
2. Mounts successfully on Linux 5.x
3. Supports creating files and directories
4. Matches mkfs.ext4 output for critical structures
5. Handles 100MB to 1TB devices
6. Completes in <10 seconds for typical USB drives
7. Works on Windows 10/11 without WSL

---

This plan addresses ALL critical issues. No shortcuts, no assumptions, just systematic implementation with validation at every step.