// Common FAT filesystem structures used by FAT16, FAT32, and exFAT
// This consolidates all the duplicate struct definitions

use std::fmt;

// ============================================================================
// Common BPB (BIOS Parameter Block) for FAT16/FAT32
// ============================================================================

/// Common FAT Boot Sector structure (first 36 bytes are identical for FAT16/FAT32)
#[repr(C, packed(1))]
#[derive(Debug, Clone, Copy)]
pub struct FatCommonBpb {
    pub jump_boot: [u8; 3],         // 0x00: Jump instruction
    pub oem_name: [u8; 8],          // 0x03: OEM name
    pub bytes_per_sector: u16,      // 0x0B: Bytes per sector (usually 512)
    pub sectors_per_cluster: u8,    // 0x0D: Sectors per cluster
    pub reserved_sectors: u16,      // 0x0E: Reserved sectors
    pub num_fats: u8,              // 0x10: Number of FATs (usually 2)
    pub root_entries: u16,         // 0x11: Root entries (0 for FAT32)
    pub total_sectors_16: u16,     // 0x13: Total sectors if < 65536
    pub media_descriptor: u8,      // 0x15: Media descriptor
    pub sectors_per_fat_16: u16,   // 0x16: Sectors per FAT (FAT16 only, 0 for FAT32)
    pub sectors_per_track: u16,    // 0x18: Sectors per track
    pub num_heads: u16,            // 0x1A: Number of heads
    pub hidden_sectors: u32,       // 0x1C: Hidden sectors
    pub total_sectors_32: u32,     // 0x20: Total sectors if >= 65536
}

/// FAT16 Extended BPB (follows common BPB)
#[repr(C, packed(1))]
#[derive(Debug, Clone, Copy)]
pub struct Fat16ExtendedBpb {
    pub drive_number: u8,          // 0x24: BIOS drive number
    pub reserved: u8,              // 0x25: Reserved
    pub boot_signature: u8,        // 0x26: Extended boot signature (0x29)
    pub volume_id: u32,            // 0x27: Volume serial number
    pub volume_label: [u8; 11],    // 0x2B: Volume label
    pub fs_type: [u8; 8],          // 0x36: File system type "FAT16   "
}

/// FAT32 Extended BPB (follows common BPB)
#[repr(C, packed(1))]
#[derive(Debug, Clone, Copy)]
pub struct Fat32ExtendedBpb {
    pub sectors_per_fat_32: u32,   // 0x24: Sectors per FAT for FAT32
    pub ext_flags: u16,            // 0x28: Extended flags
    pub fs_version: u16,           // 0x2A: Filesystem version
    pub root_cluster: u32,         // 0x2C: Root directory cluster
    pub fs_info: u16,              // 0x30: FSInfo structure sector
    pub backup_boot_sector: u16,   // 0x32: Backup boot sector location
    pub reserved: [u8; 12],        // 0x34: Reserved
    pub drive_number: u8,          // 0x40: BIOS drive number
    pub reserved1: u8,             // 0x41: Reserved
    pub boot_signature: u8,        // 0x42: Extended boot signature (0x29)
    pub volume_id: u32,            // 0x43: Volume serial number
    pub volume_label: [u8; 11],    // 0x47: Volume label
    pub fs_type: [u8; 8],          // 0x52: File system type "FAT32   "
}

/// Complete FAT16 Boot Sector
#[repr(C, packed(1))]
#[derive(Clone, Copy)]
pub struct Fat16BootSector {
    pub common_bpb: FatCommonBpb,
    pub extended_bpb: Fat16ExtendedBpb,
    pub boot_code: [u8; 448],      // Boot code
    pub boot_signature: u16,       // 0x55AA
}

/// Complete FAT32 Boot Sector
#[repr(C, packed(1))]
#[derive(Clone, Copy)]
pub struct Fat32BootSector {
    pub common_bpb: FatCommonBpb,
    pub extended_bpb: Fat32ExtendedBpb,
    pub boot_code: [u8; 420],      // Boot code (smaller due to larger extended BPB)
    pub boot_signature: u16,       // 0x55AA
}

// ============================================================================
// Directory Entry Structures (shared by FAT16/FAT32)
// ============================================================================

/// FAT Directory Entry Attributes
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FatAttributes(pub u8);

impl FatAttributes {
    pub const READ_ONLY: u8 = 0x01;
    pub const HIDDEN: u8 = 0x02;
    pub const SYSTEM: u8 = 0x04;
    pub const VOLUME_ID: u8 = 0x08;
    pub const DIRECTORY: u8 = 0x10;
    pub const ARCHIVE: u8 = 0x20;
    pub const LFN: u8 = 0x0F;  // Long filename entry
    
    pub fn is_read_only(&self) -> bool { self.0 & Self::READ_ONLY != 0 }
    pub fn is_hidden(&self) -> bool { self.0 & Self::HIDDEN != 0 }
    pub fn is_system(&self) -> bool { self.0 & Self::SYSTEM != 0 }
    pub fn is_volume_id(&self) -> bool { self.0 & Self::VOLUME_ID != 0 }
    pub fn is_directory(&self) -> bool { self.0 & Self::DIRECTORY != 0 }
    pub fn is_archive(&self) -> bool { self.0 & Self::ARCHIVE != 0 }
    pub fn is_lfn(&self) -> bool { self.0 == Self::LFN }
}

/// FAT Directory Entry (32 bytes)
#[repr(C, packed(1))]
#[derive(Debug, Clone, Copy)]
pub struct FatDirEntry {
    pub name: [u8; 11],            // Short filename (8.3 format)
    pub attributes: u8,            // File attributes
    pub nt_reserved: u8,           // Reserved for Windows NT
    pub creation_time_tenth: u8,   // Creation time in tenths of second
    pub creation_time: u16,        // Creation time
    pub creation_date: u16,        // Creation date
    pub last_access_date: u16,     // Last access date
    pub first_cluster_high: u16,   // High 16 bits of first cluster (FAT32)
    pub write_time: u16,           // Last write time
    pub write_date: u16,           // Last write date
    pub first_cluster_low: u16,    // Low 16 bits of first cluster
    pub file_size: u32,            // File size in bytes
}

impl FatDirEntry {
    /// Get the first cluster number (handles both FAT16 and FAT32)
    pub fn first_cluster(&self) -> u32 {
        ((self.first_cluster_high as u32) << 16) | (self.first_cluster_low as u32)
    }
    
    /// Set the first cluster number (handles both FAT16 and FAT32)
    pub fn set_first_cluster(&mut self, cluster: u32) {
        self.first_cluster_low = (cluster & 0xFFFF) as u16;
        self.first_cluster_high = ((cluster >> 16) & 0xFFFF) as u16;
    }
    
    /// Check if this is a valid entry
    pub fn is_valid(&self) -> bool {
        self.name[0] != 0x00 && self.name[0] != 0xE5
    }
    
    /// Check if this entry is deleted
    pub fn is_deleted(&self) -> bool {
        self.name[0] == 0xE5
    }
    
    /// Check if this is the end of directory
    pub fn is_end(&self) -> bool {
        self.name[0] == 0x00
    }
}

// ============================================================================
// Helper Methods
// ============================================================================

impl FatCommonBpb {
    /// Calculate total sectors from BPB
    pub fn total_sectors(&self) -> u64 {
        if self.total_sectors_16 != 0 {
            self.total_sectors_16 as u64
        } else {
            self.total_sectors_32 as u64
        }
    }
    
    /// Determine if this is removable media
    pub fn is_removable(&self) -> bool {
        self.media_descriptor == 0xF0
    }
    
    /// Validate basic BPB fields
    pub fn validate(&self) -> Result<(), String> {
        // Copy values to avoid packed struct alignment issues
        let jump_boot_0 = self.jump_boot[0];
        let bytes_per_sector = self.bytes_per_sector;
        let sectors_per_cluster = self.sectors_per_cluster;
        let num_fats = self.num_fats;
        
        // Check jump instruction
        if jump_boot_0 != 0xEB && jump_boot_0 != 0xE9 {
            return Err(format!("Invalid jump instruction: 0x{:02X}", jump_boot_0));
        }
        
        // Check bytes per sector
        if ![512, 1024, 2048, 4096].contains(&bytes_per_sector) {
            return Err(format!("Invalid bytes per sector: {}", bytes_per_sector));
        }
        
        // Check sectors per cluster
        if !sectors_per_cluster.is_power_of_two() {
            return Err(format!("Sectors per cluster not power of 2: {}", sectors_per_cluster));
        }
        
        // Check number of FATs
        if num_fats == 0 {
            return Err("Number of FATs cannot be 0".to_string());
        }
        
        Ok(())
    }
}

impl Fat16BootSector {
    /// Create a properly initialized FAT16 boot sector
    pub fn new() -> Self {
        let boot = Self {
            common_bpb: FatCommonBpb {
                jump_boot: [0xEB, 0x3C, 0x90],
                oem_name: *b"MOSES   ",
                bytes_per_sector: 512,
                sectors_per_cluster: 0,  // Must be set
                reserved_sectors: 1,
                num_fats: 2,
                root_entries: 512,
                total_sectors_16: 0,
                media_descriptor: 0xF8,
                sectors_per_fat_16: 0,  // Must be calculated
                sectors_per_track: 63,
                num_heads: 255,
                hidden_sectors: 0,
                total_sectors_32: 0,
            },
            extended_bpb: Fat16ExtendedBpb {
                drive_number: 0x80,
                reserved: 0,
                boot_signature: 0x29,
                volume_id: 0,
                volume_label: [0x20; 11],
                fs_type: *b"FAT16   ",
            },
            boot_code: [0; 448],
            boot_signature: 0xAA55,
        };
        boot
    }
}

impl Fat32BootSector {
    /// Create a properly initialized FAT32 boot sector
    pub fn new() -> Self {
        let boot = Self {
            common_bpb: FatCommonBpb {
                jump_boot: [0xEB, 0x58, 0x90],
                oem_name: *b"MOSES   ",
                bytes_per_sector: 512,
                sectors_per_cluster: 0,  // Must be set
                reserved_sectors: 32,  // FAT32 typically uses 32
                num_fats: 2,
                root_entries: 0,  // FAT32 has no fixed root
                total_sectors_16: 0,
                media_descriptor: 0xF8,
                sectors_per_fat_16: 0,  // Not used in FAT32
                sectors_per_track: 63,
                num_heads: 255,
                hidden_sectors: 0,
                total_sectors_32: 0,
            },
            extended_bpb: Fat32ExtendedBpb {
                sectors_per_fat_32: 0,  // Must be calculated
                ext_flags: 0,
                fs_version: 0,
                root_cluster: 2,  // Usually starts at cluster 2
                fs_info: 1,
                backup_boot_sector: 6,
                reserved: [0; 12],
                drive_number: 0x80,
                reserved1: 0,
                boot_signature: 0x29,
                volume_id: 0,
                volume_label: [0x20; 11],
                fs_type: *b"FAT32   ",
            },
            boot_code: [0; 420],
            boot_signature: 0xAA55,
        };
        boot
    }
}

// Implement Debug manually to avoid packed struct issues
impl fmt::Debug for Fat16BootSector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Copy values to avoid packed struct alignment issues
        let bytes_per_sector = self.common_bpb.bytes_per_sector;
        let sectors_per_cluster = self.common_bpb.sectors_per_cluster;
        let total_sectors = self.common_bpb.total_sectors();
        
        f.debug_struct("Fat16BootSector")
            .field("oem_name", &String::from_utf8_lossy(&self.common_bpb.oem_name))
            .field("bytes_per_sector", &bytes_per_sector)
            .field("sectors_per_cluster", &sectors_per_cluster)
            .field("total_sectors", &total_sectors)
            .field("volume_label", &String::from_utf8_lossy(&self.extended_bpb.volume_label))
            .finish()
    }
}

impl fmt::Debug for Fat32BootSector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Copy values to avoid packed struct alignment issues
        let bytes_per_sector = self.common_bpb.bytes_per_sector;
        let sectors_per_cluster = self.common_bpb.sectors_per_cluster;
        let total_sectors = self.common_bpb.total_sectors();
        let root_cluster = self.extended_bpb.root_cluster;
        
        f.debug_struct("Fat32BootSector")
            .field("oem_name", &String::from_utf8_lossy(&self.common_bpb.oem_name))
            .field("bytes_per_sector", &bytes_per_sector)
            .field("sectors_per_cluster", &sectors_per_cluster)
            .field("total_sectors", &total_sectors)
            .field("root_cluster", &root_cluster)
            .field("volume_label", &String::from_utf8_lossy(&self.extended_bpb.volume_label))
            .finish()
    }
}