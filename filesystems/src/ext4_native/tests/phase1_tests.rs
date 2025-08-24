// Phase 1 tests: Superblock creation and validation

#[cfg(test)]
mod tests {
    use crate::ext4_native::core::{*, structures::Ext4Superblock};
    use std::fs::File;
    use std::io::Write;
    
    #[test]
    fn test_superblock_size() {
        // Verify structure is exactly 1024 bytes
        assert_eq!(std::mem::size_of::<Ext4Superblock>(), 1024);
    }
    
    #[test]
    fn test_superblock_creation() {
        let params = FilesystemParams {
            size_bytes: 100 * 1024 * 1024, // 100MB
            block_size: 4096,
            inode_size: 256,
            label: Some("TestDrive".to_string()),
            reserved_percent: 5,
            enable_checksums: true,
            enable_64bit: true,
            enable_journal: false,
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        let mut sb = Ext4Superblock::new();
        sb.init_minimal(&params, &layout);
        
        // Validate basic fields
        assert_eq!(sb.s_magic, EXT4_SUPER_MAGIC);
        assert_eq!(sb.s_state, EXT4_VALID_FS);
        assert_eq!(sb.s_rev_level, EXT4_DYNAMIC_REV);
        assert_eq!(sb.s_log_block_size, 2); // 4096 = 1024 << 2
        assert_eq!(sb.s_first_ino, 11);
        assert_eq!(sb.s_inode_size, 256);
        
        // Check features
        assert!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_FILETYPE != 0);
        assert!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_EXTENTS != 0);
        assert!(sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_64BIT != 0);
        
        // Validate the structure
        assert!(sb.validate().is_ok());
    }
    
    #[test]
    fn test_superblock_checksum() {
        let params = FilesystemParams {
            size_bytes: 100 * 1024 * 1024,
            enable_checksums: true,
            ..Default::default()
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        let mut sb = Ext4Superblock::new();
        sb.init_minimal(&params, &layout);
        sb.update_checksum();
        
        // Checksum should be non-zero
        assert_ne!(sb.s_checksum, 0);
        
        // Verify checksum calculation is consistent
        let checksum1 = sb.s_checksum;
        sb.s_checksum = 0;
        sb.update_checksum();
        let checksum2 = sb.s_checksum;
        
        assert_eq!(checksum1, checksum2);
    }
    
    #[test]
    fn test_superblock_write_to_buffer() {
        let params = FilesystemParams {
            size_bytes: 100 * 1024 * 1024,
            label: Some("Test".to_string()),
            ..Default::default()
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        let mut sb = Ext4Superblock::new();
        sb.init_minimal(&params, &layout);
        sb.update_checksum();
        
        // Write to buffer
        let mut buffer = vec![0u8; 2048];
        sb.write_to_buffer(&mut buffer[1024..]).unwrap();
        
        // Check magic at correct offset (0x438 = 1024 + 0x38)
        assert_eq!(buffer[1024 + 0x38], 0x53);
        assert_eq!(buffer[1024 + 0x39], 0xEF);
        
        // Check state at offset 0x43A
        assert_eq!(buffer[1024 + 0x3A], 0x01); // EXT4_VALID_FS
        assert_eq!(buffer[1024 + 0x3B], 0x00);
    }
    
    #[test]
    fn test_create_minimal_image() {
        // Create a minimal image with just superblock
        let params = FilesystemParams {
            size_bytes: 10 * 1024 * 1024, // 10MB minimum
            block_size: 4096,
            inode_size: 256,
            label: Some("Phase1Test".to_string()),
            enable_checksums: true,
            enable_64bit: false, // Keep it simple for now
            ..Default::default()
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        let mut sb = Ext4Superblock::new();
        sb.init_minimal(&params, &layout);
        sb.update_checksum();
        
        // Create a test image file
        let test_file = "test_phase1.img";
        
        // Create minimal image (just first 4KB with superblock)
        let mut buffer = AlignedBuffer::<4096>::new();
        
        // Superblock at offset 1024
        sb.write_to_buffer(&mut buffer[1024..]).unwrap();
        
        // Write to file
        let mut file = File::create(test_file).unwrap();
        file.write_all(&buffer[..]).unwrap();
        file.sync_all().unwrap();
        
        // Now try to read it back and validate
        let mut file = File::open(test_file).unwrap();
        let mut read_buffer = vec![0u8; 4096];
        use std::io::Read;
        file.read_exact(&mut read_buffer).unwrap();
        
        // Check magic
        assert_eq!(read_buffer[1024 + 0x38], 0x53);
        assert_eq!(read_buffer[1024 + 0x39], 0xEF);
        
        // Clean up
        std::fs::remove_file(test_file).unwrap();
    }
    
    fn dump_superblock_hex(sb: &Ext4Superblock) {
        let mut buffer = vec![0u8; 1024];
        sb.write_to_buffer(&mut buffer).unwrap();
        
        println!("\n=== Superblock Hex Dump ===");
        for (i, chunk) in buffer.chunks(16).enumerate() {
            print!("{:04X}: ", i * 16);
            for byte in chunk {
                print!("{:02X} ", byte);
            }
            print!("  ");
            for byte in chunk {
                if *byte >= 0x20 && *byte < 0x7F {
                    print!("{}", *byte as char);
                } else {
                    print!(".");
                }
            }
            println!();
            
            // Just show first few lines for brevity
            if i >= 10 {
                println!("...");
                break;
            }
        }
        
        // Show critical offsets
        println!("\nCritical fields:");
        println!("  Magic (0x038): {:02X} {:02X}", buffer[0x38], buffer[0x39]);
        println!("  State (0x03A): {:02X} {:02X}", buffer[0x3A], buffer[0x3B]);
        println!("  Rev level (0x04C): {:02X} {:02X} {:02X} {:02X}", 
                 buffer[0x4C], buffer[0x4D], buffer[0x4E], buffer[0x4F]);
        println!("  Checksum (0x3FC): {:02X} {:02X} {:02X} {:02X}",
                 buffer[0x3FC], buffer[0x3FD], buffer[0x3FE], buffer[0x3FF]);
    }
    
    #[test]
    fn test_superblock_hex_dump() {
        let params = FilesystemParams {
            size_bytes: 100 * 1024 * 1024,
            label: Some("HexTest".to_string()),
            enable_checksums: true,
            ..Default::default()
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        let mut sb = Ext4Superblock::new();
        sb.init_minimal(&params, &layout);
        sb.update_checksum();
        
        dump_superblock_hex(&sb);
    }
}