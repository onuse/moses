// NTFS $LogFile Writer
// Handles writing log records and managing transactions

use moses_core::MosesError;
use super::{
    structures::*,
    lsn::{Lsn, LsnManager},
    LogOperation, TransactionState,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// Transaction information
struct Transaction {
    /// Transaction ID
    id: u32,
    /// Transaction state
    state: TransactionState,
    /// First LSN in transaction
    first_lsn: Lsn,
    /// Previous LSN in transaction
    prev_lsn: Lsn,
    /// Undo next LSN
    undo_next_lsn: Lsn,
}

/// LogFile writer
pub struct LogFileWriter {
    /// LSN manager
    lsn_manager: Arc<LsnManager>,
    /// Active transactions
    transactions: Arc<RwLock<HashMap<u32, Transaction>>>,
    /// Next transaction ID
    next_transaction_id: Arc<Mutex<u32>>,
    /// Log buffer (would write to $LogFile in practice)
    log_buffer: Arc<Mutex<Vec<u8>>>,
    /// Log file size
    log_size: u64,
    /// Page size
    page_size: u32,
    /// Current restart area
    restart_area: Arc<RwLock<RestartAreaData>>,
}

impl LogFileWriter {
    /// Create a new LogFile writer
    pub fn new(log_size: u64, page_size: u32) -> Self {
        let restart_area_size = page_size as u64 * 2; // Two restart areas
        
        let restart_area = RestartAreaData {
            current_lsn: Lsn::INVALID,
            log_clients: 1,
            client_free_list: 0xFFFF,
            client_in_use_list: 0,
            flags: 0,
            seq_number_bits: 32,
            restart_area_length: std::mem::size_of::<RestartAreaData>() as u16,
            client_array_offset: std::mem::size_of::<RestartAreaData>() as u16,
            file_size: log_size,
            last_lsn_data_length: 0,
            log_record_header_length: std::mem::size_of::<LogRecordHeader>() as u16,
            log_page_data_offset: std::mem::size_of::<LogPageHeader>() as u16,
            restart_log_open_count: 1,
            reserved: 0,
        };
        
        Self {
            lsn_manager: Arc::new(LsnManager::new(log_size, restart_area_size)),
            transactions: Arc::new(RwLock::new(HashMap::new())),
            next_transaction_id: Arc::new(Mutex::new(1)),
            log_buffer: Arc::new(Mutex::new(Vec::with_capacity(log_size as usize))),
            log_size,
            page_size,
            restart_area: Arc::new(RwLock::new(restart_area)),
        }
    }
    
    /// Start a new transaction
    pub fn begin_transaction(&self) -> Result<u32, MosesError> {
        let mut next_id = self.next_transaction_id.lock().unwrap();
        let transaction_id = *next_id;
        *next_id += 1;
        
        let transaction = Transaction {
            id: transaction_id,
            state: TransactionState::Active,
            first_lsn: Lsn::INVALID,
            prev_lsn: Lsn::INVALID,
            undo_next_lsn: Lsn::INVALID,
        };
        
        let mut transactions = self.transactions.write().unwrap();
        transactions.insert(transaction_id, transaction);
        
        Ok(transaction_id)
    }
    
    /// Write a log record
    pub fn write_record(
        &self,
        transaction_id: u32,
        operation: LogOperation,
        target_vcn: u64,
        target_attribute: u16,
        redo_data: &[u8],
        undo_data: &[u8],
    ) -> Result<Lsn, MosesError> {
        // Get transaction
        let mut transactions = self.transactions.write().unwrap();
        let transaction = transactions.get_mut(&transaction_id)
            .ok_or_else(|| MosesError::Other("Invalid transaction ID".to_string()))?;
        
        if transaction.state != TransactionState::Active {
            return Err(MosesError::Other("Transaction not active".to_string()));
        }
        
        // Calculate record size
        let header_size = std::mem::size_of::<LogRecordHeader>();
        let total_size = header_size + redo_data.len() + undo_data.len();
        
        // Allocate LSN
        let lsn = self.lsn_manager.allocate(total_size as u64);
        
        // Create log record header
        let header = LogRecordHeader {
            this_lsn: lsn,
            prev_lsn: transaction.prev_lsn,
            client_undo_next_lsn: transaction.undo_next_lsn,
            client_data_length: (redo_data.len() + undo_data.len()) as u32,
            client_id: 0,  // NTFS client
            record_type: LOG_RECORD_NORMAL,
            transaction_id,
            flags: 0,
            reserved: [0; 3],
            redo_operation: operation as u16,
            undo_operation: self.get_undo_operation(operation) as u16,
            redo_offset: header_size as u16,
            redo_length: redo_data.len() as u16,
            undo_offset: (header_size + redo_data.len()) as u16,
            undo_length: undo_data.len() as u16,
            target_attribute,
            lcn_list_size: 0,
            record_offset: 0,
            attribute_offset: 0,
            cluster_block_offset: 0,
            reserved2: 0,
            target_vcn,
            reserved3: 0,
        };
        
        // Update transaction tracking
        if !transaction.first_lsn.is_valid() {
            transaction.first_lsn = lsn;
        }
        transaction.prev_lsn = lsn;
        transaction.undo_next_lsn = lsn;
        
        // Write to log buffer (simplified - would write to actual $LogFile)
        self.write_to_buffer(&header, redo_data, undo_data)?;
        
        // Update restart area
        let mut restart_area = self.restart_area.write().unwrap();
        restart_area.current_lsn = lsn;
        
        Ok(lsn)
    }
    
    /// Commit a transaction
    pub fn commit_transaction(&self, transaction_id: u32) -> Result<(), MosesError> {
        let mut transactions = self.transactions.write().unwrap();
        let transaction = transactions.get_mut(&transaction_id)
            .ok_or_else(|| MosesError::Other("Invalid transaction ID".to_string()))?;
        
        if transaction.state != TransactionState::Active {
            return Err(MosesError::Other("Transaction not active".to_string()));
        }
        
        // Write commit record
        let commit_lsn = self.write_transaction_record(
            transaction_id,
            LogOperation::CommitTransaction,
            transaction.prev_lsn,
        )?;
        
        // Update transaction state
        transaction.state = TransactionState::Committed;
        transaction.prev_lsn = commit_lsn;
        
        Ok(())
    }
    
    /// Abort a transaction
    pub fn abort_transaction(&self, transaction_id: u32) -> Result<(), MosesError> {
        let mut transactions = self.transactions.write().unwrap();
        let transaction = transactions.get_mut(&transaction_id)
            .ok_or_else(|| MosesError::Other("Invalid transaction ID".to_string()))?;
        
        if transaction.state != TransactionState::Active {
            return Err(MosesError::Other("Transaction not active".to_string()));
        }
        
        // Write forget record
        let forget_lsn = self.write_transaction_record(
            transaction_id,
            LogOperation::ForgetTransaction,
            transaction.prev_lsn,
        )?;
        
        // Update transaction state
        transaction.state = TransactionState::Aborted;
        transaction.prev_lsn = forget_lsn;
        
        // TODO: Write compensation log records for undo
        
        Ok(())
    }
    
    /// Write a checkpoint
    pub fn write_checkpoint(&self) -> Result<Lsn, MosesError> {
        // Create checkpoint record
        let checkpoint = CheckpointRecord {
            virtual_clock: 0,
            allocation_list: Lsn::INVALID,
            deallocation_list: Lsn::INVALID,
            transaction_table: Lsn::INVALID,
            dirty_page_table: Lsn::INVALID,
            attribute_table: Lsn::INVALID,
            current_target_attribute: 0,
            transaction_counter: *self.next_transaction_id.lock().unwrap() as u64,
            unknown: [0; 24],
        };
        
        let checkpoint_size = std::mem::size_of::<CheckpointRecord>();
        let lsn = self.lsn_manager.allocate(checkpoint_size as u64);
        
        // Write checkpoint to buffer
        let checkpoint_bytes = unsafe {
            std::slice::from_raw_parts(
                &checkpoint as *const _ as *const u8,
                checkpoint_size
            )
        };
        
        let mut buffer = self.log_buffer.lock().unwrap();
        buffer.extend_from_slice(checkpoint_bytes);
        
        // Update restart area
        let mut restart_area = self.restart_area.write().unwrap();
        restart_area.current_lsn = lsn;
        
        log::info!("Checkpoint written at {}", lsn);
        
        Ok(lsn)
    }
    
    /// Get undo operation for a given redo operation
    fn get_undo_operation(&self, redo_op: LogOperation) -> LogOperation {
        match redo_op {
            LogOperation::SetAttributeValue => LogOperation::SetAttributeValue,
            LogOperation::AddAttribute => LogOperation::DeleteAttribute,
            LogOperation::DeleteAttribute => LogOperation::AddAttribute,
            LogOperation::UpdateResidentValue => LogOperation::UpdateResidentValue,
            LogOperation::UpdateNonResidentValue => LogOperation::UpdateNonResidentValue,
            LogOperation::AddIndexEntryRoot => LogOperation::DeleteIndexEntryRoot,
            LogOperation::DeleteIndexEntryRoot => LogOperation::AddIndexEntryRoot,
            LogOperation::AddIndexEntryAllocation => LogOperation::DeleteIndexEntryAllocation,
            LogOperation::DeleteIndexEntryAllocation => LogOperation::AddIndexEntryAllocation,
            LogOperation::SetBitsInBitmap => LogOperation::ClearBitsInBitmap,
            LogOperation::ClearBitsInBitmap => LogOperation::SetBitsInBitmap,
            _ => LogOperation::Noop,
        }
    }
    
    /// Write transaction control record
    fn write_transaction_record(
        &self,
        transaction_id: u32,
        operation: LogOperation,
        prev_lsn: Lsn,
    ) -> Result<Lsn, MosesError> {
        let header_size = std::mem::size_of::<LogRecordHeader>();
        let lsn = self.lsn_manager.allocate(header_size as u64);
        
        let header = LogRecordHeader {
            this_lsn: lsn,
            prev_lsn,
            client_undo_next_lsn: Lsn::INVALID,
            client_data_length: 0,
            client_id: 0,
            record_type: LOG_RECORD_NORMAL,
            transaction_id,
            flags: 0,
            reserved: [0; 3],
            redo_operation: operation as u16,
            undo_operation: LogOperation::Noop as u16,
            redo_offset: 0,
            redo_length: 0,
            undo_offset: 0,
            undo_length: 0,
            target_attribute: 0,
            lcn_list_size: 0,
            record_offset: 0,
            attribute_offset: 0,
            cluster_block_offset: 0,
            reserved2: 0,
            target_vcn: 0,
            reserved3: 0,
        };
        
        self.write_header_to_buffer(&header)?;
        
        Ok(lsn)
    }
    
    /// Write log record to buffer
    fn write_to_buffer(
        &self,
        header: &LogRecordHeader,
        redo_data: &[u8],
        undo_data: &[u8],
    ) -> Result<(), MosesError> {
        let mut buffer = self.log_buffer.lock().unwrap();
        
        // Write header
        let header_bytes = unsafe {
            std::slice::from_raw_parts(
                header as *const _ as *const u8,
                std::mem::size_of::<LogRecordHeader>()
            )
        };
        buffer.extend_from_slice(header_bytes);
        
        // Write redo data
        buffer.extend_from_slice(redo_data);
        
        // Write undo data
        buffer.extend_from_slice(undo_data);
        
        Ok(())
    }
    
    /// Write header to buffer
    fn write_header_to_buffer(&self, header: &LogRecordHeader) -> Result<(), MosesError> {
        let mut buffer = self.log_buffer.lock().unwrap();
        
        let header_bytes = unsafe {
            std::slice::from_raw_parts(
                header as *const _ as *const u8,
                std::mem::size_of::<LogRecordHeader>()
            )
        };
        buffer.extend_from_slice(header_bytes);
        
        Ok(())
    }
    
    /// Get current LSN
    pub fn current_lsn(&self) -> Lsn {
        self.lsn_manager.current_lsn()
    }
    
    /// Check if checkpoint is needed
    pub fn needs_checkpoint(&self) -> bool {
        // Check if we're running low on log space
        self.lsn_manager.needs_checkpoint(self.page_size as u64 * 100)
    }
}