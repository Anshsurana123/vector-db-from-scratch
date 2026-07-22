# Graph Report - vector db from scratch  (2026-07-22)

## Corpus Check
- 31 files · ~17,953 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 423 nodes · 894 edges · 22 communities (20 shown, 2 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 5 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `c3b01aea`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

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
- Milestone 1 Verification Script
- Main Application Binary Entry
- Graphify Rule System Guidance
- Graphify Workflow Automation
- snapshot.rs
- 🏆 Completed Milestones Detail
- Technical Audit Report: Vector Database from Scratch
- test_milestone3_wal_and_snapshot_recovery
- test_milestone6_gate
- flaw_audit_gate.rs
- main

## God Nodes (most connected - your core abstractions)
1. `VectorStorage` - 39 edges
2. `Collection` - 28 edges
3. `VectorDb` - 28 edges
4. `MetricType` - 27 edges
5. `HnswIndex` - 24 edges
6. `ConcurrentHnswIndex` - 18 edges
7. `SearchResult` - 17 edges
8. `FilterExpression` - 16 edges
9. `HnswConfig` - 15 edges
10. `AppError` - 12 edges

## Surprising Connections (you probably didn't know these)
- `app()` --references--> `VectorDb`  [EXTRACTED]
  vectordb-server/src/api.rs → vectordb-core/src/collection.rs
- `AppState` --references--> `VectorDb`  [EXTRACTED]
  vectordb-server/src/api.rs → vectordb-core/src/collection.rs
- `CollectionInfoResponse` --references--> `MetricType`  [EXTRACTED]
  vectordb-server/src/api.rs → vectordb-core/src/distance.rs
- `CreateCollectionRequest` --references--> `MetricType`  [EXTRACTED]
  vectordb-server/src/api.rs → vectordb-core/src/distance.rs
- `AppError` --references--> `VectorDbError`  [EXTRACTED]
  vectordb-server/src/api.rs → vectordb-core/src/error.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Vector DB Core Subsystems** — project_spec_md_storage_layer_spec, project_spec_md_hnsw_index_spec, project_spec_md_persistence_spec, project_spec_md_filtered_search_spec, project_spec_md_quantization_spec, project_spec_md_api_planner_spec [EXTRACTED 1.00]

## Communities (22 total, 2 thin omitted)

### Community 0 - "Collection API & Concurrent Access"
Cohesion: 0.11
Nodes (20): AtomicU64, Debug, Formatter, Into, Collection, Arc, AsRef, HashMap (+12 more)

### Community 1 - "Vector Storage CRUD & Tests"
Cohesion: 0.11
Nodes (15): Candidate, Eq, HashMap, Option, Ord, Ordering, PartialEq, PartialOrd (+7 more)

### Community 2 - "HNSW Configuration & Priority Candidates"
Cohesion: 0.11
Nodes (24): Clone, Default, Candidate, compute_distance(), HnswConfig, HnswIndex, HnswNode, MaxCandidate (+16 more)

### Community 3 - "HNSW Graph Indexing & Distance Ops"
Cohesion: 0.12
Nodes (21): ArcNode, AtomicBool, AtomicUsize, Candidate, ConcurrentHnswIndex, ConcurrentHnswNode, MaxCandidate, MinCandidate (+13 more)

### Community 4 - "Distance Metrics Subsystem"
Cohesion: 0.17
Nodes (8): Send, Sync, CosineDistance, DistanceMetric, DotProductDistance, get_distance_metric(), L2Distance, Box

### Community 5 - "50k Recall & Verification Gate"
Cohesion: 0.06
Nodes (33): HashSet, Box, Error, Result, test_debug_recall_50k(), generate_normalized_vector(), Box, Error (+25 more)

### Community 6 - "Milestone 1 Ground Truth Verification"
Cohesion: 0.13
Nodes (23): File, download_sift1m_if_needed(), generate_normalized_vector(), main(), read_fvecs(), Box, Error, Option (+15 more)

### Community 7 - "Candidate Ordering & Priority Queues"
Cohesion: 0.17
Nodes (15): BufWriter, AsRef, Option, Path, PathBuf, Result, Self, String (+7 more)

### Community 8 - "Vector Database Error Handling & Modules"
Cohesion: 0.12
Nodes (19): Error, String, VectorDbError, Candidate, kmeans_plus_plus(), ProductQuantizer, QuantizedVectorStorage, Eq (+11 more)

### Community 9 - "Technical Architecture Specifications"
Cohesion: 0.33
Nodes (6): API Layer & Query Planner Spec, Filtered Search Subsystem Spec, HNSW Index Subsystem Spec, Persistence Subsystem Spec (WAL & Snapshots), Product Quantization Subsystem Spec, Storage Layer Subsystem Spec

### Community 10 - "Milestone 1 Verification Script"
Cohesion: 0.20
Nodes (26): From, IntoResponse, Json, Response, Router, State, app(), AppError (+18 more)

### Community 11 - "Main Application Binary Entry"
Cohesion: 0.16
Nodes (13): RoaringBitmap, FilterExpression, Result, String, Value, Vec, filter_matching_count(), QueryPlan (+5 more)

### Community 15 - "snapshot.rs"
Cohesion: 0.25
Nodes (11): CollectionSnapshotData, DbSnapshotData, AsRef, Option, Path, PathBuf, Result, String (+3 more)

### Community 16 - "🏆 Completed Milestones Detail"
Cohesion: 0.17
Nodes (11): 🏆 Completed Milestones Detail, Milestone 1: Storage + Brute-force Exact Search + API — **PASS**, Milestone 2: HNSW Single-threaded Graph Implementation — **PASS**, Milestone 3: Persistence (WAL + Bincode Snapshot + Crash Recovery) — **PASS**, Milestone 4: Product Quantization (PQ) Vector Compression — **PASS**, Milestone 5: Filtering & Metadata Storage — **PASS**, Milestone 6: HTTP API Layer (`vectordb-server`) — **PASS**, Milestone 7: Comprehensive Benchmarking Suite (`vectordb-bench`) — **PASS** (+3 more)

### Community 17 - "Technical Audit Report: Vector Database from Scratch"
Cohesion: 0.20
Nodes (9): 1. HTTP API Bypasses WAL (Data Loss on Crash), 2. Broken Indexing on Re-insertion of Deleted IDs, 3. Broken and Unused Bitset Mapping in Filter Engine, 📋 COMPREHENSIVE SPECIFICATION COMPLIANCE MATRIX, 🚨 CRITICAL FLAWS & DATA CORRUPTION BUGS, 📊 EXECUTIVE SUMMARY, 🧱 ORPHANED & UNINTEGRATED MODULES, 🛠️ RECOMMENDED REMEDIATION PLAN (+1 more)

### Community 18 - "test_milestone3_wal_and_snapshot_recovery"
Cohesion: 0.29
Nodes (7): generate_normalized_vector(), Box, Error, R, Result, Vec, test_milestone3_wal_and_snapshot_recovery()

### Community 19 - "test_milestone6_gate"
Cohesion: 0.29
Nodes (7): generate_normalized_vector(), Box, Error, R, Result, Vec, test_milestone6_gate()

### Community 20 - "flaw_audit_gate.rs"
Cohesion: 0.60
Nodes (4): Result, test_flaw_1_and_4_http_wal_and_snapshot_truncation(), test_flaw_2_reinsertion_and_compaction(), test_flaw_7_query_planner_routing()

### Community 21 - "main"
Cohesion: 0.40
Nodes (4): main(), Box, Error, Result

## Knowledge Gaps
- **18 isolated node(s):** `📊 Milestone Summary`, `Milestone 1: Storage + Brute-force Exact Search + API — **PASS**`, `Milestone 2: HNSW Single-threaded Graph Implementation — **PASS**`, `Milestone 3: Persistence (WAL + Bincode Snapshot + Crash Recovery) — **PASS**`, `Milestone 4: Product Quantization (PQ) Vector Compression — **PASS**` (+13 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **2 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `VectorStorage` connect `Vector Storage CRUD & Tests` to `Collection API & Concurrent Access`, `HNSW Configuration & Priority Candidates`, `HNSW Graph Indexing & Distance Ops`, `50k Recall & Verification Gate`, `Main Application Binary Entry`, `snapshot.rs`?**
  _High betweenness centrality (0.269) - this node is a cross-community bridge._
- **Why does `MetricType` connect `Collection API & Concurrent Access` to `Vector Storage CRUD & Tests`, `HNSW Configuration & Priority Candidates`, `HNSW Graph Indexing & Distance Ops`, `Distance Metrics Subsystem`, `Candidate Ordering & Priority Queues`, `Vector Database Error Handling & Modules`, `Milestone 1 Verification Script`, `snapshot.rs`?**
  _High betweenness centrality (0.174) - this node is a cross-community bridge._
- **Why does `Collection` connect `Collection API & Concurrent Access` to `Vector Storage CRUD & Tests`, `HNSW Configuration & Priority Candidates`?**
  _High betweenness centrality (0.113) - this node is a cross-community bridge._
- **What connects `📊 Milestone Summary`, `Milestone 1: Storage + Brute-force Exact Search + API — **PASS**`, `Milestone 2: HNSW Single-threaded Graph Implementation — **PASS**` to the rest of the system?**
  _18 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Collection API & Concurrent Access` be split into smaller, more focused modules?**
  _Cohesion score 0.1054421768707483 - nodes in this community are weakly interconnected._
- **Should `Vector Storage CRUD & Tests` be split into smaller, more focused modules?**
  _Cohesion score 0.11092436974789915 - nodes in this community are weakly interconnected._
- **Should `HNSW Configuration & Priority Candidates` be split into smaller, more focused modules?**
  _Cohesion score 0.1147086031452359 - nodes in this community are weakly interconnected._