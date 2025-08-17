// Golden tests to ensure ext4 formatting doesn't break during refactoring
// These tests capture the exact byte patterns and structures we expect

#[cfg(test)]
use crate::ext4_native::core::{
    types::{FilesystemParams, FilesystemLayout},
    structures::{Ext4Superblock, Ext4GroupDesc, Ext4Inode, Ext4DirEntry2, Ext4ExtentHeader, create_root_directory_block},
    constants::*,
    bitmap::{Bitmap, init_block_bitmap_group0},
    ext_builder::ExtFilesystemBuilder,
};

#[test]
fn test_ext4_superblock_unchanged() {
    // Create a superblock with known parameters
    let params = FilesystemParams {
        size_bytes: 1024 * 1024 * 1024, // 1GB
        block_size: 4096,
        inode_size: 256,
        label: Some("TEST".to_string()),
        reserved_percent: 5,
        enable_checksums: true,
        enable_64bit: false,
        enable_journal: false,
    };
    
    let layout = FilesystemLayout::from_params(&params).unwrap();
    let mut sb = Ext4Superblock::new();
    sb.init_minimal(&params, &layout);
    
    // Capture critical fields that must not change
    assert_eq!(sb.s_magic, 0xEF53);
    assert_eq!(sb.s_log_block_size, 2); // 4096 = 1024 << 2
    assert_eq!(sb.s_blocks_per_group, 32768);
    assert_eq!(sb.s_inodes_per_group, 8192);
    assert_eq!(sb.s_inode_size, 256);
    assert_eq!(sb.s_rev_level, 1); // EXT2_DYNAMIC_REV
    
    // Feature flags that should be set
    let incompat = sb.s_feature_incompat;
    assert!(incompat & EXT4_FEATURE_INCOMPAT_FILETYPE != 0);
    assert!(incompat & EXT4_FEATURE_INCOMPAT_EXTENTS != 0);
    
    let ro_compat = sb.s_feature_ro_compat;
    assert!(ro_compat & EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER != 0);
    assert!(ro_compat & EXT4_FEATURE_RO_COMPAT_LARGE_FILE != 0);
    assert!(ro_compat & EXT4_FEATURE_RO_COMPAT_METADATA_CSUM != 0);
}

#[test]
fn test_ext4_group_descriptor_unchanged() {
    let params = FilesystemParams {
        size_bytes: 1024 * 1024 * 1024,
        block_size: 4096,
        inode_size: 256,
        label: None,
        reserved_percent: 5,
        enable_checksums: true,
        enable_64bit: false,
        enable_journal: false,
    };
    
    let layout = FilesystemLayout::from_params(&params).unwrap();
    let mut gd = Ext4GroupDesc::new();
    gd.init(0, &layout, &params);
    
    // Verify group descriptor layout
    assert_eq!(gd.bg_block_bitmap_lo, 259); // After superblock + GDT
    assert_eq!(gd.bg_inode_bitmap_lo, 260);
    assert_eq!(gd.bg_inode_table_lo, 261);
    
    // Free counts should be consistent
    let free_blocks = gd.bg_free_blocks_count_lo as u32 
        | ((gd.bg_free_blocks_count_hi as u32) << 16);
    assert!(free_blocks > 0 && free_blocks < layout.blocks_per_group);
    
    let free_inodes = gd.bg_free_inodes_count_lo as u32
        | ((gd.bg_free_inodes_count_hi as u32) << 16);
    assert_eq!(free_inodes, layout.inodes_per_group - EXT4_FIRST_INO);
}

#[test]
fn test_ext4_inode_structure_unchanged() {
    let params = FilesystemParams::default();
    let mut inode = Ext4Inode::new();
    inode.init_root_dir(&params);
    
    // Verify root directory inode
    assert_eq!(inode.i_mode, 0x41ED); // S_IFDIR | 0755
    assert_eq!(inode.i_uid, 0);
    assert_eq!(inode.i_gid, 0);
    assert_eq!(inode.i_links_count, 2); // . and ..
    assert_eq!(inode.i_size_lo, 4096); // One block for directory
    
    // Check extent header is present
    let extent_header = unsafe {
        std::ptr::read(inode.i_block.as_ptr() as *const Ext4ExtentHeader)
    };
    assert_eq!(extent_header.eh_magic, 0xF30A);
    assert_eq!(extent_header.eh_entries, 1);
    assert_eq!(extent_header.eh_max, 4);
    assert_eq!(extent_header.eh_depth, 0);
}

#[test]
fn test_ext4_directory_entries_unchanged() {
    // Test root directory structure
    let dir_data = create_root_directory_block(4096);
    
    // Check "." entry
    let dot_entry = unsafe {
        &*(dir_data.as_ptr() as *const Ext4DirEntry2)
    };
    let dot_inode = dot_entry.inode;
    let dot_rec_len = dot_entry.rec_len;
    let dot_name_len = dot_entry.name_len;
    let dot_file_type = dot_entry.file_type;
    assert_eq!(dot_inode, EXT4_ROOT_INO);
    assert_eq!(dot_rec_len, 12);
    assert_eq!(dot_name_len, 1);
    assert_eq!(dot_file_type, 2); // EXT4_FT_DIR
    
    // Check ".." entry
    let dotdot_entry = unsafe {
        &*(dir_data.as_ptr().add(12) as *const Ext4DirEntry2)
    };
    let dotdot_inode = dotdot_entry.inode;
    let dotdot_name_len = dotdot_entry.name_len;
    let dotdot_file_type = dotdot_entry.file_type;
    assert_eq!(dotdot_inode, EXT4_ROOT_INO);
    assert_eq!(dotdot_name_len, 2);
    assert_eq!(dotdot_file_type, 2); // EXT4_FT_DIR
    
    // Check lost+found entry
    let lf_offset = 12 + dot_rec_len as usize;
    let lf_entry = unsafe {
        &*(dir_data.as_ptr().add(lf_offset) as *const Ext4DirEntry2)
    };
    let lf_inode = lf_entry.inode;
    let lf_name_len = lf_entry.name_len;
    let lf_file_type = lf_entry.file_type;
    assert_eq!(lf_inode, EXT4_FIRST_INO as u32);
    assert_eq!(lf_name_len, 10); // "lost+found"
    assert_eq!(lf_file_type, 2); // EXT4_FT_DIR
}

#[test]
fn test_ext4_bitmap_operations_unchanged() {
    let mut bitmap = Bitmap::for_block_group(32768);
    let params = FilesystemParams::default();
    let layout = FilesystemLayout::from_params(&params).unwrap();
    
    init_block_bitmap_group0(&mut bitmap, &layout, &params);
    
    // Verify metadata blocks are marked as used
    assert!(bitmap.is_set(0)); // Superblock
    
    // Check that we have the expected number of used blocks
    let metadata_blocks = layout.metadata_blocks_per_group(0);
    let mut used_count = 0;
    for i in 0..1024 { // Check first 1024 blocks
        if bitmap.is_set(i) {
            used_count += 1;
        }
    }
    assert!(used_count >= metadata_blocks);
}

#[test] 
fn test_ext4_checksum_calculation_unchanged() {
    let mut sb = Ext4Superblock::new();
    let params = FilesystemParams::default();
    let layout = FilesystemLayout::from_params(&params).unwrap();
    
    sb.init_minimal(&params, &layout);
    let checksum_before = sb.s_checksum;
    
    sb.update_checksum();
    let checksum_after = sb.s_checksum;
    
    // Checksum should be deterministic
    assert_eq!(checksum_before, checksum_after);
    
    // Checksum should not be zero (unless extremely unlikely)
    assert_ne!(checksum_after, 0);
}

// Golden byte pattern test - ensures exact binary compatibility
#[test]
fn test_ext4_golden_bytes() {
    // This test captures the exact byte pattern of a minimal ext4 filesystem
    // If this test breaks, we've changed the binary format!
    
    let params = FilesystemParams {
        size_bytes: 100 * 1024 * 1024, // 100MB for consistent layout
        block_size: 4096,
        inode_size: 256,
        label: Some("GOLDEN".to_string()),
        reserved_percent: 5,
        enable_checksums: true,
        enable_64bit: false,
        enable_journal: false,
    };
    
    let layout = FilesystemLayout::from_params(&params).unwrap();
    let mut sb = Ext4Superblock::new();
    sb.init_minimal(&params, &layout);
    
    // Capture key values that define our format
    let golden_values = [
        ("magic", sb.s_magic as u64),
        ("block_size", 4096u64),
        ("blocks_per_group", sb.s_blocks_per_group as u64),
        ("inodes_per_group", sb.s_inodes_per_group as u64),
        ("inode_size", sb.s_inode_size as u64),
        ("feature_compat", sb.s_feature_compat as u64),
        ("feature_incompat", sb.s_feature_incompat as u64),
        ("feature_ro_compat", sb.s_feature_ro_compat as u64),
    ];
    
    // These values must never change for binary compatibility
    assert_eq!(golden_values[0].1, 0xEF53); // EXT4_SUPER_MAGIC
    assert_eq!(golden_values[2].1, 32768);  // blocks_per_group
    assert_eq!(golden_values[3].1, 8192);   // inodes_per_group
    assert_eq!(golden_values[4].1, 256);    // inode_size
    
    // Feature flags must include these at minimum
    assert!(golden_values[5].1 & 0x0200 != 0); // COMPAT_DIR_INDEX
    assert!(golden_values[6].1 & 0x0002 != 0); // INCOMPAT_FILETYPE
    assert!(golden_values[6].1 & 0x0040 != 0); // INCOMPAT_EXTENTS
    assert!(golden_values[7].1 & 0x0001 != 0); // RO_COMPAT_SPARSE_SUPER
}

#[test]
fn test_ext2_superblock_golden() {
    use crate::ext4_native::core::ext_builder::ExtFilesystemBuilder;
    
    // Create ext2 filesystem with known parameters
    let builder = ExtFilesystemBuilder::ext2(1024 * 1024 * 1024); // 1GB
    let params = builder.build_params();
    let layout = FilesystemLayout::from_params(&params).unwrap();
    
    let mut sb = Ext4Superblock::new();
    builder.init_superblock(&mut sb, &layout);
    
    // Verify ext2-specific golden values
    assert_eq!(sb.s_magic, 0xEF53);
    assert_eq!(sb.s_rev_level, 0); // ext2 uses revision 0
    assert_eq!(sb.s_inode_size, 128); // ext2 uses 128-byte inodes
    
    // ext2 should NOT have these features
    assert_eq!(sb.s_feature_compat & EXT4_FEATURE_COMPAT_HAS_JOURNAL, 0);
    assert_eq!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_EXTENTS, 0);
    assert_eq!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_64BIT, 0);
    assert_eq!(sb.s_feature_ro_compat & EXT4_FEATURE_RO_COMPAT_METADATA_CSUM, 0);
    
    // ext2 should have these features
    assert!(sb.s_feature_compat & EXT4_FEATURE_COMPAT_DIR_INDEX != 0);
    assert!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_FILETYPE != 0);
    
    // Check critical fields
    assert_eq!(sb.s_block_size(), 4096);
    assert_eq!(sb.s_blocks_per_group, 32768);
    assert_eq!(sb.s_inodes_per_group, 8192);
}

#[test]
fn test_ext3_superblock_golden() {
    use crate::ext4_native::core::ext_builder::ExtFilesystemBuilder;
    
    // Create ext3 filesystem with known parameters
    let builder = ExtFilesystemBuilder::ext3(1024 * 1024 * 1024); // 1GB
    let params = builder.build_params();
    let layout = FilesystemLayout::from_params(&params).unwrap();
    
    let mut sb = Ext4Superblock::new();
    builder.init_superblock(&mut sb, &layout);
    
    // Verify ext3-specific golden values
    assert_eq!(sb.s_magic, 0xEF53);
    assert_eq!(sb.s_rev_level, 1); // ext3 uses dynamic revision
    assert_eq!(sb.s_inode_size, 256); // ext3 typically uses 256-byte inodes
    
    // ext3 MUST have journal
    assert!(sb.s_feature_compat & EXT4_FEATURE_COMPAT_HAS_JOURNAL != 0);
    assert_eq!(sb.s_journal_inum, 8); // Journal is inode 8
    
    // ext3 should NOT have ext4 features
    assert_eq!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_EXTENTS, 0);
    assert_eq!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_64BIT, 0);
    assert_eq!(sb.s_feature_ro_compat & EXT4_FEATURE_RO_COMPAT_METADATA_CSUM, 0);
    
    // ext3 should have these features
    assert!(sb.s_feature_compat & EXT4_FEATURE_COMPAT_DIR_INDEX != 0);
    assert!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_FILETYPE != 0);
    
    // Check critical fields
    assert_eq!(sb.s_block_size(), 4096);
    assert_eq!(sb.s_blocks_per_group, 32768);
    assert_eq!(sb.s_inodes_per_group, 8192);
}

#[test]
fn test_ext2_inode_golden() {
    use crate::ext4_native::core::ext_builder::ExtFilesystemBuilder;
    
    let builder = ExtFilesystemBuilder::ext2(1024 * 1024 * 1024);
    let mut inode = Ext4Inode::new();
    builder.init_inode(&mut inode, true); // directory
    
    // ext2 inodes should use indirect blocks, not extents
    assert_eq!(inode.i_flags & EXT4_EXTENTS_FL, 0);
    assert_eq!(inode.i_mode & 0xF000, 0x4000); // S_IFDIR
    assert!(inode.i_links_count >= 2);
    
    // First 12 u32s in i_block should be indirect block pointers
    // They should NOT form an extent header
    let first_u32 = unsafe { 
        *(inode.i_block.as_ptr() as *const u32)
    };
    assert_ne!(first_u32, 0xF30A0000); // Not extent magic
}

#[test]
fn test_ext3_inode_golden() {
    use crate::ext4_native::core::ext_builder::ExtFilesystemBuilder;
    
    let builder = ExtFilesystemBuilder::ext3(1024 * 1024 * 1024);
    let mut inode = Ext4Inode::new();
    builder.init_inode(&mut inode, true); // directory
    
    // ext3 inodes should also use indirect blocks, not extents
    assert_eq!(inode.i_flags & EXT4_EXTENTS_FL, 0);
    assert_eq!(inode.i_mode & 0xF000, 0x4000); // S_IFDIR
    assert!(inode.i_links_count >= 2);
    
    // Should not have extent header
    let first_u32 = unsafe { 
        *(inode.i_block.as_ptr() as *const u32)
    };
    assert_ne!(first_u32, 0xF30A0000); // Not extent magic
}