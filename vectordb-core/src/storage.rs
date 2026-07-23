use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use serde::{Deserialize, Serialize};

use crate::distance::{MetricType, get_distance_metric};
use crate::error::{Result, VectorDbError};

/// Search result holding the vector ID, metric distance score, and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: u64,
    pub distance: f32,
    pub metadata: Option<serde_json::Value>,
}

impl PartialEq for SearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && (self.distance - other.distance).abs() < 1e-6
    }
}

impl Eq for SearchResult {}

#[derive(Debug, Clone, Copy)]
struct Candidate {
    id: u64,
    distance: f32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.distance == other.distance
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

        if self.deleted.remove(&id) {
            let idx = self.id_to_idx[&id];
            let start = idx * self.dim;
            let end = start + self.dim;
            self.data[start..end].copy_from_slice(vector);
        } else {
            let idx = self.idx_to_id.len();
            self.id_to_idx.insert(id, idx);
            self.idx_to_id.push(id);
            self.data.extend_from_slice(vector);
        }

        if let Some(meta) = metadata {
            self.metadata_store.insert(id, meta);
        } else {
            self.metadata_store.remove(&id);
        }

        Ok(())
    }

    pub fn delete(&mut self, id: u64) -> Result<bool> {
        if self.id_to_idx.contains_key(&id) && !self.deleted.contains(&id) {
            self.deleted.insert(id);
            self.metadata_store.remove(&id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn is_deleted(&self, id: u64) -> bool {
        self.deleted.contains(&id)
    }

    pub fn get_vector(&self, id: u64) -> Option<&[f32]> {
        if self.deleted.contains(&id) {
            return None;
        }
        let &idx = self.id_to_idx.get(&id)?;
        let start = idx * self.dim;
        let end = start + self.dim;
        Some(&self.data[start..end])
    }

    pub fn get_idx_by_id(&self, id: u64) -> Option<usize> {
        if self.deleted.contains(&id) {
            return None;
        }
        self.id_to_idx.get(&id).copied()
    }

    pub fn get_vector_by_idx(&self, idx: usize) -> Option<&[f32]> {
        if idx >= self.idx_to_id.len() {
            return None;
        }
        let id = self.idx_to_id[idx];
        if self.deleted.contains(&id) {
            return None;
        }
        let start = idx * self.dim;
        let end = start + self.dim;
        Some(&self.data[start..end])
    }

    pub fn get_metadata(&self, id: u64) -> Option<&serde_json::Value> {
        if self.deleted.contains(&id) {
            return None;
        }
        self.metadata_store.get(&id)
    }

    pub fn search_brute_force(
        &self,
        query: &[f32],
        k: usize,
        metric_type: MetricType,
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

        let metric = get_distance_metric(metric_type);
        let mut heap = BinaryHeap::with_capacity(k);

        for (idx, &id) in self.idx_to_id.iter().enumerate() {
            if self.deleted.contains(&id) {
                continue;
            }

            let start = idx * self.dim;
            let end = start + self.dim;
            let vec_slice = &self.data[start..end];
            let dist = metric.distance(query, vec_slice);

            let cand = Candidate { id, distance: dist };

            if heap.len() < k {
                heap.push(cand);
            } else if let Some(top) = heap.peek() {
                if cand.distance < top.distance {
                    heap.pop();
                    heap.push(cand);
                }
            }
        }

        let mut results: Vec<SearchResult> = heap
            .into_sorted_vec()
            .into_iter()
            .map(|cand| SearchResult {
                id: cand.id,
                distance: cand.distance,
                metadata: self.metadata_store.get(&cand.id).cloned(),
            })
            .collect();

        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
        Ok(results)
    }

    pub fn raw_data(&self) -> &[f32] {
        &self.data
    }

    pub fn raw_idx_to_id(&self) -> &[u64] {
        &self.idx_to_id
    }

    pub fn compact(&mut self) -> HashMap<u64, usize> {
        let dim = self.dim;
        let mut new_data = Vec::with_capacity((self.idx_to_id.len() - self.deleted.len()) * dim);
        let mut new_idx_to_id = Vec::with_capacity(self.idx_to_id.len() - self.deleted.len());
        let mut new_id_to_idx = HashMap::with_capacity(self.idx_to_id.len() - self.deleted.len());

        for (idx, &id) in self.idx_to_id.iter().enumerate() {
            if self.deleted.contains(&id) {
                continue;
            }
            let start = idx * dim;
            new_data.extend_from_slice(&self.data[start..start + dim]);
            new_id_to_idx.insert(id, new_idx_to_id.len());
            new_idx_to_id.push(id);
        }

        self.data = new_data;
        self.idx_to_id = new_idx_to_id;
        self.id_to_idx = new_id_to_idx.clone();
        self.deleted.clear();

        new_id_to_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_storage_crud() -> Result<()> {
        let mut storage = VectorStorage::new(3);

        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![4.0, 5.0, 6.0];

        storage.insert(1, &v1, Some(serde_json::json!({"name": "v1"})))?;
        storage.insert(2, &v2, None)?;

        assert_eq!(storage.len(), 2);
        assert_eq!(storage.get_vector(1).unwrap(), &v1[..]);

        let deleted = storage.delete(1)?;
        assert!(deleted);
        assert_eq!(storage.len(), 1);
        assert!(storage.get_vector(1).is_none());

        Ok(())
    }

    #[test]
    fn test_brute_force_search() -> Result<()> {
        let mut storage = VectorStorage::new(2);

        storage.insert(1, &[0.0, 0.0], None)?;
        storage.insert(2, &[1.0, 1.0], None)?;
        storage.insert(3, &[5.0, 5.0], None)?;

        let results = storage.search_brute_force(&[0.1, 0.1], 2, MetricType::L2)?;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, 1);
        assert_eq!(results[1].id, 2);

        Ok(())
    }
}
