// Helper functions for FilesystemOps implementations
use crate::device_reader::FilesystemInfo as ReaderFilesystemInfo;
use crate::ops::FilesystemInfo as OpsFilesystemInfo;

/// Convert from reader's FilesystemInfo to ops' FilesystemInfo
pub fn convert_filesystem_info(info: ReaderFilesystemInfo) -> OpsFilesystemInfo {
    let free_bytes = if info.total_bytes > info.used_bytes {
        info.total_bytes - info.used_bytes
    } else {
        0
    };
    
    let block_size = info.cluster_size.unwrap_or(4096) as u32;
    
    OpsFilesystemInfo {
        total_space: info.total_bytes,
        free_space: free_bytes,
        available_space: free_bytes,
        total_inodes: 0,  // FAT doesn't have inodes
        free_inodes: 0,
        block_size,
        fragment_size: block_size,
        max_filename_length: 255,
        filesystem_type: info.fs_type,
        volume_label: info.label,
        volume_uuid: None,
        is_readonly: false,
    }
}