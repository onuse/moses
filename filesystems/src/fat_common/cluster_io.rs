// Shared cluster I/O operations for FAT family filesystems
// Provides common cluster reading/writing functionality

use std::io::{Read, Write, Seek, SeekFrom};
use moses_core::MosesError;
use log::trace;

/// Calculate the byte offset of a cluster in the data region
/// Works for FAT16, FAT32, and exFAT
pub fn cluster_to_offset(
    cluster: u32,
    sectors_per_cluster: u32,
    bytes_per_sector: u32,
    data_start_offset: u64,
) -> u64 {
    // Clusters are numbered from 2 in FAT filesystems
    let cluster_offset = (cluster - 2) as u64 * sectors_per_cluster as u64 * bytes_per_sector as u64;
    data_start_offset + cluster_offset
}

/// Read a single cluster from the device
pub fn read_cluster<R: Read + Seek>(
    reader: &mut R,
    cluster: u32,
    sectors_per_cluster: u32,
    bytes_per_sector: u32,
    data_start_offset: u64,
) -> Result<Vec<u8>, MosesError> {
    if cluster < 2 {
        return Err(MosesError::Other(format!("Invalid cluster number: {}", cluster)));
    }
    
    let offset = cluster_to_offset(cluster, sectors_per_cluster, bytes_per_sector, data_start_offset);
    let cluster_size = sectors_per_cluster * bytes_per_sector;
    
    trace!("Reading cluster {} at offset {:#x}, size: {} bytes", cluster, offset, cluster_size);
    
    reader.seek(SeekFrom::Start(offset))?;
    let mut buffer = vec![0u8; cluster_size as usize];
    reader.read_exact(&mut buffer)?;
    
    Ok(buffer)
}

/// Write a single cluster to the device
pub fn write_cluster<W: Write + Seek>(
    writer: &mut W,
    cluster: u32,
    data: &[u8],
    sectors_per_cluster: u32,
    bytes_per_sector: u32,
    data_start_offset: u64,
) -> Result<(), MosesError> {
    if cluster < 2 {
        return Err(MosesError::Other(format!("Invalid cluster number: {}", cluster)));
    }
    
    let cluster_size = (sectors_per_cluster * bytes_per_sector) as usize;
    if data.len() > cluster_size {
        return Err(MosesError::Other(format!(
            "Data size {} exceeds cluster size {}", 
            data.len(), 
            cluster_size
        )));
    }
    
    let offset = cluster_to_offset(cluster, sectors_per_cluster, bytes_per_sector, data_start_offset);
    
    trace!("Writing cluster {} at offset {:#x}, size: {} bytes", cluster, offset, data.len());
    
    writer.seek(SeekFrom::Start(offset))?;
    writer.write_all(data)?;
    
    // Pad with zeros if data is smaller than cluster
    if data.len() < cluster_size {
        let padding = vec![0u8; cluster_size - data.len()];
        writer.write_all(&padding)?;
    }
    
    Ok(())
}

/// Read multiple clusters (for following cluster chains)
pub fn read_cluster_chain<R: Read + Seek>(
    reader: &mut R,
    clusters: &[u32],
    sectors_per_cluster: u32,
    bytes_per_sector: u32,
    data_start_offset: u64,
) -> Result<Vec<u8>, MosesError> {
    let cluster_size = (sectors_per_cluster * bytes_per_sector) as usize;
    let mut result = Vec::with_capacity(clusters.len() * cluster_size);
    
    for &cluster in clusters {
        let data = read_cluster(
            reader,
            cluster,
            sectors_per_cluster,
            bytes_per_sector,
            data_start_offset
        )?;
        result.extend_from_slice(&data);
    }
    
    Ok(result)
}

/// Write data across multiple clusters
pub fn write_cluster_chain<W: Write + Seek>(
    writer: &mut W,
    clusters: &[u32],
    data: &[u8],
    sectors_per_cluster: u32,
    bytes_per_sector: u32,
    data_start_offset: u64,
) -> Result<(), MosesError> {
    let cluster_size = (sectors_per_cluster * bytes_per_sector) as usize;
    let mut offset = 0;
    
    for &cluster in clusters {
        let end = std::cmp::min(offset + cluster_size, data.len());
        let chunk = &data[offset..end];
        
        write_cluster(
            writer,
            cluster,
            chunk,
            sectors_per_cluster,
            bytes_per_sector,
            data_start_offset
        )?;
        
        offset = end;
        if offset >= data.len() {
            break;
        }
    }
    
    if offset < data.len() {
        return Err(MosesError::Other(
            "Not enough clusters to write all data".to_string()
        ));
    }
    
    Ok(())
}

/// Calculate how many clusters are needed for a given size
pub fn clusters_needed(size: u64, bytes_per_cluster: u32) -> u32 {
    ((size + bytes_per_cluster as u64 - 1) / bytes_per_cluster as u64) as u32
}

/// Check if a cluster number is valid
pub fn is_valid_cluster(cluster: u32, total_clusters: u32) -> bool {
    cluster >= 2 && cluster < total_clusters + 2
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cluster_to_offset() {
        // Test with typical FAT32 values
        let offset = cluster_to_offset(2, 8, 512, 0x100000);
        assert_eq!(offset, 0x100000);  // First cluster starts at data region
        
        let offset = cluster_to_offset(3, 8, 512, 0x100000);
        assert_eq!(offset, 0x101000);  // Second cluster is 4KB later
    }
    
    #[test]
    fn test_clusters_needed() {
        assert_eq!(clusters_needed(0, 4096), 0);
        assert_eq!(clusters_needed(1, 4096), 1);
        assert_eq!(clusters_needed(4096, 4096), 1);
        assert_eq!(clusters_needed(4097, 4096), 2);
        assert_eq!(clusters_needed(8192, 4096), 2);
    }
}