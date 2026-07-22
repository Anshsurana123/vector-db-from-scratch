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

pub struct ConcurrentHnswNode {
    pub id: u64,
    pub level: usize,
    /// Fine-grained read-write locks per layer neighbor array
    pub neighbors: Vec<RwLock<Vec<usize>>>,
}

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

    /// Search a single layer `lc` with zero heap allocation per node read
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

            if curr.idx < nodes.len() {
                let node = &nodes[curr.idx];
                if lc < node.neighbors.len() {
                    let nbr_read_guard = node.neighbors[lc].read();
                    for &nbr_idx in nbr_read_guard.iter() {
                        if nbr_idx < visited.len() && visited[nbr_idx] != visit_id {
                            visited[nbr_idx] = visit_id;

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
        }

        (max_results, ep.len())
    }

    /// Insert a single vector into the concurrent HNSW index
    pub fn insert(&self, id: u64, storage: &VectorStorage) -> Result<()> {
        let node_idx = storage.get_idx_by_id(id).ok_or(VectorDbError::VectorNotFound(id))?;
        let q_vec = storage.get_vector_by_idx(node_idx).ok_or(VectorDbError::VectorNotFound(id))?;

        let target_level = self.random_level();
        let mut neighbors = Vec::with_capacity(target_level + 1);
        for _ in 0..=target_level {
            neighbors.push(RwLock::new(Vec::new()));
        }

        let new_node = std::sync::Arc::new(ConcurrentHnswNode {
            id,
            level: target_level,
            neighbors,
        });

        let q_node_idx = {
            let mut nodes_guard = self.nodes.write();
            let idx = nodes_guard.len();
            nodes_guard.push(std::sync::Arc::clone(&new_node));
            idx
        };

        if !self.has_entry_point.swap(true, AtomicOrdering::SeqCst) {
            self.entry_point.store(q_node_idx, AtomicOrdering::SeqCst);
            self.max_layer.store(target_level, AtomicOrdering::SeqCst);
            return Ok(());
        }

        let nodes_read = self.nodes.read();
        let curr_ep = self.entry_point.load(AtomicOrdering::SeqCst);
        let max_l = self.max_layer.load(AtomicOrdering::SeqCst);

        let mut ep_vec = vec![curr_ep];

        // 1. Top layer down to target_level + 1
        for lc in (target_level + 1..=max_l).rev() {
            let (candidates, _) = self.search_layer(q_vec, &ep_vec, 4, lc, storage, &nodes_read);
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
                &nodes_read,
            );

            let candidates_vec: Vec<Candidate> = candidates_heap.into_vec().into_iter().map(|m| m.0).collect();
            let m_max = if lc == 0 { self.config.m_max0 } else { self.config.m };

            // Connect neighbors with fine-grained RwLock
            for cand in candidates_vec.iter().take(m_max) {
                let nbr_idx = cand.idx;
                if nbr_idx < nodes_read.len() {
                    new_node.neighbors[lc].write().push(nbr_idx);
                    nodes_read[nbr_idx].neighbors[lc].write().push(q_node_idx);
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
        if k == 0 || !self.has_entry_point.load(AtomicOrdering::Relaxed) {
            return Ok(Vec::new());
        }

        let ef = std::cmp::max(ef_search, k);
        let nodes_read = self.nodes.read();
        let curr_ep = self.entry_point.load(AtomicOrdering::SeqCst);
        let max_l = self.max_layer.load(AtomicOrdering::SeqCst);

        let mut ep_vec = vec![curr_ep];

        let ef_upper = std::cmp::min(ef, 8);
        for lc in (1..=max_l).rev() {
            let (candidates, _) = self.search_layer(query, &ep_vec, ef_upper, lc, storage, &nodes_read);
            let top_cands: Vec<usize> = candidates.into_vec().into_iter().map(|m| m.0.idx).collect();
            if !top_cands.is_empty() {
                ep_vec = top_cands;
            }
        }

        let (candidates_heap, _) = self.search_layer(query, &ep_vec, ef, 0, storage, &nodes_read);

        let mut sorted_cands: Vec<Candidate> = candidates_heap
            .into_vec()
            .into_iter()
            .map(|m| m.0)
            .filter(|c| {
                if c.idx < nodes_read.len() {
                    let id = nodes_read[c.idx].id;
                    !storage.is_deleted(id)
                } else {
                    false
                }
            })
            .collect();

        sorted_cands.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
        sorted_cands.truncate(k);

        let results = sorted_cands
            .into_iter()
            .map(|c| {
                let id = nodes_read[c.idx].id;
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
