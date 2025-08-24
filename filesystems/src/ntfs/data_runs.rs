// NTFS Data Run decoder
// Phase 1.4: Decode runlists for non-resident attributes

use moses_core::MosesError;

/// Data run entry
#[derive(Debug, Clone)]
pub struct DataRun {
    pub lcn: Option<u64>,                // Logical cluster number (None for sparse)
    pub length: u64,                     // Length in clusters
}

/// Decode NTFS data runs from raw bytes
pub fn decode_data_runs(data: &[u8]) -> Result<Vec<DataRun>, MosesError> {
    let mut runs = Vec::new();
    let mut pos = 0;
    let mut prev_lcn = 0i64;
    
    while pos < data.len() {
        let header = data[pos];
        if header == 0 {
            break; // End marker
        }
        
        let length_size = (header & 0x0F) as usize;
        let offset_size = ((header >> 4) & 0x0F) as usize;
        pos += 1;
        
        if pos + length_size + offset_size > data.len() {
            return Err(MosesError::Other("Data run extends beyond buffer".to_string()));
        }
        
        // Read run length (in clusters)
        let length = read_le_bytes(&data[pos..pos + length_size]);
        pos += length_size;
        
        if offset_size == 0 {
            // Sparse run (hole in sparse file)
            runs.push(DataRun {
                lcn: None,
                length,
            });
        } else {
            // Read offset (signed, relative to previous)
            let offset = read_le_bytes_signed(&data[pos..pos + offset_size]);
            pos += offset_size;
            
            let lcn = prev_lcn + offset;
            prev_lcn = lcn;
            
            if lcn < 0 {
                return Err(MosesError::Other(format!("Invalid LCN: {}", lcn)));
            }
            
            runs.push(DataRun {
                lcn: Some(lcn as u64),
                length,
            });
        }
    }
    
    Ok(runs)
}

/// Read little-endian bytes as unsigned integer
fn read_le_bytes(bytes: &[u8]) -> u64 {
    let mut value = 0u64;
    for (i, &byte) in bytes.iter().enumerate() {
        value |= (byte as u64) << (i * 8);
    }
    value
}

/// Read little-endian bytes as signed integer
fn read_le_bytes_signed(bytes: &[u8]) -> i64 {
    if bytes.is_empty() {
        return 0;
    }
    
    let mut value = 0i64;
    for (i, &byte) in bytes.iter().enumerate() {
        value |= (byte as i64) << (i * 8);
    }
    
    // Sign extend if negative
    let bits = bytes.len() * 8;
    if bits < 64 && (value & (1 << (bits - 1))) != 0 {
        // Set all higher bits to 1
        value |= !((1i64 << bits) - 1);
    }
    
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_decode_simple_run() {
        // Single run: 16 clusters at LCN 100
        // Header: 0x21 (1 byte length, 2 bytes offset)
        // Length: 0x10 (16 clusters)
        // Offset: 0x64 0x00 (100)
        let data = vec![0x21, 0x10, 0x64, 0x00, 0x00];
        
        let runs = decode_data_runs(&data).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].lcn, Some(100));
        assert_eq!(runs[0].length, 16);
    }
    
    #[test]
    fn test_decode_multiple_runs() {
        // Two runs:
        // 1. 10 clusters at LCN 100
        // 2. 20 clusters at LCN 200 (offset +100 from previous)
        let data = vec![
            0x21, 0x0A, 0x64, 0x00,  // 10 clusters at 100
            0x21, 0x14, 0x64, 0x00,  // 20 clusters at +100 (= 200)
            0x00,  // End marker
        ];
        
        let runs = decode_data_runs(&data).unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].lcn, Some(100));
        assert_eq!(runs[0].length, 10);
        assert_eq!(runs[1].lcn, Some(200));
        assert_eq!(runs[1].length, 20);
    }
    
    #[test]
    fn test_decode_sparse_run() {
        // Sparse run (hole): 32 clusters of zeros
        // Header: 0x01 (1 byte length, 0 bytes offset = sparse)
        // Length: 0x20 (32 clusters)
        let data = vec![0x01, 0x20, 0x00];
        
        let runs = decode_data_runs(&data).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].lcn, None);  // Sparse
        assert_eq!(runs[0].length, 32);
    }
    
    #[test]
    fn test_negative_offset() {
        // Run with negative offset (going backwards)
        // First run at 1000, second run at 900 (offset -100)
        let data = vec![
            0x22,                           // Header: 2-byte length, 2-byte offset
            0x0A, 0x00,                     // Length: 10 clusters (little-endian)
            0xE8, 0x03,                     // Offset: 1000 (little-endian)
            0x11,                           // Header: 1-byte length, 1-byte offset
            0x05,                           // Length: 5 clusters
            0x9C,                           // Offset: -100 (signed byte)
            0x00,                           // End marker
        ];
        
        let runs = decode_data_runs(&data).unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].lcn, Some(1000));
        assert_eq!(runs[1].lcn, Some(900));
    }
}