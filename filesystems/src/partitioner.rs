// Partition table management for Moses
// Handles creation of MBR and GPT partition tables


pub mod mbr_verifier;
use moses_core::{Device, MosesError};

#[cfg(test)]
mod mbr_tests;
use std::io::{Write, Seek, SeekFrom};
use log::info;

/// Type of partition table to create
#[derive(Debug, Clone, Copy)]
pub enum PartitionTableType {
    MBR,
    GPT,
}

/// Partition entry for creation
#[derive(Debug, Clone)]
pub struct PartitionEntry {
    pub start_lba: u64,
    pub size_lba: u64,
    pub partition_type: u8,  // For MBR
    pub name: String,         // For GPT
}

/// Create a partition table with a single partition spanning the whole disk
pub fn create_single_partition_table(
    device: &Device,
    table_type: PartitionTableType,
    filesystem_type: &str,
) -> Result<Vec<u8>, MosesError> {
    match table_type {
        PartitionTableType::MBR => create_mbr_single_partition(device, filesystem_type),
        PartitionTableType::GPT => create_gpt_single_partition(device, filesystem_type),
    }
}

/// Create an MBR with a single partition
fn create_mbr_single_partition(device: &Device, filesystem_type: &str) -> Result<Vec<u8>, MosesError> {
    let mut mbr = vec![0u8; 512];
    
    // MBR boot code (minimal - just enough to be valid)
    // Jump instruction
    mbr[0] = 0xEB;
    mbr[1] = 0x3C;
    mbr[2] = 0x90;
    
    // Partition table starts at offset 446 (0x1BE)
    let partition_offset = 446;
    
    // Determine partition type based on filesystem
    let partition_type = match filesystem_type.to_lowercase().as_str() {
        "fat16" => 0x06,  // FAT16
        "fat32" => 0x0C,  // FAT32 LBA
        "ntfs" => 0x07,   // NTFS
        "exfat" => 0x07,  // exFAT also uses 0x07
        _ => 0x83,        // Linux native
    };
    
    // Calculate partition parameters
    let start_lba = 2048u32;  // Start at 1MB for alignment (standard for modern systems)
    let total_sectors = (device.size / 512) as u32;
    let partition_size = total_sectors.saturating_sub(start_lba);
    
    // Calculate CHS values (for compatibility, though LBA is used)
    // Standard geometry: 255 heads, 63 sectors per track
    let heads = 255u32;
    let sectors_per_track = 63u32;
    let cylinder_size = heads * sectors_per_track;
    
    // Starting CHS (for LBA 2048)
    let start_cylinder = start_lba / cylinder_size;
    let start_temp = start_lba % cylinder_size;
    let start_head = start_temp / sectors_per_track;
    let start_sector = (start_temp % sectors_per_track) + 1; // Sectors are 1-based
    
    // Ending CHS
    let end_lba = start_lba + partition_size - 1;
    let end_cylinder = end_lba / cylinder_size;
    let end_temp = end_lba % cylinder_size;
    let end_head = end_temp / sectors_per_track;
    let end_sector = (end_temp % sectors_per_track) + 1;
    
    // If cylinder > 1023, use maximum CHS values (LBA will be used instead)
    let (end_chs_head, end_chs_sector, end_chs_cyl) = if end_cylinder > 1023 {
        (0xFE, 0xFF, 0xFF)  // Maximum CHS values - indicates to use LBA
    } else {
        (
            end_head as u8,
            ((end_sector & 0x3F) | ((end_cylinder >> 2) & 0xC0)) as u8,
            (end_cylinder & 0xFF) as u8
        )
    };
    
    // Partition entry 1
    mbr[partition_offset] = 0x80;  // Bootable flag
    mbr[partition_offset + 1] = start_head as u8;  // Starting head
    mbr[partition_offset + 2] = ((start_sector & 0x3F) | ((start_cylinder >> 2) & 0xC0)) as u8;  // Starting sector + cylinder high
    mbr[partition_offset + 3] = (start_cylinder & 0xFF) as u8;  // Starting cylinder low
    mbr[partition_offset + 4] = partition_type;  // Partition type
    mbr[partition_offset + 5] = end_chs_head;  // Ending head
    mbr[partition_offset + 6] = end_chs_sector;  // Ending sector + cylinder high
    mbr[partition_offset + 7] = end_chs_cyl;  // Ending cylinder low
    
    // LBA values
    mbr[partition_offset + 8..partition_offset + 12].copy_from_slice(&start_lba.to_le_bytes());
    mbr[partition_offset + 12..partition_offset + 16].copy_from_slice(&partition_size.to_le_bytes());
    
    // Disk signature (required by Windows to recognize the MBR)
    // Random 4-byte signature at offset 440 (0x1B8)
    // Windows requires this to be non-zero for MBR disks
    let disk_sig = rand::random::<u32>();
    // Ensure it's not zero (Windows requirement)
    let disk_sig = if disk_sig == 0 { 0x12345678 } else { disk_sig };
    mbr[440..444].copy_from_slice(&disk_sig.to_le_bytes());
    
    // MBR signature
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    
    info!("Created MBR with single {} partition:", filesystem_type);
    info!("  Partition type: 0x{:02X}", partition_type);
    info!("  Start LBA: {} (offset {} bytes)", start_lba, start_lba * 512);
    info!("  Size: {} sectors ({} MB)", partition_size, partition_size * 512 / 1024 / 1024);
    info!("  Disk signature: 0x{:08X}", disk_sig);
    info!("  CHS geometry: 255 heads, 63 sectors/track");
    
    Ok(mbr)
}

/// Create a GPT with a single partition
fn create_gpt_single_partition(device: &Device, filesystem_type: &str) -> Result<Vec<u8>, MosesError> {
    // GPT requires:
    // 1. Protective MBR at LBA 0
    // 2. GPT header at LBA 1
    // 3. Partition entries starting at LBA 2 (usually)
    // 4. Backup GPT at end of disk
    
    let mut result = Vec::new();
    
    // 1. Create protective MBR
    let mut protective_mbr = vec![0u8; 512];
    
    // Minimal boot code
    protective_mbr[0] = 0xEB;
    protective_mbr[1] = 0x3C;
    protective_mbr[2] = 0x90;
    
    // Protective MBR partition entry at offset 446
    protective_mbr[446] = 0x00;  // Not bootable
    protective_mbr[446 + 1] = 0x00;  // Starting head
    protective_mbr[446 + 2] = 0x01;  // Starting sector
    protective_mbr[446 + 3] = 0x00;  // Starting cylinder
    protective_mbr[446 + 4] = 0xEE;  // GPT protective partition type
    protective_mbr[446 + 5] = 0xFE;  // Ending head
    protective_mbr[446 + 6] = 0xFF;  // Ending sector + cylinder
    protective_mbr[446 + 7] = 0xFF;  // Ending cylinder
    
    // LBA 1 to end of disk
    protective_mbr[446 + 8..446 + 12].copy_from_slice(&1u32.to_le_bytes());
    let protective_size = ((device.size / 512).saturating_sub(1) as u32).min(0xFFFFFFFF);
    protective_mbr[446 + 12..446 + 16].copy_from_slice(&protective_size.to_le_bytes());
    
    // MBR signature
    protective_mbr[510] = 0x55;
    protective_mbr[511] = 0xAA;
    
    result.extend_from_slice(&protective_mbr);
    
    // 2. Create GPT header at LBA 1
    let mut gpt_header = vec![0u8; 512];
    
    // Signature "EFI PART"
    gpt_header[0..8].copy_from_slice(b"EFI PART");
    
    // Revision (1.0)
    gpt_header[8..12].copy_from_slice(&[0x00, 0x00, 0x01, 0x00]);
    
    // Header size (92 bytes)
    gpt_header[12..16].copy_from_slice(&92u32.to_le_bytes());
    
    // CRC32 of header (calculated later)
    // Zero for now at offset 16
    
    // Current LBA (1)
    gpt_header[24..32].copy_from_slice(&1u64.to_le_bytes());
    
    // Backup LBA (last sector)
    let backup_lba = (device.size / 512) - 1;
    gpt_header[32..40].copy_from_slice(&backup_lba.to_le_bytes());
    
    // First usable LBA (after partition table, typically 34)
    gpt_header[40..48].copy_from_slice(&34u64.to_le_bytes());
    
    // Last usable LBA (before backup GPT)
    let last_usable = backup_lba - 33;
    gpt_header[48..56].copy_from_slice(&last_usable.to_le_bytes());
    
    // Disk GUID (random)
    let disk_guid = uuid::Uuid::new_v4();
    gpt_header[56..72].copy_from_slice(disk_guid.as_bytes());
    
    // Partition entries start LBA (2)
    gpt_header[72..80].copy_from_slice(&2u64.to_le_bytes());
    
    // Number of partition entries (128)
    gpt_header[80..84].copy_from_slice(&128u32.to_le_bytes());
    
    // Size of partition entry (128 bytes)
    gpt_header[84..88].copy_from_slice(&128u32.to_le_bytes());
    
    // CRC32 of partition array (calculated later)
    // Zero for now at offset 88
    
    result.extend_from_slice(&gpt_header);
    
    // 3. Create partition entries (128 entries * 128 bytes = 16KB = 32 sectors)
    let mut partition_entries = vec![0u8; 128 * 128];
    
    // Create single partition entry
    let partition_type_guid = match filesystem_type.to_lowercase().as_str() {
        "fat16" | "fat32" => uuid::Uuid::parse_str("EBD0A0A2-B9E5-4433-87C0-68B6B72699C7").unwrap(), // Basic data
        "ntfs" | "exfat" => uuid::Uuid::parse_str("EBD0A0A2-B9E5-4433-87C0-68B6B72699C7").unwrap(),  // Basic data
        _ => uuid::Uuid::parse_str("0FC63DAF-8483-4772-8E79-3D69D8477DE4").unwrap(), // Linux filesystem
    };
    
    // Partition type GUID
    partition_entries[0..16].copy_from_slice(partition_type_guid.as_bytes());
    
    // Unique partition GUID
    let partition_guid = uuid::Uuid::new_v4();
    partition_entries[16..32].copy_from_slice(partition_guid.as_bytes());
    
    // First LBA (align to 1MB = 2048 sectors)
    partition_entries[32..40].copy_from_slice(&2048u64.to_le_bytes());
    
    // Last LBA
    partition_entries[40..48].copy_from_slice(&last_usable.to_le_bytes());
    
    // Attributes (0 = normal)
    partition_entries[48..56].copy_from_slice(&0u64.to_le_bytes());
    
    // Partition name (UTF-16LE)
    let name = format!("{} Volume", filesystem_type.to_uppercase());
    let name_utf16: Vec<u16> = name.encode_utf16().collect();
    for (i, &ch) in name_utf16.iter().take(36).enumerate() {  // Max 36 UTF-16 chars
        partition_entries[56 + i * 2..56 + i * 2 + 2].copy_from_slice(&ch.to_le_bytes());
    }
    
    // Calculate CRC32 of partition array
    let partition_crc = crc32_of(&partition_entries);
    
    // Update GPT header with partition array CRC
    result[512 + 88..512 + 92].copy_from_slice(&partition_crc.to_le_bytes());
    
    // Calculate CRC32 of GPT header
    let header_crc = crc32_of(&result[512..512 + 92]);
    result[512 + 16..512 + 20].copy_from_slice(&header_crc.to_le_bytes());
    
    // Add partition entries
    result.extend_from_slice(&partition_entries);
    
    info!("Created GPT with single {} partition", filesystem_type);
    
    Ok(result)
}

/// Calculate CRC32 (using the CRC32C algorithm that GPT uses)
fn crc32_of(data: &[u8]) -> u32 {
    // For now, use a simple CRC32 implementation
    // In production, use the crc32c crate
    let mut crc = 0xFFFFFFFFu32;
    let polynomial = 0xEDB88320u32;
    
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ polynomial;
            } else {
                crc >>= 1;
            }
        }
    }
    
    !crc
}

/// Write partition table to device
pub fn write_partition_table<W: Write + Seek>(
    writer: &mut W,
    partition_table: &[u8],
) -> Result<(), MosesError> {
    writer.seek(SeekFrom::Start(0))
        .map_err(|e| MosesError::Other(format!("Failed to seek to start: {}", e)))?;
    
    writer.write_all(partition_table)
        .map_err(|e| MosesError::Other(format!("Failed to write partition table: {}", e)))?;
    
    writer.flush()
        .map_err(|e| MosesError::Other(format!("Failed to flush: {}", e)))?;
    
    Ok(())
}