# Graph Report - .  (2026-07-22)

## Corpus Check
- Corpus is ~6,987 words - fits in a single context window. You may not need a graph.

## Summary
- 167 nodes · 333 edges · 15 communities (13 shown, 2 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 1 edges (avg confidence: 0.8)
- Token cost: 1,200 input · 450 output

## Community Hubs (Navigation)
- Collection API & Concurrent Access
- Vector Storage CRUD & Tests
- HNSW Configuration & Priority Candidates
- HNSW Graph Indexing & Distance Ops
- Distance Metrics Subsystem
- 50k Recall & Verification Gate
- Milestone 1 Ground Truth Verification
- Candidate Ordering & Priority Queues
- Vector Database Error Handling & Modules
- Technical Architecture Specifications
- Graphify Rule System Guidance
- Graphify Workflow Automation

## God Nodes (most connected - your core abstractions)
1. `VectorStorage` - 24 edges
2. `Collection` - 23 edges
3. `HnswIndex` - 20 edges
4. `MetricType` - 14 edges
5. `VectorDb` - 11 edges
6. `MinCandidate` - 10 edges
7. `MaxCandidate` - 10 edges
8. `HnswConfig` - 8 edges
9. `SearchResult` - 8 edges
10. `Candidate` - 8 edges

## Surprising Connections (you probably didn't know these)
- `Collection` --references--> `HnswIndex`  [EXTRACTED]
  vectordb-core/src/collection.rs → vectordb-core/src/hnsw.rs
- `Collection` --references--> `VectorStorage`  [EXTRACTED]
  vectordb-core/src/collection.rs → vectordb-core/src/storage.rs
- `compute_distance()` --references--> `MetricType`  [EXTRACTED]
  vectordb-core/src/hnsw.rs → vectordb-core/src/distance.rs
- `HnswIndex` --references--> `MetricType`  [EXTRACTED]
  vectordb-core/src/hnsw.rs → vectordb-core/src/distance.rs
- `Collection` --references--> `MetricType`  [EXTRACTED]
  vectordb-core/src/collection.rs → vectordb-core/src/distance.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Vector DB Core Subsystems** — project_spec_md_storage_layer_spec, project_spec_md_hnsw_index_spec, project_spec_md_persistence_spec, project_spec_md_filtered_search_spec, project_spec_md_quantization_spec, project_spec_md_api_planner_spec [EXTRACTED 1.00]

## Communities (15 total, 2 thin omitted)

### Community 0 - "Collection API & Concurrent Access"
Cohesion: 0.15
Nodes (14): Arc, Into, RwLock, Collection, HashMap, Option, Result, Self (+6 more)

### Community 1 - "Vector Storage CRUD & Tests"
Cohesion: 0.17
Nodes (8): HashMap, Option, Result, Value, Vec, test_brute_force_search(), test_vector_storage_crud(), VectorStorage

### Community 2 - "HNSW Configuration & Priority Candidates"
Cohesion: 0.20
Nodes (12): Default, Candidate, HnswConfig, MaxCandidate, MinCandidate, Eq, Option, Ord (+4 more)

### Community 3 - "HNSW Graph Indexing & Distance Ops"
Cohesion: 0.23
Nodes (10): AtomicU32, Clone, Mutex, compute_distance(), HnswIndex, HnswNode, HashMap, Result (+2 more)

### Community 4 - "Distance Metrics Subsystem"
Cohesion: 0.17
Nodes (8): Send, Sync, CosineDistance, DistanceMetric, DotProductDistance, get_distance_metric(), L2Distance, Box

### Community 5 - "50k Recall & Verification Gate"
Cohesion: 0.15
Nodes (12): HashSet, R, Box, Error, Result, test_debug_recall_50k(), generate_normalized_vector(), Box (+4 more)

### Community 6 - "Milestone 1 Ground Truth Verification"
Cohesion: 0.25
Nodes (10): PathBuf, DatasetData, get_bench_file_path(), GroundTruthItem, GroundTruthSpotCheck, Box, Error, Result (+2 more)

### Community 7 - "Candidate Ordering & Priority Queues"
Cohesion: 0.29
Nodes (7): Candidate, Eq, Ord, Ordering, PartialEq, PartialOrd, Self

### Community 8 - "Vector Database Error Handling & Modules"
Cohesion: 0.39
Nodes (4): BinaryHeap, Error, String, VectorDbError

### Community 9 - "Technical Architecture Specifications"
Cohesion: 0.33
Nodes (6): API Layer & Query Planner Spec, Filtered Search Subsystem Spec, HNSW Index Subsystem Spec, Persistence Subsystem Spec (WAL & Snapshots), Product Quantization Subsystem Spec, Storage Layer Subsystem Spec

## Knowledge Gaps
- **2 isolated node(s):** `Graphify Rule Instructions`, `Graphify Workflow Guide`
  These have ≤1 connection - possible missing edges or undocumented components.
- **2 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `VectorStorage` connect `Vector Storage CRUD & Tests` to `Collection API & Concurrent Access`, `Vector Database Error Handling & Modules`, `HNSW Graph Indexing & Distance Ops`, `50k Recall & Verification Gate`?**
  _High betweenness centrality (0.281) - this node is a cross-community bridge._
- **Why does `Collection` connect `Collection API & Concurrent Access` to `Vector Storage CRUD & Tests`, `HNSW Graph Indexing & Distance Ops`?**
  _High betweenness centrality (0.171) - this node is a cross-community bridge._
- **Why does `MetricType` connect `Collection API & Concurrent Access` to `Vector Database Error Handling & Modules`, `Vector Storage CRUD & Tests`, `HNSW Graph Indexing & Distance Ops`, `Distance Metrics Subsystem`?**
  _High betweenness centrality (0.101) - this node is a cross-community bridge._
- **What connects `Graphify Rule Instructions`, `Graphify Workflow Guide` to the rest of the system?**
  _2 weakly-connected nodes found - possible documentation gaps or missing edges._