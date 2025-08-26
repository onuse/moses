// Filesystem writer trait - provides write operations for filesystems
// This complements the FilesystemReader trait for full read-write support

use moses_core::MosesError;
use crate::device_reader::FileMetadata;

/// Core filesystem writer operations
pub trait FilesystemWriter: FilesystemReader {
    /// Write data to a file at the specified offset
    fn write_file(&mut self, path: &str, offset: u64, data: &[u8]) -> Result<usize, MosesError>;
    
    /// Create a new file
    fn create_file(&mut self, path: &str, initial_size: u64) -> Result<(), MosesError>;
    
    /// Delete a file
    fn delete_file(&mut self, path: &str) -> Result<(), MosesError>;
    
    /// Create a directory
    fn create_directory(&mut self, path: &str) -> Result<(), MosesError>;
    
    /// Delete a directory (must be empty)
    fn delete_directory(&mut self, path: &str) -> Result<(), MosesError>;
    
    /// Rename/move a file or directory
    fn rename(&mut self, old_path: &str, new_path: &str) -> Result<(), MosesError>;
    
    /// Truncate or extend a file to the specified size
    fn truncate_file(&mut self, path: &str, new_size: u64) -> Result<(), MosesError>;
    
    /// Update file metadata (timestamps, attributes)
    fn set_metadata(&mut self, path: &str, metadata: FileMetadata) -> Result<(), MosesError>;
    
    /// Flush all pending writes to disk
    fn flush(&mut self) -> Result<(), MosesError>;
    
    /// Check if the filesystem supports write operations
    fn supports_write(&self) -> bool {
        true
    }
}

use crate::device_reader::FilesystemReader;

/// Write configuration options
#[derive(Debug, Clone)]
pub struct WriteConfig {
    /// Enable actual writes (false = dry run)
    pub enable_writes: bool,
    /// Verify writes by reading back
    pub verify_writes: bool,
    /// Create backups before modifying
    pub create_backups: bool,
    /// Maximum transaction size
    pub max_transaction_size: usize,
}

impl Default for WriteConfig {
    fn default() -> Self {
        Self {
            enable_writes: false,  // Safe default: dry run
            verify_writes: true,
            create_backups: true,
            max_transaction_size: 100,
        }
    }
}