use std::collections::BinaryHeap;
use std::cmp::Ordering;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering as AtomicOrdering};
use parking_lot::{Mutex, RwLock};
use rand::Rng;
use rayon::prelude::*;

use crate::distance::MetricType;
use crate::error::{Result, VectorDbError};
use crate::hnsw::{HnswConfig, compute_distance};
use crate::storage::{SearchResult, VectorStorage};

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

#[derive(Debug)]
pub struct ConcurrentHnswNode {
    pub id: u64,
    pub storage_idx: usize,
    pub level: usize,
    pub neighbors: Vec<RwLock<Vec<usize>>>,
}

#[derive(Debug)]
pub struct ConcurrentHnswIndex {
    pub config: HnswConfig,
    pub metric: MetricType,
    pub nodes: RwLock<Vec<ArcNode>>,
    pub entry_point: AtomicUsize,
    pub max_layer: AtomicUsize,
    pub has_entry_point: std::sync::atomic::AtomicBool,

    visited_tags: Mutex<Vec<u32>>,
    visit_id_counter: AtomicU32,
}

type ArcNode = std::sync::Arc<ConcurrentHnswNode>;

impl ConcurrentHnswIndex {
    pub fn new(config: HnswConfig, metric: MetricType) -> Self {
        Self {
            config,
            metric,
            nodes: RwLock::new(Vec::new()),
            entry_point: AtomicUsize::new(0),
            max_layer: AtomicUsize::new(0),
            has_entry_point: std::sync::atomic::AtomicBool::new(false),
            visited_tags: Mutex::new(Vec::new()),
            visit_id_counter: AtomicU32::new(1),
        }
    }

    fn random_level(&self) -> usize {
        let mut rng = rand::thread_rng();
        let r: f64 = rng.gen_range(1e-9..1.0);
        (-r.ln() * self.config.m_l).floor() as usize
    }

    fn search_layer(
        &self,
        query: &[f32],
        ep: &[usize],
        ef: usize,
        lc: usize,
        storage: &VectorStorage,
        nodes: &[ArcNode],
    ) -> (BinaryHeap<MaxCandidate>, usize) {
        let mut visit_id = self.visit_id_counter.fetch_add(1, AtomicOrdering::Relaxed);
        let mut visited = self.visited_tags.lock();
        
        let num_nodes = nodes.len();
        if visited.len() < num_nodes {
            visited.resize(num_nodes + 1024, 0);
        }

        if visit_id == u32::MAX {
            for tag in visited.iter_mut() {
                *tag = 0;
            }
            self.visit_id_counter.store(1, AtomicOrdering::Relaxed);
            visit_id = 1;
        }

        let mut min_candidates = BinaryHeap::new();
        let mut max_results = BinaryHeap::new();

        for &node_idx in ep {
            if node_idx < visited.len() {
                visited[node_idx] = visit_id;
            }
            if node_idx < nodes.len() {
                let st_idx = nodes[node_idx].storage_idx;
                if let Some(vec) = storage.get_vector_by_idx(st_idx) {
                    let dist = compute_distance(self.metric, query, vec);
                    let cand = Candidate { idx: node_idx, distance: dist };
                    min_candidates.push(MinCandidate(cand));
                    max_results.push(MaxCandidate(cand));
                }
            }
        }

        while let Some(MinCandidate(curr)) = min_candidates.pop() {
            let furthest_dist = max_results.peek().map(|m| m.0.distance).unwrap_or(f32::MAX);
            if curr.distance > furthest_dist && max_results.len() >= ef {
                break;
            }

            if curr.idx < nodes.len() {
                let node = &nodes[curr.idx];
                if lc < node.neighbors.len() {
                    let nbr_read_guard = node.neighbors[lc].read();
                    for &nbr_idx in nbr_read_guard.iter() {
                        if nbr_idx < visited.len() && visited[nbr_idx] != visit_id {
                            visited[nbr_idx] = visit_id;

                            if nbr_idx < nodes.len() {
                                let nbr_st_idx = nodes[nbr_idx].storage_idx;
                                if let Some(nbr_vec) = storage.get_vector_by_idx(nbr_st_idx) {
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
            }
        }

        (max_results, ep.len())
    }

    /// Algorithm 4: Heuristic Neighbor Selection (Malikov & Yashunin) for Concurrent Graph
    fn select_neighbors_heuristic(
        &self,
        query: &[f32],
        candidates: Vec<Candidate>,
        m: usize,
        lc: usize,
        storage: &VectorStorage,
        snapshot_nodes: &[std::sync::Arc<ConcurrentHnswNode>],
    ) -> Vec<usize> {
        let mut w_candidates = candidates;
        w_candidates.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));

        if self.config.extend_candidates {
            let mut extended = std::collections::HashSet::new();
            for cand in &w_candidates {
                extended.insert(cand.idx);
                if cand.idx < snapshot_nodes.len() {
                    let node = &snapshot_nodes[cand.idx];
                    if lc < node.neighbors.len() {
                        let nbrs = node.neighbors[lc].read();
                        for &nbr_idx in nbrs.iter() {
                            extended.insert(nbr_idx);
                        }
                    }
                }
            }

            w_candidates = extended
                .into_iter()
                .filter_map(|idx| {
                    if idx < snapshot_nodes.len() {
                        let st_idx = snapshot_nodes[idx].storage_idx;
                        storage.get_vector_by_idx(st_idx).map(|v| Candidate {
                            idx,
                            distance: compute_distance(self.metric, query, v),
                        })
                    } else {
                        None
                    }
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

            if e.idx >= snapshot_nodes.len() {
                continue;
            }
            let e_st_idx = snapshot_nodes[e.idx].storage_idx;
            let e_vec = match storage.get_vector_by_idx(e_st_idx) {
                Some(v) => v,
                None => continue,
            };

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

    /// Insert vector into the concurrent HNSW index with fine-grained neighbor locks
    pub fn insert(&self, id: u64, storage: &VectorStorage) -> Result<()> {
        let storage_idx = storage.get_idx_by_id(id).ok_or(VectorDbError::VectorNotFound(id))?;
        let q_vec = storage.get_vector_by_idx(storage_idx).ok_or(VectorDbError::VectorNotFound(id))?;

        let target_level = self.random_level();
        let mut neighbors = Vec::with_capacity(target_level + 1);
        for _ in 0..=target_level {
            neighbors.push(RwLock::new(Vec::new()));
        }

        let new_node = std::sync::Arc::new(ConcurrentHnswNode {
            id,
            storage_idx,
            level: target_level,
            neighbors,
        });

        let (q_node_idx, snapshot_nodes) = {
            let mut nodes_guard = self.nodes.write();
            let idx = nodes_guard.len();
            nodes_guard.push(std::sync::Arc::clone(&new_node));
            (idx, nodes_guard.clone())
        };

        if q_node_idx == 0 {
            self.has_entry_point.store(true, AtomicOrdering::SeqCst);
            self.entry_point.store(0, AtomicOrdering::SeqCst);
            self.max_layer.store(target_level, AtomicOrdering::SeqCst);
            return Ok(());
        }

        let curr_ep = self.entry_point.load(AtomicOrdering::SeqCst);
        let max_l = self.max_layer.load(AtomicOrdering::SeqCst);

        let mut ep_vec = vec![curr_ep];

        // 1. Top layer down to target_level + 1
        for lc in (target_level + 1..=max_l).rev() {
            let (candidates, _) = self.search_layer(q_vec, &ep_vec, 4, lc, storage, &snapshot_nodes);
            let top_cands: Vec<usize> = candidates.into_vec().into_iter().map(|m| m.0.idx).collect();
            if !top_cands.is_empty() {
                ep_vec = top_cands;
            }
        }

        // 2. Target level down to 0
        let start_l = std::cmp::min(target_level, max_l);
        for lc in (0..=start_l).rev() {
            let (candidates_heap, _) = self.search_layer(
                q_vec,
                &ep_vec,
                self.config.ef_construction,
                lc,
                storage,
                &snapshot_nodes,
            );

            let candidates_vec: Vec<Candidate> = candidates_heap.into_vec().into_iter().map(|m| m.0).collect();
            let m_max = if lc == 0 { self.config.m_max0 } else { self.config.m };

            let selected_nbrs = self.select_neighbors_heuristic(
                q_vec,
                candidates_vec.clone(),
                m_max,
                lc,
                storage,
                &snapshot_nodes,
            );

            for &nbr_idx in &selected_nbrs {
                if nbr_idx < snapshot_nodes.len() && nbr_idx != q_node_idx {
                    new_node.neighbors[lc].write().push(nbr_idx);
                    
                    let mut nbr_neighbors = snapshot_nodes[nbr_idx].neighbors[lc].write();
                    nbr_neighbors.push(q_node_idx);

                    if nbr_neighbors.len() > m_max {
                        let nbr_st_idx = snapshot_nodes[nbr_idx].storage_idx;
                        if let Some(nbr_vec) = storage.get_vector_by_idx(nbr_st_idx) {
                            let existing_cand: Vec<Candidate> = nbr_neighbors
                                .iter()
                                .filter_map(|&c_idx| {
                                    if c_idx < snapshot_nodes.len() {
                                        let c_st_idx = snapshot_nodes[c_idx].storage_idx;
                                        storage.get_vector_by_idx(c_st_idx).map(|v| Candidate {
                                            idx: c_idx,
                                            distance: compute_distance(self.metric, nbr_vec, v),
                                        })
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            let pruned = self.select_neighbors_heuristic(
                                nbr_vec,
                                existing_cand,
                                m_max,
                                lc,
                                storage,
                                &snapshot_nodes,
                            );

                            *nbr_neighbors = pruned;
                        }
                    }
                }
            }

            ep_vec = candidates_vec.iter().map(|c| c.idx).collect();
        }

        if target_level > self.max_layer.load(AtomicOrdering::SeqCst) {
            self.max_layer.store(target_level, AtomicOrdering::SeqCst);
            self.entry_point.store(q_node_idx, AtomicOrdering::SeqCst);
        }

        Ok(())
    }

    /// Parallel batch vector insertion using Rayon chunking
    pub fn insert_batch_parallel(&self, ids: &[u64], storage: &VectorStorage) -> Result<()> {
        ids.par_iter().for_each(|&id| {
            let _ = self.insert(id, storage);
        });
        Ok(())
    }

    /// Search K nearest neighbors using HNSW graph with fine-grained node locks
    pub fn search(
        &self,
        query: &[f32],
        k: usize,
        ef_search: usize,
        storage: &VectorStorage,
    ) -> Result<Vec<SearchResult>> {
        self.search_with_filter(query, k, ef_search, storage, None)
    }

    pub fn search_with_filter(
        &self,
        query: &[f32],
        k: usize,
        ef_search: usize,
        storage: &VectorStorage,
        filter: Option<&crate::filter::FilterExpression>,
    ) -> Result<Vec<SearchResult>> {
        if k == 0 || !self.has_entry_point.load(AtomicOrdering::Relaxed) {
            return Ok(Vec::new());
        }

        let ef = std::cmp::max(ef_search, k);
        let snapshot_nodes = self.nodes.read().clone();
        let curr_ep = self.entry_point.load(AtomicOrdering::SeqCst);
        let max_l = self.max_layer.load(AtomicOrdering::SeqCst);

        let mut ep_vec = vec![curr_ep];

        let ef_upper = std::cmp::min(ef, 8);
        for lc in (1..=max_l).rev() {
            let (candidates, _) = self.search_layer(query, &ep_vec, ef_upper, lc, storage, &snapshot_nodes);
            let top_cands: Vec<usize> = candidates.into_vec().into_iter().map(|m| m.0.idx).collect();
            if !top_cands.is_empty() {
                ep_vec = top_cands;
            }
        }

        let (candidates_heap, _) = self.search_layer(query, &ep_vec, ef, 0, storage, &snapshot_nodes);

        let mut sorted_cands: Vec<Candidate> = candidates_heap
            .into_vec()
            .into_iter()
            .map(|m| m.0)
            .filter(|c| {
                if c.idx < snapshot_nodes.len() {
                    let id = snapshot_nodes[c.idx].id;
                    if storage.is_deleted(id) {
                        return false;
                    }
                    if let Some(f) = filter {
                        f.matches_id(storage, id)
                    } else {
                        true
                    }
                } else {
                    false
                }
            })
            .collect();

        if filter.is_some() && sorted_cands.len() < k {
            let all_bf = storage.search_brute_force(query, storage.len(), self.metric)?;
            let f_expr = filter.unwrap();
            let filtered_results: Vec<SearchResult> = all_bf
                .into_iter()
                .filter(|r| {
                    if let Some(meta) = &r.metadata {
                        f_expr.matches(meta)
                    } else {
                        false
                    }
                })
                .take(k)
                .collect();
            return Ok(filtered_results);
        }

        sorted_cands.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
        sorted_cands.truncate(k);

        let results = sorted_cands
            .into_iter()
            .map(|c| {
                let id = snapshot_nodes[c.idx].id;
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
        self.nodes.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}


use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct ConcurrentHnswNodeSurrogate {
    pub id: u64,
    pub storage_idx: usize,
    pub level: usize,
    pub neighbors: Vec<Vec<usize>>,
}

#[derive(Serialize, Deserialize)]
pub struct ConcurrentHnswIndexSurrogate {
    pub config: HnswConfig,
    pub metric: MetricType,
    pub nodes: Vec<ConcurrentHnswNodeSurrogate>,
    pub entry_point: usize,
    pub max_layer: usize,
    pub has_entry_point: bool,
}

impl Serialize for ConcurrentHnswIndex {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let nodes = self.nodes.read().iter().map(|n| {
            ConcurrentHnswNodeSurrogate {
                id: n.id,
                storage_idx: n.storage_idx,
                level: n.level,
                neighbors: n.neighbors.iter().map(|l| l.read().clone()).collect(),
            }
        }).collect();
        
        let surrogate = ConcurrentHnswIndexSurrogate {
            config: self.config.clone(),
            metric: self.metric,
            nodes,
            entry_point: self.entry_point.load(AtomicOrdering::SeqCst),
            max_layer: self.max_layer.load(AtomicOrdering::SeqCst),
            has_entry_point: self.has_entry_point.load(AtomicOrdering::SeqCst),
        };
        surrogate.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ConcurrentHnswIndex {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let surrogate = ConcurrentHnswIndexSurrogate::deserialize(deserializer)?;
        let nodes = surrogate.nodes.into_iter().map(|n| {
            std::sync::Arc::new(ConcurrentHnswNode {
                id: n.id,
                storage_idx: n.storage_idx,
                level: n.level,
                neighbors: n.neighbors.into_iter().map(|l| RwLock::new(l)).collect(),
            })
        }).collect();
        
        Ok(Self {
            config: surrogate.config,
            metric: surrogate.metric,
            nodes: RwLock::new(nodes),
            entry_point: AtomicUsize::new(surrogate.entry_point),
            max_layer: AtomicUsize::new(surrogate.max_layer),
            has_entry_point: std::sync::atomic::AtomicBool::new(surrogate.has_entry_point),
            visited_tags: Mutex::new(Vec::new()),
            visit_id_counter: AtomicU32::new(1),
        })
    }
}

impl Clone for ConcurrentHnswIndex {
    fn clone(&self) -> Self {
        let nodes = self.nodes.read().iter().map(|n| {
            std::sync::Arc::new(ConcurrentHnswNode {
                id: n.id,
                storage_idx: n.storage_idx,
                level: n.level,
                neighbors: n.neighbors.iter().map(|l| RwLock::new(l.read().clone())).collect(),
            })
        }).collect();
        
        Self {
            config: self.config.clone(),
            metric: self.metric,
            nodes: RwLock::new(nodes),
            entry_point: AtomicUsize::new(self.entry_point.load(AtomicOrdering::SeqCst)),
            max_layer: AtomicUsize::new(self.max_layer.load(AtomicOrdering::SeqCst)),
            has_entry_point: std::sync::atomic::AtomicBool::new(self.has_entry_point.load(AtomicOrdering::SeqCst)),
            visited_tags: Mutex::new(Vec::new()),
            visit_id_counter: AtomicU32::new(1),
        }
    }
}
