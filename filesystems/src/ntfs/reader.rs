// NTFS Reader - Phase 1 implementation
// Provides basic read-only support for NTFS volumes

use moses_core::{Device, MosesError};
use crate::device_reader::{FilesystemReader, FileEntry, FileMetadata, FilesystemInfo, AlignedDeviceReader};
use crate::ntfs::boot_sector::NtfsBootSectorReader;
use crate::ntfs::mft::{MftReader, MftRecord};
use crate::ntfs::structures::*;
use crate::ntfs::attributes::AttributeData;
use crate::ntfs::data_runs::DataRun;
use log::{info, debug, trace};
use std::collections::HashMap;

pub struct NtfsReader {
    _device: Device,
    boot_sector: NtfsBootSector,
    reader: AlignedDeviceReader,
    mft_reader: MftReader,
    bytes_per_cluster: u32,
    
    // Cache for MFT records
    mft_cache: HashMap<u64, MftRecord>,
    // Cache for MFT's own data runs (to read other MFT records)
    mft_data_runs: Option<Vec<DataRun>>,
}

impl NtfsReader {
    pub fn new(device: Device) -> Result<Self, MosesError> {
        info!("Opening NTFS filesystem on device: {}", device.name);
        
        // Phase 1.1 - Read and parse boot sector
        let boot_reader = NtfsBootSectorReader::new(device.clone())?;
        let boot_sector = *boot_reader.boot_sector();
        boot_reader.sanity_check()?;
        
        // Open device for reading (we'll open it twice - once for general reading, once for MFT)
        use crate::utils::open_device_with_fallback;
        let file = open_device_with_fallback(&device)?;
        let reader = AlignedDeviceReader::new(file);
        
        // Open another handle for MFT reader
        let mft_file = open_device_with_fallback(&device)?;
        let mft_device_reader = AlignedDeviceReader::new(mft_file);
        
        // Phase 1.2 - Initialize MFT reader
        let mft_offset = boot_reader.mft_offset();
        let mft_record_size = boot_sector.mft_record_size();
        let bytes_per_cluster = boot_sector.bytes_per_cluster();
        
        let mft_reader = MftReader::new(
            mft_device_reader,
            mft_offset,
            mft_record_size,
        );
        
        let mut ntfs_reader = Self {
            _device: device,
            boot_sector,
            reader,
            mft_reader,
            bytes_per_cluster,
            mft_cache: HashMap::new(),
            mft_data_runs: None,
        };
        
        // Phase 1.3 - Read MFT record 0 (the MFT itself)
        ntfs_reader.initialize_mft()?;
        
        Ok(ntfs_reader)
    }
    
    /// Initialize MFT by reading record 0 and parsing its data runs
    fn initialize_mft(&mut self) -> Result<(), MosesError> {
        info!("Reading MFT record 0 (MFT itself)");
        
        let mut mft_record = self.mft_reader.read_mft_record()?;
        
        if !mft_record.is_in_use() {
            return Err(MosesError::Other("MFT record 0 is not in use".to_string()));
        }
        
        // Find the DATA attribute of the MFT
        if let Some(data_attr) = mft_record.find_attribute(ATTR_TYPE_DATA) {
            match data_attr {
                AttributeData::DataRuns(runs) => {
                    debug!("MFT has {} data runs", runs.len());
                    self.mft_data_runs = Some(runs.clone());
                }
                _ => {
                    return Err(MosesError::Other("MFT DATA attribute is not non-resident".to_string()));
                }
            }
        } else {
            return Err(MosesError::Other("MFT record 0 has no DATA attribute".to_string()));
        }
        
        // Cache the MFT record
        self.mft_cache.insert(MFT_RECORD_MFT, mft_record);
        
        Ok(())
    }
    
    /// Read an MFT record by number
    pub fn read_mft_record(&mut self, record_num: u64) -> Result<MftRecord, MosesError> {
        // Check cache first
        if let Some(record) = self.mft_cache.get(&record_num) {
            return Ok(record.clone());
        }
        
        // For record 0 and other records in the first chunk, use direct reading
        if record_num < 16 || self.mft_data_runs.is_none() {
            let record = self.mft_reader.read_record(record_num)?;
            self.mft_cache.insert(record_num, record.clone());
            return Ok(record);
        }
        
        // For other records, we need to follow MFT data runs
        // This is complex and would require reading through the MFT's data runs
        // For now, try direct reading
        let record = self.mft_reader.read_record(record_num)?;
        self.mft_cache.insert(record_num, record.clone());
        Ok(record)
    }
    
    /// Read data from cluster chains
    fn read_clusters(&mut self, runs: &[DataRun]) -> Result<Vec<u8>, MosesError> {
        let mut data = Vec::new();
        
        for run in runs {
            if let Some(lcn) = run.lcn {
                // Real data
                let offset = lcn * self.bytes_per_cluster as u64;
                let size = run.length * self.bytes_per_cluster as u64;
                
                trace!("Reading {} clusters at LCN {}", run.length, lcn);
                let cluster_data = self.reader.read_at(offset, size as usize)?;
                data.extend_from_slice(&cluster_data);
            } else {
                // Sparse run - fill with zeros
                let size = run.length * self.bytes_per_cluster as u64;
                data.resize(data.len() + size as usize, 0);
            }
        }
        
        Ok(data)
    }
}

impl FilesystemReader for NtfsReader {
    fn read_metadata(&mut self) -> Result<(), MosesError> {
        // Metadata is read in new()
        Ok(())
    }
    
    fn list_directory(&mut self, path: &str) -> Result<Vec<FileEntry>, MosesError> {
        // Phase 2.1: Enhanced directory listing with B+ tree index support
        let dir_record = if path == "/" || path.is_empty() {
            // Read root directory (MFT record 5)
            self.read_mft_record(MFT_RECORD_ROOT)?
        } else {
            // For now, only support root directory
            return Err(MosesError::Other("Subdirectory navigation not yet implemented".to_string()));
        };
        
        if !dir_record.is_in_use() {
            return Err(MosesError::Other("Directory record not in use".to_string()));
        }
        
        if !dir_record.is_directory() {
            return Err(MosesError::Other("Not a directory".to_string()));
        }
        
        let mut entries = Vec::new();
        let mut index_block_size = 4096u32; // Default
        
        // Parse INDEX_ROOT attribute (small directories)
        let mut mft_record = dir_record;
        if let Some(index_root_attr) = mft_record.find_attribute(ATTR_TYPE_INDEX_ROOT) {
            if let AttributeData::IndexRoot(data) = index_root_attr {
                // Parse the index root
                match crate::ntfs::index::parse_index_root(&data) {
                    Ok(index_entries) => {
                        for entry in index_entries {
                            // Skip . and .. entries
                            if entry.file_name == "." || entry.file_name == ".." {
                                continue;
                            }
                            
                            entries.push(FileEntry {
                                name: entry.file_name,
                                is_directory: entry.is_directory,
                                size: 0, // Would need to read the MFT record for size
                                cluster: Some(entry.mft_reference as u32),
                                metadata: FileMetadata::default(),
                            });
                        }
                    }
                    Err(e) => {
                        log::debug!("Failed to parse INDEX_ROOT: {}", e);
                    }
                }
                
                // Get index block size from INDEX_ROOT
                if data.len() >= 16 {
                    index_block_size = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                }
            }
        }
        
        // Parse INDEX_ALLOCATION attribute (large directories)
        if let Some(index_alloc_attr) = mft_record.find_attribute(ATTR_TYPE_INDEX_ALLOCATION) {
            match index_alloc_attr {
                AttributeData::DataRuns(runs) => {
                    // Read the index allocation data
                    let index_data = self.read_clusters(&runs)?;
                    
                    // Parse the index blocks
                    match crate::ntfs::index::parse_index_allocation(&index_data, index_block_size) {
                        Ok(index_entries) => {
                            for entry in index_entries {
                                // Skip . and .. entries
                                if entry.file_name == "." || entry.file_name == ".." {
                                    continue;
                                }
                                
                                // Avoid duplicates
                                if !entries.iter().any(|e| e.name == entry.file_name) {
                                    entries.push(FileEntry {
                                        name: entry.file_name,
                                        is_directory: entry.is_directory,
                                        size: 0, // Would need to read the MFT record for size
                                        cluster: Some(entry.mft_reference as u32),
                                        metadata: FileMetadata::default(),
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            log::debug!("Failed to parse INDEX_ALLOCATION: {}", e);
                        }
                    }
                }
                _ => {}
            }
        }
        
        // If we didn't find any entries through indexes, fall back to the basic approach
        if entries.is_empty() && (path == "/" || path.is_empty()) {
            // Add some known system files that should exist
            entries.push(FileEntry {
                name: "$MFT".to_string(),
                is_directory: false,
                size: 0,
                cluster: Some(0),
                metadata: FileMetadata::default(),
            });
            
            entries.push(FileEntry {
                name: "$Volume".to_string(),
                is_directory: false,
                size: 0,
                cluster: Some(3),
                metadata: FileMetadata::default(),
            });
        }
        
        Ok(entries)
    }
    
    fn read_file(&mut self, path: &str) -> Result<Vec<u8>, MosesError> {
        // Phase 1.5: Implement file reading through data runs
        
        // For now, only support reading system files by MFT number
        let mft_num = if path == "/$MFT" {
            MFT_RECORD_MFT
        } else if path == "/$Volume" {
            MFT_RECORD_VOLUME
        } else {
            return Err(MosesError::Other("File path resolution not yet implemented".to_string()));
        };
        
        let mut file_record = self.read_mft_record(mft_num)?;
        
        if !file_record.is_in_use() {
            return Err(MosesError::Other("File record not in use".to_string()));
        }
        
        // Find the DATA attribute
        if let Some(data_attr) = file_record.find_attribute(ATTR_TYPE_DATA) {
            match &data_attr {
                AttributeData::Data(resident_data) => {
                    // Resident data - return directly
                    Ok(resident_data.clone())
                }
                AttributeData::DataRuns(runs) => {
                    // Phase 2.3: Enhanced sparse file support
                    // Check if this is a sparse file
                    let sparse_info = crate::ntfs::sparse::analyze_sparse_runs(runs, self.bytes_per_cluster);
                    
                    if sparse_info.is_sparse {
                        trace!("Reading sparse file with {} sparse ranges, {:.1}% space savings",
                            sparse_info.sparse_ranges.len(),
                            crate::ntfs::sparse::calculate_space_savings(&sparse_info));
                    }
                    
                    // Non-resident data - read clusters
                    let data = self.read_clusters(runs)?;
                    
                    // Get actual file size from FILE_NAME attribute
                    if let Some(AttributeData::FileName(file_attr, _)) = 
                        file_record.find_attribute(ATTR_TYPE_FILE_NAME) {
                        // Truncate to actual file size
                        let file_size = file_attr.data_size as usize;
                        if file_size < data.len() {
                            Ok(data[..file_size].to_vec())
                        } else {
                            Ok(data)
                        }
                    } else {
                        Ok(data)
                    }
                }
                AttributeData::CompressedDataRuns(runs, _compression_unit, data_size, _initialized_size) => {
                    // Phase 2.2: Compressed data - read and decompress
                    let compressed_data = self.read_clusters(runs)?;
                    
                    // Decompress the data
                    let decompressed = crate::ntfs::compression::decompress_lznt1(
                        &compressed_data, 
                        *data_size as usize
                    )?;
                    
                    Ok(decompressed)
                }
                _ => {
                    Err(MosesError::Other("Invalid DATA attribute type".to_string()))
                }
            }
        } else {
            // No DATA attribute means empty file
            Ok(Vec::new())
        }
    }
    
    fn get_info(&self) -> FilesystemInfo {
        let total_sectors = self.boot_sector.total_sectors;
        let bytes_per_sector = self.boot_sector.bytes_per_sector;
        let total_bytes = total_sectors * bytes_per_sector as u64;
        
        FilesystemInfo {
            fs_type: "ntfs".to_string(),
            label: None, // TODO: Get from $Volume
            total_bytes,
            used_bytes: 0,  // TODO: Calculate from $Bitmap
            cluster_size: Some(self.bytes_per_cluster),
        }
    }
}