// EXT4 filesystem validator
// Validates structures and checksums

use crate::families::ext::ext4_native::core::types::*;

pub struct Ext4Validator {
    verbose: bool,
}

impl Ext4Validator {
    pub fn new() -> Self {
        Self { verbose: false }
    }
    
    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }
    
    /// Validate filesystem parameters
    pub fn validate_params(&self, params: &FilesystemParams) -> Ext4Result<()> {
        use crate::families::ext::ext4_native::core::constants::*;
        
        // Check block size
        if params.block_size < EXT4_MIN_BLOCK_SIZE || params.block_size > EXT4_MAX_BLOCK_SIZE {
            return Err(Ext4Error::InvalidParameters(
                format!("Block size must be between {} and {}", 
                        EXT4_MIN_BLOCK_SIZE, EXT4_MAX_BLOCK_SIZE)
            ));
        }
        
        // Block size must be power of 2
        if !params.block_size.is_power_of_two() {
            return Err(Ext4Error::InvalidParameters(
                "Block size must be a power of 2".to_string()
            ));
        }
        
        // Check inode size
        if params.inode_size < 128 || params.inode_size > params.block_size as u16 {
            return Err(Ext4Error::InvalidParameters(
                "Invalid inode size".to_string()
            ));
        }
        
        // Inode size must be power of 2
        if !params.inode_size.is_power_of_two() {
            return Err(Ext4Error::InvalidParameters(
                "Inode size must be a power of 2".to_string()
            ));
        }
        
        // Check label
        if let Some(ref label) = params.label {
            if label.len() > 16 {
                return Err(Ext4Error::InvalidParameters(
                    "Label must be 16 characters or less".to_string()
                ));
            }
        }
        
        // Check device size
        let min_size = params.block_size as u64 * 64; // Minimum 64 blocks
        if params.size_bytes < min_size {
            return Err(Ext4Error::DeviceTooSmall {
                required: min_size,
                actual: params.size_bytes,
            });
        }
        
        Ok(())
    }
    
    /// Validate filesystem layout
    pub fn validate_layout(&self, layout: &FilesystemLayout) -> Ext4Result<()> {
        if layout.num_groups == 0 {
            return Err(Ext4Error::InvalidParameters(
                "No block groups".to_string()
            ));
        }
        
        if layout.total_blocks == 0 {
            return Err(Ext4Error::InvalidParameters(
                "No blocks".to_string()
            ));
        }
        
        // Check that metadata doesn't exceed group size
        let metadata_blocks = layout.metadata_blocks_per_group(0);
        if metadata_blocks >= layout.blocks_per_group {
            return Err(Ext4Error::InvalidParameters(
                "Metadata exceeds group size".to_string()
            ));
        }
        
        Ok(())
    }
}

impl Default for Ext4Validator {
    fn default() -> Self {
        Self::new()
    }
}