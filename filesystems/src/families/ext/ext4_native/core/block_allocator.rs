// EXT4 Block Allocator
// Manages block allocation and deallocation for the filesystem

use std::collections::HashMap;
use super::{
    bitmap::Bitmap,
    types::{BlockNumber, GroupNumber, Ext4Result, Ext4Error},
    structures::{Ext4Superblock, Ext4GroupDesc},
};

/// Block allocation hint for optimizing allocations
#[derive(Debug, Clone)]
pub struct AllocationHint {
    /// Preferred block group for allocation
    pub group: Option<GroupNumber>,
    /// Goal block within the group
    pub goal_block: Option<BlockNumber>,
    /// Whether this is for a directory (affects spreading)
    pub is_directory: bool,
}

/// Block allocator manages free blocks across all block groups
pub struct BlockAllocator {
    /// Cached block bitmaps (group -> bitmap)
    bitmap_cache: HashMap<GroupNumber, Bitmap>,
    /// Modified bitmaps that need to be written back
    dirty_bitmaps: HashMap<GroupNumber, bool>,
    /// Superblock reference for metadata
    superblock: Ext4Superblock,
    /// Group descriptors
    group_descriptors: Vec<Ext4GroupDesc>,
    /// Blocks per group
    blocks_per_group: u32,
    /// Total number of groups
    num_groups: u32,
    /// Reserved blocks count
    reserved_blocks: u64,
}

impl BlockAllocator {
    /// Create a new block allocator
    pub fn new(
        superblock: Ext4Superblock,
        group_descriptors: Vec<Ext4GroupDesc>,
    ) -> Self {
        let blocks_per_group = superblock.s_blocks_per_group;
        let num_groups = group_descriptors.len() as u32;
        
        // Calculate reserved blocks (for root)
        let reserved_blocks = superblock.s_r_blocks_count_lo as u64 
            | ((superblock.s_r_blocks_count_hi as u64) << 32);
        
        Self {
            bitmap_cache: HashMap::new(),
            dirty_bitmaps: HashMap::new(),
            superblock,
            group_descriptors,
            blocks_per_group,
            num_groups,
            reserved_blocks,
        }
    }
    
    /// Allocate a single block with an optional hint
    pub fn allocate_block(&mut self, hint: Option<AllocationHint>) -> Ext4Result<BlockNumber> {
        let blocks = self.allocate_blocks(1, hint)?;
        Ok(blocks[0])
    }
    
    /// Allocate multiple contiguous blocks if possible
    pub fn allocate_blocks(
        &mut self,
        count: u32,
        hint: Option<AllocationHint>,
    ) -> Ext4Result<Vec<BlockNumber>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        
        // Check if we have enough free blocks
        let free_blocks = self.get_free_blocks_count();
        if free_blocks < count as u64 {
            return Err(Ext4Error::Io(format!(
                "Not enough free blocks: requested {}, available {}",
                count, free_blocks
            )));
        }
        
        // Determine starting group based on hint
        let start_group = hint.as_ref()
            .and_then(|h| h.group)
            .unwrap_or_else(|| self.find_best_group(hint.as_ref()));
        
        // Try to allocate from the preferred group first
        if let Ok(blocks) = self.try_allocate_in_group(start_group, count, hint.as_ref()) {
            return Ok(blocks);
        }
        
        // If that fails, try other groups
        for offset in 1..self.num_groups {
            let group = (start_group + offset) % self.num_groups;
            if let Ok(blocks) = self.try_allocate_in_group(group, count, None) {
                return Ok(blocks);
            }
        }
        
        // If we still can't allocate contiguously, allocate blocks individually
        self.allocate_scattered_blocks(count, hint)
    }
    
    /// Free a single block
    pub fn free_block(&mut self, block: BlockNumber) -> Ext4Result<()> {
        self.free_blocks(&[block])
    }
    
    /// Free multiple blocks
    pub fn free_blocks(&mut self, blocks: &[BlockNumber]) -> Ext4Result<()> {
        for &block in blocks {
            let group = self.block_to_group(block);
            let blocks_per_group = self.blocks_per_group;
            
            // Ensure bitmap is loaded
            self.ensure_bitmap_loaded(group);
            
            let block_in_group = (block % blocks_per_group as u64) as u32;
            
            // Check if block is already free
            let is_set = self.bitmap_cache.get(&group).unwrap().is_set(block_in_group);
            if !is_set {
                return Err(Ext4Error::Io(format!(
                    "Attempting to free already free block {}",
                    block
                )));
            }
            
            // Clear the bit
            self.bitmap_cache.get_mut(&group).unwrap().clear(block_in_group);
            self.dirty_bitmaps.insert(group, true);
            
            // Update group descriptor free block count
            let gd = &mut self.group_descriptors[group as usize];
            let current_free = gd.bg_free_blocks_count_lo as u32 
                | ((gd.bg_free_blocks_count_hi as u32) << 16);
            let new_free = current_free + 1;
            gd.bg_free_blocks_count_lo = (new_free & 0xFFFF) as u16;
            gd.bg_free_blocks_count_hi = ((new_free >> 16) & 0xFFFF) as u16;
        }
        
        // Update superblock free block count
        let current_free = self.superblock.s_free_blocks_count_lo as u64 
            | ((self.superblock.s_free_blocks_count_hi as u64) << 32);
        let new_free = current_free + blocks.len() as u64;
        self.superblock.s_free_blocks_count_lo = (new_free & 0xFFFFFFFF) as u32;
        self.superblock.s_free_blocks_count_hi = ((new_free >> 32) & 0xFFFFFFFF) as u32;
        
        Ok(())
    }
    
    /// Check if a specific block is allocated
    pub fn is_block_allocated(&mut self, block: BlockNumber) -> Ext4Result<bool> {
        let group = self.block_to_group(block);
        let blocks_per_group = self.blocks_per_group;
        
        self.ensure_bitmap_loaded(group);
        
        let block_in_group = (block % blocks_per_group as u64) as u32;
        Ok(self.bitmap_cache.get(&group).unwrap().is_set(block_in_group))
    }
    
    /// Get the total number of free blocks
    pub fn get_free_blocks_count(&self) -> u64 {
        self.superblock.s_free_blocks_count_lo as u64 
            | ((self.superblock.s_free_blocks_count_hi as u64) << 32)
    }
    
    /// Get available blocks for allocation (accounting for reserved blocks)
    pub fn get_available_blocks(&self, is_privileged: bool) -> u64 {
        let free_blocks = self.get_free_blocks_count();
        if is_privileged {
            // Root/privileged users can use reserved blocks
            free_blocks
        } else {
            // Regular users cannot use reserved blocks
            free_blocks.saturating_sub(self.reserved_blocks)
        }
    }
    
    /// Get the number of reserved blocks
    pub fn get_reserved_blocks(&self) -> u64 {
        self.reserved_blocks
    }
    
    /// Get modified bitmaps that need to be written back
    pub fn get_dirty_bitmaps(&self) -> Vec<(GroupNumber, &Bitmap)> {
        self.dirty_bitmaps
            .iter()
            .filter_map(|(&group, &dirty)| {
                if dirty {
                    self.bitmap_cache.get(&group).map(|bitmap| (group, bitmap))
                } else {
                    None
                }
            })
            .collect()
    }
    
    /// Mark all bitmaps as clean (after writing to disk)
    pub fn mark_bitmaps_clean(&mut self) {
        self.dirty_bitmaps.clear();
    }
    
    // Private helper methods
    
    /// Convert a block number to its group number
    fn block_to_group(&self, block: BlockNumber) -> GroupNumber {
        (block / self.blocks_per_group as u64) as GroupNumber
    }
    
    /// Find the best group for allocation based on hint
    fn find_best_group(&self, hint: Option<&AllocationHint>) -> GroupNumber {
        // For directories, try to spread them across groups
        if hint.map_or(false, |h| h.is_directory) {
            // Simple round-robin for directory spreading
            // In production, use Orlov allocator or similar
            let mut best_group = 0;
            let mut max_free = 0;
            
            for (i, gd) in self.group_descriptors.iter().enumerate() {
                let free_blocks = gd.bg_free_blocks_count_lo as u32 
                    | ((gd.bg_free_blocks_count_hi as u32) << 16);
                if free_blocks > max_free {
                    max_free = free_blocks;
                    best_group = i as GroupNumber;
                }
            }
            
            best_group
        } else {
            // For regular files, prefer groups with more free blocks
            self.group_descriptors
                .iter()
                .enumerate()
                .max_by_key(|(_, gd)| {
                    gd.bg_free_blocks_count_lo as u32 
                        | ((gd.bg_free_blocks_count_hi as u32) << 16)
                })
                .map(|(i, _)| i as GroupNumber)
                .unwrap_or(0)
        }
    }
    
    /// Try to allocate blocks in a specific group
    fn try_allocate_in_group(
        &mut self,
        group: GroupNumber,
        count: u32,
        hint: Option<&AllocationHint>,
    ) -> Ext4Result<Vec<BlockNumber>> {
        // First check if group has enough free blocks
        let free_blocks = {
            let gd = &self.group_descriptors[group as usize];
            gd.bg_free_blocks_count_lo as u32 
                | ((gd.bg_free_blocks_count_hi as u32) << 16)
        };
        
        if free_blocks < count {
            return Err(Ext4Error::Io("Not enough blocks in group".to_string()));
        }
        
        // Determine starting position
        let blocks_per_group = self.blocks_per_group;
        let start_bit = hint
            .and_then(|h| h.goal_block)
            .map(|goal| (goal % blocks_per_group as u64) as u32)
            .unwrap_or(0);
        
        // Ensure bitmap is loaded
        self.ensure_bitmap_loaded(group);
        
        // Try to find contiguous blocks
        let first_bit = self.bitmap_cache.get(&group).unwrap()
            .find_contiguous_clear(start_bit, count);
        
        if let Some(first_bit) = first_bit {
            let mut blocks = Vec::with_capacity(count as usize);
            let base_block = group as u64 * blocks_per_group as u64;
            
            // Update the bitmap
            let bitmap = self.bitmap_cache.get_mut(&group).unwrap();
            for i in 0..count {
                let bit = first_bit + i;
                bitmap.set(bit);
                blocks.push(base_block + bit as u64);
            }
            
            self.dirty_bitmaps.insert(group, true);
            
            // Update group descriptor
            let gd = &mut self.group_descriptors[group as usize];
            let new_free = free_blocks - count;
            gd.bg_free_blocks_count_lo = (new_free & 0xFFFF) as u16;
            gd.bg_free_blocks_count_hi = ((new_free >> 16) & 0xFFFF) as u16;
            
            // Update superblock
            let current_free = self.superblock.s_free_blocks_count_lo as u64 
                | ((self.superblock.s_free_blocks_count_hi as u64) << 32);
            let new_free = current_free - count as u64;
            self.superblock.s_free_blocks_count_lo = (new_free & 0xFFFFFFFF) as u32;
            self.superblock.s_free_blocks_count_hi = ((new_free >> 32) & 0xFFFFFFFF) as u32;
            
            return Ok(blocks);
        }
        
        Err(Ext4Error::Io("Cannot find contiguous blocks in group".to_string()))
    }
    
    /// Allocate blocks scattered across groups
    fn allocate_scattered_blocks(
        &mut self,
        count: u32,
        hint: Option<AllocationHint>,
    ) -> Ext4Result<Vec<BlockNumber>> {
        let mut blocks = Vec::with_capacity(count as usize);
        
        for _ in 0..count {
            let block = self.allocate_block(hint.clone())?;
            blocks.push(block);
        }
        
        Ok(blocks)
    }
    
    /// Ensure a bitmap is loaded for a group
    fn ensure_bitmap_loaded(&mut self, group: GroupNumber) {
        if !self.bitmap_cache.contains_key(&group) {
            let bitmap = Bitmap::for_block_group(self.blocks_per_group);
            self.bitmap_cache.insert(group, bitmap);
        }
    }
}