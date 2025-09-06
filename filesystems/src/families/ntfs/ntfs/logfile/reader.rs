// NTFS $LogFile Reader
// Handles reading and parsing log records

use moses_core::MosesError;
use super::{structures::*, lsn::Lsn};

/// LogFile reader
pub struct LogFileReader {
    /// Log data
    log_data: Vec<u8>,
    /// Page size
    page_size: u32,
    /// Log size
    log_size: u64,
}

impl LogFileReader {
    /// Create a new LogFile reader
    pub fn new(log_data: Vec<u8>, page_size: u32) -> Self {
        let log_size = log_data.len() as u64;
        
        Self {
            log_data,
            page_size,
            log_size,
        }
    }
    
    /// Read restart area
    pub fn read_restart_area(&self, index: u32) -> Result<(RestartArea, RestartAreaData), MosesError> {
        let offset = (self.page_size * index) as usize;
        
        if offset + std::mem::size_of::<RestartArea>() > self.log_data.len() {
            return Err(MosesError::Other("Invalid restart area offset".to_string()));
        }
        
        // Read restart area header
        let restart_area = unsafe {
            std::ptr::read_unaligned(
                self.log_data.as_ptr().add(offset) as *const RestartArea
            )
        };
        
        // Verify magic
        if restart_area.magic != RSTR_MAGIC {
            return Err(MosesError::Other("Invalid restart area magic".to_string()));
        }
        
        // Read restart area data
        let data_offset = offset + restart_area.restart_area_offset as usize;
        if data_offset + std::mem::size_of::<RestartAreaData>() > self.log_data.len() {
            return Err(MosesError::Other("Invalid restart area data offset".to_string()));
        }
        
        let restart_data = unsafe {
            std::ptr::read_unaligned(
                self.log_data.as_ptr().add(data_offset) as *const RestartAreaData
            )
        };
        
        Ok((restart_area, restart_data))
    }
    
    /// Read log record at LSN
    pub fn read_record(&self, lsn: Lsn) -> Result<(LogRecordHeader, Vec<u8>, Vec<u8>), MosesError> {
        let physical_offset = lsn.to_physical_offset(self.page_size);
        
        if physical_offset >= self.log_size {
            return Err(MosesError::Other("LSN offset out of range".to_string()));
        }
        
        // Read record header
        let header_size = std::mem::size_of::<LogRecordHeader>();
        if physical_offset + header_size as u64 > self.log_size {
            return Err(MosesError::Other("Record extends beyond log".to_string()));
        }
        
        let header = unsafe {
            std::ptr::read_unaligned(
                self.log_data.as_ptr().add(physical_offset as usize) as *const LogRecordHeader
            )
        };
        
        // Verify LSN
        let this_lsn = header.this_lsn;
        if this_lsn != lsn {
            return Err(MosesError::Other(format!(
                "LSN mismatch: expected {}, got {}",
                lsn, this_lsn
            )));
        }
        
        // Read redo data
        let redo_offset = physical_offset + header.redo_offset as u64;
        let redo_data = if header.redo_length > 0 {
            if redo_offset + header.redo_length as u64 > self.log_size {
                return Err(MosesError::Other("Redo data extends beyond log".to_string()));
            }
            self.log_data[redo_offset as usize..(redo_offset + header.redo_length as u64) as usize].to_vec()
        } else {
            Vec::new()
        };
        
        // Read undo data
        let undo_offset = physical_offset + header.undo_offset as u64;
        let undo_data = if header.undo_length > 0 {
            if undo_offset + header.undo_length as u64 > self.log_size {
                return Err(MosesError::Other("Undo data extends beyond log".to_string()));
            }
            self.log_data[undo_offset as usize..(undo_offset + header.undo_length as u64) as usize].to_vec()
        } else {
            Vec::new()
        };
        
        Ok((header, redo_data, undo_data))
    }
    
    /// Read log page header at offset
    pub fn read_page_header(&self, page_offset: u64) -> Result<LogPageHeader, MosesError> {
        if page_offset + std::mem::size_of::<LogPageHeader>() as u64 > self.log_size {
            return Err(MosesError::Other("Page header extends beyond log".to_string()));
        }
        
        let header = unsafe {
            std::ptr::read_unaligned(
                self.log_data.as_ptr().add(page_offset as usize) as *const LogPageHeader
            )
        };
        
        // Verify magic
        if header.magic != RCRD_MAGIC {
            return Err(MosesError::Other("Invalid log page magic".to_string()));
        }
        
        Ok(header)
    }
    
    /// Iterate through records starting from LSN
    pub fn iterate_records(&self, start_lsn: Lsn) -> RecordIterator<'_> {
        RecordIterator {
            reader: self,
            current_lsn: start_lsn,
        }
    }
    
    /// Find the most recent valid restart area
    pub fn find_valid_restart_area(&self) -> Result<(RestartArea, RestartAreaData), MosesError> {
        let mut best_area = None;
        let mut best_data = None;
        let mut best_lsn = Lsn::INVALID;
        
        // Check both restart areas (typically at pages 0 and 1)
        for i in 0..2 {
            match self.read_restart_area(i) {
                Ok((area, data)) => {
                    let current_lsn = data.current_lsn;
                    if current_lsn > best_lsn {
                        best_area = Some(area);
                        best_data = Some(data);
                        best_lsn = current_lsn;
                    }
                }
                Err(_) => continue,
            }
        }
        
        if let (Some(area), Some(data)) = (best_area, best_data) {
            Ok((area, data))
        } else {
            Err(MosesError::Other("No valid restart area found".to_string()))
        }
    }
}

/// Iterator for log records
pub struct RecordIterator<'a> {
    reader: &'a LogFileReader,
    current_lsn: Lsn,
}

impl<'a> Iterator for RecordIterator<'a> {
    type Item = Result<(LogRecordHeader, Vec<u8>, Vec<u8>), MosesError>;
    
    fn next(&mut self) -> Option<Self::Item> {
        if !self.current_lsn.is_valid() {
            return None;
        }
        
        match self.reader.read_record(self.current_lsn) {
            Ok((header, redo_data, undo_data)) => {
                // Move to previous record in chain
                self.current_lsn = header.prev_lsn;
                Some(Ok((header, redo_data, undo_data)))
            }
            Err(e) => {
                self.current_lsn = Lsn::INVALID;
                Some(Err(e))
            }
        }
    }
}