// NTFS module tests

#[cfg(test)]
mod tests {
    use super::super::*;
    
    #[test]
    fn test_ntfs_detection() {
        // Create a minimal valid NTFS boot sector
        let mut boot_sector = vec![0u8; 512];
        
        // Jump instruction
        boot_sector[0] = 0xEB;
        boot_sector[1] = 0x52;
        boot_sector[2] = 0x90;
        
        // OEM ID "NTFS    "
        boot_sector[3..11].copy_from_slice(b"NTFS    ");
        
        // Bytes per sector (512)
        boot_sector[0x0B] = 0x00;
        boot_sector[0x0C] = 0x02;
        
        // Sectors per cluster (8)
        boot_sector[0x0D] = 8;
        
        // Media descriptor (0xF8)
        boot_sector[0x15] = 0xF8;
        
        // Total sectors (1000000)
        let total_sectors = 1000000u64;
        boot_sector[0x28..0x30].copy_from_slice(&total_sectors.to_le_bytes());
        
        // MFT LCN (4)
        let mft_lcn = 4u64;
        boot_sector[0x30..0x38].copy_from_slice(&mft_lcn.to_le_bytes());
        
        // MFT mirror LCN (1000)
        let mftmirr_lcn = 1000u64;
        boot_sector[0x38..0x40].copy_from_slice(&mftmirr_lcn.to_le_bytes());
        
        // Clusters per MFT record (-10 = 1024 bytes)
        boot_sector[0x40] = 0xF6; // -10 in signed byte
        
        // Boot signature
        boot_sector[0x1FE] = 0x55;
        boot_sector[0x1FF] = 0xAA;
        
        // Test detection
        use crate::detection::FilesystemDetector;
        assert_eq!(NtfsDetector::detect(&boot_sector, None), Some("ntfs".to_string()));
    }
    
    #[test]
    fn test_boot_sector_validation() {
        use crate::ntfs::structures::NtfsBootSector;
        
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
        
        // Media descriptor (0xF8)
        data[0x15] = 0xF8;
        
        // Total sectors (1000000)
        let total_sectors = 1000000u64;
        data[0x28..0x30].copy_from_slice(&total_sectors.to_le_bytes());
        
        // MFT LCN (4)
        let mft_lcn = 4u64;
        data[0x30..0x38].copy_from_slice(&mft_lcn.to_le_bytes());
        
        // MFT mirror LCN (1000)
        let mftmirr_lcn = 1000u64;
        data[0x38..0x40].copy_from_slice(&mftmirr_lcn.to_le_bytes());
        
        // Clusters per MFT record (-10 = 1024 bytes)
        data[0x40] = 0xF6; // -10 in signed byte
        
        // Boot signature
        data[0x1FE] = 0x55;
        data[0x1FF] = 0xAA;
        
        // Parse and validate
        let boot_sector = unsafe {
            std::ptr::read_unaligned(data.as_ptr() as *const NtfsBootSector)
        };
        
        assert!(boot_sector.validate().is_ok());
        assert_eq!(boot_sector.bytes_per_cluster(), 4096); // 512 * 8
        assert_eq!(boot_sector.mft_record_size(), 1024); // 2^10
    }
    
    #[test]
    fn test_data_run_decoding() {
        use crate::ntfs::data_runs::decode_data_runs;
        
        // Single run: 16 clusters at LCN 100
        let data = vec![0x21, 0x10, 0x64, 0x00, 0x00];
        let runs = decode_data_runs(&data).unwrap();
        
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].lcn, Some(100));
        assert_eq!(runs[0].length, 16);
    }
    
    #[test]
    fn test_mft_fixup() {
        use crate::ntfs::mft::apply_fixup;
        
        // Create a mock MFT record with fixup
        let mut data = vec![0u8; 1024];
        
        // Set up USA at offset 0x30
        let usa_offset = 0x30;
        let usa_count = 3; // 1 USN + 2 fixup values
        
        // USN
        data[usa_offset] = 0x01;
        data[usa_offset + 1] = 0x00;
        
        // Original values for end of sectors
        data[usa_offset + 2] = 0xAA;
        data[usa_offset + 3] = 0xBB;
        data[usa_offset + 4] = 0xCC;
        data[usa_offset + 5] = 0xDD;
        
        // Place USN at end of sectors
        data[510] = 0x01;
        data[511] = 0x00;
        data[1022] = 0x01;
        data[1023] = 0x00;
        
        // Apply fixup
        apply_fixup(&mut data, usa_offset as u16, usa_count as u16).unwrap();
        
        // Check that fixup was applied
        assert_eq!(data[510], 0xAA);
        assert_eq!(data[511], 0xBB);
        assert_eq!(data[1022], 0xCC);
        assert_eq!(data[1023], 0xDD);
    }
    
    #[test]
    fn test_attribute_parsing() {
        use crate::ntfs::attributes::{parse_attribute, AttributeData};
        use crate::ntfs::structures::*;
        
        // Create a minimal FILE_NAME attribute
        let mut data = vec![0u8; 256];
        
        // Attribute header
        data[0..4].copy_from_slice(&ATTR_TYPE_FILE_NAME.to_le_bytes());
        data[4..8].copy_from_slice(&128u32.to_le_bytes()); // Record length
        data[8] = 0; // Resident
        data[9] = 0; // No name
        data[10..12].copy_from_slice(&0u16.to_le_bytes()); // Name offset
        data[12..14].copy_from_slice(&0u16.to_le_bytes()); // Flags
        data[14..16].copy_from_slice(&0u16.to_le_bytes()); // Attribute ID
        
        // Resident header
        data[16..20].copy_from_slice(&90u32.to_le_bytes()); // Value length
        data[20..22].copy_from_slice(&24u16.to_le_bytes()); // Value offset
        
        // FILE_NAME attribute value
        let value_offset = 24;
        data[value_offset..value_offset + 8].copy_from_slice(&5u64.to_le_bytes()); // Parent ref
        data[value_offset + 8..value_offset + 16].copy_from_slice(&0u64.to_le_bytes()); // Creation time
        data[value_offset + 16..value_offset + 24].copy_from_slice(&0u64.to_le_bytes()); // Modification time
        data[value_offset + 24..value_offset + 32].copy_from_slice(&0u64.to_le_bytes()); // MFT modification time
        data[value_offset + 32..value_offset + 40].copy_from_slice(&0u64.to_le_bytes()); // Access time
        data[value_offset + 40..value_offset + 48].copy_from_slice(&1024u64.to_le_bytes()); // Allocated size
        data[value_offset + 48..value_offset + 56].copy_from_slice(&512u64.to_le_bytes()); // Data size
        data[value_offset + 56..value_offset + 60].copy_from_slice(&0x20u32.to_le_bytes()); // File attributes
        data[value_offset + 60..value_offset + 64].copy_from_slice(&0u32.to_le_bytes()); // EA size
        data[value_offset + 64] = 4; // Name length (4 chars)
        data[value_offset + 65] = FILE_NAME_WIN32; // Name type
        
        // File name "test" in UTF-16LE
        data[value_offset + 66] = b't';
        data[value_offset + 67] = 0;
        data[value_offset + 68] = b'e';
        data[value_offset + 69] = 0;
        data[value_offset + 70] = b's';
        data[value_offset + 71] = 0;
        data[value_offset + 72] = b't';
        data[value_offset + 73] = 0;
        
        // Parse the attribute
        let (header, attr_data) = parse_attribute(&data, 0).unwrap();
        
        assert_eq!(header.type_code, ATTR_TYPE_FILE_NAME);
        assert_eq!(header.non_resident, 0);
        
        if let AttributeData::FileName(file_name_attr, name) = attr_data {
            assert_eq!(name, "test");
            assert_eq!(file_name_attr.data_size, 512);
            assert_eq!(file_name_attr.allocated_size, 1024);
        } else {
            panic!("Expected FileName attribute");
        }
    }
    
    #[test]
    fn test_filetime_conversion() {
        use crate::ntfs::structures::filetime_to_unix;
        
        // Test epoch conversion
        // Windows FILETIME for Unix epoch (1970-01-01 00:00:00)
        let unix_epoch_filetime = 116444736000000000u64;
        assert_eq!(filetime_to_unix(unix_epoch_filetime), 0);
        
        // Test a known date
        // 2024-01-01 00:00:00 UTC
        let jan_2024_filetime = 133477056000000000u64;
        let jan_2024_unix = 1704067200u64; // Unix timestamp for 2024-01-01
        assert_eq!(filetime_to_unix(jan_2024_filetime), jan_2024_unix);
    }
}