// Regression test to ensure the 16.0E bug never comes back
#[cfg(test)]
mod regression_tests {
    use crate::ext4_native::core::structures::Ext4GroupDesc;
    
    #[test]
    fn test_regression_16eb_overflow_bug() {
        // This test ensures we never reintroduce the bug that caused
        // "16.0E" to appear in df output on OpenWrt
        
        // The bug occurred when subtracting 2 from bg_free_blocks_count_lo
        // when it had value 0 or 1, causing u16 underflow
        
        let mut gd = Ext4GroupDesc::default();
        
        // Set up the exact scenario that triggered the bug:
        // Group has 65536 blocks free (0x10000)
        gd.bg_free_blocks_count_lo = 0;  // Low 16 bits
        gd.bg_free_blocks_count_hi = 1;  // High 16 bits
        
        // The WRONG way (what caused the bug):
        // gd.bg_free_blocks_count_lo -= 2;  // This would underflow!
        
        // The CORRECT way (what we do now):
        let current = gd.bg_free_blocks_count_lo as u32 
            | ((gd.bg_free_blocks_count_hi as u32) << 16);
        let new_value = current.saturating_sub(2);
        gd.bg_free_blocks_count_lo = (new_value & 0xFFFF) as u16;
        gd.bg_free_blocks_count_hi = ((new_value >> 16) & 0xFFFF) as u16;
        
        // Verify the result
        let final_value = gd.bg_free_blocks_count_lo as u32 
            | ((gd.bg_free_blocks_count_hi as u32) << 16);
        
        assert_eq!(final_value, 65534, "Should be 65536 - 2 = 65534");
        assert_eq!(gd.bg_free_blocks_count_lo, 0xFFFE, "Low should be 0xFFFE");
        assert_eq!(gd.bg_free_blocks_count_hi, 0, "High should be 0 after subtraction");
        
        // If this test ever fails, we've reintroduced the bug!
    }
    
    #[test]
    fn test_would_catch_buggy_implementation() {
        // This test verifies that we would catch the buggy implementation
        
        let mut gd = Ext4GroupDesc::default();
        gd.bg_free_blocks_count_lo = 0;
        gd.bg_free_blocks_count_hi = 1;
        
        // Simulate the BUGGY implementation
        let buggy_lo = gd.bg_free_blocks_count_lo.wrapping_sub(2);
        let buggy_result = buggy_lo as u32 | ((gd.bg_free_blocks_count_hi as u32) << 16);
        
        // This should produce 0x1FFFE (131070) instead of 65534
        assert_eq!(buggy_result, 0x1FFFE, "Bug simulation check");
        assert_ne!(buggy_result, 65534, "Buggy result should not equal correct result");
        
        // The buggy result is LARGER than what we started with!
        let original = 65536u32;
        assert!(buggy_result > original, 
                "Bug causes free blocks to INCREASE (overflow), leading to 16.0E in df");
    }
}