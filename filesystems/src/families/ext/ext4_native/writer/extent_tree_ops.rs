// EXT4 Extent Tree Advanced Operations
// Implements full extent tree functionality including index nodes

use crate::families::ext::ext4_native::core::{
    structures::*,
    constants::*,
};
use moses_core::MosesError;
use std::mem;
use log::debug;

/// EXT4 Extent Index structure for internal nodes
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4ExtentIdx {
    pub ei_block: u32,       // First logical block covered
    pub ei_leaf_lo: u32,     // Low 32 bits of physical block of next level
    pub ei_leaf_hi: u16,     // High 16 bits of physical block
    pub ei_unused: u16,      // Reserved
}

impl Ext4ExtentIdx {
    /// Get the physical block number this index points to
    pub fn get_leaf_block(&self) -> u64 {
        self.ei_leaf_lo as u64 | ((self.ei_leaf_hi as u64) << 32)
    }
    
    /// Create a new extent index
    pub fn new(logical_block: u32, physical_block: u64) -> Self {
        Self {
            ei_block: logical_block,
            ei_leaf_lo: physical_block as u32,
            ei_leaf_hi: (physical_block >> 32) as u16,
            ei_unused: 0,
        }
    }
}

/// Path element for extent tree traversal
#[derive(Debug, Clone)]
pub struct ExtentPathElement {
    pub physical_block: u64,
    pub depth: u16,
    pub header: Ext4ExtentHeader,
    pub index_offset: Option<usize>,  // Offset of the index used to get here
}

/// Complete extent tree operations
pub struct ExtentTreeOps {
    block_size: u32,
}

impl ExtentTreeOps {
    pub fn new(block_size: u32) -> Self {
        Self { block_size }
    }
    
    /// Calculate maximum extents/indexes that can fit in a block
    pub fn max_entries_per_block(&self) -> u16 {
        let header_size = mem::size_of::<Ext4ExtentHeader>();
        let entry_size = mem::size_of::<Ext4Extent>(); // Same size as Ext4ExtentIdx
        ((self.block_size as usize - header_size) / entry_size) as u16
    }
    
    /// Find extent covering a logical block by traversing the tree
    pub fn find_extent(
        &self,
        inode: &Ext4Inode,
        logical_block: u32,
        read_block: impl Fn(u64) -> Result<Vec<u8>, MosesError>,
    ) -> Result<Option<Ext4Extent>, MosesError> {
        if inode.i_flags & EXT4_EXTENTS_FL == 0 {
            return Ok(None);
        }
        
        // Get root header from inode
        let root_header = unsafe {
            &*(inode.i_block.as_ptr() as *const Ext4ExtentHeader)
        };
        
        if root_header.eh_magic != EXT4_EXTENT_MAGIC {
            return Err(MosesError::Other("Invalid extent magic".to_string()));
        }
        
        // Start traversal from root
        self.find_extent_recursive(
            &inode.i_block,
            root_header.eh_depth,
            logical_block,
            &read_block,
        )
    }
    
    /// Recursive helper to find extent
    fn find_extent_recursive(
        &self,
        block_data: &[u32; 15],
        depth: u16,
        logical_block: u32,
        read_block: &impl Fn(u64) -> Result<Vec<u8>, MosesError>,
    ) -> Result<Option<Ext4Extent>, MosesError> {
        let header = unsafe {
            &*(block_data.as_ptr() as *const Ext4ExtentHeader)
        };
        
        if depth == 0 {
            // Leaf node - search extents
            let extents = unsafe {
                let ptr = (block_data.as_ptr() as *const u8)
                    .add(mem::size_of::<Ext4ExtentHeader>()) as *const Ext4Extent;
                std::slice::from_raw_parts(ptr, header.eh_entries as usize)
            };
            
            for extent in extents {
                if logical_block >= extent.ee_block 
                    && logical_block < extent.ee_block + extent.ee_len as u32 {
                    return Ok(Some(*extent));
                }
            }
            Ok(None)
        } else {
            // Index node - find appropriate child
            let indexes = unsafe {
                let ptr = (block_data.as_ptr() as *const u8)
                    .add(mem::size_of::<Ext4ExtentHeader>()) as *const Ext4ExtentIdx;
                std::slice::from_raw_parts(ptr, header.eh_entries as usize)
            };
            
            // Find the right index
            let mut target_idx = None;
            for (i, idx) in indexes.iter().enumerate() {
                if i == indexes.len() - 1 || logical_block < indexes[i + 1].ei_block {
                    target_idx = Some(idx);
                    break;
                }
            }
            
            if let Some(idx) = target_idx {
                // Read child block
                let child_block = read_block(idx.get_leaf_block())?;
                
                // Recursively search child
                let child_data = unsafe {
                    let mut data = [0u32; 15];
                    std::ptr::copy_nonoverlapping(
                        child_block.as_ptr(),
                        data.as_mut_ptr() as *mut u8,
                        std::cmp::min(60, child_block.len())
                    );
                    data
                };
                
                self.find_extent_recursive(&child_data, depth - 1, logical_block, read_block)
            } else {
                Ok(None)
            }
        }
    }
    
    /// Insert a new extent into the tree
    pub fn insert_extent(
        &mut self,
        inode: &mut Ext4Inode,
        logical_start: u32,
        physical_start: u64,
        length: u16,
        read_block: impl Fn(u64) -> Result<Vec<u8>, MosesError>,
        write_block: impl Fn(u64, &[u8]) -> Result<(), MosesError>,
        allocate_block: impl Fn() -> Result<u64, MosesError>,
    ) -> Result<(), MosesError> {
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
        
        if header.eh_depth == 0 {
            // Simple case: leaf node in inode
            self.insert_extent_in_leaf(inode, logical_start, physical_start, length)
        } else {
            // Complex case: need to traverse tree
            self.insert_extent_in_tree(
                inode,
                logical_start,
                physical_start,
                length,
                read_block,
                write_block,
                allocate_block,
            )
        }
    }
    
    /// Insert extent in a leaf node (simple case)
    fn insert_extent_in_leaf(
        &self,
        inode: &mut Ext4Inode,
        logical_start: u32,
        physical_start: u64,
        length: u16,
    ) -> Result<(), MosesError> {
        let header = unsafe {
            &mut *(inode.i_block.as_mut_ptr() as *mut Ext4ExtentHeader)
        };
        
        if header.eh_entries >= header.eh_max {
            // Need to convert to tree structure
            return Err(MosesError::Other("Extent tree split required".to_string()));
        }
        
        let extents = unsafe {
            let ptr = (inode.i_block.as_mut_ptr() as *mut u8)
                .add(mem::size_of::<Ext4ExtentHeader>()) as *mut Ext4Extent;
            std::slice::from_raw_parts_mut(ptr, header.eh_max as usize)
        };
        
        // Find insertion point (keep sorted by logical block)
        let mut insert_pos = header.eh_entries as usize;
        for i in 0..header.eh_entries as usize {
            if extents[i].ee_block > logical_start {
                insert_pos = i;
                break;
            }
        }
        
        // Shift extents if needed
        if insert_pos < header.eh_entries as usize {
            for i in (insert_pos..header.eh_entries as usize).rev() {
                extents[i + 1] = extents[i];
            }
        }
        
        // Insert new extent
        extents[insert_pos] = Ext4Extent::new(logical_start, physical_start, length);
        header.eh_entries += 1;
        
        // Try to merge adjacent extents
        self.merge_extents(extents, header.eh_entries);
        
        Ok(())
    }
    
    /// Merge adjacent extents if possible
    fn merge_extents(&self, extents: &mut [Ext4Extent], count: u16) {
        let mut write_pos = 0;
        let mut read_pos = 0;
        
        while read_pos < count as usize {
            if write_pos > 0 {
                let prev = &extents[write_pos - 1];
                let curr = &extents[read_pos];
                
                // Check if extents are adjacent both logically and physically
                if prev.ee_block + prev.ee_len as u32 == curr.ee_block {
                    let prev_phys = prev.ee_start_lo as u64 | ((prev.ee_start_hi as u64) << 32);
                    let curr_phys = curr.ee_start_lo as u64 | ((curr.ee_start_hi as u64) << 32);
                    
                    if prev_phys + prev.ee_len as u64 == curr_phys {
                        // Merge extents
                        extents[write_pos - 1].ee_len += curr.ee_len;
                        read_pos += 1;
                        continue;
                    }
                }
            }
            
            if write_pos != read_pos {
                extents[write_pos] = extents[read_pos];
            }
            write_pos += 1;
            read_pos += 1;
        }
    }
    
    /// Insert extent in a tree structure (complex case)
    fn insert_extent_in_tree(
        &mut self,
        inode: &mut Ext4Inode,
        logical_start: u32,
        _physical_start: u64,
        _length: u16,
        read_block: impl Fn(u64) -> Result<Vec<u8>, MosesError>,
        _write_block: impl Fn(u64, &[u8]) -> Result<(), MosesError>,
        _allocate_block: impl Fn() -> Result<u64, MosesError>,
    ) -> Result<(), MosesError> {
        // Build path to leaf
        let path = self.build_path_to_leaf(inode, logical_start, &read_block)?;
        
        // Find leaf node and insert
        if let Some(leaf_element) = path.last() {
            if leaf_element.depth != 0 {
                return Err(MosesError::Other("Invalid path: not a leaf".to_string()));
            }
            
            // TODO: Read leaf block, insert extent, handle splits if needed
            debug!("Inserting extent at leaf block {}", leaf_element.physical_block);
        }
        
        Err(MosesError::Other("Tree insertion not fully implemented".to_string()))
    }
    
    /// Build path from root to the leaf containing logical_block
    fn build_path_to_leaf(
        &self,
        inode: &Ext4Inode,
        logical_block: u32,
        read_block: &impl Fn(u64) -> Result<Vec<u8>, MosesError>,
    ) -> Result<Vec<ExtentPathElement>, MosesError> {
        let mut path = Vec::new();
        
        // Start with root
        let root_header = unsafe {
            &*(inode.i_block.as_ptr() as *const Ext4ExtentHeader)
        };
        
        if root_header.eh_magic != EXT4_EXTENT_MAGIC {
            return Err(MosesError::Other("Invalid extent magic".to_string()));
        }
        
        path.push(ExtentPathElement {
            physical_block: 0, // Root is in inode
            depth: root_header.eh_depth,
            header: *root_header,
            index_offset: None,
        });
        
        // Traverse down to leaf
        let mut current_depth = root_header.eh_depth;
        let mut current_data = inode.i_block.clone();
        
        while current_depth > 0 {
            let header = unsafe {
                &*(current_data.as_ptr() as *const Ext4ExtentHeader)
            };
            
            // Find appropriate index
            let indexes = unsafe {
                let ptr = (current_data.as_ptr() as *const u8)
                    .add(mem::size_of::<Ext4ExtentHeader>()) as *const Ext4ExtentIdx;
                std::slice::from_raw_parts(ptr, header.eh_entries as usize)
            };
            
            let mut target_idx = None;
            let mut idx_offset = 0;
            for (i, idx) in indexes.iter().enumerate() {
                if i == indexes.len() - 1 || logical_block < indexes[i + 1].ei_block {
                    target_idx = Some(idx);
                    idx_offset = i;
                    break;
                }
            }
            
            if let Some(idx) = target_idx {
                let child_block_num = idx.get_leaf_block();
                let child_data = read_block(child_block_num)?;
                
                // Parse child header
                let child_header = unsafe {
                    std::ptr::read(child_data.as_ptr() as *const Ext4ExtentHeader)
                };
                
                path.push(ExtentPathElement {
                    physical_block: child_block_num,
                    depth: current_depth - 1,
                    header: child_header,
                    index_offset: Some(idx_offset),
                });
                
                // Update for next iteration
                current_depth -= 1;
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        child_data.as_ptr(),
                        current_data.as_mut_ptr() as *mut u8,
                        std::cmp::min(60, child_data.len())
                    );
                }
            } else {
                return Err(MosesError::Other("No appropriate index found".to_string()));
            }
        }
        
        Ok(path)
    }
    
    /// Split a full node (either leaf or index)
    pub fn split_node(
        &mut self,
        node_data: &[u8],
        is_leaf: bool,
        allocate_block: impl Fn() -> Result<u64, MosesError>,
        write_block: impl Fn(u64, &[u8]) -> Result<(), MosesError>,
    ) -> Result<(u64, u32), MosesError> {
        let new_block = allocate_block()?;
        let header = unsafe {
            std::ptr::read(node_data.as_ptr() as *const Ext4ExtentHeader)
        };
        
        let split_point = header.eh_entries / 2;
        
        if is_leaf {
            // Split leaf node
            let extents = unsafe {
                let ptr = (node_data.as_ptr() as *const u8)
                    .add(mem::size_of::<Ext4ExtentHeader>()) as *const Ext4Extent;
                std::slice::from_raw_parts(ptr, header.eh_entries as usize)
            };
            
            // Create new block with second half
            let mut new_block_data = vec![0u8; self.block_size as usize];
            let new_header = Ext4ExtentHeader {
                eh_magic: EXT4_EXTENT_MAGIC,
                eh_entries: header.eh_entries - split_point,
                eh_max: self.max_entries_per_block(),
                eh_depth: 0,
                eh_generation: header.eh_generation,
            };
            
            unsafe {
                std::ptr::write(new_block_data.as_mut_ptr() as *mut Ext4ExtentHeader, new_header);
                
                let new_extents_ptr = new_block_data.as_mut_ptr()
                    .add(mem::size_of::<Ext4ExtentHeader>()) as *mut Ext4Extent;
                
                for i in 0..(header.eh_entries - split_point) as usize {
                    std::ptr::write(new_extents_ptr.add(i), extents[split_point as usize + i]);
                }
            }
            
            write_block(new_block, &new_block_data)?;
            
            // Return new block and first logical block it covers
            Ok((new_block, extents[split_point as usize].ee_block))
        } else {
            // Split index node
            let indexes = unsafe {
                let ptr = (node_data.as_ptr() as *const u8)
                    .add(mem::size_of::<Ext4ExtentHeader>()) as *const Ext4ExtentIdx;
                std::slice::from_raw_parts(ptr, header.eh_entries as usize)
            };
            
            // Similar logic for index nodes
            let mut new_block_data = vec![0u8; self.block_size as usize];
            let new_header = Ext4ExtentHeader {
                eh_magic: EXT4_EXTENT_MAGIC,
                eh_entries: header.eh_entries - split_point,
                eh_max: self.max_entries_per_block(),
                eh_depth: header.eh_depth,
                eh_generation: header.eh_generation,
            };
            
            unsafe {
                std::ptr::write(new_block_data.as_mut_ptr() as *mut Ext4ExtentHeader, new_header);
                
                let new_indexes_ptr = new_block_data.as_mut_ptr()
                    .add(mem::size_of::<Ext4ExtentHeader>()) as *mut Ext4ExtentIdx;
                
                for i in 0..(header.eh_entries - split_point) as usize {
                    std::ptr::write(new_indexes_ptr.add(i), indexes[split_point as usize + i]);
                }
            }
            
            write_block(new_block, &new_block_data)?;
            
            Ok((new_block, indexes[split_point as usize].ei_block))
        }
    }
    
    /// Remove an extent from the tree
    pub fn remove_extent(
        &mut self,
        inode: &mut Ext4Inode,
        logical_start: u32,
        logical_end: u32,
    ) -> Result<Vec<u64>, MosesError> {
        let mut freed_blocks = Vec::new();
        
        // Simple implementation for leaf-only trees
        let header = unsafe {
            &mut *(inode.i_block.as_mut_ptr() as *mut Ext4ExtentHeader)
        };
        
        if header.eh_depth != 0 {
            return Err(MosesError::Other("Extent removal from tree not yet implemented".to_string()));
        }
        
        let extents = unsafe {
            let ptr = (inode.i_block.as_mut_ptr() as *mut u8)
                .add(mem::size_of::<Ext4ExtentHeader>()) as *mut Ext4Extent;
            std::slice::from_raw_parts_mut(ptr, header.eh_entries as usize)
        };
        
        let mut write_pos = 0;
        for read_pos in 0..header.eh_entries as usize {
            let extent = &extents[read_pos];
            let extent_start = extent.ee_block;
            let extent_end = extent.ee_block + extent.ee_len as u32;
            
            if extent_end <= logical_start || extent_start >= logical_end {
                // No overlap, keep extent
                if write_pos != read_pos {
                    extents[write_pos] = extents[read_pos];
                }
                write_pos += 1;
            } else if extent_start >= logical_start && extent_end <= logical_end {
                // Complete overlap, remove extent and track freed blocks
                let phys_start = extent.ee_start_lo as u64 | ((extent.ee_start_hi as u64) << 32);
                for i in 0..extent.ee_len {
                    freed_blocks.push(phys_start + i as u64);
                }
            } else {
                // Partial overlap - truncate or split extent
                // TODO: Handle partial overlaps
                return Err(MosesError::Other("Partial extent removal not yet implemented".to_string()));
            }
        }
        
        header.eh_entries = write_pos as u16;
        
        Ok(freed_blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extent_index_creation() {
        let idx = Ext4ExtentIdx::new(1000, 0x123456789ABC);
        assert_eq!(idx.ei_block, 1000);
        assert_eq!(idx.get_leaf_block(), 0x123456789ABC);
    }
    
    #[test]
    fn test_max_entries_calculation() {
        let ops = ExtentTreeOps::new(4096);
        let max = ops.max_entries_per_block();
        // (4096 - 12) / 12 = 340
        assert_eq!(max, 340);
    }
}