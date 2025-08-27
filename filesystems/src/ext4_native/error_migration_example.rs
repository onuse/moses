// EXT4 Error Migration Example - Using Enhanced MosesError
// Shows practical migration patterns for EXT4 operations

use moses_core::error::{MosesError, CorruptionLevel, MosesResult, ErrorContext};
use std::path::PathBuf;

/// Example EXT4 inode operations with enhanced error handling
pub struct Ext4InodeOperations;

impl Ext4InodeOperations {
    /// Read inode with enhanced error handling
    pub fn read_inode(
        &mut self,
        device: &mut std::fs::File,
        inode_number: u32,
        superblock: &Ext4Superblock,
    ) -> MosesResult<Ext4Inode> {
        // Validate inode number
        if inode_number == 0 || inode_number > superblock.total_inodes {
            return Err(MosesError::InvalidArgument {
                message: format!(
                    "Invalid inode number {}, valid range is 1..{}",
                    inode_number, superblock.total_inodes
                ),
            });
        }
        
        // Calculate inode location
        let group = (inode_number - 1) / superblock.inodes_per_group;
        let index = (inode_number - 1) % superblock.inodes_per_group;
        
        let group_desc = self.read_group_descriptor(device, group, superblock)?;
        let inode_table_block = group_desc.inode_table;
        
        let offset = (inode_table_block * superblock.block_size as u64) +
                    (index as u64 * superblock.inode_size as u64);
        
        // Read inode data
        use std::io::{Seek, SeekFrom, Read};
        device.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::io(e, offset))
            .context("Seeking to inode")?;
        
        let mut inode_data = vec![0u8; superblock.inode_size as usize];
        device.read_exact(&mut inode_data)
            .map_err(|e| MosesError::io(e, offset))
            .fs_context("EXT4", &format!("reading inode {}", inode_number))?;
        
        // Parse and validate inode
        self.parse_inode(&inode_data, inode_number)
    }
    
    /// Parse inode from raw data
    fn parse_inode(&self, data: &[u8], inode_number: u32) -> MosesResult<Ext4Inode> {
        if data.len() < 128 {
            return Err(MosesError::corruption(
                format!("Inode {} data too small: {} bytes", inode_number, data.len()),
                CorruptionLevel::Severe,
            ));
        }
        
        let mode = u16::from_le_bytes([data[0], data[1]]);
        let size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let links_count = u16::from_le_bytes([data[26], data[27]]);
        
        // Validate inode magic (for ext4)
        if data.len() >= 256 {
            let magic = u16::from_le_bytes([data[0xFC], data[0xFD]]);
            if magic != 0xEF53 && magic != 0 {
                return Err(MosesError::ValidationFailed {
                    field: "inode_magic".into(),
                    expected: "EF53".into(),
                    actual: format!("{:04X}", magic),
                });
            }
        }
        
        // Check for corruption indicators
        if links_count == 0 && mode != 0 {
            return Err(MosesError::corruption(
                format!("Inode {} has mode but no links", inode_number),
                CorruptionLevel::Moderate,
            ));
        }
        
        Ok(Ext4Inode {
            mode,
            size,
            links_count,
            blocks: self.parse_blocks(&data[40..100])?,
        })
    }
    
    /// Parse block pointers
    fn parse_blocks(&self, data: &[u8]) -> MosesResult<Vec<u32>> {
        let mut blocks = Vec::new();
        
        for chunk in data.chunks(4) {
            if chunk.len() != 4 {
                return Err(MosesError::corruption(
                    "Incomplete block pointer in inode",
                    CorruptionLevel::Minor,
                ));
            }
            
            let block = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            blocks.push(block);
        }
        
        Ok(blocks)
    }
    
    /// Read group descriptor
    fn read_group_descriptor(
        &mut self,
        device: &mut std::fs::File,
        group: u32,
        superblock: &Ext4Superblock,
    ) -> MosesResult<Ext4GroupDesc> {
        let gdt_block = if superblock.block_size == 1024 { 2 } else { 1 };
        let offset = (gdt_block * superblock.block_size as u64) +
                    (group as u64 * 32); // 32 bytes per group descriptor
        
        use std::io::{Seek, SeekFrom, Read};
        device.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::io(e, offset))?;
        
        let mut gdt_data = [0u8; 32];
        device.read_exact(&mut gdt_data)
            .map_err(|e| MosesError::io(e, offset))
            .context("Reading group descriptor")?;
        
        Ok(Ext4GroupDesc {
            block_bitmap: u32::from_le_bytes([gdt_data[0], gdt_data[1], gdt_data[2], gdt_data[3]]),
            inode_bitmap: u32::from_le_bytes([gdt_data[4], gdt_data[5], gdt_data[6], gdt_data[7]]),
            inode_table: u32::from_le_bytes([gdt_data[8], gdt_data[9], gdt_data[10], gdt_data[11]]),
        })
    }
    
    /// Write inode with safety checks
    pub fn write_inode(
        &mut self,
        device: &mut std::fs::File,
        inode_number: u32,
        inode: &Ext4Inode,
        superblock: &Ext4Superblock,
    ) -> MosesResult<()> {
        // Safety check - don't write to system inodes
        if inode_number <= 10 {
            return Err(MosesError::SafetyViolation {
                message: format!("Refusing to modify system inode {}", inode_number),
            });
        }
        
        // Build inode data
        let mut inode_data = vec![0u8; superblock.inode_size as usize];
        
        // Mode
        inode_data[0..2].copy_from_slice(&inode.mode.to_le_bytes());
        
        // Size
        inode_data[4..8].copy_from_slice(&inode.size.to_le_bytes());
        
        // Links count
        inode_data[26..28].copy_from_slice(&inode.links_count.to_le_bytes());
        
        // Calculate location and write
        let group = (inode_number - 1) / superblock.inodes_per_group;
        let index = (inode_number - 1) % superblock.inodes_per_group;
        
        let group_desc = self.read_group_descriptor(device, group, superblock)?;
        let offset = (group_desc.inode_table * superblock.block_size as u64) +
                    (index as u64 * superblock.inode_size as u64);
        
        use std::io::{Seek, SeekFrom, Write};
        device.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::io(e, offset))?;
        
        device.write_all(&inode_data)
            .map_err(|e| MosesError::io(e, offset))
            .fs_context("EXT4", &format!("writing inode {}", inode_number))?;
        
        device.flush()
            .map_err(|e| MosesError::io_simple(e))?;
        
        Ok(())
    }
}

/// Example EXT4 extent operations
pub struct Ext4ExtentOperations;

impl Ext4ExtentOperations {
    /// Parse extent tree with corruption detection
    pub fn parse_extent_header(&self, data: &[u8]) -> MosesResult<Ext4ExtentHeader> {
        if data.len() < 12 {
            return Err(MosesError::corruption(
                "Extent header too small",
                CorruptionLevel::Severe,
            ));
        }
        
        let magic = u16::from_le_bytes([data[0], data[1]]);
        if magic != 0xF30A {
            return Err(MosesError::ValidationFailed {
                field: "extent_magic".into(),
                expected: "F30A".into(),
                actual: format!("{:04X}", magic),
            });
        }
        
        let entries = u16::from_le_bytes([data[2], data[3]]);
        let max = u16::from_le_bytes([data[4], data[5]]);
        let depth = u16::from_le_bytes([data[6], data[7]]);
        
        // Validate header fields
        if entries > max {
            return Err(MosesError::corruption(
                format!("Extent entries {} exceeds max {}", entries, max),
                CorruptionLevel::Moderate,
            ).at_offset(2));
        }
        
        if depth > 5 {
            return Err(MosesError::corruption(
                format!("Extent tree depth {} too large", depth),
                CorruptionLevel::Minor,
            ));
        }
        
        Ok(Ext4ExtentHeader {
            magic,
            entries,
            max,
            depth,
        })
    }
    
    /// Find extent for logical block
    pub fn find_extent(
        &self,
        extent_tree: &[u8],
        logical_block: u32,
    ) -> MosesResult<Option<Ext4Extent>> {
        let header = self.parse_extent_header(extent_tree)?;
        
        if header.depth == 0 {
            // Leaf node - contains extents
            self.search_extent_leaf(extent_tree, logical_block, header.entries)
        } else {
            // Internal node - contains index entries
            Err(MosesError::NotImplemented {
                feature: "Extent tree traversal".into(),
            })
        }
    }
    
    fn search_extent_leaf(
        &self,
        data: &[u8],
        logical_block: u32,
        entries: u16,
    ) -> MosesResult<Option<Ext4Extent>> {
        let mut offset = 12; // Skip header
        
        for i in 0..entries {
            if offset + 12 > data.len() {
                return Err(MosesError::corruption(
                    format!("Extent entry {} extends beyond data", i),
                    CorruptionLevel::Moderate,
                ));
            }
            
            let ee_block = u32::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
            ]);
            let ee_len = u16::from_le_bytes([data[offset + 4], data[offset + 5]]);
            let ee_start_hi = u16::from_le_bytes([data[offset + 6], data[offset + 7]]);
            let ee_start_lo = u32::from_le_bytes([
                data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]
            ]);
            
            let physical_block = ((ee_start_hi as u64) << 32) | ee_start_lo as u64;
            
            // Check if logical block is in this extent
            if logical_block >= ee_block && logical_block < ee_block + ee_len as u32 {
                return Ok(Some(Ext4Extent {
                    logical_block: ee_block,
                    length: ee_len,
                    physical_block,
                }));
            }
            
            offset += 12;
        }
        
        Ok(None)
    }
}

/// Example directory operations
pub struct Ext4DirectoryOperations;

impl Ext4DirectoryOperations {
    /// Read directory entry with validation
    pub fn read_directory_entry(
        &self,
        data: &[u8],
        offset: usize,
    ) -> MosesResult<Option<Ext4DirEntry>> {
        if offset + 8 > data.len() {
            return Ok(None); // End of directory
        }
        
        let inode = u32::from_le_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
        ]);
        let rec_len = u16::from_le_bytes([data[offset + 4], data[offset + 5]]);
        let name_len = data[offset + 6];
        let file_type = data[offset + 7];
        
        // Validate directory entry
        if rec_len < 8 {
            return Err(MosesError::corruption(
                format!("Directory entry rec_len {} too small", rec_len),
                CorruptionLevel::Moderate,
            ).at_offset(offset as u64 + 4));
        }
        
        if rec_len as usize > data.len() - offset {
            return Err(MosesError::corruption(
                "Directory entry extends beyond block",
                CorruptionLevel::Moderate,
            ));
        }
        
        if name_len as usize > rec_len as usize - 8 {
            return Err(MosesError::corruption(
                format!("Name length {} exceeds record length {}", name_len, rec_len),
                CorruptionLevel::Minor,
            ));
        }
        
        // Read name
        let name_end = offset + 8 + name_len as usize;
        if name_end > data.len() {
            return Err(MosesError::corruption(
                "Directory entry name extends beyond data",
                CorruptionLevel::Moderate,
            ));
        }
        
        let name = String::from_utf8_lossy(&data[offset + 8..name_end]).to_string();
        
        // Check for invalid names
        if name.contains('\0') {
            return Err(MosesError::InvalidPath {
                path: name.clone(),
                reason: "Null character in filename".into(),
            });
        }
        
        Ok(Some(Ext4DirEntry {
            inode,
            rec_len,
            name_len,
            file_type,
            name,
        }))
    }
    
    /// Add directory entry with safety checks
    pub fn add_directory_entry(
        &mut self,
        parent_inode: u32,
        name: &str,
        target_inode: u32,
        file_type: u8,
    ) -> MosesResult<()> {
        // Validate filename
        if name.is_empty() {
            return Err(MosesError::InvalidArgument {
                message: "Cannot create directory entry with empty name".into(),
            });
        }
        
        if name.len() > 255 {
            return Err(MosesError::InvalidPath {
                path: name.into(),
                reason: "Filename too long (max 255 bytes)".into(),
            });
        }
        
        if name == "." || name == ".." {
            return Err(MosesError::PathExists {
                path: PathBuf::from(name),
            });
        }
        
        // Check for invalid characters
        for ch in name.chars() {
            if ch == '\0' || ch == '/' {
                return Err(MosesError::InvalidPath {
                    path: name.into(),
                    reason: format!("Invalid character '{}' in filename", 
                        if ch == '\0' { "\\0" } else { "/" }),
                });
            }
        }
        
        // Would implement actual directory modification here
        Ok(())
    }
}

// Data structures
pub struct Ext4Superblock {
    pub block_size: u32,
    pub total_inodes: u32,
    pub inodes_per_group: u32,
    pub inode_size: u32,
}

pub struct Ext4Inode {
    pub mode: u16,
    pub size: u32,
    pub links_count: u16,
    pub blocks: Vec<u32>,
}

pub struct Ext4GroupDesc {
    pub block_bitmap: u32,
    pub inode_bitmap: u32,
    pub inode_table: u32,
}

pub struct Ext4ExtentHeader {
    pub magic: u16,
    pub entries: u16,
    pub max: u16,
    pub depth: u16,
}

pub struct Ext4Extent {
    pub logical_block: u32,
    pub length: u16,
    pub physical_block: u64,
}

pub struct Ext4DirEntry {
    pub inode: u32,
    pub rec_len: u16,
    pub name_len: u8,
    pub file_type: u8,
    pub name: String,
}

/// Example showing migration patterns:
/// 
/// OLD CODE:
/// ```
/// fn read_inode(num: u32) -> Result<Inode, MosesError> {
///     if num == 0 {
///         return Err(MosesError::Other("Invalid inode".to_string()));
///     }
///     // ... read logic ...
/// }
/// ```
/// 
/// MIGRATED CODE (shown above):
/// - Uses InvalidArgument for parameter validation
/// - Uses IoError with offset for I/O operations
/// - Uses ValidationFailed for magic number checks
/// - Uses Corruption with severity levels for data integrity issues
/// - Uses SafetyViolation for dangerous operations
/// - Uses PathExists and InvalidPath for path-related errors
/// - Adds context with .context() and .fs_context()

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extent_header_validation() {
        let ops = Ext4ExtentOperations;
        
        // Valid header
        let valid_data = vec![
            0x0A, 0xF3, // magic
            0x02, 0x00, // entries
            0x04, 0x00, // max
            0x00, 0x00, // depth
            0x00, 0x00, 0x00, 0x00, // generation
        ];
        assert!(ops.parse_extent_header(&valid_data).is_ok());
        
        // Invalid magic
        let mut invalid_magic = valid_data.clone();
        invalid_magic[0] = 0xFF;
        let err = ops.parse_extent_header(&invalid_magic).unwrap_err();
        assert!(matches!(err, MosesError::ValidationFailed { field, .. } if field == "extent_magic"));
        
        // Corrupt entries > max
        let mut corrupt_entries = valid_data.clone();
        corrupt_entries[2] = 0x05; // entries = 5, max = 4
        let err = ops.parse_extent_header(&corrupt_entries).unwrap_err();
        assert!(matches!(err, MosesError::Corruption { .. }));
    }
}