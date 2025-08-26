// NTFS Writer - Phase 3: Basic Write Support
// Provides write capabilities for NTFS volumes with safety checks
//
// IMPORTANT: Writing to NTFS is complex and dangerous. This implementation
// includes multiple safety layers to prevent data corruption.

use moses_core::{Device, MosesError};
use crate::device_reader::AlignedDeviceReader;
use crate::ntfs::boot_sector::NtfsBootSectorReader;
use crate::ntfs::mft::{MftReader, MftRecord};
use crate::ntfs::structures::*;
use crate::ntfs::attributes::AttributeData;
use crate::ntfs::data_runs::DataRun;
use log::{info, debug, warn, error};
use std::collections::{HashMap, HashSet};
use std::io::{Write, Seek, SeekFrom};

/// Safety configuration for NTFS write operations
#[derive(Debug, Clone)]
pub struct NtfsWriteConfig {
    /// Enable actual writes to disk (false = dry run mode)
    pub enable_writes: bool,
    
    /// Verify all writes by reading back
    pub verify_writes: bool,
    
    /// Create backup of modified sectors before writing
    pub backup_before_write: bool,
    
    /// Maximum number of MFT records to modify in one operation
    pub max_mft_modifications: usize,
    
    /// Enable transaction logging
    pub enable_transactions: bool,
}

impl Default for NtfsWriteConfig {
    fn default() -> Self {
        Self {
            enable_writes: false,  // Safe default: dry run
            verify_writes: true,
            backup_before_write: true,
            max_mft_modifications: 10,
            enable_transactions: true,
        }
    }
}

/// Transaction log entry for rollback support
#[derive(Debug, Clone)]
struct TransactionEntry {
    /// Offset in the volume
    offset: u64,
    /// Original data before modification
    original_data: Vec<u8>,
    /// New data to write
    new_data: Vec<u8>,
    /// Description of the operation
    description: String,
}

/// NTFS Writer with safety checks and transaction support
pub struct NtfsWriter {
    _device: Device,
    pub(crate) boot_sector: NtfsBootSector,
    pub(crate) reader: AlignedDeviceReader,
    pub(crate) writer: std::fs::File,  // Separate handle for writing
    pub(crate) mft_reader: MftReader,
    pub(crate) bytes_per_cluster: u32,
    pub(crate) sectors_per_cluster: u8,
    
    // Configuration
    pub(crate) config: NtfsWriteConfig,
    
    // Transaction management
    transaction_log: Vec<TransactionEntry>,
    transaction_active: bool,
    
    // Cached data structures
    pub(crate) mft_cache: HashMap<u64, MftRecord>,
    pub(crate) mft_data_runs: Option<Vec<DataRun>>,
    
    // Bitmap management
    pub(crate) mft_bitmap: Option<Vec<u8>>,
    pub(crate) volume_bitmap: Option<Vec<u8>>,
    
    // Safety tracking
    pub(crate) modified_mft_records: HashSet<u64>,
    pub(crate) modified_clusters: HashSet<u64>,
}

impl NtfsWriter {
    /// Create a new NTFS writer with the given configuration
    pub fn new(device: Device, config: NtfsWriteConfig) -> Result<Self, MosesError> {
        info!("Opening NTFS filesystem for writing on device: {}", device.name);
        
        if config.enable_writes {
            warn!("NTFS write mode is ENABLED - modifications will be written to disk!");
        } else {
            info!("NTFS writer in DRY RUN mode - no actual writes will occur");
        }
        
        // Read and verify boot sector
        let boot_reader = NtfsBootSectorReader::new(device.clone())?;
        let boot_sector = *boot_reader.boot_sector();
        boot_reader.sanity_check()?;
        
        // Open device for reading
        use crate::utils::open_device_with_fallback;
        let read_file = open_device_with_fallback(&device)?;
        let reader = AlignedDeviceReader::new(read_file);
        
        // Open device for writing (separate handle)
        let write_file = if config.enable_writes {
            use std::fs::OpenOptions;
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::fs::OpenOptionsExt;
                use winapi::um::winnt::{GENERIC_READ, GENERIC_WRITE, FILE_SHARE_READ};
                
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .access_mode(GENERIC_READ | GENERIC_WRITE)
                    .share_mode(FILE_SHARE_READ)
                    .open(&device.mount_points[0])?
            }
            #[cfg(not(target_os = "windows"))]
            {
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&device.mount_points[0])?
            }
        } else {
            // Dummy file handle for dry run mode
            open_device_with_fallback(&device)?
        };
        
        // Initialize MFT reader
        let mft_file = open_device_with_fallback(&device)?;
        let mft_device_reader = AlignedDeviceReader::new(mft_file);
        
        let mft_offset = boot_reader.mft_offset();
        let mft_record_size = boot_sector.mft_record_size();
        let bytes_per_cluster = boot_sector.bytes_per_cluster();
        let sectors_per_cluster = boot_sector.sectors_per_cluster;
        
        let mft_reader = MftReader::new(
            mft_device_reader,
            mft_offset,
            mft_record_size,
        );
        
        let mut writer = Self {
            _device: device,
            boot_sector,
            reader,
            writer: write_file,
            mft_reader,
            bytes_per_cluster,
            sectors_per_cluster,
            config,
            transaction_log: Vec::new(),
            transaction_active: false,
            mft_cache: HashMap::new(),
            mft_data_runs: None,
            mft_bitmap: None,
            volume_bitmap: None,
            modified_mft_records: HashSet::new(),
            modified_clusters: HashSet::new(),
        };
        
        // Initialize MFT data runs and bitmaps
        writer.initialize()?;
        
        Ok(writer)
    }
    
    /// Initialize the writer by loading critical metadata
    fn initialize(&mut self) -> Result<(), MosesError> {
        info!("Initializing NTFS writer metadata");
        
        // Read MFT record 0 (MFT itself)
        let mut mft_record = self.mft_reader.read_mft_record()?;
        
        if !mft_record.is_in_use() {
            return Err(MosesError::Other("MFT record 0 is not in use".to_string()));
        }
        
        // Get MFT data runs
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
        }
        
        // Cache the MFT record
        self.mft_cache.insert(MFT_RECORD_MFT, mft_record);
        
        // Load MFT bitmap (record 6)
        self.load_mft_bitmap()?;
        
        // Load volume bitmap (record 6)
        self.load_volume_bitmap()?;
        
        Ok(())
    }
    
    /// Load the MFT bitmap to track allocated records
    fn load_mft_bitmap(&mut self) -> Result<(), MosesError> {
        debug!("Loading MFT bitmap from record 6");
        
        // Read MFT record 6 ($Bitmap)
        let mut bitmap_record = self.read_mft_record(MFT_RECORD_BITMAP)?;
        
        if !bitmap_record.is_in_use() {
            return Err(MosesError::Other("$Bitmap record is not in use".to_string()));
        }
        
        // Find the DATA attribute
        if let Some(data_attr) = bitmap_record.find_attribute(ATTR_TYPE_DATA) {
            match data_attr {
                AttributeData::Data(data) => {
                    self.mft_bitmap = Some(data.clone());
                    debug!("Loaded MFT bitmap: {} bytes", data.len());
                }
                AttributeData::DataRuns(runs) => {
                    // Read bitmap from data runs
                    let mut bitmap_data = Vec::new();
                    for run in runs {
                        if let Some(lcn) = run.lcn {
                            let cluster_data = self.read_cluster(lcn)?;
                            bitmap_data.extend_from_slice(&cluster_data);
                        }
                    }
                    self.mft_bitmap = Some(bitmap_data);
                    debug!("Loaded MFT bitmap from {} data runs", runs.len());
                }
                _ => {}
            }
        }
        
        Ok(())
    }
    
    /// Load the volume bitmap to track allocated clusters
    fn load_volume_bitmap(&mut self) -> Result<(), MosesError> {
        debug!("Loading volume bitmap");
        
        // The volume bitmap is typically stored as a named stream in $Bitmap
        // For now, we'll initialize an empty bitmap
        // TODO: Implement proper volume bitmap loading
        
        let total_clusters = self.boot_sector.total_sectors / self.sectors_per_cluster as u64;
        let bitmap_size = (total_clusters + 7) / 8; // Round up to byte boundary
        
        self.volume_bitmap = Some(vec![0; bitmap_size as usize]);
        debug!("Initialized volume bitmap for {} clusters", total_clusters);
        
        Ok(())
    }
    
    /// Read an MFT record by number
    pub fn read_mft_record(&mut self, record_num: u64) -> Result<MftRecord, MosesError> {
        // Check cache first
        if let Some(cached) = self.mft_cache.get(&record_num) {
            return Ok(cached.clone());
        }
        
        // Read from disk using the read_record method
        let record = self.mft_reader.read_record(record_num)?;
        
        // Cache it
        self.mft_cache.insert(record_num, record.clone());
        
        Ok(record)
    }
    
    /// Read a cluster by number
    fn read_cluster(&mut self, cluster_num: u64) -> Result<Vec<u8>, MosesError> {
        let offset = cluster_num * self.bytes_per_cluster as u64;
        self.reader.read_at(offset, self.bytes_per_cluster as usize)
    }
    
    // ===== TRANSACTION MANAGEMENT =====
    
    /// Begin a new transaction
    pub fn begin_transaction(&mut self) -> Result<(), MosesError> {
        if self.transaction_active {
            return Err(MosesError::Other("Transaction already active".to_string()));
        }
        
        info!("Beginning NTFS write transaction");
        self.transaction_active = true;
        self.transaction_log.clear();
        self.modified_mft_records.clear();
        self.modified_clusters.clear();
        
        Ok(())
    }
    
    /// Commit the current transaction
    pub fn commit_transaction(&mut self) -> Result<(), MosesError> {
        if !self.transaction_active {
            return Err(MosesError::Other("No active transaction".to_string()));
        }
        
        info!("Committing NTFS transaction with {} operations", self.transaction_log.len());
        
        // Check safety limits
        if self.modified_mft_records.len() > self.config.max_mft_modifications {
            return Err(MosesError::Other(format!(
                "Transaction modifies too many MFT records: {} > {}",
                self.modified_mft_records.len(),
                self.config.max_mft_modifications
            )));
        }
        
        // Apply all writes
        let entries = self.transaction_log.clone();
        for entry in &entries {
            self.apply_transaction_entry(entry)?;
        }
        
        // Clear transaction state
        self.transaction_active = false;
        self.transaction_log.clear();
        
        info!("Transaction committed successfully");
        Ok(())
    }
    
    /// Rollback the current transaction
    pub fn rollback_transaction(&mut self) -> Result<(), MosesError> {
        if !self.transaction_active {
            return Ok(());
        }
        
        warn!("Rolling back NTFS transaction");
        
        // Restore original data for any writes that were applied
        for entry in self.transaction_log.iter().rev() {
            if self.config.enable_writes {
                self.writer.seek(SeekFrom::Start(entry.offset))?;
                self.writer.write_all(&entry.original_data)?;
                self.writer.flush()?;
            }
        }
        
        // Clear transaction state
        self.transaction_active = false;
        self.transaction_log.clear();
        self.modified_mft_records.clear();
        self.modified_clusters.clear();
        
        // Clear caches to force re-reading from disk
        self.mft_cache.clear();
        
        info!("Transaction rolled back");
        Ok(())
    }
    
    /// Apply a single transaction entry
    fn apply_transaction_entry(&mut self, entry: &TransactionEntry) -> Result<(), MosesError> {
        debug!("Applying transaction: {} at offset {:#x}", entry.description, entry.offset);
        
        if self.config.backup_before_write {
            // Backup is already stored in entry.original_data
            debug!("Backup available: {} bytes", entry.original_data.len());
        }
        
        if self.config.enable_writes {
            // Perform the actual write
            self.writer.seek(SeekFrom::Start(entry.offset))?;
            self.writer.write_all(&entry.new_data)?;
            self.writer.flush()?;
            
            if self.config.verify_writes {
                // Read back and verify
                let verify_buf = vec![0u8; entry.new_data.len()];
                self.reader.read_at(entry.offset, entry.new_data.len())?;
                
                if verify_buf != entry.new_data {
                    error!("Write verification failed at offset {:#x}", entry.offset);
                    return Err(MosesError::Other("Write verification failed".to_string()));
                }
                
                debug!("Write verified successfully");
            }
        } else {
            debug!("DRY RUN: Would write {} bytes to offset {:#x}", 
                   entry.new_data.len(), entry.offset);
        }
        
        Ok(())
    }
    
    // ===== BITMAP MANAGEMENT =====
    
    /// Find a free MFT record
    pub fn find_free_mft_record(&self) -> Result<u64, MosesError> {
        let bitmap = self.mft_bitmap.as_ref()
            .ok_or_else(|| MosesError::Other("MFT bitmap not loaded".to_string()))?;
        
        // Search for a clear bit in the bitmap
        for (byte_idx, byte) in bitmap.iter().enumerate() {
            if *byte != 0xFF {
                // This byte has at least one free bit
                for bit in 0..8 {
                    if (*byte & (1 << bit)) == 0 {
                        let record_num = (byte_idx * 8 + bit) as u64;
                        debug!("Found free MFT record: {}", record_num);
                        return Ok(record_num);
                    }
                }
            }
        }
        
        Err(MosesError::Other("No free MFT records available".to_string()))
    }
    
    /// Allocate an MFT record
    pub fn allocate_mft_record(&mut self, record_num: u64) -> Result<(), MosesError> {
        if !self.transaction_active {
            return Err(MosesError::Other("No active transaction".to_string()));
        }
        
        let bitmap = self.mft_bitmap.as_mut()
            .ok_or_else(|| MosesError::Other("MFT bitmap not loaded".to_string()))?;
        
        let byte_idx = (record_num / 8) as usize;
        let bit_idx = (record_num % 8) as u8;
        
        if byte_idx >= bitmap.len() {
            return Err(MosesError::Other("MFT record number out of range".to_string()));
        }
        
        // Check if already allocated
        if (bitmap[byte_idx] & (1 << bit_idx)) != 0 {
            return Err(MosesError::Other("MFT record already allocated".to_string()));
        }
        
        // Mark as allocated
        bitmap[byte_idx] |= 1 << bit_idx;
        self.modified_mft_records.insert(record_num);
        
        debug!("Allocated MFT record {}", record_num);
        Ok(())
    }
    
    /// Free an MFT record
    pub fn free_mft_record(&mut self, record_num: u64) -> Result<(), MosesError> {
        if !self.transaction_active {
            return Err(MosesError::Other("No active transaction".to_string()));
        }
        
        let bitmap = self.mft_bitmap.as_mut()
            .ok_or_else(|| MosesError::Other("MFT bitmap not loaded".to_string()))?;
        
        let byte_idx = (record_num / 8) as usize;
        let bit_idx = (record_num % 8) as u8;
        
        if byte_idx >= bitmap.len() {
            return Err(MosesError::Other("MFT record number out of range".to_string()));
        }
        
        // Mark as free
        bitmap[byte_idx] &= !(1 << bit_idx);
        self.modified_mft_records.insert(record_num);
        
        debug!("Freed MFT record {}", record_num);
        Ok(())
    }
    
    /// Find free clusters for allocation
    pub fn find_free_clusters(&self, count: u64) -> Result<Vec<u64>, MosesError> {
        let bitmap = self.volume_bitmap.as_ref()
            .ok_or_else(|| MosesError::Other("Volume bitmap not loaded".to_string()))?;
        
        let mut free_clusters = Vec::new();
        let mut consecutive = 0u64;
        let mut start_cluster = 0u64;
        
        for (byte_idx, byte) in bitmap.iter().enumerate() {
            for bit in 0..8 {
                let cluster_num = (byte_idx * 8 + bit) as u64;
                
                if (*byte & (1 << bit)) == 0 {
                    // Free cluster
                    if consecutive == 0 {
                        start_cluster = cluster_num;
                    }
                    consecutive += 1;
                    
                    if consecutive >= count {
                        // Found enough consecutive clusters
                        for i in 0..count {
                            free_clusters.push(start_cluster + i);
                        }
                        debug!("Found {} free clusters starting at {}", count, start_cluster);
                        return Ok(free_clusters);
                    }
                } else {
                    // Allocated cluster, reset search
                    consecutive = 0;
                }
            }
        }
        
        Err(MosesError::Other(format!("Could not find {} consecutive free clusters", count)))
    }
    
    /// Allocate clusters
    pub fn allocate_clusters(&mut self, clusters: &[u64]) -> Result<(), MosesError> {
        if !self.transaction_active {
            return Err(MosesError::Other("No active transaction".to_string()));
        }
        
        let bitmap = self.volume_bitmap.as_mut()
            .ok_or_else(|| MosesError::Other("Volume bitmap not loaded".to_string()))?;
        
        for &cluster_num in clusters {
            let byte_idx = (cluster_num / 8) as usize;
            let bit_idx = (cluster_num % 8) as u8;
            
            if byte_idx >= bitmap.len() {
                return Err(MosesError::Other("Cluster number out of range".to_string()));
            }
            
            // Check if already allocated
            if (bitmap[byte_idx] & (1 << bit_idx)) != 0 {
                return Err(MosesError::Other(format!("Cluster {} already allocated", cluster_num)));
            }
            
            // Mark as allocated
            bitmap[byte_idx] |= 1 << bit_idx;
            self.modified_clusters.insert(cluster_num);
        }
        
        debug!("Allocated {} clusters", clusters.len());
        Ok(())
    }
}

// Public API methods will be added in subsequent phases
impl NtfsWriter {
    /// Get the current configuration
    pub fn config(&self) -> &NtfsWriteConfig {
        &self.config
    }
    
    /// Set a new configuration
    pub fn set_config(&mut self, config: NtfsWriteConfig) {
        self.config = config;
    }
    
    /// Check if writer is in dry run mode
    pub fn is_dry_run(&self) -> bool {
        !self.config.enable_writes
    }
}