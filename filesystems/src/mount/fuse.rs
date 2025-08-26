// FUSE filesystem implementation for Linux/macOS
// This bridges Moses FilesystemOps to FUSE API using the fuser crate

use super::{MountOptions, MountProvider};
use crate::ops::{FilesystemOps, FileAttributes};
use moses_core::{Device, MosesError};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::ffi::OsStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::HashMap;

#[cfg(all(unix, feature = "mount-unix"))]
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory,
    ReplyEntry, ReplyEmpty, ReplyOpen, ReplyStatfs, Request, TimeOrNow,
};

/// Convert Moses FileAttributes to FUSE FileAttr
#[cfg(all(unix, feature = "mount-unix"))]
fn convert_to_fuse_attr(attrs: &FileAttributes, ino: u64) -> FileAttr {
    let kind = if attrs.is_directory {
        FileType::Directory
    } else if attrs.is_symlink {
        FileType::Symlink
    } else {
        FileType::RegularFile
    };
    
    let perm = attrs.permissions as u16;
    let uid = attrs.owner.unwrap_or(1000);
    let gid = attrs.group.unwrap_or(1000);
    
    let atime = attrs.accessed
        .map(|t| UNIX_EPOCH + Duration::from_secs(t))
        .unwrap_or_else(SystemTime::now);
    let mtime = attrs.modified
        .map(|t| UNIX_EPOCH + Duration::from_secs(t))
        .unwrap_or_else(SystemTime::now);
    let ctime = attrs.created
        .map(|t| UNIX_EPOCH + Duration::from_secs(t))
        .unwrap_or_else(SystemTime::now);
    
    FileAttr {
        ino,
        size: attrs.size,
        blocks: (attrs.size + 511) / 512,  // Number of 512-byte blocks
        atime,
        mtime,
        ctime,
        crtime: ctime,  // macOS creation time
        kind,
        perm,
        nlink: if attrs.is_directory { 2 } else { 1 },
        uid,
        gid,
        rdev: 0,
        blksize: 4096,
        flags: 0,  // macOS only
    }
}

/// Moses FUSE filesystem implementation
#[cfg(all(unix, feature = "mount-unix"))]
struct MosesFuseFilesystem {
    ops: Arc<Mutex<Box<dyn FilesystemOps>>>,
    device: Device,
    readonly: bool,
    
    // Inode management
    inode_counter: Arc<Mutex<u64>>,
    path_to_inode: Arc<Mutex<HashMap<PathBuf, u64>>>,
    inode_to_path: Arc<Mutex<HashMap<u64, PathBuf>>>,
    
    // File handle management  
    handle_counter: Arc<Mutex<u64>>,
    handles: Arc<Mutex<HashMap<u64, PathBuf>>>,
}

#[cfg(all(unix, feature = "mount-unix"))]
impl MosesFuseFilesystem {
    fn new(ops: Box<dyn FilesystemOps>, device: Device, readonly: bool) -> Self {
        let mut path_to_inode = HashMap::new();
        let mut inode_to_path = HashMap::new();
        
        // Root directory always has inode 1
        path_to_inode.insert(PathBuf::from("/"), 1);
        inode_to_path.insert(1, PathBuf::from("/"));
        
        Self {
            ops: Arc::new(Mutex::new(ops)),
            device,
            readonly,
            inode_counter: Arc::new(Mutex::new(2)), // Start at 2, 1 is root
            path_to_inode: Arc::new(Mutex::new(path_to_inode)),
            inode_to_path: Arc::new(Mutex::new(inode_to_path)),
            handle_counter: Arc::new(Mutex::new(1)),
            handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    fn get_or_create_inode(&self, path: &Path) -> u64 {
        let mut path_to_inode = self.path_to_inode.lock().unwrap();
        
        if let Some(&ino) = path_to_inode.get(path) {
            return ino;
        }
        
        let mut counter = self.inode_counter.lock().unwrap();
        let ino = *counter;
        *counter += 1;
        
        path_to_inode.insert(path.to_path_buf(), ino);
        self.inode_to_path.lock().unwrap().insert(ino, path.to_path_buf());
        
        ino
    }
    
    fn get_path_from_inode(&self, ino: u64) -> Option<PathBuf> {
        self.inode_to_path.lock().unwrap().get(&ino).cloned()
    }
}

#[cfg(all(unix, feature = "mount-unix"))]
impl Filesystem for MosesFuseFilesystem {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let parent_path = match self.get_path_from_inode(parent) {
            Some(p) => p,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };
        
        let path = parent_path.join(name);
        let mut ops = self.ops.lock().unwrap();
        
        match ops.stat(&path) {
            Ok(attrs) => {
                let ino = self.get_or_create_inode(&path);
                let attr = convert_to_fuse_attr(&attrs, ino);
                let ttl = Duration::from_secs(1);
                reply.entry(&ttl, &attr, 0);
            }
            Err(_) => {
                reply.error(libc::ENOENT);
            }
        }
    }
    
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        let path = match self.get_path_from_inode(ino) {
            Some(p) => p,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };
        
        let mut ops = self.ops.lock().unwrap();
        
        match ops.stat(&path) {
            Ok(attrs) => {
                let attr = convert_to_fuse_attr(&attrs, ino);
                let ttl = Duration::from_secs(1);
                reply.attr(&ttl, &attr);
            }
            Err(e) => {
                log::error!("Failed to stat {:?}: {}", path, e);
                reply.error(libc::ENOENT);
            }
        }
    }
    
    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let path = match self.get_path_from_inode(ino) {
            Some(p) => p,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };
        
        let mut ops = self.ops.lock().unwrap();
        
        match ops.read(&path, offset as u64, size) {
            Ok(data) => {
                reply.data(&data);
            }
            Err(e) => {
                log::error!("Failed to read {:?}: {}", path, e);
                reply.error(libc::EIO);
            }
        }
    }
    
    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let path = match self.get_path_from_inode(ino) {
            Some(p) => p,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };
        
        let mut ops = self.ops.lock().unwrap();
        
        match ops.readdir(&path) {
            Ok(entries) => {
                let mut idx = 0i64;
                
                // Add . and .. entries
                if offset <= idx {
                    if reply.add(ino, idx + 1, FileType::Directory, ".") {
                        reply.ok();
                        return;
                    }
                }
                idx += 1;
                
                if offset <= idx {
                    let parent_ino = if path == Path::new("/") { 1 } else {
                        self.get_or_create_inode(path.parent().unwrap_or(Path::new("/")))
                    };
                    if reply.add(parent_ino, idx + 1, FileType::Directory, "..") {
                        reply.ok();
                        return;
                    }
                }
                idx += 1;
                
                // Add regular entries
                for entry in entries {
                    if offset <= idx {
                        let entry_path = path.join(&entry.name);
                        let entry_ino = self.get_or_create_inode(&entry_path);
                        
                        let kind = if entry.attributes.is_directory {
                            FileType::Directory
                        } else if entry.attributes.is_symlink {
                            FileType::Symlink
                        } else {
                            FileType::RegularFile
                        };
                        
                        if reply.add(entry_ino, idx + 1, kind, &entry.name) {
                            reply.ok();
                            return;
                        }
                    }
                    idx += 1;
                }
                
                reply.ok();
            }
            Err(e) => {
                log::error!("Failed to readdir {:?}: {}", path, e);
                reply.error(libc::EIO);
            }
        }
    }
    
    fn open(&mut self, _req: &Request, ino: u64, flags: i32, reply: ReplyOpen) {
        // Check if trying to open for write on readonly filesystem
        if self.readonly && (flags & libc::O_WRONLY != 0 || flags & libc::O_RDWR != 0) {
            reply.error(libc::EROFS);
            return;
        }
        
        let path = match self.get_path_from_inode(ino) {
            Some(p) => p,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };
        
        // Generate a file handle
        let mut handle_counter = self.handle_counter.lock().unwrap();
        let fh = *handle_counter;
        *handle_counter += 1;
        
        self.handles.lock().unwrap().insert(fh, path);
        
        reply.opened(fh, flags as u32);
    }
    
    fn release(
        &mut self,
        _req: &Request,
        _ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        self.handles.lock().unwrap().remove(&fh);
        reply.ok();
    }
    
    fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
        let mut ops = self.ops.lock().unwrap();
        
        match ops.statfs() {
            Ok(info) => {
                reply.statfs(
                    info.total_space / info.block_size as u64,  // Total blocks
                    info.free_space / info.block_size as u64,    // Free blocks
                    info.available_space / info.block_size as u64, // Available blocks
                    info.total_inodes,                            // Total inodes
                    info.free_inodes,                             // Free inodes
                    info.block_size,                              // Block size
                    info.max_filename_length,                     // Max name length
                    info.fragment_size,                           // Fragment size
                );
            }
            Err(e) => {
                log::error!("Failed to statfs: {}", e);
                // Return some reasonable defaults
                reply.statfs(
                    1000000,  // blocks
                    500000,   // bfree
                    500000,   // bavail
                    100000,   // files
                    50000,    // ffree
                    4096,     // bsize
                    255,      // namelen
                    4096,     // frsize
                );
            }
        }
    }
    
    // Write operations - all return error for read-only filesystem
    fn write(
        &mut self,
        _req: &Request,
        _ino: u64,
        _fh: u64,
        _offset: i64,
        _data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        if self.readonly {
            reply.error(libc::EROFS);
        } else {
            // TODO: Implement write when FilesystemOps supports it
            reply.error(libc::ENOSYS);
        }
    }
    
    fn mkdir(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        reply.error(libc::EROFS);
    }
    
    fn unlink(&mut self, _req: &Request, _parent: u64, _name: &OsStr, reply: ReplyEmpty) {
        reply.error(libc::EROFS);
    }
    
    fn rmdir(&mut self, _req: &Request, _parent: u64, _name: &OsStr, reply: ReplyEmpty) {
        reply.error(libc::EROFS);
    }
    
    fn rename(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _newparent: u64,
        _newname: &OsStr,
        _flags: u32,
        reply: ReplyEmpty,
    ) {
        reply.error(libc::EROFS);
    }
}

/// FUSE mount provider implementation
#[cfg(all(unix, feature = "mount-unix"))]
pub struct FuseMount {
    mounts: Vec<(PathBuf, std::thread::JoinHandle<()>)>,
}

#[cfg(all(unix, feature = "mount-unix"))]
impl FuseMount {
    pub fn new() -> Result<Self, MosesError> {
        Ok(Self {
            mounts: Vec::new(),
        })
    }
}

#[cfg(all(unix, feature = "mount-unix"))]
impl MountProvider for FuseMount {
    fn mount(
        &mut self,
        device: &Device,
        mut ops: Box<dyn FilesystemOps>,
        options: &MountOptions,
    ) -> Result<(), MosesError> {
        // Initialize the filesystem ops
        ops.init(device)?;
        
        // Create the FUSE filesystem
        let fs = MosesFuseFilesystem::new(ops, device.clone(), options.readonly);
        
        // Prepare mount options
        let mut mount_options = vec![MountOption::FSName(format!("moses.{}", fs.ops.lock().unwrap().filesystem_type()))];
        
        if options.readonly {
            mount_options.push(MountOption::RO);
        }
        
        if options.allow_other {
            mount_options.push(MountOption::AllowOther);
        }
        
        if options.direct_io {
            mount_options.push(MountOption::DirectIO);
        }
        
        // Mount in a separate thread
        let mount_point = PathBuf::from(&options.mount_point);
        let mount_point_clone = mount_point.clone();
        
        let handle = std::thread::spawn(move || {
            log::info!("Mounting FUSE filesystem at {:?}", mount_point_clone);
            
            if let Err(e) = fuser::mount2(fs, &mount_point_clone, &mount_options) {
                log::error!("FUSE mount failed: {}", e);
            }
        });
        
        // Give it a moment to mount
        std::thread::sleep(Duration::from_millis(500));
        
        // Check if mount point exists
        if !mount_point.exists() {
            return Err(MosesError::Other(format!(
                "Mount point {} does not exist. Create it first with: sudo mkdir -p {}",
                options.mount_point, options.mount_point
            )));
        }
        
        self.mounts.push((mount_point, handle));
        
        log::info!("Successfully mounted {} at {}", device.name, options.mount_point);
        Ok(())
    }
    
    fn unmount(&mut self, mount_point: &Path) -> Result<(), MosesError> {
        // Find the mount
        if let Some(index) = self.mounts.iter().position(|(path, _)| path == mount_point) {
            let (path, _handle) = self.mounts.remove(index);
            
            // Use fusermount to unmount
            #[cfg(target_os = "linux")]
            {
                let result = std::process::Command::new("fusermount")
                    .arg("-u")
                    .arg(&path)
                    .status();
                    
                match result {
                    Ok(status) if status.success() => {
                        log::info!("Successfully unmounted {:?}", path);
                        Ok(())
                    }
                    _ => {
                        log::warn!("fusermount failed, trying umount");
                        let result = std::process::Command::new("umount")
                            .arg(&path)
                            .status();
                            
                        match result {
                            Ok(status) if status.success() => Ok(()),
                            _ => Err(MosesError::Other(format!("Failed to unmount {:?}", path)))
                        }
                    }
                }
            }
            
            #[cfg(target_os = "macos")]
            {
                let result = std::process::Command::new("umount")
                    .arg(&path)
                    .status();
                    
                match result {
                    Ok(status) if status.success() => {
                        log::info!("Successfully unmounted {:?}", path);
                        Ok(())
                    }
                    _ => Err(MosesError::Other(format!("Failed to unmount {:?}", path)))
                }
            }
        } else {
            Err(MosesError::Other(format!("No filesystem mounted at {:?}", mount_point)))
        }
    }
    
    fn is_mounted(&self, mount_point: &Path) -> bool {
        self.mounts.iter().any(|(path, _)| path == mount_point)
    }
}

// Stub implementation when FUSE is not available
#[cfg(not(all(unix, feature = "mount-unix")))]
pub struct FuseMount;

#[cfg(not(all(unix, feature = "mount-unix")))]
impl FuseMount {
    pub fn new() -> Result<Self, MosesError> {
        Err(MosesError::NotSupported(
            "FUSE support not compiled in. Build with --features mount-unix".to_string()
        ))
    }
}

#[cfg(not(all(unix, feature = "mount-unix")))]
impl MountProvider for FuseMount {
    fn mount(
        &mut self,
        _device: &Device,
        _ops: Box<dyn FilesystemOps>,
        _options: &MountOptions,
    ) -> Result<(), MosesError> {
        Err(MosesError::NotSupported(
            "FUSE support not available on this platform".to_string()
        ))
    }
    
    fn unmount(&mut self, _mount_point: &Path) -> Result<(), MosesError> {
        Err(MosesError::NotSupported(
            "FUSE support not available on this platform".to_string()
        ))
    }
    
    fn is_mounted(&self, _mount_point: &Path) -> bool {
        false
    }
}