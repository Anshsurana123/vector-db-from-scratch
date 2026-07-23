# Graph Report - vector db from scratch  (2026-07-23)

## Corpus Check
- 63 files · ~27,027 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 657 nodes · 1205 edges · 52 communities (39 shown, 13 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 7 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `611fe66c`
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
- Requirements
- Requirements
- BRIEFING — 2026-07-22T20:50:35Z
- BRIEFING — 2026-07-22T20:54:03Z
- BRIEFING — 2026-07-22T21:00:17Z
- BRIEFING — 2026-07-22T15:30:38Z
- milestone1_gate.rs
- BRIEFING — 2026-07-22T15:23:50Z
- BRIEFING — 2026-07-22T20:50:04Z
- Handoff Report: Requirement R1 — Integrate QueryPlanner & Decision Path Logging
- Handoff Report — Sentinel Initialization
- Progress Tracking
- Project Plan: Vector Database Remediation & 100% Spec Compliance
- Progress Tracker
- explorer_m1/ORIGINAL_REQUEST.md
- explorer_m1/progress.md
- worker_m1_gen2/ORIGINAL_REQUEST.md
- worker_m1_gen2/progress.md
- worker_m1/ORIGINAL_REQUEST.md
- worker_m1/progress.md
- worker_m1_v2/ORIGINAL_REQUEST.md
- BRIEFING — 2026-07-22T15:32:54Z
- test_milestone7_gate_pq_endpoints
- Handoff Report: Requirement R1 — Integrate QueryPlanner & Decision Path Logging
- compare_faiss.py
- reviewer_m1_1/ORIGINAL_REQUEST.md
- reviewer_m1_1/progress.md
- reviewer_m1_2/ORIGINAL_REQUEST.md
- reviewer_m1_2/progress.md

## God Nodes (most connected - your core abstractions)
1. `VectorStorage` - 40 edges
2. `Collection` - 35 edges
3. `VectorDb` - 31 edges
4. `MetricType` - 30 edges
5. `ConcurrentHnswIndex` - 29 edges
6. `HnswIndex` - 22 edges
7. `SearchResult` - 19 edges
8. `HnswConfig` - 18 edges
9. `FilterExpression` - 17 edges
10. `AppError` - 16 edges

## Surprising Connections (you probably didn't know these)
- `app()` --references--> `VectorDb`  [EXTRACTED]
  vectordb-server/src/api.rs → vectordb-core/src/collection.rs
- `AppState` --references--> `VectorDb`  [EXTRACTED]
  vectordb-server/src/api.rs → vectordb-core/src/collection.rs
- `CollectionInfoResponse` --references--> `MetricType`  [EXTRACTED]
  vectordb-server/src/api.rs → vectordb-core/src/distance.rs
- `CreateCollectionRequest` --references--> `MetricType`  [EXTRACTED]
  vectordb-server/src/api.rs → vectordb-core/src/distance.rs
- `SearchRequest` --references--> `FilterExpression`  [EXTRACTED]
  vectordb-server/src/api.rs → vectordb-core/src/filter.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Vector DB Core Subsystems** — project_spec_md_storage_layer_spec, project_spec_md_hnsw_index_spec, project_spec_md_persistence_spec, project_spec_md_filtered_search_spec, project_spec_md_quantization_spec, project_spec_md_api_planner_spec [EXTRACTED 1.00]

## Communities (52 total, 13 thin omitted)

### Community 0 - "Collection API & Concurrent Access"
Cohesion: 0.08
Nodes (29): AtomicU64, Debug, Default, Formatter, Into, Collection, IndexWrapper, Arc (+21 more)

### Community 1 - "Vector Storage CRUD & Tests"
Cohesion: 0.07
Nodes (26): FilterExpression, String, Value, Vec, filter_matching_count(), QueryPlan, QueryPlanner, QueryStrategy (+18 more)

### Community 2 - "HNSW Configuration & Priority Candidates"
Cohesion: 0.12
Nodes (21): Candidate, compute_distance(), HnswIndex, HnswNode, MaxCandidate, MinCandidate, AtomicU32, BinaryHeap (+13 more)

### Community 3 - "HNSW Graph Indexing & Distance Ops"
Cohesion: 0.09
Nodes (31): ArcNode, AtomicBool, AtomicUsize, D, Ok, S, Serialize, Candidate (+23 more)

### Community 4 - "Distance Metrics Subsystem"
Cohesion: 0.17
Nodes (8): Send, Sync, CosineDistance, DistanceMetric, DotProductDistance, get_distance_metric(), L2Distance, Box

### Community 5 - "50k Recall & Verification Gate"
Cohesion: 0.06
Nodes (33): HashSet, Box, Error, Result, test_debug_recall_50k(), generate_normalized_vector(), Box, Error (+25 more)

### Community 6 - "Milestone 1 Ground Truth Verification"
Cohesion: 0.11
Nodes (27): Deserialize, download_sift1m_if_needed(), FaissResults, generate_normalized_vector(), main(), read_fvecs(), Box, Error (+19 more)

### Community 7 - "Candidate Ordering & Priority Queues"
Cohesion: 0.15
Nodes (16): BufWriter, File, AsRef, Option, Path, PathBuf, Result, Self (+8 more)

### Community 8 - "Vector Database Error Handling & Modules"
Cohesion: 0.15
Nodes (15): Candidate, kmeans_plus_plus(), ProductQuantizer, QuantizedVectorStorage, Eq, HashMap, Option, Ord (+7 more)

### Community 9 - "Technical Architecture Specifications"
Cohesion: 0.33
Nodes (6): API Layer & Query Planner Spec, Filtered Search Subsystem Spec, HNSW Index Subsystem Spec, Persistence Subsystem Spec (WAL & Snapshots), Product Quantization Subsystem Spec, Storage Layer Subsystem Spec

### Community 10 - "Milestone 1 Verification Script"
Cohesion: 0.16
Nodes (35): From, IntoResponse, Json, Response, Router, State, Error, String (+27 more)

### Community 11 - "Main Application Binary Entry"
Cohesion: 0.15
Nodes (12): 1. Executive Summary, 2.1 `QueryPlanner` Implementation (`vectordb-core/src/planner.rs`), 2.2 `Collection::search_with_filter` (`vectordb-core/src/collection.rs`), 2.3 `search_vectors` REST API Endpoint (`vectordb-server/src/api.rs`), 2.4 Dependencies (`Cargo.toml`), 2. Codebase Investigation & Current State Analysis, 3.1 Step 1: Add `tracing` to `Cargo.toml`, 3.2 Step 2: Implement Dynamic Routing and Logging in `Collection::search_with_filter` (+4 more)

### Community 15 - "snapshot.rs"
Cohesion: 0.36
Nodes (7): AsRef, Option, Path, PathBuf, Result, SnapshotEngine, test_atomic_snapshot_save_and_load()

### Community 16 - "🏆 Completed Milestones Detail"
Cohesion: 0.15
Nodes (12): 🏆 Completed Milestones Detail, Milestone 1: Storage + Brute-force Exact Search + API — **PASS**, Milestone 2: HNSW Single-threaded Graph Implementation — **PASS**, Milestone 3: Persistence (WAL + Bincode Snapshot + Crash Recovery) — **PASS**, Milestone 4: Product Quantization (PQ) Vector Compression — **PASS**, Milestone 5: Filtering & Metadata Storage — **PASS**, Milestone 6: HTTP API Layer (`vectordb-server`) — **PASS**, Milestone 7: Comprehensive Benchmarking Suite (`vectordb-bench`) — **PASS** (+4 more)

### Community 17 - "Technical Audit Report: Vector Database from Scratch"
Cohesion: 0.14
Nodes (13): 1. Compilation & Syntax Status (Critical Errors), 2. Inaccurate Claims in [PROGRESS.md](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/PROGRESS.md), 3. Subsystem Detailed Compliance Gap Analysis, 4. Leftover Workspace Artifacts, A. Storage Layer & Compaction ([project spec.md §2](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/project%20spec.md#L34-L40)), B. HNSW Graph Index ([project spec.md §3 & §10](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/project%20spec.md#L43-L67)), C. Persistence & Crash Recovery ([project spec.md §4](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/project%20spec.md#L70-L77)), Comprehensive Audit Report: Project Compliance vs. [project spec.md](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/project%20spec.md) (+5 more)

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

### Community 22 - "Requirements"
Cohesion: 0.15
Nodes (12): 2026-07-22T20:50:04Z, Acceptance Criteria, Original User Request, R1. Integrate QueryPlanner & Decision Path Logging, R2. Integrate Product Quantization (PQ) into Collection & REST API, R3. Integrate ConcurrentHnswIndex into Collection Core, R4. Complete Administrative HTTP REST Endpoints, R5. Complete Benchmark Suite & FAISS Baseline (+4 more)

### Community 23 - "Requirements"
Cohesion: 0.15
Nodes (12): 2026-07-22T20:50:04Z, Acceptance Criteria, Original User Request, R1. Integrate QueryPlanner & Decision Path Logging, R2. Integrate Product Quantization (PQ) into Collection & REST API, R3. Integrate ConcurrentHnswIndex into Collection Core, R4. Complete Administrative HTTP REST Endpoints, R5. Complete Benchmark Suite & FAISS Baseline (+4 more)

### Community 24 - "BRIEFING — 2026-07-22T20:50:35Z"
Cohesion: 0.17
Nodes (11): Active Timers, Artifact Index, BRIEFING — 2026-07-22T20:50:35Z, Current Parent, 🔒 Key Constraints, Key Decisions Made, Mission, 🔒 My Identity (+3 more)

### Community 25 - "BRIEFING — 2026-07-22T20:54:03Z"
Cohesion: 0.17
Nodes (11): Artifact Index, BRIEFING — 2026-07-22T21:02:30Z, Change Tracker, Current Parent, 🔒 Key Constraints, Key Decisions Made, Loaded Skills, Mission (+3 more)

### Community 26 - "BRIEFING — 2026-07-22T21:00:17Z"
Cohesion: 0.17
Nodes (11): Artifact Index, BRIEFING — 2026-07-22T21:02:13Z, Change Tracker, Current Parent, 🔒 Key Constraints, Key Decisions Made, Loaded Skills, Mission (+3 more)

### Community 27 - "BRIEFING — 2026-07-22T15:30:38Z"
Cohesion: 0.17
Nodes (11): Artifact Index, BRIEFING — 2026-07-22T15:30:38Z, Change Tracker, Current Parent, 🔒 Key Constraints, Key Decisions Made, Loaded Skills, Mission (+3 more)

### Community 28 - "milestone1_gate.rs"
Cohesion: 0.18
Nodes (10): Artifact Index, Attack Surface, BRIEFING — 2026-07-22T21:03:00Z, Current Parent, 🔒 Key Constraints, Key Decisions Made, Mission, 🔒 My Identity (+2 more)

### Community 29 - "BRIEFING — 2026-07-22T15:23:50Z"
Cohesion: 0.22
Nodes (8): Artifact Index, BRIEFING — 2026-07-22T15:23:50Z, Current Parent, Investigation State, 🔒 Key Constraints, Key Decisions Made, Mission, 🔒 My Identity

### Community 30 - "BRIEFING — 2026-07-22T20:50:04Z"
Cohesion: 0.22
Nodes (8): Artifact Index, BRIEFING — 2026-07-22T20:50:04Z, 🔒 Key Constraints, Mission, 🔒 My Identity, Project Status, User Context, Victory Audit Status

### Community 31 - "Handoff Report: Requirement R1 — Integrate QueryPlanner & Decision Path Logging"
Cohesion: 0.29
Nodes (6): 1. Observation, 2. Logic Chain, 3. Caveats, 4. Conclusion, 5. Verification Method, Handoff Report: Requirement R1 — Integrate QueryPlanner & Decision Path Logging

### Community 32 - "Handoff Report — Sentinel Initialization"
Cohesion: 0.29
Nodes (6): Caveats, Conclusion, Handoff Report — Sentinel Initialization, Logic Chain, Observation, Verification Method

### Community 33 - "Progress Tracking"
Cohesion: 0.33
Nodes (5): Checklist, Current Status, Iteration Status, Progress Tracking, Retrospective & Notes

### Community 34 - "Project Plan: Vector Database Remediation & 100% Spec Compliance"
Cohesion: 0.40
Nodes (4): Architecture Overview, Milestones, Project Plan: Vector Database Remediation & 100% Spec Compliance, Verification & Audit Criteria

### Community 35 - "Progress Tracker"
Cohesion: 0.50
Nodes (3): Progress Steps, Progress Tracker, Task Overview

### Community 44 - "BRIEFING — 2026-07-22T15:32:54Z"
Cohesion: 0.22
Nodes (8): Artifact Index, BRIEFING — 2026-07-22T15:32:54Z, Current Parent, 🔒 Key Constraints, Key Decisions Made, Mission, 🔒 My Identity, Review Scope

### Community 45 - "test_milestone7_gate_pq_endpoints"
Cohesion: 0.29
Nodes (7): generate_normalized_vector(), Box, Error, R, Result, Vec, test_milestone7_gate_pq_endpoints()

### Community 46 - "Handoff Report: Requirement R1 — Integrate QueryPlanner & Decision Path Logging"
Cohesion: 0.29
Nodes (6): 1. Observation, 2. Logic Chain, 3. Caveats, 4. Conclusion, 5. Verification Method, Handoff Report: Requirement R1 — Integrate QueryPlanner & Decision Path Logging

### Community 47 - "compare_faiss.py"
Cohesion: 0.83
Nodes (3): generate_synthetic(), main(), read_fvecs()

## Knowledge Gaps
- **153 isolated node(s):** `2026-07-22T20:50:04Z`, `R1. Integrate QueryPlanner & Decision Path Logging`, `R2. Integrate Product Quantization (PQ) into Collection & REST API`, `R3. Integrate ConcurrentHnswIndex into Collection Core`, `R4. Complete Administrative HTTP REST Endpoints` (+148 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **13 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `VectorStorage` connect `Vector Storage CRUD & Tests` to `Collection API & Concurrent Access`, `HNSW Configuration & Priority Candidates`, `HNSW Graph Indexing & Distance Ops`, `50k Recall & Verification Gate`?**
  _High betweenness centrality (0.116) - this node is a cross-community bridge._
- **Why does `MetricType` connect `Collection API & Concurrent Access` to `Vector Storage CRUD & Tests`, `HNSW Configuration & Priority Candidates`, `HNSW Graph Indexing & Distance Ops`, `Distance Metrics Subsystem`, `Candidate Ordering & Priority Queues`, `Vector Database Error Handling & Modules`, `Milestone 1 Verification Script`?**
  _High betweenness centrality (0.092) - this node is a cross-community bridge._
- **Why does `ConcurrentHnswIndex` connect `HNSW Graph Indexing & Distance Ops` to `Collection API & Concurrent Access`, `Milestone 1 Ground Truth Verification`?**
  _High betweenness centrality (0.064) - this node is a cross-community bridge._
- **What connects `2026-07-22T20:50:04Z`, `R1. Integrate QueryPlanner & Decision Path Logging`, `R2. Integrate Product Quantization (PQ) into Collection & REST API` to the rest of the system?**
  _153 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Collection API & Concurrent Access` be split into smaller, more focused modules?**
  _Cohesion score 0.08289738430583501 - nodes in this community are weakly interconnected._
- **Should `Vector Storage CRUD & Tests` be split into smaller, more focused modules?**
  _Cohesion score 0.07013574660633484 - nodes in this community are weakly interconnected._
- **Should `HNSW Configuration & Priority Candidates` be split into smaller, more focused modules?**
  _Cohesion score 0.12439024390243902 - nodes in this community are weakly interconnected._