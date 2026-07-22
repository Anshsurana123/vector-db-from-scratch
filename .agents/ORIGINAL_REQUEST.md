# Original User Request

## 2026-07-22T20:50:04Z

# Teamwork Project Prompt — Vector Database Remediation & 100% Spec Compliance

Working directory: c:\Users\ANSH\.gemini\antigravity\scratch\vector db from scratch
Integrity mode: development

## Requirements

### R1. Integrate QueryPlanner & Decision Path Logging
- Wire `QueryPlanner::plan()` into `Collection::search_with_filter` and `vectordb-server` `search_vectors`.
- Dynamically route queries between `BruteForceScan`, `FilteredScan`, and `HnswFiltered` based on estimated filter selectivity.
- Add structured decision path logging (`tracing::info!`) recording the chosen query strategy, selectivity %, and match counts.

### R2. Integrate Product Quantization (PQ) into Collection & REST API
- Integrate `ProductQuantizer` and `QuantizedVectorStorage` into `Collection` and `VectorDb`.
- Provide methods on `Collection` to train PQ codebooks and perform ADC vector search using PQ codes.
- Add REST endpoints/parameters in `vectordb-server`: `POST /collections/:name/quantize` and `use_pq` boolean option in search requests.

### R3. Integrate ConcurrentHnswIndex into Collection Core
- Replace single-threaded `HnswIndex` in `Collection` with fine-grained locked `ConcurrentHnswIndex` (or integrate lock-free/fine-grained concurrency into collection indexing).
- Eliminate global write lock contention during vector insertions so multi-threaded insertions achieve high throughput.

### R4. Complete Administrative HTTP REST Endpoints
- Implement `POST /collections/:name/snapshot` (triggers atomic snapshot & WAL truncation).
- Implement `POST /collections/:name/compact` (triggers vector storage and HNSW graph compaction).
- Implement `DELETE /collections/:name` (drops collection).

### R5. Complete Benchmark Suite & FAISS Baseline
- Create a FAISS benchmark harness (`vectordb-bench/faiss_benchmark.py`) comparing `vectordb-core` against FAISS HNSW on `SIFT1M`.
- Report QPS vs Recall Pareto curves and build time vs `efConstruction`.

### R6. Update PROGRESS.md and Verify All Gates
- Update `PROGRESS.md` to accurately document all milestone achievements, architectural fixes, and empirical benchmark results.
- Ensure all tests (`cargo test`) pass cleanly.

## Acceptance Criteria

### Verification Criteria
- [ ] `cargo test` passes 100% cleanly across all packages (`vectordb-core`, `vectordb-server`, `vectordb-bench`).
- [ ] `search_with_filter` invokes `QueryPlanner` and emits decision logs.
- [ ] PQ search is accessible via `Collection` API and Axum HTTP REST server.
- [ ] Administrative HTTP endpoints (`/snapshot`, `/compact`, `DELETE /collections/:name`) return 200 OK and operate correctly.
- [ ] FAISS comparison harness exists and outputs benchmark comparison metrics.
- [ ] `PROGRESS.md` accurately reflects zero remaining audit flaws.
