// MFT Writer - Phase 3.2: MFT Record Creation and Modification
// Provides safe methods for creating and modifying MFT records

use crate::ntfs::structures::*;
use moses_core::MosesError;
// use log::debug;  // Uncomment when needed

/// Builder for creating new MFT records
pub struct MftRecordBuilder {
    record_num: u64,
    record_size: u32,
    flags: u16,
    attributes: Vec<(AttributeHeader, Vec<u8>)>,
    used_size: u32,
}

impl MftRecordBuilder {
    /// Create a new MFT record builder
    pub fn new(record_num: u64, record_size: u32) -> Self {
        Self {
            record_num,
            record_size,
            flags: 0,
            attributes: Vec::new(),
            used_size: 56, // Size of MFT header + first attribute offset
        }
    }
    
    /// Set the record as a file
    pub fn as_file(mut self) -> Self {
        self.flags |= MFT_RECORD_IN_USE;
        self
    }
    
    /// Set the record as a directory
    pub fn as_directory(mut self) -> Self {
        self.flags |= MFT_RECORD_IN_USE | MFT_RECORD_IS_DIRECTORY;
        self
    }
    
    /// Add a standard information attribute
    pub fn with_standard_info(
        mut self,
        created: u64,
        modified: u64,
        accessed: u64,
        file_attributes: u32,
    ) -> Result<Self, MosesError> {
        let mut data = vec![0u8; 72];
        
        // Standard Information structure (resident)
        // Creation time
        data[0..8].copy_from_slice(&created.to_le_bytes());
        // Modified time
        data[8..16].copy_from_slice(&modified.to_le_bytes());
        // MFT modified time
        data[16..24].copy_from_slice(&modified.to_le_bytes());
        // Accessed time
        data[24..32].copy_from_slice(&accessed.to_le_bytes());
        // File attributes
        data[32..36].copy_from_slice(&file_attributes.to_le_bytes());
        // Maximum versions (0)
        data[36..40].copy_from_slice(&0u32.to_le_bytes());
        // Version number (0)
        data[40..44].copy_from_slice(&0u32.to_le_bytes());
        // Class ID (0)
        data[44..48].copy_from_slice(&0u32.to_le_bytes());
        
        // Owner ID (0) - NTFS 3.0+
        if data.len() >= 52 {
            data[48..52].copy_from_slice(&0u32.to_le_bytes());
        }
        // Security ID (0) - NTFS 3.0+
        if data.len() >= 56 {
            data[52..56].copy_from_slice(&0u32.to_le_bytes());
        }
        // Quota charged (0) - NTFS 3.0+
        if data.len() >= 64 {
            data[56..64].copy_from_slice(&0u64.to_le_bytes());
        }
        // USN (0) - NTFS 3.0+
        if data.len() >= 72 {
            data[64..72].copy_from_slice(&0u64.to_le_bytes());
        }
        
        self.add_resident_attribute(ATTR_TYPE_STANDARD_INFORMATION, data)?;
        Ok(self)
    }
    
    /// Add a file name attribute
    pub fn with_file_name(
        mut self,
        parent_ref: u64,
        name: &str,
        namespace: u8,
        created: u64,
        modified: u64,
        accessed: u64,
        allocated_size: u64,
        real_size: u64,
        _file_attributes: u32,
    ) -> Result<Self, MosesError> {
        // Convert name to UTF-16
        let name_utf16: Vec<u16> = name.encode_utf16().collect();
        let name_length = name_utf16.len() as u8;
        
        // Calculate attribute size
        let attr_size = 66 + name_utf16.len() * 2;
        let mut data = vec![0u8; attr_size];
        
        // Parent reference (6 bytes + 2 bytes sequence)
        data[0..8].copy_from_slice(&parent_ref.to_le_bytes());
        
        // Creation time
        data[8..16].copy_from_slice(&created.to_le_bytes());
        // Modified time
        data[16..24].copy_from_slice(&modified.to_le_bytes());
        // MFT modified time
        data[24..32].copy_from_slice(&modified.to_le_bytes());
        // Accessed time
        data[32..40].copy_from_slice(&accessed.to_le_bytes());
        
        // Allocated size
        data[40..48].copy_from_slice(&allocated_size.to_le_bytes());
        // Real size
        data[48..56].copy_from_slice(&real_size.to_le_bytes());
        
        // Flags
        data[56..60].copy_from_slice(&0u32.to_le_bytes());
        
        // EA size and reparse tag
        data[60..64].copy_from_slice(&0u32.to_le_bytes());
        
        // File name length in characters
        data[64] = name_length;
        
        // Namespace (1 = POSIX, 3 = Win32 & DOS)
        data[65] = namespace;
        
        // File name (UTF-16LE)
        for (i, &ch) in name_utf16.iter().enumerate() {
            let offset = 66 + i * 2;
            data[offset..offset + 2].copy_from_slice(&ch.to_le_bytes());
        }
        
        self.add_resident_attribute(ATTR_TYPE_FILE_NAME, data)?;
        Ok(self)
    }
    
    /// Add an empty data attribute (for files)
    pub fn with_empty_data(mut self) -> Result<Self, MosesError> {
        // Empty resident data attribute
        self.add_resident_attribute(ATTR_TYPE_DATA, Vec::new())?;
        Ok(self)
    }
    
    /// Add an index root attribute (for directories)
    pub fn with_index_root(mut self, index_type: u32) -> Result<Self, MosesError> {
        // Basic index root structure
        let mut data = vec![0u8; 64];
        
        // Attribute type being indexed (usually FILE_NAME for directories)
        data[0..4].copy_from_slice(&index_type.to_le_bytes());
        // Collation rule (1 = filename)
        data[4..8].copy_from_slice(&1u32.to_le_bytes());
        // Index record size in bytes
        data[8..12].copy_from_slice(&4096u32.to_le_bytes());
        // Index record size in clusters
        data[12] = 1;
        
        // Index header at offset 16
        let header_offset = 16;
        // First entry offset (relative to header)
        data[header_offset..header_offset + 4].copy_from_slice(&24u32.to_le_bytes());
        // Total size of entries
        data[header_offset + 4..header_offset + 8].copy_from_slice(&32u32.to_le_bytes());
        // Allocated size of entries
        data[header_offset + 8..header_offset + 12].copy_from_slice(&32u32.to_le_bytes());
        // Flags (1 = has children in index allocation)
        data[header_offset + 12] = 0;
        
        // End entry at offset 40
        let entry_offset = 40;
        // Entry length
        data[entry_offset..entry_offset + 2].copy_from_slice(&24u16.to_le_bytes());
        // Key length
        data[entry_offset + 2..entry_offset + 4].copy_from_slice(&0u16.to_le_bytes());
        // Flags (2 = last entry)
        data[entry_offset + 4..entry_offset + 8].copy_from_slice(&2u32.to_le_bytes());
        
        self.add_resident_attribute(ATTR_TYPE_INDEX_ROOT, data)?;
        Ok(self)
    }
    
    /// Add an index allocation attribute (for large directories)
    pub fn with_index_allocation(mut self, clusters: Vec<u64>) -> Result<Self, MosesError> {
        // Create data runs for the allocated clusters
        let mut data_runs = Vec::new();
        
        for (i, &cluster) in clusters.iter().enumerate() {
            let run_length = 1u64; // One cluster at a time for simplicity
            let run_offset = if i == 0 {
                cluster as i64
            } else {
                cluster as i64 - clusters[i - 1] as i64
            };
            
            // Encode data run
            let mut run = Vec::new();
            
            // Header byte: length size | offset size << 4
            let length_bytes = if run_length <= 0xFF { 1 }
                else if run_length <= 0xFFFF { 2 }
                else if run_length <= 0xFFFFFF { 3 }
                else { 4 };
            
            let offset_bytes = if run_offset.abs() <= 0x7F { 1 }
                else if run_offset.abs() <= 0x7FFF { 2 }
                else if run_offset.abs() <= 0x7FFFFF { 3 }
                else { 4 };
            
            run.push(length_bytes | (offset_bytes << 4));
            
            // Length
            for i in 0..length_bytes {
                run.push(((run_length >> (i * 8)) & 0xFF) as u8);
            }
            
            // Offset
            for i in 0..offset_bytes {
                run.push(((run_offset >> (i * 8)) & 0xFF) as u8);
            }
            
            data_runs.extend_from_slice(&run);
        }
        
        // End marker
        data_runs.push(0);
        
        self.add_non_resident_attribute(
            ATTR_TYPE_INDEX_ALLOCATION,
            data_runs,
            clusters.len() as u64 * 4096,  // Assuming 4KB clusters
            clusters.len() as u64 * 4096,
        )?;
        Ok(self)
    }
    
    /// Add a resident attribute
    fn add_resident_attribute(
        &mut self,
        type_code: u32,
        data: Vec<u8>,
    ) -> Result<(), MosesError> {
        let data_len = data.len() as u16;
        let attr_len = 24 + data_len; // Header + data
        let attr_len_aligned = ((attr_len + 7) / 8) * 8; // 8-byte aligned
        
        // Check if we have space
        if self.used_size + attr_len_aligned as u32 > self.record_size {
            return Err(MosesError::Other("MFT record full".to_string()));
        }
        
        let header = AttributeHeader {
            type_code,
            record_length: attr_len_aligned as u32,
            non_resident: 0,
            name_length: 0,
            name_offset: 0,
            flags: 0,
            attribute_id: self.attributes.len() as u16,
        };
        
        // Create attribute buffer
        let mut attr_buf = vec![0u8; attr_len_aligned as usize];
        
        // Write header
        unsafe {
            let header_bytes = std::slice::from_raw_parts(
                &header as *const _ as *const u8,
                std::mem::size_of::<AttributeHeader>()
            );
            attr_buf[..header_bytes.len()].copy_from_slice(header_bytes);
        }
        
        // Write resident-specific fields after the common header (16 bytes)
        // Value length
        attr_buf[16..20].copy_from_slice(&(data_len as u32).to_le_bytes());
        // Value offset (24 = after resident header)
        attr_buf[20..22].copy_from_slice(&24u16.to_le_bytes());
        // Indexed flag
        attr_buf[22] = 0;
        // Padding
        attr_buf[23] = 0;
        
        // Write data
        attr_buf[24..24 + data.len()].copy_from_slice(&data);
        
        self.attributes.push((header, attr_buf));
        self.used_size += attr_len_aligned as u32;
        
        Ok(())
    }
    
    /// Add a non-resident attribute
    fn add_non_resident_attribute(
        &mut self,
        type_code: u32,
        data_runs: Vec<u8>,
        allocated_size: u64,
        real_size: u64,
    ) -> Result<(), MosesError> {
        let attr_len = 64 + data_runs.len(); // Non-resident header + data runs
        let attr_len_aligned = ((attr_len + 7) / 8) * 8; // 8-byte aligned
        
        // Check if we have space
        if self.used_size + attr_len_aligned as u32 > self.record_size {
            return Err(MosesError::Other("MFT record full".to_string()));
        }
        
        let header = AttributeHeader {
            type_code,
            record_length: attr_len_aligned as u32,
            non_resident: 1,
            name_length: 0,
            name_offset: 0,
            flags: 0,
            attribute_id: self.attributes.len() as u16,
        };
        
        // Create attribute buffer
        let mut attr_buf = vec![0u8; attr_len_aligned];
        
        // Write header
        unsafe {
            let header_bytes = std::slice::from_raw_parts(
                &header as *const _ as *const u8,
                std::mem::size_of::<AttributeHeader>()
            );
            attr_buf[..header_bytes.len()].copy_from_slice(header_bytes);
        }
        
        // Write non-resident specific fields
        // Starting VCN
        attr_buf[16..24].copy_from_slice(&0u64.to_le_bytes());
        // Ending VCN
        let ending_vcn = (allocated_size / 4096) - 1;
        attr_buf[24..32].copy_from_slice(&ending_vcn.to_le_bytes());
        // Data runs offset
        attr_buf[32..34].copy_from_slice(&64u16.to_le_bytes());
        // Compression unit size
        attr_buf[34..36].copy_from_slice(&0u16.to_le_bytes());
        // Padding
        attr_buf[36..40].copy_from_slice(&[0u8; 4]);
        // Allocated size
        attr_buf[40..48].copy_from_slice(&allocated_size.to_le_bytes());
        // Real size
        attr_buf[48..56].copy_from_slice(&real_size.to_le_bytes());
        // Initialized size
        attr_buf[56..64].copy_from_slice(&real_size.to_le_bytes());
        
        // Write data runs
        attr_buf[64..64 + data_runs.len()].copy_from_slice(&data_runs);
        
        self.attributes.push((header, attr_buf));
        self.used_size += attr_len_aligned as u32;
        
        Ok(())
    }
    
    /// Build the MFT record
    pub fn build(self) -> Result<Vec<u8>, MosesError> {
        let mut record = vec![0u8; self.record_size as usize];
        
        // Create MFT header
        let header = MftRecordHeader {
            signature: *b"FILE",
            usa_offset: 48,  // Right after header
            usa_count: (self.record_size / 512) as u16 + 1,
            lsn: 0,
            sequence_number: 1,
            link_count: 1,
            attrs_offset: 56,  // After USA
            flags: self.flags,
            bytes_used: self.used_size,
            bytes_allocated: self.record_size,
            base_mft_record: 0,
            next_attr_id: self.attributes.len() as u16,
            reserved: 0,
            mft_record_number: self.record_num as u32,
        };
        
        // Write header
        unsafe {
            let header_bytes = std::slice::from_raw_parts(
                &header as *const _ as *const u8,
                std::mem::size_of::<MftRecordHeader>()
            );
            record[..header_bytes.len()].copy_from_slice(header_bytes);
        }
        
        // Write USA (Update Sequence Array)
        let usa_offset = header.usa_offset as usize;
        let usa_count = header.usa_count as usize;
        
        // USN (Update Sequence Number) - arbitrary value
        record[usa_offset] = 0x01;
        record[usa_offset + 1] = 0x00;
        
        // Save and replace last 2 bytes of each sector
        for i in 1..usa_count {
            let sector_offset = i * 512 - 2;
            let usa_value_offset = usa_offset + i * 2;
            
            // Save original bytes to USA
            record[usa_value_offset] = record[sector_offset];
            record[usa_value_offset + 1] = record[sector_offset + 1];
            
            // Replace with USN
            record[sector_offset] = 0x01;
            record[sector_offset + 1] = 0x00;
        }
        
        // Write attributes
        let mut attr_offset = header.attrs_offset as usize;
        for (_, attr_data) in self.attributes {
            let attr_len = attr_data.len();
            record[attr_offset..attr_offset + attr_len].copy_from_slice(&attr_data);
            attr_offset += attr_len;
        }
        
        // Write end marker
        record[attr_offset..attr_offset + 4].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        
        Ok(record)
    }
}

/// Create a basic file MFT record
pub fn create_file_record(
    record_num: u64,
    record_size: u32,
    parent_ref: u64,
    name: &str,
    created: u64,
    modified: u64,
    accessed: u64,
    size: u64,
) -> Result<Vec<u8>, MosesError> {
    MftRecordBuilder::new(record_num, record_size)
        .as_file()
        .with_standard_info(created, modified, accessed, 0x20)? // FILE_ATTRIBUTE_ARCHIVE
        .with_file_name(
            parent_ref,
            name,
            3, // Win32 namespace
            created,
            modified,
            accessed,
            size,
            size,
            0x20, // FILE_ATTRIBUTE_ARCHIVE
        )?
        .with_empty_data()?
        .build()
}

/// Create a basic directory MFT record
pub fn create_directory_record(
    record_num: u64,
    record_size: u32,
    parent_ref: u64,
    name: &str,
    created: u64,
    modified: u64,
    accessed: u64,
) -> Result<Vec<u8>, MosesError> {
    MftRecordBuilder::new(record_num, record_size)
        .as_directory()
        .with_standard_info(created, modified, accessed, 0x10)? // FILE_ATTRIBUTE_DIRECTORY
        .with_file_name(
            parent_ref,
            name,
            3, // Win32 namespace
            created,
            modified,
            accessed,
            0,
            0,
            0x10, // FILE_ATTRIBUTE_DIRECTORY
        )?
        .with_index_root(ATTR_TYPE_FILE_NAME)?
        .build()
}