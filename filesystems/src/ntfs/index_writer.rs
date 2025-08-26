// NTFS Index Writer - B+ tree updates for directories
// Implements directory index modifications for file creation/deletion

use super::index::{IndexRoot, IndexHeader, IndexEntryHeader, IndexEntry, INDEX_ENTRY_END};
use super::structures::*;
use super::attributes::AttributeData;
use moses_core::MosesError;
use log::{debug, warn};

/// Directory index updater for NTFS
pub struct IndexWriter {
    // Nothing to store for now, methods work on provided data
}

impl IndexWriter {
    /// Create a new index writer
    pub fn new() -> Self {
        Self {}
    }
    
    /// Add a file entry to a directory's index
    /// This updates the INDEX_ROOT attribute and possibly INDEX_ALLOCATION
    pub fn add_file_to_directory(
        &self,
        parent_mft_record: &mut super::mft::MftRecord,
        mft_reference: u64,
        file_name: &str,
        is_directory: bool,
    ) -> Result<(), MosesError> {
        debug!("Adding file '{}' to directory index (MFT: {})", file_name, mft_reference);
        
        // Find the INDEX_ROOT attribute
        let index_root_attr = parent_mft_record.find_attribute(ATTR_TYPE_INDEX_ROOT)
            .ok_or_else(|| MosesError::Other("Parent directory has no INDEX_ROOT".to_string()))?;
        
        match index_root_attr {
            AttributeData::IndexRoot(root_data) => {
                // Parse the current index
                let entries = super::index::parse_index_root(root_data)?;
                
                // Check if file already exists
                if entries.iter().any(|e| e.file_name == file_name) {
                    return Err(MosesError::Other(format!("File '{}' already exists", file_name)));
                }
                
                // Create new entry
                let new_entry = IndexEntry {
                    mft_reference,
                    file_name: file_name.to_string(),
                    is_directory,
                    has_subnode: false,
                };
                
                // Try to add to INDEX_ROOT (small directories)
                if self.can_fit_in_index_root(root_data, &new_entry) {
                    let _updated_root = self.insert_into_index_root(root_data, new_entry)?;
                    // TODO: Update the MFT record with new INDEX_ROOT
                    warn!("INDEX_ROOT update not fully implemented - entry added but not persisted");
                    Ok(())
                } else {
                    // Need to use INDEX_ALLOCATION for large directories
                    warn!("INDEX_ALLOCATION update not implemented - large directories not supported");
                    Err(MosesError::NotSupported("Large directory index update not implemented".to_string()))
                }
            }
            _ => Err(MosesError::Other("Invalid INDEX_ROOT attribute type".to_string()))
        }
    }
    
    /// Remove a file entry from a directory's index
    pub fn remove_file_from_directory(
        &self,
        parent_mft_record: &mut super::mft::MftRecord,
        file_name: &str,
    ) -> Result<(), MosesError> {
        debug!("Removing file '{}' from directory index", file_name);
        
        // Find the INDEX_ROOT attribute
        let index_root_attr = parent_mft_record.find_attribute(ATTR_TYPE_INDEX_ROOT)
            .ok_or_else(|| MosesError::Other("Parent directory has no INDEX_ROOT".to_string()))?;
        
        match index_root_attr {
            AttributeData::IndexRoot(root_data) => {
                let entries = super::index::parse_index_root(root_data)?;
                
                // Find and remove the entry
                let filtered: Vec<IndexEntry> = entries.into_iter()
                    .filter(|e| e.file_name != file_name)
                    .collect();
                
                // Rebuild the index
                let _updated_root = self.rebuild_index_root(root_data, filtered)?;
                // TODO: Update the MFT record with new INDEX_ROOT
                warn!("INDEX_ROOT removal not fully implemented - entry removed but not persisted");
                Ok(())
            }
            _ => Err(MosesError::Other("Invalid INDEX_ROOT attribute type".to_string()))
        }
    }
    
    /// Check if a new entry can fit in the INDEX_ROOT
    fn can_fit_in_index_root(&self, root_data: &[u8], entry: &IndexEntry) -> bool {
        // Estimate entry size: header + filename (UTF-16)
        let entry_size = std::mem::size_of::<IndexEntryHeader>() + 
                        std::mem::size_of::<FileNameAttr>() + 
                        (entry.file_name.len() * 2); // UTF-16
        
        // Check against typical INDEX_ROOT size limit (about 4KB)
        root_data.len() + entry_size < 4096
    }
    
    /// Insert an entry into INDEX_ROOT (for small directories)
    fn insert_into_index_root(&self, root_data: &[u8], _new_entry: IndexEntry) -> Result<Vec<u8>, MosesError> {
        // Parse existing structure
        let _root = unsafe {
            std::ptr::read_unaligned(root_data.as_ptr() as *const IndexRoot)
        };
        
        // This is a simplified implementation
        // Real implementation would:
        // 1. Parse all existing entries
        // 2. Find correct position (entries are sorted)
        // 3. Insert new entry
        // 4. Update size fields
        // 5. Serialize back to bytes
        
        warn!("Simplified INDEX_ROOT insertion - full implementation needed");
        
        // For now, return original data
        Ok(root_data.to_vec())
    }
    
    /// Rebuild INDEX_ROOT with a new set of entries
    fn rebuild_index_root(&self, root_data: &[u8], _entries: Vec<IndexEntry>) -> Result<Vec<u8>, MosesError> {
        // Parse header
        let _root = unsafe {
            std::ptr::read_unaligned(root_data.as_ptr() as *const IndexRoot)
        };
        
        // This would rebuild the entire index structure
        warn!("Simplified INDEX_ROOT rebuild - full implementation needed");
        
        // For now, return original data
        Ok(root_data.to_vec())
    }
    
    /// Create a new INDEX_ROOT for an empty directory
    pub fn create_empty_index_root(&self) -> Vec<u8> {
        let mut buffer = vec![0u8; 64]; // Minimum INDEX_ROOT size
        
        // Create IndexRoot structure
        let root = IndexRoot {
            attribute_type: ATTR_TYPE_FILE_NAME,
            collation_rule: 1, // COLLATION_FILE_NAME
            index_block_size: 4096,
            clusters_per_block: 1,
            reserved: [0; 3],
            header: IndexHeader {
                entries_offset: 16, // After the IndexHeader
                index_length: 24,    // Just the end entry
                allocated_size: 32,
                flags: 0,
            },
        };
        
        // Write root structure
        unsafe {
            let root_bytes = std::slice::from_raw_parts(
                &root as *const _ as *const u8,
                std::mem::size_of::<IndexRoot>()
            );
            buffer[..root_bytes.len()].copy_from_slice(root_bytes);
        }
        
        // Add end entry
        let entry_offset = std::mem::size_of::<IndexRoot>() + root.header.entries_offset as usize;
        if entry_offset + std::mem::size_of::<IndexEntryHeader>() <= buffer.len() {
            let end_entry = IndexEntryHeader {
                mft_reference: 0,
                length: 24,
                key_length: 0,
                flags: INDEX_ENTRY_END,
                reserved: 0,
            };
            
            unsafe {
                let entry_bytes = std::slice::from_raw_parts(
                    &end_entry as *const _ as *const u8,
                    std::mem::size_of::<IndexEntryHeader>()
                );
                buffer[entry_offset..entry_offset + entry_bytes.len()].copy_from_slice(entry_bytes);
            }
        }
        
        buffer
    }
}

/// Helper to encode a file name for index entry
#[allow(dead_code)]
fn encode_file_name_for_index(name: &str, mft_parent: u64, is_directory: bool) -> Vec<u8> {
    // This would create a FILE_NAME attribute structure
    // For now, simplified implementation
    let mut buffer = Vec::new();
    
    // Parent reference
    buffer.extend_from_slice(&mft_parent.to_le_bytes());
    
    // Timestamps (placeholder)
    for _ in 0..4 {
        buffer.extend_from_slice(&0u64.to_le_bytes());
    }
    
    // Sizes
    buffer.extend_from_slice(&0u64.to_le_bytes()); // allocated
    buffer.extend_from_slice(&0u64.to_le_bytes()); // real
    
    // Flags
    let flags = if is_directory { 0x10000000u32 } else { 0u32 };
    buffer.extend_from_slice(&flags.to_le_bytes());
    
    // Name length and namespace
    buffer.push(name.len() as u8);
    buffer.push(1); // Win32 namespace
    
    // Name in UTF-16LE
    for ch in name.encode_utf16() {
        buffer.extend_from_slice(&ch.to_le_bytes());
    }
    
    buffer
}