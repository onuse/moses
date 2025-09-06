// Disk I/O Operations for EXT4 Writer
// Handles reading and writing data to/from the actual device

use super::*;
use moses_core::MosesError;
use crate::families::ext::ext4_native::core::{
    structures::*,
    types::*,
};

impl Ext4Writer {
    /// Read an inode from disk
    pub(super) fn read_inode_from_disk(&mut self, inode_num: u32) -> Result<Ext4Inode, MosesError> {
        // Validate inode number
        if inode_num == 0 || inode_num > self.superblock.s_inodes_count {
            return Err(MosesError::Other(format!("Invalid inode number: {}", inode_num)));
        }
        
        // Calculate which group the inode is in
        let group = (inode_num - 1) / self.superblock.s_inodes_per_group;
        let index = (inode_num - 1) % self.superblock.s_inodes_per_group;
        
        // Get the inode table location for this group
        let group_desc = &self.group_descriptors[group as usize];
        let inode_table_block = group_desc.bg_inode_table_lo as u64 
            | ((group_desc.bg_inode_table_hi as u64) << 32);
        
        // Calculate the block and offset for this inode
        let inode_size = self.superblock.s_inode_size as u64;
        let inodes_per_block = self.block_size as u64 / inode_size;
        let inode_block = inode_table_block + (index as u64 / inodes_per_block);
        let inode_offset = (index as u64 % inodes_per_block) * inode_size;
        
        // Read the block containing the inode
        let block_data = self.read_block_from_disk(inode_block)?;
        
        // Extract the inode from the block
        if inode_offset + inode_size > block_data.len() as u64 {
            return Err(MosesError::Other("Inode extends beyond block boundary".to_string()));
        }
        
        let inode_bytes = &block_data[inode_offset as usize..(inode_offset + inode_size) as usize];
        
        // Parse the inode structure
        let inode = unsafe {
            std::ptr::read_unaligned(inode_bytes.as_ptr() as *const Ext4Inode)
        };
        
        Ok(inode)
    }
    
    /// Write an inode to disk
    pub(super) fn write_inode_to_disk(
        &mut self,
        inode_num: u32,
        inode: &Ext4Inode,
    ) -> Result<(), MosesError> {
        // Validate inode number
        if inode_num == 0 || inode_num > self.superblock.s_inodes_count {
            return Err(MosesError::Other(format!("Invalid inode number: {}", inode_num)));
        }
        
        // Calculate which group the inode is in
        let group = (inode_num - 1) / self.superblock.s_inodes_per_group;
        let index = (inode_num - 1) % self.superblock.s_inodes_per_group;
        
        // Get the inode table location for this group
        let group_desc = &self.group_descriptors[group as usize];
        let inode_table_block = group_desc.bg_inode_table_lo as u64 
            | ((group_desc.bg_inode_table_hi as u64) << 32);
        
        // Calculate the block and offset for this inode
        let inode_size = self.superblock.s_inode_size as u64;
        let inodes_per_block = self.block_size as u64 / inode_size;
        let inode_block = inode_table_block + (index as u64 / inodes_per_block);
        let inode_offset = (index as u64 % inodes_per_block) * inode_size;
        
        // Read the current block
        let mut block_data = self.read_block_from_disk(inode_block)?;
        
        // Write the inode into the block
        let inode_bytes = unsafe {
            std::slice::from_raw_parts(
                inode as *const Ext4Inode as *const u8,
                std::mem::size_of::<Ext4Inode>()
            )
        };
        
        let end_offset = (inode_offset as usize) + inode_bytes.len();
        if end_offset > block_data.len() {
            return Err(MosesError::Other("Inode extends beyond block boundary".to_string()));
        }
        
        block_data[inode_offset as usize..end_offset].copy_from_slice(inode_bytes);
        
        // Write the block back
        self.write_block_to_disk(inode_block, &block_data)?;
        
        Ok(())
    }
    
    /// Read a block from disk
    pub(super) fn read_block_from_disk(&mut self, block_num: BlockNumber) -> Result<Vec<u8>, MosesError> {
        let mut buffer = vec![0u8; self.block_size as usize];
        let offset = block_num * self.block_size as u64;
        
        // Platform-specific device I/O
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use std::fs::OpenOptions;
            use std::io::{Read, Seek, SeekFrom};
            use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE};
            
            let mut file = OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .open(&self.device.mount_points[0])
                .map_err(|e| MosesError::Other(e.to_string()))?;
            
            file.seek(SeekFrom::Start(offset)).map_err(|e| MosesError::Other(e.to_string()))?;
            file.read_exact(&mut buffer).map_err(|e| MosesError::Other(e.to_string()))?;
        }
        
        #[cfg(target_os = "linux")]
        {
            use std::fs::OpenOptions;
            use std::io::{Read, Seek, SeekFrom};
            
            let mut file = OpenOptions::new()
                .read(true)
                .open(&self.device.mount_points[0])
                .map_err(|e| MosesError::Other(e.to_string()))?;
            
            file.seek(SeekFrom::Start(offset)).map_err(|e| MosesError::Other(e.to_string()))?;
            file.read_exact(&mut buffer).map_err(|e| MosesError::Other(e.to_string()))?;
        }
        
        #[cfg(target_os = "macos")]
        {
            use std::fs::OpenOptions;
            use std::io::{Read, Seek, SeekFrom};
            
            let mut file = OpenOptions::new()
                .read(true)
                .open(&self.device.mount_points[0])
                .map_err(|e| MosesError::Other(e.to_string()))?;
            
            file.seek(SeekFrom::Start(offset)).map_err(|e| MosesError::Other(e.to_string()))?;
            file.read_exact(&mut buffer).map_err(|e| MosesError::Other(e.to_string()))?;
        }
        
        Ok(buffer)
    }
    
    /// Write a block to disk
    pub(super) fn write_block_to_disk(&mut self, block_num: BlockNumber, data: &[u8]) -> Result<(), MosesError> {
        if data.len() != self.block_size as usize {
            return Err(MosesError::Other(format!(
                "Block data size {} doesn't match block size {}",
                data.len(),
                self.block_size
            )));
        }
        
        let offset = block_num * self.block_size as u64;
        
        // Platform-specific device I/O
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use std::fs::OpenOptions;
            use std::io::{Write, Seek, SeekFrom};
            use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE};
            
            let mut file = OpenOptions::new()
                .write(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .open(&self.device.mount_points[0])
                .map_err(|e| MosesError::Other(e.to_string()))?;
            
            file.seek(SeekFrom::Start(offset)).map_err(|e| MosesError::Other(e.to_string()))?;
            file.write_all(data).map_err(|e| MosesError::Other(e.to_string()))?;
        }
        
        #[cfg(target_os = "linux")]
        {
            use std::fs::OpenOptions;
            use std::io::{Write, Seek, SeekFrom};
            
            let mut file = OpenOptions::new()
                .write(true)
                .open(&self.device.mount_points[0])
                .map_err(|e| MosesError::Other(e.to_string()))?;
            
            file.seek(SeekFrom::Start(offset)).map_err(|e| MosesError::Other(e.to_string()))?;
            file.write_all(data).map_err(|e| MosesError::Other(e.to_string()))?;
        }
        
        #[cfg(target_os = "macos")]
        {
            use std::fs::OpenOptions;
            use std::io::{Write, Seek, SeekFrom};
            
            let mut file = OpenOptions::new()
                .write(true)
                .open(&self.device.mount_points[0])
                .map_err(|e| MosesError::Other(e.to_string()))?;
            
            file.seek(SeekFrom::Start(offset)).map_err(|e| MosesError::Other(e.to_string()))?;
            file.write_all(data).map_err(|e| MosesError::Other(e.to_string()))?;
        }
        
        Ok(())
    }
    
    /// Write data to allocated blocks
    pub(super) fn write_data_to_blocks_impl(
        &mut self,
        inode: &Ext4Inode,
        offset: u64,
        data: &[u8],
    ) -> Result<(), MosesError> {
        if data.is_empty() {
            return Ok(());
        }
        
        // Get the blocks allocated to this inode
        let blocks = self.get_all_inode_blocks(inode)?;
        
        // Calculate starting block and offset within that block
        let block_size = self.block_size as u64;
        let start_block_index = (offset / block_size) as usize;
        let start_offset = offset % block_size;
        
        // Check if we have enough blocks
        let end_offset = offset + data.len() as u64;
        let blocks_needed = ((end_offset + block_size - 1) / block_size) as usize;
        if blocks_needed > blocks.len() {
            return Err(MosesError::Other(format!(
                "Not enough blocks allocated: need {}, have {}",
                blocks_needed,
                blocks.len()
            )));
        }
        
        let mut data_offset = 0;
        let mut current_block = start_block_index;
        let mut block_offset = start_offset;
        
        while data_offset < data.len() && current_block < blocks.len() {
            let block_num = blocks[current_block];
            
            // Read current block data (for partial writes)
            let mut block_data = if block_offset != 0 || (data.len() - data_offset) < block_size as usize {
                self.read_block_from_disk(block_num)?
            } else {
                vec![0u8; self.block_size as usize]
            };
            
            // Calculate how much to write to this block
            let bytes_to_write = std::cmp::min(
                block_size as usize - block_offset as usize,
                data.len() - data_offset
            );
            
            // Copy data into block
            let end = block_offset as usize + bytes_to_write;
            block_data[block_offset as usize..end].copy_from_slice(
                &data[data_offset..data_offset + bytes_to_write]
            );
            
            // Write block back
            self.write_block_to_disk(block_num, &block_data)?;
            
            // Move to next block
            data_offset += bytes_to_write;
            current_block += 1;
            block_offset = 0;
        }
        
        Ok(())
    }
    
    /// Read data from blocks
    pub(super) fn read_data_from_blocks(
        &mut self,
        inode: &Ext4Inode,
        offset: u64,
        size: usize,
    ) -> Result<Vec<u8>, MosesError> {
        if size == 0 {
            return Ok(Vec::new());
        }
        
        // Get file size
        let file_size = inode.i_size_lo as u64 | ((inode.i_size_high as u64) << 32);
        if offset >= file_size {
            return Ok(Vec::new());
        }
        
        // Adjust size if it extends past EOF
        let actual_size = std::cmp::min(size as u64, file_size - offset) as usize;
        
        // Get the blocks allocated to this inode
        let blocks = self.get_all_inode_blocks(inode)?;
        
        // Calculate starting block and offset
        let block_size = self.block_size as u64;
        let start_block_index = (offset / block_size) as usize;
        let start_offset = offset % block_size;
        
        // Check if we have the required blocks
        let end_offset = offset + actual_size as u64;
        let _blocks_needed = ((end_offset + block_size - 1) / block_size) as usize;
        if start_block_index >= blocks.len() {
            return Ok(Vec::new());
        }
        
        let mut result = Vec::with_capacity(actual_size);
        let mut current_block = start_block_index;
        let mut block_offset = start_offset;
        
        while result.len() < actual_size && current_block < blocks.len() {
            let block_num = blocks[current_block];
            
            // Read block data
            let block_data = self.read_block_from_disk(block_num)?;
            
            // Calculate how much to read from this block
            let bytes_to_read = std::cmp::min(
                block_size as usize - block_offset as usize,
                actual_size - result.len()
            );
            
            // Copy data from block
            result.extend_from_slice(
                &block_data[block_offset as usize..block_offset as usize + bytes_to_read]
            );
            
            // Move to next block
            current_block += 1;
            block_offset = 0;
        }
        
        Ok(result)
    }
    
    /// Flush any pending writes to disk
    pub(super) fn flush_to_disk(&mut self) -> Result<(), MosesError> {
        // Platform-specific device sync
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        {
            use std::fs::OpenOptions;
            
            let file = OpenOptions::new()
                .write(true)
                .open(&self.device.mount_points[0])
                .map_err(|e| MosesError::Other(e.to_string()))?;
            
            file.sync_all().map_err(|e| MosesError::Other(e.to_string()))?;
        }
        
        Ok(())
    }
}