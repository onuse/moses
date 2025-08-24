// exFAT filesystem structures
// Based on Microsoft exFAT specification

// Entry type definitions
pub const EXFAT_ENTRY_BITMAP: u8 = 0x81;
pub const EXFAT_ENTRY_UPCASE: u8 = 0x82;
pub const EXFAT_ENTRY_VOLUME_LABEL: u8 = 0x83;
pub const EXFAT_ENTRY_FILE: u8 = 0x85;
pub const EXFAT_ENTRY_VOLUME_GUID: u8 = 0xA0;
pub const EXFAT_ENTRY_STREAM: u8 = 0xC0;
pub const EXFAT_ENTRY_FILE_NAME: u8 = 0xC1;

/// exFAT boot sector structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatBootSector {
    pub jump_boot: [u8; 3],
    pub file_system_name: [u8; 8],  // "EXFAT   "
    pub must_be_zero: [u8; 53],
    pub partition_offset: u64,
    pub volume_length: u64,
    pub fat_offset: u32,
    pub fat_length: u32,
    pub cluster_heap_offset: u32,
    pub cluster_count: u32,
    pub first_cluster_of_root: u32,
    pub volume_serial_number: u32,
    pub file_system_revision: u16,
    pub volume_flags: u16,
    pub bytes_per_sector_shift: u8,
    pub sectors_per_cluster_shift: u8,
    pub number_of_fats: u8,
    pub drive_select: u8,
    pub percent_in_use: u8,
    pub reserved: [u8; 7],
    pub boot_code: [u8; 390],
    pub boot_signature: [u8; 2],  // 0x55, 0xAA
}

/// exFAT directory entry (32 bytes)
#[repr(C)]
#[derive(Clone, Copy)]
pub union ExFatDirectoryEntry {
    pub generic: ExFatGenericEntry,
    pub bitmap: ExFatBitmapEntry,
    pub upcase: ExFatUpcaseEntry,
    pub label: ExFatVolumeLabelEntry,
    pub file: ExFatFileEntry,
    pub stream: ExFatStreamEntry,
    pub filename: ExFatFileNameEntry,
    pub volume_guid: ExFatVolumeGuidEntry,
    pub raw: [u8; 32],
}

impl Default for ExFatDirectoryEntry {
    fn default() -> Self {
        Self { raw: [0u8; 32] }
    }
}

impl ExFatDirectoryEntry {
    pub fn to_bytes(&self) -> [u8; 32] {
        unsafe { self.raw }
    }
    
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { raw: bytes }
    }
    
    pub fn entry_type(&self) -> u8 {
        unsafe { self.generic.entry_type }
    }
}

/// Generic directory entry header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatGenericEntry {
    pub entry_type: u8,
    pub custom_defined: [u8; 19],
    pub first_cluster: u32,
    pub data_length: u64,
}

/// Allocation bitmap directory entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatBitmapEntry {
    pub entry_type: u8,  // 0x81
    pub flags: u8,
    pub reserved: [u8; 18],
    pub first_cluster: u32,
    pub data_length: u64,
}

/// Upcase table directory entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatUpcaseEntry {
    pub entry_type: u8,  // 0x82
    pub reserved1: [u8; 3],
    pub table_checksum: u32,
    pub reserved2: [u8; 12],
    pub first_cluster: u32,
    pub data_length: u64,
}

/// Volume label directory entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatVolumeLabelEntry {
    pub entry_type: u8,  // 0x83
    pub character_count: u8,  // Number of Unicode characters (max 11)
    pub volume_label: [u16; 11],  // UTF-16LE
    pub reserved: [u8; 8],
}

/// File directory entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatFileEntry {
    pub entry_type: u8,  // 0x85
    pub secondary_count: u8,  // Number of secondary entries
    pub set_checksum: u16,
    pub file_attributes: u16,
    pub reserved1: u16,
    pub create_timestamp: u32,
    pub last_modified_timestamp: u32,
    pub last_accessed_timestamp: u32,
    pub create_10ms_increment: u8,
    pub last_modified_10ms_increment: u8,
    pub create_tz_offset: u8,
    pub last_modified_tz_offset: u8,
    pub last_accessed_tz_offset: u8,
    pub reserved2: [u8; 7],
}

/// Stream extension directory entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatStreamEntry {
    pub entry_type: u8,  // 0xC0
    pub flags: u8,
    pub reserved1: u8,
    pub name_length: u8,  // Length of filename in Unicode chars
    pub name_hash: u16,
    pub reserved2: u16,
    pub valid_data_length: u64,
    pub reserved3: u32,
    pub first_cluster: u32,
    pub data_length: u64,
}

/// File name directory entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatFileNameEntry {
    pub entry_type: u8,  // 0xC1
    pub flags: u8,
    pub file_name: [u16; 15],  // UTF-16LE, up to 15 chars per entry
}

/// Volume GUID directory entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatVolumeGuidEntry {
    pub entry_type: u8,  // 0xA0
    pub secondary_count: u8,  // Always 0
    pub set_checksum: u16,
    pub flags: u16,
    pub volume_guid: [u8; 16],  // GUID (UUID)
    pub reserved: [u8; 10],
}

// File attributes
pub const EXFAT_ATTR_READ_ONLY: u16 = 0x0001;
pub const EXFAT_ATTR_HIDDEN: u16 = 0x0002;
pub const EXFAT_ATTR_SYSTEM: u16 = 0x0004;
pub const EXFAT_ATTR_DIRECTORY: u16 = 0x0010;
pub const EXFAT_ATTR_ARCHIVE: u16 = 0x0020;

// Volume flags
pub const EXFAT_VOLUME_FLAG_ACTIVE_FAT: u16 = 0x0001;  // 0 = first FAT, 1 = second FAT
pub const EXFAT_VOLUME_FLAG_DIRTY: u16 = 0x0002;       // 0 = clean, 1 = dirty
pub const EXFAT_VOLUME_FLAG_MEDIA_FAILURE: u16 = 0x0004; // 0 = no failures, 1 = failures

/// Helper to manage exFAT volume flags
#[derive(Debug, Clone, Copy)]
pub struct ExFatVolumeFlags(u16);

impl ExFatVolumeFlags {
    pub fn new() -> Self {
        Self(0)
    }
    
    pub fn set_active_fat(&mut self, use_second: bool) {
        if use_second {
            self.0 |= EXFAT_VOLUME_FLAG_ACTIVE_FAT;
        } else {
            self.0 &= !EXFAT_VOLUME_FLAG_ACTIVE_FAT;
        }
    }
    
    pub fn set_dirty(&mut self, dirty: bool) {
        if dirty {
            self.0 |= EXFAT_VOLUME_FLAG_DIRTY;
        } else {
            self.0 &= !EXFAT_VOLUME_FLAG_DIRTY;
        }
    }
    
    pub fn set_media_failure(&mut self, failure: bool) {
        if failure {
            self.0 |= EXFAT_VOLUME_FLAG_MEDIA_FAILURE;
        } else {
            self.0 &= !EXFAT_VOLUME_FLAG_MEDIA_FAILURE;
        }
    }
    
    pub fn to_u16(self) -> u16 {
        self.0
    }
}

impl Default for ExFatVolumeFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate checksum for a directory entry set
pub fn calculate_entry_set_checksum(entries: &[ExFatDirectoryEntry]) -> u16 {
    let mut checksum: u16 = 0;
    
    for (i, entry) in entries.iter().enumerate() {
        let bytes = entry.to_bytes();
        for (j, &byte) in bytes.iter().enumerate() {
            // Skip the checksum field itself (bytes 2-3 of first entry)
            if i == 0 && (j == 2 || j == 3) {
                continue;
            }
            checksum = ((checksum << 15) | (checksum >> 1)) + byte as u16;
        }
    }
    
    checksum
}

/// Create a timestamp from system time
pub fn create_exfat_timestamp() -> u32 {
    use crate::fat_common::timestamps::ExFatTimestamp;
    let ts = ExFatTimestamp::now();
    (ts.timestamp / 10_000_000) as u32  // Convert to seconds
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_entry_size() {
        assert_eq!(std::mem::size_of::<ExFatDirectoryEntry>(), 32);
        assert_eq!(std::mem::size_of::<ExFatBootSector>(), 512);
    }
    
    #[test]
    fn test_entry_type() {
        let mut entry = ExFatDirectoryEntry::default();
        unsafe { entry.generic.entry_type = EXFAT_ENTRY_FILE; }
        assert_eq!(entry.entry_type(), EXFAT_ENTRY_FILE);
    }
}