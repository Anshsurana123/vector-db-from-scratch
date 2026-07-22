use std::collections::{BinaryHeap, HashSet};
use std::cmp::Ordering;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::distance::MetricType;
use crate::error::{Result, VectorDbError};
use crate::storage::{SearchResult, VectorStorage};

#[inline(always)]
pub fn compute_distance(metric: MetricType, a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    match metric {
        MetricType::L2 => {
            let mut sum0 = 0.0f32;
            let mut sum1 = 0.0f32;
            let mut sum2 = 0.0f32;
            let mut sum3 = 0.0f32;

            let chunks_a = a.chunks_exact(4);
            let chunks_b = b.chunks_exact(4);
            let rem_a = chunks_a.remainder();
            let rem_b = chunks_b.remainder();

            for (ca, cb) in chunks_a.zip(chunks_b) {
                let d0 = ca[0] - cb[0];
                let d1 = ca[1] - cb[1];
                let d2 = ca[2] - cb[2];
                let d3 = ca[3] - cb[3];
                sum0 += d0 * d0;
                sum1 += d1 * d1;
                sum2 += d2 * d2;
                sum3 += d3 * d3;
            }

            let mut sum = sum0 + sum1 + sum2 + sum3;
            for (&x, &y) in rem_a.iter().zip(rem_b) {
                let diff = x - y;
                sum += diff * diff;
            }
            sum
        }
        MetricType::Cosine => {
            let mut dot0 = 0.0f32;
            let mut dot1 = 0.0f32;
            let mut na0 = 0.0f32;
            let mut na1 = 0.0f32;
            let mut nb0 = 0.0f32;
            let mut nb1 = 0.0f32;

            let chunks_a = a.chunks_exact(2);
            let chunks_b = b.chunks_exact(2);
            let rem_a = chunks_a.remainder();
            let rem_b = chunks_b.remainder();

            for (ca, cb) in chunks_a.zip(chunks_b) {
                let x0 = ca[0];
                let x1 = ca[1];
                let y0 = cb[0];
                let y1 = cb[1];

                dot0 += x0 * y0;
                dot1 += x1 * y1;
                na0 += x0 * x0;
                na1 += x1 * x1;
                nb0 += y0 * y0;
                nb1 += y1 * y1;
            }

            let mut dot = dot0 + dot1;
            let mut norm_a = na0 + na1;
            let mut norm_b = nb0 + nb1;

            for (&x, &y) in rem_a.iter().zip(rem_b) {
                dot += x * y;
                norm_a += x * x;
                norm_b += y * y;
            }

            let norm = (norm_a * norm_b).sqrt();
            if norm < 1e-10 {
                1.0
            } else {
                1.0 - (dot / norm)
            }
        }
        MetricType::DotProduct => {
            let mut sum0 = 0.0f32;
            let mut sum1 = 0.0f32;
            let mut sum2 = 0.0f32;
            let mut sum3 = 0.0f32;

            let chunks_a = a.chunks_exact(4);
            let chunks_b = b.chunks_exact(4);
            let rem_a = chunks_a.remainder();
            let rem_b = chunks_b.remainder();

            for (ca, cb) in chunks_a.zip(chunks_b) {
                sum0 += ca[0] * cb[0];
                sum1 += ca[1] * cb[1];
                sum2 += ca[2] * cb[2];
                sum3 += ca[3] * cb[3];
            }

            let mut dot = sum0 + sum1 + sum2 + sum3;
            for (&x, &y) in rem_a.iter().zip(rem_b) {
                dot += x * y;
            }
            -dot
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    pub m: usize,
    pub m_max0: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
    pub m_l: f64,
    pub extend_candidates: bool,
    pub keep_pruned_connections: bool,
}

impl Default for HnswConfig {
    fn default() -> Self {
        let m = 16;
        Self {
            m,
            m_max0: m * 2,
            ef_construction: 100,
            ef_search: 100,
            m_l: 1.0 / (m as f64).ln(),
            extend_candidates: false,
            keep_pruned_connections: true,
        }
    }
}

impl HnswConfig {
    pub fn new(m: usize, ef_construction: usize, ef_search: usize) -> Self {
        let m_max0 = m * 2;
        let m_l = 1.0 / (m as f64).ln();
        Self {
            m,
            m_max0,
            ef_construction,
            ef_search,
            m_l,
            extend_candidates: false,
            keep_pruned_connections: true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Candidate {
    idx: usize,
    distance: f32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance && self.idx == other.idx
    }
}

impl Eq for Candidate {}

#[derive(Debug, Clone, Copy)]
struct MinCandidate(Candidate);

impl PartialEq for MinCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Eq for MinCandidate {}

impl PartialOrd for MinCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MinCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.distance.partial_cmp(&self.0.distance).unwrap_or(Ordering::Equal)
    }
}

#[derive(Debug, Clone, Copy)]
struct MaxCandidate(Candidate);

impl PartialEq for MaxCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Eq for MaxCandidate {}

impl PartialOrd for MaxCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MaxCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.distance.partial_cmp(&other.0.distance).unwrap_or(Ordering::Equal)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswNode {
    pub id: u64,
    pub level: usize,
    pub neighbors: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswIndex {
    pub config: HnswConfig,
    pub metric: MetricType,
    pub nodes: Vec<HnswNode>,
    pub id_to_node_idx: std::collections::HashMap<u64, usize>,
    pub entry_point: Option<usize>,
    pub max_layer: usize,
}

impl HnswIndex {
    pub fn new(config: HnswConfig, metric: MetricType) -> Self {
        Self {
            config,
            metric,
            nodes: Vec::new(),
            id_to_node_idx: std::collections::HashMap::new(),
            entry_point: None,
            max_layer: 0,
        }
    }

    fn random_level(&self) -> usize {
        let mut rng = rand::thread_rng();
        let r: f64 = rng.gen_range(1e-9..1.0);
        (-r.ln() * self.config.m_l).floor() as usize
    }

    /// Search a single layer `lc` starting from `ep` entry points with candidate limit `ef`
    #[inline]
    fn search_layer(
        &self,
        query: &[f32],
        ep: &[usize],
        ef: usize,
        lc: usize,
        storage: &VectorStorage,
    ) -> (BinaryHeap<MaxCandidate>, HashSet<usize>) {
        let mut visited = HashSet::new();
        let mut min_candidates = BinaryHeap::new();
        let mut max_results = BinaryHeap::new();

        for &node_idx in ep {
            visited.insert(node_idx);
            if let Some(vec) = storage.get_vector_by_idx(node_idx) {
                let dist = compute_distance(self.metric, query, vec);
                let cand = Candidate { idx: node_idx, distance: dist };
                min_candidates.push(MinCandidate(cand));
                max_results.push(MaxCandidate(cand));
            }
        }

        while let Some(MinCandidate(curr)) = min_candidates.pop() {
            let furthest_dist = max_results.peek().map(|m| m.0.distance).unwrap_or(f32::MAX);
            if curr.distance > furthest_dist && max_results.len() >= ef {
                break;
            }

            let node = &self.nodes[curr.idx];
            if lc < node.neighbors.len() {
                for &nbr_idx in &node.neighbors[lc] {
                    if visited.insert(nbr_idx) {
                        if let Some(nbr_vec) = storage.get_vector_by_idx(nbr_idx) {
                            let dist = compute_distance(self.metric, query, nbr_vec);
                            let cand = Candidate { idx: nbr_idx, distance: dist };
                            let worst_dist = max_results.peek().map(|m| m.0.distance).unwrap_or(f32::MAX);

                            if dist < worst_dist || max_results.len() < ef {
                                min_candidates.push(MinCandidate(cand));
                                max_results.push(MaxCandidate(cand));

                                if max_results.len() > ef {
                                    max_results.pop();
                                }
                            }
                        }
                    }
                }
            }
        }

        (max_results, visited)
    }

    fn get_nearest_candidate(heap: &BinaryHeap<MaxCandidate>) -> Option<Candidate> {
        heap.iter().min_by(|a, b| a.0.distance.partial_cmp(&b.0.distance).unwrap_or(Ordering::Equal)).map(|m| m.0)
    }

    /// Algorithm 4: Heuristic Neighbor Selection (Malikov & Yashunin)
    fn select_neighbors_heuristic(
        &self,
        query: &[f32],
        candidates: Vec<Candidate>,
        m: usize,
        lc: usize,
        storage: &VectorStorage,
    ) -> Vec<usize> {
        let mut w_candidates = candidates;
        w_candidates.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));

        if self.config.extend_candidates {
            let mut extended = HashSet::new();
            for cand in &w_candidates {
                extended.insert(cand.idx);
                let node = &self.nodes[cand.idx];
                if lc < node.neighbors.len() {
                    for &nbr_idx in &node.neighbors[lc] {
                        extended.insert(nbr_idx);
                    }
                }
            }

            w_candidates = extended
                .into_iter()
                .filter_map(|idx| {
                    storage.get_vector_by_idx(idx).map(|v| Candidate {
                        idx,
                        distance: compute_distance(self.metric, query, v),
                    })
                })
                .collect();
            w_candidates.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
        }

        let mut result_indices: Vec<usize> = Vec::with_capacity(m);
        let mut result_vectors: Vec<&[f32]> = Vec::with_capacity(m);
        let mut discarded = Vec::new();

        for e in w_candidates {
            if result_indices.len() >= m {
                break;
            }

            let e_vec = match storage.get_vector_by_idx(e.idx) {
                Some(v) => v,
                None => continue,
            };

            // Fast L1-cache sequential evaluation of diversity condition
            let metric = self.metric;
            let e_dist = e.distance;
            let is_closer = result_vectors
                .iter()
                .all(|&r_vec| compute_distance(metric, e_vec, r_vec) > e_dist);

            if is_closer {
                result_indices.push(e.idx);
                result_vectors.push(e_vec);
            } else {
                discarded.push(e.idx);
            }
        }

        if self.config.keep_pruned_connections && result_indices.len() < m {
            for idx in discarded {
                if result_indices.len() >= m {
                    break;
                }
                if !result_indices.contains(&idx) {
                    result_indices.push(idx);
                }
            }
        }

        result_indices
    }

    /// Insert a vector into the HNSW index
    pub fn insert(&mut self, id: u64, storage: &VectorStorage) -> Result<()> {
        let node_idx = storage.get_idx_by_id(id).ok_or(VectorDbError::VectorNotFound(id))?;
        let q_vec = storage.get_vector_by_idx(node_idx).ok_or(VectorDbError::VectorNotFound(id))?;

        let target_level = self.random_level();
        let mut neighbors = Vec::with_capacity(target_level + 1);
        for _ in 0..=target_level {
            neighbors.push(Vec::new());
        }

        let new_node = HnswNode {
            id,
            level: target_level,
            neighbors,
        };

        let q_node_idx = self.nodes.len();
        self.nodes.push(new_node);
        self.id_to_node_idx.insert(id, q_node_idx);

        let mut curr_ep = match self.entry_point {
            None => {
                self.entry_point = Some(q_node_idx);
                self.max_layer = target_level;
                return Ok(());
            }
            Some(ep) => ep,
        };

        let mut ep_vec = vec![curr_ep];
        let max_l = self.max_layer;

        // 1. Top layer down to target_level + 1: Greedy Search (ef=1)
        for lc in (target_level + 1..=max_l).rev() {
            let (candidates, _) = self.search_layer(q_vec, &ep_vec, 1, lc, storage);
            if let Some(best) = Self::get_nearest_candidate(&candidates) {
                curr_ep = best.idx;
                ep_vec = vec![curr_ep];
            }
        }

        // 2. Target level down to 0: Search with ef_construction and connect neighbors using Algorithm 4
        let start_l = std::cmp::min(target_level, max_l);
        for lc in (0..=start_l).rev() {
            let (candidates_heap, _) = self.search_layer(
                q_vec,
                &ep_vec,
                self.config.ef_construction,
                lc,
                storage,
            );

            let candidates_vec: Vec<Candidate> = candidates_heap.into_vec().into_iter().map(|m| m.0).collect();
            let m_max = if lc == 0 { self.config.m_max0 } else { self.config.m };

            let selected_nbrs = self.select_neighbors_heuristic(
                q_vec,
                candidates_vec.clone(),
                m_max,
                lc,
                storage,
            );

            for &nbr_idx in &selected_nbrs {
                self.nodes[q_node_idx].neighbors[lc].push(nbr_idx);
                self.nodes[nbr_idx].neighbors[lc].push(q_node_idx);

                if self.nodes[nbr_idx].neighbors[lc].len() > m_max {
                    let nbr_vec = match storage.get_vector_by_idx(nbr_idx) {
                        Some(v) => v,
                        None => continue,
                    };

                    let existing_cand: Vec<Candidate> = self.nodes[nbr_idx].neighbors[lc]
                        .iter()
                        .filter_map(|&c_idx| {
                            storage.get_vector_by_idx(c_idx).map(|v| Candidate {
                                idx: c_idx,
                                distance: compute_distance(self.metric, nbr_vec, v),
                            })
                        })
                        .collect();

                    let pruned = self.select_neighbors_heuristic(
                        nbr_vec,
                        existing_cand,
                        m_max,
                        lc,
                        storage,
                    );

                    self.nodes[nbr_idx].neighbors[lc] = pruned;
                }
            }

            // Paper Algorithm 1 Line 14: ep = W (all candidates found at layer lc)
            ep_vec = candidates_vec.iter().map(|c| c.idx).collect();
        }

        if target_level > self.max_layer {
            self.max_layer = target_level;
            self.entry_point = Some(q_node_idx);
        }

        Ok(())
    }

    /// Search K nearest neighbors using HNSW graph (Algorithm 5)
    pub fn search(
        &self,
        query: &[f32],
        k: usize,
        ef_search: usize,
        storage: &VectorStorage,
    ) -> Result<Vec<SearchResult>> {
        if k == 0 || self.nodes.is_empty() {
            return Ok(Vec::new());
        }

        let ef = std::cmp::max(ef_search, k);

        let mut curr_ep = match self.entry_point {
            Some(ep) => ep,
            None => return Ok(Vec::new()),
        };

        let mut ep_vec = vec![curr_ep];

        for lc in (1..=self.max_layer).rev() {
            let (candidates, _) = self.search_layer(query, &ep_vec, 1, lc, storage);
            if let Some(best) = Self::get_nearest_candidate(&candidates) {
                curr_ep = best.idx;
                ep_vec = vec![curr_ep];
            }
        }

        let (candidates_heap, _) = self.search_layer(query, &ep_vec, ef, 0, storage);

        let mut sorted_cands: Vec<Candidate> = candidates_heap
            .into_vec()
            .into_iter()
            .map(|m| m.0)
            .filter(|c| !storage.is_deleted(self.nodes[c.idx].id))
            .collect();

        sorted_cands.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
        sorted_cands.truncate(k);

        let results = sorted_cands
            .into_iter()
            .map(|c| {
                let id = self.nodes[c.idx].id;
                SearchResult {
                    id,
                    distance: c.distance,
                    metadata: storage.get_metadata(id).cloned(),
                }
            })
            .collect();

        Ok(results)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_construction_and_search() -> Result<()> {
        let mut storage = VectorStorage::new(2);
        let config = HnswConfig::new(8, 32, 32);
        let mut index = HnswIndex::new(config, MetricType::L2);

        for i in 0..100 {
            let id = i as u64;
            let vec = vec![i as f32, (i * 2) as f32];
            storage.insert(id, &vec, None)?;
            index.insert(id, &storage)?;
        }

        assert_eq!(index.len(), 100);

        let results = index.search(&[5.1, 10.1], 5, 32, &storage)?;
        assert!(!results.is_empty());
        assert_eq!(results[0].id, 5);

        Ok(())
    }
}
