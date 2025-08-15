// Test for free blocks calculation overflow fix
#[cfg(test)]
use crate::ext4_native::core::types::{FilesystemLayout, FilesystemParams};

#[test]
fn test_free_blocks_calculation_no_overflow() {
    // Test case that previously caused overflow
    // 60GB drive = 15728640 blocks (with 4K blocks)
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
    
    println!("Test 60GB filesystem:");
    println!("  Total blocks: {}", layout.total_blocks);
    println!("  Blocks per group: {}", layout.blocks_per_group);
    println!("  Number of groups: {}", layout.num_groups);
    
    // Calculate last group size
    let last_group_idx = layout.num_groups - 1;
    let last_group_start = last_group_idx as u64 * layout.blocks_per_group as u64;
    let last_group_blocks = layout.total_blocks - last_group_start;
    
    println!("  Last group blocks: {} (vs full group: {})", 
             last_group_blocks, layout.blocks_per_group);
    
    // Simulate the corrected free blocks calculation
    let mut total_free_blocks = 0u64;
    
    for group_idx in 0..layout.num_groups {
        let group_start = group_idx as u64 * layout.blocks_per_group as u64;
        let blocks_in_group = if group_idx == layout.num_groups - 1 {
            (layout.total_blocks - group_start).min(layout.blocks_per_group as u64) as u32
        } else {
            layout.blocks_per_group
        };
        
        let metadata_blocks = layout.metadata_blocks_per_group(group_idx);
        let free_blocks = blocks_in_group.saturating_sub(metadata_blocks) as u64;
        
        total_free_blocks += free_blocks;
        
        if group_idx == 0 || group_idx == last_group_idx {
            println!("  Group {}: {} blocks, {} metadata, {} free", 
                     group_idx, blocks_in_group, metadata_blocks, free_blocks);
        }
    }
    
    println!("  Total free blocks: {}", total_free_blocks);
    println!("  Free percentage: {:.2}%", 
             (total_free_blocks as f64 / layout.total_blocks as f64) * 100.0);
    
    // The critical assertion - free blocks must not exceed total blocks
    assert!(total_free_blocks < layout.total_blocks,
            "Free blocks {} must be less than total blocks {}",
            total_free_blocks, layout.total_blocks);
    
    // Verify reasonable overhead (metadata should be 1-5% typically)
    let metadata_blocks = layout.total_blocks - total_free_blocks;
    let metadata_percentage = (metadata_blocks as f64 / layout.total_blocks as f64) * 100.0;
    
    println!("  Metadata blocks: {} ({:.2}%)", metadata_blocks, metadata_percentage);
    
    assert!(metadata_percentage > 0.5 && metadata_percentage < 10.0,
            "Metadata percentage {:.2}% seems unreasonable", metadata_percentage);
}

#[test]
fn test_various_drive_sizes() {
    let test_sizes = vec![
        (1, "1GB"),     // Small drive - 1 group
        (10, "10GB"),   // Medium drive - few groups  
        (60, "60GB"),   // Previously problematic size
        (100, "100GB"), // Larger drive
        (500, "500GB"), // Very large drive
    ];
    
    for (size_gb, label) in test_sizes {
        println!("\nTesting {} drive:", label);
        
        let params = FilesystemParams {
            size_bytes: size_gb * 1024 * 1024 * 1024,
            block_size: 4096,
            inode_size: 256,
            label: Some(label.to_string()),
            reserved_percent: 5,
            enable_checksums: true,
            enable_64bit: size_gb > 16,
            enable_journal: false,
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        // Calculate free blocks with corrected algorithm
        let mut total_free = 0u64;
        for group_idx in 0..layout.num_groups {
            let group_start = group_idx as u64 * layout.blocks_per_group as u64;
            let blocks_in_group = if group_idx == layout.num_groups - 1 {
                (layout.total_blocks - group_start).min(layout.blocks_per_group as u64) as u32
            } else {
                layout.blocks_per_group
            };
            
            let metadata = layout.metadata_blocks_per_group(group_idx);
            total_free += blocks_in_group.saturating_sub(metadata) as u64;
        }
        
        let metadata_pct = ((layout.total_blocks - total_free) as f64 / layout.total_blocks as f64) * 100.0;
        
        println!("  Total blocks: {}", layout.total_blocks);
        println!("  Free blocks: {}", total_free);
        println!("  Metadata: {:.2}%", metadata_pct);
        
        assert!(total_free < layout.total_blocks,
                "{}: Free blocks {} >= total blocks {}", 
                label, total_free, layout.total_blocks);
    }
}