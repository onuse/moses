// FAT16 Long Filename Support Module
// Provides LFN (VFAT) support for FAT16 filesystems

use crate::families::fat::common::FatAttributes;

// LFN entry structure (same as FAT32)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct LongNameEntry {
    pub order: u8,           // Order & last entry flag
    pub name1: [u16; 5],     // First 5 Unicode chars
    pub attributes: u8,       // Always 0x0F (ATTR_LONG_NAME)
    pub entry_type: u8,      // Always 0x00
    pub checksum: u8,        // Checksum of short name
    pub name2: [u16; 6],     // Next 6 Unicode chars  
    pub first_cluster: u16,  // Always 0x0000
    pub name3: [u16; 2],     // Last 2 Unicode chars
}

const ATTR_LONG_NAME: u8 = FatAttributes::READ_ONLY | FatAttributes::HIDDEN | 
                           FatAttributes::SYSTEM | FatAttributes::VOLUME_ID;
const LAST_LONG_ENTRY: u8 = 0x40;

/// Parse long filename entries from a directory
pub struct LfnParser {
    lfn_entries: Vec<LongNameEntry>,
}

impl LfnParser {
    pub fn new() -> Self {
        Self {
            lfn_entries: Vec::new(),
        }
    }
    
    /// Process a directory entry - returns true if it's an LFN entry
    pub fn process_entry(&mut self, entry_bytes: &[u8]) -> bool {
        if entry_bytes.len() < 32 {
            return false;
        }
        
        // Check if it's an LFN entry
        if entry_bytes[11] == ATTR_LONG_NAME {
            let lfn = unsafe {
                std::ptr::read_unaligned(entry_bytes.as_ptr() as *const LongNameEntry)
            };
            self.lfn_entries.push(lfn);
            return true;
        }
        
        false
    }
    
    /// Get the long filename if available, then reset
    pub fn get_long_name(&mut self) -> Option<String> {
        if self.lfn_entries.is_empty() {
            return None;
        }
        
        let name = Self::parse_long_name(&self.lfn_entries);
        self.lfn_entries.clear();
        name
    }
    
    /// Parse long filename from collected LFN entries
    fn parse_long_name(lfn_entries: &[LongNameEntry]) -> Option<String> {
        if lfn_entries.is_empty() {
            return None;
        }
        
        let mut full_name = String::new();
        
        // LFN entries are stored in reverse order
        for lfn in lfn_entries.iter().rev() {
            // Extract characters from each part (copy arrays to avoid packed field issues)
            let name1 = lfn.name1;
            for &ch in &name1 {
                if ch == 0 || ch == 0xFFFF { 
                    return Some(full_name);
                }
                if let Some(c) = char::from_u32(ch as u32) {
                    full_name.push(c);
                }
            }
            
            let name2 = lfn.name2;
            for &ch in &name2 {
                if ch == 0 || ch == 0xFFFF { 
                    return Some(full_name);
                }
                if let Some(c) = char::from_u32(ch as u32) {
                    full_name.push(c);
                }
            }
            
            let name3 = lfn.name3;
            for &ch in &name3 {
                if ch == 0 || ch == 0xFFFF { 
                    return Some(full_name);
                }
                if let Some(c) = char::from_u32(ch as u32) {
                    full_name.push(c);
                }
            }
        }
        
        Some(full_name)
    }
    
    /// Reset the parser state
    pub fn reset(&mut self) {
        self.lfn_entries.clear();
    }
}

/// Create LFN entries for a long filename
pub fn create_lfn_entries(long_name: &str, short_name: &[u8; 11]) -> Vec<LongNameEntry> {
    let mut entries = Vec::new();
    let checksum = calculate_checksum(short_name);
    
    // Convert name to UTF-16
    let name_utf16: Vec<u16> = long_name.encode_utf16().collect();
    let mut padded = name_utf16.clone();
    
    // Pad with 0x0000 and then 0xFFFF
    if padded.len() < 255 {
        padded.push(0x0000);
        while padded.len() < 255 {
            padded.push(0xFFFF);
        }
    }
    
    // Calculate number of entries needed
    let num_entries = (padded.len() + 12) / 13; // 13 chars per entry
    
    for i in 0..num_entries {
        let mut entry = LongNameEntry {
            order: (i + 1) as u8,
            name1: [0xFFFF; 5],
            attributes: ATTR_LONG_NAME,
            entry_type: 0,
            checksum,
            name2: [0xFFFF; 6],
            first_cluster: 0,
            name3: [0xFFFF; 2],
        };
        
        // Mark last entry
        if i == num_entries - 1 {
            entry.order |= LAST_LONG_ENTRY;
        }
        
        // Fill in the name characters
        let offset = i * 13;
        for j in 0..5 {
            if offset + j < padded.len() {
                entry.name1[j] = padded[offset + j];
            }
        }
        for j in 0..6 {
            if offset + 5 + j < padded.len() {
                entry.name2[j] = padded[offset + 5 + j];
            }
        }
        for j in 0..2 {
            if offset + 11 + j < padded.len() {
                entry.name3[j] = padded[offset + 11 + j];
            }
        }
        
        entries.push(entry);
    }
    
    // Reverse to store in correct order
    entries.reverse();
    entries
}

/// Calculate checksum for short name
fn calculate_checksum(short_name: &[u8; 11]) -> u8 {
    let mut sum: u8 = 0;
    for &byte in short_name {
        sum = ((sum >> 1) | (sum << 7)).wrapping_add(byte);
    }
    sum
}

/// Check if a name needs LFN support
pub fn needs_lfn(name: &str) -> bool {
    // Check length
    let (base, ext) = if let Some(pos) = name.rfind('.') {
        (&name[..pos], &name[pos + 1..])
    } else {
        (name, "")
    };
    
    if base.len() > 8 || ext.len() > 3 {
        return true;
    }
    
    // Check for lowercase letters
    if name.chars().any(|c| c.is_lowercase()) {
        return true;
    }
    
    // Check for special characters
    for c in name.chars() {
        match c {
            'A'..='Z' | '0'..='9' | '!' | '#' | '$' | '%' | '&' | '\'' | 
            '(' | ')' | '-' | '@' | '^' | '_' | '`' | '{' | '}' | '~' | '.' => {},
            _ => return true,
        }
    }
    
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_needs_lfn() {
        assert!(!needs_lfn("TEST.TXT"));
        assert!(!needs_lfn("FILE123.DOC"));
        assert!(needs_lfn("lowercase.txt"));
        assert!(needs_lfn("very_long_filename.txt"));
        assert!(needs_lfn("file with spaces.txt"));
    }
    
    #[test]
    fn test_checksum() {
        let short_name = b"TEST    TXT";
        let checksum = calculate_checksum(short_name);
        assert_ne!(checksum, 0);
    }
}