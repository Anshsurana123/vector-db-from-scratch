use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

use crate::distance::MetricType;
use crate::error::{Result, VectorDbError};
use crate::hnsw::{HnswConfig, HnswIndex};
use crate::storage::{SearchResult, VectorStorage};

#[derive(Debug)]
pub struct Collection {
    name: String,
    dim: usize,
    metric: MetricType,
    storage: RwLock<VectorStorage>,
    hnsw: RwLock<HnswIndex>,
}

impl Collection {
    pub fn new(name: impl Into<String>, dim: usize, metric: MetricType) -> Self {
        Self::new_with_config(name, dim, metric, HnswConfig::default())
    }

    pub fn new_with_config(
        name: impl Into<String>,
        dim: usize,
        metric: MetricType,
        config: HnswConfig,
    ) -> Self {
        Self {
            name: name.into(),
            dim,
            metric,
            storage: RwLock::new(VectorStorage::new(dim)),
            hnsw: RwLock::new(HnswIndex::new(config, metric)),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    pub fn metric(&self) -> MetricType {
        self.metric
    }

    pub fn insert(
        &self,
        id: u64,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        let mut storage = self.storage.write();
        storage.insert(id, vector, metadata)?;

        let mut hnsw = self.hnsw.write();
        hnsw.insert(id, &storage)?;

        Ok(())
    }

    pub fn search_brute_force(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        let storage = self.storage.read();
        storage.search_brute_force(query, k, self.metric)
    }

    pub fn search_hnsw(&self, query: &[f32], k: usize, ef_search: usize) -> Result<Vec<SearchResult>> {
        let storage = self.storage.read();
        let hnsw = self.hnsw.read();
        hnsw.search(query, k, ef_search, &storage)
    }

    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        let hnsw = self.hnsw.read();
        let ef_search = hnsw.config.ef_search;
        drop(hnsw);
        self.search_hnsw(query, k, ef_search)
    }

    pub fn delete(&self, id: u64) -> Result<bool> {
        let mut storage = self.storage.write();
        storage.delete(id)
    }

    pub fn get_vector(&self, id: u64) -> Option<Vec<f32>> {
        let storage = self.storage.read();
        storage.get_vector(id).map(|v| v.to_vec())
    }

    pub fn len(&self) -> usize {
        let storage = self.storage.read();
        storage.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// In-memory Database Manager handling multiple collections
#[derive(Debug, Default)]
pub struct VectorDb {
    collections: RwLock<HashMap<String, Arc<Collection>>>,
}

impl VectorDb {
    pub fn new() -> Self {
        Self {
            collections: RwLock::new(HashMap::new()),
        }
    }

    pub fn create_collection(
        &self,
        name: impl Into<String>,
        dim: usize,
        metric: MetricType,
    ) -> Result<Arc<Collection>> {
        self.create_collection_with_config(name, dim, metric, HnswConfig::default())
    }

    pub fn create_collection_with_config(
        &self,
        name: impl Into<String>,
        dim: usize,
        metric: MetricType,
        config: HnswConfig,
    ) -> Result<Arc<Collection>> {
        let name_str = name.into();
        let mut collections = self.collections.write();
        if collections.contains_key(&name_str) {
            return Err(VectorDbError::CollectionAlreadyExists(name_str));
        }

        let collection = Arc::new(Collection::new_with_config(name_str.clone(), dim, metric, config));
        collections.insert(name_str, Arc::clone(&collection));
        Ok(collection)
    }

    pub fn get_collection(&self, name: &str) -> Result<Arc<Collection>> {
        let collections = self.collections.read();
        collections
            .get(name)
            .cloned()
            .ok_or_else(|| VectorDbError::CollectionNotFound(name.to_string()))
    }

    pub fn drop_collection(&self, name: &str) -> Result<bool> {
        let mut collections = self.collections.write();
        Ok(collections.remove(name).is_some())
    }
}
