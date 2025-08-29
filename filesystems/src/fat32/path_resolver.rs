// FAT32 Path Resolution Module
// Handles navigation through directories, LFN support, and path parsing

use moses_core::MosesError;
type MosesResult<T> = Result<T, MosesError>;
use crate::fat32::reader::{Fat32Reader, Fat32DirEntry, LongNameEntry};
use crate::device_reader::FileEntry;
use std::path::PathBuf;

// Directory entry attributes
const ATTR_READ_ONLY: u8 = 0x01;
const ATTR_HIDDEN: u8 = 0x02;
const ATTR_SYSTEM: u8 = 0x04;
const ATTR_VOLUME_ID: u8 = 0x08;
const ATTR_DIRECTORY: u8 = 0x10;
const ATTR_ARCHIVE: u8 = 0x20;
const ATTR_LONG_NAME: u8 = ATTR_READ_ONLY | ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME_ID;

/// Represents a resolved path in FAT32
#[derive(Debug, Clone)]
pub struct ResolvedPath {
    pub cluster: u32,
    pub is_directory: bool,
    pub size: u32,
    pub attributes: u8,
    pub full_path: PathBuf,
    pub name: String,
}

/// FAT32 path resolver with enhanced capabilities
pub struct Fat32PathResolver<'a> {
    reader: &'a mut Fat32Reader,
}

impl<'a> Fat32PathResolver<'a> {
    pub fn new(reader: &'a mut Fat32Reader) -> Self {
        Self { reader }
    }
    
    /// Resolve a path to its cluster and metadata
    pub fn resolve_path(&mut self, path: &str) -> MosesResult<ResolvedPath> {
        // Normalize path
        let path = Self::normalize_path(path);
        
        // Handle root directory
        if path.is_empty() || path == "/" {
            return Ok(ResolvedPath {
                cluster: self.reader.get_root_cluster(),
                is_directory: true,
                size: 0,
                attributes: ATTR_DIRECTORY,
                full_path: PathBuf::from("/"),
                name: String::new(),
            });
        }
        
        // Split path into components
        let components: Vec<&str> = path
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        
        if components.is_empty() {
            return Ok(ResolvedPath {
                cluster: self.reader.get_root_cluster(),
                is_directory: true,
                size: 0,
                attributes: ATTR_DIRECTORY,
                full_path: PathBuf::from("/"),
                name: String::new(),
            });
        }
        
        // Navigate through path components
        let mut current_cluster = self.reader.get_root_cluster();
        let mut current_path = PathBuf::from("/");
        let mut is_directory = true;
        
        for (idx, component) in components.iter().enumerate() {
            if !is_directory {
                return Err(MosesError::Other(format!("Not a directory: {:?}", current_path)));
            }
            
            // Read directory entries from current cluster
            let entries = self.read_directory_entries(current_cluster)?;
            
            // Find matching entry
            let entry = self.find_entry(&entries, component)
                .ok_or_else(|| MosesError::Other(format!("Path not found: {:?}", current_path.join(component))))?;
            
            // Update current state
            current_cluster = entry.cluster;
            is_directory = entry.is_directory;
            current_path.push(&entry.name);
            
            // If this is not the last component and it's not a directory, error
            if idx < components.len() - 1 && !is_directory {
                return Err(MosesError::Other(format!("Not a directory: {:?}", current_path.clone())));
            }
        }
        
        // Get the final entry details
        let parent_cluster = if components.len() > 1 {
            // Need to re-resolve parent
            let parent_path = path.rsplit_once('/').map(|p| p.0).unwrap_or("/");
            self.resolve_path(parent_path)?.cluster
        } else {
            self.reader.get_root_cluster()
        };
        
        let entries = self.read_directory_entries(parent_cluster)?;
        let final_entry = self.find_entry(&entries, components.last().unwrap())
            .ok_or_else(|| MosesError::Other(format!("Path not found: {:?}", PathBuf::from(&path))))?;
        
        Ok(ResolvedPath {
            cluster: final_entry.cluster,
            is_directory: final_entry.is_directory,
            size: final_entry.size,
            attributes: final_entry.attributes,
            full_path: PathBuf::from(&path),
            name: final_entry.name.clone(),
        })
    }
    
    /// Read all entries from a directory cluster chain
    pub fn read_directory_entries(&mut self, start_cluster: u32) -> MosesResult<Vec<DirectoryEntry>> {
        let clusters = self.reader.get_cluster_chain(start_cluster)?;
        let mut all_entries = Vec::new();
        
        for cluster in clusters {
            let data = self.reader.read_cluster(cluster)?;
            let entries = self.parse_directory_data(&data)?;
            all_entries.extend(entries);
        }
        
        Ok(all_entries)
    }
    
    /// Parse raw directory data into entries
    fn parse_directory_data(&self, data: &[u8]) -> MosesResult<Vec<DirectoryEntry>> {
        let mut entries = Vec::new();
        let mut long_name_parts: Vec<LongNameEntry> = Vec::new();
        
        for chunk in data.chunks_exact(32) {
            // Check for end of directory
            if chunk[0] == 0x00 {
                break;
            }
            
            // Skip deleted entries
            if chunk[0] == 0xE5 {
                long_name_parts.clear();
                continue;
            }
            
            let dir_entry = unsafe {
                std::ptr::read_unaligned(chunk.as_ptr() as *const Fat32DirEntry)
            };
            
            // Check if this is a long name entry
            if dir_entry.attributes == ATTR_LONG_NAME {
                let lfn = unsafe {
                    std::ptr::read_unaligned(chunk.as_ptr() as *const LongNameEntry)
                };
                long_name_parts.push(lfn);
                continue;
            }
            
            // Skip volume label entries
            if dir_entry.attributes & ATTR_VOLUME_ID != 0 {
                long_name_parts.clear();
                continue;
            }
            
            // Skip . and .. entries for now (handle separately if needed)
            let short_name = Self::parse_short_name(&dir_entry);
            if short_name == "." || short_name == ".." {
                long_name_parts.clear();
                continue;
            }
            
            // Build the entry
            let name = if !long_name_parts.is_empty() {
                Self::parse_long_name(&long_name_parts)?
            } else {
                short_name
            };
            
            let cluster = ((dir_entry.first_cluster_hi as u32) << 16) | 
                         (dir_entry.first_cluster_lo as u32);
            
            entries.push(DirectoryEntry {
                name,
                short_name: Self::parse_short_name(&dir_entry),
                cluster,
                size: dir_entry.file_size,
                attributes: dir_entry.attributes,
                is_directory: (dir_entry.attributes & ATTR_DIRECTORY) != 0,
                creation_time: Self::parse_datetime(dir_entry.creation_date, dir_entry.creation_time),
                modified_time: Self::parse_datetime(dir_entry.write_date, dir_entry.write_time),
                accessed_date: Self::parse_date(dir_entry.last_access_date),
            });
            
            long_name_parts.clear();
        }
        
        Ok(entries)
    }
    
    /// Parse short (8.3) name from directory entry
    fn parse_short_name(entry: &Fat32DirEntry) -> String {
        let name_part = String::from_utf8_lossy(&entry.name[0..8])
            .trim_end()
            .to_string();
        let ext_part = String::from_utf8_lossy(&entry.name[8..11])
            .trim_end()
            .to_string();
        
        if ext_part.is_empty() {
            name_part
        } else {
            format!("{}.{}", name_part, ext_part)
        }
    }
    
    /// Parse long filename from LFN entries
    fn parse_long_name(lfn_entries: &[LongNameEntry]) -> MosesResult<String> {
        let mut full_name = String::new();
        
        // LFN entries are stored in reverse order
        for lfn in lfn_entries.iter().rev() {
            // Check if this is the last entry (bit 6 set in order field)
            let is_last = (lfn.order & 0x40) != 0;
            let _sequence = lfn.order & 0x3F;
            
            // Extract characters from each part (copy arrays to avoid packed field issues)
            let name1 = lfn.name1;
            for &ch in &name1 {
                if ch == 0 || ch == 0xFFFF { 
                    return Ok(full_name);
                }
                if let Some(c) = char::from_u32(ch as u32) {
                    full_name.push(c);
                }
            }
            
            let name2 = lfn.name2;
            for &ch in &name2 {
                if ch == 0 || ch == 0xFFFF { 
                    return Ok(full_name);
                }
                if let Some(c) = char::from_u32(ch as u32) {
                    full_name.push(c);
                }
            }
            
            let name3 = lfn.name3;
            for &ch in &name3 {
                if ch == 0 || ch == 0xFFFF { 
                    return Ok(full_name);
                }
                if let Some(c) = char::from_u32(ch as u32) {
                    full_name.push(c);
                }
            }
        }
        
        Ok(full_name)
    }
    
    /// Find entry by name (case-insensitive)
    fn find_entry(&self, entries: &[DirectoryEntry], name: &str) -> Option<DirectoryEntry> {
        entries.iter()
            .find(|e| e.name.eq_ignore_ascii_case(name) || 
                     e.short_name.eq_ignore_ascii_case(name))
            .cloned()
    }
    
    /// Normalize path (remove redundant separators, etc.)
    fn normalize_path(path: &str) -> String {
        // Convert backslashes to forward slashes
        let path = path.replace('\\', "/");
        
        // Remove duplicate slashes
        let mut normalized = String::new();
        let mut prev_slash = false;
        
        for ch in path.chars() {
            if ch == '/' {
                if !prev_slash {
                    normalized.push(ch);
                }
                prev_slash = true;
            } else {
                normalized.push(ch);
                prev_slash = false;
            }
        }
        
        // Remove trailing slash unless it's root
        if normalized.len() > 1 && normalized.ends_with('/') {
            normalized.pop();
        }
        
        normalized
    }
    
    /// Parse FAT date/time format
    fn parse_datetime(date: u16, time: u16) -> Option<chrono::NaiveDateTime> {
        if date == 0 || time == 0 {
            return None;
        }
        
        let year = 1980 + ((date >> 9) & 0x7F) as i32;
        let month = ((date >> 5) & 0x0F) as u32;
        let day = (date & 0x1F) as u32;
        
        let hour = ((time >> 11) & 0x1F) as u32;
        let minute = ((time >> 5) & 0x3F) as u32;
        let second = ((time & 0x1F) * 2) as u32;
        
        chrono::NaiveDate::from_ymd_opt(year, month, day)
            .and_then(|d| d.and_hms_opt(hour, minute, second))
    }
    
    /// Parse FAT date format
    fn parse_date(date: u16) -> Option<chrono::NaiveDate> {
        if date == 0 {
            return None;
        }
        
        let year = 1980 + ((date >> 9) & 0x7F) as i32;
        let month = ((date >> 5) & 0x0F) as u32;
        let day = (date & 0x1F) as u32;
        
        chrono::NaiveDate::from_ymd_opt(year, month, day)
    }
}

/// Directory entry with full metadata
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub name: String,
    pub short_name: String,
    pub cluster: u32,
    pub size: u32,
    pub attributes: u8,
    pub is_directory: bool,
    pub creation_time: Option<chrono::NaiveDateTime>,
    pub modified_time: Option<chrono::NaiveDateTime>,
    pub accessed_date: Option<chrono::NaiveDate>,
}

impl DirectoryEntry {
    pub fn is_hidden(&self) -> bool {
        (self.attributes & ATTR_HIDDEN) != 0
    }
    
    pub fn is_system(&self) -> bool {
        (self.attributes & ATTR_SYSTEM) != 0
    }
    
    pub fn is_read_only(&self) -> bool {
        (self.attributes & ATTR_READ_ONLY) != 0
    }
    
    pub fn is_archive(&self) -> bool {
        (self.attributes & ATTR_ARCHIVE) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_path_normalization() {
        assert_eq!(Fat32PathResolver::<'_>::normalize_path("/"), "/");
        assert_eq!(Fat32PathResolver::<'_>::normalize_path("//foo//bar//"), "/foo/bar");
        assert_eq!(Fat32PathResolver::<'_>::normalize_path("\\foo\\bar\\"), "/foo/bar");
        assert_eq!(Fat32PathResolver::<'_>::normalize_path("foo/bar"), "foo/bar");
    }
    
    #[test]
    fn test_short_name_parsing() {
        let mut entry = Fat32DirEntry {
            name: [b'T', b'E', b'S', b'T', b' ', b' ', b' ', b' ', b'T', b'X', b'T'],
            attributes: 0,
            nt_reserved: 0,
            creation_time_tenth: 0,
            creation_time: 0,
            creation_date: 0,
            last_access_date: 0,
            first_cluster_hi: 0,
            write_time: 0,
            write_date: 0,
            first_cluster_lo: 0,
            file_size: 0,
        };
        
        assert_eq!(Fat32PathResolver::<'_>::parse_short_name(&entry), "TEST.TXT");
        
        entry.name = [b'F', b'I', b'L', b'E', b'N', b'A', b'M', b'E', b' ', b' ', b' '];
        assert_eq!(Fat32PathResolver::<'_>::parse_short_name(&entry), "FILENAME");
    }
}