import re
content = open('vectordb-core/src/collection.rs').read()

collection_methods = """
    pub fn compact(&self) {
        let mut storage = self.storage.write();
        storage.compact();
        match &self.index {
            IndexWrapper::Standard(hnsw) => {
                let mut h = hnsw.write();
                h.compact(&storage);
            }
            IndexWrapper::Concurrent(_) => {
                // Not supported
            }
        }
    }

    pub fn enable_pq(&self, num_subvectors: usize) -> Result<()> {
        let storage = self.storage.read();
        let mut pq_storage = crate::pq::QuantizedVectorStorage::new(num_subvectors);
        pq_storage.train_and_quantize(&storage)?;
        *self.pq_storage.write() = Some(pq_storage);
        Ok(())
    }

    pub fn search_pq(&self, query: &[f32], k: usize, ef_search: usize) -> Result<Vec<SearchResult>> {
        let pq_guard = self.pq_storage.read();
        if let Some(pq_storage) = &*pq_guard {
            match &self.index {
                IndexWrapper::Standard(hnsw) => hnsw.read().search_pq(query, k, ef_search, pq_storage),
                IndexWrapper::Concurrent(_) => Err(crate::error::VectorDbError::Internal("PQ search not supported for Concurrent HNSW".into())),
            }
        } else {
            Err(crate::error::VectorDbError::Internal("PQ not enabled".into()))
        }
    }
}
"""
content = re.sub(r'pub fn len\(&self\) -> usize \{\n        self\.storage\.read\(\)\.len\(\)\n    \}\n\}',
    'pub fn len(&self) -> usize {\n        self.storage.read().len()\n    }\n' + collection_methods,
    content)

# Fix load_snapshot pq_storage init
content = content.replace(
    'use_concurrent_index: col_snap.use_concurrent_index,\n                    storage: std::sync::Arc::new(RwLock::new(col_snap.storage)),\n                    index,\n                });',
    'use_concurrent_index: col_snap.use_concurrent_index,\n                    storage: std::sync::Arc::new(RwLock::new(col_snap.storage)),\n                    index,\n                    pq_storage: parking_lot::RwLock::new(col_snap.pq_storage),\n                });'
)

# And VectorDb methods:
db_methods = """
    pub fn compact_collection(&self, name: &str) -> Result<()> {
        let collections = self.collections.read();
        let col = collections.get(name).ok_or_else(|| {
            crate::error::VectorDbError::CollectionNotFound(format!("Collection '{}' not found", name))
        })?;
        col.compact();
        Ok(())
    }

    pub fn train_pq(&self, name: &str, num_subvectors: usize) -> Result<()> {
        let collections = self.collections.read();
        let col = collections.get(name).ok_or_else(|| {
            crate::error::VectorDbError::CollectionNotFound(format!("Collection '{}' not found", name))
        })?;
        col.enable_pq(num_subvectors)?;
        Ok(())
    }
}
"""
content = re.sub(r'pub fn drop_collection\(&self, name: &str\) -> Result<bool> \{\n        let mut collections = self\.collections\.write\(\);\n        Ok\(collections\.remove\(name\)\.is_some\(\)\)\n    \}\n\}',
    'pub fn drop_collection(&self, name: &str) -> Result<bool> {\n        let mut collections = self.collections.write();\n        Ok(collections.remove(name).is_some())\n    }\n' + db_methods,
    content)

open('vectordb-core/src/collection.rs', 'w').write(content)
