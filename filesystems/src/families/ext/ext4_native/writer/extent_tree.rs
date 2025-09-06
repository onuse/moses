// EXT4 Extent Tree Operations
// Handles reading and modifying extent trees for file block management

use crate::families::ext::ext4_native::core::{
    structures::*,
    types::*,
    constants::*,
};
use moses_core::MosesError;
use std::mem;

/// Maximum depth of extent tree
const EXT4_MAX_EXTENT_DEPTH: u16 = 5;

/// Extent tree operations for EXT4 Writer
impl super::Ext4Writer {
    /// Parse the extent tree in an inode to count blocks
    pub(super) fn count_extent_blocks(&self, inode: &Ext4Inode) -> Result<u64, MosesError> {
        if inode.i_flags & EXT4_EXTENTS_FL == 0 {
            return Ok(0);
        }
        
        // Get extent header from i_block
        let header = unsafe {
            &*(inode.i_block.as_ptr() as *const Ext4ExtentHeader)
        };
        
        // Validate magic number
        if header.eh_magic != EXT4_EXTENT_MAGIC {
            return Err(MosesError::Other("Invalid extent magic".to_string()));
        }
        
        let mut total_blocks = 0u64;
        
        if header.eh_depth == 0 {
            // Leaf node - contains extents
            let extents = unsafe {
                let ptr = (inode.i_block.as_ptr() as *const u8)
                    .add(mem::size_of::<Ext4ExtentHeader>()) as *const Ext4Extent;
                std::slice::from_raw_parts(ptr, header.eh_entries as usize)
            };
            
            for extent in extents {
                total_blocks += extent.ee_len as u64;
            }
        } else {
            // Index node - would need to recursively traverse
            // For now, return an error as full implementation requires disk access
            return Err(MosesError::Other("Extent index traversal not yet implemented".to_string()));
        }
        
        Ok(total_blocks)
    }
    
    /// Get all blocks from extent tree
    pub(super) fn get_extent_blocks(&self, inode: &Ext4Inode) -> Result<Vec<BlockNumber>, MosesError> {
        if inode.i_flags & EXT4_EXTENTS_FL == 0 {
            return Ok(Vec::new());
        }
        
        let header = unsafe {
            &*(inode.i_block.as_ptr() as *const Ext4ExtentHeader)
        };
        
        if header.eh_magic != EXT4_EXTENT_MAGIC {
            return Err(MosesError::Other("Invalid extent magic".to_string()));
        }
        
        let mut blocks = Vec::new();
        
        if header.eh_depth == 0 {
            // Leaf node
            let extents = unsafe {
                let ptr = (inode.i_block.as_ptr() as *const u8)
                    .add(mem::size_of::<Ext4ExtentHeader>()) as *const Ext4Extent;
                std::slice::from_raw_parts(ptr, header.eh_entries as usize)
            };
            
            for extent in extents {
                let start_block = extent.ee_start_lo as u64 | ((extent.ee_start_hi as u64) << 32);
                for i in 0..extent.ee_len {
                    blocks.push(start_block + i as u64);
                }
            }
        } else {
            return Err(MosesError::Other("Extent index traversal not yet implemented".to_string()));
        }
        
        Ok(blocks)
    }
    
    /// Add new extents to an inode's extent tree
    pub(super) fn add_extents(
        &mut self,
        inode: &mut Ext4Inode,
        logical_start: u32,
        blocks: &[BlockNumber],
    ) -> Result<(), MosesError> {
        if blocks.is_empty() {
            return Ok(());
        }
        
        // Ensure extent flag is set
        inode.i_flags |= EXT4_EXTENTS_FL;
        
        let header = unsafe {
            &mut *(inode.i_block.as_mut_ptr() as *mut Ext4ExtentHeader)
        };
        
        // Initialize header if needed
        if header.eh_magic != EXT4_EXTENT_MAGIC {
            header.eh_magic = EXT4_EXTENT_MAGIC;
            header.eh_entries = 0;
            header.eh_max = 4; // Can hold 4 extents in inode
            header.eh_depth = 0;
            header.eh_generation = 0;
        }
        
        if header.eh_depth != 0 {
            return Err(MosesError::Other("Extent index modification not yet implemented".to_string()));
        }
        
        // Check if we have space for new extent
        if header.eh_entries >= header.eh_max {
            return Err(MosesError::Other("Extent tree split not yet implemented".to_string()));
        }
        
        // Try to merge with existing extents or add new one
        let extents = unsafe {
            let ptr = (inode.i_block.as_mut_ptr() as *mut u8)
                .add(mem::size_of::<Ext4ExtentHeader>()) as *mut Ext4Extent;
            std::slice::from_raw_parts_mut(ptr, header.eh_max as usize)
        };
        
        // For simplicity, just append new extent (in production, should merge adjacent blocks)
        let new_extent_idx = header.eh_entries as usize;
        extents[new_extent_idx] = Ext4Extent::new(
            logical_start,
            blocks[0],
            blocks.len() as u16,
        );
        
        header.eh_entries += 1;
        
        Ok(())
    }
    
    /// Get the last allocated block from extent tree
    pub(super) fn get_last_extent_block(&self, inode: &Ext4Inode) -> Option<BlockNumber> {
        if inode.i_flags & EXT4_EXTENTS_FL == 0 {
            return None;
        }
        
        let header = unsafe {
            &*(inode.i_block.as_ptr() as *const Ext4ExtentHeader)
        };
        
        if header.eh_magic != EXT4_EXTENT_MAGIC || header.eh_entries == 0 {
            return None;
        }
        
        if header.eh_depth == 0 {
            // Leaf node
            let extents = unsafe {
                let ptr = (inode.i_block.as_ptr() as *const u8)
                    .add(mem::size_of::<Ext4ExtentHeader>()) as *const Ext4Extent;
                std::slice::from_raw_parts(ptr, header.eh_entries as usize)
            };
            
            // Find extent with highest logical block
            let last_extent = extents.iter()
                .max_by_key(|e| e.ee_block)?;
            
            let physical_start = last_extent.ee_start_lo as u64 
                | ((last_extent.ee_start_hi as u64) << 32);
            
            Some(physical_start + last_extent.ee_len as u64 - 1)
        } else {
            None // Index traversal not implemented
        }
    }
    
    /// Initialize extent tree for a directory
    pub(super) fn init_dir_extent_tree(
        &mut self,
        inode: &mut Ext4Inode,
        first_block: BlockNumber,
    ) -> Result<(), MosesError> {
        // Set extent flag
        inode.i_flags |= EXT4_EXTENTS_FL;
        
        // Clear i_block
        inode.i_block = [0u32; 15];
        
        // Initialize extent header
        let header = unsafe {
            &mut *(inode.i_block.as_mut_ptr() as *mut Ext4ExtentHeader)
        };
        
        header.eh_magic = EXT4_EXTENT_MAGIC;
        header.eh_entries = 1; // One extent for directory
        header.eh_max = 4;
        header.eh_depth = 0;
        header.eh_generation = 0;
        
        // Add first extent
        let extents = unsafe {
            let ptr = (inode.i_block.as_mut_ptr() as *mut u8)
                .add(mem::size_of::<Ext4ExtentHeader>()) as *mut Ext4Extent;
            std::slice::from_raw_parts_mut(ptr, 4)
        };
        
        extents[0] = Ext4Extent::new(0, first_block, 1);
        
        Ok(())
    }
}