// NTFS Boot Sector parser
// Reads and validates NTFS boot sector (first 512 bytes)

use crate::device_reader::AlignedDeviceReader;
use crate::ntfs::structures::*;
use moses_core::{Device, MosesError};
use log::{info, debug};

pub struct NtfsBootSectorReader {
    boot_sector: NtfsBootSector,
    _device: Device,
}

impl NtfsBootSectorReader {
    /// Read and parse NTFS boot sector from device
    pub fn new(device: Device) -> Result<Self, MosesError> {
        use crate::utils::open_device_with_fallback;
        
        info!("Reading NTFS boot sector from device: {}", device.name);
        
        // Open device
        let file = open_device_with_fallback(&device)?;
        let mut reader = AlignedDeviceReader::new(file);
        
        // Read first sector (512 bytes)
        let boot_data = reader.read_at(0, 512)?;
        
        // Parse boot sector
        let boot_sector = unsafe {
            std::ptr::read_unaligned(boot_data.as_ptr() as *const NtfsBootSector)
        };
        
        // Validate
        boot_sector.validate()?;
        
        info!("NTFS boot sector validated successfully");
        // Copy fields to avoid unaligned access
        let bytes_per_sector = boot_sector.bytes_per_sector;
        let sectors_per_cluster = boot_sector.sectors_per_cluster;
        let total_sectors = boot_sector.total_sectors;
        let mft_lcn = boot_sector.mft_lcn;
        debug!("  Bytes per sector: {}", bytes_per_sector);
        debug!("  Sectors per cluster: {}", sectors_per_cluster);
        debug!("  Total sectors: {}", total_sectors);
        debug!("  MFT starts at cluster: {}", mft_lcn);
        debug!("  MFT record size: {} bytes", boot_sector.mft_record_size());
        
        Ok(Self {
            boot_sector,
            _device: device,
        })
    }
    
    /// Get the boot sector
    pub fn boot_sector(&self) -> &NtfsBootSector {
        &self.boot_sector
    }
    
    /// Get MFT location in bytes
    pub fn mft_offset(&self) -> u64 {
        let mft_lcn = self.boot_sector.mft_lcn;
        mft_lcn * self.boot_sector.bytes_per_cluster() as u64
    }
    
    /// Get MFT mirror location in bytes
    pub fn mftmirr_offset(&self) -> u64 {
        let mftmirr_lcn = self.boot_sector.mftmirr_lcn;
        mftmirr_lcn * self.boot_sector.bytes_per_cluster() as u64
    }
    
    /// Get volume size in bytes
    pub fn volume_size(&self) -> u64 {
        let total_sectors = self.boot_sector.total_sectors;
        let bytes_per_sector = self.boot_sector.bytes_per_sector;
        total_sectors * bytes_per_sector as u64
    }
    
    /// Check if volume parameters are reasonable
    pub fn sanity_check(&self) -> Result<(), MosesError> {
        let mft_offset = self.mft_offset();
        let volume_size = self.volume_size();
        
        // MFT should be within volume
        if mft_offset >= volume_size {
            return Err(MosesError::Other(format!(
                "MFT offset {} exceeds volume size {}",
                mft_offset, volume_size
            )));
        }
        
        // MFT mirror should be within volume
        let mftmirr_offset = self.mftmirr_offset();
        if mftmirr_offset >= volume_size {
            return Err(MosesError::Other(format!(
                "MFT mirror offset {} exceeds volume size {}",
                mftmirr_offset, volume_size
            )));
        }
        
        // Cluster size should be reasonable (512 bytes to 64KB)
        let cluster_size = self.boot_sector.bytes_per_cluster();
        if cluster_size < 512 || cluster_size > 65536 {
            return Err(MosesError::Other(format!(
                "Unreasonable cluster size: {} bytes",
                cluster_size
            )));
        }
        
        // MFT record size should be reasonable (typically 1024 bytes)
        let mft_record_size = self.boot_sector.mft_record_size();
        if mft_record_size < 512 || mft_record_size > 4096 {
            return Err(MosesError::Other(format!(
                "Unreasonable MFT record size: {} bytes",
                mft_record_size
            )));
        }
        
        Ok(())
    }
}

/// Parse NTFS boot sector from raw bytes
pub fn parse_boot_sector(data: &[u8]) -> Result<NtfsBootSector, MosesError> {
    if data.len() < 512 {
        return Err(MosesError::Other("Boot sector must be at least 512 bytes".to_string()));
    }
    
    let boot_sector = unsafe {
        std::ptr::read_unaligned(data.as_ptr() as *const NtfsBootSector)
    };
    
    boot_sector.validate()?;
    Ok(boot_sector)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_boot_sector_validation() {
        // Create a minimal valid NTFS boot sector
        let mut data = vec![0u8; 512];
        
        // Jump instruction
        data[0] = 0xEB;
        data[1] = 0x52;
        data[2] = 0x90;
        
        // OEM ID "NTFS    "
        data[3..11].copy_from_slice(b"NTFS    ");
        
        // Bytes per sector (512)
        data[0x0B] = 0x00;
        data[0x0C] = 0x02;
        
        // Sectors per cluster (8)
        data[0x0D] = 8;
        
        // Media descriptor
        data[0x15] = 0xF8;
        
        // Total sectors (example: 1000000)
        let total_sectors = 1000000u64;
        data[0x28..0x30].copy_from_slice(&total_sectors.to_le_bytes());
        
        // MFT LCN (example: cluster 4)
        let mft_lcn = 4u64;
        data[0x30..0x38].copy_from_slice(&mft_lcn.to_le_bytes());
        
        // MFT mirror LCN (example: cluster 1000)
        let mftmirr_lcn = 1000u64;
        data[0x38..0x40].copy_from_slice(&mftmirr_lcn.to_le_bytes());
        
        // Clusters per MFT record (-10 = 1024 bytes)
        data[0x40] = 0xF6; // -10 in signed byte
        
        // Clusters per index buffer (-10 = 1024 bytes)
        data[0x44] = 0xF6;
        
        // Boot signature
        data[0x1FE] = 0x55;
        data[0x1FF] = 0xAA;
        
        // Parse and validate
        let boot_sector = parse_boot_sector(&data).unwrap();
        
        // Copy fields to avoid unaligned access
        let bytes_per_sector = boot_sector.bytes_per_sector;
        let sectors_per_cluster = boot_sector.sectors_per_cluster;
        let total_sectors = boot_sector.total_sectors;
        let mft_lcn = boot_sector.mft_lcn;
        
        assert_eq!(bytes_per_sector, 512);
        assert_eq!(sectors_per_cluster, 8);
        assert_eq!(total_sectors, 1000000);
        assert_eq!(mft_lcn, 4);
        assert_eq!(boot_sector.mft_record_size(), 1024);
    }
}