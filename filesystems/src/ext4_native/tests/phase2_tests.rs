// Phase 2 tests: Complete block group with all metadata

#[cfg(test)]
mod tests {
    use crate::ext4_native::core::{
        structures::{Ext4Superblock, Ext4GroupDesc, Ext4Inode},
        types::{FilesystemParams, FilesystemLayout},
        alignment::AlignedBuffer,
        bitmap::{Bitmap, init_block_bitmap_group0, init_inode_bitmap_group0},
        constants::*,
    };
    use std::fs::File;
    use std::io::{Write, Seek, SeekFrom};
    
    #[test]
    fn test_block_group_descriptor() {
        let params = FilesystemParams {
            size_bytes: 100 * 1024 * 1024,
            block_size: 4096,
            inode_size: 256,
            ..Default::default()
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        let mut gd = Ext4GroupDesc::new();
        gd.init(0, &layout, &params);
        
        // Check block numbers are sequential
        assert!(gd.bg_block_bitmap_lo > 0);
        assert_eq!(gd.bg_inode_bitmap_lo, gd.bg_block_bitmap_lo + 1);
        assert_eq!(gd.bg_inode_table_lo, gd.bg_inode_bitmap_lo + 1);
        
        // Check counts
        assert!(gd.bg_free_blocks_count_lo > 0);
        assert_eq!(gd.bg_free_inodes_count_lo, (layout.inodes_per_group - EXT4_FIRST_INO) as u16);
        assert_eq!(gd.bg_used_dirs_count_lo, 1); // Root directory
    }
    
    #[test]
    fn test_root_inode_initialization() {
        let params = FilesystemParams {
            size_bytes: 100 * 1024 * 1024,
            block_size: 4096,
            ..Default::default()
        };
        
        let mut inode = Ext4Inode::new();
        inode.init_root_dir(&params);
        
        // Check mode
        assert_eq!(inode.i_mode & S_IFMT, S_IFDIR);
        assert_eq!(inode.i_mode & 0o777, 0o755);
        
        // Check ownership
        assert_eq!(inode.i_uid, 0);
        assert_eq!(inode.i_gid, 0);
        
        // Check size
        assert_eq!(inode.i_size_lo, 4096);
        
        // Check links
        assert_eq!(inode.i_links_count, 2);
        
        // Check flags
        assert_eq!(inode.i_flags, EXT4_EXTENTS_FL);
    }
    
    #[test]
    fn test_create_phase2_image() {
        let image_path = "test_phase2.img";
        let image_size = 100 * 1024 * 1024; // 100MB
        
        let params = FilesystemParams {
            size_bytes: image_size,
            block_size: 4096,
            inode_size: 256,
            label: Some("Phase2Test".to_string()),
            reserved_percent: 5,
            enable_checksums: true,
            enable_64bit: false,
            enable_journal: false,
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        // Create and initialize superblock
        let mut sb = Ext4Superblock::new();
        sb.init_minimal(&params, &layout);
        sb.update_checksum();
        
        // Create group descriptor
        let mut gd = Ext4GroupDesc::new();
        gd.init(0, &layout, &params);
        gd.update_checksum(0, &sb);
        
        // Create block bitmap
        let mut block_bitmap = Bitmap::for_block_group(layout.blocks_per_group);
        init_block_bitmap_group0(&mut block_bitmap, &layout, &params);
        
        // Create inode bitmap
        let mut inode_bitmap = Bitmap::for_inode_group(layout.inodes_per_group);
        init_inode_bitmap_group0(&mut inode_bitmap);
        
        // Create inode table with root inode
        let mut root_inode = Ext4Inode::new();
        root_inode.init_root_dir(&params);
        root_inode.update_checksum(EXT4_ROOT_INO, &sb);
        
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
        
        // Calculate block offsets
        let mut current_block = 0u64;
        
        // Block 0: Superblock (at offset 1024 within the block)
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
                64 // Using 64-byte descriptors
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
        // Calculate size needed for inode table
        let inode_table_size = layout.inode_table_blocks() as usize * 4096;
        let mut inode_table_buffer = vec![0u8; inode_table_size];
        
        // Write root inode at position 1 (inode 2, since inode numbers start at 1)
        let root_inode_offset = 1 * params.inode_size as usize; // Inode 2 is at index 1
        let root_inode_bytes = unsafe {
            std::slice::from_raw_parts(
                &root_inode as *const _ as *const u8,
                256
            )
        };
        inode_table_buffer[root_inode_offset..root_inode_offset + 256].copy_from_slice(root_inode_bytes);
        
        // Write the inode table blocks
        file.seek(SeekFrom::Start(current_block * 4096)).unwrap();
        file.write_all(&inode_table_buffer).unwrap();
        
        file.sync_all().unwrap();
        
        println!("Phase 2 image created: {}", image_path);
        println!("  Superblock at block 0 (offset 1024)");
        println!("  Group descriptor at block 1");
        println!("  Block bitmap at block {}", gd.bg_block_bitmap_lo);
        println!("  Inode bitmap at block {}", gd.bg_inode_bitmap_lo);
        println!("  Inode table at block {}", gd.bg_inode_table_lo);
        
        // Clean up
        std::fs::remove_file(image_path).ok();
    }
    
    #[test]
    fn test_block_bitmap_initialization() {
        let params = FilesystemParams {
            size_bytes: 100 * 1024 * 1024,
            block_size: 4096,
            ..Default::default()
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        let mut bitmap = Bitmap::for_block_group(layout.blocks_per_group);
        init_block_bitmap_group0(&mut bitmap, &layout, &params);
        
        // Check that metadata blocks are marked as used
        assert!(bitmap.is_set(0)); // Superblock
        assert!(bitmap.is_set(1)); // GDT
        
        // Check free blocks count
        let free_blocks = bitmap.count_free();
        assert!(free_blocks > 0);
        assert!(free_blocks < layout.blocks_per_group);
    }
    
    #[test]
    fn test_inode_bitmap_initialization() {
        let params = FilesystemParams {
            size_bytes: 100 * 1024 * 1024,
            ..Default::default()
        };
        
        let layout = FilesystemLayout::from_params(&params).unwrap();
        
        let mut bitmap = Bitmap::for_inode_group(layout.inodes_per_group);
        init_inode_bitmap_group0(&mut bitmap);
        
        // Check reserved inodes are marked as used
        for i in 0..EXT4_FIRST_INO {
            assert!(bitmap.is_set(i));
        }
        
        // Check rest are free
        assert!(!bitmap.is_set(EXT4_FIRST_INO));
        
        let free_inodes = bitmap.count_free();
        assert_eq!(free_inodes, layout.inodes_per_group - EXT4_FIRST_INO);
    }
}