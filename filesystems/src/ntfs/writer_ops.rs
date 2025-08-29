// Full NTFS write operations with MftRecord integration - V2
// Fixed version with correct field names and method signatures

use super::writer::NtfsWriter;
use super::mft_writer::MftRecordBuilder;
use super::attributes::AttributeData;
use super::resident_data_writer::ResidentDataWriter;
use super::data_runs::DataRun;
use super::structures::*;
use moses_core::MosesError;
use std::io::{Write, Seek, SeekFrom};
use log::{info, debug, warn};

// File attribute constants
const FILE_ATTRIBUTE_NORMAL: u32 = 0x00000080;

/// Get current time in Windows FILETIME format
fn windows_time_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let unix_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    // Convert Unix time to Windows FILETIME (100ns intervals since 1601)
    // Unix epoch (1970) is 11644473600 seconds after Windows epoch (1601)
    (unix_time + 11644473600) * 10_000_000
}

impl NtfsWriter {
    /// Write data to an existing file
    pub fn write_file_data(&mut self, mft_record_num: u64, offset: u64, data: &[u8]) -> Result<usize, MosesError> {
        info!("Writing {} bytes to MFT record {} at offset {}", data.len(), mft_record_num, offset);
        
        // Start transaction for safety
        self.begin_transaction()?;
        
        // Read the MFT record using the mft_reader
        let mut mft_record = self.mft_reader.read_record(mft_record_num)?;
        
        if !mft_record.is_in_use() {
            self.rollback_transaction()?;
            return Err(MosesError::Other("MFT record not in use".to_string()));
        }
        
        if mft_record.is_directory() {
            self.rollback_transaction()?;
            return Err(MosesError::Other("Cannot write data to directory".to_string()));
        }
        
        // Find the DATA attribute using MftRecord's method
        let data_attr = mft_record.find_attribute(ATTR_TYPE_DATA)
            .ok_or_else(|| {
                self.rollback_transaction().ok();
                MosesError::Other("No DATA attribute found".to_string())
            })?;
        
        // Handle based on attribute type
        let bytes_written = match data_attr {
            AttributeData::Data(resident_data) => {
                // Resident data - stored directly in MFT
                debug!("Writing to resident DATA attribute");
                
                // Check if the write is within bounds of existing data
                if offset as usize > resident_data.len() {
                    self.rollback_transaction()?;
                    return Err(MosesError::Other("Write offset beyond resident data size".to_string()));
                }
                
                // For now, we'll only support overwriting existing resident data
                // Full implementation would require ResidentDataWriter to modify MFT record
                if offset as usize + data.len() > resident_data.len() {
                    warn!("Cannot extend resident data - would require converting to non-resident");
                    self.rollback_transaction()?;
                    return Err(MosesError::NotSupported("Extending resident data not yet supported".to_string()));
                }
                
                // Get the raw MFT record data
                let mft_offset = self.mft_reader.mft_offset + (mft_record_num * self.boot_sector.mft_record_size() as u64);
                let mft_record_data = self.reader.read_at(mft_offset, self.boot_sector.mft_record_size() as usize)?;
                
                // Use ResidentDataWriter to update the resident data
                let resident_writer = ResidentDataWriter::new();
                let updated_mft_record = resident_writer.write_resident_data(&mft_record_data, offset, data)?;
                
                // Write the updated MFT record back
                self.write_raw_mft_record(mft_record_num, &updated_mft_record)?;
                
                data.len()
            }
            AttributeData::DataRuns(runs) => {
                // Non-resident data - stored in clusters
                let bytes_written = self.write_to_data_runs(runs, offset, data)?;
                bytes_written
            }
            _ => {
                self.rollback_transaction()?;
                return Err(MosesError::NotSupported("Unsupported DATA attribute type".to_string()));
            }
        };
        
        // Update file size if we extended it
        let _new_size = offset + bytes_written as u64;
        // TODO: Update STANDARD_INFORMATION timestamps and size
        
        // Commit transaction
        self.commit_transaction()?;
        
        Ok(bytes_written)
    }
    
    /// Helper to write data to non-resident data runs
    fn write_to_data_runs(&mut self, runs: &[DataRun], offset: u64, data: &[u8]) -> Result<usize, MosesError> {
        let bytes_per_cluster = self.bytes_per_cluster as u64;
        let cluster_offset = offset / bytes_per_cluster;
        let offset_in_cluster = offset % bytes_per_cluster;
        
        // Find which data run contains our offset
        let mut current_vcn = 0u64;
        
        for run in runs {
            let run_clusters = run.length;
            if cluster_offset >= current_vcn && cluster_offset < current_vcn + run_clusters {
                // This run contains our offset
                let lcn = run.lcn.ok_or_else(|| MosesError::Other("Sparse run not supported".to_string()))?;
                let cluster_in_run = cluster_offset - current_vcn;
                let physical_cluster = lcn + cluster_in_run;
                
                // Calculate physical offset
                let physical_offset = physical_cluster * bytes_per_cluster + offset_in_cluster;
                
                // Write data
                if self.config.enable_writes {
                    self.writer.seek(SeekFrom::Start(physical_offset))?;
                    let bytes_written = self.writer.write(data)?;
                    
                    if self.config.verify_writes {
                        // Read back and verify
                        // Read back and verify
                        let verify_buffer = self.reader.read_at(physical_offset, bytes_written)?;
                        if verify_buffer != data[..bytes_written] {
                            return Err(MosesError::Other("Write verification failed".to_string()));
                        }
                    }
                    
                    return Ok(bytes_written);
                } else {
                    debug!("Dry run: would write {} bytes at offset {:#x}", data.len(), physical_offset);
                    return Ok(data.len());
                }
            }
            current_vcn += run_clusters;
        }
        
        Err(MosesError::Other("Offset beyond file data runs".to_string()))
    }
    
    /// Create a new file in the root directory
    pub fn create_file(&mut self, name: &str, initial_size: u64) -> Result<u64, MosesError> {
        info!("Creating file '{}' with initial size {}", name, initial_size);
        
        self.begin_transaction()?;
        
        // Allocate a new MFT record
        let mft_record_num = self.find_free_mft_record()?;
        self.allocate_mft_record(mft_record_num)?;
        
        // Use MftRecordBuilder to create the record
        let mut builder = MftRecordBuilder::new(mft_record_num, self.boot_sector.mft_record_size())
            .as_file();
        
        // Add standard information
        let current_time = windows_time_now();
        builder = builder.with_standard_info(
            current_time,
            current_time, 
            current_time,
            FILE_ATTRIBUTE_NORMAL
        )?;
        
        // Add file name with full signature (9 arguments)
        builder = builder.with_file_name(
            MFT_RECORD_ROOT,  // Parent is root
            name,
            3,  // Win32 namespace
            current_time,  // created
            current_time,  // modified
            current_time,  // accessed
            initial_size,  // allocated_size
            0,  // real_size starts at 0
            FILE_ATTRIBUTE_NORMAL  // file_attributes
        )?;
        
        // Add DATA attribute
        if initial_size == 0 {
            // Empty file - use with_empty_data which exists
            builder = builder.with_empty_data()?;
        } else {
            // For non-empty files, we need to allocate clusters
            // But since with_non_resident_data doesn't exist yet,
            // we'll just create an empty file for now
            warn!("Non-resident data creation not yet fully implemented");
            builder = builder.with_empty_data()?;
        }
        
        // Build and serialize the MFT record
        let mft_buffer = builder.build()?;
        
        // Write the MFT record
        self.write_raw_mft_record(mft_record_num, &mft_buffer)?;
        
        // Add to root directory index
        // Update root directory index
        // For now, log that this needs implementation
        warn!("Directory index update not yet implemented - file won't appear in listings");
        // self.update_directory_index(MFT_RECORD_ROOT, mft_record_num, name)?;
        
        self.commit_transaction()?;
        
        Ok(mft_record_num)
    }
    
    /// Delete a file
    pub fn delete_file(&mut self, mft_record_num: u64) -> Result<(), MosesError> {
        info!("Deleting file with MFT record {}", mft_record_num);
        
        self.begin_transaction()?;
        
        // Read the MFT record
        let mut mft_record = self.mft_reader.read_record(mft_record_num)?;
        
        if !mft_record.is_in_use() {
            self.rollback_transaction()?;
            return Err(MosesError::Other("MFT record not in use".to_string()));
        }
        
        if mft_record.is_directory() {
            self.rollback_transaction()?;
            return Err(MosesError::Other("Cannot delete directory with delete_file".to_string()));
        }
        
        // Free any allocated clusters
        if let Some(data_attr) = mft_record.find_attribute(ATTR_TYPE_DATA) {
            if let AttributeData::DataRuns(runs) = data_attr {
                for run in runs {
                    if let Some(lcn) = run.lcn {
                        let length = run.length;
                        for cluster in lcn..lcn + length {
                            self.free_cluster(cluster)?;
                        }
                    }
                }
            }
        }
        
        // Mark MFT record as not in use - update the header flags
        // This requires modifying the raw MFT record
        let mft_offset = self.mft_reader.mft_offset + (mft_record_num * self.boot_sector.mft_record_size() as u64);
        
        // Read current record
        // Read current record
        let buffer = self.reader.read_at(mft_offset, self.boot_sector.mft_record_size() as usize)?;
        let mut buffer = buffer;
        
        // Clear the IN_USE flag
        if buffer.len() >= 22 {
            let flags_offset = 22; // Offset of flags in MFT record header
            let mut flags = u16::from_le_bytes([buffer[flags_offset], buffer[flags_offset + 1]]);
            flags &= !MFT_RECORD_IN_USE;
            buffer[flags_offset..flags_offset + 2].copy_from_slice(&flags.to_le_bytes());
        }
        
        // Write back
        self.write_raw_mft_record(mft_record_num, &buffer)?;
        
        // Free the MFT record
        self.free_mft_record(mft_record_num)?;
        
        // Remove from parent directory index
        // Update parent directory index  
        warn!("Directory index update not yet implemented");
        // self.remove_from_directory_index(parent_mft, mft_record_num)?;
        
        self.commit_transaction()?;
        
        Ok(())
    }
    
    /// Helper to write raw MFT record
    pub fn write_raw_mft_record(&mut self, record_num: u64, buffer: &[u8]) -> Result<(), MosesError> {
        if !self.config.enable_writes {
            debug!("Dry run: would write MFT record {}", record_num);
            return Ok(());
        }
        
        // Calculate MFT offset
        let mft_offset = self.mft_reader.mft_offset + (record_num * self.boot_sector.mft_record_size() as u64);
        
        // Write to disk
        self.writer.seek(SeekFrom::Start(mft_offset))?;
        self.writer.write_all(buffer)?;
        
        // Update cache
        self.modified_mft_records.insert(record_num);
        
        Ok(())
    }
    
    /// Free a cluster
    fn free_cluster(&mut self, cluster: u64) -> Result<(), MosesError> {
        if let Some(bitmap) = &mut self.volume_bitmap {
            bitmap[cluster as usize / 8] &= !(1 << (cluster % 8));
            self.modified_clusters.remove(&cluster);
            debug!("Freed cluster {}", cluster);
        }
        Ok(())
    }
}