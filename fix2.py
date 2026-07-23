lines = open('vectordb-core/src/collection.rs').readlines()
collection_methods = """
    pub fn compact(&self) {
        let mut storage = self.storage.write();
        storage.compact();
        match &self.index {
            crate::collection::IndexWrapper::Standard(hnsw) => {
                let mut h = hnsw.write();
                h.compact(&storage);
            }
            crate::collection::IndexWrapper::Concurrent(_) => {
                // Not supported
            }
        }
    }

    pub fn enable_pq(&self, num_subvectors: usize) -> crate::error::Result<()> {
        let storage = self.storage.read();
        let mut pq_storage = crate::pq::QuantizedVectorStorage::new(num_subvectors);
        pq_storage.train_and_quantize(&storage)?;
        *self.pq_storage.write() = Some(pq_storage);
        Ok(())
    }

    pub fn search_pq(&self, query: &[f32], k: usize, ef_search: usize) -> crate::error::Result<Vec<crate::storage::SearchResult>> {
        let pq_guard = self.pq_storage.read();
        if let Some(pq_storage) = &*pq_guard {
            match &self.index {
                crate::collection::IndexWrapper::Standard(hnsw) => hnsw.read().search_pq(query, k, ef_search, pq_storage),
                crate::collection::IndexWrapper::Concurrent(_) => Err(crate::error::VectorDbError::Internal("PQ search not supported for Concurrent HNSW".into())),
            }
        } else {
            Err(crate::error::VectorDbError::Internal("PQ not enabled".into()))
        }
    }
"""
lines.insert(181, collection_methods)
open('vectordb-core/src/collection.rs', 'w').writelines(lines)
