// exFAT Unicode upcase table generation
// The upcase table is used for case-insensitive filename comparison

/// Generate the standard exFAT upcase table
/// This table maps Unicode characters to their uppercase equivalents
/// for case-insensitive filename comparison
pub fn generate_upcase_table() -> Vec<u8> {
    let mut table = Vec::with_capacity(128 * 1024);
    
    // The upcase table contains 65536 entries (one for each UTF-16 code unit)
    // Each entry is 2 bytes (UTF-16LE)
    for i in 0u16..=0xFFFF {
        let upper = to_upper_case(i);
        table.extend_from_slice(&upper.to_le_bytes());
    }
    
    table
}

/// Convert a UTF-16 code unit to uppercase
/// This implements the standard Unicode uppercase mapping
fn to_upper_case(ch: u16) -> u16 {
    match ch {
        // ASCII lowercase to uppercase
        0x0061..=0x007A => ch - 0x20,
        
        // Latin-1 Supplement lowercase
        0x00E0..=0x00F6 | 0x00F8..=0x00FE => ch - 0x20,
        
        // Latin Extended-A
        0x0100..=0x012F => {
            if ch % 2 == 1 { ch - 1 } else { ch }
        }
        0x0132..=0x0137 => {
            if ch % 2 == 1 { ch - 1 } else { ch }
        }
        0x0139..=0x0148 => {
            if ch % 2 == 0 { ch - 1 } else { ch }
        }
        0x014A..=0x0177 => {
            if ch % 2 == 1 { ch - 1 } else { ch }
        }
        
        // Latin Extended-B
        0x0180..=0x01F5 => {
            match ch {
                0x0180 => 0x0243,
                0x0183 => 0x0182,
                0x0185 => 0x0184,
                0x0188 => 0x0187,
                0x018C => 0x018B,
                0x0192 => 0x0191,
                0x0195 => 0x01F6,
                0x0199 => 0x0198,
                0x019A => 0x023D,
                0x019E => 0x0220,
                0x01A1 | 0x01A3 | 0x01A5 | 0x01A8 | 0x01AD | 0x01B0 | 0x01B4 | 0x01B6 |
                0x01B9 | 0x01BD | 0x01BF | 0x01C5 | 0x01C6 | 0x01C8 | 0x01C9 | 0x01CB |
                0x01CC | 0x01CE | 0x01D0 | 0x01D2 | 0x01D4 | 0x01D6 | 0x01D8 | 0x01DA |
                0x01DC | 0x01DD | 0x01DF | 0x01E1 | 0x01E3 | 0x01E5 | 0x01E7 | 0x01E9 |
                0x01EB | 0x01ED | 0x01EF | 0x01F2 | 0x01F3 | 0x01F5 => ch - 1,
                _ => ch,
            }
        }
        
        // Greek lowercase
        0x03B1..=0x03C9 => ch - 0x20,
        
        // Cyrillic lowercase
        0x0430..=0x044F => ch - 0x20,
        0x0450..=0x045F => ch - 0x50,
        
        // Special cases
        0x00FF => 0x0178,  // ÿ -> Ÿ
        0x0131 => 0x0049,  // ı -> I (Turkish)
        0x017F => 0x0053,  // ſ -> S (long s)
        0x0253 => 0x0181,  // ɓ -> Ɓ
        0x0254 => 0x0186,  // ɔ -> Ɔ
        0x0259 => 0x018F,  // ə -> Ə
        0x025B => 0x0190,  // ɛ -> Ɛ
        0x0260 => 0x0193,  // ɠ -> Ɠ
        0x0263 => 0x0194,  // ɣ -> Ɣ
        0x0268 => 0x0197,  // ɨ -> Ɨ
        0x0269 => 0x0196,  // ɩ -> Ɩ
        0x026F => 0x019C,  // ɯ -> Ɯ
        0x0272 => 0x019D,  // ɲ -> Ɲ
        0x0275 => 0x019F,  // ɵ -> Ɵ
        0x0283 => 0x01A9,  // ʃ -> Ʃ
        0x0288 => 0x01AE,  // ʈ -> Ʈ
        0x028A => 0x01B1,  // ʊ -> Ʊ
        0x028B => 0x01B2,  // ʋ -> Ʋ
        0x0292 => 0x01B7,  // ʒ -> Ʒ
        
        // Default: character maps to itself
        _ => ch,
    }
}

/// Calculate checksum for the upcase table (for validation)
pub fn calculate_upcase_checksum(table: &[u8]) -> u32 {
    let mut checksum = 0u32;
    
    // Process as 32-bit words
    for chunk in table.chunks_exact(4) {
        let value = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        checksum = ((checksum << 31) | (checksum >> 1)) + value;
    }
    
    checksum
}

/// Verify if a character needs case conversion
pub fn needs_upcase(ch: u16) -> bool {
    to_upper_case(ch) != ch
}

/// Compare two filenames using the upcase table
pub fn compare_filenames(name1: &[u16], name2: &[u16]) -> bool {
    if name1.len() != name2.len() {
        return false;
    }
    
    for (&ch1, &ch2) in name1.iter().zip(name2.iter()) {
        if to_upper_case(ch1) != to_upper_case(ch2) {
            return false;
        }
    }
    
    true
}

/// Convert a UTF-8 string to UTF-16LE for exFAT
pub fn utf8_to_utf16le(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Convert UTF-16LE to UTF-8 string
pub fn utf16le_to_utf8(data: &[u16]) -> String {
    String::from_utf16_lossy(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ascii_upcase() {
        assert_eq!(to_upper_case(b'a' as u16), b'A' as u16);
        assert_eq!(to_upper_case(b'z' as u16), b'Z' as u16);
        assert_eq!(to_upper_case(b'A' as u16), b'A' as u16);
        assert_eq!(to_upper_case(b'0' as u16), b'0' as u16);
    }
    
    #[test]
    fn test_latin1_upcase() {
        assert_eq!(to_upper_case(0x00E0), 0x00C0);  // à -> À
        assert_eq!(to_upper_case(0x00F1), 0x00D1);  // ñ -> Ñ
        assert_eq!(to_upper_case(0x00FF), 0x0178);  // ÿ -> Ÿ
    }
    
    #[test]
    fn test_cyrillic_upcase() {
        assert_eq!(to_upper_case(0x0430), 0x0410);  // а -> А
        assert_eq!(to_upper_case(0x044F), 0x042F);  // я -> Я
    }
    
    #[test]
    fn test_filename_comparison() {
        let name1 = utf8_to_utf16le("test.txt");
        let name2 = utf8_to_utf16le("TEST.TXT");
        assert!(compare_filenames(&name1, &name2));
        
        let name3 = utf8_to_utf16le("файл.dat");
        let name4 = utf8_to_utf16le("ФАЙЛ.DAT");
        assert!(compare_filenames(&name3, &name4));
    }
    
    #[test]
    fn test_upcase_table_size() {
        let table = generate_upcase_table();
        assert_eq!(table.len(), 128 * 1024);  // Should be exactly 128KB
    }
}