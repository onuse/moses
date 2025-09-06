// NTFS LZNT1 compression/decompression
// Phase 2.2: Support for reading compressed files

use moses_core::MosesError;

/// Compression unit size (typically 16 clusters = 64KB for 4KB clusters)
pub const COMPRESSION_UNIT_SIZE: usize = 65536;

/// Decompress LZNT1 compressed data
pub fn decompress_lznt1(compressed: &[u8], decompressed_size: usize) -> Result<Vec<u8>, MosesError> {
    let mut result = Vec::with_capacity(decompressed_size);
    let mut pos = 0;
    
    while pos < compressed.len() && result.len() < decompressed_size {
        if pos + 2 > compressed.len() {
            break;
        }
        
        // Read chunk header (2 bytes)
        let header = u16::from_le_bytes([compressed[pos], compressed[pos + 1]]);
        pos += 2;
        
        if header == 0 {
            // End of compression
            break;
        }
        
        // Parse header
        let signature = (header >> 12) & 0x7;
        let chunk_size = ((header & 0x0FFF) + 1) as usize;
        
        if signature != 0x3 {
            // Not compressed, should not happen in valid LZNT1
            return Err(MosesError::Other(format!("Invalid LZNT1 signature: {}", signature)));
        }
        
        if pos + chunk_size > compressed.len() {
            return Err(MosesError::Other("LZNT1 chunk extends beyond buffer".to_string()));
        }
        
        // Decompress the chunk
        let chunk_data = &compressed[pos..pos + chunk_size];
        decompress_chunk(chunk_data, &mut result)?;
        
        pos += chunk_size;
    }
    
    Ok(result)
}

/// Decompress a single LZNT1 chunk
fn decompress_chunk(chunk: &[u8], output: &mut Vec<u8>) -> Result<(), MosesError> {
    let mut pos = 0;
    
    while pos < chunk.len() {
        if pos >= chunk.len() {
            break;
        }
        
        // Read flag byte
        let flags = chunk[pos];
        pos += 1;
        
        // Process 8 flag bits
        for i in 0..8 {
            if pos >= chunk.len() {
                break;
            }
            
            if flags & (1 << i) != 0 {
                // Compressed token (2 bytes)
                if pos + 1 >= chunk.len() {
                    break;
                }
                
                let token = u16::from_le_bytes([chunk[pos], chunk[pos + 1]]);
                pos += 2;
                
                // Decode the back reference
                let (offset, length) = decode_token(token, output.len());
                
                // Copy from back reference
                if offset > output.len() {
                    return Err(MosesError::Other(format!(
                        "Invalid LZNT1 back reference: offset {} > output length {}",
                        offset, output.len()
                    )));
                }
                
                let copy_start = output.len() - offset;
                for j in 0..length {
                    let byte = output[copy_start + (j % offset)];
                    output.push(byte);
                }
            } else {
                // Literal byte
                output.push(chunk[pos]);
                pos += 1;
            }
        }
    }
    
    Ok(())
}

/// Decode an LZNT1 compression token
fn decode_token(token: u16, output_pos: usize) -> (usize, usize) {
    // The token format depends on the output position
    let pos_bits = if output_pos == 0 {
        0
    } else {
        (output_pos - 1).leading_zeros() as usize
    };
    
    let length_bits = if pos_bits < 4 {
        4
    } else if pos_bits < 16 {
        16 - pos_bits
    } else {
        0
    };
    
    let length_mask = (1 << length_bits) - 1;
    let offset_mask = !length_mask;
    
    let length = ((token as usize) & length_mask) + 3;
    let offset = (((token as usize) & offset_mask) >> length_bits) + 1;
    
    (offset, length)
}

/// Check if data runs indicate compression
pub fn is_compressed(compression_unit: u16) -> bool {
    compression_unit != 0
}

/// Calculate compression unit size from the compression_unit field
pub fn get_compression_unit_size(compression_unit: u16, cluster_size: u32) -> usize {
    if compression_unit == 0 {
        return 0;
    }
    
    // Compression unit is 2^compression_unit clusters
    let clusters = 1usize << compression_unit;
    clusters * cluster_size as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_decode_token() {
        // Test token decoding at various positions
        let (offset, length) = decode_token(0x1234, 100);
        assert!(offset > 0);
        assert!(length >= 3);
    }
    
    #[test]
    fn test_compression_unit_size() {
        // Test with 4KB clusters
        let cluster_size = 4096;
        
        // No compression
        assert_eq!(get_compression_unit_size(0, cluster_size), 0);
        
        // 2^4 = 16 clusters = 64KB
        assert_eq!(get_compression_unit_size(4, cluster_size), 65536);
    }
    
    #[test]
    fn test_simple_decompression() {
        // Create a simple compressed chunk
        // LZNT1 header format: 0xBSSS where B is signature (3) and SSS is size-1
        let compressed = vec![
            0x0A, 0x30,  // Header: 0x300A = signature 3, size 0x00A+1=11 bytes
            0x00,        // Flags: all literals (no compression)
            b'H', b'e', b'l', b'l', b'o', b' ', b'W', b'o',  // 8 literal bytes
            0x00, 0x00,  // End marker
        ];
        
        match decompress_lznt1(&compressed, 100) {
            Ok(decompressed) => {
                assert_eq!(&decompressed[..8], b"Hello Wo");
            }
            Err(e) => {
                panic!("Decompression failed: {}", e);
            }
        }
    }
}