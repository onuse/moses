// NTFS Attribute List Support
// Phase 2.4: Handle files with attributes spanning multiple MFT records

use moses_core::MosesError;
use log::trace;

/// Attribute list entry
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct AttributeListEntry {
    pub attribute_type: u32,         // Attribute type
    pub record_length: u16,           // Length of this entry
    pub name_length: u8,              // Name length in characters
    pub name_offset: u8,              // Offset to name
    pub starting_vcn: u64,            // Starting VCN (for non-resident)
    pub base_file_reference: u64,    // MFT reference containing the attribute
    pub attribute_id: u16,            // Attribute ID
}

/// Parsed attribute list entry
#[derive(Debug, Clone)]
pub struct ParsedAttributeListEntry {
    pub attribute_type: u32,
    pub name: String,
    pub starting_vcn: u64,
    pub mft_reference: u64,
    pub attribute_id: u16,
}

/// Parse an ATTRIBUTE_LIST attribute
pub fn parse_attribute_list(data: &[u8]) -> Result<Vec<ParsedAttributeListEntry>, MosesError> {
    let mut entries = Vec::new();
    let mut offset = 0;
    
    while offset < data.len() {
        if offset + std::mem::size_of::<AttributeListEntry>() > data.len() {
            break;
        }
        
        // Read the entry header
        let entry = unsafe {
            std::ptr::read_unaligned(&data[offset] as *const u8 as *const AttributeListEntry)
        };
        
        let record_length = entry.record_length as usize;
        if record_length == 0 || offset + record_length > data.len() {
            break;
        }
        
        // Parse the name if present
        let name = if entry.name_length > 0 {
            let name_offset = offset + entry.name_offset as usize;
            let name_length = entry.name_length as usize * 2; // UTF-16
            
            if name_offset + name_length <= data.len() {
                parse_utf16le_string(&data[name_offset..name_offset + name_length])?
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        
        entries.push(ParsedAttributeListEntry {
            attribute_type: entry.attribute_type,
            name,
            starting_vcn: entry.starting_vcn,
            mft_reference: entry.base_file_reference & 0xFFFFFFFFFFFF, // Lower 48 bits
            attribute_id: entry.attribute_id,
        });
        
        // Copy fields to avoid unaligned access
        let attr_type = entry.attribute_type;
        let mft_ref = entry.base_file_reference & 0xFFFFFFFFFFFF;
        let vcn = entry.starting_vcn;
        
        trace!("Attribute list entry: type=0x{:X}, MFT ref={}, VCN={}", 
            attr_type, mft_ref, vcn);
        
        offset += record_length;
    }
    
    Ok(entries)
}

/// Group attribute list entries by type
pub fn group_attributes_by_type(
    entries: &[ParsedAttributeListEntry]
) -> std::collections::HashMap<u32, Vec<ParsedAttributeListEntry>> {
    use std::collections::HashMap;
    
    let mut grouped = HashMap::new();
    
    for entry in entries {
        grouped.entry(entry.attribute_type)
            .or_insert_with(Vec::new)
            .push(entry.clone());
    }
    
    // Sort entries within each type by VCN
    for entries in grouped.values_mut() {
        entries.sort_by_key(|e| e.starting_vcn);
    }
    
    grouped
}

/// Find all MFT records referenced by an attribute list
pub fn get_referenced_mft_records(entries: &[ParsedAttributeListEntry]) -> Vec<u64> {
    let mut records: Vec<u64> = entries.iter()
        .map(|e| e.mft_reference)
        .collect();
    
    records.sort_unstable();
    records.dedup();
    records
}

/// Check if an attribute list contains fragmented attributes
pub fn has_fragmented_attributes(entries: &[ParsedAttributeListEntry]) -> bool {
    use std::collections::HashMap;
    
    let mut type_counts = HashMap::new();
    
    for entry in entries {
        let key = (entry.attribute_type, entry.name.clone());
        *type_counts.entry(key).or_insert(0) += 1;
    }
    
    // If any attribute appears more than once, it's fragmented
    type_counts.values().any(|&count| count > 1)
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_attribute_list_entry_parsing() {
        // Create a minimal attribute list with one entry
        let mut data = vec![0u8; 64];
        
        // AttributeListEntry
        data[0..4].copy_from_slice(&ATTR_TYPE_DATA.to_le_bytes());
        data[4..6].copy_from_slice(&32u16.to_le_bytes()); // Record length
        data[6] = 0; // No name
        data[7] = 0; // Name offset
        data[8..16].copy_from_slice(&0u64.to_le_bytes()); // Starting VCN
        data[16..24].copy_from_slice(&1234u64.to_le_bytes()); // MFT reference
        data[24..26].copy_from_slice(&1u16.to_le_bytes()); // Attribute ID
        
        let entries = parse_attribute_list(&data[..32]).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].attribute_type, ATTR_TYPE_DATA);
        assert_eq!(entries[0].mft_reference, 1234);
    }
    
    #[test]
    fn test_fragmented_attribute_detection() {
        let entries = vec![
            ParsedAttributeListEntry {
                attribute_type: ATTR_TYPE_DATA,
                name: String::new(),
                starting_vcn: 0,
                mft_reference: 100,
                attribute_id: 1,
            },
            ParsedAttributeListEntry {
                attribute_type: ATTR_TYPE_DATA,
                name: String::new(),
                starting_vcn: 1000,
                mft_reference: 101,
                attribute_id: 2,
            },
        ];
        
        assert!(has_fragmented_attributes(&entries));
    }
    
    #[test]
    fn test_mft_record_references() {
        let entries = vec![
            ParsedAttributeListEntry {
                attribute_type: ATTR_TYPE_DATA,
                name: String::new(),
                starting_vcn: 0,
                mft_reference: 100,
                attribute_id: 1,
            },
            ParsedAttributeListEntry {
                attribute_type: ATTR_TYPE_FILE_NAME,
                name: String::new(),
                starting_vcn: 0,
                mft_reference: 100,
                attribute_id: 2,
            },
            ParsedAttributeListEntry {
                attribute_type: ATTR_TYPE_DATA,
                name: String::new(),
                starting_vcn: 1000,
                mft_reference: 101,
                attribute_id: 3,
            },
        ];
        
        let records = get_referenced_mft_records(&entries);
        assert_eq!(records, vec![100, 101]);
    }
    
    #[test]
    fn test_group_by_type() {
        let entries = vec![
            ParsedAttributeListEntry {
                attribute_type: ATTR_TYPE_DATA,
                name: String::new(),
                starting_vcn: 100,
                mft_reference: 100,
                attribute_id: 1,
            },
            ParsedAttributeListEntry {
                attribute_type: ATTR_TYPE_DATA,
                name: String::new(),
                starting_vcn: 0,
                mft_reference: 100,
                attribute_id: 2,
            },
            ParsedAttributeListEntry {
                attribute_type: ATTR_TYPE_FILE_NAME,
                name: String::new(),
                starting_vcn: 0,
                mft_reference: 100,
                attribute_id: 3,
            },
        ];
        
        let grouped = group_attributes_by_type(&entries);
        assert_eq!(grouped.len(), 2);
        
        let data_attrs = &grouped[&ATTR_TYPE_DATA];
        assert_eq!(data_attrs.len(), 2);
        // Should be sorted by VCN
        assert_eq!(data_attrs[0].starting_vcn, 0);
        assert_eq!(data_attrs[1].starting_vcn, 100);
    }
}