use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::distance::MetricType;
use crate::error::{Result, VectorDbError};
use crate::hnsw::{HnswConfig, HnswIndex};
use crate::storage::VectorStorage;

fn default_concurrent_hnsw() -> crate::concurrent_hnsw::ConcurrentHnswIndex {
    crate::concurrent_hnsw::ConcurrentHnswIndex::new(HnswConfig::default(), MetricType::L2)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionSnapshotData {
    pub name: String,
    pub dim: usize,
    pub metric: MetricType,
    pub config: HnswConfig,
    #[serde(default)]
    pub use_concurrent_index: bool,
    pub storage: VectorStorage,
    pub hnsw: HnswIndex,
    #[serde(default = "default_concurrent_hnsw")]
    pub concurrent_hnsw: crate::concurrent_hnsw::ConcurrentHnswIndex,
    #[serde(default)]
    pub pq_storage: Option<crate::pq::QuantizedVectorStorage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DbSnapshotData {
    pub last_seq: u64,
    pub collections: Vec<CollectionSnapshotData>,
}

pub struct SnapshotEngine;

impl SnapshotEngine {
    pub fn save_snapshot_atomic(db_dir: impl AsRef<Path>, snapshot: &DbSnapshotData) -> Result<PathBuf> {
        let dir = db_dir.as_ref();
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }

        let tmp_path = dir.join("snapshot.snap.tmp");
        let final_path = dir.join("snapshot.snap");

        let file = File::create(&tmp_path)?;
        let mut writer = BufWriter::new(file);

        let bytes = bincode::serialize(snapshot)
            .map_err(|e| VectorDbError::StorageError(format!("Snapshot serialization failed: {}", e)))?;

        writer.write_all(&bytes)?;
        writer.flush()?;
        writer.get_ref().sync_all()?;
        drop(writer);

        // Atomic rename .snap.tmp -> .snap
        fs::rename(&tmp_path, &final_path)?;

        Ok(final_path)
    }

    pub fn load_snapshot(db_dir: impl AsRef<Path>) -> Result<Option<DbSnapshotData>> {
        let final_path = db_dir.as_ref().join("snapshot.snap");
        if !final_path.exists() {
            return Ok(None);
        }

        let file = File::open(&final_path)?;
        let reader = BufReader::new(file);

        let snapshot: DbSnapshotData = bincode::deserialize_from(reader)
            .map_err(|e| VectorDbError::StorageError(format!("Snapshot deserialization failed: {}", e)))?;

        Ok(Some(snapshot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_atomic_snapshot_save_and_load() -> Result<()> {
        let dir = tempdir()?;
        let db_dir = dir.path();

        let mut storage = VectorStorage::new(2);
        storage.insert(1, &[1.0, 2.0], None)?;
        let hnsw = HnswIndex::new(HnswConfig::default(), MetricType::L2);

        let snapshot = DbSnapshotData {
            last_seq: 42,
            collections: vec![CollectionSnapshotData {
                name: "test_col".into(),
                dim: 2,
                metric: MetricType::L2,
                config: HnswConfig::default(),
                storage,
                hnsw,
                use_concurrent_index: false,
                concurrent_hnsw: crate::concurrent_hnsw::ConcurrentHnswIndex::new(HnswConfig::default(), MetricType::L2),
            }],
        };

        let snap_path = SnapshotEngine::save_snapshot_atomic(db_dir, &snapshot)?;
        assert!(snap_path.exists());
        assert!(!db_dir.join("snapshot.snap.tmp").exists());

        let loaded = SnapshotEngine::load_snapshot(db_dir)?.expect("Snapshot should exist");
        assert_eq!(loaded.last_seq, 42);
        assert_eq!(loaded.collections.len(), 1);
        assert_eq!(loaded.collections[0].name, "test_col");

        Ok(())
    }
}
