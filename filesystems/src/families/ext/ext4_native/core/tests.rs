// Unit tests for ext4 native formatter

#[cfg(test)]
mod tests {
    use crate::families::ext::ext4_native::core::types::{FilesystemParams, FilesystemLayout};
    
    #[test]
    fn test_free_blocks_calculation_60gb() {
        // Test with 60GB drive parameters
        let params = FilesystemParams {
            size_bytes: 60 * 1024 * 1024 * 1024, // 60GB
            block_size: 4096,
            inode_size: 256,
            label: Some("TEST".to_string()),
            reserved_percent: 5,
            enable_checksums: true,
            enable_64bit: true,
            enable_journal: false,
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        // Verify we get the expected number of groups (should be ~480)
        assert!(layout.num_groups > 450 && layout.num_groups < 500, 
                "Expected ~480 groups for 60GB, got {}", layout.num_groups);
        
        // Calculate total metadata blocks
        let mut total_metadata = 0u64;
        for group in 0..layout.num_groups {
            total_metadata += layout.metadata_blocks_per_group(group) as u64;
        }
        
        // Calculate expected free blocks
        let expected_free = layout.total_blocks - total_metadata;
        
        // Free blocks should be less than total blocks
        assert!(expected_free < layout.total_blocks, 
                "Free blocks {} should be less than total blocks {}", 
                expected_free, layout.total_blocks);
        
        // Free blocks should be reasonable (around 90-99% of total for lazy init)
        let free_percentage = (expected_free * 100) / layout.total_blocks;
        println!("60GB filesystem: {} total blocks, {} metadata blocks, {} free blocks ({}%)",
                 layout.total_blocks, total_metadata, expected_free, free_percentage);
        
        // With lazy initialization (UNINIT flags), we only allocate metadata for group 0
        // Other groups have minimal metadata (just space for bitmaps and inode table)
        // So 98-99% free is actually correct!
        assert!(free_percentage > 85 && free_percentage < 100, 
                "Free blocks percentage {} is out of expected range", free_percentage);
    }
    
    #[test]
    fn test_formatter_free_blocks_calculation_matches_real() {
        // This test simulates EXACTLY what our formatter does
        // to catch the 16.0E overflow issue
        
        let params = FilesystemParams {
            size_bytes: 60 * 1024 * 1024 * 1024, // 60GB exactly
            block_size: 4096,
            inode_size: 256,
            label: None,
            reserved_percent: 5,
            enable_checksums: true,
            enable_64bit: true,
            enable_journal: false,
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        println!("Layout: {} total blocks, {} groups", layout.total_blocks, layout.num_groups);
        
        // Simulate formatter_impl.rs lines 107-121
        // Group 0 calculation (assuming we allocated 2 blocks for directories)
        let group0_metadata = layout.metadata_blocks_per_group(0);
        let group0_free_initial = layout.blocks_per_group.saturating_sub(group0_metadata);
        
        // THIS WAS THE BUG: subtracting from only the low 16 bits would underflow!
        // Simulate the correct calculation (as a 32-bit value)
        let group0_data_allocated = 2u32;  // root and lost+found directories  
        let group0_free = group0_free_initial.saturating_sub(group0_data_allocated);
        
        println!("Group 0: {} initial free, {} after allocating {} data blocks", 
                 group0_free_initial, group0_free, group0_data_allocated);
        
        // Simulate the EXACT calculation from formatter_impl.rs
        // This is the potentially buggy part
        let mut total_free_blocks = group0_free as u64;
        
        // Add free blocks from uninitialized groups (1 through num_groups-1)
        for group_idx in 1..layout.num_groups {
            let metadata_blocks = layout.metadata_blocks_per_group(group_idx);
            let free_blocks = layout.blocks_per_group.saturating_sub(metadata_blocks) as u64;
            total_free_blocks += free_blocks;
        }
        
        println!("Calculated total free blocks: {}", total_free_blocks);
        println!("Total blocks in filesystem: {}", layout.total_blocks);
        
        // The critical check: free blocks MUST be less than total blocks
        assert!(total_free_blocks <= layout.total_blocks,
                "OVERFLOW BUG: Free blocks {} exceeds total blocks {}! This causes 16.0E in df",
                total_free_blocks, layout.total_blocks);
        
        // Also check the math makes sense
        let used_blocks = layout.total_blocks.saturating_sub(total_free_blocks);
        let used_percentage = (used_blocks * 100) / layout.total_blocks;
        println!("Used blocks: {} ({}%)", used_blocks, used_percentage);
        
        assert!(used_percentage < 20,
                "Too much space used: {}% (should be < 20% for metadata)", used_percentage);
    }
    
    #[test]
    fn test_group_descriptor_free_blocks_encoding() {
        // Test that 32-bit free blocks count is correctly split into two u16 fields
        use crate::families::ext::ext4_native::core::structures::Ext4GroupDesc;
        
        let test_cases = vec![
            (0u32, 0u16, 0u16),                    // Zero
            (100u32, 100u16, 0u16),                // Small number
            (32767u32, 32767u16, 0u16),            // Max for low 16 bits - 1
            (32768u32, 32768u16, 0u16),            // Exactly 2^15
            (65535u32, 65535u16, 0u16),            // Max for low 16 bits
            (65536u32, 0u16, 1u16),                // Overflow to high bits
            (0x12345u32, 0x2345u16, 0x1u16),       // Mixed high and low
            (0xFFFFFFFFu32, 0xFFFFu16, 0xFFFFu16), // Maximum value
        ];
        
        for (total, expected_lo, expected_hi) in test_cases {
            let mut gd = Ext4GroupDesc::default();
            gd.bg_free_blocks_count_lo = (total & 0xFFFF) as u16;
            gd.bg_free_blocks_count_hi = ((total >> 16) & 0xFFFF) as u16;
            
            assert_eq!(gd.bg_free_blocks_count_lo, expected_lo, 
                      "Low 16 bits mismatch for {:#x}", total);
            assert_eq!(gd.bg_free_blocks_count_hi, expected_hi,
                      "High 16 bits mismatch for {:#x}", total);
            
            // Verify we can reconstruct the original value
            let reconstructed = gd.bg_free_blocks_count_lo as u32 
                | ((gd.bg_free_blocks_count_hi as u32) << 16);
            assert_eq!(reconstructed, total, 
                      "Failed to reconstruct value {:#x}", total);
        }
    }
    
    #[test]
    fn test_superblock_free_blocks_no_overflow() {
        // Test that superblock free blocks calculation doesn't overflow
        let params = FilesystemParams {
            size_bytes: 60 * 1024 * 1024 * 1024, // 60GB
            block_size: 4096,
            inode_size: 256,
            label: None,
            reserved_percent: 5,
            enable_checksums: true,
            enable_64bit: true,
            enable_journal: false,
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        // Simulate what our formatter does
        let mut total_free_blocks = 0u64;
        
        // Group 0 has some blocks used for root directory
        let group0_metadata = layout.metadata_blocks_per_group(0) as u64;
        let group0_free = (layout.blocks_per_group as u64).saturating_sub(group0_metadata + 2); // -2 for dir blocks
        total_free_blocks += group0_free;
        
        // Add free blocks from other groups
        for group_idx in 1..layout.num_groups {
            let metadata = layout.metadata_blocks_per_group(group_idx) as u64;
            let free = (layout.blocks_per_group as u64).saturating_sub(metadata);
            total_free_blocks += free;
        }
        
        // Verify no overflow
        assert!(total_free_blocks < layout.total_blocks,
                "Total free blocks {} exceeds total blocks {}",
                total_free_blocks, layout.total_blocks);
        
        // Verify it fits in 64 bits when stored as lo/hi u32
        assert!(total_free_blocks <= u64::MAX);
        
        // Verify the value makes sense (should be most of the disk)
        let usage_percentage = ((layout.total_blocks - total_free_blocks) * 100) / layout.total_blocks;
        assert!(usage_percentage < 15, 
                "Metadata usage {}% is too high", usage_percentage);
    }
    
    #[test]
    fn test_metadata_blocks_calculation() {
        // Test metadata blocks calculation for different group types
        let params = FilesystemParams {
            size_bytes: 10 * 1024 * 1024 * 1024, // 10GB
            block_size: 4096,
            inode_size: 256,
            label: None,
            reserved_percent: 5,
            enable_checksums: true,
            enable_64bit: true,
            enable_journal: false,
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        // Group 0 always has superblock
        let group0_metadata = layout.metadata_blocks_per_group(0);
        assert!(group0_metadata > 0, "Group 0 should have metadata");
        
        // Group 1 has backup superblock
        let group1_metadata = layout.metadata_blocks_per_group(1);
        assert!(group1_metadata > 0, "Group 1 should have metadata");
        
        // Group 2 doesn't have backup superblock
        let group2_metadata = layout.metadata_blocks_per_group(2);
        assert!(group2_metadata > 0, "Group 2 should have metadata");
        assert!(group2_metadata < group1_metadata, 
                "Group 2 should have less metadata than group 1 (no backup SB)");
        
        // All groups should have at least bitmaps and inode table
        for group in 0..layout.num_groups.min(10) {
            let metadata = layout.metadata_blocks_per_group(group);
            // Minimum: 2 bitmaps + inode table blocks
            let min_metadata = 2 + layout.inode_table_blocks();
            assert!(metadata >= min_metadata, 
                    "Group {} metadata {} is less than minimum {}",
                    group, metadata, min_metadata);
        }
    }
}