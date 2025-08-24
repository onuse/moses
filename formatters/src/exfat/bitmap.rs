// exFAT allocation bitmap management
// Unlike FAT16/32 which use FAT chains, exFAT uses a bitmap for cluster allocation

use moses_core::MosesError;

/// exFAT allocation bitmap
pub struct ExFatBitmap {
    data: Vec<u8>,
    cluster_count: u32,
}

impl ExFatBitmap {
    /// Create a new bitmap for the given number of clusters
    pub fn new(cluster_count: u32) -> Self {
        let bytes_needed = ((cluster_count + 7) / 8) as usize;
        Self {
            data: vec![0u8; bytes_needed],
            cluster_count,
        }
    }
    
    /// Create from existing bitmap data
    pub fn from_bytes(data: Vec<u8>, cluster_count: u32) -> Self {
        Self {
            data,
            cluster_count,
        }
    }
    
    /// Check if a cluster is allocated
    pub fn is_allocated(&self, cluster: u32) -> bool {
        if cluster >= self.cluster_count {
            return false;
        }
        
        let byte_index = (cluster / 8) as usize;
        let bit_index = (cluster % 8) as u8;
        
        (self.data[byte_index] & (1 << bit_index)) != 0
    }
    
    /// Mark a cluster as allocated
    pub fn set_allocated(&mut self, cluster: u32) {
        if cluster >= self.cluster_count {
            return;
        }
        
        let byte_index = (cluster / 8) as usize;
        let bit_index = (cluster % 8) as u8;
        
        self.data[byte_index] |= 1 << bit_index;
    }
    
    /// Mark a cluster as free
    pub fn set_free(&mut self, cluster: u32) {
        if cluster >= self.cluster_count {
            return;
        }
        
        let byte_index = (cluster / 8) as usize;
        let bit_index = (cluster % 8) as u8;
        
        self.data[byte_index] &= !(1 << bit_index);
    }
    
    /// Find the next free cluster starting from a given position
    pub fn find_free_cluster(&self, start_from: u32) -> Option<u32> {
        for cluster in start_from..self.cluster_count {
            if !self.is_allocated(cluster) {
                return Some(cluster);
            }
        }
        
        // Wrap around and search from beginning
        for cluster in 2..start_from {
            if !self.is_allocated(cluster) {
                return Some(cluster);
            }
        }
        
        None
    }
    
    /// Allocate a contiguous range of clusters
    pub fn allocate_contiguous(&mut self, count: u32, start_hint: u32) -> Option<u32> {
        let mut search_start = start_hint.max(2);  // Clusters start at 2
        
        'search: loop {
            // Check if we have enough contiguous free clusters starting here
            let mut found_count = 0;
            for i in 0..count {
                let cluster = search_start + i;
                if cluster >= self.cluster_count || self.is_allocated(cluster) {
                    // Not enough space or allocated cluster found
                    search_start = cluster + 1;
                    if search_start >= self.cluster_count {
                        // Wrap around
                        if start_hint <= 2 {
                            return None;  // We've searched everything
                        }
                        search_start = 2;
                    }
                    continue 'search;
                }
                found_count += 1;
            }
            
            if found_count == count {
                // Found contiguous range, allocate it
                for i in 0..count {
                    self.set_allocated(search_start + i);
                }
                return Some(search_start);
            }
            
            // Shouldn't reach here
            break;
        }
        
        None
    }
    
    /// Count the number of free clusters
    pub fn count_free(&self) -> u32 {
        let mut free_count = 0;
        for cluster in 2..self.cluster_count {
            if !self.is_allocated(cluster) {
                free_count += 1;
            }
        }
        free_count
    }
    
    /// Get the raw bitmap data
    pub fn to_bytes(&self) -> Vec<u8> {
        self.data.clone()
    }
    
    /// Calculate bitmap checksum (for validation)
    pub fn checksum(&self) -> u32 {
        let mut sum = 0u32;
        for &byte in &self.data {
            sum = sum.wrapping_add(byte as u32);
        }
        sum
    }
}

/// Bitmap allocation manager for exFAT
pub struct BitmapAllocator {
    bitmap: ExFatBitmap,
    next_free_hint: u32,
}

impl BitmapAllocator {
    /// Create a new allocator
    pub fn new(cluster_count: u32) -> Self {
        Self {
            bitmap: ExFatBitmap::new(cluster_count),
            next_free_hint: 2,  // Start searching from cluster 2
        }
    }
    
    /// Load from existing bitmap
    pub fn from_bitmap(bitmap: ExFatBitmap) -> Self {
        Self {
            next_free_hint: 2,
            bitmap,
        }
    }
    
    /// Allocate a single cluster
    pub fn allocate_cluster(&mut self) -> Result<u32, MosesError> {
        if let Some(cluster) = self.bitmap.find_free_cluster(self.next_free_hint) {
            self.bitmap.set_allocated(cluster);
            self.next_free_hint = cluster + 1;
            Ok(cluster)
        } else {
            Err(MosesError::Other("No free clusters available".to_string()))
        }
    }
    
    /// Allocate multiple contiguous clusters
    pub fn allocate_contiguous(&mut self, count: u32) -> Result<u32, MosesError> {
        if let Some(start) = self.bitmap.allocate_contiguous(count, self.next_free_hint) {
            self.next_free_hint = start + count;
            Ok(start)
        } else {
            Err(MosesError::Other(format!("Cannot allocate {} contiguous clusters", count)))
        }
    }
    
    /// Free a cluster
    pub fn free_cluster(&mut self, cluster: u32) {
        self.bitmap.set_free(cluster);
        if cluster < self.next_free_hint {
            self.next_free_hint = cluster;
        }
    }
    
    /// Free a contiguous range of clusters
    pub fn free_contiguous(&mut self, start: u32, count: u32) {
        for i in 0..count {
            self.free_cluster(start + i);
        }
    }
    
    /// Get free cluster count
    pub fn free_clusters(&self) -> u32 {
        self.bitmap.count_free()
    }
    
    /// Get the bitmap for writing to disk
    pub fn get_bitmap(&self) -> &ExFatBitmap {
        &self.bitmap
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bitmap_operations() {
        let mut bitmap = ExFatBitmap::new(100);
        
        // Initially all clusters should be free
        assert!(!bitmap.is_allocated(5));
        
        // Allocate a cluster
        bitmap.set_allocated(5);
        assert!(bitmap.is_allocated(5));
        
        // Free it
        bitmap.set_free(5);
        assert!(!bitmap.is_allocated(5));
    }
    
    #[test]
    fn test_find_free() {
        let mut bitmap = ExFatBitmap::new(10);
        
        // Allocate some clusters
        bitmap.set_allocated(2);
        bitmap.set_allocated(3);
        bitmap.set_allocated(5);
        
        // Find next free starting from 2
        assert_eq!(bitmap.find_free_cluster(2), Some(4));
        
        // Find next free starting from 5
        assert_eq!(bitmap.find_free_cluster(5), Some(6));
    }
    
    #[test]
    fn test_contiguous_allocation() {
        let mut bitmap = ExFatBitmap::new(20);
        
        // Allocate some scattered clusters
        bitmap.set_allocated(3);
        bitmap.set_allocated(7);
        
        // Try to allocate 3 contiguous clusters
        let start = bitmap.allocate_contiguous(3, 2);
        assert_eq!(start, Some(4));  // Should find space at 4,5,6
        
        // Verify they're allocated
        assert!(bitmap.is_allocated(4));
        assert!(bitmap.is_allocated(5));
        assert!(bitmap.is_allocated(6));
    }
}