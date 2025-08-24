// Simple FAT32 format tool - directly formats a device with FAT32
// Uses the low-level implementation without async wrappers

use std::env;
use std::fs::OpenOptions;
use std::io::{Write, Seek, SeekFrom};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <device_path> [volume_label]", args[0]);
        eprintln!("Example: {} \\\\.\\PHYSICALDRIVE2", args[0]);
        eprintln!("Example: {} \\\\.\\PHYSICALDRIVE2 \"MYDISK\"", args[0]);
        std::process::exit(1);
    }
    
    let device_path = &args[1];
    let volume_label = if args.len() > 2 {
        Some(args[2].as_str())
    } else {
        Some("MOSES_FAT32")
    };
    
    println!("Moses FAT32 Formatter");
    println!("=====================");
    println!("Device: {}", device_path);
    println!("Volume Label: {}", volume_label.unwrap_or(""));
    println!();
    
    // Directly format the device with FAT32
    match format_fat32_direct(device_path, volume_label) {
        Ok(_) => {
            println!("✓ FAT32 format completed successfully!");
            println!();
            println!("The drive should now be recognized by Windows.");
            println!("You may need to unplug and replug the device.");
        }
        Err(e) => {
            eprintln!("✗ Error formatting device: {}", e);
            std::process::exit(1);
        }
    }
}

fn create_mbr_with_fat32_partition(total_sectors: u32) -> [u8; 512] {
    let mut mbr = [0u8; 512];
    
    // Partition entry at offset 446
    let partition_offset = 446;
    
    // Boot indicator (0x80 = bootable)
    mbr[partition_offset] = 0x00; // Not bootable
    
    // Starting CHS (1,0,1) - start at sector 1
    mbr[partition_offset + 1] = 0x00; // Head 0
    mbr[partition_offset + 2] = 0x02; // Sector 2, Cylinder 0
    mbr[partition_offset + 3] = 0x00; // Cylinder 0
    
    // Partition type (0x0C = FAT32 LBA)
    mbr[partition_offset + 4] = 0x0C;
    
    // Ending CHS (use LBA values)
    mbr[partition_offset + 5] = 0xFE; // Head 254
    mbr[partition_offset + 6] = 0xFF; // Sector 63, Cylinder 1023
    mbr[partition_offset + 7] = 0xFF; // Cylinder 1023
    
    // Starting LBA (sector 1, after MBR)
    mbr[partition_offset + 8..partition_offset + 12].copy_from_slice(&1u32.to_le_bytes());
    
    // Size in sectors (total - 1 for MBR)
    let partition_size = total_sectors - 1;
    mbr[partition_offset + 12..partition_offset + 16].copy_from_slice(&partition_size.to_le_bytes());
    
    // MBR signature
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    
    mbr
}

fn format_fat32_direct(device_path: &str, volume_label: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use moses_filesystems::fat_common::*;
    
    // Open the device
    let mut device = OpenOptions::new()
        .read(true)
        .write(true)
        .open(device_path)?;
    
    // Get device size
    device.seek(SeekFrom::End(0))?;
    let device_size = device.stream_position()?;
    device.seek(SeekFrom::Start(0))?;
    
    println!("Device size: {} bytes ({} MB)", device_size, device_size / (1024 * 1024));
    
    // Calculate parameters
    let bytes_per_sector = 512u32;
    let total_sectors = (device_size / bytes_per_sector as u64) as u32;
    
    // Use standard FAT32 cluster size based on volume size
    let sectors_per_cluster = if device_size <= 260 * 1024 * 1024 {
        1  // <= 260MB: 512 bytes
    } else if device_size <= 8 * 1024_u64.pow(3) {
        8  // <= 8GB: 4KB
    } else if device_size <= 16 * 1024_u64.pow(3) {
        16 // <= 16GB: 8KB
    } else if device_size <= 32 * 1024_u64.pow(3) {
        32 // <= 32GB: 16KB
    } else {
        64 // > 32GB: 32KB
    };
    
    println!("Sectors per cluster: {} ({} KB)", sectors_per_cluster, 
             (sectors_per_cluster * bytes_per_sector / 1024));
    
    // FAT32 parameters
    let reserved_sectors = 32u16;
    let num_fats = 2u8;
    let root_cluster = 2u32;
    
    // Calculate FAT size
    let tmp1 = total_sectors - reserved_sectors as u32;
    let tmp2 = (256 * sectors_per_cluster as u32) + num_fats as u32;
    let tmp3 = tmp2 / 2;
    let sectors_per_fat = (tmp1 + tmp3 - 1) / tmp3;
    
    println!("Sectors per FAT: {}", sectors_per_fat);
    
    // Calculate data sectors and clusters
    let data_start = reserved_sectors as u32 + (num_fats as u32 * sectors_per_fat);
    let data_sectors = total_sectors - data_start;
    let total_clusters = data_sectors / sectors_per_cluster as u32;
    
    println!("Total clusters: {}", total_clusters);
    
    if total_clusters < FAT32_MIN_CLUSTERS {
        return Err(format!("Volume too small for FAT32 (need at least {} clusters, have {})",
                          FAT32_MIN_CLUSTERS, total_clusters).into());
    }
    
    // Create partition table with FAT32 partition
    println!("Creating partition table...");
    let mbr = create_mbr_with_fat32_partition(total_sectors);
    device.write_all(&mbr)?;
    
    // Seek to partition start (skip MBR)
    device.seek(SeekFrom::Start(512))?;
    
    // Create boot sector
    println!("Writing boot sector...");
    let mut boot_sector = [0u8; 512];
    
    // Jump instruction
    boot_sector[0] = 0xEB;
    boot_sector[1] = 0x58;
    boot_sector[2] = 0x90;
    
    // OEM name
    boot_sector[BS_OEM_NAME..BS_OEM_NAME + 8].copy_from_slice(b"MOSES1.0");
    
    // BPB
    boot_sector[BPB_BYTES_PER_SEC..BPB_BYTES_PER_SEC + 2].copy_from_slice(&bytes_per_sector.to_le_bytes()[0..2]);
    boot_sector[BPB_SEC_PER_CLUS] = sectors_per_cluster as u8;
    boot_sector[BPB_RSVD_SEC_CNT..BPB_RSVD_SEC_CNT + 2].copy_from_slice(&reserved_sectors.to_le_bytes());
    boot_sector[BPB_NUM_FATS] = num_fats;
    boot_sector[BPB_ROOT_ENT_CNT..BPB_ROOT_ENT_CNT + 2].copy_from_slice(&0u16.to_le_bytes()); // 0 for FAT32
    boot_sector[BPB_TOT_SEC16..BPB_TOT_SEC16 + 2].copy_from_slice(&0u16.to_le_bytes());
    boot_sector[BPB_MEDIA] = MEDIA_FIXED;
    boot_sector[BPB_FAT_SZ16..BPB_FAT_SZ16 + 2].copy_from_slice(&0u16.to_le_bytes()); // 0 for FAT32
    boot_sector[BPB_SEC_PER_TRK..BPB_SEC_PER_TRK + 2].copy_from_slice(&63u16.to_le_bytes());
    boot_sector[BPB_NUM_HEADS..BPB_NUM_HEADS + 2].copy_from_slice(&255u16.to_le_bytes());
    boot_sector[BPB_HIDD_SEC..BPB_HIDD_SEC + 4].copy_from_slice(&1u32.to_le_bytes()); // Start at sector 1
    boot_sector[BPB_TOT_SEC32..BPB_TOT_SEC32 + 4].copy_from_slice(&(total_sectors - 1).to_le_bytes());
    
    // FAT32 specific fields
    boot_sector[BPB_FAT_SZ32..BPB_FAT_SZ32 + 4].copy_from_slice(&sectors_per_fat.to_le_bytes());
    boot_sector[BPB_EXT_FLAGS..BPB_EXT_FLAGS + 2].copy_from_slice(&0u16.to_le_bytes());
    boot_sector[BPB_FS_VER..BPB_FS_VER + 2].copy_from_slice(&0u16.to_le_bytes());
    boot_sector[BPB_ROOT_CLUS..BPB_ROOT_CLUS + 4].copy_from_slice(&root_cluster.to_le_bytes());
    boot_sector[BPB_FS_INFO..BPB_FS_INFO + 2].copy_from_slice(&1u16.to_le_bytes()); // FSInfo at sector 1
    boot_sector[BPB_BK_BOOT_SEC..BPB_BK_BOOT_SEC + 2].copy_from_slice(&6u16.to_le_bytes()); // Backup at sector 6
    
    // Extended boot record
    boot_sector[BS32_DRV_NUM] = 0x80;
    boot_sector[BS32_BOOT_SIG] = 0x29;
    boot_sector[BS32_VOL_ID..BS32_VOL_ID + 4].copy_from_slice(&generate_volume_serial().to_le_bytes());
    
    // Volume label
    let label_bytes = format_volume_label(volume_label);
    boot_sector[BS32_VOL_LAB..BS32_VOL_LAB + 11].copy_from_slice(&label_bytes);
    
    // File system type
    boot_sector[BS32_FIL_SYS_TYPE..BS32_FIL_SYS_TYPE + 8].copy_from_slice(b"FAT32   ");
    
    // Boot signature
    boot_sector[510] = 0x55;
    boot_sector[511] = 0xAA;
    
    device.write_all(&boot_sector)?;
    
    // Create FSInfo sector
    println!("Writing FSInfo sector...");
    let mut fsinfo = [0u8; 512];
    
    // Lead signature "RRaA"
    fsinfo[0..4].copy_from_slice(&0x41615252u32.to_le_bytes());
    
    // Struct signature "rrAa" at offset 484
    fsinfo[484..488].copy_from_slice(&0x61417272u32.to_le_bytes());
    
    // Free cluster count (total - 1 for root)
    let free_clusters = total_clusters - 1;
    fsinfo[488..492].copy_from_slice(&free_clusters.to_le_bytes());
    
    // Next free cluster (start after root)
    fsinfo[492..496].copy_from_slice(&3u32.to_le_bytes());
    
    // Trail signature
    fsinfo[508..512].copy_from_slice(&0xAA550000u32.to_le_bytes());
    
    device.write_all(&fsinfo)?;
    
    // Write empty sectors 2-5
    let empty_sector = [0u8; 512];
    for _ in 2..6 {
        device.write_all(&empty_sector)?;
    }
    
    // Write backup boot sector at sector 6
    println!("Writing backup boot sector...");
    device.write_all(&boot_sector)?;
    
    // Write backup FSInfo at sector 7
    device.write_all(&fsinfo)?;
    
    // Pad to reserved sectors
    for _ in 8..reserved_sectors {
        device.write_all(&empty_sector)?;
    }
    
    // Initialize FAT tables
    println!("Initializing FAT tables...");
    
    // Create first FAT entries
    let mut fat_buffer = vec![0u8; 512];
    
    // Entry 0: Media descriptor with high bits set
    fat_buffer[0..4].copy_from_slice(&0xFFFFFFF8u32.to_le_bytes());
    
    // Entry 1: End of chain marker
    fat_buffer[4..8].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());
    
    // Entry 2: Root directory - end of chain
    fat_buffer[8..12].copy_from_slice(&0x0FFFFFFFu32.to_le_bytes());
    
    // Write first FAT
    device.write_all(&fat_buffer)?;
    
    // Clear rest of first sector
    fat_buffer = vec![0u8; 512];
    
    // Write rest of FAT
    let fat_sectors = sectors_per_fat as usize;
    for _i in 1..fat_sectors {
        device.write_all(&fat_buffer)?;
    }
    
    // Write second FAT (copy of first)
    device.seek(SeekFrom::Start(512 + (reserved_sectors as u64 * 512)))?;
    
    // Rewrite first entries for second FAT
    fat_buffer[0..4].copy_from_slice(&0xFFFFFFF8u32.to_le_bytes());
    fat_buffer[4..8].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());
    fat_buffer[8..12].copy_from_slice(&0x0FFFFFFFu32.to_le_bytes());
    device.write_all(&fat_buffer)?;
    
    // Clear rest
    fat_buffer = vec![0u8; 512];
    for _i in 1..fat_sectors {
        device.write_all(&fat_buffer)?;
    }
    
    // Initialize root directory (cluster 2)
    println!("Initializing root directory...");
    let root_start = 512 + (data_start as u64 * 512);
    device.seek(SeekFrom::Start(root_start))?;
    
    // Volume label entry in root directory
    if let Some(label) = volume_label {
        let mut vol_entry = [0u8; 32];
        let label_bytes = format_volume_label(Some(label));
        vol_entry[0..11].copy_from_slice(&label_bytes);
        vol_entry[0x0B] = 0x08; // Volume label attribute
        device.write_all(&vol_entry)?;
        
        // Clear rest of first sector
        let padding = [0u8; 480];
        device.write_all(&padding)?;
    } else {
        // Empty root directory
        device.write_all(&empty_sector)?;
    }
    
    // Clear rest of root directory cluster
    for _ in 1..sectors_per_cluster {
        device.write_all(&empty_sector)?;
    }
    
    device.flush()?;
    
    println!("✓ FAT32 format complete!");
    Ok(())
}