// Implementation of complete ext4 filesystem formatting

use moses_core::{Device, FormatOptions, MosesError};
use crate::ext4_native::core::{
    structures::*,
    types::{FilesystemParams, FilesystemLayout},
    alignment::AlignedBuffer,
    bitmap::{Bitmap, init_block_bitmap_group0, init_inode_bitmap_group0},
    constants::*,
};
#[cfg(not(target_os = "windows"))]
use std::fs::OpenOptions;
#[cfg(not(target_os = "windows"))]
use std::io::{Write, Seek, SeekFrom};
#[cfg(target_os = "windows")]
use crate::ext4_native::windows::WindowsDeviceIO;

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
    let device_path = if device.id.starts_with(r"\\.\") {
        device.id.clone()
    } else {
        format!(r"\\.\{}", device.id)
    };
    #[cfg(not(target_os = "windows"))]
    let device_path = format!("/dev/{}", device.id);
    
    eprintln!("DEBUG: Formatting device - ID: '{}', Path: '{}'", device.id, device_path);
    
    #[cfg(target_os = "windows")]
    let mut device_io = WindowsDeviceIO::open(&device_path)
        .map_err(|e| MosesError::Other(format!("Failed to open device {}: {:?}", device_path, e)))?;
    
    #[cfg(not(target_os = "windows"))]
    let mut file = OpenOptions::new()
        .write(true)
        .open(&device_path)
        .map_err(|e| MosesError::Other(format!("Failed to open device {}: {}", device_path, e)))?;
    
    // Write zeros for initial part of device
    #[cfg(target_os = "windows")]
    {
        let sector_size = 512;
        let zeros_size = ((1024 * 1024) / sector_size) * sector_size;
        let zeros = vec![0u8; zeros_size];
        let mut written = 0u64;
        let write_size = device.size.min(100 * 1024 * 1024);
        let aligned_write_size = (write_size / sector_size as u64) * sector_size as u64;
        
        while written < aligned_write_size {
            let to_write = ((aligned_write_size - written) as usize).min(zeros.len());
            device_io.write_aligned(written, &zeros[..to_write])
                .map_err(|e| MosesError::Other(format!("Failed to zero device: {:?}", e)))?;
            written += to_write as u64;
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        let zeros = vec![0u8; 1024 * 1024];
        let mut written = 0u64;
        let write_size = device.size.min(100 * 1024 * 1024);
        while written < write_size {
            let to_write = ((write_size - written) as usize).min(zeros.len());
            file.write_all(&zeros[..to_write])
                .map_err(|e| MosesError::Other(format!("Failed to zero device: {}", e)))?;
            written += to_write as u64;
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
    }
    
    // Write all filesystem structures
    let mut current_block = 0u64;
    
    // Block 0: Superblock
    let mut sb_buffer = AlignedBuffer::<4096>::new();
    sb.write_to_buffer(&mut sb_buffer[1024..2048])
        .map_err(|e| MosesError::Other(format!("Failed to serialize superblock: {}", e)))?;
    
    #[cfg(target_os = "windows")]
    device_io.write_aligned(current_block * 4096, &sb_buffer[..])
        .map_err(|e| MosesError::Other(format!("Failed to write superblock: {:?}", e)))?;
    
    #[cfg(not(target_os = "windows"))]
    {
        file.seek(SeekFrom::Start(current_block * 4096))
            .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
        file.write_all(&sb_buffer[..])
            .map_err(|e| MosesError::Other(format!("Failed to write superblock: {}", e)))?;
    }
    current_block += 1;
    
    // Block 1+: Group descriptor table
    // We need to write descriptors for ALL groups, not just group 0
    let mut gdt_buffer = vec![0u8; layout.gdt_blocks as usize * 4096];
    
    // Write group 0 descriptor
    let gd_bytes = unsafe {
        std::slice::from_raw_parts(
            &gd as *const _ as *const u8,
            64
        )
    };
    gdt_buffer[..64].copy_from_slice(gd_bytes);
    
    // Initialize empty descriptors for remaining groups
    // IMPORTANT: Must set valid block numbers even for unused groups!
    // Linux always validates these, regardless of UNINIT flags
    for group_idx in 1..layout.num_groups {
        let offset = group_idx as usize * 64;
        if offset + 64 <= gdt_buffer.len() {
            // Calculate where this group's metadata would be
            let group_first_block = group_idx as u64 * layout.blocks_per_group as u64;
            
            // For simplicity, put metadata at the start of each group
            // (In reality, only certain groups have backup superblocks)
            let mut block_offset = group_first_block;
            
            // Skip superblock backup if this group has one
            if layout.has_superblock(group_idx) {
                block_offset += 1;  // Skip superblock
                block_offset += layout.gdt_blocks as u64;  // Skip GDT blocks
            }
            
            let block_bitmap_block = block_offset;
            let inode_bitmap_block = block_offset + 1;
            let inode_table_block = block_offset + 2;
            
            // Create group descriptor with valid block numbers
            let mut empty_gd = Ext4GroupDesc {
                bg_block_bitmap_lo: (block_bitmap_block & 0xFFFFFFFF) as u32,
                bg_block_bitmap_hi: ((block_bitmap_block >> 32) & 0xFFFFFFFF) as u32,
                bg_inode_bitmap_lo: (inode_bitmap_block & 0xFFFFFFFF) as u32,
                bg_inode_bitmap_hi: ((inode_bitmap_block >> 32) & 0xFFFFFFFF) as u32,
                bg_inode_table_lo: (inode_table_block & 0xFFFFFFFF) as u32,
                bg_inode_table_hi: ((inode_table_block >> 32) & 0xFFFFFFFF) as u32,
                bg_free_blocks_count_lo: 0,  // No free blocks
                bg_free_blocks_count_hi: 0,
                bg_free_inodes_count_lo: 0,  // No free inodes
                bg_free_inodes_count_hi: 0,
                bg_used_dirs_count_lo: 0,
                bg_used_dirs_count_hi: 0,
                bg_flags: EXT4_BG_INODE_UNINIT | EXT4_BG_BLOCK_UNINIT,  // Mark as uninitialized
                bg_exclude_bitmap_lo: 0,
                bg_exclude_bitmap_hi: 0,
                bg_block_bitmap_csum_lo: 0,
                bg_block_bitmap_csum_hi: 0,
                bg_inode_bitmap_csum_lo: 0,
                bg_inode_bitmap_csum_hi: 0,
                bg_itable_unused_lo: layout.inodes_per_group as u16,  // All inodes unused
                bg_itable_unused_hi: 0,
                bg_checksum: 0,
                bg_reserved: 0,
            };
            
            // Calculate checksum for this group descriptor
            empty_gd.update_checksum(group_idx, &sb);
            
            let empty_gd_bytes = unsafe {
                std::slice::from_raw_parts(
                    &empty_gd as *const _ as *const u8,
                    64
                )
            };
            gdt_buffer[offset..offset + 64].copy_from_slice(empty_gd_bytes);
        }
    }
    
    // Write the GDT blocks
    for gdt_block_idx in 0..layout.gdt_blocks {
        let block_offset = (current_block + gdt_block_idx as u64) * 4096;
        let data_offset = gdt_block_idx as usize * 4096;
        let data_end = ((gdt_block_idx + 1) as usize * 4096).min(gdt_buffer.len());
        
        #[cfg(target_os = "windows")]
        device_io.write_aligned(block_offset, &gdt_buffer[data_offset..data_end])
            .map_err(|e| MosesError::Other(format!("Failed to write GDT block {}: {:?}", gdt_block_idx, e)))?;
        
        #[cfg(not(target_os = "windows"))]
        {
            file.seek(SeekFrom::Start(block_offset))
                .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
            file.write_all(&gdt_buffer[data_offset..data_end])
                .map_err(|e| MosesError::Other(format!("Failed to write GDT: {}", e)))?;
        }
    }
    current_block += layout.gdt_blocks as u64;
    
    // Skip reserved GDT blocks
    current_block += layout.reserved_gdt_blocks as u64;
    
    // Block bitmap
    let mut bitmap_buffer = AlignedBuffer::<4096>::new();
    block_bitmap.write_to_buffer(&mut bitmap_buffer)
        .map_err(|e| MosesError::Other(format!("Failed to prepare block bitmap: {}", e)))?;
    
    #[cfg(target_os = "windows")]
    device_io.write_aligned(current_block * 4096, &bitmap_buffer[..])
        .map_err(|e| MosesError::Other(format!("Failed to write block bitmap: {:?}", e)))?;
    
    #[cfg(not(target_os = "windows"))]
    {
        file.seek(SeekFrom::Start(current_block * 4096))
            .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
        file.write_all(&bitmap_buffer[..])
            .map_err(|e| MosesError::Other(format!("Failed to write block bitmap: {}", e)))?;
    }
    current_block += 1;
    
    // Inode bitmap
    let mut inode_bitmap_buffer = AlignedBuffer::<4096>::new();
    inode_bitmap.write_to_buffer(&mut inode_bitmap_buffer)
        .map_err(|e| MosesError::Other(format!("Failed to prepare inode bitmap: {}", e)))?;
    
    #[cfg(target_os = "windows")]
    device_io.write_aligned(current_block * 4096, &inode_bitmap_buffer[..])
        .map_err(|e| MosesError::Other(format!("Failed to write inode bitmap: {:?}", e)))?;
    
    #[cfg(not(target_os = "windows"))]
    {
        file.seek(SeekFrom::Start(current_block * 4096))
            .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
        file.write_all(&inode_bitmap_buffer[..])
            .map_err(|e| MosesError::Other(format!("Failed to write inode bitmap: {}", e)))?;
    }
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
    
    // Write inode table
    #[cfg(target_os = "windows")]
    {
        // Ensure inode table buffer is aligned to sector size
        let sector_size = 512;
        let aligned_size = ((inode_table_buffer.len() + sector_size - 1) / sector_size) * sector_size;
        if inode_table_buffer.len() < aligned_size {
            inode_table_buffer.resize(aligned_size, 0);
        }
        device_io.write_aligned(current_block * 4096, &inode_table_buffer)
            .map_err(|e| MosesError::Other(format!("Failed to write inode table: {:?}", e)))?;
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        file.seek(SeekFrom::Start(current_block * 4096))
            .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
        file.write_all(&inode_table_buffer)
            .map_err(|e| MosesError::Other(format!("Failed to write inode table: {}", e)))?;
    }
    
    // Write root directory data at its allocated block
    #[cfg(target_os = "windows")]
    device_io.write_aligned(dir_data_block * 4096, &dir_data)
        .map_err(|e| MosesError::Other(format!("Failed to write root directory: {:?}", e)))?;
    
    #[cfg(not(target_os = "windows"))]
    {
        file.seek(SeekFrom::Start(dir_data_block * 4096))
            .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
        file.write_all(&dir_data)
            .map_err(|e| MosesError::Other(format!("Failed to write root directory: {}", e)))?;
    }
    
    // Write lost+found directory data at its allocated block
    #[cfg(target_os = "windows")]
    device_io.write_aligned(lf_data_block * 4096, &lf_data)
        .map_err(|e| MosesError::Other(format!("Failed to write lost+found: {:?}", e)))?;
    
    #[cfg(not(target_os = "windows"))]
    {
        file.seek(SeekFrom::Start(lf_data_block * 4096))
            .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
        file.write_all(&lf_data)
            .map_err(|e| MosesError::Other(format!("Failed to write lost+found: {}", e)))?;
    }
    
    // Flush to disk
    #[cfg(target_os = "windows")]
    device_io.flush()
        .map_err(|e| MosesError::Other(format!("Failed to flush: {:?}", e)))?;
    
    #[cfg(not(target_os = "windows"))]
    file.sync_all()
        .map_err(|e| MosesError::Other(format!("Failed to sync device: {}", e)))?;
    
    Ok(())
}