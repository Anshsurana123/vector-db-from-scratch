use std::collections::{BinaryHeap, HashSet};
use std::cmp::Ordering;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::distance::{DistanceMetric, MetricType, get_distance_metric};
use crate::error::{Result, VectorDbError};
use crate::storage::{SearchResult, VectorStorage};

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

// Min-heap ordering by distance (smaller distance has higher priority)
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
        // Reverse order so min distance is popped first
        other.0.distance.partial_cmp(&self.0.distance).unwrap_or(Ordering::Equal)
    }
}

// Max-heap ordering by distance (larger distance has higher priority)
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
    fn search_layer(
        &self,
        query: &[f32],
        ep: &[usize],
        ef: usize,
        lc: usize,
        storage: &VectorStorage,
        dist_fn: &dyn DistanceMetric,
    ) -> (BinaryHeap<MaxCandidate>, HashSet<usize>) {
        let mut visited = HashSet::new();
        let mut min_candidates = BinaryHeap::new();
        let mut max_results = BinaryHeap::new();

        for &node_idx in ep {
            visited.insert(node_idx);
            if let Some(vec) = storage.get_vector_by_idx(node_idx) {
                let dist = dist_fn.distance(query, vec);
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
                            let dist = dist_fn.distance(query, nbr_vec);
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

    /// Algorithm 4: Heuristic Neighbor Selection (Malikov & Yashunin)
    /// Prefers spatial diversity over pure closest-M.
    fn select_neighbors_heuristic(
        &self,
        query: &[f32],
        candidates: Vec<Candidate>,
        m: usize,
        lc: usize,
        storage: &VectorStorage,
        dist_fn: &dyn DistanceMetric,
    ) -> Vec<usize> {
        let mut result_indices = Vec::with_capacity(m);
        let mut w_candidates = candidates;

        // Sort candidates by distance to query ascending
        w_candidates.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));

        // Optionally extend candidates with their neighbors at level lc
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
                        distance: dist_fn.distance(query, v),
                    })
                })
                .collect();
            w_candidates.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
        }

        let mut discarded = Vec::new();

        for e in w_candidates {
            if result_indices.len() >= m {
                break;
            }

            let e_vec = match storage.get_vector_by_idx(e.idx) {
                Some(v) => v,
                None => continue,
            };

            // Heuristic Diversity Check:
            // Candidate e is added to R if e is closer to query q than to any already-selected neighbor r in R.
            let mut is_closer = true;
            for &r_idx in &result_indices {
                if let Some(r_vec) = storage.get_vector_by_idx(r_idx) {
                    let dist_e_r = dist_fn.distance(e_vec, r_vec);
                    if dist_e_r <= e.distance {
                        is_closer = false;
                        break;
                    }
                }
            }

            if is_closer {
                result_indices.push(e.idx);
            } else {
                discarded.push(e.idx);
            }
        }

        // If keep_pruned_connections is set and we have remaining slots, fill from discarded candidates
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
        let dist_fn = get_distance_metric(self.metric);

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
            let (candidates, _) = self.search_layer(q_vec, &ep_vec, 1, lc, storage, dist_fn.as_ref());
            if let Some(best) = candidates.peek() {
                curr_ep = best.0.idx;
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
                dist_fn.as_ref(),
            );

            let candidates_vec: Vec<Candidate> = candidates_heap.into_vec().into_iter().map(|m| m.0).collect();

            let m_max = if lc == 0 { self.config.m_max0 } else { self.config.m };

            let selected_nbrs = self.select_neighbors_heuristic(
                q_vec,
                candidates_vec,
                m_max,
                lc,
                storage,
                dist_fn.as_ref(),
            );

            // Connect bidirectional edges
            for &nbr_idx in &selected_nbrs {
                self.nodes[q_node_idx].neighbors[lc].push(nbr_idx);
                self.nodes[nbr_idx].neighbors[lc].push(q_node_idx);

                // Prune neighbor's edge list if it exceeds m_max using Heuristic
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
                                distance: dist_fn.distance(nbr_vec, v),
                            })
                        })
                        .collect();

                    let pruned = self.select_neighbors_heuristic(
                        nbr_vec,
                        existing_cand,
                        m_max,
                        lc,
                        storage,
                        dist_fn.as_ref(),
                    );

                    self.nodes[nbr_idx].neighbors[lc] = pruned;
                }
            }

            ep_vec = selected_nbrs;
        }

        // Update entry point if target level exceeds current max layer
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

        let dist_fn = get_distance_metric(self.metric);
        let ef = std::cmp::max(ef_search, k);

        let mut curr_ep = match self.entry_point {
            Some(ep) => ep,
            None => return Ok(Vec::new()),
        };

        let mut ep_vec = vec![curr_ep];

        // Greedy search top layer down to 1
        for lc in (1..=self.max_layer).rev() {
            let (candidates, _) = self.search_layer(query, &ep_vec, 1, lc, storage, dist_fn.as_ref());
            if let Some(best) = candidates.peek() {
                curr_ep = best.0.idx;
                ep_vec = vec![curr_ep];
            }
        }

        // Layer 0 search with ef_search
        let (candidates_heap, _) = self.search_layer(query, &ep_vec, ef, 0, storage, dist_fn.as_ref());

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

        // Insert 100 2D points
        for i in 0..100 {
            let id = i as u64;
            let vec = vec![i as f32, (i * 2) as f32];
            storage.insert(id, &vec, None)?;
            index.insert(id, &storage)?;
        }

        assert_eq!(index.len(), 100);

        // Search near point (5.0, 10.0) -> ID 5
        let results = index.search(&[5.1, 10.1], 5, 32, &storage)?;
        assert!(!results.is_empty());
        assert_eq!(results[0].id, 5);

        Ok(())
    }
}
