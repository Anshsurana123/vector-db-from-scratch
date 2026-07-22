# Project Plan: Vector Database Remediation & 100% Spec Compliance

## Architecture Overview
The project is a high-performance vector database in Rust comprising three crates:
- `vectordb-core`: Vector storage engine, HNSW index (single & concurrent), metadata filter engine, product quantizer, query planner, persistence (WAL & snapshotting).
- `vectordb-server`: Axum HTTP REST API exposing vector indexing, search, administrative endpoints, and quantization interfaces.
- `vectordb-bench`: Benchmarking suite including synthetic benchmarks and SIFT1M vs FAISS comparison harness.

## Milestones

| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | R1: QueryPlanner & Decision Path Logging | Wire `QueryPlanner::plan()` into `Collection::search_with_filter` and REST `search_vectors`. Implement dynamic query routing (`BruteForceScan`, `FilteredScan`, `HnswFiltered`) based on selectivity estimation. Add `tracing::info!` decision path logging. | None | PLANNED |
| M2 | R2: Product Quantization (PQ) Integration | Integrate `ProductQuantizer` and `QuantizedVectorStorage` into `Collection` and `VectorDb`. Expose PQ training & ADC vector search on `Collection`. Expose `POST /collections/:name/quantize` and `use_pq` search param in REST API. | M1 | PLANNED |
| M3 | R3: ConcurrentHnswIndex Integration | Replace single-threaded `HnswIndex` in `Collection` with fine-grained locked `ConcurrentHnswIndex`. Eliminate global write lock contention during vector insertions. | M1, M2 | PLANNED |
| M4 | R4: Administrative REST Endpoints | Implement `POST /collections/:name/snapshot` (atomic snapshot + WAL truncation), `POST /collections/:name/compact` (storage + HNSW graph compaction), and `DELETE /collections/:name` (drop collection). | M1, M2, M3 | PLANNED |
| M5 | R5: FAISS Benchmark Harness & SIFT1M | Create `vectordb-bench/faiss_benchmark.py` harness comparing `vectordb-core` against FAISS HNSW on `SIFT1M`. Report QPS vs Recall Pareto curves and build time vs `efConstruction`. | M1..M4 | PLANNED |
| M6 | R6: Verification & PROGRESS.md Update | Update `PROGRESS.md` to document all milestone achievements, architectural fixes, and empirical benchmark results. Ensure 100% `cargo test` pass across all packages. | M1..M5 | PLANNED |

## Verification & Audit Criteria
- All changes verified by Reviewers, Challengers, and Forensic Auditors.
- Zero integrity violations permitted.
- All unit, integration, and benchmark tests must pass cleanly.
