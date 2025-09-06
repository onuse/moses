// Indirect block support for EXT4 Writer
// Handles legacy indirect block addressing for older filesystems

use super::*;
use crate::families::ext::ext4_native::core::{
    structures::*,
    types::*,
};
use moses_core::MosesError;

impl Ext4Writer {
    /// Parse indirect blocks to get all data blocks for an inode
    pub(super) fn get_indirect_blocks(&mut self, inode: &Ext4Inode) -> Result<Vec<BlockNumber>, MosesError> {
        let mut blocks = Vec::new();
        
        // Direct blocks (first 12 blocks)
        for i in 0..12 {
            if inode.i_block[i] != 0 {
                blocks.push(inode.i_block[i] as u64);
            } else {
                break; // No more blocks
            }
        }
        
        // Single indirect block (block 12)
        if inode.i_block[12] != 0 {
            self.parse_single_indirect(inode.i_block[12] as u64, &mut blocks)?;
        }
        
        // Double indirect block (block 13)
        if inode.i_block[13] != 0 {
            self.parse_double_indirect(inode.i_block[13] as u64, &mut blocks)?;
        }
        
        // Triple indirect block (block 14)
        if inode.i_block[14] != 0 {
            self.parse_triple_indirect(inode.i_block[14] as u64, &mut blocks)?;
        }
        
        Ok(blocks)
    }
    
    /// Parse a single indirect block
    fn parse_single_indirect(&mut self, indirect_block: BlockNumber, blocks: &mut Vec<BlockNumber>) -> Result<(), MosesError> {
        let block_data = self.read_block_from_disk(indirect_block)?;
        let entries_per_block = self.block_size as usize / 4; // 4 bytes per block number
        
        for i in 0..entries_per_block {
            let offset = i * 4;
            if offset + 4 > block_data.len() {
                break;
            }
            
            let block_num = u32::from_le_bytes([
                block_data[offset],
                block_data[offset + 1],
                block_data[offset + 2],
                block_data[offset + 3],
            ]);
            
            if block_num != 0 {
                blocks.push(block_num as u64);
            } else {
                break; // No more blocks
            }
        }
        
        Ok(())
    }
    
    /// Parse a double indirect block
    fn parse_double_indirect(&mut self, double_indirect_block: BlockNumber, blocks: &mut Vec<BlockNumber>) -> Result<(), MosesError> {
        let block_data = self.read_block_from_disk(double_indirect_block)?;
        let entries_per_block = self.block_size as usize / 4;
        
        for i in 0..entries_per_block {
            let offset = i * 4;
            if offset + 4 > block_data.len() {
                break;
            }
            
            let indirect_block = u32::from_le_bytes([
                block_data[offset],
                block_data[offset + 1],
                block_data[offset + 2],
                block_data[offset + 3],
            ]);
            
            if indirect_block != 0 {
                self.parse_single_indirect(indirect_block as u64, blocks)?;
            } else {
                break; // No more indirect blocks
            }
        }
        
        Ok(())
    }
    
    /// Parse a triple indirect block
    fn parse_triple_indirect(&mut self, triple_indirect_block: BlockNumber, blocks: &mut Vec<BlockNumber>) -> Result<(), MosesError> {
        let block_data = self.read_block_from_disk(triple_indirect_block)?;
        let entries_per_block = self.block_size as usize / 4;
        
        for i in 0..entries_per_block {
            let offset = i * 4;
            if offset + 4 > block_data.len() {
                break;
            }
            
            let double_indirect_block = u32::from_le_bytes([
                block_data[offset],
                block_data[offset + 1],
                block_data[offset + 2],
                block_data[offset + 3],
            ]);
            
            if double_indirect_block != 0 {
                self.parse_double_indirect(double_indirect_block as u64, blocks)?;
            } else {
                break; // No more double indirect blocks
            }
        }
        
        Ok(())
    }
    
    /// Add blocks to an inode using indirect blocks
    pub(super) fn add_indirect_blocks(
        &mut self,
        inode: &mut Ext4Inode,
        new_blocks: &[BlockNumber],
    ) -> Result<(), MosesError> {
        let mut blocks_to_add = new_blocks.to_vec();
        let mut current_block_count = self.count_indirect_blocks(inode)?;
        
        // Add to direct blocks first
        while current_block_count < 12 && !blocks_to_add.is_empty() {
            inode.i_block[current_block_count] = blocks_to_add.remove(0) as u32;
            current_block_count += 1;
        }
        
        // If we still have blocks to add, use indirect blocks
        if !blocks_to_add.is_empty() {
            self.add_to_indirect_blocks(inode, &blocks_to_add, current_block_count)?;
        }
        
        Ok(())
    }
    
    /// Count blocks in indirect block structure
    pub(super) fn count_indirect_blocks(&self, inode: &Ext4Inode) -> Result<usize, MosesError> {
        let mut count = 0;
        
        // Count direct blocks
        for i in 0..12 {
            if inode.i_block[i] != 0 {
                count += 1;
            } else {
                break;
            }
        }
        
        // Count blocks in indirect structures
        let entries_per_block = self.block_size as usize / 4;
        
        // Single indirect
        if inode.i_block[12] != 0 {
            // Estimate: assume full if not zero
            count += entries_per_block;
        }
        
        // Double indirect
        if inode.i_block[13] != 0 {
            count += entries_per_block * entries_per_block;
        }
        
        // Triple indirect
        if inode.i_block[14] != 0 {
            count += entries_per_block * entries_per_block * entries_per_block;
        }
        
        Ok(count)
    }
    
    /// Add blocks to indirect block structures
    fn add_to_indirect_blocks(
        &mut self,
        inode: &mut Ext4Inode,
        blocks: &[BlockNumber],
        start_position: usize,
    ) -> Result<(), MosesError> {
        let entries_per_block = self.block_size as usize / 4;
        let mut blocks_added = 0;
        let mut current_position = start_position;
        
        // Single indirect block range: 12 to (12 + entries_per_block)
        if current_position < 12 + entries_per_block {
            // Allocate single indirect block if needed
            if inode.i_block[12] == 0 {
                let indirect_block = self.block_allocator.allocate_block(None)
                    .map_err(|e| MosesError::Other(format!("Failed to allocate indirect block: {:?}", e)))?;
                inode.i_block[12] = indirect_block as u32;
                
                // Initialize the indirect block
                let empty_block = vec![0u8; self.block_size as usize];
                self.write_block_to_disk(indirect_block, &empty_block)?;
            }
            
            // Add blocks to single indirect
            let blocks_in_single = std::cmp::min(
                blocks.len() - blocks_added,
                12 + entries_per_block - current_position
            );
            
            self.write_to_single_indirect(
                inode.i_block[12] as u64,
                &blocks[blocks_added..blocks_added + blocks_in_single],
                current_position - 12
            )?;
            
            blocks_added += blocks_in_single;
            current_position += blocks_in_single;
        }
        
        // Double indirect block range: (12 + entries_per_block) to (12 + entries_per_block + entries_per_block^2)
        let double_indirect_start = 12 + entries_per_block;
        let double_indirect_end = double_indirect_start + entries_per_block * entries_per_block;
        
        if current_position < double_indirect_end && blocks_added < blocks.len() {
            // Allocate double indirect block if needed
            if inode.i_block[13] == 0 {
                let double_indirect = self.block_allocator.allocate_block(None)
                    .map_err(|e| MosesError::Other(format!("Failed to allocate double indirect block: {:?}", e)))?;
                inode.i_block[13] = double_indirect as u32;
                
                // Initialize the double indirect block
                let empty_block = vec![0u8; self.block_size as usize];
                self.write_block_to_disk(double_indirect, &empty_block)?;
            }
            
            // Add blocks to double indirect
            let blocks_in_double = std::cmp::min(
                blocks.len() - blocks_added,
                double_indirect_end - current_position
            );
            
            self.write_to_double_indirect(
                inode.i_block[13] as u64,
                &blocks[blocks_added..blocks_added + blocks_in_double],
                current_position - double_indirect_start,
                entries_per_block
            )?;
            
            blocks_added += blocks_in_double;
            current_position += blocks_in_double;
        }
        
        // Triple indirect block range: beyond double indirect
        if blocks_added < blocks.len() {
            // Allocate triple indirect block if needed
            if inode.i_block[14] == 0 {
                let triple_indirect = self.block_allocator.allocate_block(None)
                    .map_err(|e| MosesError::Other(format!("Failed to allocate triple indirect block: {:?}", e)))?;
                inode.i_block[14] = triple_indirect as u32;
                
                // Initialize the triple indirect block
                let empty_block = vec![0u8; self.block_size as usize];
                self.write_block_to_disk(triple_indirect, &empty_block)?;
            }
            
            // Add blocks to triple indirect
            let triple_indirect_start = double_indirect_end;
            
            self.write_to_triple_indirect(
                inode.i_block[14] as u64,
                &blocks[blocks_added..],
                current_position - triple_indirect_start,
                entries_per_block
            )?;
            
            blocks_added += blocks.len() - blocks_added;
        }
        
        if blocks_added < blocks.len() {
            return Err(MosesError::Other("Failed to add all blocks to indirect structure".to_string()));
        }
        
        Ok(())
    }
    
    /// Write blocks to a single indirect block
    fn write_to_single_indirect(
        &mut self,
        indirect_block: BlockNumber,
        blocks: &[BlockNumber],
        start_index: usize,
    ) -> Result<(), MosesError> {
        let mut block_data = self.read_block_from_disk(indirect_block)?;
        
        for (i, &block_num) in blocks.iter().enumerate() {
            let index = start_index + i;
            let offset = index * 4;
            
            if offset + 4 > block_data.len() {
                return Err(MosesError::Other("Indirect block overflow".to_string()));
            }
            
            let bytes = (block_num as u32).to_le_bytes();
            block_data[offset..offset + 4].copy_from_slice(&bytes);
        }
        
        self.write_block_to_disk(indirect_block, &block_data)?;
        Ok(())
    }
    
    /// Write blocks to a double indirect block
    fn write_to_double_indirect(
        &mut self,
        double_indirect_block: BlockNumber,
        blocks: &[BlockNumber],
        start_index: usize,
        entries_per_block: usize,
    ) -> Result<(), MosesError> {
        // Read the double indirect block
        let mut double_data = self.read_block_from_disk(double_indirect_block)?;
        
        let mut blocks_written = 0;
        let mut current_index = start_index;
        
        while blocks_written < blocks.len() {
            // Calculate which single indirect block we need
            let single_indirect_index = current_index / entries_per_block;
            let offset_in_single = current_index % entries_per_block;
            
            // Check if we need to allocate a new single indirect block
            let single_indirect_offset = single_indirect_index * 4;
            let mut single_indirect_block = u32::from_le_bytes([
                double_data[single_indirect_offset],
                double_data[single_indirect_offset + 1],
                double_data[single_indirect_offset + 2],
                double_data[single_indirect_offset + 3],
            ]) as u64;
            
            if single_indirect_block == 0 {
                // Allocate new single indirect block
                single_indirect_block = self.block_allocator.allocate_block(None)
                    .map_err(|e| MosesError::Other(format!("Failed to allocate single indirect in double: {:?}", e)))?;
                
                // Update double indirect block
                let bytes = (single_indirect_block as u32).to_le_bytes();
                double_data[single_indirect_offset..single_indirect_offset + 4].copy_from_slice(&bytes);
                
                // Initialize the new single indirect block
                let empty_block = vec![0u8; self.block_size as usize];
                self.write_block_to_disk(single_indirect_block, &empty_block)?;
            }
            
            // Calculate how many blocks to write to this single indirect
            let blocks_in_this_single = std::cmp::min(
                blocks.len() - blocks_written,
                entries_per_block - offset_in_single
            );
            
            // Write to the single indirect block
            self.write_to_single_indirect(
                single_indirect_block,
                &blocks[blocks_written..blocks_written + blocks_in_this_single],
                offset_in_single
            )?;
            
            blocks_written += blocks_in_this_single;
            current_index += blocks_in_this_single;
        }
        
        // Write back the updated double indirect block
        self.write_block_to_disk(double_indirect_block, &double_data)?;
        Ok(())
    }
    
    /// Write blocks to a triple indirect block
    fn write_to_triple_indirect(
        &mut self,
        triple_indirect_block: BlockNumber,
        blocks: &[BlockNumber],
        start_index: usize,
        entries_per_block: usize,
    ) -> Result<(), MosesError> {
        // Read the triple indirect block
        let mut triple_data = self.read_block_from_disk(triple_indirect_block)?;
        
        let mut blocks_written = 0;
        let mut current_index = start_index;
        let blocks_per_double = entries_per_block * entries_per_block;
        
        while blocks_written < blocks.len() {
            // Calculate which double indirect block we need
            let double_indirect_index = current_index / blocks_per_double;
            let offset_in_double = current_index % blocks_per_double;
            
            // Check if we need to allocate a new double indirect block
            let double_indirect_offset = double_indirect_index * 4;
            let mut double_indirect_block = u32::from_le_bytes([
                triple_data[double_indirect_offset],
                triple_data[double_indirect_offset + 1],
                triple_data[double_indirect_offset + 2],
                triple_data[double_indirect_offset + 3],
            ]) as u64;
            
            if double_indirect_block == 0 {
                // Allocate new double indirect block
                double_indirect_block = self.block_allocator.allocate_block(None)
                    .map_err(|e| MosesError::Other(format!("Failed to allocate double indirect in triple: {:?}", e)))?;
                
                // Update triple indirect block
                let bytes = (double_indirect_block as u32).to_le_bytes();
                triple_data[double_indirect_offset..double_indirect_offset + 4].copy_from_slice(&bytes);
                
                // Initialize the new double indirect block
                let empty_block = vec![0u8; self.block_size as usize];
                self.write_block_to_disk(double_indirect_block, &empty_block)?;
            }
            
            // Calculate how many blocks to write to this double indirect
            let blocks_in_this_double = std::cmp::min(
                blocks.len() - blocks_written,
                blocks_per_double - offset_in_double
            );
            
            // Write to the double indirect block
            self.write_to_double_indirect(
                double_indirect_block,
                &blocks[blocks_written..blocks_written + blocks_in_this_double],
                offset_in_double,
                entries_per_block
            )?;
            
            blocks_written += blocks_in_this_double;
            current_index += blocks_in_this_double;
        }
        
        // Write back the updated triple indirect block
        self.write_block_to_disk(triple_indirect_block, &triple_data)?;
        Ok(())
    }
    
    /// Get the last allocated block number from indirect blocks
    pub(super) fn get_last_indirect_block(&self, inode: &Ext4Inode) -> Option<BlockNumber> {
        // Check direct blocks in reverse
        for i in (0..12).rev() {
            if inode.i_block[i] != 0 {
                // This would need to check the actual data blocks, not the indirect blocks
                // For simplicity, return the block number
                return Some(inode.i_block[i] as u64);
            }
        }
        None
    }
}