// Configuration for different ext filesystem versions
// This allows ext4_native to format ext2/ext3/ext4 without breaking existing code

use crate::ext4_native::core::constants::*;

#[derive(Debug, Clone)]
pub struct ExtConfig {
    pub version: ExtVersion,
    pub has_journal: bool,
    pub use_extents: bool,
    pub use_64bit: bool,
    pub use_metadata_csum: bool,
    pub use_flex_bg: bool,
    pub journal_blocks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtVersion {
    Ext2,
    Ext3,
    Ext4,
}

impl ExtConfig {
    /// Get configuration for ext4 (current default)
    pub fn ext4(device_size: u64) -> Self {
        Self {
            version: ExtVersion::Ext4,
            has_journal: false,  // Currently disabled in our implementation
            use_extents: true,
            use_64bit: device_size > 16 * 1024 * 1024 * 1024, // >16GB
            use_metadata_csum: true,
            use_flex_bg: true,
            journal_blocks: 0,  // Would be 32768 (128MB) if enabled
        }
    }
    
    /// Get configuration for ext2
    pub fn ext2() -> Self {
        Self {
            version: ExtVersion::Ext2,
            has_journal: false,
            use_extents: false,
            use_64bit: false,
            use_metadata_csum: false,
            use_flex_bg: false,
            journal_blocks: 0,
        }
    }
    
    /// Get configuration for ext3
    pub fn ext3() -> Self {
        Self {
            version: ExtVersion::Ext3,
            has_journal: true,
            use_extents: false,
            use_64bit: false,
            use_metadata_csum: false,
            use_flex_bg: false,
            journal_blocks: 32768,  // 128MB journal
        }
    }
    
    /// Get compatible feature flags
    pub fn get_compat_features(&self) -> u32 {
        let mut features = 0;
        
        // All versions support these
        features |= EXT4_FEATURE_COMPAT_DIR_INDEX;
        features |= EXT4_FEATURE_COMPAT_RESIZE_INODE;
        
        if self.has_journal {
            features |= EXT4_FEATURE_COMPAT_HAS_JOURNAL;
        }
        
        features
    }
    
    /// Get incompatible feature flags
    pub fn get_incompat_features(&self) -> u32 {
        let mut features = 0;
        
        // All versions need this
        features |= EXT4_FEATURE_INCOMPAT_FILETYPE;
        
        if self.use_extents {
            features |= EXT4_FEATURE_INCOMPAT_EXTENTS;
        }
        
        if self.use_64bit {
            features |= EXT4_FEATURE_INCOMPAT_64BIT;
        }
        
        if self.use_flex_bg {
            features |= EXT4_FEATURE_INCOMPAT_FLEX_BG;
        }
        
        features
    }
    
    /// Get read-only compatible features
    pub fn get_ro_compat_features(&self) -> u32 {
        let mut features = 0;
        
        // Basic features all versions can have
        features |= EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER;
        features |= EXT4_FEATURE_RO_COMPAT_LARGE_FILE;
        
        if self.use_metadata_csum {
            features |= EXT4_FEATURE_RO_COMPAT_METADATA_CSUM;
        }
        
        features
    }
    
    /// Check if this config needs extent header in inode
    pub fn needs_extent_header(&self) -> bool {
        self.use_extents
    }
    
    /// Get the filesystem revision level
    pub fn get_revision(&self) -> u32 {
        match self.version {
            ExtVersion::Ext2 => 0,  // EXT2_GOOD_OLD_REV
            ExtVersion::Ext3 | ExtVersion::Ext4 => 1,  // EXT2_DYNAMIC_REV
        }
    }
}