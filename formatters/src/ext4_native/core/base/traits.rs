// Trait definitions for ext filesystem components
// These traits allow different ext versions to customize behavior while sharing code

use moses_core::MosesError;

/// Parameters that vary between ext2/ext3/ext4
pub trait ExtParams {
    fn has_journal(&self) -> bool;
    fn use_extents(&self) -> bool;
    fn use_64bit(&self) -> bool;
    fn use_metadata_csum(&self) -> bool;
    fn get_compat_features(&self) -> u32;
    fn get_incompat_features(&self) -> u32;
    fn get_ro_compat_features(&self) -> u32;
}

/// Filesystem layout calculations
pub trait ExtLayout {
    fn total_blocks(&self) -> u64;
    fn blocks_per_group(&self) -> u32;
    fn num_groups(&self) -> u32;
    fn inodes_per_group(&self) -> u32;
    fn metadata_blocks_per_group(&self, group: u32) -> u32;
}

/// Superblock operations
pub trait ExtSuperblock: Send + Sync {
    fn init(&mut self, params: &dyn ExtParams, layout: &dyn ExtLayout);
    fn set_volume_label(&mut self, label: &str);
    fn update_free_counts(&mut self, free_blocks: u64, free_inodes: u32);
    fn update_checksum(&mut self);
    fn serialize(&self, buffer: &mut [u8]) -> Result<(), MosesError>;
}

/// Inode operations
pub trait ExtInode: Send + Sync {
    fn init_directory(&mut self, mode: u16);
    fn init_file(&mut self, mode: u16);
    fn set_size(&mut self, size: u64);
    fn set_links_count(&mut self, count: u16);
    fn set_blocks(&mut self, block_data: &[u8]);
    fn update_checksum(&mut self, inode_num: u32, sb_uuid: &[u8; 16]);
    fn serialize(&self, buffer: &mut [u8]) -> Result<(), MosesError>;
}

/// Group descriptor operations
pub trait ExtGroupDesc: Send + Sync {
    fn init(&mut self, group: u32, layout: &dyn ExtLayout);
    fn set_block_bitmap(&mut self, block: u64);
    fn set_inode_bitmap(&mut self, block: u64);
    fn set_inode_table(&mut self, block: u64);
    fn set_free_counts(&mut self, free_blocks: u32, free_inodes: u32);
    fn update_checksum(&mut self, group: u32, sb_uuid: &[u8; 16]);
    fn serialize(&self, buffer: &mut [u8]) -> Result<(), MosesError>;
}