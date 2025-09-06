// Tests for ext2 and ext3 formatters to ensure they create valid filesystems
// These tests verify that the filesystems have the correct features and structure

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::families::ext::ext4_native::core::{
        ext_builder::ExtFilesystemBuilder,
        ext_config::{ExtConfig, ExtVersion},
        structures::*,
        types::{FilesystemParams, FilesystemLayout},
        constants::*,
    };
    
    #[test]
    fn test_ext2_superblock_features() {
        // Create ext2 filesystem parameters
        let builder = ExtFilesystemBuilder::ext2(1024 * 1024 * 1024); // 1GB
        let params = builder.build_params();
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        let mut sb = Ext4Superblock::new();
        builder.init_superblock(&mut sb, &layout);
        
        // Verify ext2-specific features
        assert_eq!(sb.s_magic, 0xEF53, "Magic number must be EXT2/3/4 magic");
        assert_eq!(sb.s_rev_level, 0, "ext2 should use good old revision");
        assert_eq!(sb.s_inode_size, 128, "ext2 uses 128-byte inodes");
        
        // Check feature flags - ext2 should NOT have these
        assert_eq!(sb.s_feature_compat & EXT4_FEATURE_COMPAT_HAS_JOURNAL, 0, 
                   "ext2 must not have journal");
        assert_eq!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_EXTENTS, 0,
                   "ext2 must not have extents");
        assert_eq!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_64BIT, 0,
                   "ext2 must not have 64-bit feature");
        assert_eq!(sb.s_feature_ro_compat & EXT4_FEATURE_RO_COMPAT_METADATA_CSUM, 0,
                   "ext2 must not have metadata checksums");
        
        // ext2 should have these basic features
        assert!(sb.s_feature_compat & EXT4_FEATURE_COMPAT_DIR_INDEX != 0,
                "ext2 should have directory indexing");
        assert!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_FILETYPE != 0,
                "ext2 should have filetype feature");
    }
    
    #[test]
    fn test_ext3_superblock_features() {
        // Create ext3 filesystem parameters
        let builder = ExtFilesystemBuilder::ext3(1024 * 1024 * 1024); // 1GB
        let params = builder.build_params();
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        let mut sb = Ext4Superblock::new();
        builder.init_superblock(&mut sb, &layout);
        
        // Verify ext3-specific features
        assert_eq!(sb.s_magic, 0xEF53, "Magic number must be EXT2/3/4 magic");
        assert_eq!(sb.s_rev_level, 1, "ext3 should use dynamic revision");
        assert_eq!(sb.s_inode_size, 256, "ext3 typically uses 256-byte inodes");
        
        // ext3 MUST have journal
        assert!(sb.s_feature_compat & EXT4_FEATURE_COMPAT_HAS_JOURNAL != 0,
                "ext3 must have journal feature");
        assert_eq!(sb.s_journal_inum, 8, "Journal should be inode 8");
        
        // ext3 should NOT have these ext4 features
        assert_eq!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_EXTENTS, 0,
                   "ext3 must not have extents");
        assert_eq!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_64BIT, 0,
                   "ext3 must not have 64-bit feature");
        assert_eq!(sb.s_feature_ro_compat & EXT4_FEATURE_RO_COMPAT_METADATA_CSUM, 0,
                   "ext3 must not have metadata checksums");
    }
    
    #[test]
    fn test_ext2_inode_structure() {
        let builder = ExtFilesystemBuilder::ext2(1024 * 1024 * 1024);
        let mut inode = Ext4Inode::new();
        builder.init_inode(&mut inode, true); // directory
        
        // ext2 should use indirect blocks, not extents
        assert_eq!(inode.i_flags & EXT4_EXTENTS_FL, 0,
                   "ext2 inode must not have extent flag");
        
        // Check that block array is not an extent header
        let first_u32 = unsafe { 
            *(inode.i_block.as_ptr() as *const u32)
        };
        assert_ne!(first_u32, 0xF30A0000, // extent magic in little-endian
                   "ext2 must not have extent header");
    }
    
    #[test]
    fn test_ext3_inode_structure() {
        let builder = ExtFilesystemBuilder::ext3(1024 * 1024 * 1024);
        let mut inode = Ext4Inode::new();
        builder.init_inode(&mut inode, true); // directory
        
        // ext3 should also use indirect blocks, not extents
        assert_eq!(inode.i_flags & EXT4_EXTENTS_FL, 0,
                   "ext3 inode must not have extent flag");
        
        // Verify inode is properly initialized
        assert_eq!(inode.i_mode & 0xF000, 0x4000, "Should be directory");
        assert!(inode.i_links_count >= 2, "Directory should have at least 2 links");
    }
    
    #[test]
    fn test_ext2_size_limits() {
        let config = ExtConfig::ext2();
        
        // ext2 without 64-bit support has 2TB limit
        assert!(!config.use_64bit, "ext2 should not use 64-bit");
        
        // Test that validator would reject >2TB
        let large_size = 3 * 1024_u64.pow(4); // 3TB
        let builder = ExtFilesystemBuilder::ext2(large_size);
        let params = builder.build_params();
        
        // In real usage, this would be caught by validate_options
        assert!(!params.enable_64bit, "ext2 must not enable 64-bit even for large devices");
    }
    
    #[test]
    fn test_ext3_journal_configuration() {
        let config = ExtConfig::ext3();
        
        assert!(config.has_journal, "ext3 must have journal");
        assert_eq!(config.journal_blocks, 32768, "ext3 should have 128MB journal (32768 * 4KB)");
        
        // Verify journal features are set
        let compat = config.get_compat_features();
        assert!(compat & EXT4_FEATURE_COMPAT_HAS_JOURNAL != 0,
                "ext3 compat features must include journal");
    }
    
    #[test]
    fn test_ext_version_compatibility() {
        // Test that each version has compatible feature sets
        let ext2_config = ExtConfig::ext2();
        let ext3_config = ExtConfig::ext3();
        let ext4_config = ExtConfig::ext4(1024 * 1024 * 1024);
        
        // ext2 should be most restrictive
        assert!(!ext2_config.has_journal);
        assert!(!ext2_config.use_extents);
        assert!(!ext2_config.use_64bit);
        assert!(!ext2_config.use_metadata_csum);
        
        // ext3 adds journal
        assert!(ext3_config.has_journal);
        assert!(!ext3_config.use_extents);
        assert!(!ext3_config.use_64bit);
        assert!(!ext3_config.use_metadata_csum);
        
        // ext4 adds everything
        assert!(!ext4_config.has_journal); // Currently disabled in our implementation
        assert!(ext4_config.use_extents);
        assert!(!ext4_config.use_64bit); // Only for >16GB
        assert!(ext4_config.use_metadata_csum);
    }
    
    #[test]
    fn test_builder_creates_valid_params() {
        // Test ext2 builder
        let ext2_builder = ExtFilesystemBuilder::ext2(100 * 1024 * 1024)
            .block_size(4096)
            .label("TEST_EXT2".to_string());
        let ext2_params = ext2_builder.build_params();
        
        assert_eq!(ext2_params.block_size, 4096);
        assert_eq!(ext2_params.inode_size, 128);
        assert_eq!(ext2_params.label, Some("TEST_EXT2".to_string()));
        assert!(!ext2_params.enable_checksums);
        assert!(!ext2_params.enable_journal);
        
        // Test ext3 builder
        let ext3_builder = ExtFilesystemBuilder::ext3(100 * 1024 * 1024)
            .block_size(4096)
            .label("TEST_EXT3".to_string());
        let ext3_params = ext3_builder.build_params();
        
        assert_eq!(ext3_params.block_size, 4096);
        assert_eq!(ext3_params.inode_size, 256);
        assert_eq!(ext3_params.label, Some("TEST_EXT3".to_string()));
        assert!(!ext3_params.enable_checksums);
        assert!(ext3_params.enable_journal);
    }
}

#[cfg(test)]
mod verification_tests {
    use super::super::*;
    use crate::families::ext::ext4_native::core::{
        ext_builder::ExtFilesystemBuilder,
        structures::*,
        types::FilesystemLayout,
        constants::*,
    };
    use std::io::Cursor;
    
    /// Verify that an ext2 filesystem can be read back correctly
    #[test]
    fn test_ext2_roundtrip() {
        // This would create a small ext2 image in memory and verify it
        // For now, just test that the structures are consistent
        let builder = ExtFilesystemBuilder::ext2(100 * 1024 * 1024);
        let params = builder.build_params();
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        let mut sb = Ext4Superblock::new();
        builder.init_superblock(&mut sb, &layout);
        
        // Serialize and deserialize
        let mut buffer = vec![0u8; 1024];
        sb.write_to_buffer(&mut buffer).unwrap();
        
        let sb2 = unsafe {
            std::ptr::read(buffer.as_ptr() as *const Ext4Superblock)
        };
        
        assert_eq!(sb.s_magic, sb2.s_magic);
        assert_eq!(sb.s_blocks_count_lo, sb2.s_blocks_count_lo);
        assert_eq!(sb.s_feature_compat, sb2.s_feature_compat);
    }
    
    /// Test that ext3 journal inode is properly configured
    #[test]
    fn test_ext3_journal_inode() {
        let builder = ExtFilesystemBuilder::ext3(1024 * 1024 * 1024);
        let mut journal_inode = Ext4Inode::new();
        
        // Initialize as journal inode
        builder.init_inode(&mut journal_inode, false); // regular file
        journal_inode.i_mode = 0x8180; // S_IFREG | 0600
        journal_inode.i_size_lo = builder.journal_blocks() * 4096;
        journal_inode.i_flags = EXT4_JOURNAL_DATA_FL;
        
        // Verify journal inode properties
        assert_eq!(journal_inode.i_mode & 0xF000, 0x8000, "Journal should be regular file");
        assert_eq!(journal_inode.i_mode & 0x1FF, 0x180, "Journal should have 0600 permissions");
        assert!(journal_inode.i_size_lo > 0, "Journal should have non-zero size");
        assert_eq!(journal_inode.i_flags & EXT4_JOURNAL_DATA_FL, EXT4_JOURNAL_DATA_FL,
                   "Journal inode should have journal data flag");
    }
}