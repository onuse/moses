// NTFS $LogFile Structures
// Defines on-disk structures for the NTFS transaction log

use super::lsn::Lsn;

/// Magic numbers
pub const RSTR_MAGIC: u32 = 0x52545352;  // "RSTR" - Restart area
pub const RCRD_MAGIC: u32 = 0x44524352;  // "RCRD" - Log record page
pub const CHKD_MAGIC: u32 = 0x444B4843;  // "CHKD" - Check disk

/// Restart area - stored at beginning of $LogFile
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RestartArea {
    /// Magic number "RSTR"
    pub magic: u32,
    /// Update sequence array offset
    pub usa_offset: u16,
    /// Update sequence array size
    pub usa_size: u16,
    /// Checkpoint LSN
    pub checkpoint_lsn: Lsn,
    /// System page size (bytes)
    pub system_page_size: u32,
    /// Log page size (bytes)
    pub log_page_size: u32,
    /// Restart area offset
    pub restart_area_offset: u16,
    /// Minor version
    pub minor_version: i16,
    /// Major version
    pub major_version: i16,
    /// Update sequence array
    pub usa: [u8; 0],  // Variable size
}

/// Restart area data
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RestartAreaData {
    /// Current LSN
    pub current_lsn: Lsn,
    /// Number of log clients
    pub log_clients: u16,
    /// Client free list
    pub client_free_list: u16,
    /// Client in use list
    pub client_in_use_list: u16,
    /// Flags
    pub flags: u16,
    /// Sequence number bits
    pub seq_number_bits: u32,
    /// Restart area length
    pub restart_area_length: u16,
    /// Client array offset
    pub client_array_offset: u16,
    /// File size
    pub file_size: u64,
    /// Last LSN data length
    pub last_lsn_data_length: u32,
    /// Log record header length
    pub log_record_header_length: u16,
    /// Log page data offset
    pub log_page_data_offset: u16,
    /// Restart log open count
    pub restart_log_open_count: u32,
    /// Reserved
    pub reserved: u32,
}

/// Log client record
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct LogClient {
    /// Oldest LSN
    pub oldest_lsn: Lsn,
    /// Client restart LSN
    pub client_restart_lsn: Lsn,
    /// Previous client
    pub prev_client: u16,
    /// Next client
    pub next_client: u16,
    /// Sequence number
    pub seq_number: u16,
    /// Reserved
    pub reserved: [u8; 6],
    /// Client name length
    pub client_name_length: u32,
    /// Client name (Unicode)
    pub client_name: [u16; 64],
}

/// Log record page header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct LogPageHeader {
    /// Magic number "RCRD"
    pub magic: u32,
    /// Update sequence array offset
    pub usa_offset: u16,
    /// Update sequence array size
    pub usa_size: u16,
    /// Last LSN on this page
    pub last_lsn: Lsn,
    /// Last end LSN on this page
    pub last_end_lsn: Lsn,
    /// Flags
    pub flags: u16,
    /// Page count (for multi-page records)
    pub page_count: u16,
    /// Page position (for multi-page records)
    pub page_position: u16,
    /// Next record offset
    pub next_record_offset: u16,
    /// Reserved
    pub reserved: [u8; 6],
}

/// Log record header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct LogRecordHeader {
    /// This LSN
    pub this_lsn: Lsn,
    /// Previous LSN
    pub prev_lsn: Lsn,
    /// Client undo next LSN
    pub client_undo_next_lsn: Lsn,
    /// Client data length
    pub client_data_length: u32,
    /// Client ID
    pub client_id: u16,
    /// Record type
    pub record_type: u32,
    /// Transaction ID
    pub transaction_id: u32,
    /// Flags
    pub flags: u16,
    /// Reserved
    pub reserved: [u16; 3],
    /// Redo operation
    pub redo_operation: u16,
    /// Undo operation
    pub undo_operation: u16,
    /// Redo offset
    pub redo_offset: u16,
    /// Redo length
    pub redo_length: u16,
    /// Undo offset
    pub undo_offset: u16,
    /// Undo length
    pub undo_length: u16,
    /// Target attribute
    pub target_attribute: u16,
    /// LCN (Logical Cluster Number) list size
    pub lcn_list_size: u16,
    /// Record offset
    pub record_offset: u16,
    /// Attribute offset
    pub attribute_offset: u16,
    /// Cluster block offset
    pub cluster_block_offset: u16,
    /// Reserved2
    pub reserved2: u16,
    /// Target VCN (Virtual Cluster Number)
    pub target_vcn: u64,
    /// Reserved3
    pub reserved3: u64,
}

/// Log record types
pub const LOG_RECORD_NORMAL: u32 = 0x0001;
pub const LOG_RECORD_CHECKPOINT: u32 = 0x0002;

/// Log record flags
pub const LOG_RECORD_MULTI_PAGE: u16 = 0x0001;

/// Checkpoint record
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CheckpointRecord {
    /// Virtual clock
    pub virtual_clock: u64,
    /// Allocation list
    pub allocation_list: Lsn,
    /// Deallocation list
    pub deallocation_list: Lsn,
    /// Transaction table
    pub transaction_table: Lsn,
    /// Dirty page table
    pub dirty_page_table: Lsn,
    /// Attribute table
    pub attribute_table: Lsn,
    /// Current target attribute
    pub current_target_attribute: u32,
    /// Transaction counter
    pub transaction_counter: u64,
    /// Unknown
    pub unknown: [u8; 24],
}

/// Open attribute entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct OpenAttributeEntry {
    /// Allocation list
    pub allocation_list: Lsn,
    /// Length of name
    pub name_length: u16,
    /// Attribute type
    pub attribute_type: u32,
    /// Unknown
    pub unknown: u32,
    /// File reference
    pub file_reference: u64,
    /// Unknown2
    pub unknown2: u64,
    /// Attribute name (Unicode)
    pub attribute_name: [u16; 0],  // Variable size
}

/// Dirty page entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DirtyPageEntry {
    /// Target attribute
    pub target_attribute: u32,
    /// Length of transfer
    pub transfer_length: u32,
    /// Number of LCNs
    pub lcn_count: u32,
    /// Reserved
    pub reserved: u32,
    /// VCN of dirty page
    pub vcn: u64,
    /// Oldest LSN
    pub oldest_lsn: Lsn,
    // LCN list follows in the actual structure
}

/// Transaction entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TransactionEntry {
    /// Transaction state
    pub transaction_state: u32,
    /// Reserved
    pub reserved: u32,
    /// First LSN
    pub first_lsn: Lsn,
    /// Previous LSN
    pub prev_lsn: Lsn,
    /// Undo next LSN
    pub undo_next_lsn: Lsn,
    /// Unknown
    pub unknown: [u8; 32],
}