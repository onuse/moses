// NTFS Attribute parsers
// Phase 1.3: Parse standard NTFS attributes

use crate::ntfs::structures::*;
use crate::ntfs::data_runs::{decode_data_runs, DataRun};
use moses_core::MosesError;
use log::trace;

/// Parsed attribute data
#[derive(Debug, Clone)]
pub enum AttributeData {
    StandardInformation(StandardInformation),
    FileName(FileNameAttr, String),
    Data(Vec<u8>),  // For resident data
    DataRuns(Vec<DataRun>),  // For non-resident data
    CompressedDataRuns(Vec<DataRun>, u16, u64, u64),  // runs, compression_unit, data_size, initialized_size
    IndexRoot(Vec<u8>),  // Directory index
    Unknown(Vec<u8>),
}

/// Parse an attribute from raw MFT record data
pub fn parse_attribute(data: &[u8], offset: usize) -> Result<(AttributeHeader, AttributeData), MosesError> {
    if offset + 16 > data.len() {
        return Err(MosesError::Other("Attribute header beyond buffer".to_string()));
    }
    
    // Parse common header
    let header = unsafe {
        std::ptr::read_unaligned(&data[offset] as *const u8 as *const AttributeHeader)
    };
    
    // Validate header
    if header.type_code == ATTR_TYPE_END || header.record_length == 0 {
        return Err(MosesError::Other("Invalid attribute header".to_string()));
    }
    
    let attr_data = if header.non_resident == 0 {
        // Resident attribute
        parse_resident_attribute(data, offset, &header)?
    } else {
        // Non-resident attribute
        parse_non_resident_attribute(data, offset, &header)?
    };
    
    Ok((header, attr_data))
}

/// Parse a resident attribute
fn parse_resident_attribute(data: &[u8], offset: usize, header: &AttributeHeader) -> Result<AttributeData, MosesError> {
    // Parse resident header
    let res_header = unsafe {
        std::ptr::read_unaligned(&data[offset] as *const u8 as *const ResidentAttributeHeader)
    };
    
    let value_offset = offset + res_header.value_offset as usize;
    let value_length = res_header.value_length as usize;
    
    if value_offset + value_length > data.len() {
        return Err(MosesError::Other("Attribute value beyond buffer".to_string()));
    }
    
    let value_data = &data[value_offset..value_offset + value_length];
    
    match header.type_code {
        ATTR_TYPE_STANDARD_INFORMATION => {
            if value_length >= std::mem::size_of::<StandardInformation>() {
                let std_info = unsafe {
                    std::ptr::read_unaligned(value_data.as_ptr() as *const StandardInformation)
                };
                Ok(AttributeData::StandardInformation(std_info))
            } else {
                Err(MosesError::Other("Standard information too small".to_string()))
            }
        }
        
        ATTR_TYPE_FILE_NAME => {
            if value_length >= std::mem::size_of::<FileNameAttr>() {
                let file_name_attr = unsafe {
                    std::ptr::read_unaligned(value_data.as_ptr() as *const FileNameAttr)
                };
                
                // Parse the filename (UTF-16LE)
                let name_offset = std::mem::size_of::<FileNameAttr>();
                let name_length_field = file_name_attr.name_length; // Copy to avoid unaligned access
                let name_length = name_length_field as usize * 2; // UTF-16
                
                if name_offset + name_length <= value_length {
                    let name_bytes = &value_data[name_offset..name_offset + name_length];
                    let name = parse_utf16le_string(name_bytes)?;
                    Ok(AttributeData::FileName(file_name_attr, name))
                } else {
                    Err(MosesError::Other("File name beyond buffer".to_string()))
                }
            } else {
                Err(MosesError::Other("File name attribute too small".to_string()))
            }
        }
        
        ATTR_TYPE_DATA => {
            // Resident data - just return the bytes
            Ok(AttributeData::Data(value_data.to_vec()))
        }
        
        ATTR_TYPE_INDEX_ROOT => {
            // Directory index root
            Ok(AttributeData::IndexRoot(value_data.to_vec()))
        }
        
        _ => {
            let type_code = header.type_code; // Copy to avoid unaligned access
            trace!("Unknown resident attribute type: 0x{:X}", type_code);
            Ok(AttributeData::Unknown(value_data.to_vec()))
        }
    }
}

/// Parse a non-resident attribute
fn parse_non_resident_attribute(data: &[u8], offset: usize, header: &AttributeHeader) -> Result<AttributeData, MosesError> {
    // Parse non-resident header
    let nr_header = unsafe {
        std::ptr::read_unaligned(&data[offset] as *const u8 as *const NonResidentAttributeHeader)
    };
    
    let runs_offset = offset + nr_header.data_runs_offset as usize;
    
    // Find the end of data runs (terminated by 0x00)
    let mut runs_end = runs_offset;
    while runs_end < data.len() && data[runs_end] != 0 {
        let header_byte = data[runs_end];
        let length_size = (header_byte & 0x0F) as usize;
        let offset_size = ((header_byte >> 4) & 0x0F) as usize;
        runs_end += 1 + length_size + offset_size;
    }
    
    if runs_end > data.len() {
        return Err(MosesError::Other("Data runs extend beyond buffer".to_string()));
    }
    
    let runs_data = &data[runs_offset..runs_end];
    
    match header.type_code {
        ATTR_TYPE_DATA => {
            // Parse data runs
            let runs = decode_data_runs(runs_data)?;
            
            // Check if compressed (compression_unit != 0)
            let compression_unit = nr_header.compression_unit;
            if compression_unit != 0 {
                let data_size = nr_header.data_size;
                let initialized_size = nr_header.initialized_size;
                Ok(AttributeData::CompressedDataRuns(runs, compression_unit, data_size, initialized_size))
            } else {
                Ok(AttributeData::DataRuns(runs))
            }
        }
        
        ATTR_TYPE_INDEX_ALLOCATION => {
            // Index allocations are never compressed
            let runs = decode_data_runs(runs_data)?;
            Ok(AttributeData::DataRuns(runs))
        }
        
        _ => {
            let type_code = header.type_code; // Copy to avoid unaligned access
            trace!("Unknown non-resident attribute type: 0x{:X}", type_code);
            let runs = decode_data_runs(runs_data)?;
            Ok(AttributeData::DataRuns(runs))
        }
    }
}

/// Parse UTF-16LE string
fn parse_utf16le_string(data: &[u8]) -> Result<String, MosesError> {
    if data.len() % 2 != 0 {
        return Err(MosesError::Other("Invalid UTF-16 string length".to_string()));
    }
    
    let utf16_chars: Vec<u16> = data
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    
    String::from_utf16(&utf16_chars)
        .map_err(|_| MosesError::Other("Invalid UTF-16 string".to_string()))
}