// Partition Table Converter - Convert between MBR and GPT
use std::io::{Write, Read, Seek, SeekFrom};
use moses_core::{Device, MosesError};
use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PartitionStyle {
    MBR,
    GPT,
    Uninitialized,
}

pub struct PartitionStyleConverter;

impl PartitionStyleConverter {
    /// Convert a disk to the specified partition style
    pub fn convert(device: &Device, target_style: PartitionStyle) -> Result<(), MosesError> {
        log::info!("Converting {} to {:?} partition style", device.name, target_style);
        
        // Safety check
        if device.is_system {
            return Err(MosesError::InvalidInput(
                "Cannot convert system disk partition style".to_string()
            ));
        }
        
        match target_style {
            PartitionStyle::MBR => Self::convert_to_mbr(device),
            PartitionStyle::GPT => Self::convert_to_gpt(device),
            PartitionStyle::Uninitialized => Self::make_uninitialized(device),
        }
    }
    
    /// Detect current partition style
    pub fn detect_style(device: &Device) -> Result<PartitionStyle, MosesError> {
        #[cfg(target_os = "windows")]
        {
            Self::detect_style_windows(device)
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            Self::detect_style_unix(device)
        }
    }
    
    #[cfg(target_os = "windows")]
    fn detect_style_windows(device: &Device) -> Result<PartitionStyle, MosesError> {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;
        use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ};
        
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .access_mode(GENERIC_READ)
            .open(&device.id)
            .map_err(|e| MosesError::IoError(e))?;
        
        // Read first 512 bytes for MBR
        let mut mbr_buffer = vec![0u8; 512];
        file.read_exact(&mut mbr_buffer)
            .map_err(|e| MosesError::Other(format!("Failed to read MBR: {}", e)))?;
        
        // Check MBR signature
        if mbr_buffer[0x1FE] != 0x55 || mbr_buffer[0x1FF] != 0xAA {
            return Ok(PartitionStyle::Uninitialized);
        }
        
        // Read LBA 1 for GPT header
        file.seek(SeekFrom::Start(512))
            .map_err(|e| MosesError::Other(format!("Failed to seek to GPT: {}", e)))?;
        
        let mut gpt_buffer = vec![0u8; 512];
        file.read_exact(&mut gpt_buffer)
            .map_err(|e| MosesError::Other(format!("Failed to read GPT: {}", e)))?;
        
        // Check for GPT signature "EFI PART"
        if &gpt_buffer[0..8] == b"EFI PART" {
            Ok(PartitionStyle::GPT)
        } else {
            // Check if MBR has valid partitions
            let has_partitions = (0..4).any(|i| {
                let offset = 0x1BE + (i * 16);
                mbr_buffer[offset + 4] != 0 // Partition type != 0
            });
            
            if has_partitions {
                Ok(PartitionStyle::MBR)
            } else {
                Ok(PartitionStyle::Uninitialized)
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    fn detect_style_unix(device: &Device) -> Result<PartitionStyle, MosesError> {
        use std::fs::OpenOptions;
        
        let mut file = OpenOptions::new()
            .read(true)
            .open(&device.id)
            .map_err(|e| MosesError::IoError(e))?;
        
        // Read first 512 bytes for MBR
        let mut mbr_buffer = vec![0u8; 512];
        file.read_exact(&mut mbr_buffer)
            .map_err(|e| MosesError::Other(format!("Failed to read MBR: {}", e)))?;
        
        // Check MBR signature
        if mbr_buffer[0x1FE] != 0x55 || mbr_buffer[0x1FF] != 0xAA {
            return Ok(PartitionStyle::Uninitialized);
        }
        
        // Read LBA 1 for GPT header
        file.seek(SeekFrom::Start(512))
            .map_err(|e| MosesError::Other(format!("Failed to seek to GPT: {}", e)))?;
        
        let mut gpt_buffer = vec![0u8; 512];
        file.read_exact(&mut gpt_buffer)
            .map_err(|e| MosesError::Other(format!("Failed to read GPT: {}", e)))?;
        
        // Check for GPT signature
        if &gpt_buffer[0..8] == b"EFI PART" {
            Ok(PartitionStyle::GPT)
        } else {
            Ok(PartitionStyle::MBR)
        }
    }
    
    /// Convert to MBR partition table
    fn convert_to_mbr(device: &Device) -> Result<(), MosesError> {
        log::info!("Converting {} to MBR partition table", device.name);
        
        #[cfg(target_os = "windows")]
        {
            use std::fs::OpenOptions;
            use std::os::windows::fs::OpenOptionsExt;
            use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_WRITE};
            
            let mut file = OpenOptions::new()
                .write(true)
                .custom_flags(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .access_mode(GENERIC_WRITE)
                .open(&device.id)
                .map_err(|e| MosesError::IoError(e))?;
            
            Self::write_mbr_structure(&mut file, device.size)?;
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            use std::fs::OpenOptions;
            
            let mut file = OpenOptions::new()
                .write(true)
                .open(&device.id)
                .map_err(|e| MosesError::IoError(e))?;
            
            Self::write_mbr_structure(&mut file, device.size)?;
        }
        
        log::info!("Successfully converted to MBR");
        Ok(())
    }
    
    /// Write a clean MBR structure
    fn write_mbr_structure<W: Write + Seek>(writer: &mut W, disk_size: u64) -> Result<(), MosesError> {
        // Create empty MBR
        let mut mbr = vec![0u8; 512];
        
        // Write disk signature (required by Windows)
        let disk_sig = rand::random::<u32>();
        mbr[0x1B8..0x1BC].copy_from_slice(&disk_sig.to_le_bytes());
        
        // MBR signature
        mbr[0x1FE] = 0x55;
        mbr[0x1FF] = 0xAA;
        
        // Write MBR
        writer.seek(SeekFrom::Start(0))
            .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
        writer.write_all(&mbr)
            .map_err(|e| MosesError::Other(format!("Failed to write MBR: {}", e)))?;
        
        // Clear any GPT structures that might exist
        // Clear primary GPT header (LBA 1)
        let zero_sector = vec![0u8; 512];
        writer.seek(SeekFrom::Start(512))
            .map_err(|e| MosesError::Other(format!("Failed to seek to GPT: {}", e)))?;
        writer.write_all(&zero_sector)
            .map_err(|e| MosesError::Other(format!("Failed to clear GPT header: {}", e)))?;
        
        // Clear GPT partition entries (LBA 2-33)
        let zero_entries = vec![0u8; 32 * 512];
        writer.write_all(&zero_entries)
            .map_err(|e| MosesError::Other(format!("Failed to clear GPT entries: {}", e)))?;
        
        // Clear backup GPT if disk is large enough
        if disk_size > 33 * 512 {
            let backup_gpt_start = disk_size - (33 * 512);
            writer.seek(SeekFrom::Start(backup_gpt_start))
                .map_err(|e| MosesError::Other(format!("Failed to seek to backup GPT: {}", e)))?;
            writer.write_all(&zero_entries)
                .map_err(|e| MosesError::Other(format!("Failed to clear backup GPT: {}", e)))?;
        }
        
        writer.flush()
            .map_err(|e| MosesError::Other(format!("Failed to flush: {}", e)))?;
        
        Ok(())
    }
    
    /// Convert to GPT partition table
    fn convert_to_gpt(device: &Device) -> Result<(), MosesError> {
        log::info!("Converting {} to GPT partition table", device.name);
        
        #[cfg(target_os = "windows")]
        {
            use std::fs::OpenOptions;
            use std::os::windows::fs::OpenOptionsExt;
            use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_WRITE};
            
            let mut file = OpenOptions::new()
                .write(true)
                .custom_flags(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .access_mode(GENERIC_WRITE)
                .open(&device.id)
                .map_err(|e| MosesError::IoError(e))?;
            
            Self::write_gpt_structure(&mut file, device.size)?;
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            use std::fs::OpenOptions;
            
            let mut file = OpenOptions::new()
                .write(true)
                .open(&device.id)
                .map_err(|e| MosesError::IoError(e))?;
            
            Self::write_gpt_structure(&mut file, device.size)?;
        }
        
        log::info!("Successfully converted to GPT");
        Ok(())
    }
    
    /// Write a clean GPT structure
    fn write_gpt_structure<W: Write + Seek>(writer: &mut W, disk_size: u64) -> Result<(), MosesError> {
        // Create protective MBR
        let mut mbr = vec![0u8; 512];
        
        // Single protective partition covering whole disk
        let offset = 0x1BE;
        mbr[offset] = 0x00; // Not bootable
        mbr[offset + 1] = 0x00; // Start head
        mbr[offset + 2] = 0x02; // Start sector
        mbr[offset + 3] = 0x00; // Start cylinder
        mbr[offset + 4] = 0xEE; // GPT protective partition type
        mbr[offset + 5] = 0xFF; // End head
        mbr[offset + 6] = 0xFF; // End sector
        mbr[offset + 7] = 0xFF; // End cylinder
        
        // Start LBA = 1
        mbr[offset + 8..offset + 12].copy_from_slice(&1u32.to_le_bytes());
        
        // Size in sectors (capped at max u32 for large disks)
        let sectors = (disk_size / 512).min(0xFFFFFFFF) as u32;
        mbr[offset + 12..offset + 16].copy_from_slice(&sectors.to_le_bytes());
        
        // MBR signature
        mbr[0x1FE] = 0x55;
        mbr[0x1FF] = 0xAA;
        
        // Write protective MBR
        writer.seek(SeekFrom::Start(0))
            .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
        writer.write_all(&mbr)
            .map_err(|e| MosesError::Other(format!("Failed to write protective MBR: {}", e)))?;
        
        // Create GPT header
        let mut gpt_header = vec![0u8; 512];
        
        // Signature "EFI PART"
        gpt_header[0..8].copy_from_slice(b"EFI PART");
        
        // Revision (1.0)
        gpt_header[8..12].copy_from_slice(&[0x00, 0x00, 0x01, 0x00]);
        
        // Header size (92 bytes)
        gpt_header[12..16].copy_from_slice(&92u32.to_le_bytes());
        
        // Current LBA (1 for primary)
        gpt_header[24..32].copy_from_slice(&1u64.to_le_bytes());
        
        // Backup LBA (last sector)
        let backup_lba = (disk_size / 512) - 1;
        gpt_header[32..40].copy_from_slice(&backup_lba.to_le_bytes());
        
        // First usable LBA (after partition entries, typically 34)
        gpt_header[40..48].copy_from_slice(&34u64.to_le_bytes());
        
        // Last usable LBA (before backup GPT)
        let last_usable = backup_lba - 33;
        gpt_header[48..56].copy_from_slice(&last_usable.to_le_bytes());
        
        // Disk GUID
        let disk_guid = Uuid::new_v4();
        gpt_header[56..72].copy_from_slice(disk_guid.as_bytes());
        
        // Partition entries start LBA (2)
        gpt_header[72..80].copy_from_slice(&2u64.to_le_bytes());
        
        // Number of partition entries (128)
        gpt_header[80..84].copy_from_slice(&128u32.to_le_bytes());
        
        // Size of partition entry (128 bytes)
        gpt_header[84..88].copy_from_slice(&128u32.to_le_bytes());
        
        // Calculate CRC32 of partition array (all zeros for empty)
        let empty_partitions = vec![0u8; 128 * 128]; // 128 entries * 128 bytes
        let partitions_crc = crc32fast::hash(&empty_partitions);
        gpt_header[88..92].copy_from_slice(&partitions_crc.to_le_bytes());
        
        // Calculate header CRC32 (with CRC field zeroed)
        let header_crc = crc32fast::hash(&gpt_header[0..92]);
        gpt_header[16..20].copy_from_slice(&header_crc.to_le_bytes());
        
        // Write primary GPT header
        writer.seek(SeekFrom::Start(512))
            .map_err(|e| MosesError::Other(format!("Failed to seek to GPT: {}", e)))?;
        writer.write_all(&gpt_header)
            .map_err(|e| MosesError::Other(format!("Failed to write GPT header: {}", e)))?;
        
        // Write empty partition entries
        writer.write_all(&empty_partitions)
            .map_err(|e| MosesError::Other(format!("Failed to write GPT entries: {}", e)))?;
        
        // Create backup GPT header (same but with swapped current/backup LBA)
        let mut backup_header = gpt_header.clone();
        backup_header[24..32].copy_from_slice(&backup_lba.to_le_bytes()); // Current LBA
        backup_header[32..40].copy_from_slice(&1u64.to_le_bytes()); // Backup LBA
        backup_header[72..80].copy_from_slice(&(backup_lba - 32).to_le_bytes()); // Partition entries before header
        
        // Recalculate CRC for backup header
        backup_header[16..20].copy_from_slice(&[0; 4]); // Zero CRC field
        let backup_crc = crc32fast::hash(&backup_header[0..92]);
        backup_header[16..20].copy_from_slice(&backup_crc.to_le_bytes());
        
        // Write backup partition entries
        writer.seek(SeekFrom::Start((backup_lba - 32) * 512))
            .map_err(|e| MosesError::Other(format!("Failed to seek to backup entries: {}", e)))?;
        writer.write_all(&empty_partitions)
            .map_err(|e| MosesError::Other(format!("Failed to write backup entries: {}", e)))?;
        
        // Write backup GPT header
        writer.write_all(&backup_header)
            .map_err(|e| MosesError::Other(format!("Failed to write backup GPT: {}", e)))?;
        
        writer.flush()
            .map_err(|e| MosesError::Other(format!("Failed to flush: {}", e)))?;
        
        Ok(())
    }
    
    /// Make disk uninitialized (no partition table)
    fn make_uninitialized(device: &Device) -> Result<(), MosesError> {
        log::info!("Removing partition table from {}", device.name);
        
        // This is essentially a quick clean - just wipe critical sectors
        // Use the cleaner module for this
        use super::cleaner::{DiskCleaner, CleanOptions, WipeMethod};
        
        let options = CleanOptions {
            wipe_method: WipeMethod::Quick,
            zero_entire_disk: false,
        };
        
        DiskCleaner::clean(device, &options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    
    #[test]
    fn test_mbr_creation() {
        let mut buffer = vec![0u8; 2048];
        let mut cursor = Cursor::new(&mut buffer);
        
        PartitionStyleConverter::write_mbr_structure(&mut cursor, 1024 * 1024 * 1024).unwrap();
        
        // Check MBR signature
        assert_eq!(buffer[0x1FE], 0x55);
        assert_eq!(buffer[0x1FF], 0xAA);
        
        // Check disk signature is not zero
        let disk_sig = u32::from_le_bytes([
            buffer[0x1B8], buffer[0x1B9], buffer[0x1BA], buffer[0x1BB]
        ]);
        assert_ne!(disk_sig, 0);
        
        // Check GPT header is cleared
        assert_ne!(&buffer[512..520], b"EFI PART");
    }
    
    #[test]
    fn test_gpt_creation() {
        let mut buffer = vec![0u8; 35 * 512]; // Enough for MBR + GPT header + entries
        let mut cursor = Cursor::new(&mut buffer);
        
        PartitionStyleConverter::write_gpt_structure(&mut cursor, 100 * 512).unwrap();
        
        // Check protective MBR
        assert_eq!(buffer[0x1FE], 0x55);
        assert_eq!(buffer[0x1FF], 0xAA);
        assert_eq!(buffer[0x1BE + 4], 0xEE); // GPT protective partition type
        
        // Check GPT signature
        assert_eq!(&buffer[512..520], b"EFI PART");
        
        // Check revision
        assert_eq!(&buffer[520..524], &[0x00, 0x00, 0x01, 0x00]);
    }
}