// Extended NTFS writer operations - Directory index management
// Provides methods for updating directory indexes when files are created/deleted

use super::writer::NtfsWriter;
use super::structures::*;
use super::index_updater::IndexUpdater;
use super::mft_updater::MftUpdater;
use super::attributes::AttributeData;
use moses_core::MosesError;
use log::{debug, warn, info};

/// Get current time in Windows FILETIME format
fn windows_time_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let unix_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    // Convert Unix time to Windows FILETIME (100ns intervals since 1601)
    // Unix epoch (1970) is 11644473600 seconds after Windows epoch (1601)
    (unix_time + 11644473600) * 10_000_000
}

impl NtfsWriter {
    /// Update a directory's index to include a new file
    pub fn update_directory_index(
        &mut self,
        parent_mft_num: u64,
        child_mft_num: u64,
        file_name: &str,
    ) -> Result<(), MosesError> {
        info!("Updating directory {} index to add file '{}' (MFT: {})", 
              parent_mft_num, file_name, child_mft_num);
        
        // Read the parent directory MFT record
        let mut parent_record = self.read_mft_record(parent_mft_num)?;
        
        if !parent_record.is_in_use() {
            return Err(MosesError::Other("Parent directory MFT record not in use".to_string()));
        }
        
        if !parent_record.is_directory() {
            return Err(MosesError::Other("Parent MFT record is not a directory".to_string()));
        }
        
        // Find the INDEX_ROOT attribute
        let index_root_attr = parent_record.find_attribute(ATTR_TYPE_INDEX_ROOT)
            .ok_or_else(|| MosesError::Other("Parent has no INDEX_ROOT attribute".to_string()))?;
        
        match index_root_attr {
            AttributeData::IndexRoot(root_data) => {
                // Use the IndexUpdater to properly insert the entry
                let updater = IndexUpdater::new();
                
                // Get current timestamp
                let current_time = windows_time_now();
                
                // Insert the new entry into the index
                let updated_root_data = updater.insert_file_entry(
                    root_data,
                    child_mft_num,
                    file_name,
                    0x20, // FILE_ATTRIBUTE_ARCHIVE
                    0,    // File size (0 for new file)
                    current_time,
                    current_time,
                )?;
                
                debug!("Updated INDEX_ROOT from {} to {} bytes", 
                       root_data.len(), updated_root_data.len());
                
                // Now write this back to the MFT record
                // Read the raw MFT record
                let mft_offset = self.mft_reader.mft_offset + (parent_mft_num * self.boot_sector.mft_record_size() as u64);
                let _mft_record_data = self.reader.read_at(mft_offset, self.boot_sector.mft_record_size() as usize)?;
                
                // Use our helper method to update the MFT record with new INDEX_ROOT
                self.update_mft_record_index_root(parent_mft_num, updated_root_data)?;
                
                info!("Successfully updated directory index for '{}' and wrote to disk", file_name);
                
                Ok(())
            }
            _ => Err(MosesError::Other("Invalid INDEX_ROOT attribute type".to_string()))
        }
    }
    
    /// Remove a file from a directory's index
    pub fn remove_from_directory_index(
        &mut self,
        parent_mft_num: u64,
        child_mft_num: u64,
    ) -> Result<(), MosesError> {
        debug!("Removing MFT {} from directory {} index", child_mft_num, parent_mft_num);
        
        if parent_mft_num == MFT_RECORD_ROOT {
            warn!("Root directory index removal not fully implemented");
            return Ok(());
        }
        
        warn!("Non-root directory index removal not implemented");
        Ok(())
    }
    
    /// Helper for creating an updated MFT record with new INDEX_ROOT
    pub fn update_mft_record_index_root(
        &mut self,
        mft_num: u64,
        new_index_root: Vec<u8>,
    ) -> Result<(), MosesError> {
        debug!("Updating INDEX_ROOT in MFT record {}", mft_num);
        
        // Read the current MFT record raw data
        let mft_offset = self.mft_reader.mft_offset + (mft_num * self.boot_sector.mft_record_size() as u64);
        let record_data = self.reader.read_at(mft_offset, self.boot_sector.mft_record_size() as usize)?;
        
        // Use MftUpdater to replace the INDEX_ROOT attribute
        let updater = MftUpdater::new();
        let updated_mft = updater.replace_attribute(
            &record_data,
            ATTR_TYPE_INDEX_ROOT,
            &new_index_root,
        )?;
        
        // Write the updated MFT record back to disk
        self.write_raw_mft_record(mft_num, &updated_mft)?;
        
        Ok(())
    }
    
    /// Create a simple directory index entry for a file
    pub fn create_index_entry(
        &self,
        mft_reference: u64,
        file_name: &str,
        is_directory: bool,
    ) -> Vec<u8> {
        // This would create a properly formatted index entry
        // For now, return a placeholder
        debug!("Creating index entry for '{}' (dir: {})", file_name, is_directory);
        
        // Index entry structure:
        // - MFT reference (8 bytes)
        // - Entry length (2 bytes)
        // - File name length (2 bytes)
        // - Flags (4 bytes)
        // - File name (variable, UTF-16)
        
        let name_utf16: Vec<u16> = file_name.encode_utf16().collect();
        let entry_size = 16 + (name_utf16.len() * 2);
        
        let mut entry = Vec::with_capacity(entry_size);
        
        // MFT reference
        entry.extend_from_slice(&mft_reference.to_le_bytes());
        
        // Entry length
        entry.extend_from_slice(&(entry_size as u16).to_le_bytes());
        
        // File name length
        entry.extend_from_slice(&(name_utf16.len() as u16).to_le_bytes());
        
        // Flags (2 = has subnode for directories)
        let flags = if is_directory { 0x10000003u32 } else { 0x00000001u32 };
        entry.extend_from_slice(&flags.to_le_bytes());
        
        // File name
        for ch in name_utf16 {
            entry.extend_from_slice(&ch.to_le_bytes());
        }
        
        entry
    }
}