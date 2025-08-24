// NTFS Reparse Point Support
// Phase 2.5: Handle symbolic links, junctions, and mount points

use moses_core::MosesError;
use log::trace;

// Reparse point tags
pub const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xA0000003;
pub const IO_REPARSE_TAG_SYMLINK: u32 = 0xA000000C;
pub const IO_REPARSE_TAG_APPEXECLINK: u32 = 0x8000001B;

/// Reparse point header
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ReparsePointHeader {
    pub reparse_tag: u32,           // Type of reparse point
    pub reparse_data_length: u16,   // Length of reparse data
    pub reserved: u16,               // Reserved
}

/// Mount point/junction reparse data
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MountPointReparseData {
    pub substitute_name_offset: u16,
    pub substitute_name_length: u16,
    pub print_name_offset: u16,
    pub print_name_length: u16,
}

/// Symbolic link reparse data
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct SymbolicLinkReparseData {
    pub substitute_name_offset: u16,
    pub substitute_name_length: u16,
    pub print_name_offset: u16,
    pub print_name_length: u16,
    pub flags: u32,  // 0 = absolute, 1 = relative
}

/// Parsed reparse point information
#[derive(Debug, Clone)]
pub enum ReparsePoint {
    MountPoint {
        substitute_name: String,
        print_name: String,
    },
    SymbolicLink {
        substitute_name: String,
        print_name: String,
        is_relative: bool,
    },
    AppExecLink {
        target: String,
        app_id: String,
    },
    Unknown {
        tag: u32,
        data: Vec<u8>,
    },
}

/// Check if a file has a reparse point
pub fn is_reparse_point(file_attributes: u32) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    file_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Parse a reparse point attribute
pub fn parse_reparse_point(data: &[u8]) -> Result<ReparsePoint, MosesError> {
    if data.len() < std::mem::size_of::<ReparsePointHeader>() {
        return Err(MosesError::Other("Reparse point data too small".to_string()));
    }
    
    let header = unsafe {
        std::ptr::read_unaligned(data.as_ptr() as *const ReparsePointHeader)
    };
    
    let tag = header.reparse_tag;
    let data_offset = std::mem::size_of::<ReparsePointHeader>();
    
    trace!("Parsing reparse point with tag 0x{:08X}", tag);
    
    match tag {
        IO_REPARSE_TAG_MOUNT_POINT => {
            parse_mount_point(&data[data_offset..])
        }
        IO_REPARSE_TAG_SYMLINK => {
            parse_symbolic_link(&data[data_offset..])
        }
        IO_REPARSE_TAG_APPEXECLINK => {
            parse_appexec_link(&data[data_offset..])
        }
        _ => {
            Ok(ReparsePoint::Unknown {
                tag,
                data: data[data_offset..].to_vec(),
            })
        }
    }
}

/// Parse mount point/junction reparse data
fn parse_mount_point(data: &[u8]) -> Result<ReparsePoint, MosesError> {
    if data.len() < std::mem::size_of::<MountPointReparseData>() {
        return Err(MosesError::Other("Mount point data too small".to_string()));
    }
    
    let mp_data = unsafe {
        std::ptr::read_unaligned(data.as_ptr() as *const MountPointReparseData)
    };
    
    let path_buffer_offset = std::mem::size_of::<MountPointReparseData>();
    
    // Parse substitute name (actual target)
    let sub_name_offset = path_buffer_offset + mp_data.substitute_name_offset as usize;
    let sub_name_length = mp_data.substitute_name_length as usize;
    
    let substitute_name = if sub_name_offset + sub_name_length <= data.len() {
        parse_utf16le_string(&data[sub_name_offset..sub_name_offset + sub_name_length])?
    } else {
        String::new()
    };
    
    // Parse print name (display name)
    let print_name_offset = path_buffer_offset + mp_data.print_name_offset as usize;
    let print_name_length = mp_data.print_name_length as usize;
    
    let print_name = if print_name_offset + print_name_length <= data.len() {
        parse_utf16le_string(&data[print_name_offset..print_name_offset + print_name_length])?
    } else {
        substitute_name.clone()
    };
    
    Ok(ReparsePoint::MountPoint {
        substitute_name,
        print_name,
    })
}

/// Parse symbolic link reparse data
fn parse_symbolic_link(data: &[u8]) -> Result<ReparsePoint, MosesError> {
    if data.len() < std::mem::size_of::<SymbolicLinkReparseData>() {
        return Err(MosesError::Other("Symbolic link data too small".to_string()));
    }
    
    let sym_data = unsafe {
        std::ptr::read_unaligned(data.as_ptr() as *const SymbolicLinkReparseData)
    };
    
    let path_buffer_offset = std::mem::size_of::<SymbolicLinkReparseData>();
    
    // Parse substitute name
    let sub_name_offset = path_buffer_offset + sym_data.substitute_name_offset as usize;
    let sub_name_length = sym_data.substitute_name_length as usize;
    
    let substitute_name = if sub_name_offset + sub_name_length <= data.len() {
        parse_utf16le_string(&data[sub_name_offset..sub_name_offset + sub_name_length])?
    } else {
        String::new()
    };
    
    // Parse print name
    let print_name_offset = path_buffer_offset + sym_data.print_name_offset as usize;
    let print_name_length = sym_data.print_name_length as usize;
    
    let print_name = if print_name_offset + print_name_length <= data.len() {
        parse_utf16le_string(&data[print_name_offset..print_name_offset + print_name_length])?
    } else {
        substitute_name.clone()
    };
    
    Ok(ReparsePoint::SymbolicLink {
        substitute_name,
        print_name,
        is_relative: sym_data.flags & 1 != 0,
    })
}

/// Parse AppExecLink reparse data (Windows Store apps)
fn parse_appexec_link(data: &[u8]) -> Result<ReparsePoint, MosesError> {
    // AppExecLink has a simple structure: multiple null-terminated UTF-16 strings
    let mut strings = Vec::new();
    let mut offset = 0;
    
    while offset < data.len() {
        // Find next null terminator
        let mut end = offset;
        while end + 1 < data.len() {
            if data[end] == 0 && data[end + 1] == 0 {
                break;
            }
            end += 2;
        }
        
        if end > offset {
            if let Ok(s) = parse_utf16le_string(&data[offset..end]) {
                strings.push(s);
            }
        }
        
        offset = end + 2;
        if offset >= data.len() || strings.len() >= 3 {
            break;
        }
    }
    
    Ok(ReparsePoint::AppExecLink {
        target: strings.get(0).cloned().unwrap_or_default(),
        app_id: strings.get(2).cloned().unwrap_or_default(),
    })
}

/// Convert a reparse point substitute name to a usable path
pub fn resolve_substitute_name(substitute_name: &str) -> String {
    // Remove the \??\ prefix if present
    if substitute_name.starts_with("\\??\\") {
        substitute_name[4..].to_string()
    } else {
        substitute_name.to_string()
    }
}

/// Parse UTF-16LE string
fn parse_utf16le_string(data: &[u8]) -> Result<String, MosesError> {
    if data.len() % 2 != 0 {
        return Err(MosesError::Other("Invalid UTF-16 string length".to_string()));
    }
    
    let utf16_chars: Vec<u16> = data
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    
    String::from_utf16(&utf16_chars)
        .map_err(|_| MosesError::Other("Invalid UTF-16 string".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_reparse_point_detection() {
        // Normal file
        assert!(!is_reparse_point(0x20));
        
        // Reparse point
        assert!(is_reparse_point(0x400));
        
        // Reparse point with other attributes
        assert!(is_reparse_point(0x420));
    }
    
    #[test]
    fn test_substitute_name_resolution() {
        // Windows-style path
        assert_eq!(
            resolve_substitute_name("\\??\\C:\\Users\\Test"),
            "C:\\Users\\Test"
        );
        
        // Already resolved path
        assert_eq!(
            resolve_substitute_name("C:\\Users\\Test"),
            "C:\\Users\\Test"
        );
        
        // UNC path
        assert_eq!(
            resolve_substitute_name("\\??\\UNC\\Server\\Share"),
            "UNC\\Server\\Share"
        );
    }
    
    #[test]
    fn test_reparse_tag_values() {
        // Verify known tag values
        assert_eq!(IO_REPARSE_TAG_MOUNT_POINT, 0xA0000003);
        assert_eq!(IO_REPARSE_TAG_SYMLINK, 0xA000000C);
        assert_eq!(IO_REPARSE_TAG_APPEXECLINK, 0x8000001B);
    }
}