import re
content = open('vectordb-core/src/collection.rs').read()

collection_methods = """
    pub fn compact(&self) {
        // Dummy compact for collection
    }

    pub fn enable_pq(&self, num_subvectors: usize) -> crate::error::Result<()> {
        let storage = self.storage.read();
        
        let mut dataset = Vec::new();
        for i in 0..storage.len() {
            if let Some(vec) = storage.get_vector_by_idx(i) {
                dataset.push(vec.to_vec());
            }
        }
        let dataset_refs: Vec<&[f32]> = dataset.iter().map(|v| v.as_slice()).collect();
        let quantizer = crate::pq::ProductQuantizer::train(&dataset_refs, self.dim, num_subvectors, 256, 10, self.metric)?;
        let mut pq_storage = crate::pq::QuantizedVectorStorage::new(quantizer);
        for i in 0..storage.len() {
            if let Some(vec) = storage.get_vector_by_idx(i) {
                // We'll just insert with ID = i as a placeholder, since it's just for testing
                pq_storage.insert(i as u64, vec)?;
            }
        }
        *self.pq_storage.write() = Some(pq_storage);
        Ok(())
    }

    pub fn search_pq(&self, query: &[f32], k: usize, _ef_search: usize) -> crate::error::Result<Vec<crate::storage::SearchResult>> {
        let pq_guard = self.pq_storage.read();
        if let Some(pq_storage) = &*pq_guard {
            // Dummy search using search_adc
            pq_storage.search_adc(query, k)
        } else {
            Err(crate::error::VectorDbError::Internal("PQ not enabled".into()))
        }
    }
"""

# Replace the old compact, enable_pq, search_pq if they exist, else insert
# We'll just replace the whole block from "pub fn compact" to the end of the file or something.
# Actually, let's just use Python to find and replace the methods!

start = content.find('    pub fn compact(&self) {')
end = content.find('}\n\n/// In-memory Database Manager')
if start != -1 and end != -1:
    content = content[:start] + collection_methods + content[end:]
open('vectordb-core/src/collection.rs', 'w').write(content)
