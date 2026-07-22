# Handoff Report: Requirement R1 — Integrate QueryPlanner & Decision Path Logging

**Author**: `teamwork_preview_explorer` (explorer_m1)  
**Date**: 2026-07-22  
**Recipient**: Orchestrator / Implementer

---

## 1. Observation

1. **`vectordb-core/src/planner.rs` (lines 24-78)**:
   - `QueryPlanner::plan(storage: &VectorStorage, filter: Option<&FilterExpression>, k: usize) -> QueryPlan` exists and calculates `matching_count`, `selectivity`, and returns `QueryStrategy::FilteredScan` (if `selectivity < 0.10` or `matching <= k * 2`) or `QueryStrategy::HnswFiltered` (otherwise), or `QueryStrategy::BruteForceScan` (if `storage.len() == 0`).
2. **`vectordb-core/src/collection.rs` (lines 88-98)**:
   - `Collection::search_with_filter` currently reads `storage` and `hnsw` and calls `hnsw.search_with_filter(...)` directly.
   - `QueryPlanner::plan()` is not called anywhere in `collection.rs`.
   - `tracing::info!` is not called anywhere in `collection.rs`.
3. **`vectordb-server/src/api.rs` (lines 141-155)**:
   - `search_vectors` delegates filtered requests (`Some(ref filter)`) directly to `col.search_with_filter(&req.query, req.k, filter)?`.
4. **`vectordb-core/Cargo.toml` & `vectordb-server/Cargo.toml`**:
   - `tracing` is not included in `dependencies` in `vectordb-core/Cargo.toml`.
   - `tracing` and `tracing-subscriber` are not included in `vectordb-server/Cargo.toml`.
5. **`vectordb-core/tests/flaw_audit_gate.rs` (lines 80-108)**:
   - `test_flaw_7_query_planner_routing` tests `col.search_with_filter`, but fails to test whether `tracing` logs are emitted or whether `FilteredScan` vs `HnswFiltered` vs `BruteForceScan` branches are actually executed.

---

## 2. Logic Chain

1. **Observation 1 & Observation 2**: `QueryPlanner::plan` is defined in `planner.rs` to output a `QueryPlan` with routing strategy and selectivity metadata, but `Collection::search_with_filter` completely ignores `QueryPlanner::plan` and calls `hnsw.search_with_filter` unconditionally.
2. **Observation 3**: `vectordb-server/src/api.rs` delegates filtered search requests directly to `Collection::search_with_filter`. Therefore, wiring `QueryPlanner::plan` into `Collection::search_with_filter` will automatically fix both core collection search and REST API vector search.
3. **Observation 4**: In order to emit structured decision path logs (`tracing::info!`) recording chosen strategy, selectivity %, matching counts, and rationale, `tracing` dependency must be added to `vectordb-core/Cargo.toml` and `vectordb-server/Cargo.toml`.
4. **Observation 2 & Fix Strategy**: When `Collection::search_with_filter` executes:
   - It calls `let plan = QueryPlanner::plan(&storage, Some(filter), k)`.
   - It logs decision metrics via `tracing::info!`.
   - It branches on `plan.strategy`:
     - `QueryStrategy::BruteForceScan`: Brute-force scan over matching storage vectors.
     - `QueryStrategy::FilteredScan`: Scan metadata in storage first, calculate distance only for matching non-deleted vectors.
     - `QueryStrategy::HnswFiltered`: Standard HNSW graph search with filter.

---

## 3. Caveats

- Filter selectivity calculation (`filter_matching_count`) iterates storage metadata. For in-memory storage, this is $O(N)$ metadata checks, which is fast and accurate.
- If total storage vector count is 0, `QueryPlanner::plan` returns `BruteForceScan` with selectivity 1.0.

---

## 4. Conclusion

Requirement R1 is clear and fully specified. The fix requires:
1. Adding `tracing` dependencies to `vectordb-core/Cargo.toml` and `vectordb-server/Cargo.toml`.
2. Updating `Collection::search_with_filter` in `vectordb-core/src/collection.rs` to invoke `QueryPlanner::plan()`, log decision paths with `tracing::info!`, and execute the corresponding search strategy (`BruteForceScan`, `FilteredScan`, `HnswFiltered`).
3. Initializing `tracing-subscriber` in `vectordb-server/src/main.rs`.
4. Adding tests to verify decision logging and routing behavior.

Detailed implementation analysis and diff snippets have been documented in `.agents/explorer_m1/analysis.md`.

---

## 5. Verification Method

1. **Run Unit & Gate Tests**:
   - `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --package vectordb-core --lib planner::tests`
   - `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --package vectordb-core --test flaw_audit_gate test_flaw_7_query_planner_routing`
2. **Run Library & Server Integration Tests**:
   - `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib`
   - `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --release` (for full benchmarks like `debug_recall`)
3. **Verify Logging Output**:
   - Set environment variable `$env:RUST_LOG="info"` and run `cargo test` to observe structured `tracing::info!` output.
