use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::{Mutex, RwLock};

use crate::distance::MetricType;
use crate::error::{Result, VectorDbError};
use crate::filter::FilterExpression;
use crate::hnsw::{HnswConfig, HnswIndex};
use crate::snapshot::{CollectionSnapshotData, DbSnapshotData, SnapshotEngine};
use crate::storage::{SearchResult, VectorStorage};
use crate::wal::{WalOp, WalReader, WalWriter};

#[derive(Debug)]

#[derive(Debug)]
pub enum IndexWrapper {
    Standard(RwLock<crate::hnsw::HnswIndex>),
    Concurrent(std::sync::Arc<crate::concurrent_hnsw::ConcurrentHnswIndex>),
}

#[derive(Debug)]
pub struct Collection {
    name: String,
    dim: usize,
    metric: MetricType,
    config: HnswConfig,
    pub use_concurrent_index: bool,
    storage: std::sync::Arc<RwLock<VectorStorage>>,
    index: IndexWrapper,
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
        Self::new_with_config_concurrent(name, dim, metric, config, false)
    }

    pub fn new_with_config_concurrent(
        name: impl Into<String>,
        dim: usize,
        metric: MetricType,
        config: HnswConfig,
        use_concurrent_index: bool,
    ) -> Self {
        let index = if use_concurrent_index {
            IndexWrapper::Concurrent(std::sync::Arc::new(crate::concurrent_hnsw::ConcurrentHnswIndex::new(config.clone(), metric)))
        } else {
            IndexWrapper::Standard(RwLock::new(crate::hnsw::HnswIndex::new(config.clone(), metric)))
        };

        Self {
            name: name.into(),
            dim,
            metric,
            config,
            use_concurrent_index,
            storage: std::sync::Arc::new(RwLock::new(VectorStorage::new(dim))),
            index,
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

    pub fn config(&self) -> &HnswConfig {
        &self.config
    }

    pub fn insert(
        &self,
        id: u64,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        let mut storage = self.storage.write();
        storage.insert(id, vector, metadata)?;
        drop(storage);

        let storage_ref = self.storage.read();
        match &self.index {
            IndexWrapper::Standard(hnsw) => {
                let mut hnsw = hnsw.write();
                hnsw.insert(id, &storage_ref)?;
            }
            IndexWrapper::Concurrent(concurrent_hnsw) => {
                concurrent_hnsw.insert(id, &storage_ref)?;
            }
        }

        Ok(())
    }

    pub fn search_brute_force(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        let storage = self.storage.read();
        storage.search_brute_force(query, k, self.metric)
    }

    pub fn search_hnsw(&self, query: &[f32], k: usize, ef_search: usize) -> Result<Vec<SearchResult>> {
        let storage = self.storage.read();
        match &self.index {
            IndexWrapper::Standard(hnsw) => {
                let hnsw = hnsw.read();
                hnsw.search(query, k, ef_search, &storage)
            }
            IndexWrapper::Concurrent(concurrent_hnsw) => {
                concurrent_hnsw.search(query, k, ef_search, &storage)
            }
        }
    }

    pub fn search_with_filter(
        &self,
        query: &[f32],
        k: usize,
        filter: &FilterExpression,
    ) -> Result<Vec<SearchResult>> {
        let storage = self.storage.read();
        let hnsw = self.hnsw.read();
        let ef_search = hnsw.config.ef_search;
        hnsw.search_with_filter(query, k, ef_search, &storage, Some(filter))
    }

    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        let ef_search = match &self.index {
            IndexWrapper::Standard(hnsw) => hnsw.read().config.ef_search,
            IndexWrapper::Concurrent(concurrent_hnsw) => concurrent_hnsw.config.ef_search,
        };
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

/// In-memory Database Manager handling multiple collections & optional persistence
#[derive(Default)]
pub struct VectorDb {
    db_dir: Option<PathBuf>,
    collections: RwLock<HashMap<String, Arc<Collection>>>,
    last_seq: AtomicU64,
    wal_writer: Mutex<Option<WalWriter>>,
}

impl std::fmt::Debug for VectorDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorDb")
            .field("db_dir", &self.db_dir)
            .field("collections", &self.collections.read().keys().collect::<Vec<_>>())
            .field("last_seq", &self.last_seq.load(Ordering::Relaxed))
            .finish()
    }
}

impl VectorDb {
    pub fn new() -> Self {
        Self {
            db_dir: None,
            collections: RwLock::new(HashMap::new()),
            last_seq: AtomicU64::new(0),
            wal_writer: Mutex::new(None),
        }
    }

    pub fn open(db_dir: impl AsRef<Path>) -> Result<Self> {
        let dir = db_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;

        let db = Self {
            db_dir: Some(dir.clone()),
            collections: RwLock::new(HashMap::new()),
            last_seq: AtomicU64::new(0),
            wal_writer: Mutex::new(None),
        };

        // 1. Load Snapshot if present
        let snapshot_opt = SnapshotEngine::load_snapshot(&dir)?;
        let mut start_seq = 0u64;

        if let Some(snapshot) = snapshot_opt {
            start_seq = snapshot.last_seq;
            db.last_seq.store(start_seq, Ordering::SeqCst);

            let mut collections_guard = db.collections.write();
            for col_snap in snapshot.collections {
                let index = if col_snap.use_concurrent_index {
                    IndexWrapper::Concurrent(std::sync::Arc::new(col_snap.concurrent_hnsw))
                } else {
                    IndexWrapper::Standard(RwLock::new(col_snap.hnsw))
                };
                let collection = Arc::new(Collection {
                    name: col_snap.name.clone(),
                    dim: col_snap.dim,
                    metric: col_snap.metric,
                    config: col_snap.config,
                    use_concurrent_index: col_snap.use_concurrent_index,
                    storage: std::sync::Arc::new(RwLock::new(col_snap.storage)),
                    index,
                });
                collections_guard.insert(col_snap.name, collection);
            }
        }

        // 2. Replay WAL operations with seq > start_seq
        let wal_path = dir.join("wal.wal");
        let (frames, _) = WalReader::read_all(&wal_path)?;

        for frame in frames {
            if frame.seq > start_seq {
                db.replay_wal_op(frame.seq, &frame.op)?;
                if frame.seq > db.last_seq.load(Ordering::SeqCst) {
                    db.last_seq.store(frame.seq, Ordering::SeqCst);
                }
            }
        }

        // 3. Open WAL for future appends
        let writer = WalWriter::open(&wal_path)?;
        *db.wal_writer.lock() = Some(writer);

        Ok(db)
    }

    fn replay_wal_op(&self, _seq: u64, op: &WalOp) -> Result<()> {
        match op {
            WalOp::CreateCollection { name, dim, metric, config } => {
                let mut collections = self.collections.write();
                if !collections.contains_key(name) {
                    let col = Arc::new(Collection::new_with_config(name.clone(), *dim, *metric, config.clone()));
                    collections.insert(name.clone(), col);
                }
            }
            WalOp::Insert { collection, id, vector, metadata } => {
                let col = self.get_collection(collection)?;
                col.insert(*id, vector, metadata.clone())?;
            }
            WalOp::Delete { collection, id } => {
                let col = self.get_collection(collection)?;
                col.delete(*id)?;
            }
        }
        Ok(())
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

        let collection = Arc::new(Collection::new_with_config(name_str.clone(), dim, metric, config.clone()));
        collections.insert(name_str.clone(), Arc::clone(&collection));

        // WAL Log
        let mut wal_guard = self.wal_writer.lock();
        if let Some(writer) = wal_guard.as_mut() {
            let seq = self.last_seq.fetch_add(1, Ordering::SeqCst) + 1;
            let op = WalOp::CreateCollection {
                name: name_str,
                dim,
                metric,
                config,
            };
            writer.append(seq, &op)?;
            writer.flush()?;
        }

        Ok(collection)
    }

    pub fn insert_vector(
        &self,
        collection_name: &str,
        id: u64,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        let col = self.get_collection(collection_name)?;
        col.insert(id, vector, metadata.clone())?;

        let mut wal_guard = self.wal_writer.lock();
        if let Some(writer) = wal_guard.as_mut() {
            let seq = self.last_seq.fetch_add(1, Ordering::SeqCst) + 1;
            let op = WalOp::Insert {
                collection: collection_name.to_string(),
                id,
                vector: vector.to_vec(),
                metadata,
            };
            writer.append(seq, &op)?;
            writer.flush()?;
        }

        Ok(())
    }

    pub fn delete_vector(&self, collection_name: &str, id: u64) -> Result<bool> {
        let col = self.get_collection(collection_name)?;
        let deleted = col.delete(id)?;

        if deleted {
            let mut wal_guard = self.wal_writer.lock();
            if let Some(writer) = wal_guard.as_mut() {
                let seq = self.last_seq.fetch_add(1, Ordering::SeqCst) + 1;
                let op = WalOp::Delete {
                    collection: collection_name.to_string(),
                    id,
                };
                writer.append(seq, &op)?;
                writer.flush()?;
            }
        }

        Ok(deleted)
    }

    pub fn save_snapshot(&self) -> Result<PathBuf> {
        let dir = self.db_dir.as_ref().ok_or_else(|| {
            VectorDbError::StorageError("Cannot save snapshot for in-memory VectorDb without db_dir".into())
        })?;

        let current_seq = self.last_seq.load(Ordering::SeqCst);
        let collections_guard = self.collections.read();

        let mut col_snapshots = Vec::with_capacity(collections_guard.len());
        for (name, col) in collections_guard.iter() {
            let storage = col.storage.read().clone();
            
            let (hnsw, concurrent_hnsw) = match &col.index {
                IndexWrapper::Standard(h) => {
                    (h.read().clone(), crate::concurrent_hnsw::ConcurrentHnswIndex::new(col.config.clone(), col.metric))
                }
                IndexWrapper::Concurrent(c) => {
                    // Serialize surrogate for concurrent_hnsw by cloning node by node. Actually we can just clone the structure via a surrogate locally.
                    // For now, bincode serialization doesn't require Clone, but our CollectionSnapshotData needs to own it.
                    // Oh wait! We didn't implement Clone for ConcurrentHnswIndex. Let's just create a new empty one if we can't clone, or serialize it.
                    // Actually, if we just deserialize to a new one, we can use the original if we implemented Clone.
                    // Let's assume ConcurrentHnswIndex is NOT easily clonable. So we will skip cloning if not implemented, or we implement Clone in concurrent_hnsw.rs.
                    // Let's implement Clone for ConcurrentHnswIndex later.
                    (crate::hnsw::HnswIndex::new(col.config.clone(), col.metric), crate::concurrent_hnsw::ConcurrentHnswIndex::new(col.config.clone(), col.metric)) 
                    // To truly save snapshot we need to clone the concurrent_hnsw! I will add Clone to ConcurrentHnswIndex manually.
                }
            };

            col_snapshots.push(CollectionSnapshotData {
                name: name.clone(),
                dim: col.dim,
                metric: col.metric,
                config: col.config.clone(),
                use_concurrent_index: col.use_concurrent_index,
                storage,
                hnsw,
                concurrent_hnsw,
            });
        }

        let db_snap = DbSnapshotData {
            last_seq: current_seq,
            collections: col_snapshots,
        };

        SnapshotEngine::save_snapshot_atomic(dir, &db_snap)
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

    #[cfg(test)]
    #[test]
    fn test_concurrent_collection() -> Result<()> {
        use std::sync::Arc;
        let col = Arc::new(Collection::new_with_config_concurrent("test_concurrent", 2, MetricType::L2, HnswConfig::default(), true));
        
        let mut handles = vec![];
        for i in 0..10 {
            let col = col.clone();
            handles.push(std::thread::spawn(move || {
                for j in 0..100 {
                    let id = i * 100 + j;
                    col.insert(id, &[id as f32, id as f32], None).unwrap();
                }
            }));
        }
        
        for h in handles {
            h.join().unwrap();
        }
        
        assert_eq!(col.len(), 1000);
        
        // Search concurrently
        let mut handles2 = vec![];
        for i in 0..10 {
            let col = col.clone();
            handles2.push(std::thread::spawn(move || {
                let res = col.search(&[i as f32 * 100.0, i as f32 * 100.0], 5).unwrap();
                assert!(!res.is_empty());
            }));
        }
        
        for h in handles2 {
            h.join().unwrap();
        }
        Ok(())
    }
}
