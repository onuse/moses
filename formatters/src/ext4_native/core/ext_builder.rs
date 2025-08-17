// Builder pattern for creating different ext filesystem versions
// This allows us to reuse the existing ext4 code with different parameters
// WITHOUT modifying the existing structures or formatting logic

use super::{
    structures::*,
    types::{FilesystemParams, FilesystemLayout},
    constants::*,
    ext_config::{ExtConfig, ExtVersion},
};

/// Builder for creating ext filesystems with version-specific behavior
pub struct ExtFilesystemBuilder {
    config: ExtConfig,
    device_size: u64,
    block_size: u32,
    label: Option<String>,
}

impl ExtFilesystemBuilder {
    /// Create a new builder for ext2
    pub fn ext2(device_size: u64) -> Self {
        Self {
            config: ExtConfig::ext2(),
            device_size,
            block_size: 4096,
            label: None,
        }
    }
    
    /// Create a new builder for ext3
    pub fn ext3(device_size: u64) -> Self {
        Self {
            config: ExtConfig::ext3(),
            device_size,
            block_size: 4096,
            label: None,
        }
    }
    
    /// Create a new builder for ext4
    pub fn ext4(device_size: u64) -> Self {
        Self {
            config: ExtConfig::ext4(device_size),
            device_size,
            block_size: 4096,
            label: None,
        }
    }
    
    /// Set the block size
    pub fn block_size(mut self, size: u32) -> Self {
        self.block_size = size;
        self
    }
    
    /// Set the volume label
    pub fn label(mut self, label: String) -> Self {
        self.label = Some(label);
        self
    }
    
    /// Build FilesystemParams appropriate for this ext version
    pub fn build_params(&self) -> FilesystemParams {
        FilesystemParams {
            size_bytes: self.device_size,
            block_size: self.block_size,
            inode_size: if self.config.version == ExtVersion::Ext2 { 128 } else { 256 },
            label: self.label.clone(),
            reserved_percent: 5,
            enable_checksums: self.config.use_metadata_csum,
            enable_64bit: self.config.use_64bit,
            enable_journal: self.config.has_journal,
        }
    }
    
    /// Initialize a superblock for this ext version
    pub fn init_superblock(&self, sb: &mut Ext4Superblock, layout: &FilesystemLayout) {
        let params = self.build_params();
        sb.init_minimal(&params, layout);
        
        // Override features based on version
        sb.s_feature_compat = self.config.get_compat_features();
        sb.s_feature_incompat = self.config.get_incompat_features();
        sb.s_feature_ro_compat = self.config.get_ro_compat_features();
        sb.s_rev_level = self.config.get_revision();
        
        // Adjust other fields for ext2/ext3
        if self.config.version == ExtVersion::Ext2 {
            sb.s_inode_size = 128;
            sb.s_desc_size = 32;  // ext2 uses smaller descriptors
        }
        
        if self.config.has_journal {
            sb.s_journal_inum = 8;  // Journal inode
            sb.s_journal_dev = 0;   // Same device
        }
    }
    
    /// Initialize an inode for this ext version
    pub fn init_inode(&self, inode: &mut Ext4Inode, is_directory: bool) {
        let params = self.build_params();
        
        if is_directory {
            inode.init_root_dir(&params);
        } else {
            // Initialize as regular file
            inode.i_mode = 0x81A4;  // S_IFREG | 0644
            inode.i_links_count = 1;
        }
        
        // For ext2/ext3, don't use extents
        if !self.config.use_extents {
            // Clear the extent flag and extent header
            inode.i_flags &= !EXT4_EXTENTS_FL;
            // Zero out the block array (no extent header)
            inode.i_block = [0; 15];
        }
    }
    
    /// Check if this version needs a journal
    pub fn needs_journal(&self) -> bool {
        self.config.has_journal
    }
    
    /// Get the number of blocks to reserve for journal
    pub fn journal_blocks(&self) -> u32 {
        self.config.journal_blocks
    }
}