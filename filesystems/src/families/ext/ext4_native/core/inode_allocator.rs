// EXT4 Inode Allocator
// Manages inode allocation and deallocation for the filesystem

use std::collections::HashMap;
use super::{
    bitmap::Bitmap,
    types::{InodeNumber, GroupNumber, Ext4Result, Ext4Error},
    structures::{Ext4Superblock, Ext4GroupDesc, Ext4Inode},
    constants::*,
};

/// Inode allocation strategy
#[derive(Debug, Clone, Copy)]
pub enum AllocationStrategy {
    /// Spread directories across groups (Orlov allocator)
    DirectorySpread,
    /// Keep files close to parent directory
    LocalityGroup,
    /// Use first available inode
    FirstAvailable,
}

/// Inode allocator manages free inodes across all block groups
pub struct InodeAllocator {
    /// Cached inode bitmaps (group -> bitmap)
    bitmap_cache: HashMap<GroupNumber, Bitmap>,
    /// Modified bitmaps that need to be written back
    dirty_bitmaps: HashMap<GroupNumber, bool>,
    /// Superblock reference for metadata
    superblock: Ext4Superblock,
    /// Group descriptors
    group_descriptors: Vec<Ext4GroupDesc>,
    /// Inodes per group
    inodes_per_group: u32,
    /// Total number of groups
    num_groups: u32,
    /// Last allocated group (for spreading)
    last_alloc_group: GroupNumber,
    /// Inode size
    inode_size: u16,
}

impl InodeAllocator {
    /// Create a new inode allocator
    pub fn new(
        superblock: Ext4Superblock,
        group_descriptors: Vec<Ext4GroupDesc>,
    ) -> Self {
        let inodes_per_group = superblock.s_inodes_per_group;
        let num_groups = group_descriptors.len() as u32;
        let inode_size = superblock.s_inode_size;
        
        Self {
            bitmap_cache: HashMap::new(),
            dirty_bitmaps: HashMap::new(),
            superblock,
            group_descriptors,
            inodes_per_group,
            num_groups,
            last_alloc_group: 0,
            inode_size,
        }
    }
    
    /// Allocate a new inode
    pub fn allocate_inode(
        &mut self,
        is_directory: bool,
        parent_inode: Option<InodeNumber>,
    ) -> Ext4Result<InodeNumber> {
        // Determine allocation strategy
        let strategy = if is_directory {
            AllocationStrategy::DirectorySpread
        } else {
            AllocationStrategy::LocalityGroup
        };
        
        // Find the best group for allocation
        let group = self.find_best_group(strategy, parent_inode)?;
        
        // Allocate inode in the selected group
        let inode_num = self.allocate_in_group(group)?;
        
        // Update directory count if allocating a directory
        if is_directory {
            let gd = &mut self.group_descriptors[group as usize];
            gd.bg_used_dirs_count_lo += 1;
            // Note: bg_used_dirs_count_hi exists in 64-bit descriptors
        }
        
        Ok(inode_num)
    }
    
    /// Free an inode
    pub fn free_inode(&mut self, inode_num: InodeNumber) -> Ext4Result<()> {
        if inode_num == 0 || inode_num > self.superblock.s_inodes_count {
            return Err(Ext4Error::Io(format!("Invalid inode number: {}", inode_num)));
        }
        
        // Special inodes (1-10) should not be freed
        if inode_num < EXT4_FIRST_INO {
            return Err(Ext4Error::Io(format!(
                "Cannot free reserved inode: {}",
                inode_num
            )));
        }
        
        // Calculate group and index
        let group = (inode_num - 1) / self.inodes_per_group;
        let index = (inode_num - 1) % self.inodes_per_group;
        
        // Get bitmap and clear the bit
        let bitmap = self.get_or_load_bitmap(group)?;
        if !bitmap.is_set(index) {
            return Err(Ext4Error::Io(format!(
                "Attempting to free already free inode: {}",
                inode_num
            )));
        }
        
        bitmap.clear(index);
        self.dirty_bitmaps.insert(group, true);
        
        // Update group descriptor
        let gd = &mut self.group_descriptors[group as usize];
        let current_free = gd.bg_free_inodes_count_lo as u32 
            | ((gd.bg_free_inodes_count_hi as u32) << 16);
        let new_free = current_free + 1;
        gd.bg_free_inodes_count_lo = (new_free & 0xFFFF) as u16;
        gd.bg_free_inodes_count_hi = ((new_free >> 16) & 0xFFFF) as u16;
        
        // Update superblock
        self.superblock.s_free_inodes_count += 1;
        
        Ok(())
    }
    
    /// Initialize a newly allocated inode with default values
    pub fn initialize_inode(
        &self,
        inode: &mut Ext4Inode,
        mode: u16,
        uid: u32,
        gid: u32,
    ) -> Ext4Result<()> {
        // Get current time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs() as u32;
        
        // Clear the inode first
        *inode = Ext4Inode::new();
        
        // Set basic fields
        inode.i_mode = mode;
        inode.i_uid = uid as u16;
        inode.i_gid = gid as u16;
        inode.i_links_count = 0;
        inode.i_size_lo = 0;
        inode.i_size_high = 0;
        inode.i_blocks_lo = 0;
        
        // Set timestamps
        inode.i_atime = now;
        inode.i_ctime = now;
        inode.i_mtime = now;
        inode.i_crtime = now;
        
        // Set generation (should be random in production)
        inode.i_generation = now; // Using timestamp as simple generation
        
        // Set flags based on filesystem features
        if self.superblock.s_feature_incompat & EXT4_FEATURE_INCOMPAT_EXTENTS != 0 {
            // Use extents for new files
            inode.i_flags |= EXT4_EXTENTS_FL;
        }
        
        // Initialize extent header if using extents
        if inode.i_flags & EXT4_EXTENTS_FL != 0 {
            self.init_extent_header(inode)?;
        }
        
        Ok(())
    }
    
    /// Check if an inode is allocated
    pub fn is_inode_allocated(&mut self, inode_num: InodeNumber) -> Ext4Result<bool> {
        if inode_num == 0 || inode_num > self.superblock.s_inodes_count {
            return Ok(false);
        }
        
        let group = (inode_num - 1) / self.inodes_per_group;
        let index = (inode_num - 1) % self.inodes_per_group;
        
        let bitmap = self.get_or_load_bitmap(group)?;
        Ok(bitmap.is_set(index))
    }
    
    /// Get the total number of free inodes
    pub fn get_free_inodes_count(&self) -> u32 {
        self.superblock.s_free_inodes_count
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
    
    /// Find the best group for inode allocation
    fn find_best_group(
        &mut self,
        strategy: AllocationStrategy,
        parent_inode: Option<InodeNumber>,
    ) -> Ext4Result<GroupNumber> {
        match strategy {
            AllocationStrategy::DirectorySpread => {
                // Orlov allocator for directories - spread them across groups
                self.find_group_orlov()
            }
            AllocationStrategy::LocalityGroup => {
                // Keep files close to parent directory
                if let Some(parent) = parent_inode {
                    let parent_group = (parent - 1) / self.inodes_per_group;
                    if self.group_has_free_inodes(parent_group) {
                        return Ok(parent_group);
                    }
                }
                self.find_group_other()
            }
            AllocationStrategy::FirstAvailable => {
                // Simple first-fit allocation
                self.find_group_other()
            }
        }
    }
    
    /// Orlov allocator for directory spreading
    fn find_group_orlov(&mut self) -> Ext4Result<GroupNumber> {
        let mut best_group = 0;
        let mut best_score = 0i32;
        
        for group in 0..self.num_groups {
            let gd = &self.group_descriptors[group as usize];
            
            // Calculate score based on free inodes and blocks
            let free_inodes = gd.bg_free_inodes_count_lo as u32 
                | ((gd.bg_free_inodes_count_hi as u32) << 16);
            let free_blocks = gd.bg_free_blocks_count_lo as u32 
                | ((gd.bg_free_blocks_count_hi as u32) << 16);
            let used_dirs = gd.bg_used_dirs_count_lo as u32;
            
            // Prefer groups with fewer directories and more free space
            let score = (free_inodes as i32 / 4) + (free_blocks as i32 / 16) - (used_dirs as i32 * 2);
            
            if score > best_score {
                best_score = score;
                best_group = group;
            }
        }
        
        self.last_alloc_group = best_group;
        Ok(best_group)
    }
    
    /// Find a group for regular file allocation
    fn find_group_other(&self) -> Ext4Result<GroupNumber> {
        // Start from last allocated group and search linearly
        for offset in 0..self.num_groups {
            let group = (self.last_alloc_group + offset) % self.num_groups;
            if self.group_has_free_inodes(group) {
                return Ok(group);
            }
        }
        
        Err(Ext4Error::Io("No free inodes available".to_string()))
    }
    
    /// Check if a group has free inodes
    fn group_has_free_inodes(&self, group: GroupNumber) -> bool {
        let gd = &self.group_descriptors[group as usize];
        let free_inodes = gd.bg_free_inodes_count_lo as u32 
            | ((gd.bg_free_inodes_count_hi as u32) << 16);
        free_inodes > 0
    }
    
    /// Allocate an inode within a specific group
    fn allocate_in_group(&mut self, group: GroupNumber) -> Ext4Result<InodeNumber> {
        let inodes_per_group = self.inodes_per_group;
        let bitmap = self.get_or_load_bitmap(group)?;
        
        // Find first free inode in bitmap
        for i in 0..inodes_per_group {
            if !bitmap.is_set(i) {
                // Found a free inode
                bitmap.set(i);
                self.dirty_bitmaps.insert(group, true);
                
                // Calculate actual inode number
                let inode_num = group * inodes_per_group + i + 1;
                
                // Update group descriptor
                let gd = &mut self.group_descriptors[group as usize];
                let current_free = gd.bg_free_inodes_count_lo as u32 
                    | ((gd.bg_free_inodes_count_hi as u32) << 16);
                let new_free = current_free - 1;
                gd.bg_free_inodes_count_lo = (new_free & 0xFFFF) as u16;
                gd.bg_free_inodes_count_hi = ((new_free >> 16) & 0xFFFF) as u16;
                
                // Update superblock
                self.superblock.s_free_inodes_count -= 1;
                
                return Ok(inode_num);
            }
        }
        
        Err(Ext4Error::Io("No free inodes in group".to_string()))
    }
    
    /// Initialize extent header for a new inode
    fn init_extent_header(&self, inode: &mut Ext4Inode) -> Ext4Result<()> {
        use super::structures::Ext4ExtentHeader;
        
        // Clear the i_block field
        inode.i_block = [0u32; 15];
        
        // Create extent header at the beginning of i_block
        let header = unsafe {
            &mut *(inode.i_block.as_mut_ptr() as *mut Ext4ExtentHeader)
        };
        
        header.eh_magic = EXT4_EXTENT_MAGIC;
        header.eh_entries = 0;
        header.eh_max = 4; // Can hold 4 extents in inode
        header.eh_depth = 0; // Leaf node
        header.eh_generation = 0;
        
        Ok(())
    }
    
    /// Get or load an inode bitmap for a group
    fn get_or_load_bitmap(&mut self, group: GroupNumber) -> Ext4Result<&mut Bitmap> {
        // This is a placeholder - in real implementation, would load from disk
        // For now, create an empty bitmap with reserved inodes marked
        if !self.bitmap_cache.contains_key(&group) {
            let mut bitmap = Bitmap::for_inode_group(self.inodes_per_group);
            
            // Mark reserved inodes as used in group 0
            if group == 0 {
                for i in 0..EXT4_FIRST_INO {
                    bitmap.set(i - 1); // Inodes are 1-indexed
                }
            }
            
            self.bitmap_cache.insert(group, bitmap);
        }
        
        Ok(self.bitmap_cache.get_mut(&group).unwrap())
    }
}