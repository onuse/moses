// Specific test for the 16.0E overflow bug
#[cfg(test)]
mod overflow_tests {
    use crate::families::ext::ext4_native::core::structures::Ext4GroupDesc;
    
    #[test]
    fn test_u16_underflow_bug_that_causes_16eb() {
        // This test reproduces the EXACT bug that caused 16.0E in df
        
        // Case 1: When free blocks count has 0 in low 16 bits
        let mut gd = Ext4GroupDesc::default();
        gd.bg_free_blocks_count_lo = 0;      // 0x0000
        gd.bg_free_blocks_count_hi = 1;      // 0x0001
        // Total: 0x00010000 = 65536 blocks free
        
        // WRONG WAY (the bug): Subtract only from low 16 bits
        let buggy_lo = gd.bg_free_blocks_count_lo.wrapping_sub(2);  // 0 - 2 = 0xFFFE
        let buggy_total = buggy_lo as u32 | ((gd.bg_free_blocks_count_hi as u32) << 16);
        // Result: 0x0001FFFE = 131070 (WRONG! More than we started with!)
        
        // RIGHT WAY: Handle as 32-bit value
        let correct_total_before = gd.bg_free_blocks_count_lo as u32 
            | ((gd.bg_free_blocks_count_hi as u32) << 16);
        let correct_total_after = correct_total_before.saturating_sub(2);
        // Result: 0x0000FFFE = 65534 (Correct!)
        
        println!("Case 1: Starting with 0x{:08X} blocks free", correct_total_before);
        println!("  Buggy result:   0x{:08X} ({} blocks) - OVERFLOW!", buggy_total, buggy_total);
        println!("  Correct result: 0x{:08X} ({} blocks)", correct_total_after, correct_total_after);
        
        assert_ne!(buggy_total, correct_total_after, "Bug detection failed");
        assert!(buggy_total > correct_total_before, "Bug should cause overflow");
        assert!(correct_total_after < correct_total_before, "Correct should decrease");
    }
    
    #[test]
    fn test_multiple_edge_cases_for_u16_split() {
        let test_cases = vec![
            // (lo, hi, subtract, description)
            (0u16, 1u16, 2, "Zero in low bits - main trigger"),
            (1u16, 1u16, 2, "One in low bits - also triggers"),  
            (0xFFFF, 0, 1, "Max low bits, zero high"),
            (0, 0, 1, "Both zero - should saturate"),
            (100, 0, 200, "Subtract more than available in low"),
        ];
        
        for (lo, hi, subtract, desc) in test_cases {
            println!("\nTesting: {}", desc);
            
            // Buggy way (direct subtraction from lo)
            let buggy_lo = lo.wrapping_sub(subtract);
            let buggy_total = buggy_lo as u32 | ((hi as u32) << 16);
            
            // Correct way
            let correct_total_before = lo as u32 | ((hi as u32) << 16);
            let correct_total_after = correct_total_before.saturating_sub(subtract as u32);
            
            println!("  Initial: lo={:#06X} hi={:#06X} total={:#010X} ({})", 
                     lo, hi, correct_total_before, correct_total_before);
            println!("  Subtract: {}", subtract);
            println!("  Buggy:   total={:#010X} ({}) {}", 
                     buggy_total, buggy_total,
                     if buggy_total > correct_total_before { "OVERFLOW!" } else { "ok" });
            println!("  Correct: total={:#010X} ({})", 
                     correct_total_after, correct_total_after);
            
            // The correct value should never exceed the original
            assert!(correct_total_after <= correct_total_before,
                    "{}: Correct calculation should not increase value", desc);
        }
    }
    
    #[test]
    fn test_60gb_filesystem_free_blocks_calculation() {
        // Test with actual 60GB filesystem parameters
        
        // From our layout calculation:
        // - blocks_per_group = 32768
        // - Group 0 metadata blocks ≈ 515 (superblock + GDT + bitmaps + inode table)
        // - Free blocks in group 0 = 32768 - 515 = 32253
        
        let metadata = 515u32;
        let blocks_per_group = 32768u32;
        let initial_free = blocks_per_group - metadata; // 32253
        
        // Convert to u16 split
        let lo = (initial_free & 0xFFFF) as u16;  // 32253 & 0xFFFF = 32253 (0x7DFD)
        let hi = ((initial_free >> 16) & 0xFFFF) as u16;  // 0
        
        println!("Group 0 initial free: {} (lo={:#06X}, hi={:#06X})", initial_free, lo, hi);
        
        // Now subtract 2 for directory blocks
        
        // BUGGY WAY (what caused 16.0E):
        let buggy_lo = lo.wrapping_sub(2);  // 32253 - 2 = 32251 (0x7DFB) - this is OK
        let buggy_free = buggy_lo as u32 | ((hi as u32) << 16);
        
        // CORRECT WAY:
        let correct_free = initial_free.saturating_sub(2);
        
        println!("After subtracting 2 directory blocks:");
        println!("  Buggy:   {} blocks", buggy_free);
        println!("  Correct: {} blocks", correct_free);
        
        // In this case both should be the same because lo > 2
        assert_eq!(buggy_free, correct_free, 
                   "For group 0 with {} free blocks, both methods should work", initial_free);
                   
        // But now test when metadata leaves exactly 65536 blocks free (0x10000)
        // This would have lo=0, hi=1
        // Actually test with a group that has lots of metadata
        let edge_case_free = 65536u32;  // 0x10000
        let edge_lo = (edge_case_free & 0xFFFF) as u16;  // 0
        let edge_hi = ((edge_case_free >> 16) & 0xFFFF) as u16;  // 1
        
        println!("\nEdge case: {} blocks free (lo={:#06X}, hi={:#06X})", 
                 edge_case_free, edge_lo, edge_hi);
        
        // Subtract 2
        let edge_buggy_lo = edge_lo.wrapping_sub(2);  // 0 - 2 = 0xFFFE
        let edge_buggy_free = edge_buggy_lo as u32 | ((edge_hi as u32) << 16);  // 0x1FFFE = 131070
        let edge_correct_free = edge_case_free.saturating_sub(2);  // 65534
        
        println!("After subtracting 2:");
        println!("  Buggy:   {} blocks (0x{:08X}) - OVERFLOW by {}!", 
                 edge_buggy_free, edge_buggy_free, edge_buggy_free - edge_case_free);
        println!("  Correct: {} blocks (0x{:08X})", edge_correct_free, edge_correct_free);
        
        assert!(edge_buggy_free > edge_case_free, "Bug should cause overflow");
        assert_eq!(edge_correct_free, 65534, "Correct should be 65534");
        
        // This overflow would make df show:
        // Total - Available = Used
        // If Available > Total (due to overflow), Used becomes negative, wrapping to 16.0E!
    }
    
    #[test] 
    fn test_superblock_total_calculation_with_overflow() {
        // Simulate the full calculation that leads to 16.0E in df
        
        let total_blocks = 15728640u64;  // 60GB drive
        let blocks_per_group = 32768u32;
        let num_groups = 480u32;
        
        // Simulate if group 0 had the underflow bug
        // Say group 0 metadata uses all but 1 block, then we subtract 2
        let group0_free_buggy = 1u32.wrapping_sub(2);  // Underflow to 0xFFFFFFFF
        
        // Calculate total free with the bug
        let mut total_free_buggy = group0_free_buggy as u64;
        
        // Add other groups (they're fine)
        for _g in 1..num_groups {
            total_free_buggy += (blocks_per_group - 515) as u64;  // Assume 515 metadata blocks
        }
        
        println!("With u16 underflow bug:");
        println!("  Total blocks: {}", total_blocks);
        println!("  Total free (buggy): {}", total_free_buggy);
        
        if total_free_buggy > total_blocks {
            let overflow_used = (total_blocks as i64) - (total_free_buggy as i64);
            let overflow_used_unsigned = overflow_used as u64;  // Wraps to huge number
            
            println!("  Used (signed): {} blocks", overflow_used);
            println!("  Used (unsigned): {} blocks", overflow_used_unsigned);
            println!("  Used in EB: {:.1} EB", (overflow_used_unsigned as f64) * 4096.0 / 1e18);
            
            // This is what causes 16.0E!
            assert!(overflow_used < 0, "Should be negative (underflow)");
            assert!(overflow_used_unsigned > 1e15 as u64, "Should wrap to huge number");
        }
    }
}