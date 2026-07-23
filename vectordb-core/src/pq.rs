use std::cmp::Ordering;
use std::collections::BinaryHeap;
use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::distance::MetricType;
use crate::error::{Result, VectorDbError};
use crate::hnsw::compute_distance;
use crate::storage::SearchResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductQuantizer {
    pub dim: usize,
    pub num_subvectors: usize,
    pub sub_dim: usize,
    pub num_centroids: usize,
    pub metric: MetricType,
    /// Codebooks shape: [num_subvectors][num_centroids][sub_dim]
    pub codebooks: Vec<Vec<Vec<f32>>>,
}

impl ProductQuantizer {
    /// Train K-Means++ codebooks for each sub-space
    pub fn train(
        vectors: &[&[f32]],
        dim: usize,
        num_subvectors: usize,
        num_centroids: usize,
        max_iterations: usize,
        metric: MetricType,
    ) -> Result<Self> {
        if vectors.is_empty() {
            return Err(VectorDbError::StorageError("Cannot train PQ on empty dataset".into()));
        }
        if dim % num_subvectors != 0 {
            return Err(VectorDbError::DimensionMismatch {
                expected: dim,
                actual: num_subvectors,
            });
        }

        let sub_dim = dim / num_subvectors;
        let mut codebooks = Vec::with_capacity(num_subvectors);

        for m in 0..num_subvectors {
            let start = m * sub_dim;
            let end = start + sub_dim;

            // Extract sub-vectors for subspace m
            let sub_vecs: Vec<&[f32]> = vectors.iter().map(|v| &v[start..end]).collect();

            // K-Means++ clustering for subspace m
            let centroids = kmeans_plus_plus(&sub_vecs, sub_dim, num_centroids, max_iterations, metric)?;
            codebooks.push(centroids);
        }

        Ok(Self {
            dim,
            num_subvectors,
            sub_dim,
            num_centroids,
            metric,
            codebooks,
        })
    }

    /// Encode a D-dimensional vector into an m-byte quantized code array
    pub fn encode(&self, vector: &[f32]) -> Result<Vec<u8>> {
        if vector.len() != self.dim {
            return Err(VectorDbError::DimensionMismatch {
                expected: self.dim,
                actual: vector.len(),
            });
        }

        let mut code = Vec::with_capacity(self.num_subvectors);

        for m in 0..self.num_subvectors {
            let start = m * self.sub_dim;
            let end = start + self.sub_dim;
            let sub_v = &vector[start..end];

            let mut min_dist = f32::MAX;
            let mut best_c = 0u8;

            for (c, centroid) in self.codebooks[m].iter().enumerate() {
                let dist = compute_distance(self.metric, sub_v, centroid);
                if dist < min_dist {
                    min_dist = dist;
                    best_c = c as u8;
                }
            }

            code.push(best_c);
        }

        Ok(code)
    }

    /// Compute Look-Up Table (LUT) for Asymmetric Distance Computation (ADC)
    /// LUT shape: [num_subvectors][num_centroids]
    pub fn compute_adc_table(&self, query: &[f32]) -> Result<Vec<Vec<f32>>> {
        if query.len() != self.dim {
            return Err(VectorDbError::DimensionMismatch {
                expected: self.dim,
                actual: query.len(),
            });
        }

        let mut lut = Vec::with_capacity(self.num_subvectors);

        for m in 0..self.num_subvectors {
            let start = m * self.sub_dim;
            let end = start + self.sub_dim;
            let q_sub = &query[start..end];

            let mut sub_lut = Vec::with_capacity(self.num_centroids);
            for centroid in &self.codebooks[m] {
                let dist = compute_distance(self.metric, q_sub, centroid);
                sub_lut.push(dist);
            }

            lut.push(sub_lut);
        }

        Ok(lut)
    }

    /// Compute distance between query (via LUT) and quantized code in O(m) table lookups
    #[inline(always)]
    pub fn distance_adc(&self, lut: &[Vec<f32>], code: &[u8]) -> f32 {
        debug_assert_eq!(lut.len(), code.len());
        let mut dist = 0.0f32;
        for m in 0..code.len() {
            let centroid_idx = code[m] as usize;
            dist += lut[m][centroid_idx];
        }
        dist
    }
}

/// K-Means++ clustering for sub-vectors
fn kmeans_plus_plus(
    sub_vecs: &[&[f32]],
    sub_dim: usize,
    k: usize,
    max_iterations: usize,
    metric: MetricType,
) -> Result<Vec<Vec<f32>>> {
    let num_samples = sub_vecs.len();
    let k = std::cmp::min(k, num_samples);

    let mut rng = rand::thread_rng();
    let mut centroids: Vec<Vec<f32>> = Vec::with_capacity(k);

    // 1. First centroid chosen uniformly at random
    let &first = sub_vecs.choose(&mut rng).unwrap();
    centroids.push(first.to_vec());

    // 2. K-Means++ initialization for remaining centroids
    let mut min_dists = vec![f32::MAX; num_samples];

    for _ in 1..k {
        let last_centroid = centroids.last().unwrap();

        let mut sum_dist = 0.0f64;
        for (i, &v) in sub_vecs.iter().enumerate() {
            let d = compute_distance(metric, v, last_centroid);
            if d < min_dists[i] {
                min_dists[i] = d;
            }
            sum_dist += (min_dists[i] * min_dists[i]) as f64;
        }

        if sum_dist <= 1e-10 {
            // Degenerate case: fallback to random assignment
            let &rand_v = sub_vecs.choose(&mut rng).unwrap();
            centroids.push(rand_v.to_vec());
            continue;
        }

        // Weighted probability sampling based on distance squared
        let r: f64 = rng.gen_range(0.0..sum_dist);
        let mut accum = 0.0f64;
        let mut next_idx = 0;

        for (i, &d) in min_dists.iter().enumerate() {
            accum += (d * d) as f64;
            if accum >= r {
                next_idx = i;
                break;
            }
        }

        centroids.push(sub_vecs[next_idx].to_vec());
    }

    // 3. Lloyd's K-Means Iteration Loop
    let mut assignments = vec![0usize; num_samples];

    for _ in 0..max_iterations {
        let mut changed = false;

        // Assign each sub-vector to nearest centroid
        for (i, &v) in sub_vecs.iter().enumerate() {
            let mut best_c = 0;
            let mut min_d = f32::MAX;

            for (c_idx, centroid) in centroids.iter().enumerate() {
                let d = compute_distance(metric, v, centroid);
                if d < min_d {
                    min_d = d;
                    best_c = c_idx;
                }
            }

            if assignments[i] != best_c {
                assignments[i] = best_c;
                changed = true;
            }
        }

        if !changed {
            break;
        }

        // Update centroid positions to mean of assigned sub-vectors
        let mut new_centroids = vec![vec![0.0f32; sub_dim]; k];
        let mut counts = vec![0usize; k];

        for (i, &c_idx) in assignments.iter().enumerate() {
            let v = sub_vecs[i];
            counts[c_idx] += 1;
            for d in 0..sub_dim {
                new_centroids[c_idx][d] += v[d];
            }
        }

        for c_idx in 0..k {
            if counts[c_idx] > 0 {
                let count_f = counts[c_idx] as f32;
                for d in 0..sub_dim {
                    new_centroids[c_idx][d] /= count_f;
                }
            } else {
                new_centroids[c_idx] = centroids[c_idx].clone();
            }
        }

        centroids = new_centroids;
    }

    Ok(centroids)
}

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
        self.distance
            .partial_cmp(&other.distance)
            .unwrap_or(Ordering::Equal)
    }
}

/// Quantized storage holding 16-byte PQ codes for 32x memory compression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizedVectorStorage {
    pub dim: usize,
    pub quantizer: ProductQuantizer,
    pub codes: Vec<Vec<u8>>,
    pub ids: Vec<u64>,
    pub id_to_idx: std::collections::HashMap<u64, usize>,
    #[serde(default)]
    pub deleted: std::collections::HashSet<u64>,
}

impl QuantizedVectorStorage {
    pub fn new(quantizer: ProductQuantizer) -> Self {
        let dim = quantizer.dim;
        Self {
            dim,
            quantizer,
            codes: Vec::new(),
            ids: Vec::new(),
            id_to_idx: std::collections::HashMap::new(),
            deleted: std::collections::HashSet::new(),
        }
    }

    pub fn insert(&mut self, id: u64, vector: &[f32]) -> Result<()> {
        let code = self.quantizer.encode(vector)?;
        if self.id_to_idx.contains_key(&id) && !self.deleted.contains(&id) {
            let idx = self.id_to_idx[&id];
            self.codes[idx] = code;
            return Ok(());
        }
        if self.deleted.remove(&id) {
            let idx = self.id_to_idx[&id];
            self.codes[idx] = code;
        } else {
            let idx = self.ids.len();
            self.id_to_idx.insert(id, idx);
            self.ids.push(id);
            self.codes.push(code);
        }
        Ok(())
    }

    pub fn delete(&mut self, id: u64) -> bool {
        if self.id_to_idx.contains_key(&id) && !self.deleted.contains(&id) {
            self.deleted.insert(id);
            true
        } else {
            false
        }
    }

    pub fn search_adc(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        if k == 0 || self.codes.is_empty() {
            return Ok(Vec::new());
        }

        let lut = self.quantizer.compute_adc_table(query)?;
        let mut heap = BinaryHeap::with_capacity(k);

        for (idx, code) in self.codes.iter().enumerate() {
            let id = self.ids[idx];
            if self.deleted.contains(&id) {
                continue;
            }
            let dist = self.quantizer.distance_adc(&lut, code);

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
                metadata: None,
            })
            .collect();

        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
        Ok(results)
    }

    pub fn len(&self) -> usize {
        self.codes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.codes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pq_training_and_encoding() -> Result<()> {
        let v1 = vec![1.0, 2.0, 3.0, 4.0];
        let v2 = vec![5.0, 6.0, 7.0, 8.0];
        let v3 = vec![1.1, 2.1, 3.1, 4.1];

        let dataset = vec![v1.as_slice(), v2.as_slice(), v3.as_slice()];

        let quantizer = ProductQuantizer::train(&dataset, 4, 2, 2, 10, MetricType::L2)?;
        assert_eq!(quantizer.num_subvectors, 2);
        assert_eq!(quantizer.sub_dim, 2);

        let code = quantizer.encode(&v1)?;
        assert_eq!(code.len(), 2);

        let lut = quantizer.compute_adc_table(&v1)?;
        let dist = quantizer.distance_adc(&lut, &code);
        assert!(dist < 0.1);

        Ok(())
    }
}
