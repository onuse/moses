// Phase 3 tests: Complete filesystem with root directory

#[cfg(test)]
mod tests {
    use crate::ext4_native::core::{
        structures::{
            Ext4Superblock, Ext4GroupDesc, Ext4Inode, 
            Ext4DirEntry2, Ext4Extent,
            create_root_directory_block, update_root_inode_extents,
        },
        types::{FilesystemParams, FilesystemLayout},
        alignment::AlignedBuffer,
        bitmap::{Bitmap, init_block_bitmap_group0, init_inode_bitmap_group0},
        constants::*,
    };
    use std::fs::File;
    use std::io::{Write, Seek, SeekFrom};
    
    #[test]
    fn test_directory_entry_creation() {
        let (entry, name) = Ext4DirEntry2::new(2, "test", EXT4_FT_REG_FILE);
        
        // Copy fields to avoid unaligned access to packed struct
        let inode = entry.inode;
        let name_len = entry.name_len;
        let file_type = entry.file_type;
        
        assert_eq!(inode, 2);
        assert_eq!(name_len, 4);
        assert_eq!(file_type, EXT4_FT_REG_FILE);
        assert_eq!(name, b"test");
        
        // Check size calculation
        let size = Ext4DirEntry2::size_needed(4);
        assert_eq!(size, 12); // 8 bytes header + 4 bytes name, rounded to 4-byte boundary
    }
    
    #[test]
    fn test_extent_creation() {
        let extent = Ext4Extent::new(0, 12345, 10);
        assert_eq!(extent.ee_block, 0);
        assert_eq!(extent.ee_len, 10);
        assert_eq!(extent.physical_block(), 12345);
    }
    
    #[test]
    fn test_root_directory_block() {
        let dir_block = create_root_directory_block(4096);
        
        // Check "." entry
        let dot_inode = u32::from_le_bytes([dir_block[0], dir_block[1], dir_block[2], dir_block[3]]);
        assert_eq!(dot_inode, EXT4_ROOT_INO);
        
        let dot_rec_len = u16::from_le_bytes([dir_block[4], dir_block[5]]);
        assert_eq!(dot_rec_len, 12);
        
        let dot_name_len = dir_block[6];
        assert_eq!(dot_name_len, 1);
        
        let dot_file_type = dir_block[7];
        assert_eq!(dot_file_type, EXT4_FT_DIR);
        
        assert_eq!(&dir_block[8..9], b".");
        
        // Check ".." entry at offset 12
        let dotdot_inode = u32::from_le_bytes([dir_block[12], dir_block[13], dir_block[14], dir_block[15]]);
        assert_eq!(dotdot_inode, EXT4_ROOT_INO);
        
        let dotdot_rec_len = u16::from_le_bytes([dir_block[16], dir_block[17]]);
        assert_eq!(dotdot_rec_len, 4096 - 12); // Takes rest of block
        
        let dotdot_name_len = dir_block[18];
        assert_eq!(dotdot_name_len, 2);
        
        let dotdot_file_type = dir_block[19];
        assert_eq!(dotdot_file_type, EXT4_FT_DIR);
        
        assert_eq!(&dir_block[20..22], b"..");
    }
    
    #[test]
    fn test_extent_tree_in_inode() {
        let mut inode = Ext4Inode::new();
        inode.init_root_dir(&FilesystemParams::default());
        
        // Update with extent pointing to block 1000
        update_root_inode_extents(&mut inode, 1000);
        
        // Verify extent header in i_block
        let magic = (inode.i_block[0] & 0xFFFF) as u16;
        assert_eq!(magic, EXT4_EXTENT_MAGIC);
        
        let entries = ((inode.i_block[0] >> 16) & 0xFFFF) as u16;
        assert_eq!(entries, 1);
        
        let max_entries = (inode.i_block[1] & 0xFFFF) as u16;
        assert_eq!(max_entries, 4);
        
        let depth = ((inode.i_block[1] >> 16) & 0xFFFF) as u16;
        assert_eq!(depth, 0); // Leaf node
        
        // Verify extent
        let ee_block = inode.i_block[3];
        assert_eq!(ee_block, 0); // Logical block 0
        
        let ee_len = (inode.i_block[4] & 0xFFFF) as u16;
        assert_eq!(ee_len, 1); // 1 block
        
        let ee_start_lo = inode.i_block[5];
        assert_eq!(ee_start_lo, 1000); // Physical block 1000
    }
    
    #[test]
    fn test_create_complete_filesystem() {
        let image_path = "test_phase3.img";
        let image_size = 100 * 1024 * 1024; // 100MB
        
        let params = FilesystemParams {
            size_bytes: image_size,
            block_size: 4096,
            inode_size: 256,
            label: Some("Phase3Test".to_string()),
            reserved_percent: 5,
            enable_checksums: true,
            enable_64bit: false,
            enable_journal: false,
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        // Create and initialize superblock
        let mut sb = Ext4Superblock::new();
        sb.init_minimal(&params, &layout);
        
        // Create group descriptor
        let mut gd = Ext4GroupDesc::new();
        gd.init(0, &layout, &params);
        
        // Create block bitmap
        let mut block_bitmap = Bitmap::for_block_group(layout.blocks_per_group);
        init_block_bitmap_group0(&mut block_bitmap, &layout, &params);
        
        // Allocate a block for root directory data
        // Find first free block after metadata
        let mut dir_data_block = 0u64;
        for i in 0..layout.blocks_per_group {
            if !block_bitmap.is_set(i) {
                block_bitmap.set(i);
                dir_data_block = i as u64;
                break;
            }
        }
        assert!(dir_data_block > 0);
        println!("Allocated block {} for root directory data", dir_data_block);
        
        // Update group descriptor free blocks count
        gd.bg_free_blocks_count_lo -= 1;
        
        // Also update superblock's free blocks count
        let current_free = sb.s_free_blocks_count_lo as u64 | ((sb.s_free_blocks_count_hi as u64) << 32);
        let new_free = current_free - 1;
        sb.s_free_blocks_count_lo = (new_free & 0xFFFFFFFF) as u32;
        sb.s_free_blocks_count_hi = ((new_free >> 32) & 0xFFFFFFFF) as u32;
        
        // Create inode bitmap
        let mut inode_bitmap = Bitmap::for_inode_group(layout.inodes_per_group);
        init_inode_bitmap_group0(&mut inode_bitmap);
        
        // Update free inodes count for inode 11 being marked
        sb.s_free_inodes_count -= 1;
        gd.bg_free_inodes_count_lo -= 1;
        
        // Create root inode with extent pointing to directory data block
        let mut root_inode = Ext4Inode::new();
        root_inode.init_root_dir(&params);
        update_root_inode_extents(&mut root_inode, dir_data_block);
        
        // Create root directory data block
        let dir_data = create_root_directory_block(params.block_size);
        
        // Update checksums
        gd.update_checksum(0, &sb);
        root_inode.update_checksum(EXT4_ROOT_INO, &sb);
        sb.update_checksum();
        
        // Now write everything to image file
        let mut file = File::create(image_path).unwrap();
        
        // Write zeros for entire image
        let zeros = vec![0u8; 1024 * 1024];
        let mut written = 0u64;
        while written < image_size {
            let to_write = ((image_size - written) as usize).min(zeros.len());
            file.write_all(&zeros[..to_write]).unwrap();
            written += to_write as u64;
        }
        
        file.seek(SeekFrom::Start(0)).unwrap();
        
        // Write all filesystem structures
        let mut current_block = 0u64;
        
        // Block 0: Superblock
        let mut sb_buffer = AlignedBuffer::<4096>::new();
        sb.write_to_buffer(&mut sb_buffer[1024..2048]).unwrap();
        file.seek(SeekFrom::Start(current_block * 4096)).unwrap();
        file.write_all(&sb_buffer[..]).unwrap();
        current_block += 1;
        
        // Block 1: Group descriptor table
        let mut gdt_buffer = AlignedBuffer::<4096>::new();
        let gd_bytes = unsafe {
            std::slice::from_raw_parts(
                &gd as *const _ as *const u8,
                64
            )
        };
        gdt_buffer[..64].copy_from_slice(gd_bytes);
        file.seek(SeekFrom::Start(current_block * 4096)).unwrap();
        file.write_all(&gdt_buffer[..]).unwrap();
        current_block += 1;
        
        // Skip reserved GDT blocks
        current_block += layout.reserved_gdt_blocks as u64;
        
        // Block bitmap
        let mut bitmap_buffer = AlignedBuffer::<4096>::new();
        block_bitmap.write_to_buffer(&mut bitmap_buffer).unwrap();
        file.seek(SeekFrom::Start(current_block * 4096)).unwrap();
        file.write_all(&bitmap_buffer[..]).unwrap();
        current_block += 1;
        
        // Inode bitmap
        let mut inode_bitmap_buffer = AlignedBuffer::<4096>::new();
        inode_bitmap.write_to_buffer(&mut inode_bitmap_buffer).unwrap();
        file.seek(SeekFrom::Start(current_block * 4096)).unwrap();
        file.write_all(&inode_bitmap_buffer[..]).unwrap();
        current_block += 1;
        
        // Inode table
        let inode_table_size = layout.inode_table_blocks() as usize * 4096;
        let mut inode_table_buffer = vec![0u8; inode_table_size];
        
        // Write root inode at position 1 (inode 2)
        let root_inode_offset = 1 * params.inode_size as usize;
        let root_inode_bytes = unsafe {
            std::slice::from_raw_parts(
                &root_inode as *const _ as *const u8,
                256
            )
        };
        inode_table_buffer[root_inode_offset..root_inode_offset + 256].copy_from_slice(root_inode_bytes);
        
        file.seek(SeekFrom::Start(current_block * 4096)).unwrap();
        file.write_all(&inode_table_buffer).unwrap();
        
        // Write root directory data at its allocated block
        file.seek(SeekFrom::Start(dir_data_block * 4096)).unwrap();
        file.write_all(&dir_data).unwrap();
        
        file.sync_all().unwrap();
        
        println!("\nPhase 3 complete filesystem created: {}", image_path);
        println!("  Superblock at block 0");
        println!("  Group descriptor at block 1");
        println!("  Block bitmap at block {}", gd.bg_block_bitmap_lo);
        println!("  Inode bitmap at block {}", gd.bg_inode_bitmap_lo);
        println!("  Inode table at block {}", gd.bg_inode_table_lo);
        println!("  Root directory data at block {}", dir_data_block);
        println!("\nRoot inode extent tree:");
        println!("  Logical block 0 -> Physical block {}", dir_data_block);
        println!("\nTo validate:");
        println!("  Linux: e2fsck -fn {}", image_path);
        println!("  Linux: dumpe2fs {} 2>/dev/null | head -50", image_path);
        println!("  Linux: debugfs -R 'ls -l' {}", image_path);
        
        // Keep the file for manual testing
        println!("\nImage file kept at: {}", image_path);
    }
}