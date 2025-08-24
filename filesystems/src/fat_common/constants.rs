// FAT filesystem constants shared between FAT16 and FAT32

// Boot sector offsets
pub const BS_JMP_BOOT: usize = 0x00;
pub const BS_OEM_NAME: usize = 0x03;
pub const BPB_BYTES_PER_SEC: usize = 0x0B;
pub const BPB_SEC_PER_CLUS: usize = 0x0D;
pub const BPB_RSVD_SEC_CNT: usize = 0x0E;
pub const BPB_NUM_FATS: usize = 0x10;
pub const BPB_ROOT_ENT_CNT: usize = 0x11;
pub const BPB_TOT_SEC16: usize = 0x13;
pub const BPB_MEDIA: usize = 0x15;
pub const BPB_FAT_SZ16: usize = 0x16;
pub const BPB_SEC_PER_TRK: usize = 0x18;
pub const BPB_NUM_HEADS: usize = 0x1A;
pub const BPB_HIDD_SEC: usize = 0x1C;
pub const BPB_TOT_SEC32: usize = 0x20;

// FAT16-specific offsets (start at 36)
pub const BS16_DRV_NUM: usize = 0x24;
pub const BS16_RESERVED1: usize = 0x25;
pub const BS16_BOOT_SIG: usize = 0x26;
pub const BS16_VOL_ID: usize = 0x27;
pub const BS16_VOL_LAB: usize = 0x2B;
pub const BS16_FIL_SYS_TYPE: usize = 0x36;

// FAT32-specific offsets (start at 36)
pub const BPB_FAT_SZ32: usize = 0x24;
pub const BPB_EXT_FLAGS: usize = 0x28;
pub const BPB_FS_VER: usize = 0x2A;
pub const BPB_ROOT_CLUS: usize = 0x2C;
pub const BPB_FS_INFO: usize = 0x30;
pub const BPB_BK_BOOT_SEC: usize = 0x32;
pub const BPB_RESERVED: usize = 0x34;
pub const BS32_DRV_NUM: usize = 0x40;
pub const BS32_RESERVED1: usize = 0x41;
pub const BS32_BOOT_SIG: usize = 0x42;
pub const BS32_VOL_ID: usize = 0x43;
pub const BS32_VOL_LAB: usize = 0x47;
pub const BS32_FIL_SYS_TYPE: usize = 0x52;

// Boot sector signature
pub const BOOT_SIGNATURE: [u8; 2] = [0x55, 0xAA];
pub const BOOT_SIGNATURE_OFFSET: usize = 0x1FE;

// FAT entry values
pub const FAT16_EOC: u16 = 0xFFF8;  // End of chain marker
pub const FAT16_BAD: u16 = 0xFFF7;  // Bad cluster marker
pub const FAT32_EOC: u32 = 0x0FFFFFF8;  // End of chain marker (28 bits)
pub const FAT32_BAD: u32 = 0x0FFFFFF7;  // Bad cluster marker (28 bits)

// Cluster count thresholds
pub const FAT12_MAX_CLUSTERS: u32 = 4084;
pub const FAT16_MIN_CLUSTERS: u32 = 4085;
pub const FAT16_MAX_CLUSTERS: u32 = 65524;
pub const FAT32_MIN_CLUSTERS: u32 = 65525;

// Standard values
pub const STANDARD_BYTES_PER_SECTOR: u16 = 512;
pub const FAT32_ROOT_CLUSTER: u32 = 2;  // Standard root directory cluster for FAT32
pub const FAT32_FS_INFO_SECTOR: u16 = 1;  // FSInfo sector location
pub const FAT32_BACKUP_BOOT_SECTOR: u16 = 6;  // Backup boot sector location

// Media descriptors
pub const MEDIA_FIXED: u8 = 0xF8;  // Fixed disk
pub const MEDIA_REMOVABLE: u8 = 0xF0;  // Removable media

// Partition type codes for MBR
pub const PARTITION_TYPE_FAT16_SMALL: u8 = 0x04;  // FAT16 < 32MB
pub const PARTITION_TYPE_FAT16: u8 = 0x06;  // FAT16
pub const PARTITION_TYPE_FAT32: u8 = 0x0B;  // FAT32 CHS
pub const PARTITION_TYPE_FAT32_LBA: u8 = 0x0C;  // FAT32 LBA