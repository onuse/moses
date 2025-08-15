// Common types used throughout the ext4 implementation

use std::fmt;

/// Result type for ext4 operations
pub type Ext4Result<T> = Result<T, Ext4Error>;

/// Errors that can occur during ext4 operations
#[derive(Debug, Clone)]
pub enum Ext4Error {
    /// I/O error
    Io(String),
    /// Invalid filesystem parameters
    InvalidParameters(String),
    /// Checksum mismatch
    ChecksumMismatch { expected: u32, actual: u32 },
    /// Structure validation failed
    ValidationFailed(String),
    /// Alignment error
    AlignmentError(String),
    /// Device too small
    DeviceTooSmall { required: u64, actual: u64 },
    /// Feature not supported
    UnsupportedFeature(String),
    /// Windows-specific error
    #[cfg(target_os = "windows")]
    WindowsError(String),
}

impl fmt::Display for Ext4Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ext4Error::Io(msg) => write!(f, "I/O error: {}", msg),
            Ext4Error::InvalidParameters(msg) => write!(f, "Invalid parameters: {}", msg),
            Ext4Error::ChecksumMismatch { expected, actual } => {
                write!(f, "Checksum mismatch: expected 0x{:08X}, got 0x{:08X}", expected, actual)
            }
            Ext4Error::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
            Ext4Error::AlignmentError(msg) => write!(f, "Alignment error: {}", msg),
            Ext4Error::DeviceTooSmall { required, actual } => {
                write!(f, "Device too small: requires {} bytes, got {} bytes", required, actual)
            }
            Ext4Error::UnsupportedFeature(msg) => write!(f, "Unsupported feature: {}", msg),
            #[cfg(target_os = "windows")]
            Ext4Error::WindowsError(msg) => write!(f, "Windows error: {}", msg),
        }
    }
}

impl std::error::Error for Ext4Error {}

impl From<std::io::Error> for Ext4Error {
    fn from(error: std::io::Error) -> Self {
        Ext4Error::Io(error.to_string())
    }
}

impl From<String> for Ext4Error {
    fn from(error: String) -> Self {
        Ext4Error::Io(error)
    }
}

/// Block number type (64-bit for ext4)
pub type BlockNumber = u64;

/// Inode number type
pub type InodeNumber = u32;

/// Group number type
pub type GroupNumber = u32;

/// Filesystem parameters
#[derive(Debug, Clone)]
pub struct FilesystemParams {
    /// Total size in bytes
    pub size_bytes: u64,
    /// Block size (usually 4096)
    pub block_size: u32,
    /// Inode size (usually 256)
    pub inode_size: u16,
    /// Volume label
    pub label: Option<String>,
    /// Reserved blocks percentage
    pub reserved_percent: u32,
    /// Enable checksums
    pub enable_checksums: bool,
    /// Enable 64-bit support
    pub enable_64bit: bool,
    /// Enable journal
    pub enable_journal: bool,
}

impl Default for FilesystemParams {
    fn default() -> Self {
        Self {
            size_bytes: 0,
            block_size: 4096,
            inode_size: 256,
            label: None,
            reserved_percent: 5,
            enable_checksums: true,
            enable_64bit: true,
            enable_journal: false, // Not implemented yet
        }
    }
}

/// Filesystem layout information
#[derive(Debug, Clone)]
pub struct FilesystemLayout {
    pub block_size: u32,
    pub total_blocks: u64,
    pub blocks_per_group: u32,
    pub inodes_per_group: u32,
    pub num_groups: u32,
    pub gdt_blocks: u32,
    pub reserved_gdt_blocks: u32,
    pub inode_blocks_per_group: u32,
}

impl FilesystemLayout {
    /// Calculate layout from parameters
    pub fn from_params(params: &FilesystemParams) -> Ext4Result<Self> {
        use super::constants::*;
        
        if params.size_bytes < 1024 * 1024 {
            return Err(Ext4Error::DeviceTooSmall {
                required: 1024 * 1024,
                actual: params.size_bytes,
            });
        }
        
        let total_blocks = params.size_bytes / params.block_size as u64;
        let blocks_per_group = EXT4_BLOCKS_PER_GROUP;
        let inodes_per_group = EXT4_INODES_PER_GROUP;
        
        let num_groups = ((total_blocks + blocks_per_group as u64 - 1) 
                         / blocks_per_group as u64) as u32;
        
        // Calculate GDT blocks (group descriptor table)
        let desc_size = if params.enable_64bit { 64 } else { 32 };
        let gdt_blocks = ((num_groups * desc_size + params.block_size - 1) 
                         / params.block_size) as u32;
        
        // Reserved GDT blocks for future growth
        // For now, don't reserve any since we don't have resize_inode
        let reserved_gdt_blocks = 0;
        
        // Inode table blocks per group
        let inode_blocks_per_group = (inodes_per_group * params.inode_size as u32 
                                      + params.block_size - 1) / params.block_size;
        
        Ok(Self {
            block_size: params.block_size,
            total_blocks,
            blocks_per_group,
            inodes_per_group,
            num_groups,
            gdt_blocks,
            reserved_gdt_blocks,
            inode_blocks_per_group,
        })
    }
    
    /// Get blocks used by metadata in a group
    pub fn metadata_blocks_per_group(&self, group: GroupNumber) -> u32 {
        let mut blocks = 0;
        
        // Superblock and GDT (only in certain groups)
        if self.has_superblock(group) {
            blocks += 1; // Superblock
            blocks += self.gdt_blocks + self.reserved_gdt_blocks;
        }
        
        // Bitmaps
        blocks += 2; // Block bitmap + inode bitmap
        
        // Inode table
        blocks += self.inode_blocks_per_group;
        
        blocks
    }
    
    /// Check if a group has a superblock backup
    pub fn has_superblock(&self, group: GroupNumber) -> bool {
        if group == 0 {
            return true;
        }
        
        // Sparse super: backups at 0, 1, and powers of 3, 5, 7
        if group == 1 {
            return true;
        }
        
        // Check if group is a power of 3, 5, or 7
        for &base in &[3u32, 5, 7] {
            let mut power = base;
            while power < self.num_groups {
                if power == group {
                    return true;
                }
                power *= base;
            }
        }
        
        false
    }
    
    /// Get number of GDT blocks
    pub fn gdt_blocks(&self) -> u32 {
        self.gdt_blocks
    }
    
    /// Get number of inode table blocks
    pub fn inode_table_blocks(&self) -> u32 {
        self.inode_blocks_per_group
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_filesystem_layout() {
        let params = FilesystemParams {
            size_bytes: 100 * 1024 * 1024, // 100MB
            ..Default::default()
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        assert_eq!(layout.block_size, 4096);
        assert_eq!(layout.total_blocks, 25600);
        assert_eq!(layout.num_groups, 1);
    }
    
    #[test]
    fn test_has_superblock() {
        let params = FilesystemParams {
            size_bytes: 10 * 1024 * 1024 * 1024, // 10GB
            ..Default::default()
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        assert!(layout.has_superblock(0));
        assert!(layout.has_superblock(1));
        assert!(!layout.has_superblock(2));
        assert!(layout.has_superblock(3));
        assert!(!layout.has_superblock(4));
        assert!(layout.has_superblock(5));
        assert!(!layout.has_superblock(6));
        assert!(layout.has_superblock(7));
        assert!(!layout.has_superblock(8));
        assert!(layout.has_superblock(9)); // 3^2
    }
}