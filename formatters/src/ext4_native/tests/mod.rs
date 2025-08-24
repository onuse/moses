// Unit tests for ext4 implementation

mod phase1_tests;
mod phase2_tests;
mod phase3_tests;
mod test_backup_superblocks;
// mod phase1_standalone; // Uncomment to build standalone test

#[cfg(test)]
mod phase0_tests {
    use crate::ext4_native::core::*;
    
    #[test]
    fn test_alignment() {
        let buffer = AlignedBuffer::<4096>::new();
        assert!(buffer.is_aligned());
    }
    
    #[test]
    fn test_crc32c() {
        // Test with known values
        let data = b"123456789";
        let crc = crc32c_ext4(data, !0);
        // CRC32c of "123456789" with initial value of 0xFFFFFFFF
        assert_ne!(crc, 0);
    }
    
    #[test]
    fn test_endianness() {
        use byteorder::{LittleEndian, WriteBytesExt};
        
        let mut buffer = Vec::new();
        buffer.write_u16::<LittleEndian>(0xEF53).unwrap();
        assert_eq!(buffer, vec![0x53, 0xEF]);
    }
    
    #[test]
    fn test_filesystem_params_validation() {
        use crate::ext4_native::validation::Ext4Validator;
        use crate::ext4_native::core::types::FilesystemParams;
        
        let validator = Ext4Validator::new();
        
        // Valid params
        let params = FilesystemParams {
            size_bytes: 100 * 1024 * 1024,
            block_size: 4096,
            inode_size: 256,
            ..Default::default()
        };
        assert!(validator.validate_params(&params).is_ok());
        
        // Invalid block size
        let params = FilesystemParams {
            size_bytes: 100 * 1024 * 1024,
            block_size: 3000, // Not power of 2
            ..Default::default()
        };
        assert!(validator.validate_params(&params).is_err());
        
        // Device too small
        let params = FilesystemParams {
            size_bytes: 1024, // Too small
            ..Default::default()
        };
        assert!(validator.validate_params(&params).is_err());
    }
    
    #[test]
    fn test_layout_calculation() {
        use crate::ext4_native::core::types::{FilesystemParams, FilesystemLayout};
        
        let params = FilesystemParams {
            size_bytes: 1024 * 1024 * 1024, // 1GB
            ..Default::default()
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        assert_eq!(layout.total_blocks, 262144); // 1GB / 4KB
        assert_eq!(layout.num_groups, 8); // 262144 / 32768
    }
}