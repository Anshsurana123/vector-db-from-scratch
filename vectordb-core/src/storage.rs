use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Ordering;
use serde::{Deserialize, Serialize};

use crate::distance::{DistanceMetric, MetricType, get_distance_metric};
use crate::error::{Result, VectorDbError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: u64,
    pub distance: f32,
    pub metadata: Option<serde_json::Value>,
}

/// Helper struct for min-heap / max-heap priority queue ordering.
/// Ord is implemented based on distance.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: u64,
    pub distance: f32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance && self.id == other.id
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Standard float comparison, NaN handled cleanly
        self.distance
            .partial_cmp(&other.distance)
            .unwrap_or(Ordering::Equal)
    }
}

/// Flat contiguous vector storage with id-offset indexing.
#[derive(Debug, Clone)]
pub struct VectorStorage {
    dim: usize,
    data: Vec<f32>,
    id_to_idx: HashMap<u64, usize>,
    idx_to_id: Vec<u64>,
    metadata_store: HashMap<u64, serde_json::Value>,
    deleted: HashSet<u64>,
}

impl VectorStorage {
    pub fn new(dim: usize) -> Self {
        Self {
            dim,
            data: Vec::new(),
            id_to_idx: HashMap::new(),
            idx_to_id: Vec::new(),
            metadata_store: HashMap::new(),
            deleted: HashSet::new(),
        }
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    pub fn len(&self) -> usize {
        self.idx_to_id.len() - self.deleted.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn total_allocated(&self) -> usize {
        self.idx_to_id.len()
    }

    pub fn insert(
        &mut self,
        id: u64,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        if vector.len() != self.dim {
            return Err(VectorDbError::DimensionMismatch {
                expected: self.dim,
                actual: vector.len(),
            });
        }

        if self.id_to_idx.contains_key(&id) && !self.deleted.contains(&id) {
            return Err(VectorDbError::DuplicateId(id));
        }

        if self.deleted.contains(&id) {
            // Re-inserting previously deleted ID: update slot or allocate new
            self.deleted.remove(&id);
            let idx = self.id_to_idx[&id];
            let offset = idx * self.dim;
            self.data[offset..offset + self.dim].copy_from_slice(vector);
            if let Some(meta) = metadata {
                self.metadata_store.insert(id, meta);
            } else {
                self.metadata_store.remove(&id);
            }
            return Ok(());
        }

        let idx = self.idx_to_id.len();
        self.data.extend_from_slice(vector);
        self.id_to_idx.insert(id, idx);
        self.idx_to_id.push(id);

        if let Some(meta) = metadata {
            self.metadata_store.insert(id, meta);
        }

        Ok(())
    }

    pub fn delete(&mut self, id: u64) -> Result<bool> {
        if let Some(&_idx) = self.id_to_idx.get(&id) {
            if self.deleted.insert(id) {
                self.metadata_store.remove(&id);
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn is_deleted(&self, id: u64) -> bool {
        self.deleted.contains(&id)
    }

    pub fn get_vector(&self, id: u64) -> Option<&[f32]> {
        if self.deleted.contains(&id) {
            return None;
        }
        let &idx = self.id_to_idx.get(&id)?;
        let offset = idx * self.dim;
        Some(&self.data[offset..offset + self.dim])
    }

    pub fn get_vector_by_idx(&self, idx: usize) -> Option<&[f32]> {
        if idx >= self.idx_to_id.len() {
            return None;
        }
        let id = self.idx_to_id[idx];
        if self.deleted.contains(&id) {
            return None;
        }
        let offset = idx * self.dim;
        Some(&self.data[offset..offset + self.dim])
    }

    pub fn get_metadata(&self, id: u64) -> Option<&serde_json::Value> {
        if self.deleted.contains(&id) {
            return None;
        }
        self.metadata_store.get(&id)
    }

    pub fn get_id_by_idx(&self, idx: usize) -> Option<u64> {
        self.idx_to_id.get(idx).copied()
    }

    pub fn get_idx_by_id(&self, id: u64) -> Option<usize> {
        if self.deleted.contains(&id) {
            return None;
        }
        self.id_to_idx.get(&id).copied()
    }

    /// Brute-force linear scan search over all non-deleted vectors.
    pub fn search_brute_force(
        &self,
        query: &[f32],
        k: usize,
        metric: MetricType,
    ) -> Result<Vec<SearchResult>> {
        if query.len() != self.dim {
            return Err(VectorDbError::DimensionMismatch {
                expected: self.dim,
                actual: query.len(),
            });
        }

        if k == 0 || self.is_empty() {
            return Ok(Vec::new());
        }

        let dist_fn = get_distance_metric(metric);
        // Max-heap of size k: stores furthest of top-k at peak
        let mut max_heap: BinaryHeap<Candidate> = BinaryHeap::with_capacity(k);

        for (idx, &id) in self.idx_to_id.iter().enumerate() {
            if self.deleted.contains(&id) {
                continue;
            }

            let offset = idx * self.dim;
            let vec_slice = &self.data[offset..offset + self.dim];
            let dist = dist_fn.distance(query, vec_slice);

            if max_heap.len() < k {
                max_heap.push(Candidate { id, distance: dist });
            } else if let Some(top) = max_heap.peek() {
                if dist < top.distance {
                    max_heap.pop();
                    max_heap.push(Candidate { id, distance: dist });
                }
            }
        }

        // Extract and sort results ascending by distance (closest first)
        let mut results = Vec::with_capacity(max_heap.len());
        while let Some(cand) = max_heap.pop() {
            results.push(SearchResult {
                id: cand.id,
                distance: cand.distance,
                metadata: self.metadata_store.get(&cand.id).cloned(),
            });
        }
        results.reverse();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_vector_storage_crud() -> Result<()> {
        let mut storage = VectorStorage::new(3);
        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![4.0, 5.0, 6.0];

        storage.insert(1, &v1, Some(json!({"tag": "a"})))?;
        storage.insert(2, &v2, Some(json!({"tag": "b"})))?;

        assert_eq!(storage.len(), 2);
        assert_eq!(storage.get_vector(1).unwrap(), &v1);
        assert_eq!(storage.get_metadata(1).unwrap()["tag"], "a");

        // Delete v1
        assert!(storage.delete(1)?);
        assert_eq!(storage.len(), 1);
        assert!(storage.get_vector(1).is_none());
        assert!(storage.get_metadata(1).is_none());

        Ok(())
    }

    #[test]
    fn test_brute_force_search() -> Result<()> {
        let mut storage = VectorStorage::new(2);
        storage.insert(1, &[0.0, 0.0], None)?;
        storage.insert(2, &[1.0, 0.0], None)?;
        storage.insert(3, &[10.0, 0.0], None)?;

        let results = storage.search_brute_force(&[0.1, 0.0], 2, MetricType::L2)?;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, 1);
        assert_eq!(results[1].id, 2);

        Ok(())
    }
}
