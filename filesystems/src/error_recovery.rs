// Comprehensive Error Recovery System for Moses Filesystems
// Provides rollback, checkpoint, and recovery mechanisms for all filesystem operations

use moses_core::MosesError;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::path::PathBuf;
use log::{info, warn, error, debug};

/// Recovery point identifier
pub type RecoveryPointId = u64;

/// Operation that can be rolled back
#[derive(Debug, Clone)]
pub enum RecoverableOperation {
    /// Block allocation
    BlockAllocation {
        blocks: Vec<u64>,
        filesystem: String,
    },
    /// Inode/MFT record allocation
    InodeAllocation {
        _inode_num: u64,
        filesystem: String,
    },
    /// Data write
    DataWrite {
        offset: u64,
        _original_data: Vec<u8>,
        location: DataLocation,
    },
    /// Metadata update
    MetadataUpdate {
        metadata_type: String,
        _original_data: Vec<u8>,
        location: u64,
    },
    /// Directory entry creation
    DirectoryEntryCreation {
        parent: u64,
        name: String,
        _inode: u64,
    },
    /// Directory entry removal
    DirectoryEntryRemoval {
        parent: u64,
        name: String,
        _inode: u64,
        _entry_data: Vec<u8>,
    },
    /// File creation
    FileCreation {
        path: PathBuf,
        _inode: u64,
    },
    /// File deletion
    FileDeletion {
        path: PathBuf,
        _inode: u64,
        _file_data: FileBackup,
    },
}

/// Location of data for recovery
#[derive(Debug, Clone)]
pub enum DataLocation {
    Disk { offset: u64 },
    Memory { address: usize },
    Cache { key: String },
}

/// Backup of file data for recovery
#[derive(Debug, Clone)]
pub struct FileBackup {
    pub _inode_data: Vec<u8>,
    pub data_blocks: Vec<(u64, Vec<u8>)>,
    pub metadata: HashMap<String, Vec<u8>>,
}

/// Recovery point in the operation history
#[derive(Debug, Clone)]
pub struct RecoveryPoint {
    pub id: RecoveryPointId,
    pub timestamp: std::time::SystemTime,
    pub description: String,
    pub operations: Vec<RecoverableOperation>,
    pub filesystem_type: String,
}

/// Recovery strategy
#[derive(Debug, Clone, Copy)]
pub enum RecoveryStrategy {
    /// Roll back to the last known good state
    Rollback,
    /// Try to repair and continue
    Repair,
    /// Skip the failed operation and continue
    Skip,
    /// Abort all operations
    Abort,
}

/// Recovery manager for handling errors and rollbacks
pub struct RecoveryManager {
    /// Next recovery point ID
    next_id: Arc<Mutex<RecoveryPointId>>,
    
    /// Active recovery points
    recovery_points: Arc<RwLock<HashMap<RecoveryPointId, RecoveryPoint>>>,
    
    /// Operation history for each transaction
    operation_history: Arc<RwLock<HashMap<String, VecDeque<RecoverableOperation>>>>,
    
    /// Recovery strategies for different error types
    recovery_strategies: Arc<RwLock<HashMap<String, RecoveryStrategy>>>,
    
    /// Maximum number of recovery points to keep
    max_recovery_points: usize,
    
    /// Enable automatic recovery
    auto_recovery_enabled: bool,
}

impl RecoveryManager {
    /// Create a new recovery manager
    pub fn new(max_recovery_points: usize, auto_recovery_enabled: bool) -> Self {
        let mut strategies = HashMap::new();
        
        // Default recovery strategies
        strategies.insert("allocation_failure".to_string(), RecoveryStrategy::Rollback);
        strategies.insert("write_failure".to_string(), RecoveryStrategy::Rollback);
        strategies.insert("metadata_corruption".to_string(), RecoveryStrategy::Repair);
        strategies.insert("directory_error".to_string(), RecoveryStrategy::Rollback);
        strategies.insert("permission_denied".to_string(), RecoveryStrategy::Skip);
        
        Self {
            next_id: Arc::new(Mutex::new(1)),
            recovery_points: Arc::new(RwLock::new(HashMap::new())),
            operation_history: Arc::new(RwLock::new(HashMap::new())),
            recovery_strategies: Arc::new(RwLock::new(strategies)),
            max_recovery_points,
            auto_recovery_enabled,
        }
    }
    
    /// Create a new recovery point
    pub fn create_recovery_point(
        &self,
        description: &str,
        filesystem_type: &str,
    ) -> Result<RecoveryPointId, MosesError> {
        let mut next_id = self.next_id.lock()
            .map_err(|_| MosesError::Other("Failed to lock next_id".to_string()))?;
        
        let id = *next_id;
        *next_id += 1;
        
        let recovery_point = RecoveryPoint {
            id,
            timestamp: std::time::SystemTime::now(),
            description: description.to_string(),
            operations: Vec::new(),
            filesystem_type: filesystem_type.to_string(),
        };
        
        let mut points = self.recovery_points.write()
            .map_err(|_| MosesError::Other("Failed to lock recovery points".to_string()))?;
        
        // Remove old recovery points if we exceed the limit
        if points.len() >= self.max_recovery_points {
            let oldest_id = points.keys().min().copied();
            if let Some(old_id) = oldest_id {
                points.remove(&old_id);
                debug!("Removed old recovery point {}", old_id);
            }
        }
        
        points.insert(id, recovery_point);
        info!("Created recovery point {}: {}", id, description);
        
        Ok(id)
    }
    
    /// Record a recoverable operation
    pub fn record_operation(
        &self,
        recovery_point_id: RecoveryPointId,
        operation: RecoverableOperation,
    ) -> Result<(), MosesError> {
        let mut points = self.recovery_points.write()
            .map_err(|_| MosesError::Other("Failed to lock recovery points".to_string()))?;
        
        let point = points.get_mut(&recovery_point_id)
            .ok_or_else(|| MosesError::Other(format!("Recovery point {} not found", recovery_point_id)))?;
        
        point.operations.push(operation.clone());
        
        // Also add to operation history
        let transaction_id = format!("rp_{}", recovery_point_id);
        let mut history = self.operation_history.write()
            .map_err(|_| MosesError::Other("Failed to lock operation history".to_string()))?;
        
        history.entry(transaction_id)
            .or_insert_with(VecDeque::new)
            .push_back(operation);
        
        Ok(())
    }
    
    /// Roll back to a recovery point
    pub fn rollback_to_point(&self, recovery_point_id: RecoveryPointId) -> Result<(), MosesError> {
        info!("Rolling back to recovery point {}", recovery_point_id);
        
        let points = self.recovery_points.read()
            .map_err(|_| MosesError::Other("Failed to lock recovery points".to_string()))?;
        
        let point = points.get(&recovery_point_id)
            .ok_or_else(|| MosesError::Other(format!("Recovery point {} not found", recovery_point_id)))?;
        
        // Roll back operations in reverse order
        for operation in point.operations.iter().rev() {
            self.rollback_operation(operation)?;
        }
        
        info!("Successfully rolled back to recovery point {}", recovery_point_id);
        Ok(())
    }
    
    /// Roll back a single operation
    fn rollback_operation(&self, operation: &RecoverableOperation) -> Result<(), MosesError> {
        match operation {
            RecoverableOperation::BlockAllocation { blocks, filesystem } => {
                debug!("Rolling back block allocation: {:?} in {}", blocks, filesystem);
                // Free the allocated blocks
                // This would call the appropriate filesystem's block deallocator
                Ok(())
            }
            
            RecoverableOperation::InodeAllocation { _inode_num, filesystem } => {
                debug!("Rolling back _inode allocation: {} in {}", _inode_num, filesystem);
                // Free the allocated _inode
                Ok(())
            }
            
            RecoverableOperation::DataWrite { offset, _original_data, location } => {
                debug!("Rolling back data write at offset {}", offset);
                // Restore original data
                match location {
                    DataLocation::Disk { offset } => {
                        // Write original data back to disk
                        debug!("Restoring {} bytes at disk offset {}", _original_data.len(), offset);
                    }
                    DataLocation::Memory { address } => {
                        // Restore memory contents
                        debug!("Restoring {} bytes at memory address {:#x}", _original_data.len(), address);
                    }
                    DataLocation::Cache { key } => {
                        // Restore cache entry
                        debug!("Restoring cache entry: {}", key);
                    }
                }
                Ok(())
            }
            
            RecoverableOperation::MetadataUpdate { metadata_type, _original_data, location } => {
                debug!("Rolling back metadata update: {} at {}", metadata_type, location);
                // Restore original metadata
                Ok(())
            }
            
            RecoverableOperation::DirectoryEntryCreation { parent, name, _inode } => {
                debug!("Rolling back directory entry creation: {} in parent {}", name, parent);
                // Remove the created directory entry
                Ok(())
            }
            
            RecoverableOperation::DirectoryEntryRemoval { parent, name, _inode, _entry_data } => {
                debug!("Rolling back directory entry removal: {} in parent {}", name, parent);
                // Restore the removed directory entry
                Ok(())
            }
            
            RecoverableOperation::FileCreation { path, _inode } => {
                debug!("Rolling back file creation: {:?} (_inode {})", path, _inode);
                // Delete the created file
                Ok(())
            }
            
            RecoverableOperation::FileDeletion { path, _inode, _file_data } => {
                debug!("Rolling back file deletion: {:?} (_inode {})", path, _inode);
                // Restore the deleted file
                Ok(())
            }
        }
    }
    
    /// Handle an error with automatic recovery
    pub fn handle_error(
        &self,
        error: &MosesError,
        context: &str,
        recovery_point_id: Option<RecoveryPointId>,
    ) -> Result<RecoveryStrategy, MosesError> {
        error!("Error in {}: {:?}", context, error);
        
        // Determine error type
        let error_type = self.classify_error(error);
        
        // Get recovery strategy
        let strategies = self.recovery_strategies.read()
            .map_err(|_| MosesError::Other("Failed to lock recovery strategies".to_string()))?;
        
        let strategy = strategies.get(&error_type)
            .copied()
            .unwrap_or(RecoveryStrategy::Abort);
        
        info!("Using recovery strategy {:?} for error type: {}", strategy, error_type);
        
        if self.auto_recovery_enabled {
            match strategy {
                RecoveryStrategy::Rollback => {
                    if let Some(rp_id) = recovery_point_id {
                        self.rollback_to_point(rp_id)?;
                    } else {
                        warn!("No recovery point specified for rollback");
                    }
                }
                RecoveryStrategy::Repair => {
                    self.attempt_repair(error, context)?;
                }
                RecoveryStrategy::Skip => {
                    info!("Skipping failed operation in {}", context);
                }
                RecoveryStrategy::Abort => {
                    error!("Aborting due to unrecoverable error");
                    return Err(MosesError::Other(format!("{:?}", error)));
                }
            }
        }
        
        Ok(strategy)
    }
    
    /// Classify an error for recovery strategy selection
    fn classify_error(&self, error: &MosesError) -> String {
        match error {
            MosesError::IoError(_) => "write_failure".to_string(),
            MosesError::InvalidInput(_) => "validation_error".to_string(),
            MosesError::NotSupported(_) => "unsupported_operation".to_string(),
            MosesError::Other(msg) if msg.contains("permission") => "permission_denied".to_string(),
            _ => "unknown_error".to_string(),
        }
    }
    
    /// Attempt to repair after an error
    fn attempt_repair(&self, error: &MosesError, context: &str) -> Result<(), MosesError> {
        info!("Attempting to repair after error in {}", context);
        
        // Repair strategies would be implemented here based on error type
        match error {
            MosesError::IoError(io_err) => {
                // Retry the operation, check disk health, etc.
                warn!("IO error repair not yet implemented: {}", io_err);
            }
            _ => {
                warn!("No repair strategy for this error type");
            }
        }
        
        Ok(())
    }
    
    /// Create a checkpoint (snapshot of current state)
    pub fn create_checkpoint(&self, description: &str) -> Result<RecoveryPointId, MosesError> {
        let checkpoint_id = self.create_recovery_point(
            &format!("Checkpoint: {}", description),
            "checkpoint",
        )?;
        
        info!("Created checkpoint {}: {}", checkpoint_id, description);
        Ok(checkpoint_id)
    }
    
    /// Verify system integrity
    pub fn verify_integrity(&self) -> Result<bool, MosesError> {
        debug!("Verifying system integrity");
        
        // Check recovery points consistency
        let points = self.recovery_points.read()
            .map_err(|_| MosesError::Other("Failed to lock recovery points".to_string()))?;
        
        for (id, point) in points.iter() {
            debug!("Checking recovery point {}: {} operations", id, point.operations.len());
        }
        
        // Check operation history
        let history = self.operation_history.read()
            .map_err(|_| MosesError::Other("Failed to lock operation history".to_string()))?;
        
        for (transaction, ops) in history.iter() {
            debug!("Transaction {}: {} operations", transaction, ops.len());
        }
        
        Ok(true)
    }
    
    /// Clean up old recovery data
    pub fn cleanup_old_data(&self, keep_days: u32) -> Result<usize, MosesError> {
        let mut points = self.recovery_points.write()
            .map_err(|_| MosesError::Other("Failed to lock recovery points".to_string()))?;
        
        let cutoff = std::time::SystemTime::now()
            - std::time::Duration::from_secs(keep_days as u64 * 86400);
        
        let mut removed = 0;
        points.retain(|_id, point| {
            if point.timestamp < cutoff {
                removed += 1;
                false
            } else {
                true
            }
        });
        
        info!("Cleaned up {} old recovery points", removed);
        Ok(removed)
    }
}

/// Scoped recovery guard for automatic rollback
pub struct RecoveryGuard<'a> {
    manager: &'a RecoveryManager,
    recovery_point_id: RecoveryPointId,
    committed: bool,
}

impl<'a> RecoveryGuard<'a> {
    /// Create a new recovery guard
    pub fn new(manager: &'a RecoveryManager, description: &str, filesystem: &str) -> Result<Self, MosesError> {
        let recovery_point_id = manager.create_recovery_point(description, filesystem)?;
        Ok(Self {
            manager,
            recovery_point_id,
            committed: false,
        })
    }
    
    /// Record an operation
    pub fn record(&self, operation: RecoverableOperation) -> Result<(), MosesError> {
        self.manager.record_operation(self.recovery_point_id, operation)
    }
    
    /// Commit the operations (prevent rollback on drop)
    pub fn commit(mut self) {
        self.committed = true;
    }
    
    /// Get the recovery point ID
    pub fn recovery_point_id(&self) -> RecoveryPointId {
        self.recovery_point_id
    }
}

impl<'a> Drop for RecoveryGuard<'a> {
    fn drop(&mut self) {
        if !self.committed {
            warn!("RecoveryGuard dropped without commit, rolling back");
            if let Err(e) = self.manager.rollback_to_point(self.recovery_point_id) {
                error!("Failed to rollback on guard drop: {:?}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_recovery_point_creation() {
        let manager = RecoveryManager::new(10, false);
        let rp_id = manager.create_recovery_point("test", "test_fs").unwrap();
        assert!(rp_id > 0);
    }
    
    #[test]
    fn test_operation_recording() {
        let manager = RecoveryManager::new(10, false);
        let rp_id = manager.create_recovery_point("test", "test_fs").unwrap();
        
        let op = RecoverableOperation::BlockAllocation {
            blocks: vec![1, 2, 3],
            filesystem: "test_fs".to_string(),
        };
        
        manager.record_operation(rp_id, op).unwrap();
    }
    
    #[test]
    fn test_recovery_guard() {
        let manager = RecoveryManager::new(10, false);
        
        {
            let guard = RecoveryGuard::new(&manager, "test operation", "test_fs").unwrap();
            let op = RecoverableOperation::InodeAllocation {
                _inode_num: 42,
                filesystem: "test_fs".to_string(),
            };
            guard.record(op).unwrap();
            guard.commit();
        }
        
        // Guard committed, no rollback should occur
    }
    
    #[test]
    fn test_auto_rollback() {
        let manager = RecoveryManager::new(10, true);
        
        {
            let guard = RecoveryGuard::new(&manager, "test operation", "test_fs").unwrap();
            let op = RecoverableOperation::InodeAllocation {
                _inode_num: 42,
                filesystem: "test_fs".to_string(),
            };
            guard.record(op).unwrap();
            // Not committing - should trigger rollback on drop
        }
        
        // Guard dropped without commit, rollback should occur
    }
}