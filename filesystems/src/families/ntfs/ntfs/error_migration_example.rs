// NTFS Error Migration Example - Using Enhanced MosesError
// Shows practical migration patterns for NTFS operations

use moses_core::error::{MosesError, CorruptionLevel, MosesResult, ErrorContext};
use std::path::PathBuf;

/// Example NTFS MFT operations with enhanced error handling
pub struct NtfsMftOperations;

impl NtfsMftOperations {
    /// Read MFT record with enhanced error handling
    pub fn read_mft_record(
        &mut self,
        device: &mut std::fs::File,
        record_number: u64,
        mft_start: u64,
        record_size: u32,
    ) -> MosesResult<Vec<u8>> {
        let offset = mft_start + (record_number * record_size as u64);
        let mut buffer = vec![0u8; record_size as usize];
        
        // Seek to MFT record
        use std::io::{Seek, SeekFrom, Read};
        device.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::io(e, offset))
            .context("Seeking to MFT record")?;
        
        // Read MFT record
        device.read_exact(&mut buffer)
            .map_err(|e| MosesError::io(e, offset))
            .fs_context("NTFS", &format!("reading MFT record {}", record_number))?;
        
        // Validate MFT record signature
        if &buffer[0..4] != b"FILE" {
            return Err(MosesError::corruption(
                format!("Invalid MFT record signature at record {}", record_number),
                CorruptionLevel::Severe,
            ).at_offset(offset));
        }
        
        // Check fixup array
        let update_sequence_offset = u16::from_le_bytes([buffer[4], buffer[5]]) as usize;
        let update_sequence_count = u16::from_le_bytes([buffer[6], buffer[7]]) as usize;
        
        if update_sequence_offset >= record_size as usize || 
           update_sequence_count == 0 || 
           update_sequence_count > 128 {
            return Err(MosesError::corruption(
                format!("Invalid fixup array in MFT record {}", record_number),
                CorruptionLevel::Moderate,
            ).at_offset(offset + 4));
        }
        
        Ok(buffer)
    }
    
    /// Parse attribute from MFT record
    pub fn parse_attribute(
        &self,
        record_data: &[u8],
        attribute_type: u32,
    ) -> MosesResult<Option<Vec<u8>>> {
        let mut offset = u16::from_le_bytes([record_data[20], record_data[21]]) as usize;
        
        while offset < record_data.len() - 8 {
            // Read attribute header
            let attr_type = u32::from_le_bytes([
                record_data[offset],
                record_data[offset + 1],
                record_data[offset + 2],
                record_data[offset + 3],
            ]);
            
            // End marker
            if attr_type == 0xFFFFFFFF {
                break;
            }
            
            // Invalid attribute type
            if attr_type == 0 {
                return Err(MosesError::corruption(
                    "Invalid attribute type 0 in MFT record",
                    CorruptionLevel::Minor,
                ).at_offset(offset as u64));
            }
            
            let attr_length = u32::from_le_bytes([
                record_data[offset + 4],
                record_data[offset + 5],
                record_data[offset + 6],
                record_data[offset + 7],
            ]);
            
            // Validate attribute length
            if attr_length < 24 || attr_length > 0x10000 {
                return Err(MosesError::ValidationFailed {
                    field: "attribute_length".into(),
                    expected: "24..65536".into(),
                    actual: attr_length.to_string(),
                });
            }
            
            if attr_type == attribute_type {
                // Found the attribute we're looking for
                let attr_end = offset + attr_length as usize;
                if attr_end > record_data.len() {
                    return Err(MosesError::corruption(
                        "Attribute extends beyond MFT record",
                        CorruptionLevel::Moderate,
                    ));
                }
                return Ok(Some(record_data[offset..attr_end].to_vec()));
            }
            
            offset += attr_length as usize;
            
            // Align to 8 bytes
            offset = (offset + 7) & !7;
        }
        
        Ok(None)
    }
    
    /// Update MFT record with safety checks
    pub fn update_mft_record(
        &mut self,
        device: &mut std::fs::File,
        record_number: u64,
        mft_start: u64,
        record_size: u32,
        new_data: &[u8],
    ) -> MosesResult<()> {
        if new_data.len() != record_size as usize {
            return Err(MosesError::InvalidArgument {
                message: format!(
                    "MFT record data size mismatch: expected {}, got {}",
                    record_size,
                    new_data.len()
                ),
            });
        }
        
        // Validate new data has correct signature
        if &new_data[0..4] != b"FILE" {
            return Err(MosesError::SafetyViolation {
                message: "Refusing to write invalid MFT record (missing FILE signature)".into(),
            });
        }
        
        let offset = mft_start + (record_number * record_size as u64);
        
        // Backup existing record first (in production, would write to journal)
        let existing = self.read_mft_record(device, record_number, mft_start, record_size)?;
        log::debug!("Backing up MFT record {} before update", record_number);
        
        // Write new record
        use std::io::{Seek, SeekFrom, Write};
        device.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::io(e, offset))?;
        
        device.write_all(new_data)
            .map_err(|e| MosesError::io(e, offset))
            .fs_context("NTFS", &format!("updating MFT record {}", record_number))?;
        
        // Flush to ensure write completes
        device.flush()
            .map_err(|e| MosesError::io_simple(e))?;
        
        Ok(())
    }
    
    /// Check if path exists in NTFS
    pub fn path_exists(&mut self, path: &str) -> MosesResult<bool> {
        // This would normally involve proper path resolution
        if path.contains('\0') || path.contains(':') && !path.starts_with("C:") {
            return Err(MosesError::InvalidPath {
                path: path.into(),
                reason: "Invalid characters in path".into(),
            });
        }
        
        // Simplified example
        Ok(false)
    }
    
    /// Allocate clusters with proper error handling
    pub fn allocate_clusters(
        &mut self,
        count: u32,
        bytes_per_cluster: u32,
    ) -> MosesResult<Vec<u64>> {
        // Check if we have enough space
        let required_bytes = count as u64 * bytes_per_cluster as u64;
        let available_bytes = self.get_free_space()?;
        
        if required_bytes > available_bytes {
            return Err(MosesError::InsufficientSpace {
                required: required_bytes,
                available: available_bytes,
            });
        }
        
        // Allocate clusters (simplified)
        let clusters = vec![0u64; count as usize]; // Would actually allocate from bitmap
        
        Ok(clusters)
    }
    
    fn get_free_space(&self) -> MosesResult<u64> {
        // Simplified - would read from volume information
        Ok(1024 * 1024 * 1024) // 1 GB
    }
}

/// Example index operations with enhanced errors
pub struct NtfsIndexOperations;

impl NtfsIndexOperations {
    /// Find entry in B-tree index
    pub fn find_index_entry(
        &self,
        index_root: &[u8],
        key: &str,
    ) -> MosesResult<Option<u64>> {
        // Validate index root structure
        if index_root.len() < 16 {
            return Err(MosesError::corruption(
                "Index root too small",
                CorruptionLevel::Moderate,
            ));
        }
        
        let signature = u32::from_le_bytes([
            index_root[0], index_root[1], index_root[2], index_root[3]
        ]);
        
        if signature != 0x58444E49 { // "INDX"
            return Err(MosesError::ValidationFailed {
                field: "index_signature".into(),
                expected: "INDX".into(),
                actual: format!("{:08X}", signature),
            });
        }
        
        // Would implement actual B-tree search here
        Ok(None)
    }
    
    /// Insert entry into index
    pub fn insert_index_entry(
        &mut self,
        key: &str,
        mft_reference: u64,
    ) -> MosesResult<()> {
        // Validate key
        if key.is_empty() {
            return Err(MosesError::InvalidArgument {
                message: "Cannot insert empty key into index".into(),
            });
        }
        
        if key.len() > 255 {
            return Err(MosesError::InvalidPath {
                path: key.into(),
                reason: "Filename too long (max 255 characters)".into(),
            });
        }
        
        // Check for invalid characters
        for ch in key.chars() {
            if ch == '\0' || ch == '/' || ch == '\\' && key != "\\" {
                return Err(MosesError::InvalidPath {
                    path: key.into(),
                    reason: format!("Invalid character '{}' in filename", ch),
                });
            }
        }
        
        // Would implement actual B-tree insertion here
        Ok(())
    }
}

/// Example showing migration patterns:
/// 
/// OLD CODE:
/// ```
/// fn read_mft(&mut self, num: u64) -> Result<Vec<u8>, MosesError> {
///     // ... read logic ...
///     Err(MosesError::Other("Read failed".to_string()))
/// }
/// ```
/// 
/// MIGRATED CODE (shown above):
/// - Uses specific error variants (IoError with offset, Corruption with severity)
/// - Adds context with .context() and .fs_context()
/// - Validates data and returns ValidationFailed errors
/// - Uses SafetyViolation for dangerous operations
/// - Provides rich error messages with relevant details

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_types() {
        // I/O error with offset
        let err = MosesError::io(
            std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "EOF"),
            0x1000,
        );
        assert!(matches!(err, MosesError::IoError { offset: Some(0x1000), .. }));
        
        // Corruption with severity
        let err = MosesError::corruption("Bad data", CorruptionLevel::Severe);
        assert!(matches!(err, MosesError::Corruption { severity: CorruptionLevel::Severe, .. }));
        
        // Path not found
        let err = MosesError::PathNotFound { path: PathBuf::from("/test") };
        assert!(err.to_string().contains("/test"));
        
        // Validation failed
        let err = MosesError::ValidationFailed {
            field: "cluster_size".into(),
            expected: "4096".into(),
            actual: "513".into(),
        };
        assert!(err.to_string().contains("cluster_size"));
    }
}