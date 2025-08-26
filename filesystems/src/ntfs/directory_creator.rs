// NTFS Directory Creation
// Implements mkdir functionality for NTFS

use super::structures::*;
use super::index_writer::IndexWriter;
use super::path_resolver::PathResolver;
use super::writer::NtfsWriter;
use super::reader::NtfsReader;
use moses_core::MosesError;
use log::{debug, info};
use std::time::SystemTime;

/// Creates directories in NTFS filesystem
pub struct DirectoryCreator {
    index_writer: IndexWriter,
}

impl DirectoryCreator {
    /// Create a new directory creator
    pub fn new() -> Self {
        Self {
            index_writer: IndexWriter::new(),
        }
    }
    
    /// Create a directory at the specified path
    pub fn create_directory(
        &mut self,
        reader: &mut NtfsReader,
        writer: &mut NtfsWriter,
        path_resolver: &mut PathResolver,
        path: &str,
    ) -> Result<u64, MosesError> {
        debug!("Creating directory: {}", path);
        
        // Check if directory already exists
        if path_resolver.path_exists(reader, path) {
            return Err(MosesError::Other(format!("Directory already exists: {}", path)));
        }
        
        // Get parent directory path and new directory name
        let (parent_path, dir_name) = self.split_path(path)?;
        
        // Resolve parent directory MFT record
        let parent_mft = path_resolver.resolve_path(reader, parent_path)?;
        debug!("Parent directory MFT: {} for path: {}", parent_mft, parent_path);
        
        // Allocate a new MFT record for the directory
        let dir_mft = writer.find_free_mft_record()?;
        writer.allocate_mft_record(dir_mft)?;
        
        debug!("Allocated MFT record {} for new directory '{}'", dir_mft, dir_name);
        
        // Create the directory MFT record
        let mut dir_record = self.create_directory_mft_record(
            dir_mft,
            parent_mft,
            dir_name,
        )?;
        
        // TODO: Write the MFT record to disk
        // This would require the MftWriter or direct write to device
        
        // Add directory entry to parent's index
        let mut parent_record = reader.read_mft_record(parent_mft)?;
        self.index_writer.add_file_to_directory(
            &mut parent_record,
            dir_mft,
            dir_name,
            true, // is_directory = true
        )?;
        
        // TODO: Write updated parent MFT record to disk
        // This would require the MftWriter or direct write to device
        
        info!("Created directory '{}' with MFT record {}", path, dir_mft);
        Ok(dir_mft)
    }
    
    /// Create an MFT record for a new directory
    fn create_directory_mft_record(
        &self,
        mft_num: u64,
        parent_mft: u64,
        name: &str,
    ) -> Result<super::mft::MftRecord, MosesError> {
        // Create MFT record header
        let header = MftRecordHeader {
            signature: *b"FILE",
            usa_offset: 0x30,
            usa_count: 3,
            lsn: 0,
            sequence_number: 1,
            link_count: 1,
            attrs_offset: 0x38, // After header and USA
            flags: MFT_RECORD_IN_USE | MFT_RECORD_IS_DIRECTORY,
            bytes_used: 0x200, // Will be updated
            bytes_allocated: 0x400, // 1KB record
            base_mft_record: 0,
            next_attr_id: 0,
            reserved: 0,
            mft_record_number: mft_num as u32,
        };
        
        // Create data buffer for MFT record
        let mut data = vec![0u8; header.bytes_allocated as usize];
        
        // Write header to buffer
        unsafe {
            let header_bytes = std::slice::from_raw_parts(
                &header as *const _ as *const u8,
                std::mem::size_of::<MftRecordHeader>()
            );
            data[..header_bytes.len()].copy_from_slice(header_bytes);
        }
        
        let mut offset = header.attrs_offset as usize;
        
        // Add $STANDARD_INFORMATION attribute
        offset += self.add_standard_info_attribute(&mut data, offset)?;
        
        // Add $FILE_NAME attribute
        offset += self.add_filename_attribute(&mut data, offset, parent_mft, name)?;
        
        // Add $INDEX_ROOT attribute for directory entries
        offset += self.add_index_root_attribute(&mut data, offset)?;
        
        // Add $INDEX_ALLOCATION attribute (for large directories)
        // This is optional and typically added when directory grows
        
        // Add end marker
        if offset + 8 <= data.len() {
            data[offset..offset + 4].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        }
        
        // Update bytes_used in header
        let updated_header = MftRecordHeader {
            bytes_used: (offset + 8) as u32,
            ..header
        };
        
        unsafe {
            let header_bytes = std::slice::from_raw_parts(
                &updated_header as *const _ as *const u8,
                std::mem::size_of::<MftRecordHeader>()
            );
            data[..header_bytes.len()].copy_from_slice(header_bytes);
        }
        
        // Create MftRecord from data
        super::mft::MftRecord::parse(data)
    }
    
    /// Add $STANDARD_INFORMATION attribute
    fn add_standard_info_attribute(&self, data: &mut [u8], offset: usize) -> Result<usize, MosesError> {
        let attr_size = 96; // Typical size for standard info
        
        if offset + attr_size > data.len() {
            return Err(MosesError::Other("Buffer too small for standard info".to_string()));
        }
        
        // Attribute header
        data[offset..offset + 4].copy_from_slice(&ATTR_TYPE_STANDARD_INFORMATION.to_le_bytes());
        data[offset + 4..offset + 8].copy_from_slice(&(attr_size as u32).to_le_bytes());
        data[offset + 8] = 0; // Resident
        data[offset + 9] = 0; // Name length
        
        // Standard info data (simplified)
        let data_offset = offset + 24;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Convert Unix timestamp to Windows FILETIME (100ns intervals since 1601)
        let windows_time = (now + 11644473600) * 10000000;
        
        // Creation time
        data[data_offset..data_offset + 8].copy_from_slice(&windows_time.to_le_bytes());
        // Modification time
        data[data_offset + 8..data_offset + 16].copy_from_slice(&windows_time.to_le_bytes());
        // MFT change time
        data[data_offset + 16..data_offset + 24].copy_from_slice(&windows_time.to_le_bytes());
        // Access time
        data[data_offset + 24..data_offset + 32].copy_from_slice(&windows_time.to_le_bytes());
        
        // File attributes (directory)
        data[data_offset + 32..data_offset + 36].copy_from_slice(&0x10u32.to_le_bytes());
        
        Ok(attr_size)
    }
    
    /// Add $FILE_NAME attribute
    fn add_filename_attribute(
        &self,
        data: &mut [u8],
        offset: usize,
        parent_mft: u64,
        name: &str,
    ) -> Result<usize, MosesError> {
        let name_utf16: Vec<u16> = name.encode_utf16().collect();
        let name_bytes = name_utf16.len() * 2;
        let attr_size = 90 + name_bytes; // Base size + name
        
        if offset + attr_size > data.len() {
            return Err(MosesError::Other("Buffer too small for filename".to_string()));
        }
        
        // Attribute header
        data[offset..offset + 4].copy_from_slice(&ATTR_TYPE_FILE_NAME.to_le_bytes());
        data[offset + 4..offset + 8].copy_from_slice(&(attr_size as u32).to_le_bytes());
        data[offset + 8] = 0; // Resident
        data[offset + 9] = 0; // Name length
        
        // FILE_NAME structure
        let data_offset = offset + 24;
        
        // Parent directory reference
        data[data_offset..data_offset + 8].copy_from_slice(&parent_mft.to_le_bytes());
        
        // Timestamps (use current time)
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let windows_time = (now + 11644473600) * 10000000;
        
        for i in 0..4 {
            let ts_offset = data_offset + 8 + (i * 8);
            data[ts_offset..ts_offset + 8].copy_from_slice(&windows_time.to_le_bytes());
        }
        
        // Allocated size (0 for directory)
        data[data_offset + 40..data_offset + 48].copy_from_slice(&0u64.to_le_bytes());
        // Real size (0 for directory)
        data[data_offset + 48..data_offset + 56].copy_from_slice(&0u64.to_le_bytes());
        
        // Flags (directory)
        data[data_offset + 56..data_offset + 60].copy_from_slice(&0x10000000u32.to_le_bytes());
        
        // Reparse point (0)
        data[data_offset + 60..data_offset + 64].copy_from_slice(&0u32.to_le_bytes());
        
        // Name length in characters
        data[data_offset + 64] = name_utf16.len() as u8;
        
        // Namespace (Win32)
        data[data_offset + 65] = 1;
        
        // Name in UTF-16LE
        let name_offset = data_offset + 66;
        for (i, ch) in name_utf16.iter().enumerate() {
            data[name_offset + i * 2..name_offset + i * 2 + 2].copy_from_slice(&ch.to_le_bytes());
        }
        
        // Align to 8 bytes
        let aligned_size = (attr_size + 7) & !7;
        Ok(aligned_size)
    }
    
    /// Add $INDEX_ROOT attribute for directory
    fn add_index_root_attribute(&self, data: &mut [u8], offset: usize) -> Result<usize, MosesError> {
        // Create empty INDEX_ROOT
        let index_root_data = self.index_writer.create_empty_index_root();
        let attr_size = 24 + index_root_data.len(); // Header + data
        
        if offset + attr_size > data.len() {
            return Err(MosesError::Other("Buffer too small for index root".to_string()));
        }
        
        // Attribute header
        data[offset..offset + 4].copy_from_slice(&ATTR_TYPE_INDEX_ROOT.to_le_bytes());
        data[offset + 4..offset + 8].copy_from_slice(&(attr_size as u32).to_le_bytes());
        data[offset + 8] = 0; // Resident
        data[offset + 9] = 0; // Name length = 0
        data[offset + 10..offset + 12].copy_from_slice(&0u16.to_le_bytes()); // Name offset
        data[offset + 12..offset + 14].copy_from_slice(&0u16.to_le_bytes()); // Flags
        data[offset + 14..offset + 16].copy_from_slice(&0u16.to_le_bytes()); // Attribute ID
        
        // Resident header
        data[offset + 16..offset + 20].copy_from_slice(&(index_root_data.len() as u32).to_le_bytes());
        data[offset + 20..offset + 22].copy_from_slice(&24u16.to_le_bytes()); // Value offset
        data[offset + 22] = 0; // Flags
        data[offset + 23] = 0; // Reserved
        
        // Copy INDEX_ROOT data
        data[offset + 24..offset + 24 + index_root_data.len()].copy_from_slice(&index_root_data);
        
        // Align to 8 bytes
        let aligned_size = (attr_size + 7) & !7;
        Ok(aligned_size)
    }
    
    /// Split a path into parent directory and filename
    fn split_path(&self, path: &str) -> Result<(&str, &str), MosesError> {
        let normalized = path.trim_end_matches('/');
        
        if normalized.is_empty() || normalized == "/" {
            return Err(MosesError::Other("Cannot create root directory".to_string()));
        }
        
        if let Some(pos) = normalized.rfind('/') {
            if pos == 0 {
                Ok(("/", &normalized[1..]))
            } else {
                Ok((&normalized[..pos], &normalized[pos + 1..]))
            }
        } else {
            Ok(("/", normalized))
        }
    }
}