// NTFS Index (B+ tree) support for directories
// Phase 2.1: Parse INDEX_ROOT and INDEX_ALLOCATION for large directories

use crate::families::ntfs::ntfs::structures::*;
use moses_core::MosesError;
use log::trace;

// Index header flags
pub const INDEX_NODE: u32 = 0x01;  // This is an index node (has children)

/// Index header structure (common to INDEX_ROOT and INDEX_ALLOCATION)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct IndexHeader {
    pub entries_offset: u32,          // Offset to first entry
    pub index_length: u32,            // Total length of index entries
    pub allocated_size: u32,          // Allocated size of index
    pub flags: u32,                   // Index flags
}

/// Index root attribute structure
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct IndexRoot {
    pub attribute_type: u32,          // Type of attribute being indexed (usually FILE_NAME)
    pub collation_rule: u32,          // Collation rule (usually COLLATION_FILE_NAME)
    pub index_block_size: u32,        // Size of index allocation blocks
    pub clusters_per_block: u8,       // Clusters per index block
    pub reserved: [u8; 3],            // Reserved/padding
    pub header: IndexHeader,          // Index header
}

/// Index entry header
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct IndexEntryHeader {
    pub mft_reference: u64,           // MFT reference (0 for last entry)
    pub length: u16,                  // Length of this entry
    pub key_length: u16,              // Length of key (filename)
    pub flags: u16,                   // Entry flags
    pub reserved: u16,                // Reserved
}

// Index entry flags
pub const INDEX_ENTRY_NODE: u16 = 0x01;      // Has sub-node
pub const INDEX_ENTRY_END: u16 = 0x02;       // Last entry in node

/// Parse an INDEX_ROOT attribute
pub fn parse_index_root(data: &[u8]) -> Result<Vec<IndexEntry>, MosesError> {
    if data.len() < std::mem::size_of::<IndexRoot>() {
        return Err(MosesError::Other("Index root too small".to_string()));
    }
    
    let root = unsafe {
        std::ptr::read_unaligned(data.as_ptr() as *const IndexRoot)
    };
    
    // Validate
    let attribute_type = root.attribute_type;
    if attribute_type != ATTR_TYPE_FILE_NAME {
        trace!("Index root for non-filename attribute: 0x{:X}", attribute_type);
    }
    
    // Parse entries starting after the IndexRoot structure
    let entries_offset = std::mem::size_of::<IndexRoot>() + root.header.entries_offset as usize;
    let index_length = root.header.index_length as usize;
    
    if entries_offset + index_length > data.len() {
        return Err(MosesError::Other("Index entries beyond buffer".to_string()));
    }
    
    parse_index_entries(&data[entries_offset..entries_offset + index_length])
}

/// Parse an INDEX_ALLOCATION attribute (for large directories)
pub fn parse_index_allocation(data: &[u8], index_block_size: u32) -> Result<Vec<IndexEntry>, MosesError> {
    let mut all_entries = Vec::new();
    let mut offset = 0;
    
    while offset < data.len() {
        // Each index block starts with "INDX" signature
        if offset + 24 > data.len() {
            break;
        }
        
        // Check signature
        if &data[offset..offset + 4] != b"INDX" {
            trace!("Invalid index block signature at offset {}", offset);
            break;
        }
        
        // Skip INDX header (similar to MFT record header)
        // Structure: signature(4) + usa_offset(2) + usa_count(2) + lsn(8) + vcn(8)
        let _header_size = 24;
        
        // Get index header (starts after INDX header + USA)
        let usa_offset = u16::from_le_bytes([data[offset + 4], data[offset + 5]]) as usize;
        let usa_count = u16::from_le_bytes([data[offset + 6], data[offset + 7]]) as usize;
        let index_offset = offset + usa_offset + (usa_count * 2);
        
        if index_offset + std::mem::size_of::<IndexHeader>() > data.len() {
            break;
        }
        
        let index_header = unsafe {
            std::ptr::read_unaligned(&data[index_offset] as *const u8 as *const IndexHeader)
        };
        
        let entries_offset = index_offset + index_header.entries_offset as usize;
        let entries_length = index_header.index_length as usize;
        
        if entries_offset + entries_length <= data.len() {
            let entries = parse_index_entries(&data[entries_offset..entries_offset + entries_length])?;
            all_entries.extend(entries);
        }
        
        offset += index_block_size as usize;
    }
    
    Ok(all_entries)
}

/// Parsed index entry
#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub mft_reference: u64,
    pub file_name: String,
    pub is_directory: bool,
    pub has_subnode: bool,
}

/// Parse index entries from a buffer
fn parse_index_entries(data: &[u8]) -> Result<Vec<IndexEntry>, MosesError> {
    let mut entries = Vec::new();
    let mut offset = 0;
    
    while offset < data.len() {
        if offset + std::mem::size_of::<IndexEntryHeader>() > data.len() {
            break;
        }
        
        let header = unsafe {
            std::ptr::read_unaligned(&data[offset] as *const u8 as *const IndexEntryHeader)
        };
        
        let entry_length = header.length as usize;
        if entry_length == 0 || offset + entry_length > data.len() {
            break;
        }
        
        // Check if this is the last entry
        let flags = header.flags;
        if flags & INDEX_ENTRY_END != 0 {
            // Last entry, no filename
            break;
        }
        
        // Parse the FILE_NAME attribute that follows the header
        let key_offset = offset + std::mem::size_of::<IndexEntryHeader>();
        let key_length = header.key_length as usize;
        
        if key_length >= std::mem::size_of::<FileNameAttr>() && key_offset + key_length <= data.len() {
            let file_name_attr = unsafe {
                std::ptr::read_unaligned(&data[key_offset] as *const u8 as *const FileNameAttr)
            };
            
            // Parse filename
            let name_offset = key_offset + std::mem::size_of::<FileNameAttr>();
            let name_length = file_name_attr.name_length as usize * 2; // UTF-16
            
            if name_offset + name_length <= data.len() {
                let name_bytes = &data[name_offset..name_offset + name_length];
                let name = parse_utf16le_string(name_bytes)?;
                
                let file_attributes = file_name_attr.file_attributes;
                let is_directory = file_attributes & 0x10000000 != 0; // FILE_ATTRIBUTE_DIRECTORY
                
                entries.push(IndexEntry {
                    mft_reference: header.mft_reference & 0xFFFFFFFFFFFF, // Lower 48 bits
                    file_name: name,
                    is_directory,
                    has_subnode: flags & INDEX_ENTRY_NODE != 0,
                });
            }
        }
        
        offset += entry_length;
    }
    
    Ok(entries)
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
    fn test_index_entry_parsing() {
        // Create a minimal index entry
        let mut data = vec![0u8; 256];
        
        // IndexEntryHeader
        let mft_ref = 100u64;
        data[0..8].copy_from_slice(&mft_ref.to_le_bytes());
        data[8..10].copy_from_slice(&90u16.to_le_bytes()); // Entry length
        data[10..12].copy_from_slice(&74u16.to_le_bytes()); // Key length
        data[12..14].copy_from_slice(&0u16.to_le_bytes()); // Flags
        
        // FILE_NAME attribute
        let key_offset = 16;
        data[key_offset..key_offset + 8].copy_from_slice(&5u64.to_le_bytes()); // Parent ref
        // Skip timestamps (8 * 4 = 32 bytes)
        // Sizes
        data[key_offset + 40..key_offset + 48].copy_from_slice(&1024u64.to_le_bytes());
        data[key_offset + 48..key_offset + 56].copy_from_slice(&512u64.to_le_bytes());
        // Attributes
        data[key_offset + 56..key_offset + 60].copy_from_slice(&0x20u32.to_le_bytes());
        // Name length
        data[key_offset + 64] = 4; // 4 characters
        data[key_offset + 65] = FILE_NAME_WIN32;
        
        // Filename "test" in UTF-16LE
        data[key_offset + 66] = b't';
        data[key_offset + 67] = 0;
        data[key_offset + 68] = b'e';
        data[key_offset + 69] = 0;
        data[key_offset + 70] = b's';
        data[key_offset + 71] = 0;
        data[key_offset + 72] = b't';
        data[key_offset + 73] = 0;
        
        // Add end marker
        data[90..98].copy_from_slice(&0u64.to_le_bytes()); // MFT ref = 0
        data[98..100].copy_from_slice(&16u16.to_le_bytes()); // Length
        data[100..102].copy_from_slice(&0u16.to_le_bytes()); // Key length
        data[102..104].copy_from_slice(&INDEX_ENTRY_END.to_le_bytes()); // Flags
        
        let entries = parse_index_entries(&data[..106]).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file_name, "test");
        assert_eq!(entries[0].mft_reference, 100);
    }
}