// NTFS Sparse File Support
// Phase 2.3: Handle sparse files efficiently

use crate::ntfs::data_runs::DataRun;
use moses_core::MosesError;
use log::trace;

/// Sparse file information
#[derive(Debug, Clone)]
pub struct SparseInfo {
    pub is_sparse: bool,
    pub allocated_size: u64,
    pub actual_size: u64,
    pub sparse_ranges: Vec<SparseRange>,
}

/// A range of sparse (zero) data
#[derive(Debug, Clone)]
pub struct SparseRange {
    pub offset: u64,
    pub length: u64,
}

/// Check if a file is sparse based on its attributes
pub fn is_sparse_file(file_attributes: u32) -> bool {
    const FILE_ATTRIBUTE_SPARSE_FILE: u32 = 0x200;
    file_attributes & FILE_ATTRIBUTE_SPARSE_FILE != 0
}

/// Analyze data runs to identify sparse regions
pub fn analyze_sparse_runs(runs: &[DataRun], cluster_size: u32) -> SparseInfo {
    let mut sparse_ranges = Vec::new();
    let mut current_offset = 0u64;
    let mut allocated_size = 0u64;
    let mut actual_size = 0u64;
    let mut is_sparse = false;
    
    for run in runs {
        let run_size = run.length * cluster_size as u64;
        
        if run.lcn.is_none() {
            // This is a sparse run
            is_sparse = true;
            sparse_ranges.push(SparseRange {
                offset: current_offset,
                length: run_size,
            });
            trace!("Sparse range at offset {}: {} bytes", current_offset, run_size);
        } else {
            // This is allocated data
            allocated_size += run_size;
        }
        
        actual_size += run_size;
        current_offset += run_size;
    }
    
    SparseInfo {
        is_sparse,
        allocated_size,
        actual_size,
        sparse_ranges,
    }
}

/// Read sparse file data efficiently
pub fn read_sparse_data(
    runs: &[DataRun],
    cluster_size: u32,
    file_size: u64,
    read_cluster_fn: impl Fn(u64, u64) -> Result<Vec<u8>, MosesError>,
) -> Result<Vec<u8>, MosesError> {
    let mut data = Vec::with_capacity(file_size as usize);
    let mut current_offset = 0u64;
    
    for run in runs {
        let run_size = run.length * cluster_size as u64;
        
        if let Some(lcn) = run.lcn {
            // Read actual data
            let cluster_data = read_cluster_fn(lcn, run.length)?;
            data.extend_from_slice(&cluster_data);
        } else {
            // Sparse run - fill with zeros
            data.resize(data.len() + run_size as usize, 0);
            trace!("Filled {} bytes of sparse data at offset {}", run_size, current_offset);
        }
        
        current_offset += run_size;
        
        // Stop if we've read enough
        if current_offset >= file_size {
            break;
        }
    }
    
    // Truncate to actual file size
    if data.len() > file_size as usize {
        data.truncate(file_size as usize);
    }
    
    Ok(data)
}

/// Get the allocated size on disk for a sparse file
pub fn get_allocated_size(runs: &[DataRun], cluster_size: u32) -> u64 {
    runs.iter()
        .filter(|run| run.lcn.is_some())
        .map(|run| run.length * cluster_size as u64)
        .sum()
}

/// Calculate space savings from sparse allocation
pub fn calculate_space_savings(sparse_info: &SparseInfo) -> f64 {
    if sparse_info.actual_size == 0 {
        return 0.0;
    }
    
    let saved = sparse_info.actual_size - sparse_info.allocated_size;
    (saved as f64 / sparse_info.actual_size as f64) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sparse_file_detection() {
        // Normal file
        assert!(!is_sparse_file(0x20)); // FILE_ATTRIBUTE_ARCHIVE
        
        // Sparse file
        assert!(is_sparse_file(0x200)); // FILE_ATTRIBUTE_SPARSE_FILE
        
        // Sparse file with other attributes
        assert!(is_sparse_file(0x220)); // ARCHIVE | SPARSE
    }
    
    #[test]
    fn test_sparse_run_analysis() {
        let runs = vec![
            DataRun { lcn: Some(100), length: 10 },  // 10 clusters of data
            DataRun { lcn: None, length: 50 },        // 50 clusters sparse
            DataRun { lcn: Some(200), length: 5 },   // 5 clusters of data
        ];
        
        let cluster_size = 4096;
        let info = analyze_sparse_runs(&runs, cluster_size);
        
        assert!(info.is_sparse);
        assert_eq!(info.allocated_size, 15 * 4096); // 15 clusters allocated
        assert_eq!(info.actual_size, 65 * 4096);    // 65 clusters total
        assert_eq!(info.sparse_ranges.len(), 1);
        assert_eq!(info.sparse_ranges[0].offset, 10 * 4096);
        assert_eq!(info.sparse_ranges[0].length, 50 * 4096);
    }
    
    #[test]
    fn test_space_savings_calculation() {
        let info = SparseInfo {
            is_sparse: true,
            allocated_size: 1024 * 1024,      // 1 MB allocated
            actual_size: 10 * 1024 * 1024,    // 10 MB logical size
            sparse_ranges: vec![],
        };
        
        let savings = calculate_space_savings(&info);
        assert!((savings - 90.0).abs() < 0.01); // ~90% savings
    }
    
    #[test]
    fn test_sparse_data_reading() {
        let runs = vec![
            DataRun { lcn: Some(100), length: 1 },  // 1 cluster of data
            DataRun { lcn: None, length: 2 },        // 2 clusters sparse
            DataRun { lcn: Some(200), length: 1 },  // 1 cluster of data
        ];
        
        let cluster_size = 4;
        let file_size = 16;
        
        let result = read_sparse_data(&runs, cluster_size, file_size, |lcn, length| {
            // Mock cluster reading
            let mut data = vec![0xFFu8; (length * cluster_size as u64) as usize];
            data[0] = lcn as u8; // Mark with LCN for testing
            Ok(data)
        });
        
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.len(), 16);
        
        // First cluster should have data
        assert_eq!(data[0], 100);
        
        // Middle should be sparse (zeros)
        assert_eq!(data[4], 0);
        assert_eq!(data[5], 0);
        assert_eq!(data[8], 0);
        assert_eq!(data[11], 0);
        
        // Last cluster should have data
        assert_eq!(data[12], 200);
    }
}