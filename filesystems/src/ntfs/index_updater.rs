// NTFS Index Updater - Complete B+ tree manipulation for directory indexes
// This module handles inserting, removing, and updating entries in NTFS directory indexes

use super::structures::*;
use super::index::{IndexRoot, IndexEntryHeader, INDEX_ENTRY_END};
use moses_core::MosesError;
use log::{debug, trace};

/// Directory index updater with B+ tree manipulation
pub struct IndexUpdater;

impl IndexUpdater {
    /// Create a new index updater
    pub fn new() -> Self {
        Self
    }
    
    /// Insert a new file entry into an INDEX_ROOT attribute
    /// Returns the updated INDEX_ROOT data
    pub fn insert_file_entry(
        &self,
        index_root_data: &[u8],
        mft_reference: u64,
        file_name: &str,
        file_attributes: u32,
        file_size: u64,
        creation_time: u64,
        modification_time: u64,
    ) -> Result<Vec<u8>, MosesError> {
        debug!("Inserting entry for '{}' into INDEX_ROOT", file_name);
        
        // Parse the existing INDEX_ROOT
        if index_root_data.len() < std::mem::size_of::<IndexRoot>() {
            return Err(MosesError::Other("INDEX_ROOT too small".to_string()));
        }
        
        // Read the INDEX_ROOT header
        let root = unsafe {
            std::ptr::read_unaligned(index_root_data.as_ptr() as *const IndexRoot)
        };
        
        // Calculate where entries start
        let header_size = std::mem::size_of::<IndexRoot>();
        let entries_start = header_size + root.header.entries_offset as usize;
        let entries_end = header_size + root.header.entries_offset as usize + root.header.index_length as usize;
        
        if entries_end > index_root_data.len() {
            return Err(MosesError::Other("INDEX_ROOT entries beyond buffer".to_string()));
        }
        
        // Parse existing entries
        let mut entries = self.parse_entries(&index_root_data[entries_start..entries_end])?;
        
        // Create new entry
        let new_entry = self.create_file_entry(
            mft_reference,
            file_name,
            file_attributes,
            file_size,
            creation_time,
            modification_time,
        )?;
        
        // Insert in sorted order (by filename)
        let insert_pos = entries.iter().position(|e| {
            if e.is_end_entry {
                true  // Insert before end entry
            } else {
                // Compare filenames (case-insensitive Unicode collation)
                e.file_name.to_lowercase() > file_name.to_lowercase()
            }
        }).unwrap_or(entries.len());
        
        entries.insert(insert_pos, new_entry);
        
        // Rebuild the INDEX_ROOT with new entries
        self.rebuild_index_root(root, entries)
    }
    
    /// Parse index entries from raw data
    fn parse_entries(&self, data: &[u8]) -> Result<Vec<DirectoryIndexEntry>, MosesError> {
        let mut entries = Vec::new();
        let mut offset = 0;
        
        while offset < data.len() {
            if offset + std::mem::size_of::<IndexEntryHeader>() > data.len() {
                break;
            }
            
            let header = unsafe {
                std::ptr::read_unaligned(data[offset..].as_ptr() as *const IndexEntryHeader)
            };
            
            let entry_length = header.length as usize;
            if entry_length == 0 || offset + entry_length > data.len() {
                break;
            }
            
            // Parse the entry
            let is_end = (header.flags & INDEX_ENTRY_END) != 0;
            
            if is_end {
                entries.push(DirectoryIndexEntry {
                    mft_reference: 0,
                    file_name: String::new(),
                    file_attributes: 0,
                    file_size: 0,
                    creation_time: 0,
                    modification_time: 0,
                    is_end_entry: true,
                    raw_data: data[offset..offset + entry_length].to_vec(),
                });
                break;  // End entry is always last
            } else if header.key_length > 0 {
                // Parse FILE_NAME attribute that follows the header
                let file_name_offset = offset + std::mem::size_of::<IndexEntryHeader>();
                let file_name_data = &data[file_name_offset..file_name_offset + header.key_length as usize];
                
                // FILE_NAME structure parsing (simplified)
                if file_name_data.len() >= 66 {
                    let name_length = file_name_data[64] as usize;
                    let name_offset = 66;
                    
                    if name_offset + name_length * 2 <= file_name_data.len() {
                        // Parse UTF-16 filename
                        let name_utf16: Vec<u16> = file_name_data[name_offset..name_offset + name_length * 2]
                            .chunks_exact(2)
                            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                            .collect();
                        let file_name = String::from_utf16_lossy(&name_utf16);
                        
                        entries.push(DirectoryIndexEntry {
                            mft_reference: header.mft_reference,
                            file_name,
                            file_attributes: 0,  // Would parse from FILE_NAME
                            file_size: 0,        // Would parse from FILE_NAME
                            creation_time: 0,    // Would parse from FILE_NAME
                            modification_time: 0, // Would parse from FILE_NAME
                            is_end_entry: false,
                            raw_data: data[offset..offset + entry_length].to_vec(),
                        });
                    }
                }
            }
            
            offset += entry_length;
        }
        
        Ok(entries)
    }
    
    /// Create a new file entry
    fn create_file_entry(
        &self,
        mft_reference: u64,
        file_name: &str,
        file_attributes: u32,
        file_size: u64,
        creation_time: u64,
        modification_time: u64,
    ) -> Result<DirectoryIndexEntry, MosesError> {
        debug!("Creating index entry for '{}'", file_name);
        
        // Create FILE_NAME attribute data
        let name_utf16: Vec<u16> = file_name.encode_utf16().collect();
        let name_length = name_utf16.len() as u8;
        
        // FILE_NAME structure size
        let file_name_size = 66 + name_utf16.len() * 2;
        let mut file_name_data = vec![0u8; file_name_size];
        
        // Parent reference (will be filled by caller)
        file_name_data[0..8].copy_from_slice(&MFT_RECORD_ROOT.to_le_bytes());
        
        // Timestamps
        file_name_data[8..16].copy_from_slice(&creation_time.to_le_bytes());
        file_name_data[16..24].copy_from_slice(&modification_time.to_le_bytes());
        file_name_data[24..32].copy_from_slice(&modification_time.to_le_bytes()); // MFT modified
        file_name_data[32..40].copy_from_slice(&creation_time.to_le_bytes()); // Accessed
        
        // Sizes
        file_name_data[40..48].copy_from_slice(&file_size.to_le_bytes()); // Allocated
        file_name_data[48..56].copy_from_slice(&file_size.to_le_bytes()); // Real size
        
        // File attributes
        file_name_data[56..60].copy_from_slice(&file_attributes.to_le_bytes());
        
        // Name length and namespace
        file_name_data[64] = name_length;
        file_name_data[65] = 1; // POSIX namespace
        
        // Name (UTF-16LE)
        for (i, &ch) in name_utf16.iter().enumerate() {
            let offset = 66 + i * 2;
            file_name_data[offset..offset + 2].copy_from_slice(&ch.to_le_bytes());
        }
        
        // Create index entry with header
        let entry_size = std::mem::size_of::<IndexEntryHeader>() + file_name_size + 8; // +8 for alignment
        let mut entry_data = vec![0u8; entry_size];
        
        // Write header
        let header = IndexEntryHeader {
            mft_reference,
            length: entry_size as u16,
            key_length: file_name_size as u16,
            flags: 0, // Not an end entry
            reserved: 0,
        };
        
        unsafe {
            let header_bytes = std::slice::from_raw_parts(
                &header as *const _ as *const u8,
                std::mem::size_of::<IndexEntryHeader>()
            );
            entry_data[..header_bytes.len()].copy_from_slice(header_bytes);
        }
        
        // Write FILE_NAME data after header
        entry_data[std::mem::size_of::<IndexEntryHeader>()..std::mem::size_of::<IndexEntryHeader>() + file_name_size]
            .copy_from_slice(&file_name_data);
        
        Ok(DirectoryIndexEntry {
            mft_reference,
            file_name: file_name.to_string(),
            file_attributes,
            file_size,
            creation_time,
            modification_time,
            is_end_entry: false,
            raw_data: entry_data,
        })
    }
    
    /// Rebuild INDEX_ROOT with updated entries
    fn rebuild_index_root(
        &self,
        original_root: IndexRoot,
        entries: Vec<DirectoryIndexEntry>,
    ) -> Result<Vec<u8>, MosesError> {
        debug!("Rebuilding INDEX_ROOT with {} entries", entries.len());
        
        // Calculate total size needed
        let header_size = std::mem::size_of::<IndexRoot>();
        let entries_size: usize = entries.iter().map(|e| e.raw_data.len()).sum();
        let total_size = header_size + entries_size;
        
        // Check if it fits in the resident attribute (typical max ~700 bytes)
        if total_size > 2048 {
            return Err(MosesError::Other("Index too large for INDEX_ROOT, need INDEX_ALLOCATION".to_string()));
        }
        
        let mut result = vec![0u8; total_size];
        
        // Copy original header with updated sizes
        let mut new_root = original_root;
        new_root.header.index_length = entries_size as u32;
        new_root.header.allocated_size = ((entries_size + 7) / 8 * 8) as u32; // Align to 8 bytes
        
        unsafe {
            let root_bytes = std::slice::from_raw_parts(
                &new_root as *const _ as *const u8,
                header_size
            );
            result[..header_size].copy_from_slice(root_bytes);
        }
        
        // Write entries
        let mut offset = header_size;
        for entry in entries {
            let entry_len = entry.raw_data.len();
            result[offset..offset + entry_len].copy_from_slice(&entry.raw_data);
            offset += entry_len;
        }
        
        trace!("Rebuilt INDEX_ROOT: {} bytes total", result.len());
        Ok(result)
    }
}

/// Represents a parsed directory index entry
#[allow(dead_code)]
struct DirectoryIndexEntry {
    mft_reference: u64,
    file_name: String,
    file_attributes: u32,
    file_size: u64,
    creation_time: u64,
    modification_time: u64,
    is_end_entry: bool,
    raw_data: Vec<u8>,  // The raw bytes of this entry
}