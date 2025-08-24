// Test that backup superblocks are written correctly

#[cfg(test)]
mod tests {
    use crate::ext4_native::core::{
        structures::*,
        types::{FilesystemParams, FilesystemLayout},
        formatter_impl::format_device,
    };
    use moses_core::{Device, DeviceType, FormatOptions};
    use std::fs::{File, remove_file};
    use std::io::{Read, Seek, SeekFrom};
    use tempfile::NamedTempFile;
    
    #[tokio::test]
    async fn test_backup_superblocks_written() {
        // Create a test image file (256MB to have multiple block groups)
        let test_file = NamedTempFile::new().unwrap();
        let test_path = test_file.path().to_str().unwrap().to_string();
        
        // Create 256MB file
        let size = 256 * 1024 * 1024;
        test_file.as_file().set_len(size).unwrap();
        
        // Create device descriptor
        let device = Device {
            id: test_path.clone(),
            name: "test_device".to_string(),
            size,
            device_type: DeviceType::Unknown,
            is_removable: true,
            is_system: false,
            mount_points: vec![],
        };
        
        // Format options
        let options = FormatOptions {
            filesystem: "ext4".to_string(),
            label: Some("TEST".to_string()),
            cluster_size: Some(4096),
            quick_format: true,
            enable_compression: false,
            enable_encryption: false,
            verify_after_format: false,
        };
        
        // Format the device
        format_device(&device, &options).await.unwrap();
        
        // Now verify backup superblocks exist
        let mut file = File::open(&test_path).unwrap();
        
        // Calculate expected backup locations
        let params = FilesystemParams {
            size_bytes: size,
            block_size: 4096,
            inode_size: 256,
            label: Some("TEST".to_string()),
            reserved_percent: 5,
            enable_checksums: true,
            enable_64bit: false,
            enable_journal: false,
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        // Check backup superblocks at groups 1, 3, 5, 7, 9 (if they exist)
        for group in &[1, 3, 5, 7, 9] {
            if *group >= layout.num_groups {
                continue;
            }
            
            if !layout.has_superblock(*group) {
                continue;
            }
            
            // Read superblock from backup location
            let backup_offset = *group as u64 * layout.blocks_per_group as u64 * 4096;
            file.seek(SeekFrom::Start(backup_offset + 1024)).unwrap();
            
            let mut magic_bytes = [0u8; 2];
            file.read_exact(&mut magic_bytes).unwrap();
            
            // Check magic number (0xEF53 in little-endian)
            assert_eq!(magic_bytes[0], 0x53, "Missing backup superblock at group {}", group);
            assert_eq!(magic_bytes[1], 0xEF, "Invalid magic at group {}", group);
            
            // Read block group number
            file.seek(SeekFrom::Start(backup_offset + 1024 + 0x5A)).unwrap();
            let mut bg_bytes = [0u8; 2];
            file.read_exact(&mut bg_bytes).unwrap();
            let bg_num = u16::from_le_bytes(bg_bytes);
            
            assert_eq!(bg_num, *group as u16, 
                "Wrong block group number in backup at group {}: found {}", 
                group, bg_num);
            
            println!("✓ Backup superblock verified at group {}", group);
        }
        
        // Clean up
        drop(file);
        remove_file(test_path).ok();
    }
    
    #[test]
    fn test_sparse_super_groups() {
        // Test that has_superblock returns correct groups
        let params = FilesystemParams {
            size_bytes: 1024 * 1024 * 1024, // 1GB
            block_size: 4096,
            inode_size: 256,
            label: None,
            reserved_percent: 5,
            enable_checksums: true,
            enable_64bit: false,
            enable_journal: false,
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        // Groups that should have backups (0, 1, and powers of 3, 5, 7)
        assert!(layout.has_superblock(0), "Group 0 must have superblock");
        assert!(layout.has_superblock(1), "Group 1 must have superblock");
        assert!(layout.has_superblock(3), "Group 3 must have superblock");
        assert!(layout.has_superblock(5), "Group 5 must have superblock");
        assert!(layout.has_superblock(7), "Group 7 must have superblock");
        assert!(layout.has_superblock(9), "Group 9 (3^2) must have superblock");
        
        // Groups that should NOT have backups
        assert!(!layout.has_superblock(2), "Group 2 should not have superblock");
        assert!(!layout.has_superblock(4), "Group 4 should not have superblock");
        assert!(!layout.has_superblock(6), "Group 6 should not have superblock");
        assert!(!layout.has_superblock(8), "Group 8 should not have superblock");
    }
}