// Transaction Management for EXT4 Journaling
// Handles transaction lifecycle and buffer management

use moses_core::MosesError;
use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;
use super::jbd2::Jbd2Journal;

/// Transaction handle for users
pub struct Handle {
    /// Journal reference
    journal: Arc<Jbd2Journal>,
    /// Transaction ID
    tid: u64,
    /// Number of buffers reserved
    reserved_buffers: u32,
    /// Is this handle active?
    active: bool,
}

impl Handle {
    /// Create a new transaction handle
    pub fn new(journal: Arc<Jbd2Journal>, blocks_needed: u32) -> Result<Self, MosesError> {
        let tid = journal.start_transaction(blocks_needed)?;
        
        Ok(Self {
            journal,
            tid,
            reserved_buffers: blocks_needed,
            active: true,
        })
    }
    
    /// Get metadata access for a block
    pub fn get_write_access(&mut self, blocknr: u64, data: Vec<u8>) -> Result<(), MosesError> {
        if !self.active {
            return Err(MosesError::Other("Transaction handle not active".to_string()));
        }
        
        self.journal.add_block(self.tid, blocknr, data)?;
        Ok(())
    }
    
    /// Commit this handle's transaction
    pub fn commit(mut self) -> Result<(), MosesError> {
        if !self.active {
            return Err(MosesError::Other("Transaction already committed".to_string()));
        }
        
        self.journal.commit_transaction(self.tid)?;
        self.active = false;
        Ok(())
    }
    
    /// Abort this transaction
    pub fn abort(mut self) -> Result<(), MosesError> {
        if !self.active {
            return Ok(());
        }
        
        self.journal.abort_transaction(self.tid)?;
        self.active = false;
        Ok(())
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if self.active {
            // Auto-commit on drop if not explicitly handled
            let _ = self.journal.commit_transaction(self.tid);
        }
    }
}

/// Main transaction manager
pub struct Transaction {
    /// Journal reference
    journal: Arc<Jbd2Journal>,
    /// Current transaction ID
    current_tid: Arc<RwLock<Option<u64>>>,
    /// Active handles
    active_handles: Arc<Mutex<HashMap<u64, u32>>>,
}

impl Transaction {
    /// Create a new transaction manager
    pub fn new(journal: Arc<Jbd2Journal>) -> Self {
        Self {
            journal,
            current_tid: Arc::new(RwLock::new(None)),
            active_handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Begin a new transaction
    pub fn begin(&self, blocks_needed: u32) -> Result<Handle, MosesError> {
        Handle::new(self.journal.clone(), blocks_needed)
    }
    
    /// Create a nested transaction (uses same underlying transaction)
    pub fn nested(&self) -> Result<Handle, MosesError> {
        let tid = {
            let current = self.current_tid.read().unwrap();
            if let Some(tid) = *current {
                tid
            } else {
                // Start new transaction if none exists
                self.journal.start_transaction(1)?
            }
        };
        
        Ok(Handle {
            journal: self.journal.clone(),
            tid,
            reserved_buffers: 0,
            active: true,
        })
    }
    
    /// Force commit all active transactions
    pub fn sync(&self) -> Result<(), MosesError> {
        let current = self.current_tid.read().unwrap();
        if let Some(tid) = *current {
            self.journal.commit_transaction(tid)?;
        }
        
        // Checkpoint to ensure data is on disk
        self.journal.checkpoint()?;
        Ok(())
    }
}

/// Transaction barrier for ordered operations
pub struct Barrier {
    /// Transactions that must complete before this barrier
    dependencies: Vec<u64>,
    /// Is this barrier satisfied?
    satisfied: Arc<RwLock<bool>>,
}

impl Barrier {
    /// Create a new barrier
    pub fn new(dependencies: Vec<u64>) -> Self {
        Self {
            dependencies,
            satisfied: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Wait for barrier to be satisfied
    pub fn wait(&self) -> Result<(), MosesError> {
        // In a real implementation, this would block until dependencies are committed
        // For now, just check the flag
        let satisfied = self.satisfied.read().unwrap();
        if *satisfied {
            Ok(())
        } else {
            Err(MosesError::Other("Barrier not satisfied".to_string()))
        }
    }
    
    /// Mark barrier as satisfied
    pub fn satisfy(&self) {
        let mut satisfied = self.satisfied.write().unwrap();
        *satisfied = true;
    }
}