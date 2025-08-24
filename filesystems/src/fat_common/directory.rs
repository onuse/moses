// Shared directory entry handling for FAT family filesystems
// Provides common functionality for FAT16, FAT32, and exFAT

use moses_core::MosesError;

/// Common trait for directory entries across FAT variants
pub trait FatDirectoryEntry {
    fn get_name(&self) -> String;
    fn get_size(&self) -> u64;
    fn get_first_cluster(&self) -> u32;
    fn is_directory(&self) -> bool;
    fn is_volume_label(&self) -> bool;
    fn get_attributes(&self) -> u8;
}

/// Directory entry attributes common to all FAT variants
pub mod attributes {
    pub const ATTR_READ_ONLY: u8 = 0x01;
    pub const ATTR_HIDDEN: u8 = 0x02;
    pub const ATTR_SYSTEM: u8 = 0x04;
    pub const ATTR_VOLUME_ID: u8 = 0x08;
    pub const ATTR_DIRECTORY: u8 = 0x10;
    pub const ATTR_ARCHIVE: u8 = 0x20;
    pub const ATTR_LONG_NAME: u8 = 0x0F;  // FAT16/32 LFN marker
}

/// Parse 8.3 filename format (FAT16/32)
pub fn parse_83_name(name: &[u8; 11]) -> String {
    let mut result = String::new();
    
    // Parse base name (first 8 bytes)
    for &byte in &name[0..8] {
        if byte == 0x20 || byte == 0x00 {  // Space or null = end
            break;
        }
        if byte == 0x05 {  // Special case: 0x05 = 0xE5
            result.push(0xE5 as char);
        } else {
            result.push(byte as char);
        }
    }
    
    // Parse extension (last 3 bytes)
    let ext_start = result.len();
    for &byte in &name[8..11] {
        if byte != 0x20 && byte != 0x00 {
            if result.len() == ext_start {
                result.push('.');
            }
            result.push(byte as char);
        }
    }
    
    result
}

/// Format a filename to 8.3 format
pub fn format_83_name(filename: &str) -> Result<[u8; 11], MosesError> {
    let mut result = [0x20u8; 11];  // Space-padded
    
    let upper = filename.to_uppercase();
    let parts: Vec<&str> = upper.splitn(2, '.').collect();
    
    // Base name (max 8 chars)
    let base = parts[0];
    if base.is_empty() || base.len() > 8 {
        return Err(MosesError::Other(format!("Invalid filename: {}", filename)));
    }
    
    for (i, byte) in base.bytes().enumerate().take(8) {
        if !is_valid_83_char(byte) {
            return Err(MosesError::Other(format!("Invalid character in filename: {}", filename)));
        }
        result[i] = if i == 0 && byte == 0xE5 { 0x05 } else { byte };
    }
    
    // Extension (max 3 chars)
    if parts.len() > 1 {
        let ext = parts[1];
        if ext.len() > 3 {
            return Err(MosesError::Other(format!("Extension too long: {}", ext)));
        }
        
        for (i, byte) in ext.bytes().enumerate().take(3) {
            if !is_valid_83_char(byte) {
                return Err(MosesError::Other(format!("Invalid character in extension: {}", ext)));
            }
            result[8 + i] = byte;
        }
    }
    
    Ok(result)
}

/// Check if a character is valid for 8.3 filenames
fn is_valid_83_char(c: u8) -> bool {
    match c {
        b'A'..=b'Z' | b'0'..=b'9' | b'!' | b'#' | b'$' | b'%' | b'&' | 
        b'\'' | b'(' | b')' | b'-' | b'@' | b'^' | b'_' | b'`' | 
        b'{' | b'}' | b'~' => true,
        _ => false,
    }
}

/// Check if a name needs long filename support
pub fn needs_lfn(name: &str) -> bool {
    // Check length
    let parts: Vec<&str> = name.splitn(2, '.').collect();
    if parts[0].len() > 8 || (parts.len() > 1 && parts[1].len() > 3) {
        return true;
    }
    
    // Check for lowercase or invalid chars
    for c in name.chars() {
        if c.is_lowercase() || !is_valid_83_char(c as u8) {
            return true;
        }
    }
    
    false
}

/// Calculate checksum for LFN entries (FAT16/32)
pub fn lfn_checksum(short_name: &[u8; 11]) -> u8 {
    let mut sum = 0u8;
    for &byte in short_name {
        sum = ((sum >> 1) | ((sum & 1) << 7)).wrapping_add(byte);
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_83_name() {
        assert_eq!(parse_83_name(b"README  TXT"), "README.TXT");
        assert_eq!(parse_83_name(b"FOLDER     "), "FOLDER");
        assert_eq!(parse_83_name(b"TEST    C  "), "TEST.C");
    }
    
    #[test]
    fn test_format_83_name() {
        assert_eq!(format_83_name("README.TXT").unwrap(), *b"README  TXT");
        assert_eq!(format_83_name("test.c").unwrap(), *b"TEST    C  ");
        assert_eq!(format_83_name("FOLDER").unwrap(), *b"FOLDER     ");
    }
    
    #[test]
    fn test_needs_lfn() {
        assert!(!needs_lfn("README.TXT"));
        assert!(needs_lfn("readme.txt"));  // lowercase
        assert!(needs_lfn("very_long_filename.txt"));  // too long
        assert!(needs_lfn("file.jpeg"));  // extension too long
    }
}