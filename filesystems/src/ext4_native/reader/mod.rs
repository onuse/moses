// Ext filesystem reader - supports ext2/ext3/ext4
// This allows reading ext filesystems on any platform!

use moses_core::{Device, MosesError};
use log::info;
use std::collections::HashMap;

use super::core::{
    structures::*,
    constants::*,
    ext_config::ExtVersion,
};

/// Entry in a directory
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub inode: u32,
    pub entry_type: FileType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileType {
    Unknown = 0,
    Regular = 1,
    Directory = 2,
    CharDevice = 3,
    BlockDevice = 4,
    Fifo = 5,
    Socket = 6,
    Symlink = 7,
}

impl From<u8> for FileType {
    fn from(val: u8) -> Self {
        match val {
            1 => FileType::Regular,
            2 => FileType::Directory,
            3 => FileType::CharDevice,
            4 => FileType::BlockDevice,
            5 => FileType::Fifo,
            6 => FileType::Socket,
            7 => FileType::Symlink,
            _ => FileType::Unknown,
        }
    }
}

/// Metadata for a file/directory
#[derive(Debug)]
pub struct FileMetadata {
    pub size: u64,
    pub blocks: u64,
    pub mode: u16,
    pub uid: u32,
    pub gid: u32,
    pub atime: i32,  // Unix timestamp
    pub mtime: i32,  // Unix timestamp
    pub ctime: i32,  // Unix timestamp
    pub links: u16,
    pub file_type: FileType,
}

/// Ext filesystem reader
pub struct ExtReader {
    device: Device,
    superblock: Ext4Superblock,
    group_descriptors: Vec<Ext4GroupDesc>,
    block_size: u32,
    inode_size: u32,
    pub version: ExtVersion,
    
    // Cache for performance
    inode_cache: HashMap<u32, Ext4Inode>,
    block_cache: HashMap<u64, Vec<u8>>,
}

impl ExtReader {
    /// Detect the ext filesystem version from superblock features
    fn detect_version(sb: &Ext4Superblock) -> ExtVersion {
        // Check feature flags to determine version
        let has_journal = sb.s_feature_compat & EXT4_FEATURE_COMPAT_HAS_JOURNAL != 0;
        let has_extents = sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_EXTENTS != 0;
        let has_64bit = sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_64BIT != 0;
        let has_metadata_csum = sb.s_feature_ro_compat & EXT4_FEATURE_RO_COMPAT_METADATA_CSUM != 0;
        
        // ext4 has extents or 64-bit or metadata checksums
        if has_extents || has_64bit || has_metadata_csum {
            ExtVersion::Ext4
        }
        // ext3 has journal but no ext4 features
        else if has_journal {
            ExtVersion::Ext3
        }
        // ext2 has neither journal nor ext4 features
        else {
            ExtVersion::Ext2
        }
    }
    
    /// Open an ext filesystem for reading
    pub fn new(device: Device) -> Result<Self, MosesError> {
        info!("Opening ext filesystem on device: {}", device.name);
        
        // Read superblock
        let superblock = Self::read_superblock(&device)?;
        
        // Detect version
        let version = Self::detect_version(&superblock);
        info!("Detected {} filesystem", match version {
            ExtVersion::Ext2 => "ext2",
            ExtVersion::Ext3 => "ext3",
            ExtVersion::Ext4 => "ext4",
        });
        
        // Validate magic
        if superblock.s_magic != EXT4_SUPER_MAGIC {
            return Err(MosesError::Other(format!(
                "Invalid ext magic: 0x{:X}", superblock.s_magic
            )));
        }
        
        let block_size = superblock.s_block_size();
        let inode_size = superblock.s_inode_size as u32;
        
        // Read group descriptors
        let num_groups = ((superblock.s_blocks_count_lo as u64 
                          | ((superblock.s_blocks_count_hi as u64) << 32))
                          + superblock.s_blocks_per_group as u64 - 1)
                         / superblock.s_blocks_per_group as u64;
        
        let mut group_descriptors = Vec::new();
        let gdt_block = if block_size == 1024 { 2 } else { 1 };
        
        for i in 0..num_groups {
            let gd = Self::read_group_descriptor(&device, &superblock, gdt_block, i as u32)?;
            group_descriptors.push(gd);
        }
        
        Ok(ExtReader {
            device,
            superblock,
            group_descriptors,
            block_size,
            inode_size,
            version,
            inode_cache: HashMap::new(),
            block_cache: HashMap::new(),
        })
    }
    
    /// Read superblock from device
    fn read_superblock(device: &Device) -> Result<Ext4Superblock, MosesError> {
        use crate::utils::{open_device_read, read_block};
        
        let mut file = open_device_read(device)?;
        
        // Superblock is at offset 1024
        let buffer = read_block(&mut file, 1024, 1024)?;
        
        // Parse superblock
        let sb = unsafe {
            std::ptr::read_unaligned(buffer.as_ptr() as *const Ext4Superblock)
        };
        
        Ok(sb)
    }
    
    /// Read group descriptor
    fn read_group_descriptor(
        device: &Device,
        sb: &Ext4Superblock,
        gdt_block: u64,
        group_index: u32,
    ) -> Result<Ext4GroupDesc, MosesError> {
        use crate::utils::{open_device_read, read_block};
        
        let mut file = open_device_read(device)?;
        
        let block_size = sb.s_block_size();
        let gd_size = 64; // Size of group descriptor
        let offset = (gdt_block * block_size as u64) + (group_index as u64 * gd_size);
        
        let buffer = read_block(&mut file, offset, 64)?;
        
        let gd = unsafe {
            std::ptr::read_unaligned(buffer.as_ptr() as *const Ext4GroupDesc)
        };
        
        Ok(gd)
    }
    
    /// Read an inode by number
    pub fn read_inode(&mut self, inode_num: u32) -> Result<Ext4Inode, MosesError> {
        // Check cache first
        if let Some(cached) = self.inode_cache.get(&inode_num) {
            return Ok(*cached);
        }
        
        if inode_num == 0 || inode_num > self.superblock.s_inodes_count {
            return Err(MosesError::Other(format!("Invalid inode number: {}", inode_num)));
        }
        
        // Calculate inode location
        let inodes_per_group = self.superblock.s_inodes_per_group;
        let group = (inode_num - 1) / inodes_per_group;
        let index = (inode_num - 1) % inodes_per_group;
        
        let gd = &self.group_descriptors[group as usize];
        let inode_table_block = gd.bg_inode_table_lo as u64 
                               | ((gd.bg_inode_table_hi as u64) << 32);
        
        let inode_offset = inode_table_block * self.block_size as u64
                          + index as u64 * self.inode_size as u64;
        
        // Read inode from device
        use crate::utils::{open_device_read, read_block};
        
        let mut file = open_device_read(&self.device)?;
        let buffer = read_block(&mut file, inode_offset, self.inode_size as usize)?;
        
        let inode = unsafe {
            std::ptr::read_unaligned(buffer.as_ptr() as *const Ext4Inode)
        };
        
        // Cache it
        self.inode_cache.insert(inode_num, inode);
        
        Ok(inode)
    }
    
    /// Read a block by number
    pub fn read_block(&mut self, block_num: u64) -> Result<Vec<u8>, MosesError> {
        use crate::utils::{open_device_read, read_block};
        
        // Check cache first
        if let Some(cached) = self.block_cache.get(&block_num) {
            return Ok(cached.clone());
        }
        
        let offset = block_num * self.block_size as u64;
        
        let mut file = open_device_read(&self.device)?;
        let buffer = read_block(&mut file, offset, self.block_size as usize)?;
        
        // Cache if not too many cached already
        if self.block_cache.len() < 100 {
            self.block_cache.insert(block_num, buffer.clone());
        }
        
        Ok(buffer)
    }
    
    /// List directory contents
    pub fn read_directory(&mut self, path: &str) -> Result<Vec<DirEntry>, MosesError> {
        info!("Reading directory: {}", path);
        
        // Get inode for path
        let inode_num = self.path_to_inode(path)?;
        let inode = self.read_inode(inode_num)?;
        
        // Check if it's a directory
        if inode.i_mode & 0xF000 != 0x4000 {
            return Err(MosesError::Other(format!("{} is not a directory", path)));
        }
        
        let mut entries = Vec::new();
        
        // Read directory blocks
        let blocks = self.get_inode_blocks(&inode)?;
        
        for block_num in blocks {
            if block_num == 0 { continue; }
            
            let block_data = self.read_block(block_num)?;
            let mut offset = 0;
            
            while offset < block_data.len() {
                // Parse directory entry
                let entry = unsafe {
                    &*(block_data.as_ptr().add(offset) as *const Ext4DirEntry2)
                };
                
                if entry.inode == 0 {
                    // Deleted or empty entry
                    offset += entry.rec_len as usize;
                    continue;
                }
                
                // Get name
                let name_bytes = unsafe {
                    std::slice::from_raw_parts(
                        block_data.as_ptr().add(offset + 8),
                        entry.name_len as usize
                    )
                };
                
                let name = String::from_utf8_lossy(name_bytes).to_string();
                
                entries.push(DirEntry {
                    name,
                    inode: entry.inode,
                    entry_type: FileType::from(entry.file_type),
                });
                
                offset += entry.rec_len as usize;
                if entry.rec_len == 0 { break; }
            }
        }
        
        Ok(entries)
    }
    
    /// Get blocks for an inode (handles both extents and indirect blocks)
    fn get_inode_blocks(&mut self, inode: &Ext4Inode) -> Result<Vec<u64>, MosesError> {
        let mut blocks = Vec::new();
        
        // Check if using extents (ext4) or indirect blocks (ext2/ext3)
        if inode.i_flags & EXT4_EXTENTS_FL != 0 {
            // Parse extent tree
            let header = unsafe {
                &*(inode.i_block.as_ptr() as *const Ext4ExtentHeader)
            };
            
            if header.eh_magic != 0xF30A {
                return Err(MosesError::Other("Invalid extent header".to_string()));
            }
            
            // For simplicity, only handle leaf extents for now
            if header.eh_depth == 0 {
                let extents = unsafe {
                    std::slice::from_raw_parts(
                        inode.i_block.as_ptr().add(12) as *const Ext4Extent,
                        header.eh_entries as usize
                    )
                };
                
                for extent in extents {
                    let start_block = extent.ee_start_lo as u64 
                                    | ((extent.ee_start_hi as u64) << 32);
                    for i in 0..extent.ee_len {
                        blocks.push(start_block + i as u64);
                    }
                }
            }
        } else {
            // Traditional indirect blocks (ext2/ext3)
            // Direct blocks (first 12)
            for i in 0..12 {
                let block = unsafe {
                    *(inode.i_block.as_ptr().add(i * 4) as *const u32)
                };
                if block != 0 {
                    blocks.push(block as u64);
                }
            }
            
            // TODO: Handle indirect, double-indirect, triple-indirect blocks
        }
        
        Ok(blocks)
    }
    
    /// Resolve a path to an inode number
    fn path_to_inode(&mut self, path: &str) -> Result<u32, MosesError> {
        let mut current_inode = EXT4_ROOT_INO;
        
        if path == "/" {
            return Ok(current_inode);
        }
        
        let components: Vec<&str> = path.split('/')
            .filter(|s| !s.is_empty())
            .collect();
        
        for component in components {
            let entries = self.read_directory_inode(current_inode)?;
            
            let entry = entries.iter()
                .find(|e| e.name == component)
                .ok_or_else(|| MosesError::Other(
                    format!("Path component '{}' not found", component)
                ))?;
            
            current_inode = entry.inode;
        }
        
        Ok(current_inode)
    }
    
    /// Read directory by inode number
    fn read_directory_inode(&mut self, inode_num: u32) -> Result<Vec<DirEntry>, MosesError> {
        let inode = self.read_inode(inode_num)?;
        
        // Check if it's a directory
        if inode.i_mode & 0xF000 != 0x4000 {
            return Err(MosesError::Other("Not a directory".to_string()));
        }
        
        let mut entries = Vec::new();
        let blocks = self.get_inode_blocks(&inode)?;
        
        for block_num in blocks {
            if block_num == 0 { continue; }
            
            let block_data = self.read_block(block_num)?;
            let mut offset = 0;
            
            while offset < block_data.len() {
                let entry = unsafe {
                    &*(block_data.as_ptr().add(offset) as *const Ext4DirEntry2)
                };
                
                if entry.inode == 0 {
                    offset += entry.rec_len as usize;
                    continue;
                }
                
                let name_bytes = unsafe {
                    std::slice::from_raw_parts(
                        block_data.as_ptr().add(offset + 8),
                        entry.name_len as usize
                    )
                };
                
                let name = String::from_utf8_lossy(name_bytes).to_string();
                
                entries.push(DirEntry {
                    name,
                    inode: entry.inode,
                    entry_type: FileType::from(entry.file_type),
                });
                
                offset += entry.rec_len as usize;
                if entry.rec_len == 0 { break; }
            }
        }
        
        Ok(entries)
    }
    
    /// Read file contents
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>, MosesError> {
        info!("Reading file: {}", path);
        
        let inode_num = self.path_to_inode(path)?;
        let inode = self.read_inode(inode_num)?;
        
        // Check if it's a regular file
        if inode.i_mode & 0xF000 != 0x8000 {
            return Err(MosesError::Other(format!("{} is not a regular file", path)));
        }
        
        let file_size = inode.i_size_lo as u64 | ((inode.i_size_high as u64) << 32);
        let mut file_data = Vec::with_capacity(file_size as usize);
        
        let blocks = self.get_inode_blocks(&inode)?;
        let mut bytes_read = 0u64;
        
        for block_num in blocks {
            if block_num == 0 { continue; }
            
            let block_data = self.read_block(block_num)?;
            
            // Calculate how much to read from this block
            let bytes_to_read = std::cmp::min(
                self.block_size as u64,
                file_size - bytes_read
            ) as usize;
            
            file_data.extend_from_slice(&block_data[..bytes_to_read]);
            bytes_read += bytes_to_read as u64;
            
            if bytes_read >= file_size {
                break;
            }
        }
        
        Ok(file_data)
    }
    
    /// Get file metadata
    pub fn stat(&mut self, path: &str) -> Result<FileMetadata, MosesError> {
        let inode_num = self.path_to_inode(path)?;
        let inode = self.read_inode(inode_num)?;
        
        let file_type = match inode.i_mode & 0xF000 {
            0x8000 => FileType::Regular,
            0x4000 => FileType::Directory,
            0x2000 => FileType::CharDevice,
            0x6000 => FileType::BlockDevice,
            0x1000 => FileType::Fifo,
            0xC000 => FileType::Socket,
            0xA000 => FileType::Symlink,
            _ => FileType::Unknown,
        };
        
        Ok(FileMetadata {
            size: inode.i_size_lo as u64 | ((inode.i_size_high as u64) << 32),
            blocks: inode.i_blocks_lo as u64, // blocks_hi would be in osd2 for ext4
            mode: inode.i_mode,
            uid: inode.i_uid as u32,
            gid: inode.i_gid as u32,
            atime: inode.i_atime as i32,
            mtime: inode.i_mtime as i32,
            ctime: inode.i_ctime as i32,
            links: inode.i_links_count,
            file_type,
        })
    }
    
    /// Get filesystem information including volume label
    pub fn get_info(&self) -> ExtInfo {
        let label = String::from_utf8_lossy(&self.superblock.s_volume_name)
            .trim_end_matches('\0')
            .trim()
            .to_string();
        
        // Calculate 64-bit block counts
        let block_count = self.superblock.s_blocks_count_lo as u64 
            | ((self.superblock.s_blocks_count_hi as u64) << 32);
        let free_blocks = self.superblock.s_free_blocks_count_lo as u64 
            | ((self.superblock.s_free_blocks_count_hi as u64) << 32);
        let reserved_blocks = self.superblock.s_r_blocks_count_lo as u64 
            | ((self.superblock.s_r_blocks_count_hi as u64) << 32);
        
        ExtInfo {
            filesystem_type: match self.version {
                ExtVersion::Ext2 => "ext2".to_string(),
                ExtVersion::Ext3 => "ext3".to_string(),
                ExtVersion::Ext4 => "ext4".to_string(),
            },
            label: if label.is_empty() { None } else { Some(label) },
            uuid: self.format_uuid(),
            block_count,
            free_blocks,
            block_size: self.block_size,
            total_inodes: self.superblock.s_inodes_count,
            free_inodes: self.superblock.s_free_inodes_count,
            reserved_blocks,
        }
    }
    
    /// Format UUID as string
    fn format_uuid(&self) -> Option<String> {
        let uuid = &self.superblock.s_uuid;
        if uuid.iter().all(|&b| b == 0) {
            return None;
        }
        
        Some(format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            uuid[0], uuid[1], uuid[2], uuid[3],
            uuid[4], uuid[5],
            uuid[6], uuid[7],
            uuid[8], uuid[9],
            uuid[10], uuid[11], uuid[12], uuid[13], uuid[14], uuid[15]
        ))
    }
}

/// Information about an ext filesystem
#[derive(Debug)]
pub struct ExtInfo {
    pub filesystem_type: String,
    pub label: Option<String>,
    pub uuid: Option<String>,
    pub block_count: u64,
    pub free_blocks: u64,
    pub block_size: u32,
    pub total_inodes: u32,
    pub free_inodes: u32,
    pub reserved_blocks: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ext_reader_creation() {
        // This would need a test device or image
        // For now, just ensure the module compiles
    }
}