# NTFS Technical Specification for Moses Implementation

## Research Sources and References

### Existing Rust Implementations
1. **ntfs crate by ColinFinck** (https://github.com/ColinFinck/ntfs)
   - Low-level, no_std compatible implementation
   - Supports resident/non-resident attributes, sparse files
   - Good reference for attribute parsing

2. **mft crate by omerbenamram** (https://github.com/omerbenamram/mft)
   - MFT parser with JSON/CSV output support
   - 100% safe Rust implementation
   - Good reference for MFT record parsing

3. **ntfs-reader crate**
   - Supports MFT and USN journal reading
   - Good for understanding file enumeration

### Documentation Sources
- NTFS Documentation by Richard Russon and Yuval Fledel
- Linux-NTFS project documentation
- Microsoft Developer documentation
- Digital forensics course materials (UMass)

## 1. NTFS Boot Sector (First 512 bytes)

### Exact Byte Layout
```
Offset  Size  Description
------  ----  -----------
0x00    3     Jump instruction (typically EB 52 90)
0x03    8     OEM ID: "NTFS    " (4 spaces)
--- BIOS Parameter Block (BPB) ---
0x0B    2     Bytes per sector (usually 512)
0x0D    1     Sectors per cluster (1, 2, 4, 8, 16, 32, 64, 128)
0x0E    2     Reserved sectors (always 0 for NTFS)
0x10    3     Always 0
0x13    2     Not used by NTFS (0)
0x15    1     Media descriptor (0xF8 for hard disk)
0x16    2     Always 0
0x18    2     Sectors per track (for CHS)
0x1A    2     Number of heads (for CHS)
0x1C    4     Hidden sectors
0x20    4     Not used by NTFS (0)
--- Extended BPB ---
0x24    4     Not used by NTFS
0x28    8     Total sectors
0x30    8     Logical cluster number for $MFT
0x38    8     Logical cluster number for $MFTMirr
0x40    1     Clusters per MFT record (signed, negative = 2^|value| bytes)
0x41    3     Not used
0x44    1     Clusters per index buffer (signed)
0x45    3     Not used
0x48    8     Volume serial number
0x50    4     Checksum (not used)
--- Bootstrap Code ---
0x54    426   Bootstrap code
--- Signature ---
0x1FE   2     End of sector marker (0x55 0xAA)
```

### Rust Structure
```rust
#[repr(C, packed)]
pub struct NtfsBootSector {
    pub jump: [u8; 3],              // 0x00
    pub oem_id: [u8; 8],            // 0x03
    pub bytes_per_sector: u16,      // 0x0B
    pub sectors_per_cluster: u8,    // 0x0D
    pub reserved_sectors: u16,      // 0x0E
    pub zero1: [u8; 3],             // 0x10
    pub unused1: u16,               // 0x13
    pub media_descriptor: u8,        // 0x15
    pub zero2: u16,                 // 0x16
    pub sectors_per_track: u16,     // 0x18
    pub num_heads: u16,             // 0x1A
    pub hidden_sectors: u32,        // 0x1C
    pub unused2: u32,               // 0x20
    pub unused3: u32,               // 0x24
    pub total_sectors: u64,         // 0x28
    pub mft_lcn: u64,               // 0x30
    pub mftmirr_lcn: u64,           // 0x38
    pub clusters_per_mft_record: i8, // 0x40
    pub unused4: [u8; 3],           // 0x41
    pub clusters_per_index_buffer: i8, // 0x44
    pub unused5: [u8; 3],           // 0x45
    pub volume_serial: u64,         // 0x48
    pub checksum: u32,              // 0x50
    pub bootstrap: [u8; 426],       // 0x54
    pub signature: u16,             // 0x1FE (should be 0xAA55)
}
```

## 2. MFT Record Structure (1024 bytes typical)

### MFT Record Header (First 48 bytes)
```
Offset  Size  Description
------  ----  -----------
0x00    4     Signature "FILE" (0x46494C45) or "BAAD" if corrupt
0x04    2     Offset to fixup array
0x06    2     Number of entries in fixup array
0x08    8     $LogFile sequence number (LSN)
0x10    2     Sequence value
0x12    2     Link count (hard links)
0x14    2     Offset to first attribute
0x16    2     Flags (0x01 = in use, 0x02 = directory)
0x18    4     Used size of MFT entry
0x1C    4     Allocated size of MFT entry (1024)
0x20    8     File reference to base record (0 if base)
0x28    2     Next attribute ID
0x2A    2     Align to 4-byte boundary (XP and above)
0x2C    4     MFT record number (XP and above)
0x30    ...   Update sequence array
```

### Rust Structure
```rust
#[repr(C, packed)]
pub struct MftRecordHeader {
    pub signature: [u8; 4],           // "FILE" or "BAAD"
    pub usa_offset: u16,              // Update Sequence Array offset
    pub usa_count: u16,               // Update Sequence Array count
    pub lsn: u64,                     // $LogFile sequence number
    pub sequence_number: u16,         // Sequence value
    pub link_count: u16,              // Hard link count
    pub attrs_offset: u16,            // First attribute offset
    pub flags: u16,                   // MFT record flags
    pub bytes_used: u32,              // Used size
    pub bytes_allocated: u32,         // Allocated size
    pub base_mft_record: u64,         // Base file record
    pub next_attr_id: u16,            // Next attribute ID
    pub reserved: u16,                // Alignment
    pub mft_record_number: u32,       // This record number
}
```

### MFT Record Flags
```rust
pub const MFT_RECORD_IN_USE: u16 = 0x0001;
pub const MFT_RECORD_IS_DIRECTORY: u16 = 0x0002;
pub const MFT_RECORD_IS_4: u16 = 0x0004;
pub const MFT_RECORD_IS_VIEW_INDEX: u16 = 0x0008;
```

## 3. Attribute Header Structures

### Common Attribute Header (Resident and Non-Resident)
```
Offset  Size  Description
------  ----  -----------
0x00    4     Attribute type
0x04    4     Length (including header)
0x08    1     Non-resident flag (0 = resident, 1 = non-resident)
0x09    1     Name length (0 if no name)
0x0A    2     Offset to name
0x0C    2     Flags (compressed, encrypted, sparse)
0x0E    2     Attribute ID
```

### Resident Attribute Header Extension
```
0x10    4     Value length
0x14    2     Value offset
0x16    2     Indexed flag
```

### Non-Resident Attribute Header Extension
```
0x10    8     Starting VCN (Virtual Cluster Number)
0x18    8     Last VCN
0x20    2     Offset to data runs
0x22    2     Compression unit size (0 = not compressed)
0x24    4     Padding
0x28    8     Allocated size (multiple of cluster size)
0x30    8     Real size
0x38    8     Initialized data size
0x40    ...   Data runs (if offset 0x20 points here)
```

### Rust Structures
```rust
#[repr(C, packed)]
pub struct AttributeHeader {
    pub type_code: u32,              // Attribute type
    pub record_length: u32,           // Total length
    pub non_resident: u8,             // 0 or 1
    pub name_length: u8,              // Name length in wide chars
    pub name_offset: u16,             // Offset to name
    pub flags: u16,                   // Attribute flags
    pub attribute_id: u16,            // Unique ID in this MFT record
}

#[repr(C, packed)]
pub struct ResidentAttributeHeader {
    pub common: AttributeHeader,
    pub value_length: u32,            // Data length
    pub value_offset: u16,            // Data offset
    pub indexed_flag: u8,             // Is indexed
    pub padding: u8,                  // Alignment
}

#[repr(C, packed)]
pub struct NonResidentAttributeHeader {
    pub common: AttributeHeader,
    pub starting_vcn: u64,            // Starting Virtual Cluster Number
    pub last_vcn: u64,                // Last VCN
    pub data_runs_offset: u16,        // Offset to data runs
    pub compression_unit: u16,         // Compression unit size
    pub padding: [u8; 4],             // Reserved
    pub allocated_size: u64,          // Allocated size
    pub data_size: u64,               // Actual size
    pub initialized_size: u64,        // Initialized data size
    pub compressed_size: u64,         // Only when compressed
}
```

## 4. Attribute Type Codes
```rust
pub const ATTR_TYPE_STANDARD_INFORMATION: u32 = 0x10;
pub const ATTR_TYPE_ATTRIBUTE_LIST: u32 = 0x20;
pub const ATTR_TYPE_FILE_NAME: u32 = 0x30;
pub const ATTR_TYPE_OBJECT_ID: u32 = 0x40;
pub const ATTR_TYPE_SECURITY_DESCRIPTOR: u32 = 0x50;
pub const ATTR_TYPE_VOLUME_NAME: u32 = 0x60;
pub const ATTR_TYPE_VOLUME_INFORMATION: u32 = 0x70;
pub const ATTR_TYPE_DATA: u32 = 0x80;
pub const ATTR_TYPE_INDEX_ROOT: u32 = 0x90;
pub const ATTR_TYPE_INDEX_ALLOCATION: u32 = 0xA0;
pub const ATTR_TYPE_BITMAP: u32 = 0xB0;
pub const ATTR_TYPE_REPARSE_POINT: u32 = 0xC0;
pub const ATTR_TYPE_EA_INFORMATION: u32 = 0xD0;
pub const ATTR_TYPE_EA: u32 = 0xE0;
pub const ATTR_TYPE_LOGGED_UTILITY_STREAM: u32 = 0x100;
pub const ATTR_TYPE_END: u32 = 0xFFFFFFFF;
```

## 5. Data Run Encoding

### Format
Data runs use a compact variable-length encoding:
```
[Header Byte][Length Bytes][Offset Bytes]
```

### Header Byte Structure
```
Bits 0-3: Number of bytes in length field (L)
Bits 4-7: Number of bytes in offset field (O)
```

### Decoding Algorithm
```rust
pub fn decode_data_run(data: &[u8]) -> Result<Vec<DataRun>, Error> {
    let mut runs = Vec::new();
    let mut pos = 0;
    let mut prev_lcn = 0i64;
    
    while pos < data.len() {
        let header = data[pos];
        if header == 0 {
            break; // End marker
        }
        
        let length_size = (header & 0x0F) as usize;
        let offset_size = ((header >> 4) & 0x0F) as usize;
        pos += 1;
        
        // Read run length (in clusters)
        let length = read_le_bytes(&data[pos..pos + length_size]);
        pos += length_size;
        
        if offset_size == 0 {
            // Sparse run
            runs.push(DataRun {
                lcn: None,
                length,
            });
        } else {
            // Read offset (signed, relative to previous)
            let offset = read_le_bytes_signed(&data[pos..pos + offset_size]);
            pos += offset_size;
            
            let lcn = prev_lcn + offset;
            prev_lcn = lcn;
            
            runs.push(DataRun {
                lcn: Some(lcn as u64),
                length,
            });
        }
    }
    
    Ok(runs)
}
```

## 6. Standard Attribute Structures

### $STANDARD_INFORMATION (0x10)
```rust
#[repr(C, packed)]
pub struct StandardInformation {
    pub creation_time: u64,          // Windows FILETIME
    pub last_modification_time: u64, // Windows FILETIME
    pub mft_modification_time: u64,  // Windows FILETIME
    pub last_access_time: u64,       // Windows FILETIME
    pub file_attributes: u32,        // Windows file attributes
    pub max_versions: u32,            // Maximum versions (0 = disabled)
    pub version_number: u32,          // Version number
    pub class_id: u32,                // Class ID
    pub owner_id: u32,                // Owner ID (NTFS 3.0+)
    pub security_id: u32,             // Security ID (NTFS 3.0+)
    pub quota_charged: u64,           // Quota charged (NTFS 3.0+)
    pub usn: u64,                     // Update Sequence Number (NTFS 3.0+)
}
```

### $FILE_NAME (0x30)
```rust
#[repr(C, packed)]
pub struct FileName {
    pub parent_reference: u64,       // Parent directory reference
    pub creation_time: u64,          // Windows FILETIME
    pub modification_time: u64,      // Windows FILETIME
    pub mft_modification_time: u64,  // Windows FILETIME
    pub access_time: u64,            // Windows FILETIME
    pub allocated_size: u64,         // Allocated size
    pub data_size: u64,              // Real size
    pub file_attributes: u32,        // File attributes
    pub ea_size: u32,                // Extended attributes size
    pub name_length: u8,             // Filename length in characters
    pub name_type: u8,               // Filename namespace
    // Followed by: name_length * 2 bytes of Unicode name
}

pub const FILE_NAME_POSIX: u8 = 0x00;
pub const FILE_NAME_WIN32: u8 = 0x01;
pub const FILE_NAME_DOS: u8 = 0x02;
pub const FILE_NAME_WIN32_AND_DOS: u8 = 0x03;
```

## 7. Update Sequence Array (USA) / Fixup

MFT records and index buffers use fixups to detect torn writes:
```rust
pub fn apply_fixup(buffer: &mut [u8], usa_offset: u16, usa_count: u16) -> Result<(), Error> {
    let usa_offset = usa_offset as usize;
    let usa_count = usa_count as usize;
    
    // First 2 bytes are the update sequence number
    let usn = &buffer[usa_offset..usa_offset + 2];
    
    // Apply fixup to each sector
    for i in 1..usa_count {
        let usa_value = &buffer[usa_offset + i * 2..usa_offset + i * 2 + 2];
        let sector_offset = i * 512 - 2; // Last 2 bytes of each sector
        
        // Check that the sector ends with the USN
        if &buffer[sector_offset..sector_offset + 2] != usn {
            return Err(Error::FixupMismatch);
        }
        
        // Replace with original value
        buffer[sector_offset..sector_offset + 2].copy_from_slice(usa_value);
    }
    
    Ok(())
}
```

## 8. Directory Index Structure

### $INDEX_ROOT Attribute
```rust
#[repr(C, packed)]
pub struct IndexRoot {
    pub type_: u32,                  // Type of indexed attribute (0x30 for filenames)
    pub collation_rule: u32,         // Collation rule
    pub index_size: u32,             // Size of index allocation
    pub clusters_per_index: u8,      // Clusters per index record
    pub padding: [u8; 3],            // Alignment
    // Followed by INDEX_HEADER
}

#[repr(C, packed)]
pub struct IndexHeader {
    pub entries_offset: u32,         // Offset to first entry
    pub index_length: u32,           // Total length of entries
    pub allocated_size: u32,         // Allocated size
    pub flags: u32,                  // 0x00 = small index, 0x01 = large index
}

#[repr(C, packed)]
pub struct IndexEntry {
    pub file_reference: u64,         // MFT reference
    pub length: u16,                 // Length of this entry
    pub stream_length: u16,           // Length of stream
    pub flags: u32,                  // 0x01 = has sub-node, 0x02 = last entry
    // Followed by stream (usually FILE_NAME attribute)
    // If has sub-node: 8-byte VCN at end
}
```

## 9. Special MFT Records

```rust
pub const MFT_RECORD_MFT: u64 = 0;        // $MFT
pub const MFT_RECORD_MFTMIRR: u64 = 1;    // $MFTMirr
pub const MFT_RECORD_LOGFILE: u64 = 2;    // $LogFile
pub const MFT_RECORD_VOLUME: u64 = 3;     // $Volume
pub const MFT_RECORD_ATTRDEF: u64 = 4;    // $AttrDef
pub const MFT_RECORD_ROOT: u64 = 5;       // . (root directory)
pub const MFT_RECORD_BITMAP: u64 = 6;     // $Bitmap
pub const MFT_RECORD_BOOT: u64 = 7;       // $Boot
pub const MFT_RECORD_BADCLUS: u64 = 8;    // $BadClus
pub const MFT_RECORD_SECURE: u64 = 9;     // $Secure
pub const MFT_RECORD_UPCASE: u64 = 10;    // $UpCase
pub const MFT_RECORD_EXTEND: u64 = 11;    // $Extend
pub const MFT_RECORD_RESERVED_12_15: u64 = 12; // Reserved
pub const MFT_RECORD_FIRST_USER: u64 = 16; // First user file
```

## Implementation Notes

### Critical Implementation Details

1. **Fixups/Update Sequence Arrays**: Must be applied before parsing any MFT record or index buffer
2. **Attribute Lists**: Can span multiple MFT records - must handle properly
3. **Compressed Clusters**: 16-cluster compression units, LZNT1 algorithm
4. **Sparse Files**: Data runs with offset_size = 0 represent holes
5. **Unicode Handling**: All names are UTF-16LE, use $UpCase for case-insensitive comparison
6. **Timestamps**: Windows FILETIME format (100-nanosecond intervals since 1601-01-01)

### Recommended Implementation Order

1. **Parse Boot Sector** - Get cluster size, MFT location
2. **Read MFT Record 0** - The MFT itself, get its data runs
3. **Implement Fixup/USA** - Required for all MFT records
4. **Parse Basic Attributes** - STANDARD_INFO, FILE_NAME, DATA (resident only)
5. **Implement Data Runs** - For non-resident DATA attributes
6. **Read Root Directory** - MFT record 5, parse INDEX_ROOT
7. **Navigate Directories** - Follow INDEX_ALLOCATION for large dirs
8. **Read Files** - Combine resident/non-resident data reading

### Testing Strategy

1. Create test NTFS images with known content
2. Compare output with Windows dir/fsutil commands
3. Test edge cases: sparse files, compressed files, hard links
4. Validate against existing tools (NTFS-3G, ntfsprogs)
5. Fuzz testing with malformed structures

This specification provides the exact byte-level structures needed to implement NTFS support. The existing Rust crates (especially `ntfs` by ColinFinck) can serve as excellent references for the implementation.