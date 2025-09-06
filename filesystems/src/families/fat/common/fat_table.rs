// FAT table writer abstraction for FAT16 and FAT32
// Handles the differences in entry sizes and special values

use std::io::{Write, Seek, SeekFrom, Result as IoResult};
use super::constants::*;

/// Trait for writing FAT table entries
pub trait FatTableWriter {
    /// Write a FAT entry at the specified cluster index
    fn write_entry(&mut self, cluster: u32, value: u32) -> IoResult<()>;
    
    /// Mark a cluster as end-of-chain
    fn mark_end_of_chain(&mut self, cluster: u32) -> IoResult<()>;
    
    /// Mark a cluster as bad
    fn mark_bad_cluster(&mut self, cluster: u32) -> IoResult<()>;
    
    /// Initialize the FAT table with standard entries
    fn initialize_fat(&mut self) -> IoResult<()>;
}

/// FAT16 table writer
pub struct Fat16TableWriter<W: Write + Seek> {
    writer: W,
    fat_start_offset: u64,
}

impl<W: Write + Seek> Fat16TableWriter<W> {
    pub fn new(writer: W, fat_start_offset: u64) -> Self {
        Self {
            writer,
            fat_start_offset,
        }
    }
}

impl<W: Write + Seek> FatTableWriter for Fat16TableWriter<W> {
    fn write_entry(&mut self, cluster: u32, value: u32) -> IoResult<()> {
        // FAT16 uses 16-bit entries
        let offset = self.fat_start_offset + (cluster as u64 * 2);
        self.writer.seek(SeekFrom::Start(offset))?;
        self.writer.write_all(&(value as u16).to_le_bytes())?;
        Ok(())
    }
    
    fn mark_end_of_chain(&mut self, cluster: u32) -> IoResult<()> {
        self.write_entry(cluster, FAT16_EOC as u32)
    }
    
    fn mark_bad_cluster(&mut self, cluster: u32) -> IoResult<()> {
        self.write_entry(cluster, FAT16_BAD as u32)
    }
    
    fn initialize_fat(&mut self) -> IoResult<()> {
        // First two entries are reserved
        self.write_entry(0, 0xFFF8)?;  // Media descriptor in first entry
        self.write_entry(1, 0xFFFF)?;  // End of chain marker
        Ok(())
    }
}

/// FAT32 table writer
pub struct Fat32TableWriter<W: Write + Seek> {
    writer: W,
    fat_start_offset: u64,
}

impl<W: Write + Seek> Fat32TableWriter<W> {
    pub fn new(writer: W, fat_start_offset: u64) -> Self {
        Self {
            writer,
            fat_start_offset,
        }
    }
    
    /// Read existing FAT entry (needed for preserving upper 4 bits)
    #[allow(dead_code)]
    fn read_entry(&mut self, _cluster: u32) -> IoResult<u32> {
        // This is a simplification - in real implementation we'd need a separate reader
        // For formatting, we can assume the entry starts as zero
        Ok(0)
    }
}

impl<W: Write + Seek> FatTableWriter for Fat32TableWriter<W> {
    fn write_entry(&mut self, cluster: u32, value: u32) -> IoResult<()> {
        // FAT32 uses 28-bit entries (upper 4 bits are reserved)
        let offset = self.fat_start_offset + (cluster as u64 * 4);
        self.writer.seek(SeekFrom::Start(offset))?;
        
        // Mask to 28 bits and preserve upper 4 bits
        let masked_value = value & 0x0FFFFFFF;
        
        // In a real implementation, we should read the existing value
        // and preserve the upper 4 bits. For formatting, we can assume
        // they're zero.
        self.writer.write_all(&masked_value.to_le_bytes())?;
        Ok(())
    }
    
    fn mark_end_of_chain(&mut self, cluster: u32) -> IoResult<()> {
        self.write_entry(cluster, FAT32_EOC)
    }
    
    fn mark_bad_cluster(&mut self, cluster: u32) -> IoResult<()> {
        self.write_entry(cluster, FAT32_BAD)
    }
    
    fn initialize_fat(&mut self) -> IoResult<()> {
        // First two entries are reserved
        self.write_entry(0, 0x0FFFFFF8)?;  // Media descriptor in first entry
        self.write_entry(1, 0x0FFFFFFF)?;  // End of chain marker
        
        // For FAT32, cluster 2 is typically the root directory
        // Mark it as end-of-chain
        self.write_entry(2, FAT32_EOC)?;
        
        Ok(())
    }
}

/// Helper function to write FAT16 tables
pub fn write_fat16_tables<W: Write + Seek>(
    writer: &mut W,
    fat_offset: u64,
    sectors_per_fat: u16,
    num_fats: u8,
) -> IoResult<()> {
    for fat_num in 0..num_fats {
        let this_fat_offset = fat_offset + (fat_num as u64 * sectors_per_fat as u64 * 512);
        
        // Seek to FAT start
        writer.seek(SeekFrom::Start(this_fat_offset))?;
        
        // Initialize FAT with zeros
        let fat_size = sectors_per_fat as usize * 512;
        let zeros = vec![0u8; fat_size];
        writer.write_all(&zeros)?;
        
        // Write reserved entries
        writer.seek(SeekFrom::Start(this_fat_offset))?;
        writer.write_all(&[0xF8, 0xFF])?;  // First entry: media descriptor
        writer.write_all(&[0xFF, 0xFF])?;  // Second entry: end of chain
    }
    
    Ok(())
}

/// Helper function to write FAT32 tables
pub fn write_fat32_tables<W: Write + Seek>(
    writer: &mut W,
    fat_offset: u64,
    sectors_per_fat: u32,
    num_fats: u8,
) -> IoResult<()> {
    for fat_num in 0..num_fats {
        let this_fat_offset = fat_offset + (fat_num as u64 * sectors_per_fat as u64 * 512);
        
        // Seek to FAT start
        writer.seek(SeekFrom::Start(this_fat_offset))?;
        
        // Initialize FAT with zeros (this could be optimized for large FATs)
        let fat_size = sectors_per_fat as usize * 512;
        let zeros = vec![0u8; fat_size.min(1024 * 1024)];  // Write in 1MB chunks
        let mut remaining = fat_size;
        while remaining > 0 {
            let chunk_size = remaining.min(zeros.len());
            writer.write_all(&zeros[..chunk_size])?;
            remaining -= chunk_size;
        }
        
        // Write reserved entries
        writer.seek(SeekFrom::Start(this_fat_offset))?;
        writer.write_all(&[0xF8, 0xFF, 0xFF, 0x0F])?;  // First entry: media descriptor
        writer.write_all(&[0xFF, 0xFF, 0xFF, 0x0F])?;  // Second entry: end of chain
        writer.write_all(&[0xF8, 0xFF, 0xFF, 0x0F])?;  // Root directory cluster (2)
    }
    
    Ok(())
}