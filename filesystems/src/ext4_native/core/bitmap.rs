// Bitmap management for ext4 block and inode allocation

use crate::ext4_native::core::{
    types::*,
    alignment::AlignedBuffer,
};

/// Bitmap for tracking block or inode allocation
pub struct Bitmap {
    data: Vec<u8>,
    size_bits: u32,
}

impl Bitmap {
    /// Create a new bitmap with the specified number of bits
    pub fn new(size_bits: u32) -> Self {
        let size_bytes = (size_bits + 7) / 8;
        Self {
            data: vec![0u8; size_bytes as usize],
            size_bits,
        }
    }
    
    /// Create a bitmap for blocks in a group
    pub fn for_block_group(blocks_per_group: u32) -> Self {
        Self::new(blocks_per_group)
    }
    
    /// Create a bitmap for inodes in a group
    pub fn for_inode_group(inodes_per_group: u32) -> Self {
        Self::new(inodes_per_group)
    }
    
    /// Set a bit (mark as used)
    pub fn set(&mut self, index: u32) {
        if index >= self.size_bits {
            return;
        }
        let byte_index = (index / 8) as usize;
        let bit_index = (index % 8) as u8;
        self.data[byte_index] |= 1 << bit_index;
    }
    
    /// Clear a bit (mark as free)
    pub fn clear(&mut self, index: u32) {
        if index >= self.size_bits {
            return;
        }
        let byte_index = (index / 8) as usize;
        let bit_index = (index % 8) as u8;
        self.data[byte_index] &= !(1 << bit_index);
    }
    
    /// Check if a bit is set
    pub fn is_set(&self, index: u32) -> bool {
        if index >= self.size_bits {
            return false;
        }
        let byte_index = (index / 8) as usize;
        let bit_index = (index % 8) as u8;
        (self.data[byte_index] & (1 << bit_index)) != 0
    }
    
    /// Set a range of bits
    pub fn set_range(&mut self, start: u32, count: u32) {
        for i in start..start.saturating_add(count).min(self.size_bits) {
            self.set(i);
        }
    }
    
    /// Count free bits
    pub fn count_free(&self) -> u32 {
        let mut free = 0;
        for i in 0..self.size_bits {
            if !self.is_set(i) {
                free += 1;
            }
        }
        free
    }
    
    /// Find contiguous clear bits
    pub fn find_contiguous_clear(&self, start: u32, count: u32) -> Option<u32> {
        let total_bits = self.size_bits as usize;
        
        for offset in 0..total_bits {
            let bit = (start as usize + offset) % total_bits;
            let mut found = true;
            
            for i in 0..count as usize {
                let check_bit = (bit + i) % total_bits;
                if self.is_set(check_bit as u32) {
                    found = false;
                    break;
                }
            }
            
            if found {
                return Some(bit as u32);
            }
        }
        
        None
    }
    
    /// Get bitmap data as bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
    
    /// Write bitmap to an aligned buffer
    pub fn write_to_buffer<const N: usize>(&self, buffer: &mut AlignedBuffer<N>) -> Result<(), String> {
        let required_size = self.data.len();
        if N < required_size {
            return Err(format!("Buffer too small: {} < {}", N, required_size));
        }
        
        buffer[..required_size].copy_from_slice(&self.data);
        
        // Set padding bits at the end of the bitmap to 1
        // This is required by ext4 for unused bits in the last byte
        if self.size_bits % 8 != 0 {
            let last_byte_index = (self.size_bits / 8) as usize;
            let used_bits = (self.size_bits % 8) as u8;
            let padding_mask = !((1u8 << used_bits) - 1);
            buffer[last_byte_index] |= padding_mask;
        }
        
        // Also set all bytes after the bitmap to 0xFF (all bits set)
        // This is what ext4 expects for padding
        if required_size < N {
            buffer[required_size..].fill(0xFF);
        }
        
        Ok(())
    }
}

/// Initialize block bitmap for the first block group
pub fn init_block_bitmap_group0(
    bitmap: &mut Bitmap,
    layout: &FilesystemLayout,
    params: &FilesystemParams,
) {
    let mut current_block = 0u32;
    
    // Boot block (if 1K block size)
    if params.block_size == 1024 {
        bitmap.set(0); // Boot block
        current_block = 1;
    }
    
    // Superblock
    bitmap.set(current_block);
    current_block += 1;
    
    // Group descriptor table
    let gdt_blocks = layout.gdt_blocks();
    bitmap.set_range(current_block, gdt_blocks);
    current_block += gdt_blocks;
    
    // Reserved GDT blocks
    bitmap.set_range(current_block, layout.reserved_gdt_blocks);
    current_block += layout.reserved_gdt_blocks;
    
    // Block bitmap itself
    bitmap.set(current_block);
    current_block += 1;
    
    // Inode bitmap
    bitmap.set(current_block);
    current_block += 1;
    
    // Inode table
    let inode_table_blocks = layout.inode_table_blocks();
    bitmap.set_range(current_block, inode_table_blocks);
    // Note: current_block is not used after this, but keeping for clarity
    
    // Mark blocks beyond the filesystem size as used
    // This is required for proper padding in incomplete block groups
    if layout.total_blocks < layout.blocks_per_group as u64 {
        for block in layout.total_blocks as u32..layout.blocks_per_group {
            bitmap.set(block);
        }
    }
    
    // The rest are free (will allocate for root directory later)
}

/// Initialize inode bitmap for the first block group
pub fn init_inode_bitmap_group0(bitmap: &mut Bitmap) {
    // Reserved inodes 1-10
    // Mark reserved inodes 1-10 as used
    // In the bitmap: bit 0 = inode 1, bit 1 = inode 2, ..., bit 9 = inode 10
    for i in 0..10 {
        bitmap.set(i);
    }
    
    // Inode 11 (lost+found) will be marked as used separately in the test
    // The rest are free
}
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bitmap_operations() {
        let mut bitmap = Bitmap::new(100);
        
        // Initially all free
        assert_eq!(bitmap.count_free(), 100);
        
        // Set some bits
        bitmap.set(0);
        bitmap.set(10);
        bitmap.set(99);
        
        assert!(bitmap.is_set(0));
        assert!(bitmap.is_set(10));
        assert!(bitmap.is_set(99));
        assert!(!bitmap.is_set(50));
        
        assert_eq!(bitmap.count_free(), 97);
        
        // Set range
        bitmap.set_range(20, 10);
        for i in 20..30 {
            assert!(bitmap.is_set(i));
        }
        
        assert_eq!(bitmap.count_free(), 87);
    }
    
    #[test]
    fn test_bitmap_to_buffer() {
        let mut bitmap = Bitmap::new(32);
        bitmap.set(0);
        bitmap.set(7);
        bitmap.set(8);
        bitmap.set(15);
        
        let mut buffer = AlignedBuffer::<64>::new();
        bitmap.write_to_buffer(&mut buffer).unwrap();
        
        // Check the bytes
        assert_eq!(buffer[0], 0b10000001); // Bits 0 and 7 set
        assert_eq!(buffer[1], 0b10000001); // Bits 8 and 15 set
        assert_eq!(buffer[2], 0);
        assert_eq!(buffer[3], 0);
    }
}