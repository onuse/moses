// JBD2 Checksum Calculation
// Implements CRC32C checksum for journal integrity

use crate::families::ext::ext4_native::journal::jbd2::{JournalHeader, JournalBlockTag};

/// CRC32C polynomial
const CRC32C_POLY: u32 = 0x82F63B78;

/// Precomputed CRC32C table
static CRC32C_TABLE: [u32; 256] = generate_crc32c_table();

/// Generate CRC32C lookup table at compile time
const fn generate_crc32c_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ CRC32C_POLY;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        
        table[i] = crc;
        i += 1;
    }
    
    table
}

/// Calculate CRC32C checksum
pub fn crc32c(data: &[u8], initial: u32) -> u32 {
    let mut crc = !initial;
    
    for &byte in data {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32C_TABLE[index];
    }
    
    !crc
}

/// Calculate checksum for a journal descriptor block
pub fn calculate_descriptor_checksum(
    header: &JournalHeader,
    tags: &[JournalBlockTag],
    seed: u32,
) -> u32 {
    // Start with the header (excluding magic number)
    let header_bytes = unsafe {
        std::slice::from_raw_parts(
            (header as *const JournalHeader as *const u8).add(4),
            std::mem::size_of::<JournalHeader>() - 4
        )
    };
    
    let mut checksum = crc32c(header_bytes, seed);
    
    // Add tags
    for tag in tags {
        let tag_bytes = unsafe {
            std::slice::from_raw_parts(
                tag as *const JournalBlockTag as *const u8,
                std::mem::size_of::<JournalBlockTag>() - 4  // Exclude checksum field
            )
        };
        checksum = crc32c(tag_bytes, checksum);
    }
    
    checksum
}

/// Calculate checksum for a journal commit block
pub fn calculate_commit_checksum(
    header: &JournalHeader,
    seed: u32,
) -> u32 {
    let header_bytes = unsafe {
        std::slice::from_raw_parts(
            (header as *const JournalHeader as *const u8).add(4),
            std::mem::size_of::<JournalHeader>() - 4
        )
    };
    
    crc32c(header_bytes, seed)
}

/// Calculate checksum for a data block
pub fn calculate_data_checksum(
    data: &[u8],
    block_num: u64,
    tid: u32,
) -> u32 {
    // Include block number and transaction ID in checksum
    let mut seed = crc32c(&block_num.to_le_bytes(), 0);
    seed = crc32c(&tid.to_le_bytes(), seed);
    
    crc32c(data, seed)
}

/// Verify checksum for a journal block
pub fn verify_block_checksum(
    data: &[u8],
    expected: u32,
    block_num: u64,
    tid: u32,
) -> bool {
    let calculated = calculate_data_checksum(data, block_num, tid);
    calculated == expected
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_crc32c_basic() {
        // Test with known values
        let data = b"123456789";
        let checksum = crc32c(data, 0);
        assert_eq!(checksum, 0xe3069283);
    }
    
    #[test]
    fn test_crc32c_empty() {
        let data = b"";
        let checksum = crc32c(data, 0);
        assert_eq!(checksum, 0);
    }
    
    #[test]
    fn test_crc32c_incremental() {
        let data1 = b"Hello";
        let data2 = b"World";
        
        let checksum1 = crc32c(data1, 0);
        let checksum2 = crc32c(data2, checksum1);
        
        let combined = b"HelloWorld";
        let checksum_combined = crc32c(combined, 0);
        
        assert_eq!(checksum2, checksum_combined);
    }
}