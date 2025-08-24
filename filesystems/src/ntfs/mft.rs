// MFT (Master File Table) parser
// Phase 1.2: MFT Record Parser with Fixup Support

use crate::device_reader::AlignedDeviceReader;
use crate::ntfs::structures::*;
use moses_core::MosesError;
use log::{debug, trace};

/// Apply Update Sequence Array (USA) fixup to an MFT record or index buffer
/// This detects and corrects torn writes
pub fn apply_fixup(buffer: &mut [u8], usa_offset: u16, usa_count: u16) -> Result<(), MosesError> {
    let usa_offset = usa_offset as usize;
    let usa_count = usa_count as usize;
    
    if usa_offset + usa_count * 2 > buffer.len() {
        return Err(MosesError::Other("USA extends beyond buffer".to_string()));
    }
    
    // First 2 bytes are the update sequence number
    let usn = [buffer[usa_offset], buffer[usa_offset + 1]];
    trace!("Applying fixup with USN: {:02X}{:02X}", usn[0], usn[1]);
    
    // Apply fixup to each sector
    for i in 1..usa_count {
        let usa_value_offset = usa_offset + i * 2;
        let usa_value = [buffer[usa_value_offset], buffer[usa_value_offset + 1]];
        
        // Last 2 bytes of each 512-byte sector
        let sector_offset = i * 512 - 2;
        
        if sector_offset + 2 > buffer.len() {
            return Err(MosesError::Other("Sector offset exceeds buffer".to_string()));
        }
        
        // Check that the sector ends with the USN
        if buffer[sector_offset] != usn[0] || buffer[sector_offset + 1] != usn[1] {
            return Err(MosesError::Other(format!(
                "Fixup mismatch at sector {}: expected {:02X}{:02X}, found {:02X}{:02X}",
                i, usn[0], usn[1], buffer[sector_offset], buffer[sector_offset + 1]
            )));
        }
        
        // Replace with original value
        buffer[sector_offset] = usa_value[0];
        buffer[sector_offset + 1] = usa_value[1];
        trace!("Fixed sector {} at offset {}", i, sector_offset);
    }
    
    Ok(())
}

/// MFT record parser
#[derive(Clone)]
pub struct MftRecord {
    pub header: MftRecordHeader,
    pub data: Vec<u8>,
    attributes_cache: Option<Vec<(AttributeHeader, crate::ntfs::attributes::AttributeData)>>,
}

impl MftRecord {
    /// Parse an MFT record from raw bytes
    pub fn parse(mut data: Vec<u8>) -> Result<Self, MosesError> {
        if data.len() < 48 {
            return Err(MosesError::Other("MFT record too small".to_string()));
        }
        
        // Parse header
        let header = unsafe {
            std::ptr::read_unaligned(data.as_ptr() as *const MftRecordHeader)
        };
        
        // Validate signature
        if !header.is_valid() {
            return Err(MosesError::Other(format!(
                "Invalid MFT signature: {:?}",
                &header.signature
            )));
        }
        
        // Apply fixup if needed
        if header.usa_offset > 0 && header.usa_count > 0 {
            apply_fixup(&mut data, header.usa_offset, header.usa_count)?;
        }
        
        Ok(Self { 
            header, 
            data,
            attributes_cache: None,
        })
    }
    
    /// Check if this record is in use
    pub fn is_in_use(&self) -> bool {
        self.header.is_in_use()
    }
    
    /// Check if this record represents a directory
    pub fn is_directory(&self) -> bool {
        self.header.is_directory()
    }
    
    /// Get the first attribute offset
    pub fn first_attribute_offset(&self) -> usize {
        self.header.attrs_offset as usize
    }
    
    /// Parse all attributes in this record
    pub fn parse_attributes(&mut self) -> Result<&[(AttributeHeader, crate::ntfs::attributes::AttributeData)], MosesError> {
        if self.attributes_cache.is_none() {
            let mut attributes = Vec::new();
            let mut offset = self.first_attribute_offset();
            
            while offset + 16 <= self.data.len() {
                // Check for end marker
                if self.data[offset..offset + 4] == [0xFF, 0xFF, 0xFF, 0xFF] || 
                   self.data[offset..offset + 4] == [0x00, 0x00, 0x00, 0x00] {
                    break;
                }
                
                match crate::ntfs::attributes::parse_attribute(&self.data, offset) {
                    Ok((header, data)) => {
                        let record_length = header.record_length;
                        attributes.push((header, data));
                        
                        if record_length == 0 || record_length > 65536 {
                            break;
                        }
                        offset += record_length as usize;
                    }
                    Err(e) => {
                        debug!("Failed to parse attribute at offset {}: {:?}", offset, e);
                        break;
                    }
                }
            }
            
            self.attributes_cache = Some(attributes);
        }
        
        Ok(self.attributes_cache.as_ref().unwrap())
    }
    
    /// Find an attribute by type
    pub fn find_attribute(&mut self, type_code: u32) -> Option<&crate::ntfs::attributes::AttributeData> {
        self.parse_attributes().ok()?;
        self.attributes_cache.as_ref()?
            .iter()
            .find(|(h, _)| h.type_code == type_code)
            .map(|(_, d)| d)
    }
    
    /// Get all attributes of a specific type
    pub fn find_all_attributes(&mut self, type_code: u32) -> Vec<&crate::ntfs::attributes::AttributeData> {
        self.parse_attributes().ok()
            .and_then(|attrs| {
                Some(attrs.iter()
                    .filter(|(h, _)| h.type_code == type_code)
                    .map(|(_, d)| d)
                    .collect())
            })
            .unwrap_or_default()
    }
    
    /// Iterate over attributes in this record
    pub fn attributes(&self) -> AttributeIterator<'_> {
        AttributeIterator::new(&self.data, self.first_attribute_offset())
    }
    
    /// Check if this record has an attribute list
    pub fn has_attribute_list(&mut self) -> bool {
        self.find_attribute(ATTR_TYPE_ATTRIBUTE_LIST).is_some()
    }
    
    /// Get parsed attribute list entries if present
    pub fn get_attribute_list_entries(&mut self) -> Option<Vec<crate::ntfs::attribute_list::ParsedAttributeListEntry>> {
        use crate::ntfs::attributes::AttributeData;
        
        if let Some(attr) = self.find_attribute(ATTR_TYPE_ATTRIBUTE_LIST) {
            match attr {
                AttributeData::Unknown(data) | AttributeData::Data(data) => {
                    // Parse the attribute list
                    crate::ntfs::attribute_list::parse_attribute_list(&data).ok()
                }
                _ => None
            }
        } else {
            None
        }
    }
}

/// Iterator over attributes in an MFT record
pub struct AttributeIterator<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> AttributeIterator<'a> {
    fn new(data: &'a [u8], start_offset: usize) -> Self {
        Self {
            data,
            offset: start_offset,
        }
    }
}

impl<'a> Iterator for AttributeIterator<'a> {
    type Item = AttributeHeader;
    
    fn next(&mut self) -> Option<Self::Item> {
        // Check if we can read at least the attribute header
        if self.offset + 16 > self.data.len() {
            return None;
        }
        
        // Read attribute header
        let header = unsafe {
            std::ptr::read_unaligned(
                self.data[self.offset..].as_ptr() as *const AttributeHeader
            )
        };
        
        // Check for end marker
        if header.type_code == ATTR_TYPE_END || header.type_code == 0 {
            return None;
        }
        
        // Validate record length
        if header.record_length == 0 || header.record_length > 65536 {
            return None;
        }
        
        // Move to next attribute
        self.offset += header.record_length as usize;
        
        Some(header)
    }
}

/// MFT reader - reads MFT records from disk
pub struct MftReader {
    reader: AlignedDeviceReader,
    mft_offset: u64,
    record_size: u32,
}

impl MftReader {
    pub fn new(reader: AlignedDeviceReader, mft_offset: u64, record_size: u32) -> Self {
        Self {
            reader,
            mft_offset,
            record_size,
        }
    }
    
    /// Read an MFT record by number
    pub fn read_record(&mut self, record_number: u64) -> Result<MftRecord, MosesError> {
        let offset = self.mft_offset + (record_number * self.record_size as u64);
        debug!("Reading MFT record {} at offset {:#x}", record_number, offset);
        
        let data = self.reader.read_at(offset, self.record_size as usize)?;
        MftRecord::parse(data)
    }
    
    /// Read the MFT's own record (record 0)
    pub fn read_mft_record(&mut self) -> Result<MftRecord, MosesError> {
        self.read_record(MFT_RECORD_MFT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_apply_fixup() {
        // Create a mock MFT record with fixup
        let mut data = vec![0u8; 1024];
        
        // Set up USA at offset 0x30
        let usa_offset = 0x30;
        let usa_count = 3; // 1 USN + 2 fixup values
        
        // USN
        data[usa_offset] = 0x01;
        data[usa_offset + 1] = 0x00;
        
        // Original values for end of sectors
        data[usa_offset + 2] = 0xAA;
        data[usa_offset + 3] = 0xBB;
        data[usa_offset + 4] = 0xCC;
        data[usa_offset + 5] = 0xDD;
        
        // Place USN at end of sectors
        data[510] = 0x01;
        data[511] = 0x00;
        data[1022] = 0x01;
        data[1023] = 0x00;
        
        // Apply fixup
        apply_fixup(&mut data, usa_offset as u16, usa_count as u16).unwrap();
        
        // Check that fixup was applied
        assert_eq!(data[510], 0xAA);
        assert_eq!(data[511], 0xBB);
        assert_eq!(data[1022], 0xCC);
        assert_eq!(data[1023], 0xDD);
    }
}