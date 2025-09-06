// FAT Cluster Chain Management
// Common logic for following and managing cluster chains

use moses_core::MosesError;
use super::super::FatVariant;

/// FAT cluster values with special meanings
pub mod cluster_values {
    pub const FREE_CLUSTER: u32 = 0x00000000;
    pub const RESERVED_CLUSTER: u32 = 0x00000001;
    pub const BAD_CLUSTER_FAT16: u32 = 0x0000FFF7;
    pub const BAD_CLUSTER_FAT32: u32 = 0x0FFFFFF7;
    pub const END_OF_CHAIN_FAT16: u32 = 0x0000FFF8;
    pub const END_OF_CHAIN_FAT32: u32 = 0x0FFFFFF8;
}

/// Read a complete cluster chain
pub fn read_cluster_chain<F: FatVariant>(
    fat: &F,
    fat_data: &[u8],
    start_cluster: u32,
    max_clusters: Option<usize>,
) -> Result<Vec<u32>, MosesError> {
    let mut chain = Vec::new();
    let mut current = start_cluster;
    let max = max_clusters.unwrap_or(usize::MAX);
    
    // Prevent infinite loops
    let mut visited = std::collections::HashSet::new();
    
    while chain.len() < max {
        if !fat.is_valid_cluster(current) {
            break;
        }
        
        // Check for cycles
        if !visited.insert(current) {
            return Err(MosesError::Other("Circular cluster chain detected".to_string()));
        }
        
        chain.push(current);
        
        // Read next cluster from FAT
        let next = fat.read_fat_entry(current, fat_data);
        
        if fat.is_end_of_chain(next) {
            break;
        }
        
        current = next;
    }
    
    Ok(chain)
}

/// Find a free cluster in the FAT
pub fn find_free_cluster<F: FatVariant>(
    fat: &F,
    fat_data: &[u8],
    start_hint: u32,
) -> Option<u32> {
    let max_cluster = fat.max_cluster();
    
    // Try from hint first
    for cluster in start_hint..=max_cluster {
        if fat.read_fat_entry(cluster, fat_data) == cluster_values::FREE_CLUSTER {
            return Some(cluster);
        }
    }
    
    // Wrap around to beginning
    for cluster in 2..start_hint {
        if fat.read_fat_entry(cluster, fat_data) == cluster_values::FREE_CLUSTER {
            return Some(cluster);
        }
    }
    
    None
}

/// Allocate a cluster chain
pub fn allocate_cluster_chain<F: FatVariant>(
    fat: &mut F,
    fat_data: &mut [u8],
    count: usize,
    start_hint: u32,
) -> Result<Vec<u32>, MosesError> {
    let mut chain = Vec::with_capacity(count);
    let mut hint = start_hint;
    
    for i in 0..count {
        let cluster = find_free_cluster(fat, fat_data, hint)
            .ok_or_else(|| MosesError::Other("No free clusters available".to_string()))?;
        
        chain.push(cluster);
        
        // Link to previous cluster
        if i > 0 {
            fat.write_fat_entry(chain[i - 1], cluster, fat_data);
        }
        
        // Mark as end of chain (will be overwritten if not last)
        fat.write_fat_entry(cluster, 0x0FFFFFFF, fat_data);
        
        hint = cluster + 1;
    }
    
    Ok(chain)
}

/// Free a cluster chain
pub fn free_cluster_chain<F: FatVariant>(
    fat: &mut F,
    fat_data: &mut [u8],
    start_cluster: u32,
) -> Result<u32, MosesError> {
    let mut freed = 0;
    let mut current = start_cluster;
    
    // Prevent infinite loops
    let mut visited = std::collections::HashSet::new();
    
    while fat.is_valid_cluster(current) {
        // Check for cycles
        if !visited.insert(current) {
            break;
        }
        
        // Read next before freeing current
        let next = fat.read_fat_entry(current, fat_data);
        
        // Mark as free
        fat.write_fat_entry(current, cluster_values::FREE_CLUSTER, fat_data);
        freed += 1;
        
        if fat.is_end_of_chain(next) {
            break;
        }
        
        current = next;
    }
    
    Ok(freed)
}

/// Extend a cluster chain by allocating new clusters
pub fn extend_cluster_chain<F: FatVariant>(
    fat: &mut F,
    fat_data: &mut [u8],
    last_cluster: u32,
    additional_count: usize,
) -> Result<Vec<u32>, MosesError> {
    let new_clusters = allocate_cluster_chain(fat, fat_data, additional_count, last_cluster + 1)?;
    
    if !new_clusters.is_empty() {
        // Link the old chain to the new clusters
        fat.write_fat_entry(last_cluster, new_clusters[0], fat_data);
    }
    
    Ok(new_clusters)
}

/// Truncate a cluster chain at a specific cluster
pub fn truncate_cluster_chain<F: FatVariant>(
    fat: &mut F,
    fat_data: &mut [u8],
    truncate_at: u32,
) -> Result<u32, MosesError> {
    // Get the next cluster before truncating
    let next = fat.read_fat_entry(truncate_at, fat_data);
    
    // Mark truncate point as end of chain
    fat.write_fat_entry(truncate_at, 0x0FFFFFFF, fat_data);
    
    // Free the rest of the chain
    if fat.is_valid_cluster(next) && !fat.is_end_of_chain(next) {
        free_cluster_chain(fat, fat_data, next)
    } else {
        Ok(0)
    }
}

/// Count clusters in a chain
pub fn count_clusters<F: FatVariant>(
    fat: &F,
    fat_data: &[u8],
    start_cluster: u32,
) -> Result<u32, MosesError> {
    let chain = read_cluster_chain(fat, fat_data, start_cluster, None)?;
    Ok(chain.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct MockFat;
    
    impl FatVariant for MockFat {
        fn fat_bits(&self) -> u8 { 32 }
        fn max_cluster(&self) -> u32 { 0x0FFFFFF6 }
        
        fn read_fat_entry(&self, cluster: u32, fat_data: &[u8]) -> u32 {
            let offset = (cluster * 4) as usize;
            if offset + 4 <= fat_data.len() {
                u32::from_le_bytes([
                    fat_data[offset],
                    fat_data[offset + 1],
                    fat_data[offset + 2],
                    fat_data[offset + 3],
                ])
            } else {
                0
            }
        }
        
        fn write_fat_entry(&mut self, cluster: u32, value: u32, fat_data: &mut [u8]) {
            let offset = (cluster * 4) as usize;
            if offset + 4 <= fat_data.len() {
                fat_data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
    }
    
    #[test]
    fn test_cluster_chain_reading() {
        let fat = MockFat;
        let mut fat_data = vec![0u8; 1024];
        
        // Create a simple chain: 2 -> 3 -> 4 -> END
        fat_data[8..12].copy_from_slice(&3u32.to_le_bytes());
        fat_data[12..16].copy_from_slice(&4u32.to_le_bytes());
        fat_data[16..20].copy_from_slice(&0x0FFFFFFFu32.to_le_bytes());
        
        let chain = read_cluster_chain(&fat, &fat_data, 2, None).unwrap();
        assert_eq!(chain, vec![2, 3, 4]);
    }
}