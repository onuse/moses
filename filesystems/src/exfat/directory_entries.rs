// exFAT directory entry set creation
// Each file/directory requires a set of entries: File + Stream + FileName(s)

use super::structures::*;
use crate::fat_common::timestamps::ExFatTimestamp;
use crate::fat_common::long_names::exfat_name_hash;

/// Builder for creating exFAT directory entry sets
pub struct DirectoryEntrySetBuilder {
    name: String,
    is_directory: bool,
    size: u64,
    first_cluster: u32,
    attributes: u16,
    created: ExFatTimestamp,
    modified: ExFatTimestamp,
    accessed: ExFatTimestamp,
}

impl DirectoryEntrySetBuilder {
    /// Create a new file entry set builder
    pub fn new_file(name: &str) -> Self {
        let now = ExFatTimestamp::now();
        Self {
            name: name.to_string(),
            is_directory: false,
            size: 0,
            first_cluster: 0,
            attributes: 0,
            created: now,
            modified: now,
            accessed: now,
        }
    }
    
    /// Create a new directory entry set builder
    pub fn new_directory(name: &str) -> Self {
        let now = ExFatTimestamp::now();
        Self {
            name: name.to_string(),
            is_directory: true,
            size: 0,
            first_cluster: 0,
            attributes: EXFAT_ATTR_DIRECTORY,
            created: now,
            modified: now,
            accessed: now,
        }
    }
    
    pub fn size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }
    
    pub fn first_cluster(mut self, cluster: u32) -> Self {
        self.first_cluster = cluster;
        self
    }
    
    pub fn attributes(mut self, attrs: u16) -> Self {
        self.attributes = attrs;
        self
    }
    
    pub fn created(mut self, timestamp: ExFatTimestamp) -> Self {
        self.created = timestamp;
        self
    }
    
    pub fn modified(mut self, timestamp: ExFatTimestamp) -> Self {
        self.modified = timestamp;
        self
    }
    
    /// Build the complete directory entry set
    pub fn build(self) -> Vec<ExFatDirectoryEntry> {
        let mut entries = Vec::new();
        
        // Calculate number of filename entries needed (15 chars per entry)
        let name_utf16: Vec<u16> = self.name.encode_utf16().collect();
        let filename_entries_needed = (name_utf16.len() + 14) / 15;
        let secondary_count = 1 + filename_entries_needed; // Stream + FileName(s)
        
        // 1. File Directory Entry (Primary)
        let mut file_entry = ExFatDirectoryEntry::default();
        file_entry.file.entry_type = EXFAT_ENTRY_FILE;
        file_entry.file.secondary_count = secondary_count as u8;
        file_entry.file.set_checksum = 0; // Will be calculated later
        file_entry.file.file_attributes = self.attributes;
        file_entry.file.reserved1 = 0;
        
        // Timestamps (using simplified conversion for now)
        let (create_date, create_time) = self.created.to_fat_datetime();
        let (modify_date, modify_time) = self.modified.to_fat_datetime();
        let (access_date, _) = self.accessed.to_fat_datetime();
        
        file_entry.file.create_timestamp = ((create_date as u32) << 16) | create_time as u32;
        file_entry.file.last_modified_timestamp = ((modify_date as u32) << 16) | modify_time as u32;
        file_entry.file.last_accessed_timestamp = (access_date as u32) << 16;
        
        file_entry.file.create_10ms_increment = (self.created.centiseconds / 10) as u8;
        file_entry.file.last_modified_10ms_increment = (self.modified.centiseconds / 10) as u8;
        file_entry.file.create_tz_offset = self.created.timezone_offset as u8;
        file_entry.file.last_modified_tz_offset = self.modified.timezone_offset as u8;
        file_entry.file.last_accessed_tz_offset = self.accessed.timezone_offset as u8;
        entries.push(file_entry);
        
        // 2. Stream Extension Entry
        let mut stream_entry = ExFatDirectoryEntry::default();
        stream_entry.stream.entry_type = EXFAT_ENTRY_STREAM;
        stream_entry.stream.flags = 0x01; // Allocation possible
        stream_entry.stream.reserved1 = 0;
        stream_entry.stream.name_length = name_utf16.len() as u8;
        stream_entry.stream.name_hash = exfat_name_hash(&self.name);
        stream_entry.stream.reserved2 = 0;
        stream_entry.stream.valid_data_length = if self.is_directory { 0 } else { self.size };
        stream_entry.stream.reserved3 = 0;
        stream_entry.stream.first_cluster = self.first_cluster;
        stream_entry.stream.data_length = if self.is_directory { 
            0  // Directories report 0 size
        } else { 
            self.size 
        };
        entries.push(stream_entry);
        
        // 3. File Name Entries
        let mut chars_written = 0;
        for i in 0..filename_entries_needed {
            let mut name_entry = ExFatDirectoryEntry::default();
            name_entry.filename.entry_type = EXFAT_ENTRY_FILE_NAME;
            name_entry.filename.flags = 0;
            
            // Copy up to 15 characters
            let chars_to_copy = std::cmp::min(15, name_utf16.len() - chars_written);
            unsafe {
                for j in 0..chars_to_copy {
                    name_entry.filename.file_name[j] = name_utf16[chars_written + j];
                }
                
                // Pad with 0xFFFF if this is the last entry and not full
                if i == filename_entries_needed - 1 && chars_to_copy < 15 {
                    for j in chars_to_copy..15 {
                        name_entry.filename.file_name[j] = 0xFFFF;
                    }
                }
            }
            
            chars_written += chars_to_copy;
            entries.push(name_entry);
        }
        
        // Calculate and set the checksum for the entry set
        let checksum = Self::calculate_checksum(&entries);
        entries[0].file.set_checksum = checksum;
        
        entries
    }
    
    /// Calculate checksum for the directory entry set
    fn calculate_checksum(entries: &[ExFatDirectoryEntry]) -> u16 {
        let mut checksum: u16 = 0;
        
        for (i, entry) in entries.iter().enumerate() {
            let bytes = entry.to_bytes();
            for (j, &byte) in bytes.iter().enumerate() {
                // Skip the checksum field itself (bytes 2-3 of first entry)
                if i == 0 && (j == 2 || j == 3) {
                    continue;
                }
                checksum = ((checksum << 15) | (checksum >> 1)).wrapping_add(byte as u16);
            }
        }
        
        checksum
    }
}

/// Create a simple test file entry for an empty filesystem
pub fn create_test_file_entry() -> Vec<ExFatDirectoryEntry> {
    DirectoryEntrySetBuilder::new_file("test.txt")
        .size(1024)
        .first_cluster(10)
        .build()
}

/// Create the "." and ".." entries for a directory
pub fn create_dot_entries(current_cluster: u32, parent_cluster: u32) -> Vec<ExFatDirectoryEntry> {
    let mut entries = Vec::new();
    
    // "." entry pointing to current directory
    let dot_entries = DirectoryEntrySetBuilder::new_directory(".")
        .first_cluster(current_cluster)
        .attributes(EXFAT_ATTR_DIRECTORY | EXFAT_ATTR_SYSTEM)
        .build();
    entries.extend(dot_entries);
    
    // ".." entry pointing to parent directory
    let dotdot_entries = DirectoryEntrySetBuilder::new_directory("..")
        .first_cluster(parent_cluster)
        .attributes(EXFAT_ATTR_DIRECTORY | EXFAT_ATTR_SYSTEM)
        .build();
    entries.extend(dotdot_entries);
    
    entries
}

/// Validate a directory entry set
pub fn validate_entry_set(entries: &[ExFatDirectoryEntry]) -> Result<(), String> {
    if entries.is_empty() {
        return Err("Empty entry set".to_string());
    }
    
    // First entry must be File entry
    let first_type = entries[0].entry_type();
    if first_type != EXFAT_ENTRY_FILE {
        return Err(format!("First entry must be File (0x85), got 0x{:02X}", first_type));
    }
    
    // Get expected secondary count
    let secondary_count = unsafe { entries[0].file.secondary_count };
    if entries.len() != (secondary_count + 1) as usize {
        return Err(format!("Entry count mismatch: expected {}, got {}", 
            secondary_count + 1, entries.len()));
    }
    
    // Second entry must be Stream
    if entries.len() > 1 {
        let second_type = entries[1].entry_type();
        if second_type != EXFAT_ENTRY_STREAM {
            return Err(format!("Second entry must be Stream (0xC0), got 0x{:02X}", second_type));
        }
    }
    
    // Remaining entries must be FileName
    for i in 2..entries.len() {
        let entry_type = entries[i].entry_type();
        if entry_type != EXFAT_ENTRY_FILE_NAME {
            return Err(format!("Entry {} must be FileName (0xC1), got 0x{:02X}", i, entry_type));
        }
    }
    
    // Verify checksum
    let stored_checksum = unsafe { entries[0].file.set_checksum };
    let calculated_checksum = DirectoryEntrySetBuilder::calculate_checksum(entries);
    if stored_checksum != calculated_checksum {
        return Err(format!("Checksum mismatch: stored 0x{:04X}, calculated 0x{:04X}",
            stored_checksum, calculated_checksum));
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_short_filename_entry_set() {
        let entries = DirectoryEntrySetBuilder::new_file("test.txt")
            .size(1024)
            .first_cluster(10)
            .build();
        
        // Should have 3 entries: File + Stream + 1 FileName
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].entry_type(), EXFAT_ENTRY_FILE);
        assert_eq!(entries[1].entry_type(), EXFAT_ENTRY_STREAM);
        assert_eq!(entries[2].entry_type(), EXFAT_ENTRY_FILE_NAME);
        
        // Validate the set
        assert!(validate_entry_set(&entries).is_ok());
    }
    
    #[test]
    fn test_long_filename_entry_set() {
        let long_name = "this_is_a_very_long_filename_that_needs_multiple_entries.txt";
        let entries = DirectoryEntrySetBuilder::new_file(long_name)
            .size(2048)
            .first_cluster(20)
            .build();
        
        // Calculate expected entries
        let name_len = long_name.encode_utf16().count();
        let filename_entries = (name_len + 14) / 15;
        let total_entries = 2 + filename_entries; // File + Stream + FileNames
        
        assert_eq!(entries.len(), total_entries);
        assert_eq!(entries[0].entry_type(), EXFAT_ENTRY_FILE);
        assert_eq!(entries[1].entry_type(), EXFAT_ENTRY_STREAM);
        
        // All remaining should be FileName entries
        for i in 2..entries.len() {
            assert_eq!(entries[i].entry_type(), EXFAT_ENTRY_FILE_NAME);
        }
        
        // Validate the set
        assert!(validate_entry_set(&entries).is_ok());
    }
    
    #[test]
    fn test_directory_entry_set() {
        let entries = DirectoryEntrySetBuilder::new_directory("folder")
            .first_cluster(30)
            .build();
        
        assert_eq!(entries.len(), 3);
        
        // Check directory attribute is set
        let attrs = unsafe { entries[0].file.file_attributes };
        assert!(attrs & EXFAT_ATTR_DIRECTORY != 0);
        
        // Directory should have 0 size in stream entry
        let size = unsafe { entries[1].stream.data_length };
        assert_eq!(size, 0);
        
        // Validate the set
        assert!(validate_entry_set(&entries).is_ok());
    }
}