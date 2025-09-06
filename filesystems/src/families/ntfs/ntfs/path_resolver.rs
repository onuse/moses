// NTFS Path Resolver - Navigate directory hierarchies
// Implements full path resolution for subdirectories

use super::reader::NtfsReader;
use super::structures::*;
use super::index::IndexEntry;
use moses_core::MosesError;
use log::{debug, trace};
use std::collections::HashMap;

/// Path resolver for NTFS filesystem
pub struct PathResolver {
    /// Cache of path to MFT record mappings
    path_cache: HashMap<String, u64>,
}

impl PathResolver {
    /// Create a new path resolver
    pub fn new() -> Self {
        let mut path_cache = HashMap::new();
        // Pre-populate with well-known directories
        path_cache.insert("/".to_string(), MFT_RECORD_ROOT);
        
        Self { path_cache }
    }
    
    /// Resolve a full path to an MFT record number
    pub fn resolve_path(&mut self, reader: &mut NtfsReader, path: &str) -> Result<u64, MosesError> {
        // Normalize the path
        let normalized_path = self.normalize_path(path);
        
        // Check cache first
        if let Some(&mft_num) = self.path_cache.get(&normalized_path) {
            debug!("Path '{}' found in cache: MFT {}", normalized_path, mft_num);
            return Ok(mft_num);
        }
        
        // Special case for root
        if normalized_path == "/" {
            return Ok(MFT_RECORD_ROOT);
        }
        
        // Split path into components
        let components: Vec<&str> = normalized_path
            .trim_start_matches('/')
            .split('/')
            .filter(|c| !c.is_empty())
            .collect();
        
        if components.is_empty() {
            return Ok(MFT_RECORD_ROOT);
        }
        
        debug!("Resolving path '{}' with {} components", normalized_path, components.len());
        
        // Start from root and navigate down
        let mut current_mft = MFT_RECORD_ROOT;
        let mut current_path = String::from("/");
        
        for component in components {
            trace!("Looking for '{}' in directory MFT {}", component, current_mft);
            
            // Find the component in the current directory
            let entries = self.list_directory_by_mft(reader, current_mft)?;
            
            let entry = entries.iter()
                .find(|e| e.file_name.eq_ignore_ascii_case(component))
                .ok_or_else(|| MosesError::Other(format!(
                    "Path component '{}' not found in '{}'", component, current_path
                )))?;
            
            current_mft = entry.mft_reference;
            
            // Update current path
            if current_path == "/" {
                current_path = format!("/{}", component);
            } else {
                current_path = format!("{}/{}", current_path, component);
            }
            
            // Cache this intermediate path
            self.path_cache.insert(current_path.clone(), current_mft);
        }
        
        debug!("Resolved path '{}' to MFT {}", normalized_path, current_mft);
        self.path_cache.insert(normalized_path, current_mft);
        Ok(current_mft)
    }
    
    /// Get the parent directory's MFT record for a given path
    pub fn get_parent_mft(&mut self, reader: &mut NtfsReader, path: &str) -> Result<u64, MosesError> {
        let normalized_path = self.normalize_path(path);
        
        // Root has no parent
        if normalized_path == "/" {
            return Err(MosesError::Other("Root directory has no parent".to_string()));
        }
        
        // Find the last slash
        let parent_path = if let Some(pos) = normalized_path.rfind('/') {
            if pos == 0 {
                "/"
            } else {
                &normalized_path[..pos]
            }
        } else {
            "/"
        };
        
        self.resolve_path(reader, parent_path)
    }
    
    /// Extract the filename from a path
    pub fn get_filename(path: &str) -> &str {
        let normalized = path.trim_end_matches('/');
        if let Some(pos) = normalized.rfind('/') {
            &normalized[pos + 1..]
        } else {
            normalized
        }
    }
    
    /// List directory contents by MFT record number
    fn list_directory_by_mft(&mut self, reader: &mut NtfsReader, mft_num: u64) -> Result<Vec<IndexEntry>, MosesError> {
        // Read the MFT record
        let mut mft_record = reader.read_mft_record(mft_num)?;
        
        if !mft_record.is_directory() {
            return Err(MosesError::Other(format!("MFT {} is not a directory", mft_num)));
        }
        
        // Parse the INDEX_ROOT attribute
        if let Some(index_root) = mft_record.find_attribute(ATTR_TYPE_INDEX_ROOT) {
            use super::attributes::AttributeData;
            match index_root {
                AttributeData::IndexRoot(data) => {
                    super::index::parse_index_root(data)
                }
                _ => Err(MosesError::Other("Invalid INDEX_ROOT attribute".to_string()))
            }
        } else {
            // Check for INDEX_ALLOCATION for large directories
            if let Some(_index_alloc) = mft_record.find_attribute(ATTR_TYPE_INDEX_ALLOCATION) {
                // TODO: Implement INDEX_ALLOCATION parsing for large directories
                debug!("Large directory support (INDEX_ALLOCATION) not yet implemented");
                Ok(Vec::new())
            } else {
                Ok(Vec::new())
            }
        }
    }
    
    /// Normalize a path (remove duplicate slashes, trailing slash, etc.)
    fn normalize_path(&self, path: &str) -> String {
        let mut normalized = path.to_string();
        
        // Ensure it starts with /
        if !normalized.starts_with('/') {
            normalized = format!("/{}", normalized);
        }
        
        // Remove duplicate slashes
        while normalized.contains("//") {
            normalized = normalized.replace("//", "/");
        }
        
        // Remove trailing slash (except for root)
        if normalized.len() > 1 && normalized.ends_with('/') {
            normalized.pop();
        }
        
        normalized
    }
    
    /// Clear the path cache
    pub fn clear_cache(&mut self) {
        self.path_cache.clear();
        // Re-add root
        self.path_cache.insert("/".to_string(), MFT_RECORD_ROOT);
    }
    
    /// Check if a path exists
    pub fn path_exists(&mut self, reader: &mut NtfsReader, path: &str) -> bool {
        self.resolve_path(reader, path).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_normalize_path() {
        let resolver = PathResolver::new();
        
        assert_eq!(resolver.normalize_path("/"), "/");
        assert_eq!(resolver.normalize_path("//"), "/");
        assert_eq!(resolver.normalize_path("/foo/"), "/foo");
        assert_eq!(resolver.normalize_path("foo"), "/foo");
        assert_eq!(resolver.normalize_path("/foo//bar/"), "/foo/bar");
        assert_eq!(resolver.normalize_path("foo/bar"), "/foo/bar");
    }
    
    #[test]
    fn test_get_filename() {
        assert_eq!(PathResolver::get_filename("/foo/bar.txt"), "bar.txt");
        assert_eq!(PathResolver::get_filename("/foo/"), "foo");
        assert_eq!(PathResolver::get_filename("bar.txt"), "bar.txt");
        assert_eq!(PathResolver::get_filename("/"), "");
    }
}