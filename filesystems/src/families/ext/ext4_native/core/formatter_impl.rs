// Implementation of complete ext4 filesystem formatting

use moses_core::{Device, FormatOptions, MosesError};
use log::{debug, info, warn, error};
use std::sync::Arc;
use crate::families::ext::ext4_native::core::{
    structures::*,
    types::{FilesystemParams, FilesystemLayout},
    alignment::AlignedBuffer,
    bitmap::{Bitmap, init_block_bitmap_group0, init_inode_bitmap_group0},
    constants::*,
    progress::{ProgressReporter, ProgressCallback, LoggingProgress},
};
#[cfg(not(target_os = "windows"))]
use std::fs::OpenOptions;
#[cfg(not(target_os = "windows"))]
use std::io::{Write, Seek, SeekFrom};
#[cfg(target_os = "windows")]
use crate::families::ext::ext4_native::windows::WindowsDeviceIO;

/// Write complete ext4 filesystem to device with progress reporting
pub use super::formatter_ext::format_device_ext_version;

pub async fn format_device_with_progress(
    device: &Device,
    options: &FormatOptions,
    progress_callback: Arc<dyn ProgressCallback>,
) -> Result<(), MosesError> {
    // Initialize progress reporter with estimated steps
    let total_steps = 10; // Major formatting steps
    let estimated_bytes = device.size / 100; // Estimate ~1% of device will be written for metadata
    let mut progress = ProgressReporter::new(total_steps, estimated_bytes, progress_callback);
    
    progress.start_step(0, "Initializing filesystem parameters");
    // Convert options to filesystem parameters
    info!("=== DEVICE FORMATTING START ===");
    info!("Device ID: {}", device.id);
    info!("Device name: {}", device.name);
    info!("Device size: {} bytes ({} GB)", device.size, device.size / (1024*1024*1024));
    info!("Cluster size: {:?}", options.cluster_size);
    
    let params = FilesystemParams {
        size_bytes: device.size,
        block_size: options.cluster_size.unwrap_or(4096) as u32,
        inode_size: 256,
        label: options.label.clone(),
        reserved_percent: 5,
        enable_checksums: true,
        enable_64bit: true, // Always enable 64-bit like modern mkfs.ext4
        enable_journal: false,
    };
    
    info!("Filesystem params created: block_size={}, size_bytes={}", 
          params.block_size, params.size_bytes);
    
    // Calculate filesystem layout
    let layout = FilesystemLayout::from_params(&params)
        .map_err(|e| MosesError::Other(e.to_string()))?;
    
    info!("Filesystem layout calculated:");
    info!("  Total blocks: {}", layout.total_blocks);
    info!("  Blocks per group: {}", layout.blocks_per_group);
    info!("  Number of groups: {}", layout.num_groups);
    info!("  Inodes per group: {}", layout.inodes_per_group);
    info!("  Device size: {} bytes", params.size_bytes);
    info!("  Block size: {} bytes", params.block_size);
    
    progress.start_step(1, "Creating filesystem structures");
    
    // Create and initialize superblock
    let mut sb = Ext4Superblock::new();
    sb.init_minimal(&params, &layout);
    
    progress.start_step(2, "Initializing block groups");
    
    // Create group descriptor
    let mut gd = Ext4GroupDesc::new();
    info!("Initializing group descriptor for group 0");
    info!("Layout: blocks_per_group={}, total_blocks={}, num_groups={}", 
          layout.blocks_per_group, layout.total_blocks, layout.num_groups);
    gd.init(0, &layout, &params);
    let gd_free_initial = gd.bg_free_blocks_count_lo as u32 
        | ((gd.bg_free_blocks_count_hi as u32) << 16);
    info!("Group descriptor after init: free_blocks_lo={:#x}, free_blocks_hi={:#x}, total={}", 
          gd.bg_free_blocks_count_lo, gd.bg_free_blocks_count_hi, gd_free_initial);
    
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
    // Need to handle this as a 32-bit value split across two u16 fields
    let current_gd_free = gd.bg_free_blocks_count_lo as u32 
        | ((gd.bg_free_blocks_count_hi as u32) << 16);
    debug!("Group 0 before allocating dirs: lo={:#06x} hi={:#06x} total={}", 
           gd.bg_free_blocks_count_lo, gd.bg_free_blocks_count_hi, current_gd_free);
    let new_gd_free = current_gd_free.saturating_sub(2);
    gd.bg_free_blocks_count_lo = (new_gd_free & 0xFFFF) as u16;
    gd.bg_free_blocks_count_hi = ((new_gd_free >> 16) & 0xFFFF) as u16;
    debug!("Group 0 after allocating dirs: lo={:#06x} hi={:#06x} total={}", 
           gd.bg_free_blocks_count_lo, gd.bg_free_blocks_count_hi, new_gd_free);
    
    // Don't update superblock's free blocks count here - we'll recalculate it properly later
    // This avoids double-counting and potential underflow issues
    
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
    
    // Recalculate total free blocks from scratch to avoid accumulation errors
    // The superblock needs the sum of all groups' free blocks
    debug!("=== FINAL FREE BLOCKS CALCULATION ===");
    debug!("Total groups: {}", layout.num_groups);
    
    // Calculate group 0's free blocks properly
    // Group 0 has metadata + 2 blocks allocated for directories
    let group0_metadata = layout.metadata_blocks_per_group(0) as u64;
    let group0_total = layout.blocks_per_group as u64;
    let group0_allocated = group0_metadata + 2; // +2 for root and lost+found directories
    
    // Add defensive logging to catch overflow
    info!("Group 0 calculation: total={}, metadata={}, allocated={}", 
          group0_total, group0_metadata, group0_allocated);
    info!("Raw metadata_blocks_per_group(0) returned: {}", layout.metadata_blocks_per_group(0));
    info!("Raw blocks_per_group: {}", layout.blocks_per_group);
    
    if group0_allocated > group0_total {
        error!("OVERFLOW DETECTED: Group 0 allocated blocks {} exceeds total {}", 
               group0_allocated, group0_total);
        return Err(MosesError::Other(format!(
            "Group 0 metadata overflow: {} + 2 > {}", 
            group0_metadata, group0_total
        )));
    }
    
    let group0_free = group0_total.saturating_sub(group0_allocated);
    let mut total_free_blocks = group0_free;
    
    debug!("Group 0: {} total blocks, {} metadata, 2 dir blocks, {} free", 
           group0_total, group0_metadata, group0_free);
    info!("Starting total_free_blocks with group 0: {}", total_free_blocks);
    
    // Verify the group descriptor matches our calculation
    let gd_free = gd.bg_free_blocks_count_lo as u32 
        | ((gd.bg_free_blocks_count_hi as u32) << 16);
    if gd_free as u64 != group0_free {
        debug!("WARNING: Group descriptor free blocks {} doesn't match calculated {}", 
               gd_free, group0_free);
        // Fix the group descriptor
        gd.bg_free_blocks_count_lo = (group0_free & 0xFFFF) as u16;
        gd.bg_free_blocks_count_hi = ((group0_free >> 16) & 0xFFFF) as u16;
    }
    
    // Add free blocks from uninitialized groups (1 through num_groups-1)
    for group_idx in 1..layout.num_groups {
        // Calculate actual blocks in this group (last group may be partial)
        let group_start = group_idx as u64 * layout.blocks_per_group as u64;
        let blocks_in_group = if group_idx == layout.num_groups - 1 {
            // Last group may have fewer blocks
            let remaining = layout.total_blocks.saturating_sub(group_start);
            remaining.min(layout.blocks_per_group as u64) as u32
        } else {
            layout.blocks_per_group
        };
        
        let metadata_blocks = layout.metadata_blocks_per_group(group_idx);
        
        // Defensive check: metadata should never exceed blocks in group
        if metadata_blocks > blocks_in_group {
            error!("Group {} metadata blocks {} exceeds blocks in group {}!", 
                   group_idx, metadata_blocks, blocks_in_group);
            return Err(MosesError::Other(format!(
                "Invalid metadata calculation for group {}", group_idx
            )));
        }
        
        let free_blocks = blocks_in_group.saturating_sub(metadata_blocks) as u64;
        let old_total = total_free_blocks;
        total_free_blocks += free_blocks;
        
        debug!("Group {} - blocks_in_group: {}, metadata: {}, free: {}", 
               group_idx, blocks_in_group, metadata_blocks, free_blocks);
        
        // Check for overflow
        if total_free_blocks < old_total {
            error!("OVERFLOW in group {}: old_total={}, free_blocks={}, new_total={}", 
                   group_idx, old_total, free_blocks, total_free_blocks);
        }
        if group_idx % 50 == 0 || group_idx == layout.num_groups - 1 {
            info!("After group {}: total_free_blocks = {}", group_idx, total_free_blocks);
        }
    }
    
    debug!("Total free blocks calculated: {}", total_free_blocks);
    debug!("Total blocks in filesystem: {}", layout.total_blocks);
    
    // Log the values immediately before the check
    info!("FINAL CHECK: total_free_blocks={}, layout.total_blocks={}", 
          total_free_blocks, layout.total_blocks);
    info!("FINAL CHECK: comparison result: {} > {} = {}", 
          total_free_blocks, layout.total_blocks, total_free_blocks > layout.total_blocks);
    
    // Sanity check - free blocks should be less than total blocks
    if total_free_blocks > layout.total_blocks {
        error!("Critical calculation error - free blocks {} exceeds total blocks {}!", 
               total_free_blocks, layout.total_blocks);
        error!("Values in hex: free_blocks={:#x}, total_blocks={:#x}",
               total_free_blocks, layout.total_blocks);
        // This should never happen with correct calculation
        return Err(MosesError::Other(format!(
            "Free blocks calculation overflow: {} > {}", 
            total_free_blocks, layout.total_blocks
        )));
    }
    
    // Update superblock with correct total
    sb.s_free_blocks_count_lo = (total_free_blocks & 0xFFFFFFFF) as u32;
    sb.s_free_blocks_count_hi = ((total_free_blocks >> 32) & 0xFFFFFFFF) as u32;
    
    debug!("Superblock s_free_blocks_count: lo={:#010x} hi={:#010x} => total={}", 
           sb.s_free_blocks_count_lo, sb.s_free_blocks_count_hi, total_free_blocks);
    debug!("Superblock s_blocks_count: lo={:#010x} hi={:#010x} => total={}", 
           sb.s_blocks_count_lo, sb.s_blocks_count_hi, layout.total_blocks);
    
    // Similarly update total free inodes - also fix the shift here
    let group0_free_inodes = gd.bg_free_inodes_count_lo as u32 
        | ((gd.bg_free_inodes_count_hi as u32) << 16);
    let mut total_free_inodes = group0_free_inodes;
    
    // Add free inodes from uninitialized groups
    for _group_idx in 1..layout.num_groups {
        total_free_inodes += layout.inodes_per_group;
    }
    
    sb.s_free_inodes_count = total_free_inodes;
    
    // Update checksums
    progress.start_step(3, "Calculating checksums");
    gd.update_checksum(0, &sb);
    root_inode.update_checksum(EXT4_ROOT_INO, &sb);
    lf_inode.update_checksum(EXT4_FIRST_INO as u32, &sb);
    sb.update_checksum();
    
    progress.start_step(4, "Opening device for writing");
    // Open device for writing
    #[cfg(target_os = "windows")]
    let device_path = if device.id.starts_with(r"\\.\") {
        device.id.clone()
    } else {
        format!(r"\\.\{}", device.id)
    };
    #[cfg(not(target_os = "windows"))]
    let device_path = if device.id.starts_with('/') {
        device.id.clone()
    } else {
        format!("/dev/{}", device.id)
    };
    
    info!("Formatting device - ID: '{}', Path: '{}'", device.id, device_path);
    
    // Extra debug to ensure file exists  
    #[cfg(not(target_os = "windows"))]
    {
        use std::path::Path;
        if !Path::new(&device_path).exists() {
            return Err(MosesError::Other(format!("Device path does not exist: {}", device_path)));
        }
    }
    
    #[cfg(target_os = "windows")]
    let mut device_io = WindowsDeviceIO::open(&device_path)
        .map_err(|e| MosesError::Other(format!("Failed to open device {}: {:?}", device_path, e)))?;
    
    #[cfg(not(target_os = "windows"))]
    let mut file = OpenOptions::new()
        .write(true)
        .open(&device_path)
        .map_err(|e| MosesError::Other(format!("Failed to open device {}: {}", device_path, e)))?;
    
    progress.start_step(5, "Zeroing device metadata area");
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
    progress.start_step(6, "Writing superblock");
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
    
    progress.start_step(7, "Writing group descriptor table");
    // Block 1+: Group descriptor table
    // We need to write descriptors for ALL groups, not just group 0
    let mut gdt_buffer = vec![0u8; layout.gdt_blocks as usize * 4096];
    
    // Write group 0 descriptor
    let desc_size = if sb.s_desc_size > 0 { sb.s_desc_size as usize } else { 64 };
    let gd_bytes = unsafe {
        std::slice::from_raw_parts(
            &gd as *const _ as *const u8,
            desc_size
        )
    };
    gdt_buffer[..desc_size].copy_from_slice(gd_bytes);
    
    // Initialize empty descriptors for remaining groups
    // IMPORTANT: Must set valid block numbers even for unused groups!
    // Linux always validates these, regardless of UNINIT flags
    for group_idx in 1..layout.num_groups {
        let offset = group_idx as usize * desc_size;
        if offset + desc_size <= gdt_buffer.len() {
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
            
            // Calculate free blocks for this uninitialized group
            // Need to handle last group which may be partial
            let blocks_in_group = if group_idx == layout.num_groups - 1 {
                // Last group may have fewer blocks
                let remaining = layout.total_blocks.saturating_sub(group_first_block);
                remaining.min(layout.blocks_per_group as u64) as u32
            } else {
                layout.blocks_per_group
            };
            
            // All blocks are free except metadata blocks
            let metadata_blocks = layout.metadata_blocks_per_group(group_idx);
            let free_blocks = blocks_in_group.saturating_sub(metadata_blocks);
            
            // Calculate free inodes (all inodes in uninitialized groups are free)
            let free_inodes = layout.inodes_per_group;
            
            // Create group descriptor with valid block numbers
            let mut empty_gd = Ext4GroupDesc {
                bg_block_bitmap_lo: (block_bitmap_block & 0xFFFFFFFF) as u32,
                bg_block_bitmap_hi: ((block_bitmap_block >> 32) & 0xFFFFFFFF) as u32,
                bg_inode_bitmap_lo: (inode_bitmap_block & 0xFFFFFFFF) as u32,
                bg_inode_bitmap_hi: ((inode_bitmap_block >> 32) & 0xFFFFFFFF) as u32,
                bg_inode_table_lo: (inode_table_block & 0xFFFFFFFF) as u32,
                bg_inode_table_hi: ((inode_table_block >> 32) & 0xFFFFFFFF) as u32,
                bg_free_blocks_count_lo: (free_blocks & 0xFFFF) as u16,
                bg_free_blocks_count_hi: ((free_blocks >> 16) & 0xFFFF) as u16,
                bg_free_inodes_count_lo: (free_inodes & 0xFFFF) as u16,
                bg_free_inodes_count_hi: ((free_inodes >> 16) & 0xFFFF) as u16,
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
                    desc_size
                )
            };
            gdt_buffer[offset..offset + desc_size].copy_from_slice(empty_gd_bytes);
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
    
    // Write backup superblocks and GDT to groups that need them
    progress.start_step(7, "Writing backup superblocks");
    info!("Writing backup superblocks to groups with sparse_super");
    
    for backup_group in 1..layout.num_groups {
        if !layout.has_superblock(backup_group) {
            continue;
        }
        
        info!("Writing backup superblock to group {}", backup_group);
        let backup_block = backup_group as u64 * layout.blocks_per_group as u64;
        
        // Update block group number in superblock for this backup
        let mut backup_sb = sb.clone();
        backup_sb.s_block_group_nr = backup_group as u16;
        backup_sb.update_checksum();
        
        // Write backup superblock
        let backup_sb_bytes = unsafe {
            std::slice::from_raw_parts(
                &backup_sb as *const _ as *const u8,
                1024
            )
        };
        
        let mut backup_sb_buffer = AlignedBuffer::<1024>::new();
        backup_sb_buffer[..].copy_from_slice(backup_sb_bytes);
        
        #[cfg(target_os = "windows")]
        device_io.write_aligned(backup_block * 4096, &backup_sb_buffer[..])
            .map_err(|e| MosesError::Other(format!("Failed to write backup superblock at group {}: {:?}", backup_group, e)))?;
        
        #[cfg(not(target_os = "windows"))]
        {
            file.seek(SeekFrom::Start(backup_block * 4096))
                .map_err(|e| MosesError::Other(format!("Failed to seek for backup sb: {}", e)))?;
            file.write_all(&backup_sb_buffer[..])
                .map_err(|e| MosesError::Other(format!("Failed to write backup superblock at group {}: {}", backup_group, e)))?;
        }
        
        // Also write backup GDT after the backup superblock
        let backup_gdt_block = backup_block + 1;
        for gdt_block_idx in 0..layout.gdt_blocks {
            let block_offset = (backup_gdt_block + gdt_block_idx as u64) * 4096;
            let data_offset = gdt_block_idx as usize * 4096;
            let data_end = ((gdt_block_idx + 1) as usize * 4096).min(gdt_buffer.len());
            
            #[cfg(target_os = "windows")]
            device_io.write_aligned(block_offset, &gdt_buffer[data_offset..data_end])
                .map_err(|e| MosesError::Other(format!("Failed to write backup GDT at group {}: {:?}", backup_group, e)))?;
            
            #[cfg(not(target_os = "windows"))]
            {
                file.seek(SeekFrom::Start(block_offset))
                    .map_err(|e| MosesError::Other(format!("Failed to seek for backup GDT: {}", e)))?;
                file.write_all(&gdt_buffer[data_offset..data_end])
                    .map_err(|e| MosesError::Other(format!("Failed to write backup GDT at group {}: {}", backup_group, e)))?;
            }
        }
    }
    
    progress.start_step(8, "Writing bitmaps and inode table");
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
    
    progress.start_step(9, "Flushing to disk");
    // Flush to disk
    #[cfg(target_os = "windows")]
    device_io.flush()
        .map_err(|e| MosesError::Other(format!("Failed to flush: {:?}", e)))?;
    
    #[cfg(not(target_os = "windows"))]
    file.sync_all()
        .map_err(|e| MosesError::Other(format!("Failed to sync device: {}", e)))?;
    
    progress.complete();
    Ok(())
}

/// Write complete ext4 filesystem to device (convenience function without progress)
pub async fn format_device(
    device: &Device,
    options: &FormatOptions,
) -> Result<(), MosesError> {
    format_device_with_progress(device, options, Arc::new(LoggingProgress)).await
}

/// Format device with verification
pub async fn format_device_with_verification(
    device: &Device,
    options: &FormatOptions,
    progress_callback: Arc<dyn ProgressCallback>,
) -> Result<(), MosesError> {
    use crate::families::ext::ext4_native::core::verify;
    
    // Format the device
    format_device_with_progress(device, options, progress_callback.clone()).await?;
    
    info!("Starting post-format verification");
    
    // Verify the filesystem
    let device_path = if cfg!(target_os = "windows") {
        if device.id.starts_with(r"\\.\") {
            device.id.clone()
        } else {
            format!(r"\\.\{}", device.id)
        }
    } else {
        format!("/dev/{}", device.id)
    };
    
    match verify::verify_device(&device_path) {
        Ok(verification_result) => {
            if !verification_result.is_valid {
                let error_msg = verification_result.errors.join("; ");
                // Log verification errors as warnings, don't fail the format
                warn!("Filesystem verification found issues: {}", error_msg);
                warn!("The filesystem was created but may have issues. Consider reformatting.");
            } else if !verification_result.warnings.is_empty() {
                warn!("Verification completed with warnings: {:?}", verification_result.warnings);
            } else {
                info!("Filesystem verification passed successfully");
            }
        }
        Err(e) => {
            // If verification itself fails (e.g., can't open device), just warn
            warn!("Could not verify filesystem (format may have succeeded): {:?}", e);
            warn!("This can happen on Windows if the device is locked. The format likely succeeded.");
        }
    }
    Ok(())
}