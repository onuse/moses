// NTFS data structures
// Based on reverse-engineered NTFS specification

use moses_core::MosesError;

// NTFS signatures
pub const NTFS_SIGNATURE: &[u8; 8] = b"NTFS    ";
pub const MFT_RECORD_SIGNATURE: &[u8; 4] = b"FILE";
pub const MFT_RECORD_BAD_SIGNATURE: &[u8; 4] = b"BAAD";

// Standard MFT record numbers
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

// Attribute type codes
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

// MFT record flags
pub const MFT_RECORD_IN_USE: u16 = 0x0001;
pub const MFT_RECORD_IS_DIRECTORY: u16 = 0x0002;

// File name namespaces
pub const FILE_NAME_POSIX: u8 = 0x00;
pub const FILE_NAME_WIN32: u8 = 0x01;
pub const FILE_NAME_DOS: u8 = 0x02;
pub const FILE_NAME_WIN32_AND_DOS: u8 = 0x03;

/// NTFS Boot Sector structure (512 bytes)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct NtfsBootSector {
    pub jump: [u8; 3],                      // 0x00: Jump instruction
    pub oem_id: [u8; 8],                    // 0x03: "NTFS    "
    pub bytes_per_sector: u16,              // 0x0B: Usually 512
    pub sectors_per_cluster: u8,            // 0x0D: Power of 2, up to 128
    pub reserved_sectors: u16,              // 0x0E: Always 0 for NTFS
    pub zero1: [u8; 3],                     // 0x10: Always 0
    pub unused1: u16,                       // 0x13: Not used by NTFS
    pub media_descriptor: u8,                // 0x15: 0xF8 for hard disk
    pub zero2: u16,                         // 0x16: Always 0
    pub sectors_per_track: u16,             // 0x18: For CHS addressing
    pub num_heads: u16,                     // 0x1A: For CHS addressing
    pub hidden_sectors: u32,                // 0x1C: Sectors before this partition
    pub unused2: u32,                       // 0x20: Not used by NTFS
    pub unused3: u32,                       // 0x24: Not used by NTFS
    pub total_sectors: u64,                 // 0x28: Total sectors in volume
    pub mft_lcn: u64,                       // 0x30: MFT starting cluster
    pub mftmirr_lcn: u64,                   // 0x38: MFT mirror starting cluster
    pub clusters_per_mft_record: i8,        // 0x40: Clusters per MFT record
    pub unused4: [u8; 3],                   // 0x41: Not used
    pub clusters_per_index_buffer: i8,      // 0x44: Clusters per index buffer
    pub unused5: [u8; 3],                   // 0x45: Not used
    pub volume_serial: u64,                 // 0x48: Volume serial number
    pub checksum: u32,                      // 0x50: Not used
    pub bootstrap: [u8; 426],               // 0x54: Bootstrap code
    pub signature: u16,                     // 0x1FE: 0xAA55
}

impl NtfsBootSector {
    /// Validate the boot sector
    pub fn validate(&self) -> Result<(), MosesError> {
        // Check signature (copy to avoid unaligned access)
        let signature = self.signature;
        if signature != 0xAA55 {
            return Err(MosesError::Other("Invalid boot sector signature".to_string()));
        }
        
        // Check OEM ID
        if &self.oem_id != NTFS_SIGNATURE {
            return Err(MosesError::Other("Not an NTFS volume".to_string()));
        }
        
        // Validate bytes per sector (copy to avoid unaligned access)
        let bytes_per_sector = self.bytes_per_sector;
        if ![512, 1024, 2048, 4096].contains(&bytes_per_sector) {
            return Err(MosesError::Other(format!(
                "Invalid bytes per sector: {}",
                bytes_per_sector
            )));
        }
        
        // Validate sectors per cluster (must be power of 2)
        let sectors_per_cluster = self.sectors_per_cluster;
        if sectors_per_cluster == 0 || 
           sectors_per_cluster & (sectors_per_cluster - 1) != 0 {
            return Err(MosesError::Other(format!(
                "Invalid sectors per cluster: {}",
                sectors_per_cluster
            )));
        }
        
        Ok(())
    }
    
    /// Get bytes per cluster
    pub fn bytes_per_cluster(&self) -> u32 {
        let bytes_per_sector = self.bytes_per_sector;
        let sectors_per_cluster = self.sectors_per_cluster;
        bytes_per_sector as u32 * sectors_per_cluster as u32
    }
    
    /// Get MFT record size in bytes
    pub fn mft_record_size(&self) -> u32 {
        let clusters_per_mft_record = self.clusters_per_mft_record;
        if clusters_per_mft_record > 0 {
            // Positive: clusters per record
            clusters_per_mft_record as u32 * self.bytes_per_cluster()
        } else {
            // Negative: 2^|value| bytes
            1u32 << (-clusters_per_mft_record as u32)
        }
    }
    
    /// Get index buffer size in bytes
    pub fn index_buffer_size(&self) -> u32 {
        let clusters_per_index_buffer = self.clusters_per_index_buffer;
        if clusters_per_index_buffer > 0 {
            // Positive: clusters per buffer
            clusters_per_index_buffer as u32 * self.bytes_per_cluster()
        } else {
            // Negative: 2^|value| bytes
            1u32 << (-clusters_per_index_buffer as u32)
        }
    }
}

/// MFT Record Header (first 48 bytes of MFT record)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MftRecordHeader {
    pub signature: [u8; 4],              // 0x00: "FILE" or "BAAD"
    pub usa_offset: u16,                 // 0x04: Update Sequence Array offset
    pub usa_count: u16,                  // 0x06: Update Sequence Array count
    pub lsn: u64,                        // 0x08: $LogFile sequence number
    pub sequence_number: u16,            // 0x10: Sequence value
    pub link_count: u16,                 // 0x12: Hard link count
    pub attrs_offset: u16,               // 0x14: First attribute offset
    pub flags: u16,                      // 0x16: MFT record flags
    pub bytes_used: u32,                 // 0x18: Used size
    pub bytes_allocated: u32,            // 0x1C: Allocated size
    pub base_mft_record: u64,            // 0x20: Base file record
    pub next_attr_id: u16,               // 0x28: Next attribute ID
    pub reserved: u16,                   // 0x2A: Alignment
    pub mft_record_number: u32,          // 0x2C: This record number
}

impl MftRecordHeader {
    /// Check if this is a valid MFT record
    pub fn is_valid(&self) -> bool {
        &self.signature == MFT_RECORD_SIGNATURE || 
        &self.signature == MFT_RECORD_BAD_SIGNATURE
    }
    
    /// Check if record is in use
    pub fn is_in_use(&self) -> bool {
        self.flags & MFT_RECORD_IN_USE != 0
    }
    
    /// Check if record represents a directory
    pub fn is_directory(&self) -> bool {
        self.flags & MFT_RECORD_IS_DIRECTORY != 0
    }
}

/// Common attribute header
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct AttributeHeader {
    pub type_code: u32,                  // 0x00: Attribute type
    pub record_length: u32,              // 0x04: Total length
    pub non_resident: u8,                // 0x08: 0=resident, 1=non-resident
    pub name_length: u8,                 // 0x09: Name length in wide chars
    pub name_offset: u16,                // 0x0A: Offset to name
    pub flags: u16,                      // 0x0C: Attribute flags
    pub attribute_id: u16,               // 0x0E: Unique ID in this MFT record
}

/// Resident attribute header
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ResidentAttributeHeader {
    pub common: AttributeHeader,
    pub value_length: u32,               // 0x10: Data length
    pub value_offset: u16,               // 0x14: Data offset
    pub indexed_flag: u8,                // 0x16: Is indexed
    pub padding: u8,                     // 0x17: Alignment
}

/// Non-resident attribute header
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct NonResidentAttributeHeader {
    pub common: AttributeHeader,
    pub starting_vcn: u64,              // 0x10: Starting Virtual Cluster Number
    pub last_vcn: u64,                  // 0x18: Last VCN
    pub data_runs_offset: u16,          // 0x20: Offset to data runs
    pub compression_unit: u16,           // 0x22: Compression unit size
    pub padding: [u8; 4],                // 0x24: Reserved
    pub allocated_size: u64,             // 0x28: Allocated size
    pub data_size: u64,                 // 0x30: Actual size
    pub initialized_size: u64,          // 0x38: Initialized data size
}

/// Standard Information attribute (0x10)
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct StandardInformation {
    pub creation_time: u64,              // Windows FILETIME
    pub last_modification_time: u64,     // Windows FILETIME
    pub mft_modification_time: u64,      // Windows FILETIME
    pub last_access_time: u64,           // Windows FILETIME
    pub file_attributes: u32,            // Windows file attributes
    pub max_versions: u32,               // Maximum versions (0 = disabled)
    pub version_number: u32,             // Version number
    pub class_id: u32,                   // Class ID
    pub owner_id: u32,                   // Owner ID (NTFS 3.0+)
    pub security_id: u32,                // Security ID (NTFS 3.0+)
    pub quota_charged: u64,              // Quota charged (NTFS 3.0+)
    pub usn: u64,                        // Update Sequence Number (NTFS 3.0+)
}

/// File Name attribute (0x30)
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct FileNameAttr {
    pub parent_reference: u64,           // Parent directory reference
    pub creation_time: u64,              // Windows FILETIME
    pub modification_time: u64,          // Windows FILETIME
    pub mft_modification_time: u64,      // Windows FILETIME
    pub access_time: u64,                // Windows FILETIME
    pub allocated_size: u64,             // Allocated size
    pub data_size: u64,                  // Real size
    pub file_attributes: u32,            // File attributes
    pub ea_size: u32,                    // Extended attributes size
    pub name_length: u8,                 // Filename length in characters
    pub name_type: u8,                   // Filename namespace
    // Followed by: name_length * 2 bytes of Unicode name
}


/// Helper functions for Windows FILETIME conversion
pub fn filetime_to_unix(filetime: u64) -> u64 {
    // Windows FILETIME is 100-nanosecond intervals since 1601-01-01
    // Unix time is seconds since 1970-01-01
    const FILETIME_UNIX_DIFF: u64 = 11644473600; // Seconds between 1601 and 1970
    const FILETIME_TICKS_PER_SECOND: u64 = 10_000_000;
    
    (filetime / FILETIME_TICKS_PER_SECOND).saturating_sub(FILETIME_UNIX_DIFF)
}