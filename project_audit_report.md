# Technical Audit Report: Vector Database from Scratch

> **Target Repository**: [Anshsurana123/vector-db-from-scratch](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch)  
> **Evaluated Specification**: [project spec.md](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/project%20spec.md)  
> **Implementation Status**: [PROGRESS.md](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/PROGRESS.md)  

---

## 📊 EXECUTIVE SUMMARY

The codebase implements a functional proof-of-concept vector database in Rust, featuring an **HNSW graph index**, **Write-Ahead Logging (WAL)**, **Bincode snapshotting**, **Product Quantization (PQ)**, and an **Axum HTTP REST API**.

However, a rigorous line-by-line code audit against `project spec.md` reveals **critical architectural flaws, data loss bugs, orphaned modules, missing query planning components, and incomplete benchmark requirements**.

---

## 🚨 CRITICAL FLAWS & DATA CORRUPTION BUGS

### 1. HTTP API Bypasses WAL (Data Loss on Crash)
- **Location**: [api.rs:128](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/vectordb-server/src/api.rs#L128) & [api.rs:153](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/vectordb-server/src/api.rs#L153)
- **Issue**: The REST endpoints (`POST /collections/:name/insert` and `DELETE /collections/:name/vectors/:id`) invoke `col.insert()` and `col.delete()` directly on `Collection`. However, `Collection` does not have access to the database's `WalWriter`—only `VectorDb::insert_vector` and `VectorDb::delete_vector` append entries to the WAL.
- **Impact**: **Every vector insertion or deletion performed via the HTTP API bypasses the WAL completely.** Upon a server crash or restart, all data ingested or deleted via HTTP is lost upon recovery.

### 2. Broken Indexing on Re-insertion of Deleted IDs
- **Location**: [storage.rs:105-115](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/vectordb-core/src/storage.rs#L105-L115) & [hnsw.rs:463-466](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/vectordb-core/src/hnsw.rs#L463-L466)
- **Issue**: In `VectorStorage::insert`, if an ID was previously marked as deleted, it reuses the existing storage index (`let idx = self.id_to_idx[&id]`) and overwrites the vector floats in `data`. However, in `HnswIndex::insert`:
  ```rust
  let q_node_idx = self.nodes.len();
  self.nodes.push(new_node);
  self.id_to_node_idx.insert(id, q_node_idx);
  ```
  `HnswIndex` always appends a brand-new node at `self.nodes.len()`, causing `id_to_node_idx` to overwrite the mapping, while leaving the old node dangling in the graph with obsolete edges.
- **Impact**: Graph indexing becomes corrupted upon re-inserting deleted IDs.

### 3. Broken and Unused Bitset Mapping in Filter Engine
- **Location**: [filter.rs:52-71](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/vectordb-core/src/filter.rs#L52-L71)
- **Issue**: `FilterExpression::build_bitmap` contains an invalid ID assumption:
  ```rust
  let id = idx as u64; // Fallback mapping, checked via metadata store
  ```
  In `VectorStorage`, vector IDs are user-provided `u64` values (`idx_to_id[idx]`), NOT positional `idx as u64`. Furthermore, `RoaringBitmap` only accepts `u32` IDs (`id <= u32::MAX`). Finally, `build_bitmap` is **never called anywhere in search layer traversals**.
- **Impact**: Roaring bitmaps are neither populated per metadata field nor used in graph search.

---

## 📋 COMPREHENSIVE SPECIFICATION COMPLIANCE MATRIX

| Spec Section | Requirement | Status | Detailed Audit Findings |
| :--- | :--- | :---: | :--- |
| **2. Storage Layer** | Flat `f32` contiguous buffer | **PASS** | `VectorStorage` uses flat `Vec<f32>` with `id * dim` offset indexing. |
| | `id -> metadata` map | **PASS** | `HashMap<u64, serde_json::Value>` implemented. |
| | Tombstone-based deletes + Compaction | **FAIL** | Tombstones marked (`deleted: HashSet<u64>`), but **NO compaction engine exists**. Memory accumulates indefinitely. |
| **3. HNSW Index** | Malkov & Yashunin Multi-layer Graph | **PASS** | Graph structure (`m`, `m_max0`, `efConstruction`, `efSearch`, `mL`) implemented. |
| | Heuristic neighbor selection (Alg 4) | **PASS** | `select_neighbors_heuristic()` enforces spatial diversity pruning. |
| | Trait/Interface distance metric | **PARTIAL** | `DistanceMetric` trait exists in `storage.rs`, but `hnsw.rs` bypasses it using a hardcoded `match` enum function `compute_distance()`. |
| **4. Persistence** | Append-only WAL with CRC32 | **PASS** | Binary framing with `VWAL` header, seq numbers, CRC32 trailers. |
| | Snapshotting preserves graph structure | **PASS** | Bincode serialization of full graph adjacency lists. |
| | WAL Truncation after Snapshot | **FAIL** | `save_snapshot()` writes `.snap.tmp` and renames to `.snap`, but **never truncates or resets `wal.wal`**. |
| | Recovery path (Snapshot + WAL replay) | **FAIL** | Replay mechanism exists, but **HTTP API bypasses WAL**, breaking durability for web users. |
| **5. Filtered Search** | Bitset index per metadata field | **FAIL** | No inverted bitset indexes per field. Filtering parses dynamic JSON expressions per graph node during traversal. |
| | Hybrid Query Planner decision logic | **FAIL** | No pre-search selectivity estimation. Fallback to brute-force occurs only *after* graph search returns `< k` results. |
| **6. Quantization** | Product Quantization (PQ) + K-Means++ | **PASS** | `ProductQuantizer` trains codebooks and computes ADC tables. |
| | HNSW + PQ Graph Integration | **FAIL** | `QuantizedVectorStorage` is completely isolated. **HNSW graph cannot use PQ codes for distance calculations.** |
| **7. API & Planner** | Endpoints: Insert, Search, Delete, Collections | **PARTIAL** | Axum REST API implemented, but `GET /collections` is a stub returning `{"status": "ok"}`. |
| | Query Planner | **FAIL** | **No `QueryPlanner` module exists.** Decision path logging is absent. |
| **8. Benchmarks** | Standard Datasets (SIFT1M / GIST1M) | **FAIL** | Benchmark uses 10,000 synthetic random vectors; SIFT1M / GIST1M are not loaded. |
| | Baseline comparison against FAISS | **FAIL** | No FAISS comparison harness or metrics. |

---

## 🧱 ORPHANED & UNINTEGRATED MODULES

1. **`ConcurrentHnswIndex` ([concurrent_hnsw.rs](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/vectordb-core/src/concurrent_hnsw.rs))**
   - Implements fine-grained `parking_lot::RwLock` per node neighbor array (Milestone 8).
   - **Orphan status**: `Collection` and `VectorDb` only instantiate single-threaded `HnswIndex` wrapped in a global `RwLock<HnswIndex>`. `ConcurrentHnswIndex` is completely disconnected from the REST server and primary database engine.

2. **`QuantizedVectorStorage` ([pq.rs](file:///c:/Users/ANSH/.gemini/antigravity/scratch/vector%20db%20from%20scratch/vectordb-core/src/pq.rs))**
   - Implements Product Quantization with Asymmetric Distance Computation (ADC).
   - **Orphan status**: Operates only as a flat array for brute-force ADC search. Cannot be used by `HnswIndex` or `Collection`.

---

## 🛠️ RECOMMENDED REMEDIATION PLAN

1. **Fix HTTP WAL Persistence**: Update `vectordb-server/src/api.rs` to call `state.db.insert_vector()` and `state.db.delete_vector()` instead of `col.insert()` / `col.delete()`.
2. **Implement Query Planner**: Create a `QueryPlanner` struct in `vectordb-core` that estimates metadata filter selectivity and routes queries dynamically between `BruteForce`, `HnswFiltered`, and `FilteredScan`.
3. **Implement Storage Compaction**: Add a `VectorStorage::compact()` function to rebuild contiguous arrays, re-map IDs, and prune deleted tombstones from HNSW graph nodes.
4. **Integrate WAL Truncation**: Truncate `wal.wal` upon successful execution of `VectorDb::save_snapshot()`.
5. **Complete REST API Endpoints**: Replace the `GET /collections` stub with a real collection listing from `VectorDb::collections`.
6. **Integrate Real Benchmark Datasets**: Update `vectordb-bench` to download/load `SIFT1M` `.fvecs` / `.ivecs` files and execute recall benchmarks against a FAISS Python reference baseline.
