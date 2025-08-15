// Endianness handling for ext4 structures
// CRITICAL: Everything in ext4 is little-endian!

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write};

/// Trait for types that can be read/written in ext4 little-endian format
pub trait Ext4Endian: Sized {
    fn write_le<W: Write>(&self, writer: &mut W) -> io::Result<()>;
    fn read_le<R: Read>(reader: &mut R) -> io::Result<Self>;
}

// Implement for primitive types
impl Ext4Endian for u8 {
    fn write_le<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u8(*self)
    }
    
    fn read_le<R: Read>(reader: &mut R) -> io::Result<Self> {
        reader.read_u8()
    }
}

impl Ext4Endian for u16 {
    fn write_le<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u16::<LittleEndian>(*self)
    }
    
    fn read_le<R: Read>(reader: &mut R) -> io::Result<Self> {
        reader.read_u16::<LittleEndian>()
    }
}

impl Ext4Endian for u32 {
    fn write_le<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u32::<LittleEndian>(*self)
    }
    
    fn read_le<R: Read>(reader: &mut R) -> io::Result<Self> {
        reader.read_u32::<LittleEndian>()
    }
}

impl Ext4Endian for u64 {
    fn write_le<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u64::<LittleEndian>(*self)
    }
    
    fn read_le<R: Read>(reader: &mut R) -> io::Result<Self> {
        reader.read_u64::<LittleEndian>()
    }
}

impl Ext4Endian for i16 {
    fn write_le<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_i16::<LittleEndian>(*self)
    }
    
    fn read_le<R: Read>(reader: &mut R) -> io::Result<Self> {
        reader.read_i16::<LittleEndian>()
    }
}

impl Ext4Endian for i32 {
    fn write_le<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_i32::<LittleEndian>(*self)
    }
    
    fn read_le<R: Read>(reader: &mut R) -> io::Result<Self> {
        reader.read_i32::<LittleEndian>()
    }
}

impl Ext4Endian for i64 {
    fn write_le<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_i64::<LittleEndian>(*self)
    }
    
    fn read_le<R: Read>(reader: &mut R) -> io::Result<Self> {
        reader.read_i64::<LittleEndian>()
    }
}

/// Convert a structure to little-endian bytes
pub fn to_le_bytes<T>(value: &T) -> Vec<u8> 
where 
    T: Sized 
{
    let size = std::mem::size_of::<T>();
    let mut bytes = vec![0u8; size];
    
    unsafe {
        let src = value as *const T as *const u8;
        let dst = bytes.as_mut_ptr();
        std::ptr::copy_nonoverlapping(src, dst, size);
    }
    
    bytes
}

/// Read a structure from little-endian bytes
pub fn from_le_bytes<T>(bytes: &[u8]) -> Result<T, String> 
where 
    T: Sized + Default
{
    let size = std::mem::size_of::<T>();
    if bytes.len() < size {
        return Err(format!("Not enough bytes: need {}, got {}", size, bytes.len()));
    }
    
    let mut value = T::default();
    unsafe {
        let src = bytes.as_ptr();
        let dst = &mut value as *mut T as *mut u8;
        std::ptr::copy_nonoverlapping(src, dst, size);
    }
    
    Ok(value)
}

/// Macro to implement write_le for structures
#[macro_export]
macro_rules! impl_write_le {
    ($struct:ty, $($field:ident),+) => {
        impl $struct {
            pub fn write_le<W: Write>(&self, writer: &mut W) -> io::Result<()> {
                $(
                    self.$field.write_le(writer)?;
                )+
                Ok(())
            }
        }
    };
}

/// Macro to implement read_le for structures
#[macro_export]
macro_rules! impl_read_le {
    ($struct:ty, $($field:ident: $type:ty),+) => {
        impl $struct {
            pub fn read_le<R: Read>(reader: &mut R) -> io::Result<Self> {
                Ok(Self {
                    $(
                        $field: <$type>::read_le(reader)?,
                    )+
                })
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_u32_endian() {
        let value: u32 = 0x12345678;
        let mut buffer = Vec::new();
        
        value.write_le(&mut buffer).unwrap();
        assert_eq!(buffer, vec![0x78, 0x56, 0x34, 0x12]);
        
        let mut reader = &buffer[..];
        let read_value = u32::read_le(&mut reader).unwrap();
        assert_eq!(read_value, value);
    }
    
    #[test]
    fn test_u16_endian() {
        let value: u16 = 0xEF53; // EXT4_SUPER_MAGIC
        let mut buffer = Vec::new();
        
        value.write_le(&mut buffer).unwrap();
        assert_eq!(buffer, vec![0x53, 0xEF]);
        
        let mut reader = &buffer[..];
        let read_value = u16::read_le(&mut reader).unwrap();
        assert_eq!(read_value, value);
    }
}