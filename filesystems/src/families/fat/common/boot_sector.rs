// Boot sector builder for FAT filesystems
// Handles common BPB fields and provides specialized builders for FAT16/FAT32

use super::constants::*;

/// Common FAT boot sector parameters
#[derive(Debug, Clone)]
pub struct FatBootSectorParams {
    pub oem_name: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    pub media_descriptor: u8,
    pub sectors_per_track: u16,
    pub num_heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors: u64,
    pub volume_serial: u32,
    pub volume_label: [u8; 11],
}

impl Default for FatBootSectorParams {
    fn default() -> Self {
        Self {
            oem_name: *b"MSWIN4.1",
            bytes_per_sector: STANDARD_BYTES_PER_SECTOR,
            sectors_per_cluster: 0,  // Must be set
            reserved_sectors: 1,  // FAT16 default
            num_fats: 2,
            media_descriptor: MEDIA_FIXED,
            sectors_per_track: 63,
            num_heads: 255,
            hidden_sectors: 0,
            total_sectors: 0,  // Must be set
            volume_serial: 0,  // Should be set
            volume_label: [0x20; 11],  // Space-padded
        }
    }
}

/// Build a FAT16 boot sector
pub fn build_fat16_boot_sector(
    params: &FatBootSectorParams,
    root_entries: u16,
    sectors_per_fat: u16,
) -> [u8; 512] {
    let mut boot_sector = [0u8; 512];
    
    // Jump instruction
    boot_sector[BS_JMP_BOOT] = 0xEB;
    boot_sector[BS_JMP_BOOT + 1] = 0x3C;
    boot_sector[BS_JMP_BOOT + 2] = 0x90;
    
    // OEM Name
    boot_sector[BS_OEM_NAME..BS_OEM_NAME + 8].copy_from_slice(&params.oem_name);
    
    // BPB Common fields
    boot_sector[BPB_BYTES_PER_SEC..BPB_BYTES_PER_SEC + 2]
        .copy_from_slice(&params.bytes_per_sector.to_le_bytes());
    boot_sector[BPB_SEC_PER_CLUS] = params.sectors_per_cluster;
    boot_sector[BPB_RSVD_SEC_CNT..BPB_RSVD_SEC_CNT + 2]
        .copy_from_slice(&params.reserved_sectors.to_le_bytes());
    boot_sector[BPB_NUM_FATS] = params.num_fats;
    boot_sector[BPB_ROOT_ENT_CNT..BPB_ROOT_ENT_CNT + 2]
        .copy_from_slice(&root_entries.to_le_bytes());
    
    // Total sectors
    if params.total_sectors < 65536 {
        boot_sector[BPB_TOT_SEC16..BPB_TOT_SEC16 + 2]
            .copy_from_slice(&(params.total_sectors as u16).to_le_bytes());
    } else {
        boot_sector[BPB_TOT_SEC32..BPB_TOT_SEC32 + 4]
            .copy_from_slice(&(params.total_sectors as u32).to_le_bytes());
    }
    
    boot_sector[BPB_MEDIA] = params.media_descriptor;
    boot_sector[BPB_FAT_SZ16..BPB_FAT_SZ16 + 2]
        .copy_from_slice(&sectors_per_fat.to_le_bytes());
    boot_sector[BPB_SEC_PER_TRK..BPB_SEC_PER_TRK + 2]
        .copy_from_slice(&params.sectors_per_track.to_le_bytes());
    boot_sector[BPB_NUM_HEADS..BPB_NUM_HEADS + 2]
        .copy_from_slice(&params.num_heads.to_le_bytes());
    boot_sector[BPB_HIDD_SEC..BPB_HIDD_SEC + 4]
        .copy_from_slice(&params.hidden_sectors.to_le_bytes());
    
    // FAT16 Extended BPB
    boot_sector[BS16_DRV_NUM] = 0x80;  // Hard disk
    boot_sector[BS16_RESERVED1] = 0;
    boot_sector[BS16_BOOT_SIG] = 0x29;  // Extended boot signature
    boot_sector[BS16_VOL_ID..BS16_VOL_ID + 4]
        .copy_from_slice(&params.volume_serial.to_le_bytes());
    boot_sector[BS16_VOL_LAB..BS16_VOL_LAB + 11]
        .copy_from_slice(&params.volume_label);
    boot_sector[BS16_FIL_SYS_TYPE..BS16_FIL_SYS_TYPE + 8]
        .copy_from_slice(b"FAT16   ");
    
    // Boot signature
    boot_sector[BOOT_SIGNATURE_OFFSET..BOOT_SIGNATURE_OFFSET + 2]
        .copy_from_slice(&BOOT_SIGNATURE);
    
    boot_sector
}

/// Build a FAT32 boot sector
pub fn build_fat32_boot_sector(
    params: &FatBootSectorParams,
    sectors_per_fat32: u32,
    root_cluster: u32,
    fs_info_sector: u16,
    backup_boot_sector: u16,
) -> [u8; 512] {
    let mut boot_sector = [0u8; 512];
    
    // Jump instruction
    boot_sector[BS_JMP_BOOT] = 0xEB;
    boot_sector[BS_JMP_BOOT + 1] = 0x58;  // Different offset for FAT32
    boot_sector[BS_JMP_BOOT + 2] = 0x90;
    
    // OEM Name
    boot_sector[BS_OEM_NAME..BS_OEM_NAME + 8].copy_from_slice(&params.oem_name);
    
    // BPB Common fields
    boot_sector[BPB_BYTES_PER_SEC..BPB_BYTES_PER_SEC + 2]
        .copy_from_slice(&params.bytes_per_sector.to_le_bytes());
    boot_sector[BPB_SEC_PER_CLUS] = params.sectors_per_cluster;
    boot_sector[BPB_RSVD_SEC_CNT..BPB_RSVD_SEC_CNT + 2]
        .copy_from_slice(&params.reserved_sectors.to_le_bytes());
    boot_sector[BPB_NUM_FATS] = params.num_fats;
    boot_sector[BPB_ROOT_ENT_CNT..BPB_ROOT_ENT_CNT + 2]
        .copy_from_slice(&0u16.to_le_bytes());  // Always 0 for FAT32
    
    // Total sectors (always use 32-bit field for FAT32)
    boot_sector[BPB_TOT_SEC32..BPB_TOT_SEC32 + 4]
        .copy_from_slice(&(params.total_sectors as u32).to_le_bytes());
    
    boot_sector[BPB_MEDIA] = params.media_descriptor;
    boot_sector[BPB_FAT_SZ16..BPB_FAT_SZ16 + 2]
        .copy_from_slice(&0u16.to_le_bytes());  // Always 0 for FAT32
    boot_sector[BPB_SEC_PER_TRK..BPB_SEC_PER_TRK + 2]
        .copy_from_slice(&params.sectors_per_track.to_le_bytes());
    boot_sector[BPB_NUM_HEADS..BPB_NUM_HEADS + 2]
        .copy_from_slice(&params.num_heads.to_le_bytes());
    boot_sector[BPB_HIDD_SEC..BPB_HIDD_SEC + 4]
        .copy_from_slice(&params.hidden_sectors.to_le_bytes());
    
    // FAT32 Extended BPB
    boot_sector[BPB_FAT_SZ32..BPB_FAT_SZ32 + 4]
        .copy_from_slice(&sectors_per_fat32.to_le_bytes());
    boot_sector[BPB_EXT_FLAGS..BPB_EXT_FLAGS + 2]
        .copy_from_slice(&0u16.to_le_bytes());  // Mirroring enabled
    boot_sector[BPB_FS_VER..BPB_FS_VER + 2]
        .copy_from_slice(&0u16.to_le_bytes());  // Version 0.0
    boot_sector[BPB_ROOT_CLUS..BPB_ROOT_CLUS + 4]
        .copy_from_slice(&root_cluster.to_le_bytes());
    boot_sector[BPB_FS_INFO..BPB_FS_INFO + 2]
        .copy_from_slice(&fs_info_sector.to_le_bytes());
    boot_sector[BPB_BK_BOOT_SEC..BPB_BK_BOOT_SEC + 2]
        .copy_from_slice(&backup_boot_sector.to_le_bytes());
    // Reserved bytes remain zero
    
    boot_sector[BS32_DRV_NUM] = 0x80;  // Hard disk
    boot_sector[BS32_RESERVED1] = 0;
    boot_sector[BS32_BOOT_SIG] = 0x29;  // Extended boot signature
    boot_sector[BS32_VOL_ID..BS32_VOL_ID + 4]
        .copy_from_slice(&params.volume_serial.to_le_bytes());
    boot_sector[BS32_VOL_LAB..BS32_VOL_LAB + 11]
        .copy_from_slice(&params.volume_label);
    boot_sector[BS32_FIL_SYS_TYPE..BS32_FIL_SYS_TYPE + 8]
        .copy_from_slice(b"FAT32   ");
    
    // Boot signature
    boot_sector[BOOT_SIGNATURE_OFFSET..BOOT_SIGNATURE_OFFSET + 2]
        .copy_from_slice(&BOOT_SIGNATURE);
    
    boot_sector
}