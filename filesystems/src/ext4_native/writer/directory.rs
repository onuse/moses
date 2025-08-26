// EXT4 Directory Entry Management
// Handles creating, reading, and modifying directory entries

use crate::ext4_native::core::{
    structures::*,
    types::*,
    constants::*,
    transaction::TransactionHandle,
};
use moses_core::MosesError;
/// Directory entry with metadata
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub inode: u32,
    pub name: String,
    pub file_type: u8,
}

/// Directory block header for HTree indexed directories
#[repr(C, packed)]
pub struct DxRoot {
    pub dot: Ext4DirEntry2,
    pub dotdot: Ext4DirEntry2,
    pub dx_root_info: DxRootInfo,
    pub dx_entries: [DxEntry; 0], // Variable length
}

/// HTree root information
#[repr(C, packed)]
pub struct DxRootInfo {
    pub reserved_zero: u32,
    pub hash_version: u8,
    pub info_length: u8,
    pub indirect_levels: u8,
    pub unused_flags: u8,
}

/// HTree directory index entry
#[repr(C, packed)]
pub struct DxEntry {
    pub hash: u32,
    pub block: u32,
}

/// Directory operations for EXT4 Writer
impl super::Ext4Writer {
    /// Lookup an entry in a directory
    pub(super) fn lookup_directory_entry(
        &mut self,
        dir_inode_num: u32,
        name: &str,
    ) -> Result<Option<DirectoryEntry>, MosesError> {
        let dir_inode = self.read_inode(dir_inode_num)?;
        
        // Check if directory uses HTree indexing
        if dir_inode.i_flags & EXT4_INDEX_FL != 0 {
            return self.lookup_htree_entry(dir_inode_num, &dir_inode, name);
        }
        
        // Linear search for non-indexed directories
        self.lookup_linear_entry(dir_inode_num, &dir_inode, name)
    }
    
    /// Linear search through directory blocks
    fn lookup_linear_entry(
        &mut self,
        _dir_inode_num: u32,
        dir_inode: &Ext4Inode,
        name: &str,
    ) -> Result<Option<DirectoryEntry>, MosesError> {
        let blocks = self.get_extent_blocks(dir_inode)?;
        
        for block_num in blocks {
            // Read block data
            let block_data = self.read_block(block_num)?;
            
            // Parse directory entries in block
            let mut offset = 0;
            while offset < self.block_size as usize {
                if offset + std::mem::size_of::<Ext4DirEntry2>() > self.block_size as usize {
                    break;
                }
                
                let entry = unsafe {
                    &*(block_data.as_ptr().add(offset) as *const Ext4DirEntry2)
                };
                
                // Check if entry is valid
                if entry.inode == 0 {
                    offset += entry.rec_len as usize;
                    continue;
                }
                
                // Extract name
                let name_len = entry.name_len as usize;
                if name_len > 0 && name_len <= 255 {
                    let entry_name = unsafe {
                        let name_ptr = block_data.as_ptr().add(offset + 8);
                        std::str::from_utf8_unchecked(
                            std::slice::from_raw_parts(name_ptr, name_len)
                        )
                    };
                    
                    if entry_name == name {
                        return Ok(Some(DirectoryEntry {
                            inode: entry.inode,
                            name: entry_name.to_string(),
                            file_type: entry.file_type,
                        }));
                    }
                }
                
                offset += entry.rec_len as usize;
            }
        }
        
        Ok(None)
    }
    
    
    /// Add a new directory entry
    pub(super) fn add_directory_entry_impl(
        &mut self,
        dir_inode_num: u32,
        name: &str,
        target_inode: u32,
        file_type: u8,
        _transaction: &TransactionHandle,
    ) -> Result<(), MosesError> {
        let mut dir_inode = self.read_inode(dir_inode_num)?;
        
        // Check if entry already exists
        if let Some(_) = self.lookup_directory_entry(dir_inode_num, name)? {
            return Err(MosesError::Other(format!("Entry '{}' already exists", name)));
        }
        
        // Calculate entry size
        let name_len = name.len();
        let entry_size = 8 + name_len; // inode(4) + rec_len(2) + name_len(1) + file_type(1) + name
        let _aligned_size = (entry_size + 3) & !3; // Align to 4 bytes
        
        // Try to add to existing blocks
        let blocks = self.get_extent_blocks(&dir_inode)?;
        for block_num in &blocks {
            if self.try_add_entry_to_block(*block_num, name, target_inode, file_type, _transaction)? {
                self.update_directory_mtime(dir_inode_num, _transaction)?;
                return Ok(());
            }
        }
        
        // Need to allocate a new block
        let new_block = self.allocate_directory_block(&mut dir_inode, _transaction)?;
        self.init_directory_block(new_block, _transaction)?;
        
        // Add entry to new block
        if !self.try_add_entry_to_block(new_block, name, target_inode, file_type, _transaction)? {
            return Err(MosesError::Other("Failed to add entry to new block".to_string()));
        }
        
        // Update directory inode
        dir_inode.i_size_lo += self.block_size as u32;
        self.write_inode(dir_inode_num, &dir_inode, _transaction)?;
        
        Ok(())
    }
    
    /// Try to add an entry to a specific block
    fn try_add_entry_to_block(
        &mut self,
        block_num: BlockNumber,
        name: &str,
        inode: u32,
        file_type: u8,
        __transaction: &TransactionHandle,
    ) -> Result<bool, MosesError> {
        let mut block_data = self.read_block(block_num)?;
        
        let name_len = name.len();
        let required_size = 8 + name_len;
        let aligned_required = (required_size + 3) & !3;
        
        let mut offset = 0;
        while offset < self.block_size as usize {
            if offset + std::mem::size_of::<Ext4DirEntry2>() > self.block_size as usize {
                break;
            }
            
            let entry = unsafe {
                &mut *(block_data.as_mut_ptr().add(offset) as *mut Ext4DirEntry2)
            };
            
            let rec_len = entry.rec_len as usize;
            
            // Calculate actual entry size
            let actual_size = if entry.inode != 0 {
                8 + entry.name_len as usize
            } else {
                0
            };
            let aligned_actual = (actual_size + 3) & !3;
            
            // Check if there's enough space
            if rec_len >= aligned_actual + aligned_required {
                // Split the entry
                if entry.inode != 0 {
                    // Adjust current entry's rec_len
                    entry.rec_len = aligned_actual as u16;
                    
                    // Create new entry in the free space
                    let new_offset = offset + aligned_actual;
                    let new_entry = unsafe {
                        &mut *(block_data.as_mut_ptr().add(new_offset) as *mut Ext4DirEntry2)
                    };
                    
                    new_entry.inode = inode;
                    new_entry.rec_len = (rec_len - aligned_actual) as u16;
                    new_entry.name_len = name_len as u8;
                    new_entry.file_type = file_type;
                    
                    // Copy name
                    unsafe {
                        let name_ptr = block_data.as_mut_ptr().add(new_offset + 8);
                        name_ptr.copy_from_nonoverlapping(name.as_ptr(), name_len);
                    }
                } else {
                    // Use empty entry
                    entry.inode = inode;
                    entry.name_len = name_len as u8;
                    entry.file_type = file_type;
                    
                    // Copy name
                    unsafe {
                        let name_ptr = block_data.as_mut_ptr().add(offset + 8);
                        name_ptr.copy_from_nonoverlapping(name.as_ptr(), name_len);
                    }
                }
                
                // Write block back
                self.write_block(block_num, &block_data)?;
                return Ok(true);
            }
            
            offset += rec_len;
        }
        
        Ok(false)
    }
    
    /// Remove a directory entry
    pub(super) fn remove_directory_entry_impl(
        &mut self,
        dir_inode_num: u32,
        name: &str,
        _transaction: &TransactionHandle,
    ) -> Result<u32, MosesError> {
        let dir_inode = self.read_inode(dir_inode_num)?;
        let blocks = self.get_extent_blocks(&dir_inode)?;
        
        for block_num in blocks {
            let mut block_data = self.read_block(block_num)?;
            
            let mut offset = 0;
            let mut prev_offset = None;
            
            while offset < self.block_size as usize {
                let entry = unsafe {
                    &*(block_data.as_ptr().add(offset) as *const Ext4DirEntry2)
                };
                
                if entry.inode != 0 && entry.name_len > 0 {
                    let entry_name = unsafe {
                        let name_ptr = block_data.as_ptr().add(offset + 8);
                        std::str::from_utf8_unchecked(
                            std::slice::from_raw_parts(name_ptr, entry.name_len as usize)
                        )
                    };
                    
                    if entry_name == name {
                        let removed_inode = entry.inode;
                        
                        // Mark entry as deleted
                        let entry_mut = unsafe {
                            &mut *(block_data.as_mut_ptr().add(offset) as *mut Ext4DirEntry2)
                        };
                        
                        if let Some(prev) = prev_offset {
                            // Merge with previous entry
                            let prev_entry = unsafe {
                                &mut *(block_data.as_mut_ptr().add(prev) as *mut Ext4DirEntry2)
                            };
                            prev_entry.rec_len += entry_mut.rec_len;
                        } else {
                            // Just mark as deleted
                            entry_mut.inode = 0;
                        }
                        
                        self.write_block(block_num, &block_data)?;
                        self.update_directory_mtime(dir_inode_num, _transaction)?;
                        return Ok(removed_inode);
                    }
                }
                
                prev_offset = Some(offset);
                offset += entry.rec_len as usize;
            }
        }
        
        Err(MosesError::Other(format!("Entry '{}' not found", name)))
    }
    
    /// Check if a directory is empty
    pub(super) fn is_directory_empty_impl(&mut self, dir_inode_num: u32) -> Result<bool, MosesError> {
        let dir_inode = self.read_inode(dir_inode_num)?;
        let blocks = self.get_extent_blocks(&dir_inode)?;
        
        let mut entry_count = 0;
        
        for block_num in blocks {
            let block_data = self.read_block(block_num)?;
            
            let mut offset = 0;
            while offset < self.block_size as usize {
                if offset + std::mem::size_of::<Ext4DirEntry2>() > self.block_size as usize {
                    break;
                }
                
                let entry = unsafe {
                    &*(block_data.as_ptr().add(offset) as *const Ext4DirEntry2)
                };
                
                if entry.inode != 0 {
                    let name_len = entry.name_len as usize;
                    if name_len > 0 && name_len <= 255 {
                        let entry_name = unsafe {
                            let name_ptr = block_data.as_ptr().add(offset + 8);
                            std::str::from_utf8_unchecked(
                                std::slice::from_raw_parts(name_ptr, name_len)
                            )
                        };
                        
                        // Skip . and .. entries
                        if entry_name != "." && entry_name != ".." {
                            entry_count += 1;
                        }
                    }
                }
                
                offset += entry.rec_len as usize;
            }
        }
        
        Ok(entry_count == 0)
    }
    
    /// Create . and .. entries in a new directory block
    pub(super) fn create_dot_entries_impl(
        &mut self,
        dir_block: BlockNumber,
        self_inode: u32,
        parent_inode: u32,
        __transaction: &TransactionHandle,
    ) -> Result<(), MosesError> {
        let mut block_data = vec![0u8; self.block_size as usize];
        
        // Create "." entry
        let dot_entry = unsafe {
            &mut *(block_data.as_mut_ptr() as *mut Ext4DirEntry2)
        };
        dot_entry.inode = self_inode;
        dot_entry.rec_len = 12; // 4 + 2 + 1 + 1 + 1 (padded to 12)
        dot_entry.name_len = 1;
        dot_entry.file_type = EXT4_FT_DIR;
        block_data[8] = b'.';
        
        // Create ".." entry
        let dotdot_entry = unsafe {
            &mut *(block_data.as_mut_ptr().add(12) as *mut Ext4DirEntry2)
        };
        dotdot_entry.inode = parent_inode;
        dotdot_entry.rec_len = (self.block_size - 12) as u16;
        dotdot_entry.name_len = 2;
        dotdot_entry.file_type = EXT4_FT_DIR;
        block_data[20] = b'.';
        block_data[21] = b'.';
        
        self.write_block(dir_block, &block_data)?;
        Ok(())
    }
    
    /// Initialize a new directory block
    fn init_directory_block(
        &mut self,
        block_num: BlockNumber,
        __transaction: &TransactionHandle,
    ) -> Result<(), MosesError> {
        let mut block_data = vec![0u8; self.block_size as usize];
        
        // Create a single empty entry spanning the whole block
        let entry = unsafe {
            &mut *(block_data.as_mut_ptr() as *mut Ext4DirEntry2)
        };
        entry.inode = 0;
        entry.rec_len = self.block_size as u16;
        entry.name_len = 0;
        entry.file_type = 0;
        
        self.write_block(block_num, &block_data)?;
        Ok(())
    }
    
    /// Allocate a new block for directory
    fn allocate_directory_block(
        &mut self,
        dir_inode: &mut Ext4Inode,
        _transaction: &TransactionHandle,
    ) -> Result<BlockNumber, MosesError> {
        // Allocate block
        let block = self.block_allocator.allocate_block(None)
            .map_err(|e| MosesError::Other(format!("Block allocation failed: {:?}", e)))?;
        
        // Add to inode's extent tree
        let logical_block = (dir_inode.i_size_lo / self.block_size as u32) as u32;
        self.add_extents(dir_inode, logical_block, &[block])?;
        
        Ok(block)
    }
    
    /// Update directory modification time
    fn update_directory_mtime(
        &mut self,
        dir_inode_num: u32,
        _transaction: &TransactionHandle,
    ) -> Result<(), MosesError> {
        let mut dir_inode = self.read_inode(dir_inode_num)?;
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs() as u32;
        
        dir_inode.i_mtime = now;
        dir_inode.i_ctime = now;
        
        self.write_inode(dir_inode_num, &dir_inode, _transaction)?;
        Ok(())
    }
    
    /// Create an HTree indexed directory root block
    pub fn create_htree_root(&mut self, parent_inode: u32) -> Result<Vec<u8>, MosesError> {
        let mut block = vec![0u8; self.block_size as usize];
        
        // Create dot entry
        let dot = Ext4DirEntry2 {
            inode: parent_inode,
            rec_len: 12,
            name_len: 1,
            file_type: EXT4_FT_DIR,
        };
        
        // Create dotdot entry
        let dotdot = Ext4DirEntry2 {
            inode: parent_inode, // Will be updated to actual parent
            rec_len: self.block_size as u16 - 12,
            name_len: 2,
            file_type: EXT4_FT_DIR,
        };
        
        // Create DxRootInfo
        let dx_info = DxRootInfo {
            reserved_zero: 0,
            hash_version: 1, // DX_HASH_HALF_MD4
            info_length: 8,
            indirect_levels: 0,
            unused_flags: 0,
        };
        
        // Write dot entry
        unsafe {
            let dot_bytes = std::slice::from_raw_parts(
                &dot as *const _ as *const u8,
                std::mem::size_of::<Ext4DirEntry2>()
            );
            block[0..dot_bytes.len()].copy_from_slice(dot_bytes);
        }
        
        // Write name for dot
        block[8] = b'.';
        
        // Write dotdot entry at offset 12
        unsafe {
            let dotdot_bytes = std::slice::from_raw_parts(
                &dotdot as *const _ as *const u8,
                std::mem::size_of::<Ext4DirEntry2>()
            );
            block[12..12 + dotdot_bytes.len()].copy_from_slice(dotdot_bytes);
        }
        
        // Write name for dotdot
        block[20] = b'.';
        block[21] = b'.';
        
        // Write DxRootInfo at appropriate offset within dotdot's space
        let dx_info_offset = 24;
        unsafe {
            let info_bytes = std::slice::from_raw_parts(
                &dx_info as *const _ as *const u8,
                std::mem::size_of::<DxRootInfo>()
            );
            block[dx_info_offset..dx_info_offset + info_bytes.len()].copy_from_slice(info_bytes);
        }
        
        Ok(block)
    }
    
    /// Read a block from disk
    fn read_block(&mut self, block_num: BlockNumber) -> Result<Vec<u8>, MosesError> {
        self.read_block_from_disk(block_num)
    }
    
    /// Write a block to disk
    fn write_block(&mut self, block_num: BlockNumber, data: &[u8]) -> Result<(), MosesError> {
        self.write_block_to_disk(block_num, data)
    }
}