// Implementation of complete ext4 filesystem formatting

use moses_core::{Device, FormatOptions, MosesError};
use crate::ext4_native::core::{
    structures::*,
    types::{FilesystemParams, FilesystemLayout},
    alignment::AlignedBuffer,
    bitmap::{Bitmap, init_block_bitmap_group0, init_inode_bitmap_group0},
    constants::*,
};
use std::fs::{File, OpenOptions};
use std::io::{Write, Seek, SeekFrom};

/// Write complete ext4 filesystem to device
pub async fn format_device(
    device: &Device,
    options: &FormatOptions,
) -> Result<(), MosesError> {
    // Convert options to filesystem parameters
    let params = FilesystemParams {
        size_bytes: device.size,
        block_size: options.cluster_size.unwrap_or(4096) as u32,
        inode_size: 256,
        label: options.label.clone(),
        reserved_percent: 5,
        enable_checksums: true,
        enable_64bit: device.size > 16 * 1024 * 1024 * 1024, // >16GB
        enable_journal: false,
    };
    
    // Calculate filesystem layout
    let layout = FilesystemLayout::from_params(&params)
        .map_err(|e| MosesError::Other(e.to_string()))?;
    
    // Create and initialize superblock
    let mut sb = Ext4Superblock::new();
    sb.init_minimal(&params, &layout);
    
    // Create group descriptor
    let mut gd = Ext4GroupDesc::new();
    gd.init(0, &layout, &params);
    
    // Create block bitmap
    let mut block_bitmap = Bitmap::for_block_group(layout.blocks_per_group);
    init_block_bitmap_group0(&mut block_bitmap, &layout, &params);
    
    // Allocate blocks for directories
    let mut dir_data_block = 0u64;
    let mut lf_data_block = 0u64;
    let mut blocks_allocated = 0;
    for i in 0..layout.blocks_per_group {
        if !block_bitmap.is_set(i) {
            block_bitmap.set(i);
            if blocks_allocated == 0 {
                dir_data_block = i as u64;
            } else if blocks_allocated == 1 {
                lf_data_block = i as u64;
                break;
            }
            blocks_allocated += 1;
        }
    }
    
    // Update group descriptor free blocks count (2 blocks allocated)
    gd.bg_free_blocks_count_lo -= 2;
    
    // Also update superblock's free blocks count
    let current_free = sb.s_free_blocks_count_lo as u64 | ((sb.s_free_blocks_count_hi as u64) << 32);
    let new_free = current_free - 2;
    sb.s_free_blocks_count_lo = (new_free & 0xFFFFFFFF) as u32;
    sb.s_free_blocks_count_hi = ((new_free >> 32) & 0xFFFFFFFF) as u32;
    
    // Create inode bitmap
    let mut inode_bitmap = Bitmap::for_inode_group(layout.inodes_per_group);
    init_inode_bitmap_group0(&mut inode_bitmap);
    
    // Mark inode 11 (lost+found) as used
    inode_bitmap.set(10);  // Inode 11 is at index 10
    
    // Free inodes count already accounts for inodes 1-11 being used
    // (it was initialized as total - EXT4_FIRST_INO = 8192 - 11 = 8181)
    // No need to subtract more!
    
    // Update unused inodes count
    gd.bg_itable_unused_lo = 0;  // All inodes are initialized
    gd.bg_used_dirs_count_lo = 2;  // Root and lost+found directories
    
    // Create root inode with extent pointing to directory data block
    let mut root_inode = Ext4Inode::new();
    root_inode.init_root_dir(&params);
    root_inode.i_links_count = 3;  // . and .. and lost+found's parent reference
    update_root_inode_extents(&mut root_inode, dir_data_block);
    
    // Create lost+found inode
    let mut lf_inode = Ext4Inode::new();
    lf_inode.init_lost_found_dir(&params);
    update_root_inode_extents(&mut lf_inode, lf_data_block);
    
    // Create directory data blocks
    let dir_data = create_root_directory_block(params.block_size);
    let lf_data = create_lost_found_directory_block(params.block_size);
    
    // Update checksums
    gd.update_checksum(0, &sb);
    root_inode.update_checksum(EXT4_ROOT_INO, &sb);
    lf_inode.update_checksum(EXT4_FIRST_INO as u32, &sb);
    sb.update_checksum();
    
    // Open device for writing
    #[cfg(target_os = "windows")]
    let device_path = format!(r"\\.\{}", device.id);
    #[cfg(not(target_os = "windows"))]
    let device_path = format!("/dev/{}", device.id);
    
    let mut file = OpenOptions::new()
        .write(true)
        .open(&device_path)
        .map_err(|e| MosesError::Other(format!("Failed to open device {}: {}", device_path, e)))?;
    
    // Write zeros for entire device (or at least the first part)
    let zeros = vec![0u8; 1024 * 1024];
    let mut written = 0u64;
    let write_size = device.size.min(100 * 1024 * 1024); // Write at least 100MB
    while written < write_size {
        let to_write = ((write_size - written) as usize).min(zeros.len());
        file.write_all(&zeros[..to_write])
            .map_err(|e| MosesError::Other(format!("Failed to zero device: {}", e)))?;
        written += to_write as u64;
    }
    
    file.seek(SeekFrom::Start(0))
        .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
    
    // Write all filesystem structures
    let mut current_block = 0u64;
    
    // Block 0: Superblock
    let mut sb_buffer = AlignedBuffer::<4096>::new();
    sb.write_to_buffer(&mut sb_buffer[1024..2048])
        .map_err(|e| MosesError::Other(format!("Failed to serialize superblock: {}", e)))?;
    file.seek(SeekFrom::Start(current_block * 4096))
        .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
    file.write_all(&sb_buffer[..])
        .map_err(|e| MosesError::Other(format!("Failed to write superblock: {}", e)))?;
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
    file.seek(SeekFrom::Start(current_block * 4096))
        .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
    file.write_all(&gdt_buffer[..])
        .map_err(|e| MosesError::Other(format!("Failed to write GDT: {}", e)))?;
    current_block += 1;
    
    // Skip reserved GDT blocks
    current_block += layout.reserved_gdt_blocks as u64;
    
    // Block bitmap
    let mut bitmap_buffer = AlignedBuffer::<4096>::new();
    block_bitmap.write_to_buffer(&mut bitmap_buffer)
        .map_err(|e| MosesError::Other(format!("Failed to prepare block bitmap: {}", e)))?;
    file.seek(SeekFrom::Start(current_block * 4096))
        .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
    file.write_all(&bitmap_buffer[..])
        .map_err(|e| MosesError::Other(format!("Failed to write block bitmap: {}", e)))?;
    current_block += 1;
    
    // Inode bitmap
    let mut inode_bitmap_buffer = AlignedBuffer::<4096>::new();
    inode_bitmap.write_to_buffer(&mut inode_bitmap_buffer)
        .map_err(|e| MosesError::Other(format!("Failed to prepare inode bitmap: {}", e)))?;
    file.seek(SeekFrom::Start(current_block * 4096))
        .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
    file.write_all(&inode_bitmap_buffer[..])
        .map_err(|e| MosesError::Other(format!("Failed to write inode bitmap: {}", e)))?;
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
    
    // Write lost+found inode at position 10 (inode 11)
    let lf_inode_offset = 10 * params.inode_size as usize;
    let lf_inode_bytes = unsafe {
        std::slice::from_raw_parts(
            &lf_inode as *const _ as *const u8,
            256
        )
    };
    inode_table_buffer[lf_inode_offset..lf_inode_offset + 256].copy_from_slice(lf_inode_bytes);
    
    file.seek(SeekFrom::Start(current_block * 4096))
        .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
    file.write_all(&inode_table_buffer)
        .map_err(|e| MosesError::Other(format!("Failed to write inode table: {}", e)))?;
    
    // Write root directory data at its allocated block
    file.seek(SeekFrom::Start(dir_data_block * 4096))
        .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
    file.write_all(&dir_data)
        .map_err(|e| MosesError::Other(format!("Failed to write root directory: {}", e)))?;
    
    // Write lost+found directory data at its allocated block  
    file.seek(SeekFrom::Start(lf_data_block * 4096))
        .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
    file.write_all(&lf_data)
        .map_err(|e| MosesError::Other(format!("Failed to write lost+found: {}", e)))?;
    
    file.sync_all()
        .map_err(|e| MosesError::Other(format!("Failed to sync device: {}", e)))?;
    
    Ok(())
}