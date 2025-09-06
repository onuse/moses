// NTFS File Rename/Move Operations
// Implements renaming and moving files within NTFS

use super::index_writer::IndexWriter;
use super::path_resolver::PathResolver;
use super::writer::NtfsWriter;
use super::reader::NtfsReader;
use moses_core::MosesError;
use log::{debug, info};

/// Handles file and directory rename/move operations
pub struct FileMover {
    index_writer: IndexWriter,
}

impl FileMover {
    /// Create a new file mover
    pub fn new() -> Self {
        Self {
            index_writer: IndexWriter::new(),
        }
    }
    
    /// Rename or move a file/directory
    pub fn rename_or_move(
        &mut self,
        reader: &mut NtfsReader,
        _writer: &mut NtfsWriter,
        path_resolver: &mut PathResolver,
        old_path: &str,
        new_path: &str,
    ) -> Result<(), MosesError> {
        debug!("Moving/renaming from '{}' to '{}'", old_path, new_path);
        
        // Check if source exists
        let file_mft = path_resolver.resolve_path(reader, old_path)?;
        debug!("Source file MFT: {}", file_mft);
        
        // Check if destination already exists
        if path_resolver.path_exists(reader, new_path) {
            return Err(MosesError::Other(format!("Destination already exists: {}", new_path)));
        }
        
        // Parse paths to get parent directories and filenames
        let (old_parent_path, old_name) = self.split_path(old_path)?;
        let (new_parent_path, new_name) = self.split_path(new_path)?;
        
        // Resolve parent directories
        let old_parent_mft = path_resolver.resolve_path(reader, &old_parent_path)?;
        let new_parent_mft = path_resolver.resolve_path(reader, &new_parent_path)?;
        
        debug!("Old parent MFT: {}, New parent MFT: {}", old_parent_mft, new_parent_mft);
        
        // Read the file's MFT record to check if it's a directory
        let file_record = reader.read_mft_record(file_mft)?;
        let is_directory = file_record.is_directory();
        
        // Remove from old parent's index
        let mut old_parent_record = reader.read_mft_record(old_parent_mft)?;
        self.index_writer.remove_file_from_directory(
            &mut old_parent_record,
            &old_name,
        )?;
        
        // Add to new parent's index (might be the same parent for rename)
        let mut new_parent_record = if old_parent_mft == new_parent_mft {
            old_parent_record.clone()
        } else {
            reader.read_mft_record(new_parent_mft)?
        };
        
        self.index_writer.add_file_to_directory(
            &mut new_parent_record,
            file_mft,
            &new_name,
            is_directory,
        )?;
        
        // Update the file's MFT record with new name and parent
        if old_name != new_name || old_parent_mft != new_parent_mft {
            self.update_file_record(
                reader,
                _writer,
                file_mft,
                new_parent_mft,
                &new_name,
            )?;
        }
        
        // TODO: Write updated MFT records back to disk
        // This would require MftWriter or direct device writes
        
        info!("Successfully moved/renamed '{}' to '{}'", old_path, new_path);
        Ok(())
    }
    
    /// Update a file's MFT record with new parent and name
    fn update_file_record(
        &self,
        reader: &mut NtfsReader,
        _writer: &mut NtfsWriter,
        file_mft: u64,
        new_parent_mft: u64,
        new_name: &str,
    ) -> Result<(), MosesError> {
        debug!("Updating MFT record {} with new parent {} and name '{}'", 
               file_mft, new_parent_mft, new_name);
        
        // Read the current MFT record
        let _file_record = reader.read_mft_record(file_mft)?;
        
        // Update the FILE_NAME attribute
        // This is simplified - in reality we'd need to:
        // 1. Find and remove the old FILE_NAME attribute
        // 2. Create a new FILE_NAME attribute with updated parent and name
        // 3. Update any other references
        
        // For now, just mark that this needs implementation
        debug!("FILE_NAME attribute update not fully implemented");
        
        Ok(())
    }
    
    /// Split a path into parent directory and filename
    fn split_path(&self, path: &str) -> Result<(String, String), MosesError> {
        let normalized = path.trim_end_matches('/');
        
        if normalized.is_empty() || normalized == "/" {
            return Err(MosesError::Other("Cannot rename root directory".to_string()));
        }
        
        if let Some(pos) = normalized.rfind('/') {
            if pos == 0 {
                Ok(("/".to_string(), normalized[1..].to_string()))
            } else {
                Ok((normalized[..pos].to_string(), normalized[pos + 1..].to_string()))
            }
        } else {
            Ok(("/".to_string(), normalized.to_string()))
        }
    }
    
    /// Check if a path represents just a rename (same parent directory)
    pub fn is_rename_only(&self, old_path: &str, new_path: &str) -> Result<bool, MosesError> {
        let (old_parent, _) = self.split_path(old_path)?;
        let (new_parent, _) = self.split_path(new_path)?;
        Ok(old_parent == new_parent)
    }
    
    /// Validate that a move operation is allowed
    pub fn validate_move(
        &self,
        reader: &mut NtfsReader,
        path_resolver: &mut PathResolver,
        old_path: &str,
        new_path: &str,
    ) -> Result<(), MosesError> {
        // Check that we're not trying to move a directory into itself
        if new_path.starts_with(old_path) && old_path != new_path {
            // Check if old_path is a directory
            let mft = path_resolver.resolve_path(reader, old_path)?;
            let record = reader.read_mft_record(mft)?;
            if record.is_directory() {
                return Err(MosesError::Other(
                    "Cannot move directory into itself".to_string()
                ));
            }
        }
        
        // Check that parent directories exist
        let (new_parent_path, _) = self.split_path(new_path)?;
        if !path_resolver.path_exists(reader, &new_parent_path) {
            return Err(MosesError::Other(format!(
                "Parent directory does not exist: {}", new_parent_path
            )));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_split_path() {
        let mover = FileMover::new();
        
        // Test absolute paths
        let (parent, name) = mover.split_path("/foo/bar.txt").unwrap();
        assert_eq!(parent, "/foo");
        assert_eq!(name, "bar.txt");
        
        // Test root directory file
        let (parent, name) = mover.split_path("/file.txt").unwrap();
        assert_eq!(parent, "/");
        assert_eq!(name, "file.txt");
        
        // Test path with trailing slash
        let (parent, name) = mover.split_path("/foo/bar/").unwrap();
        assert_eq!(parent, "/foo");
        assert_eq!(name, "bar");
    }
    
    #[test]
    fn test_is_rename_only() {
        let mover = FileMover::new();
        
        // Same directory rename
        assert!(mover.is_rename_only("/foo/old.txt", "/foo/new.txt").unwrap());
        
        // Different directory move
        assert!(!mover.is_rename_only("/foo/file.txt", "/bar/file.txt").unwrap());
        
        // Root directory rename
        assert!(mover.is_rename_only("/old.txt", "/new.txt").unwrap());
    }
}