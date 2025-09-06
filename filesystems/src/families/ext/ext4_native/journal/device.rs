// Journal Device Implementation
// Provides access to journal blocks through inode or external device

use moses_core::{Device, MosesError};
use crate::families::ext::ext4_native::core::structures::Ext4Inode;
use std::io::{Read, Write, Seek, SeekFrom};
use std::fs::{File, OpenOptions};
use std::sync::Mutex;

/// Journal device implementation for inode-based journal
pub struct InodeJournalDevice {
    /// Device containing the filesystem
    device: Device,
    /// Journal inode
    journal_inode: Ext4Inode,
    /// Block size
    block_size: u32,
    /// File handle for device access
    file: Mutex<File>,
    /// Extent blocks for journal inode
    extent_blocks: Vec<u64>,
}

impl InodeJournalDevice {
    /// Create a new inode-based journal device
    pub fn new(
        device: Device,
        journal_inode: Ext4Inode,
        block_size: u32,
    ) -> Result<Self, MosesError> {
        // Open device for reading/writing
        let device_path = if !device.mount_points.is_empty() {
            device.mount_points[0].clone()
        } else {
            std::path::PathBuf::from(format!("/dev/{}", device.id))
        };
        
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&device_path)
            .map_err(|e| MosesError::Other(e.to_string()))?;
        
        // Parse extent tree to get journal blocks
        let extent_blocks = Self::parse_extent_blocks(&journal_inode, block_size)?;
        
        Ok(Self {
            device,
            journal_inode,
            block_size,
            file: Mutex::new(file),
            extent_blocks,
        })
    }
    
    /// Parse extent tree to get list of blocks
    fn parse_extent_blocks(inode: &Ext4Inode, _block_size: u32) -> Result<Vec<u64>, MosesError> {
        let mut blocks = Vec::new();
        
        // Check if inode uses extents
        if inode.i_flags & 0x80000 != 0 {  // EXT4_EXTENTS_FL
            // Parse extent header - i_block contains extent tree when using extents
            // Cast the u32 array to bytes for parsing
            let extent_data = unsafe {
                std::slice::from_raw_parts(
                    inode.i_block.as_ptr() as *const u8,
                    60  // 15 * 4 bytes
                )
            };
            let header = unsafe {
                std::ptr::read_unaligned(extent_data.as_ptr() as *const Ext4ExtentHeader)
            };
            
            if header.eh_magic != 0xF30A {
                return Err(MosesError::Other("Invalid extent header".to_string()));
            }
            
            // Handle based on depth
            if header.eh_depth == 0 {
                // Leaf nodes - direct extents
                blocks.extend(parse_extent_leaves(extent_data)?);
            } else {
                // Index nodes - need to read indirect blocks
                // For journal, we'll parse just the first level for now
                // Full recursive parsing would require device access
                blocks.extend(parse_extent_indices_simple(extent_data)?);
            }
        } else {
            // Traditional indirect blocks (for older filesystems)
            blocks.extend(parse_indirect_blocks(inode)?);
        }
        
        Ok(blocks)
    }
    
    /// Map journal block number to physical block
    fn map_journal_block(&self, journal_block: u64) -> Result<u64, MosesError> {
        if journal_block as usize >= self.extent_blocks.len() {
            return Err(MosesError::Other(format!(
                "Journal block {} out of range", journal_block
            )));
        }
        
        Ok(self.extent_blocks[journal_block as usize])
    }
}

/// Parse extent leaf nodes
fn parse_extent_leaves(extent_data: &[u8]) -> Result<Vec<u64>, MosesError> {
    let mut blocks = Vec::new();
    let header = unsafe {
        std::ptr::read_unaligned(extent_data.as_ptr() as *const Ext4ExtentHeader)
    };
    
    let entries_offset = std::mem::size_of::<Ext4ExtentHeader>();
    let entry_size = std::mem::size_of::<Ext4Extent>();
    
    for i in 0..header.eh_entries as usize {
        let offset = entries_offset + i * entry_size;
        if offset + entry_size <= extent_data.len() {
            let extent = unsafe {
                std::ptr::read_unaligned(
                    extent_data.as_ptr().add(offset) as *const Ext4Extent
                )
            };
            
            let start_block = extent.ee_start_lo as u64 | 
                            ((extent.ee_start_hi as u64) << 32);
            
            for j in 0..extent.ee_len as u64 {
                blocks.push(start_block + j);
            }
        }
    }
    
    Ok(blocks)
}

/// Parse extent index nodes (simplified - assumes contiguous journal)
fn parse_extent_indices_simple(extent_data: &[u8]) -> Result<Vec<u64>, MosesError> {
    let mut blocks = Vec::new();
    let header = unsafe {
        std::ptr::read_unaligned(extent_data.as_ptr() as *const Ext4ExtentHeader)
    };
    
    let entries_offset = std::mem::size_of::<Ext4ExtentHeader>();
    let _entry_size = std::mem::size_of::<Ext4ExtentIdx>();
    
    // For journal, we typically have a large contiguous allocation
    // Extract the first index which usually points to all journal blocks
    if header.eh_entries > 0 {
        let idx = unsafe {
            std::ptr::read_unaligned(
                extent_data.as_ptr().add(entries_offset) as *const Ext4ExtentIdx
            )
        };
        
        let leaf_block = idx.ei_leaf_lo as u64 | ((idx.ei_leaf_hi as u64) << 32);
        
        // Journal is typically allocated as contiguous blocks
        // Estimate based on typical journal size (32MB = 8192 4K blocks)
        for i in 0..8192 {
            blocks.push(leaf_block + i);
        }
    }
    
    Ok(blocks)
}

/// Parse traditional indirect blocks
fn parse_indirect_blocks(inode: &Ext4Inode) -> Result<Vec<u64>, MosesError> {
    let mut blocks = Vec::new();
    
    // Direct blocks (first 12)
    for i in 0..12 {
        let block = inode.i_block[i];
        if block != 0 {
            blocks.push(block as u64);
        }
    }
    
    // For journal, we rarely need indirect blocks
    // as 12 direct blocks * 4KB = 48KB is often enough
    
    Ok(blocks)
}

/// Extent header structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Ext4ExtentHeader {
    eh_magic: u16,
    eh_entries: u16,
    eh_max: u16,
    eh_depth: u16,
    eh_generation: u32,
}

/// Extent structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Ext4Extent {
    ee_block: u32,
    ee_len: u16,
    ee_start_hi: u16,
    ee_start_lo: u32,
}

/// Extent index structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Ext4ExtentIdx {
    ei_block: u32,
    ei_leaf_lo: u32,
    ei_leaf_hi: u16,
    ei_unused: u16,
}

impl super::jbd2::JournalDevice for InodeJournalDevice {
    fn read_block(&mut self, block: u64) -> Result<Vec<u8>, MosesError> {
        // Map journal block to physical block
        let physical_block = self.map_journal_block(block)?;
        let offset = physical_block * self.block_size as u64;
        
        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::Other(e.to_string()))?;
        
        let mut buffer = vec![0u8; self.block_size as usize];
        file.read_exact(&mut buffer)
            .map_err(|e| MosesError::Other(e.to_string()))?;
        
        Ok(buffer)
    }
    
    fn write_block(&mut self, block: u64, data: &[u8]) -> Result<(), MosesError> {
        if data.len() != self.block_size as usize {
            return Err(MosesError::Other(format!(
                "Invalid block size: expected {}, got {}",
                self.block_size, data.len()
            )));
        }
        
        // Map journal block to physical block
        let physical_block = self.map_journal_block(block)?;
        let offset = physical_block * self.block_size as u64;
        
        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::Other(e.to_string()))?;
        
        file.write_all(data)
            .map_err(|e| MosesError::Other(e.to_string()))?;
        
        Ok(())
    }
    
    fn sync(&mut self) -> Result<(), MosesError> {
        let file = self.file.lock().unwrap();
        file.sync_all()
            .map_err(|e| MosesError::Other(e.to_string()))?;
        Ok(())
    }
}

/// External journal device (for external journal on separate device)
pub struct ExternalJournalDevice {
    /// Path to journal device
    path: String,
    /// Block size
    block_size: u32,
    /// File handle
    file: Mutex<File>,
}

impl ExternalJournalDevice {
    /// Create a new external journal device
    pub fn new(path: String, block_size: u32) -> Result<Self, MosesError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| MosesError::Other(e.to_string()))?;
        
        Ok(Self {
            path,
            block_size,
            file: Mutex::new(file),
        })
    }
}

impl super::jbd2::JournalDevice for ExternalJournalDevice {
    fn read_block(&mut self, block: u64) -> Result<Vec<u8>, MosesError> {
        let offset = block * self.block_size as u64;
        
        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::Other(e.to_string()))?;
        
        let mut buffer = vec![0u8; self.block_size as usize];
        file.read_exact(&mut buffer)
            .map_err(|e| MosesError::Other(e.to_string()))?;
        
        Ok(buffer)
    }
    
    fn write_block(&mut self, block: u64, data: &[u8]) -> Result<(), MosesError> {
        if data.len() != self.block_size as usize {
            return Err(MosesError::Other(format!(
                "Invalid block size: expected {}, got {}",
                self.block_size, data.len()
            )));
        }
        
        let offset = block * self.block_size as u64;
        
        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::Other(e.to_string()))?;
        
        file.write_all(data)
            .map_err(|e| MosesError::Other(e.to_string()))?;
        
        Ok(())
    }
    
    fn sync(&mut self) -> Result<(), MosesError> {
        let file = self.file.lock().unwrap();
        file.sync_all()
            .map_err(|e| MosesError::Other(e.to_string()))?;
        Ok(())
    }
}