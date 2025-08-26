// Path Resolution for EXT4 Writer
// Handles resolving filesystem paths to inode numbers

use super::*;
use std::path::{Path, PathBuf};
use moses_core::MosesError;

impl Ext4Writer {
    /// Resolve a full path to an inode number by walking the directory tree
    pub(super) fn resolve_path_full(&mut self, path: &Path) -> Result<u32, MosesError> {
        // Check cache first
        if let Some(&inode) = self.dir_cache.get(path) {
            return Ok(inode);
        }
        
        // Handle root directory
        if path == Path::new("/") {
            self.dir_cache.insert(PathBuf::from("/"), EXT4_ROOT_INO);
            return Ok(EXT4_ROOT_INO);
        }
        
        // Start from root
        let mut current_inode = EXT4_ROOT_INO;
        let mut current_path = PathBuf::from("/");
        
        // Walk through each path component
        for component in path.components() {
            match component {
                std::path::Component::RootDir => {
                    // Already handled above
                    continue;
                }
                std::path::Component::Normal(name) => {
                    let name_str = name.to_str()
                        .ok_or_else(|| MosesError::InvalidInput("Invalid UTF-8 in path".to_string()))?;
                    
                    // Look up the component in the current directory
                    match self.lookup_in_directory(current_inode, name_str)? {
                        Some(next_inode) => {
                            current_inode = next_inode;
                            current_path.push(name);
                            
                            // Cache intermediate directories
                            let inode = self.read_inode(current_inode)?;
                            if inode.i_mode & 0xF000 == 0x4000 { // Is directory
                                self.dir_cache.insert(current_path.clone(), current_inode);
                            }
                        }
                        None => {
                            return Err(MosesError::Other(format!(
                                "Path component '{}' not found in '{}'", 
                                name_str, 
                                current_path.display()
                            )));
                        }
                    }
                }
                std::path::Component::CurDir => {
                    // Current directory "." - no change
                    continue;
                }
                std::path::Component::ParentDir => {
                    // Parent directory ".."
                    // We need to look up ".." in the current directory
                    match self.lookup_in_directory(current_inode, "..")? {
                        Some(parent_inode) => {
                            current_inode = parent_inode;
                            current_path.pop();
                        }
                        None => {
                            // This shouldn't happen if directory is well-formed
                            return Err(MosesError::Other("Parent directory not found".to_string()));
                        }
                    }
                }
                _ => {
                    return Err(MosesError::InvalidInput("Unsupported path component".to_string()));
                }
            }
        }
        
        // Cache the final result if it's a directory
        let inode = self.read_inode(current_inode)?;
        if inode.i_mode & 0xF000 == 0x4000 { // Is directory
            self.dir_cache.insert(path.to_path_buf(), current_inode);
        }
        
        Ok(current_inode)
    }
    
    /// Update the implementation of resolve_path in helpers.rs to use this
    pub(super) fn resolve_path_impl(&mut self, path: &Path) -> Result<u32, MosesError> {
        self.resolve_path_full(path)
    }
    
    /// Check if a path exists
    pub(super) fn path_exists(&mut self, path: &Path) -> bool {
        self.resolve_path_full(path).is_ok()
    }
    
    /// Get the parent directory of a path
    pub(super) fn get_parent_dir(&mut self, path: &Path) -> Result<u32, MosesError> {
        let parent = path.parent()
            .ok_or_else(|| MosesError::InvalidInput("Path has no parent".to_string()))?;
        self.resolve_path_full(parent)
    }
    
    /// Split a path into parent directory and filename
    pub(super) fn split_path(&mut self, path: &Path) -> Result<(u32, String), MosesError> {
        let parent = path.parent()
            .ok_or_else(|| MosesError::InvalidInput("Path has no parent".to_string()))?;
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| MosesError::InvalidInput("Invalid filename".to_string()))?;
        
        let parent_inode = self.resolve_path_full(parent)?;
        Ok((parent_inode, filename.to_string()))
    }
}