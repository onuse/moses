// Long filename support for FAT family filesystems
// Handles both FAT16/32 LFN (VFAT) and exFAT extended names

use crate::fat_common::directory::lfn_checksum;

/// Maximum filename length by filesystem type
pub const MAX_FAT_LFN_LENGTH: usize = 255;    // FAT16/32 LFN
pub const MAX_EXFAT_NAME_LENGTH: usize = 255; // exFAT

/// Common trait for long name handling
pub trait LongNameHandler {
    /// Check if a name needs long name support
    fn needs_long_name(&self, name: &str) -> bool;
    
    /// Generate short (8.3) name from long name
    fn generate_short_name(&self, long_name: &str, existing_names: &[String]) -> [u8; 11];
    
    /// Calculate number of entries needed
    fn entries_needed(&self, name: &str) -> usize;
}

/// FAT16/32 LFN (VFAT) implementation
pub struct VfatLongNameHandler;

impl LongNameHandler for VfatLongNameHandler {
    fn needs_long_name(&self, name: &str) -> bool {
        // Already implemented in directory.rs
        crate::fat_common::directory::needs_lfn(name)
    }
    
    fn generate_short_name(&self, long_name: &str, existing_names: &[String]) -> [u8; 11] {
        // Generate unique 8.3 name with ~1, ~2, etc.
        let base = Self::create_base_name(long_name);
        let mut short_name = [0x20u8; 11]; // Space-padded
        
        // Try without numeric tail first
        let candidate = Self::format_short_name(&base, None);
        if !Self::name_exists(&candidate, existing_names) {
            return candidate;
        }
        
        // Add numeric tail ~1 through ~999999
        for i in 1..=999999 {
            let candidate = Self::format_short_name(&base, Some(i));
            if !Self::name_exists(&candidate, existing_names) {
                return candidate;
            }
        }
        
        // Fallback: use first 6 chars + ~1
        short_name[0..6].copy_from_slice(&base[0..6]);
        short_name[6] = b'~';
        short_name[7] = b'1';
        short_name
    }
    
    fn entries_needed(&self, name: &str) -> usize {
        if !self.needs_long_name(name) {
            return 1; // Just the 8.3 entry
        }
        
        // Each LFN entry holds 13 chars, plus 1 for the 8.3 entry
        let lfn_entries = (name.len() + 12) / 13;
        lfn_entries + 1
    }
}

impl VfatLongNameHandler {
    fn create_base_name(long_name: &str) -> Vec<u8> {
        let upper = long_name.to_uppercase();
        let mut base = Vec::new();
        
        for ch in upper.chars() {
            if base.len() >= 8 {
                break;
            }
            
            // Skip invalid chars and spaces
            if ch.is_ascii_alphanumeric() || "-_".contains(ch) {
                base.push(ch as u8);
            }
        }
        
        // Pad to at least 1 character
        if base.is_empty() {
            base.push(b'_');
        }
        
        base
    }
    
    fn format_short_name(base: &[u8], numeric_tail: Option<u32>) -> [u8; 11] {
        let mut result = [0x20u8; 11]; // Space-padded
        
        if let Some(num) = numeric_tail {
            let tail = format!("~{}", num);
            let base_len = (8 - tail.len()).min(base.len());
            
            result[0..base_len].copy_from_slice(&base[0..base_len]);
            result[base_len..base_len + tail.len()].copy_from_slice(tail.as_bytes());
        } else {
            let len = base.len().min(8);
            result[0..len].copy_from_slice(&base[0..len]);
        }
        
        result
    }
    
    fn name_exists(name: &[u8; 11], existing: &[String]) -> bool {
        let name_str = String::from_utf8_lossy(&name[..]).trim().to_string();
        existing.iter().any(|n| n.eq_ignore_ascii_case(&name_str))
    }
}

/// exFAT extended name implementation
pub struct ExFatLongNameHandler;

impl LongNameHandler for ExFatLongNameHandler {
    fn needs_long_name(&self, _name: &str) -> bool {
        // exFAT always uses extended entries, even for short names
        true
    }
    
    fn generate_short_name(&self, _long_name: &str, _existing_names: &[String]) -> [u8; 11] {
        // exFAT doesn't use 8.3 names
        [0; 11]
    }
    
    fn entries_needed(&self, name: &str) -> usize {
        // File entry + Stream entry + FileName entries
        // Each FileName entry holds 15 chars
        let name_entries = (name.len() + 14) / 15;
        2 + name_entries // File + Stream + Name(s)
    }
}

/// Create LFN entries for FAT16/32
pub fn create_vfat_lfn_entries(long_name: &str, short_name: &[u8; 11]) -> Vec<[u8; 32]> {
    let mut entries = Vec::new();
    let checksum = lfn_checksum(short_name);
    
    // Convert to UTF-16LE
    let utf16: Vec<u16> = long_name.encode_utf16().collect();
    let mut char_offset = 0;
    let num_entries = (utf16.len() + 12) / 13;
    
    // Create entries in reverse order (last first)
    for i in (0..num_entries).rev() {
        let mut entry = [0xFFu8; 32];
        
        // Sequence number (0x40 = last entry marker)
        entry[0] = if i == num_entries - 1 {
            0x40 | ((i + 1) as u8)
        } else {
            (i + 1) as u8
        };
        
        // Copy up to 13 characters
        let mut copied = 0;
        
        // First 5 chars (offset 1-10)
        for j in 0..5 {
            if char_offset + copied < utf16.len() {
                let ch = utf16[char_offset + copied];
                entry[1 + j * 2] = (ch & 0xFF) as u8;
                entry[2 + j * 2] = (ch >> 8) as u8;
                copied += 1;
            }
        }
        
        // Attributes (offset 11)
        entry[11] = 0x0F; // LFN marker
        
        // Type (offset 12)
        entry[12] = 0x00;
        
        // Checksum (offset 13)
        entry[13] = checksum;
        
        // Next 6 chars (offset 14-25)
        for j in 0..6 {
            if char_offset + copied < utf16.len() {
                let ch = utf16[char_offset + copied];
                entry[14 + j * 2] = (ch & 0xFF) as u8;
                entry[15 + j * 2] = (ch >> 8) as u8;
                copied += 1;
            }
        }
        
        // First cluster (offset 26-27) - always 0
        entry[26] = 0x00;
        entry[27] = 0x00;
        
        // Last 2 chars (offset 28-31)
        for j in 0..2 {
            if char_offset + copied < utf16.len() {
                let ch = utf16[char_offset + copied];
                entry[28 + j * 2] = (ch & 0xFF) as u8;
                entry[29 + j * 2] = (ch >> 8) as u8;
                copied += 1;
            }
        }
        
        entries.push(entry);
        char_offset += copied;
    }
    
    // Reverse to get correct order (first entry first)
    entries.reverse();
    entries
}

/// Hash function for exFAT filename (for name hash in stream entry)
pub fn exfat_name_hash(name: &str) -> u16 {
    let mut hash = 0u16;
    let name_upper = name.to_uppercase();
    
    for ch in name_upper.encode_utf16() {
        hash = ((hash << 15) | (hash >> 1)) + (ch & 0xFF) as u16;
        hash = ((hash << 15) | (hash >> 1)) + (ch >> 8) as u16;
    }
    
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_vfat_short_name_generation() {
        let handler = VfatLongNameHandler;
        let existing = vec![];
        
        let short = handler.generate_short_name("LongFileName.txt", &existing);
        assert_eq!(&short[0..8], b"LONGFILE");
    }
    
    #[test]
    fn test_vfat_entries_needed() {
        let handler = VfatLongNameHandler;
        
        // Short name needs 1 entry
        assert_eq!(handler.entries_needed("TEST.TXT"), 1);
        
        // 13 chars need 1 LFN + 1 short = 2 entries
        assert_eq!(handler.entries_needed("thirteenchars"), 2);
        
        // 26 chars need 2 LFN + 1 short = 3 entries
        assert_eq!(handler.entries_needed("twentysixcharactersexactly"), 3);
    }
    
    #[test]
    fn test_exfat_entries_needed() {
        let handler = ExFatLongNameHandler;
        
        // Even short names need 3 entries minimum
        assert_eq!(handler.entries_needed("test.txt"), 3);
        
        // 15 chars fit in one name entry
        assert_eq!(handler.entries_needed("fifteencharfile"), 3);
        
        // 16 chars need 2 name entries
        assert_eq!(handler.entries_needed("sixteencharfiles"), 4);
    }
}