// NTFS Resident Data Writer - Handles writing data to resident attributes
// This module enables writing content to small files stored directly in MFT

use super::structures::*;
use super::mft_updater::MftUpdater;
use moses_core::MosesError;
use log::{debug, trace};

/// Writes data to resident DATA attributes in MFT records
pub struct ResidentDataWriter;

impl ResidentDataWriter {
    /// Create a new resident data writer
    pub fn new() -> Self {
        Self
    }
    
    /// Write data to a resident DATA attribute
    /// Returns the updated MFT record as raw bytes
    pub fn write_resident_data(
        &self,
        mft_record_data: &[u8],
        offset: u64,
        data: &[u8],
    ) -> Result<Vec<u8>, MosesError> {
        debug!("Writing {} bytes to resident data at offset {}", data.len(), offset);
        
        if mft_record_data.len() < 56 {
            return Err(MosesError::Other("MFT record too small".to_string()));
        }
        
        // Find the DATA attribute
        let data_attr = self.find_data_attribute(mft_record_data)?;
        
        // Check if it's resident
        if data_attr.is_non_resident {
            return Err(MosesError::Other("DATA attribute is non-resident".to_string()));
        }
        
        // Get current data
        let mut current_data = data_attr.data.clone();
        
        // Calculate new size
        let end_offset = offset as usize + data.len();
        if end_offset > current_data.len() {
            // Extend the data
            current_data.resize(end_offset, 0);
        }
        
        // Write the new data
        current_data[offset as usize..end_offset].copy_from_slice(data);
        
        // Check if we can still fit in resident storage
        if current_data.len() > 700 {
            // TODO: Convert to non-resident
            return Err(MosesError::NotSupported(
                "Converting resident to non-resident not yet implemented".to_string()
            ));
        }
        
        // Create updated DATA attribute
        let updated_attr = self.create_resident_data_attribute(&current_data)?;
        
        // Use MftUpdater to replace the attribute
        let updater = MftUpdater::new();
        updater.replace_attribute(mft_record_data, ATTR_TYPE_DATA, &updated_attr)
    }
    
    /// Find the DATA attribute in an MFT record
    fn find_data_attribute(&self, mft_record_data: &[u8]) -> Result<DataAttributeInfo, MosesError> {
        // Parse MFT header
        let header = unsafe {
            std::ptr::read_unaligned(mft_record_data.as_ptr() as *const MftRecordHeader)
        };
        
        if &header.signature != b"FILE" {
            return Err(MosesError::Other("Invalid MFT record signature".to_string()));
        }
        
        let attrs_offset = header.attrs_offset as usize;
        let bytes_used = header.bytes_used as usize;
        
        let mut offset = attrs_offset;
        
        while offset < bytes_used && offset + 4 < mft_record_data.len() {
            // Read attribute type
            let attr_type = u32::from_le_bytes([
                mft_record_data[offset],
                mft_record_data[offset + 1],
                mft_record_data[offset + 2],
                mft_record_data[offset + 3],
            ]);
            
            // Check for end marker
            if attr_type == 0xFFFFFFFF {
                break;
            }
            
            // Read attribute length
            if offset + 8 > mft_record_data.len() {
                break;
            }
            
            let attr_length = u32::from_le_bytes([
                mft_record_data[offset + 4],
                mft_record_data[offset + 5],
                mft_record_data[offset + 6],
                mft_record_data[offset + 7],
            ]);
            
            if attr_length == 0 || offset + attr_length as usize > mft_record_data.len() {
                break;
            }
            
            // Check if this is the DATA attribute
            if attr_type == ATTR_TYPE_DATA {
                // Check if resident
                let is_non_resident = mft_record_data[offset + 8] != 0;
                
                if !is_non_resident {
                    // Get resident data
                    let value_length = u32::from_le_bytes([
                        mft_record_data[offset + 16],
                        mft_record_data[offset + 17],
                        mft_record_data[offset + 18],
                        mft_record_data[offset + 19],
                    ]) as usize;
                    
                    let value_offset = u16::from_le_bytes([
                        mft_record_data[offset + 20],
                        mft_record_data[offset + 21],
                    ]) as usize;
                    
                    let data_start = offset + value_offset;
                    let data_end = data_start + value_length;
                    
                    if data_end <= mft_record_data.len() {
                        let data = mft_record_data[data_start..data_end].to_vec();
                        
                        return Ok(DataAttributeInfo {
                            offset,
                            length: attr_length,
                            is_non_resident: false,
                            data,
                        });
                    }
                } else {
                    return Ok(DataAttributeInfo {
                        offset,
                        length: attr_length,
                        is_non_resident: true,
                        data: Vec::new(),
                    });
                }
            }
            
            offset += attr_length as usize;
        }
        
        Err(MosesError::Other("DATA attribute not found".to_string()))
    }
    
    /// Create a resident DATA attribute with the given data
    fn create_resident_data_attribute(&self, data: &[u8]) -> Result<Vec<u8>, MosesError> {
        let data_len = data.len();
        let attr_len = 24 + data_len; // Header + data
        let attr_len_aligned = ((attr_len + 7) / 8) * 8; // 8-byte aligned
        
        let mut attribute = vec![0u8; attr_len_aligned];
        
        // Write attribute header
        // Type
        attribute[0..4].copy_from_slice(&ATTR_TYPE_DATA.to_le_bytes());
        // Length
        attribute[4..8].copy_from_slice(&(attr_len_aligned as u32).to_le_bytes());
        // Non-resident flag (0 = resident)
        attribute[8] = 0;
        // Name length (0 for unnamed DATA)
        attribute[9] = 0;
        // Name offset
        attribute[10..12].copy_from_slice(&0u16.to_le_bytes());
        // Flags
        attribute[12..14].copy_from_slice(&0u16.to_le_bytes());
        // Attribute ID
        attribute[14..16].copy_from_slice(&0u16.to_le_bytes());
        
        // Resident-specific fields
        // Value length
        attribute[16..20].copy_from_slice(&(data_len as u32).to_le_bytes());
        // Value offset (24 = after header)
        attribute[20..22].copy_from_slice(&24u16.to_le_bytes());
        // Indexed flag
        attribute[22] = 0;
        // Padding
        attribute[23] = 0;
        
        // Write data
        attribute[24..24 + data_len].copy_from_slice(data);
        
        trace!("Created resident DATA attribute: {} bytes", attribute.len());
        Ok(attribute)
    }
    
    /// Update file size in STANDARD_INFORMATION attribute
    pub fn update_file_metadata(
        &self,
        mft_record_data: &[u8],
        new_size: u64,
        update_timestamps: bool,
    ) -> Result<Vec<u8>, MosesError> {
        debug!("Updating file metadata: size={}, timestamps={}", new_size, update_timestamps);
        
        // Find STANDARD_INFORMATION attribute
        let std_info = self.find_standard_info(mft_record_data)?;
        
        // Update the relevant fields
        let mut updated_info = std_info.data.clone();
        
        if update_timestamps {
            let current_time = windows_time_now();
            // Update modification time (offset 8)
            updated_info[8..16].copy_from_slice(&current_time.to_le_bytes());
            // Update MFT modification time (offset 16)
            updated_info[16..24].copy_from_slice(&current_time.to_le_bytes());
            // Update access time (offset 24)
            updated_info[24..32].copy_from_slice(&current_time.to_le_bytes());
        }
        
        // Note: File size is not stored in STANDARD_INFORMATION
        // It's stored in FILE_NAME and DATA attributes
        
        // Use MftUpdater to replace the attribute
        let updater = MftUpdater::new();
        updater.replace_attribute(
            mft_record_data,
            ATTR_TYPE_STANDARD_INFORMATION,
            &self.create_standard_info_attribute(&updated_info)?,
        )
    }
    
    /// Find STANDARD_INFORMATION attribute
    fn find_standard_info(&self, _mft_record_data: &[u8]) -> Result<DataAttributeInfo, MosesError> {
        // Similar to find_data_attribute but looks for STANDARD_INFORMATION
        // Implementation would be similar...
        // For now, return a placeholder
        Err(MosesError::NotSupported("STANDARD_INFO update not fully implemented".to_string()))
    }
    
    /// Create STANDARD_INFORMATION attribute
    fn create_standard_info_attribute(&self, data: &[u8]) -> Result<Vec<u8>, MosesError> {
        // Similar to create_resident_data_attribute
        Ok(data.to_vec())
    }
}

/// Information about a DATA attribute
struct DataAttributeInfo {
    #[allow(dead_code)]
    offset: usize,
    #[allow(dead_code)]
    length: u32,
    is_non_resident: bool,
    data: Vec<u8>,
}

/// Get current time in Windows FILETIME format
fn windows_time_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let unix_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    // Convert Unix time to Windows FILETIME (100ns intervals since 1601)
    (unix_time + 11644473600) * 10_000_000
}