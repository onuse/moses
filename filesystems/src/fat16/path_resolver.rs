// FAT16 Path Resolver
// Handles path resolution and directory traversal for FAT16 filesystems

use moses_core::MosesError;
use crate::fat16::reader::Fat16Reader;
use crate::fat16::lfn_support::LfnParser;
use crate::fat16::writer::Fat16Writer;
use crate::fat_common::{FatDirEntry, FatAttributes};
use crate::device_reader::FileEntry;
use log::{debug, trace};

// FAT16 specific constants
const ATTR_LONG_NAME: u8 = FatAttributes::READ_ONLY | FatAttributes::HIDDEN | 
                           FatAttributes::SYSTEM | FatAttributes::VOLUME_ID;

/// Result of path resolution
#[derive(Debug, Clone)]
pub struct ResolvedPath {
    pub name: String,
    pub short_name: String,
    pub is_directory: bool,
    pub cluster: u16,  // FAT16 uses 16-bit clusters
    pub size: u32,
    pub parent_cluster: Option<u16>,  // None for root directory
}

/// Directory entry information
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub name: String,
    pub short_name: String,
    pub is_directory: bool,
    pub cluster: u16,
    pub size: u32,
    pub attributes: u8,
}

pub struct Fat16PathResolver<'a> {
    reader: &'a mut Fat16Reader,
}

impl<'a> Fat16PathResolver<'a> {
    pub fn new(reader: &'a mut Fat16Reader) -> Self {
        Self { reader }
    }
    
    /// Resolve a path to its directory entry
    pub fn resolve_path(&mut self, path: &str) -> Result<ResolvedPath, MosesError> {
        debug!("Resolving FAT16 path: {}", path);
        
        // Handle root directory
        if path == "/" || path.is_empty() {
            return Ok(ResolvedPath {
                name: "/".to_string(),
                short_name: "/".to_string(),
                is_directory: true,
                cluster: 0,  // FAT16 root directory doesn't have a cluster
                size: 0,
                parent_cluster: None,
            });
        }
        
        // Parse path components
        let path = path.trim_end_matches('/');
        let components: Vec<&str> = path.split('/')
            .filter(|s| !s.is_empty())
            .collect();
        
        if components.is_empty() {
            return self.resolve_path("/");
        }
        
        // Start from root directory
        let mut current_cluster: Option<u16> = None;  // None means root directory
        let mut parent_cluster: Option<u16> = None;
        
        // Traverse path components
        for (i, component) in components.iter().enumerate() {
            trace!("Resolving component: {} in cluster {:?}", component, current_cluster);
            
            // Read directory entries
            let entries = if current_cluster.is_none() {
                // Read root directory
                self.read_root_directory_entries()?
            } else {
                // Read subdirectory
                self.read_directory_entries(current_cluster.unwrap())?
            };
            
            // Find matching entry
            let entry = entries.iter()
                .find(|e| e.name.eq_ignore_ascii_case(component))
                .ok_or_else(|| MosesError::Other(format!("Path component '{}' not found", component)))?;
            
            // Check if we need to continue traversing
            if i < components.len() - 1 {
                // Not the last component - must be a directory
                if !entry.is_directory {
                    return Err(MosesError::Other(format!("'{}' is not a directory", component)));
                }
                parent_cluster = current_cluster;
                current_cluster = Some(entry.cluster);
            } else {
                // Last component - this is our target
                return Ok(ResolvedPath {
                    name: entry.name.clone(),
                    short_name: entry.short_name.clone(),
                    is_directory: entry.is_directory,
                    cluster: entry.cluster,
                    size: entry.size,
                    parent_cluster: current_cluster,
                });
            }
        }
        
        Err(MosesError::Other("Path resolution failed".to_string()))
    }
    
    /// Read entries from the root directory
    pub fn read_root_directory_entries(&mut self) -> Result<Vec<DirectoryEntry>, MosesError> {
        debug!("Reading FAT16 root directory entries");
        
        // FAT16 has a fixed-size root directory
        let root_entries = self.reader.read_root_directory()?;
        let mut entries = Vec::new();
        let mut lfn_parser = LfnParser::new();
        
        for entry in root_entries {
            // Skip empty and deleted entries
            if entry.name[0] == 0x00 {
                break;  // End of directory
            }
            if entry.name[0] == 0xE5 {
                lfn_parser.reset();  // Reset LFN on deleted entry
                continue;  // Deleted entry
            }
            
            // Skip volume labels (but not LFN entries)
            if entry.attributes & FatAttributes::VOLUME_ID != 0 && entry.attributes != ATTR_LONG_NAME {
                continue;
            }
            
            // Check if this is an LFN entry
            let entry_bytes = unsafe {
                std::slice::from_raw_parts(&entry as *const FatDirEntry as *const u8, std::mem::size_of::<FatDirEntry>())
            };
            
            if lfn_parser.process_entry(entry_bytes) {
                continue;  // This was an LFN entry, processed
            }
            
            // Parse the short name
            let short_name = Self::parse_short_name(&entry.name);
            
            // Get long name if available, otherwise use short name
            let long_name = lfn_parser.get_long_name();
            let name = long_name.unwrap_or_else(|| short_name.clone());
            
            entries.push(DirectoryEntry {
                name,
                short_name,
                is_directory: entry.attributes & FatAttributes::DIRECTORY != 0,
                cluster: entry.first_cluster_low,
                size: entry.file_size,
                attributes: entry.attributes,
            });
        }
        
        Ok(entries)
    }
    
    /// Read entries from a subdirectory cluster
    pub fn read_directory_entries(&mut self, cluster: u16) -> Result<Vec<DirectoryEntry>, MosesError> {
        debug!("Reading directory entries from cluster {}", cluster);
        
        if cluster == 0 {
            return self.read_root_directory_entries();
        }
        
        let mut entries = Vec::new();
        let mut lfn_parser = LfnParser::new();
        let mut current_cluster = cluster;
        
        // Follow the cluster chain
        loop {
            // Read cluster data
            let cluster_data = self.reader.read_cluster(current_cluster)?;
            let entry_size = std::mem::size_of::<FatDirEntry>();
            let entries_per_cluster = cluster_data.len() / entry_size;
            
            // Parse directory entries
            for i in 0..entries_per_cluster {
                let offset = i * entry_size;
                if offset + entry_size > cluster_data.len() {
                    break;
                }
                
                let entry_bytes = &cluster_data[offset..offset + entry_size];
                let entry = unsafe {
                    std::ptr::read(entry_bytes.as_ptr() as *const FatDirEntry)
                };
                
                // Check for end of directory
                if entry.name[0] == 0x00 {
                    return Ok(entries);
                }
                
                // Skip deleted entries
                if entry.name[0] == 0xE5 {
                    lfn_parser.reset();  // Reset LFN on deleted entry
                    continue;
                }
                
                // Skip volume labels (but not LFN entries)
                if entry.attributes & FatAttributes::VOLUME_ID != 0 && entry.attributes != ATTR_LONG_NAME {
                    continue;
                }
                
                // Check if this is an LFN entry
                if lfn_parser.process_entry(entry_bytes) {
                    continue;  // This was an LFN entry, processed
                }
                
                // Skip . and .. entries
                if entry.name[0] == b'.' {
                    if entry.name[1] == b' ' || (entry.name[1] == b'.' && entry.name[2] == b' ') {
                        continue;
                    }
                }
                
                let short_name = Self::parse_short_name(&entry.name);
                
                // Get long name if available, otherwise use short name
                let long_name = lfn_parser.get_long_name();
                let name = long_name.unwrap_or_else(|| short_name.clone());
                
                entries.push(DirectoryEntry {
                    name,
                    short_name,
                    is_directory: entry.attributes & FatAttributes::DIRECTORY != 0,
                    cluster: entry.first_cluster_low,
                    size: entry.file_size,
                    attributes: entry.attributes,
                });
            }
            
            // Get next cluster in chain
            let next_cluster = self.reader.get_next_cluster(current_cluster)?;
            if next_cluster >= 0xFFF8 {
                break;  // End of chain
            }
            current_cluster = next_cluster;
        }
        
        Ok(entries)
    }
    
    /// Parse a short (8.3) filename
    fn parse_short_name(name_bytes: &[u8; 11]) -> String {
        // Extract base name (first 8 bytes)
        let base = &name_bytes[0..8];
        let base_str = std::str::from_utf8(base)
            .unwrap_or("")
            .trim_end();
        
        // Extract extension (last 3 bytes)
        let ext = &name_bytes[8..11];
        let ext_str = std::str::from_utf8(ext)
            .unwrap_or("")
            .trim_end();
        
        if ext_str.is_empty() {
            base_str.to_string()
        } else {
            format!("{}.{}", base_str, ext_str)
        }
    }
    
    /// Check if a path exists
    pub fn exists(&mut self, path: &str) -> bool {
        self.resolve_path(path).is_ok()
    }
    
    /// List directory contents
    pub fn list_directory(&mut self, path: &str) -> Result<Vec<FileEntry>, MosesError> {
        let resolved = self.resolve_path(path)?;
        
        if !resolved.is_directory {
            return Err(MosesError::Other(format!("'{}' is not a directory", path)));
        }
        
        let entries = if resolved.cluster == 0 {
            // Root directory
            self.read_root_directory_entries()?
        } else {
            // Subdirectory
            self.read_directory_entries(resolved.cluster)?
        };
        
        // Convert to FileEntry format
        Ok(entries.into_iter().map(|e| FileEntry {
            name: e.name,
            is_directory: e.is_directory,
            size: e.size as u64,
            cluster: Some(e.cluster as u32),
            metadata: Default::default(),
        }).collect())
    }
}

/// Path resolver for use with FAT16Writer
pub struct Fat16PathResolverMut<'a> {
    writer: &'a mut Fat16Writer,
}

impl<'a> Fat16PathResolverMut<'a> {
    pub fn new(writer: &'a mut Fat16Writer) -> Self {
        Self { writer }
    }
    
    /// Find a free entry in a directory
    pub fn find_free_directory_entry(&mut self, parent_cluster: Option<u16>) -> Result<(usize, Option<u16>), MosesError> {
        if parent_cluster.is_none() {
            // Root directory - use writer's method
            let index = self.writer.find_free_root_entry()?;
            Ok((index, None))
        } else {
            // Subdirectory - need to search clusters
            // This would require implementing directory cluster searching
            // For now, return an error
            Err(MosesError::NotSupported("Subdirectory operations not yet implemented".to_string()))
        }
    }
    
    /// Create a directory entry
    pub fn create_directory_entry(
        &mut self,
        parent_cluster: Option<u16>,
        name: &str,
        is_directory: bool,
        cluster: u16,
        size: u32,
    ) -> Result<(), MosesError> {
        // Create the directory entry
        let entry = Fat16Writer::create_directory_entry(
            name,
            if is_directory { FatAttributes::DIRECTORY } else { FatAttributes::ARCHIVE },
            cluster,
            size,
        );
        
        if parent_cluster.is_none() {
            // Write to root directory
            let index = self.writer.find_free_root_entry()?;
            self.writer.write_root_dir_entry(index, &entry)?;
        } else {
            // Write to subdirectory
            return Err(MosesError::NotSupported("Subdirectory operations not yet implemented".to_string()));
        }
        
        Ok(())
    }
}