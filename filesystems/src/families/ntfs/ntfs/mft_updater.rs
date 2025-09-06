// MFT Record Updater - Updates attributes in MFT records
// This module handles replacing attributes in existing MFT records

use super::structures::*;
use moses_core::MosesError;
use log::{debug, trace};

/// Updates attributes in MFT records
pub struct MftUpdater;

impl MftUpdater {
    /// Create a new MFT updater
    pub fn new() -> Self {
        Self
    }
    
    /// Replace an attribute in an MFT record
    /// Returns the updated MFT record as raw bytes
    pub fn replace_attribute(
        &self,
        mft_record_data: &[u8],
        attr_type: u32,
        new_attr_data: &[u8],
    ) -> Result<Vec<u8>, MosesError> {
        debug!("Replacing attribute type 0x{:X} in MFT record", attr_type);
        
        if mft_record_data.len() < 56 {
            return Err(MosesError::Other("MFT record too small".to_string()));
        }
        
        // Parse MFT header
        let header = unsafe {
            std::ptr::read_unaligned(mft_record_data.as_ptr() as *const MftRecordHeader)
        };
        
        // Verify signature
        if &header.signature != b"FILE" {
            return Err(MosesError::Other("Invalid MFT record signature".to_string()));
        }
        
        let attrs_offset = header.attrs_offset as usize;
        let bytes_used = header.bytes_used as usize;
        
        if attrs_offset >= mft_record_data.len() || bytes_used > mft_record_data.len() {
            return Err(MosesError::Other("Invalid MFT record offsets".to_string()));
        }
        
        // Find and collect all attributes
        let mut attributes = Vec::new();
        let mut offset = attrs_offset;
        let mut found_target = false;
        
        while offset < bytes_used {
            if offset + 4 > mft_record_data.len() {
                break;
            }
            
            // Read attribute type
            let attr_type_code = u32::from_le_bytes([
                mft_record_data[offset],
                mft_record_data[offset + 1],
                mft_record_data[offset + 2],
                mft_record_data[offset + 3],
            ]);
            
            // Check for end marker
            if attr_type_code == 0xFFFFFFFF {
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
            
            // Check if this is the attribute to replace
            if attr_type_code == attr_type {
                debug!("Found attribute to replace at offset {}", offset);
                found_target = true;
                
                // Create new attribute with proper header
                let new_attr = self.create_attribute(attr_type, new_attr_data)?;
                attributes.push(new_attr);
            } else {
                // Keep existing attribute
                let attr_data = mft_record_data[offset..offset + attr_length as usize].to_vec();
                attributes.push(attr_data);
            }
            
            offset += attr_length as usize;
        }
        
        if !found_target {
            return Err(MosesError::Other(format!("Attribute type 0x{:X} not found", attr_type)));
        }
        
        // Rebuild MFT record with updated attributes
        self.rebuild_mft_record(header, attributes)
    }
    
    /// Create a properly formatted attribute
    fn create_attribute(&self, attr_type: u32, data: &[u8]) -> Result<Vec<u8>, MosesError> {
        // For INDEX_ROOT, the data already includes the full attribute structure
        if attr_type == ATTR_TYPE_INDEX_ROOT {
            // Create resident attribute header
            let data_len = data.len();
            let attr_len = 24 + data_len; // Header + data
            let attr_len_aligned = ((attr_len + 7) / 8) * 8; // 8-byte aligned
            
            let mut attribute = vec![0u8; attr_len_aligned];
            
            // Write attribute header
            // Type
            attribute[0..4].copy_from_slice(&attr_type.to_le_bytes());
            // Length
            attribute[4..8].copy_from_slice(&(attr_len_aligned as u32).to_le_bytes());
            // Non-resident flag (0 = resident)
            attribute[8] = 0;
            // Name length
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
            
            trace!("Created attribute: type=0x{:X}, size={}", attr_type, attribute.len());
            Ok(attribute)
        } else {
            // For other attributes, just wrap the data
            Ok(data.to_vec())
        }
    }
    
    /// Rebuild MFT record with new attributes
    fn rebuild_mft_record(
        &self,
        mut header: MftRecordHeader,
        attributes: Vec<Vec<u8>>,
    ) -> Result<Vec<u8>, MosesError> {
        let record_size = header.bytes_allocated as usize;
        let mut result = vec![0u8; record_size];
        
        // Write header
        unsafe {
            let header_bytes = std::slice::from_raw_parts(
                &header as *const _ as *const u8,
                std::mem::size_of::<MftRecordHeader>()
            );
            result[..header_bytes.len()].copy_from_slice(header_bytes);
        }
        
        // Write USA (Update Sequence Array) if present
        let usa_offset = header.usa_offset as usize;
        let usa_count = header.usa_count as usize;
        
        if usa_offset > 0 && usa_count > 0 && usa_offset + usa_count * 2 <= record_size {
            // Preserve USA from original record
            // This is important for record integrity
            // For now, we'll use a simple USA pattern
            result[usa_offset] = 0x01;
            result[usa_offset + 1] = 0x00;
            
            // Apply USA to sector ends
            for i in 1..usa_count {
                let sector_offset = i * 512 - 2;
                if sector_offset < record_size {
                    result[sector_offset] = 0x01;
                    result[sector_offset + 1] = 0x00;
                }
            }
        }
        
        // Write attributes
        let attrs_offset = header.attrs_offset as usize;
        let mut current_offset = attrs_offset;
        
        for attr in attributes {
            let attr_len = attr.len();
            if current_offset + attr_len > record_size {
                return Err(MosesError::Other("Attributes exceed MFT record size".to_string()));
            }
            
            result[current_offset..current_offset + attr_len].copy_from_slice(&attr);
            current_offset += attr_len;
        }
        
        // Write end marker
        if current_offset + 4 <= record_size {
            result[current_offset..current_offset + 4].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
            current_offset += 4;
        }
        
        // Update header with new size
        header.bytes_used = current_offset as u32;
        
        // Write updated header
        unsafe {
            let header_bytes = std::slice::from_raw_parts(
                &header as *const _ as *const u8,
                std::mem::size_of::<MftRecordHeader>()
            );
            result[..header_bytes.len()].copy_from_slice(header_bytes);
        }
        
        debug!("Rebuilt MFT record: {} bytes used of {}", current_offset, record_size);
        Ok(result)
    }
}