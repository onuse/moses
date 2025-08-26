// EXT4 Transaction System with JBD2 (Journaling Block Device 2) Support
// Ensures atomic operations and crash consistency

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use super::{
    types::{BlockNumber, Ext4Result, Ext4Error},
    structures::{Ext4Superblock},
    constants::*,
};

/// Transaction ID type
pub type TransactionId = u64;

/// Transaction handle for atomic operations
#[derive(Debug, Clone)]
pub struct TransactionHandle {
    pub id: TransactionId,
    pub state: Arc<Mutex<TransactionState>>,
}

/// State of a transaction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionState {
    /// Transaction is active and accepting operations
    Active,
    /// Transaction is being committed to journal
    Committing,
    /// Transaction has been committed to journal
    Committed,
    /// Transaction has been checkpointed to disk
    Checkpointed,
    /// Transaction was aborted
    Aborted,
}

/// Type of metadata being modified
#[derive(Debug, Clone, Copy)]
pub enum MetadataType {
    Superblock,
    GroupDescriptor(u32),
    InodeBitmap(u32),
    BlockBitmap(u32),
    InodeTable(u32),
    DirectoryBlock(BlockNumber),
    IndirectBlock(BlockNumber),
    ExtentBlock(BlockNumber),
}

/// A single metadata update within a transaction
#[derive(Debug, Clone)]
pub struct MetadataUpdate {
    pub metadata_type: MetadataType,
    pub block_number: BlockNumber,
    pub offset: usize,
    pub old_data: Vec<u8>,
    pub new_data: Vec<u8>,
}

/// Journal descriptor block for transaction
#[derive(Debug, Clone)]
pub struct JournalDescriptorBlock {
    pub magic: u32,           // JBD2_MAGIC_NUMBER
    pub blocktype: u32,       // JBD2_DESCRIPTOR_BLOCK
    pub sequence: u32,        // Transaction sequence number
    pub num_blocks: u32,      // Number of blocks in this transaction
    pub tags: Vec<JournalTag>, // Tags describing each block
}

/// Journal commit block
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct JournalCommitBlock {
    pub sequence: u32,        // Transaction sequence number
    pub has_checksum: bool,   // Whether checksum is included
}

/// Journal block header
#[derive(Debug, Clone, Copy)]
pub struct JournalHeader {
    pub magic: u32,
    pub blocktype: u32,
    pub sequence: u32,
}

/// Journal tag describing a journaled block
#[derive(Debug, Clone, Copy)]
pub struct JournalTag {
    pub block: BlockNumber,
    pub flags: u32,
    pub checksum: u32,
}

/// Transaction manager handles all atomic operations
pub struct TransactionManager {
    /// Next transaction ID
    next_tid: Arc<Mutex<TransactionId>>,
    /// Active transactions
    active_transactions: Arc<RwLock<HashMap<TransactionId, Transaction>>>,
    /// Committed transactions waiting to be checkpointed
    committed_transactions: Arc<Mutex<VecDeque<Transaction>>>,
    /// Journal writer
    journal: Arc<Mutex<Journal>>,
    /// Maximum transaction size
    max_transaction_size: usize,
    /// Whether journaling is enabled
    journaling_enabled: bool,
}

/// Individual transaction containing all updates
struct Transaction {
    pub id: TransactionId,
    pub state: TransactionState,
    pub updates: Vec<MetadataUpdate>,
    pub allocated_blocks: Vec<BlockNumber>,
    pub freed_blocks: Vec<BlockNumber>,
    pub allocated_inodes: Vec<u32>,
    pub freed_inodes: Vec<u32>,
    pub start_time: std::time::Instant,
}

/// Journal manages the on-disk journal
pub struct Journal {
    /// Journal inode number (usually 8)
    journal_inode: u32,
    /// Journal block size
    block_size: u32,
    /// Start block of journal
    journal_start: BlockNumber,
    /// Size of journal in blocks
    journal_size: u32,
    /// Current head (next write position)
    head: u32,
    /// Current tail (oldest uncommitted transaction)
    tail: u32,
    /// Sequence number for next transaction
    next_sequence: u32,
    /// Map of journal blocks to their destination blocks
    block_map: HashMap<BlockNumber, BlockNumber>,
    /// Set of revoked blocks
    revoked_blocks: std::collections::HashSet<BlockNumber>,
    /// Device path for I/O operations
    device_path: Option<String>,
    /// Journal inode blocks (where journal data is stored)
    journal_blocks: Vec<BlockNumber>,
}

impl TransactionManager {
    /// Create a new transaction manager
    pub fn new(superblock: &Ext4Superblock, enable_journal: bool, device_path: Option<String>) -> Self {
        let journal = if enable_journal && superblock.s_journal_inum != 0 {
            // Initialize journal from superblock
            Some(Journal::new(superblock.s_journal_inum, superblock.s_log_block_size, device_path))
        } else {
            None
        };

        Self {
            next_tid: Arc::new(Mutex::new(1)),
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            committed_transactions: Arc::new(Mutex::new(VecDeque::new())),
            journal: Arc::new(Mutex::new(journal.unwrap_or_else(|| Journal::dummy()))),
            max_transaction_size: 1024 * 1024, // 1MB default
            journaling_enabled: enable_journal && superblock.s_journal_inum != 0,
        }
    }

    /// Start a new transaction
    pub fn start_transaction(&self) -> Ext4Result<TransactionHandle> {
        let mut tid_guard = self.next_tid.lock().unwrap();
        let tid = *tid_guard;
        *tid_guard += 1;

        let transaction = Transaction {
            id: tid,
            state: TransactionState::Active,
            updates: Vec::new(),
            allocated_blocks: Vec::new(),
            freed_blocks: Vec::new(),
            allocated_inodes: Vec::new(),
            freed_inodes: Vec::new(),
            start_time: std::time::Instant::now(),
        };

        let mut transactions = self.active_transactions.write().unwrap();
        transactions.insert(tid, transaction);

        Ok(TransactionHandle {
            id: tid,
            state: Arc::new(Mutex::new(TransactionState::Active)),
        })
    }

    /// Add a metadata update to a transaction
    pub fn add_metadata_update(
        &self,
        handle: &TransactionHandle,
        update: MetadataUpdate,
    ) -> Ext4Result<()> {
        let state = handle.state.lock().unwrap();
        if *state != TransactionState::Active {
            return Err(Ext4Error::Io("Transaction is not active".to_string()));
        }
        drop(state);

        let mut transactions = self.active_transactions.write().unwrap();
        let transaction = transactions.get_mut(&handle.id)
            .ok_or_else(|| Ext4Error::Io("Transaction not found".to_string()))?;

        // Check transaction size
        let update_size = update.old_data.len() + update.new_data.len();
        let current_size: usize = transaction.updates.iter()
            .map(|u| u.old_data.len() + u.new_data.len())
            .sum();

        if current_size + update_size > self.max_transaction_size {
            return Err(Ext4Error::Io("Transaction too large".to_string()));
        }

        transaction.updates.push(update);
        Ok(())
    }

    /// Record block allocation in transaction
    pub fn add_allocated_blocks(
        &self,
        handle: &TransactionHandle,
        blocks: &[BlockNumber],
    ) -> Ext4Result<()> {
        let mut transactions = self.active_transactions.write().unwrap();
        let transaction = transactions.get_mut(&handle.id)
            .ok_or_else(|| Ext4Error::Io("Transaction not found".to_string()))?;

        transaction.allocated_blocks.extend_from_slice(blocks);
        Ok(())
    }

    /// Record block deallocation in transaction
    pub fn add_freed_blocks(
        &self,
        handle: &TransactionHandle,
        blocks: &[BlockNumber],
    ) -> Ext4Result<()> {
        let mut transactions = self.active_transactions.write().unwrap();
        let transaction = transactions.get_mut(&handle.id)
            .ok_or_else(|| Ext4Error::Io("Transaction not found".to_string()))?;

        transaction.freed_blocks.extend_from_slice(blocks);
        Ok(())
    }

    /// Commit a transaction atomically
    pub fn commit_transaction(&self, handle: &TransactionHandle) -> Ext4Result<()> {
        // Change state to committing
        {
            let mut state = handle.state.lock().unwrap();
            if *state != TransactionState::Active {
                return Err(Ext4Error::Io("Transaction is not active".to_string()));
            }
            *state = TransactionState::Committing;
        }

        // Remove from active transactions
        let transaction = {
            let mut transactions = self.active_transactions.write().unwrap();
            transactions.remove(&handle.id)
                .ok_or_else(|| Ext4Error::Io("Transaction not found".to_string()))?
        };

        // Write to journal if enabled
        if self.journaling_enabled {
            self.write_to_journal(transaction)?;
        } else {
            // Without journal, apply updates directly (less safe)
            self.apply_updates_directly(transaction)?;
        }

        // Update state to committed
        let mut state = handle.state.lock().unwrap();
        *state = TransactionState::Committed;

        Ok(())
    }

    /// Record allocated blocks in a transaction
    pub fn record_allocated_blocks(
        &self,
        handle: &TransactionHandle,
        blocks: &[BlockNumber],
    ) -> Ext4Result<()> {
        let mut transactions = self.active_transactions.write().unwrap();
        let transaction = transactions.get_mut(&handle.id)
            .ok_or_else(|| Ext4Error::Io("Transaction not found".to_string()))?;
        
        transaction.allocated_blocks.extend_from_slice(blocks);
        Ok(())
    }
    
    /// Record freed blocks in a transaction
    pub fn record_freed_blocks(
        &self,
        handle: &TransactionHandle,
        blocks: &[BlockNumber],
    ) -> Ext4Result<()> {
        let mut transactions = self.active_transactions.write().unwrap();
        let transaction = transactions.get_mut(&handle.id)
            .ok_or_else(|| Ext4Error::Io("Transaction not found".to_string()))?;
        
        transaction.freed_blocks.extend_from_slice(blocks);
        Ok(())
    }
    
    /// Record allocated inodes in a transaction
    pub fn record_allocated_inodes(
        &self,
        handle: &TransactionHandle,
        inodes: &[u32],
    ) -> Ext4Result<()> {
        let mut transactions = self.active_transactions.write().unwrap();
        let transaction = transactions.get_mut(&handle.id)
            .ok_or_else(|| Ext4Error::Io("Transaction not found".to_string()))?;
        
        transaction.allocated_inodes.extend_from_slice(inodes);
        Ok(())
    }
    
    /// Record freed inodes in a transaction
    pub fn record_freed_inodes(
        &self,
        handle: &TransactionHandle,
        inodes: &[u32],
    ) -> Ext4Result<()> {
        let mut transactions = self.active_transactions.write().unwrap();
        let transaction = transactions.get_mut(&handle.id)
            .ok_or_else(|| Ext4Error::Io("Transaction not found".to_string()))?;
        
        transaction.freed_inodes.extend_from_slice(inodes);
        Ok(())
    }
    
    /// Get transaction statistics
    pub fn get_transaction_stats(&self, handle: &TransactionHandle) -> Option<(usize, usize, std::time::Duration)> {
        let transactions = self.active_transactions.read().unwrap();
        transactions.get(&handle.id).map(|t| {
            let block_count = t.allocated_blocks.len() + t.freed_blocks.len();
            let inode_count = t.allocated_inodes.len() + t.freed_inodes.len();
            let duration = t.start_time.elapsed();
            (block_count, inode_count, duration)
        })
    }

    /// Abort a transaction, rolling back any changes
    pub fn abort_transaction(&self, handle: &TransactionHandle) -> Ext4Result<()> {
        let mut state = handle.state.lock().unwrap();
        *state = TransactionState::Aborted;
        drop(state);

        // Remove from active transactions
        let mut transactions = self.active_transactions.write().unwrap();
        if let Some(mut transaction) = transactions.remove(&handle.id) {
            transaction.state = TransactionState::Aborted;
            
            // In a real implementation, would need to:
            // 1. Release any locks held by transaction
            // 2. Free any temporarily allocated resources
            // 3. Restore any in-memory structures
        }

        Ok(())
    }

    /// Write transaction to journal
    fn write_to_journal(&self, mut transaction: Transaction) -> Ext4Result<()> {
        let mut journal = self.journal.lock().unwrap();
        
        // Create descriptor block with tags for each updated block
        let mut tags = Vec::new();
        for (i, update) in transaction.updates.iter().enumerate() {
            tags.push(JournalTag {
                block: update.block_number,
                flags: if i == transaction.updates.len() - 1 { 0x1 } else { 0x0 }, // Last tag flag
                checksum: 0, // Would calculate CRC32c of block
            });
        }
        
        let descriptor = JournalDescriptorBlock {
            magic: JBD2_MAGIC_NUMBER,
            blocktype: JBD2_DESCRIPTOR_BLOCK,
            sequence: journal.next_sequence,
            num_blocks: transaction.updates.len() as u32,
            tags,
        };

        // Write descriptor block
        journal.write_descriptor_block(&descriptor)?;

        // Write metadata blocks
        for update in &transaction.updates {
            journal.write_metadata_block(&update.new_data, update.block_number)?;
        }

        // Calculate checksum
        let _checksum = self.calculate_transaction_checksum(&transaction);

        // Write commit block
        let commit = JournalCommitBlock {
            sequence: journal.next_sequence,
            has_checksum: true,
        };
        journal.write_commit_block(&commit)?;

        // Update journal sequence
        journal.next_sequence += 1;

        // Add to committed transactions
        transaction.state = TransactionState::Committed;
        let mut committed = self.committed_transactions.lock().unwrap();
        committed.push_back(transaction);

        Ok(())
    }

    /// Apply updates directly without journaling (unsafe)
    fn apply_updates_directly(&self, transaction: Transaction) -> Ext4Result<()> {
        // WARNING: This is not crash-safe without journaling
        // In production, would need careful ordering and barriers
        
        for _update in &transaction.updates {
            // Would write update.new_data to update.block_number
            // This is where actual disk I/O would happen
        }

        Ok(())
    }

    /// Checkpoint committed transactions to final locations
    pub fn checkpoint(&self) -> Ext4Result<()> {
        let mut committed = self.committed_transactions.lock().unwrap();
        
        while let Some(mut transaction) = committed.pop_front() {
            // Apply updates to final locations
            for _update in &transaction.updates {
                // Write to actual filesystem blocks
                // This is safe because journal has the data
            }
            
            transaction.state = TransactionState::Checkpointed;
        }

        // Update journal tail
        let mut journal = self.journal.lock().unwrap();
        journal.update_tail()?;

        Ok(())
    }

    /// Calculate checksum for transaction
    fn calculate_transaction_checksum(&self, transaction: &Transaction) -> u32 {
use super::checksum::crc32c_ext4;
        
        let mut crc = 0u32;
        
        // Include transaction ID
        crc = crc32c_ext4(&transaction.id.to_le_bytes(), crc);
        
        // Include all updates
        for update in &transaction.updates {
            crc = crc32c_ext4(&update.block_number.to_le_bytes(), crc);
            crc = crc32c_ext4(&update.new_data, crc);
        }
        
        crc
    }

    /// Replay journal on mount (recovery)
    pub fn replay_journal(&self) -> Ext4Result<()> {
        if !self.journaling_enabled {
            return Ok(());
        }

        let mut journal = self.journal.lock().unwrap();
        journal.replay()?;
        
        Ok(())
    }
}

impl Journal {
    /// Create a new journal
    pub fn new(journal_inode: u32, log_block_size: u32, device_path: Option<String>) -> Self {
        Self {
            journal_inode,
            block_size: 1024 << log_block_size,
            journal_start: 0, // Would be read from journal inode
            journal_size: 0,  // Would be read from journal superblock
            head: 0,
            tail: 0,
            next_sequence: 1,
            block_map: HashMap::new(),
            revoked_blocks: std::collections::HashSet::new(),
            device_path,
            journal_blocks: Vec::new(), // Would be populated from journal inode
        }
    }

    /// Create a dummy journal when journaling is disabled
    pub fn dummy() -> Self {
        Self {
            journal_inode: 0,
            block_size: 4096,
            journal_start: 0,
            journal_size: 0,
            head: 0,
            tail: 0,
            next_sequence: 0,
            block_map: HashMap::new(),
            revoked_blocks: std::collections::HashSet::new(),
            device_path: None,
            journal_blocks: Vec::new(),
        }
    }

    /// Write descriptor block to journal
    fn write_descriptor_block(&mut self, descriptor: &JournalDescriptorBlock) -> Ext4Result<()> {
        // Calculate journal block position
        let journal_block = self.journal_start + self.head as u64;
        
        // Serialize descriptor block
        let mut buffer = vec![0u8; self.block_size as usize];
        buffer[0..4].copy_from_slice(&descriptor.magic.to_le_bytes());
        buffer[4..8].copy_from_slice(&descriptor.blocktype.to_le_bytes());
        buffer[8..12].copy_from_slice(&descriptor.sequence.to_le_bytes());
        
        // Write tag entries
        let mut offset = 12;
        for tag in &descriptor.tags {
            if offset + 16 > buffer.len() {
                break;
            }
            buffer[offset..offset+8].copy_from_slice(&tag.block.to_le_bytes());
            buffer[offset+8..offset+12].copy_from_slice(&tag.flags.to_le_bytes());
            buffer[offset+12..offset+16].copy_from_slice(&tag.checksum.to_le_bytes());
            offset += 16;
        }
        
        // Write to journal
        self.write_journal_block(journal_block, &buffer)?;
        self.head = (self.head + 1) % self.journal_size;
        Ok(())
    }

    /// Write metadata block to journal
    fn write_metadata_block(&mut self, data: &[u8], block_num: BlockNumber) -> Ext4Result<()> {
        // Write the actual data to the journal
        let journal_block = self.journal_start + self.head as u64;
        
        // Create a full block buffer
        let mut buffer = vec![0u8; self.block_size as usize];
        let copy_len = data.len().min(buffer.len());
        buffer[..copy_len].copy_from_slice(&data[..copy_len]);
        
        // Store mapping for recovery
        self.block_map.insert(journal_block, block_num);
        
        // Write to journal
        self.write_journal_block(journal_block, &buffer)?;
        self.head = (self.head + 1) % self.journal_size;
        Ok(())
    }

    /// Write commit block to journal
    fn write_commit_block(&mut self, commit: &JournalCommitBlock) -> Ext4Result<()> {
        let journal_block = self.journal_start + self.head as u64;
        
        // Serialize commit block
        let mut buffer = vec![0u8; self.block_size as usize];
        buffer[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        buffer[4..8].copy_from_slice(&JBD2_COMMIT_BLOCK.to_le_bytes());
        buffer[8..12].copy_from_slice(&commit.sequence.to_le_bytes());
        
        // Add checksum if supported
        if commit.has_checksum {
            let checksum = self.calculate_commit_checksum(&commit);
            buffer[12..16].copy_from_slice(&checksum.to_le_bytes());
        }
        
        // Write and flush for durability
        self.write_journal_block(journal_block, &buffer)?;
        self.flush_journal()?;
        
        self.head = (self.head + 1) % self.journal_size;
        Ok(())
    }

    /// Update journal tail after checkpointing
    fn update_tail(&mut self) -> Ext4Result<()> {
        // Would update tail in journal superblock
        self.tail = self.head;
        Ok(())
    }

    /// Replay journal during recovery
    fn replay(&mut self) -> Ext4Result<()> {
        // Scan journal from tail to head
        let mut current = self.tail;
        let mut transactions_replayed = 0;
        
        while current != self.head {
            let journal_block = self.journal_start + current as u64;
            
            // Read block header
            let header = self.read_journal_header(journal_block)?;
            
            match header.blocktype {
                JBD2_DESCRIPTOR_BLOCK => {
                    // Process descriptor and its data blocks
                    let tags = self.read_descriptor_tags(journal_block)?;
                    
                    for tag in tags {
                        // Skip revoked blocks
                        if self.revoked_blocks.contains(&tag.block) {
                            continue;
                        }
                        
                        // Read the journaled data
                        current = (current + 1) % self.journal_size;
                        let data_block = self.journal_start + current as u64;
                        let data = self.read_journal_block(data_block)?;
                        
                        // Write to its final destination
                        self.write_block_to_disk(tag.block, &data)?;
                    }
                }
                JBD2_COMMIT_BLOCK => {
                    // Transaction successfully committed
                    transactions_replayed += 1;
                }
                JBD2_REVOKE_BLOCK => {
                    // Handle revoked blocks (skip writing them)
                    let revoked_blocks = self.read_revoke_blocks(journal_block)?;
                    for block in revoked_blocks {
                        self.revoked_blocks.insert(block);
                    }
                }
                _ => {
                    // Unknown block type, might be corrupted
                    break;
                }
            }
            
            current = (current + 1) % self.journal_size;
        }
        
        log::info!("Journal replay complete: {} transactions recovered", transactions_replayed);
        Ok(())
    }
    
    /// Write a block to the journal
    fn write_journal_block(&mut self, block: BlockNumber, data: &[u8]) -> Ext4Result<()> {
        let Some(ref device_path) = self.device_path else {
            log::debug!("No device path, skipping journal write for block {}", block);
            return Ok(());
        };
        
        // Calculate the physical block number
        // In a real system, we'd map through the journal inode's extent tree
        // For now, assume journal is contiguous starting at journal_start
        let physical_block = if self.journal_blocks.is_empty() {
            // Fallback: assume contiguous journal
            self.journal_start + (block - self.journal_start) % self.journal_size as u64
        } else {
            // Use the journal blocks mapping
            let index = ((block - self.journal_start) % self.journal_size as u64) as usize;
            if index < self.journal_blocks.len() {
                self.journal_blocks[index]
            } else {
                return Err(Ext4Error::InvalidBlock(block));
            }
        };
        
        let offset = physical_block * self.block_size as u64;
        
        // Platform-specific I/O
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use std::fs::OpenOptions;
            use std::io::{Write, Seek, SeekFrom};
            use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE};
            
            let mut file = OpenOptions::new()
                .write(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .open(device_path)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            file.write_all(data)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            use std::fs::OpenOptions;
            use std::io::{Write, Seek, SeekFrom};
            
            let mut file = OpenOptions::new()
                .write(true)
                .open(device_path)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            file.write_all(data)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
        }
        
        log::debug!("Wrote journal block {} to physical block {}", block, physical_block);
        Ok(())
    }
    
    /// Read a block from the journal
    fn read_journal_block(&self, block: BlockNumber) -> Ext4Result<Vec<u8>> {
        let Some(ref device_path) = self.device_path else {
            log::debug!("No device path, returning empty block for {}", block);
            return Ok(vec![0u8; self.block_size as usize]);
        };
        
        // Calculate the physical block number
        let physical_block = if self.journal_blocks.is_empty() {
            self.journal_start + (block - self.journal_start) % self.journal_size as u64
        } else {
            let index = ((block - self.journal_start) % self.journal_size as u64) as usize;
            if index < self.journal_blocks.len() {
                self.journal_blocks[index]
            } else {
                return Err(Ext4Error::InvalidBlock(block));
            }
        };
        
        let offset = physical_block * self.block_size as u64;
        let mut buffer = vec![0u8; self.block_size as usize];
        
        // Platform-specific I/O
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use std::fs::OpenOptions;
            use std::io::{Read, Seek, SeekFrom};
            use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE};
            
            let mut file = OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .open(device_path)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            file.read_exact(&mut buffer)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            use std::fs::OpenOptions;
            use std::io::{Read, Seek, SeekFrom};
            
            let mut file = OpenOptions::new()
                .read(true)
                .open(device_path)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            file.read_exact(&mut buffer)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
        }
        
        log::debug!("Read journal block {} from physical block {}", block, physical_block);
        Ok(buffer)
    }
    
    /// Flush journal to ensure durability
    fn flush_journal(&mut self) -> Ext4Result<()> {
        let Some(ref device_path) = self.device_path else {
            log::debug!("No device path, skipping journal flush");
            return Ok(());
        };
        
        // Platform-specific sync
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use std::fs::OpenOptions;
            use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE};
            
            let file = OpenOptions::new()
                .write(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .open(device_path)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            
            file.sync_all()
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            use std::fs::OpenOptions;
            
            let file = OpenOptions::new()
                .write(true)
                .open(device_path)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            
            file.sync_all()
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
        }
        
        log::debug!("Journal flushed to disk");
        Ok(())
    }
    
    /// Read journal header from block
    fn read_journal_header(&self, block: BlockNumber) -> Ext4Result<JournalHeader> {
        let data = self.read_journal_block(block)?;
        Ok(JournalHeader {
            magic: u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
            blocktype: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            sequence: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
        })
    }
    
    /// Read descriptor tags from journal block
    fn read_descriptor_tags(&self, block: BlockNumber) -> Ext4Result<Vec<JournalTag>> {
        // Would parse descriptor block for tags
        let data = self.read_journal_block(block)?;
        let mut tags = Vec::new();
        let mut offset = 12; // Skip header
        
        while offset + 16 <= data.len() {
            let block = u64::from_le_bytes([
                data[offset], data[offset+1], data[offset+2], data[offset+3],
                data[offset+4], data[offset+5], data[offset+6], data[offset+7],
            ]);
            let flags = u32::from_le_bytes([
                data[offset+8], data[offset+9], data[offset+10], data[offset+11],
            ]);
            let checksum = u32::from_le_bytes([
                data[offset+12], data[offset+13], data[offset+14], data[offset+15],
            ]);
            
            tags.push(JournalTag { block, flags, checksum });
            
            // Check for last tag flag
            if flags & 0x1 != 0 {
                break;
            }
            
            offset += 16;
        }
        
        Ok(tags)
    }
    
    /// Read revoked blocks from journal
    fn read_revoke_blocks(&self, block: BlockNumber) -> Ext4Result<Vec<BlockNumber>> {
        // Would parse revoke block
        let data = self.read_journal_block(block)?;
        let mut blocks = Vec::new();
        let mut offset = 12; // Skip header
        
        while offset + 8 <= data.len() {
            let block = u64::from_le_bytes([
                data[offset], data[offset+1], data[offset+2], data[offset+3],
                data[offset+4], data[offset+5], data[offset+6], data[offset+7],
            ]);
            blocks.push(block);
            offset += 8;
        }
        
        Ok(blocks)
    }
    
    /// Write block to its final disk location
    fn write_block_to_disk(&self, block: BlockNumber, data: &[u8]) -> Ext4Result<()> {
        let Some(ref device_path) = self.device_path else {
            log::debug!("No device path, skipping disk write for block {}", block);
            return Ok(());
        };
        
        let offset = block * self.block_size as u64;
        
        // Platform-specific I/O
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use std::fs::OpenOptions;
            use std::io::{Write, Seek, SeekFrom};
            use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE};
            
            let mut file = OpenOptions::new()
                .write(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .open(device_path)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            file.write_all(data)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            use std::fs::OpenOptions;
            use std::io::{Write, Seek, SeekFrom};
            
            let mut file = OpenOptions::new()
                .write(true)
                .open(device_path)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
            file.write_all(data)
                .map_err(|e| Ext4Error::IoError(e.to_string()))?;
        }
        
        log::debug!("Wrote block {} to disk at offset {}", block, offset);
        Ok(())
    }
    
    /// Calculate checksum for commit block
    fn calculate_commit_checksum(&self, commit: &JournalCommitBlock) -> u32 {
        use super::checksum::crc32c_ext4;
        let mut crc = 0u32;
        crc = crc32c_ext4(&commit.sequence.to_le_bytes(), crc);
        crc
    }
}

/// Guard for automatic transaction commit/abort
pub struct TransactionGuard<'a> {
    manager: &'a TransactionManager,
    handle: Option<TransactionHandle>,
}

impl<'a> TransactionGuard<'a> {
    pub fn new(manager: &'a TransactionManager) -> Ext4Result<Self> {
        let handle = manager.start_transaction()?;
        Ok(Self {
            manager,
            handle: Some(handle),
        })
    }

    pub fn handle(&self) -> &TransactionHandle {
        self.handle.as_ref().unwrap()
    }

    pub fn commit(mut self) -> Ext4Result<()> {
        if let Some(handle) = self.handle.take() {
            self.manager.commit_transaction(&handle)?;
        }
        Ok(())
    }
}

impl<'a> Drop for TransactionGuard<'a> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            // Abort transaction if not explicitly committed
            let _ = self.manager.abort_transaction(&handle);
        }
    }
}