// EXT4 Reference Test Vectors
// These are known-good values from the ext4 specification and Linux implementation

use std::collections::HashMap;

/// Known good superblock values for validation
pub struct Ext4ReferenceData {
    pub superblock_offsets: HashMap<&'static str, (usize, Vec<u8>)>,
    pub feature_flags: HashMap<&'static str, u32>,
    pub known_good_checksums: HashMap<&'static str, u32>,
}

impl Ext4ReferenceData {
    pub fn new() -> Self {
        let mut sb_offsets = HashMap::new();
        
        // Magic number (0xEF53) at offset 56-57
        sb_offsets.insert("magic", (56, vec![0x53, 0xEF]));
        
        // State (1 = valid) at offset 58-59
        sb_offsets.insert("state", (58, vec![0x01, 0x00]));
        
        // Revision level (1 = dynamic) at offset 76-79
        sb_offsets.insert("rev_level", (76, vec![0x01, 0x00, 0x00, 0x00]));
        
        // First inode (11) at offset 84-87
        sb_offsets.insert("first_ino", (84, vec![0x0B, 0x00, 0x00, 0x00]));
        
        // Inode size (256) at offset 88-89
        sb_offsets.insert("inode_size", (88, vec![0x00, 0x01]));
        
        let mut features = HashMap::new();
        features.insert("FILETYPE", 0x0002);
        features.insert("EXTENTS", 0x0040);
        features.insert("64BIT", 0x0080);
        features.insert("FLEX_BG", 0x0200);
        features.insert("SPARSE_SUPER", 0x0001);
        features.insert("LARGE_FILE", 0x0002);
        features.insert("HUGE_FILE", 0x0008);
        features.insert("GDT_CSUM", 0x0010);
        features.insert("DIR_NLINK", 0x0020);
        features.insert("EXTRA_ISIZE", 0x0040);
        
        Self {
            superblock_offsets: sb_offsets,
            feature_flags: features,
            known_good_checksums: HashMap::new(),
        }
    }
    
    /// Validate a superblock buffer against known good values
    pub fn validate_superblock(&self, buffer: &[u8]) -> Vec<String> {
        let mut errors = Vec::new();
        
        for (name, (offset, expected)) in &self.superblock_offsets {
            let actual = &buffer[*offset..*offset + expected.len()];
            if actual != expected.as_slice() {
                errors.push(format!(
                    "{} mismatch at offset {}: expected {:?}, got {:?}",
                    name, offset, expected, actual
                ));
            }
        }
        
        errors
    }
    
    /// Get expected feature flags for a standard ext4 filesystem
    pub fn get_standard_features(&self) -> u32 {
        self.feature_flags["FILETYPE"] |
        self.feature_flags["EXTENTS"] |
        self.feature_flags["64BIT"] |
        self.feature_flags["FLEX_BG"]
    }
    
    /// Calculate CRC32c checksum (ext4 uses CRC32c for metadata)
    pub fn calculate_crc32c(&self, data: &[u8], initial: u32) -> u32 {
        // This would use the actual CRC32c algorithm
        // For now, placeholder
        crc32fast::hash(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_reference_data() {
        let ref_data = Ext4ReferenceData::new();
        
        // Test magic number
        let (offset, magic) = &ref_data.superblock_offsets["magic"];
        assert_eq!(*offset, 56);
        assert_eq!(magic, &vec![0x53, 0xEF]);
        
        // Test feature flags
        let features = ref_data.get_standard_features();
        assert!(features & 0x0002 != 0); // FILETYPE
        assert!(features & 0x0040 != 0); // EXTENTS
    }
    
    #[test]
    fn test_superblock_validation() {
        let ref_data = Ext4ReferenceData::new();
        
        // Create a mock superblock buffer
        let mut buffer = vec![0u8; 1024];
        
        // Set correct magic number
        buffer[56] = 0x53;
        buffer[57] = 0xEF;
        
        // Set correct state
        buffer[58] = 0x01;
        buffer[59] = 0x00;
        
        // Set correct revision
        buffer[76] = 0x01;
        
        // Set correct first inode
        buffer[84] = 0x0B;
        
        // Set correct inode size
        buffer[88] = 0x00;
        buffer[89] = 0x01;
        
        let errors = ref_data.validate_superblock(&buffer);
        assert_eq!(errors.len(), 0);
        
        // Test with wrong magic
        buffer[56] = 0x00;
        let errors = ref_data.validate_superblock(&buffer);
        assert!(errors.len() > 0);
        assert!(errors[0].contains("magic"));
    }
}