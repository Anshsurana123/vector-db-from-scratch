# Investigation & Analysis Report: Requirement R1 — Integrate QueryPlanner & Decision Path Logging

**Author**: `teamwork_preview_explorer` (explorer_m1)  
**Date**: 2026-07-22  
**Milestone**: Milestone 1 (Requirement R1)  
**Target Module(s)**: `vectordb-core` (`src/planner.rs`, `src/collection.rs`, `src/filter.rs`, `Cargo.toml`), `vectordb-server` (`src/api.rs`, `src/main.rs`, `Cargo.toml`)

---

## 1. Executive Summary

Requirement R1 mandates integrating the existing `QueryPlanner` module into vector search execution paths across `vectordb-core` (`Collection::search_with_filter`) and `vectordb-server` (`search_vectors` REST API endpoint). Queries must be dynamically routed between `BruteForceScan`, `FilteredScan`, and `HnswFiltered` based on estimated metadata filter selectivity. Furthermore, structured decision path logging (`tracing::info!`) must record the selected query strategy, filter selectivity percentage, match count, total count, and decision rationale.

---

## 2. Codebase Investigation & Current State Analysis

### 2.1 `QueryPlanner` Implementation (`vectordb-core/src/planner.rs`)
- **Structs & Enums**:
  - `QueryStrategy`: `BruteForceScan`, `HnswFiltered`, `FilteredScan`.
  - `QueryPlan`: Contains `strategy`, `selectivity` (`f32`), `matching_count` (`usize`), `total_count` (`usize`), and `rationale` (`String`).
  - `QueryPlanner::plan(storage: &VectorStorage, filter: Option<&FilterExpression>, k: usize) -> QueryPlan`:
    - Evaluates total vector count via `storage.len()`.
    - If `total == 0`: Returns `QueryStrategy::BruteForceScan` with selectivity `1.0`.
    - If `filter == None`: Returns `QueryStrategy::HnswFiltered` with selectivity `1.0`.
    - If `filter == Some(f)`:
      - Computes matching count via `filter_matching_count(storage, f)`.
      - Computes `selectivity = matching / total`.
      - If `selectivity < 0.10` OR `matching <= k * 2`: Selects `QueryStrategy::FilteredScan` (high selectivity).
      - Else: Selects `QueryStrategy::HnswFiltered` (broad selectivity).

### 2.2 `Collection::search_with_filter` (`vectordb-core/src/collection.rs`)
- **Current Code**:
  ```rust
  pub fn search_with_filter(
      &self,
      query: &[f32],
      k: usize,
      filter: &FilterExpression,
  ) -> Result<Vec<SearchResult>> {
      let storage = self.storage.read();
      let hnsw = self.hnsw.read();
      let ef_search = hnsw.config.ef_search;
      hnsw.search_with_filter(query, k, ef_search, &storage, Some(filter))
  }
  ```
- **Defects / Gaps Identified**:
  1. `QueryPlanner::plan()` is **never called**.
  2. All filtered queries bypass selectivity routing and unconditionally invoke HNSW graph search.
  3. No `tracing::info!` decision path logging is performed.

### 2.3 `search_vectors` REST API Endpoint (`vectordb-server/src/api.rs`)
- **Current Code**:
  ```rust
  async fn search_vectors(
      State(state): State<Arc<AppState>>,
      Path(name): Path<String>,
      Json(req): Json<SearchRequest>,
  ) -> Result<impl IntoResponse, AppError> {
      let col = state.db.get_collection(&name)?;
      let results: Vec<SearchResult> = match req.filter {
          Some(ref filter) => col.search_with_filter(&req.query, req.k, filter)?,
          None => match req.ef_search {
              Some(ef) => col.search_hnsw(&req.query, req.k, ef)?,
              None => col.search(&req.query, req.k)?,
          },
      };
      Ok((StatusCode::OK, Json(results)))
  }
  ```
- **Defects / Gaps Identified**:
  1. Relies on `col.search_with_filter` for filtered queries. Once `Collection::search_with_filter` is fixed, filtered REST queries will automatically invoke `QueryPlanner`.
  2. For unfiltered queries (`req.filter == None`), `QueryPlanner` can also be integrated via `search_with_filter_opt` or inside `Collection` search methods so that all searches log decision paths.

### 2.4 Dependencies (`Cargo.toml`)
- Neither `vectordb-core/Cargo.toml` nor `vectordb-server/Cargo.toml` lists `tracing` in their `[dependencies]`.
- `tracing` must be added to `vectordb-core/Cargo.toml` and `vectordb-server/Cargo.toml`, and `tracing-subscriber` added to `vectordb-server` main binary.

---

## 3. Detailed Fix Strategy & Proposed Changes

### 3.1 Step 1: Add `tracing` to `Cargo.toml`

**`vectordb-core/Cargo.toml`**:
```toml
[dependencies]
thiserror = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bincode = "1.3"
crc32fast = "1.3"
parking_lot = "0.12"
rand = "0.8"
roaring = "0.10"
rayon = "1.8"
tracing = "0.1"
```

**`vectordb-server/Cargo.toml`**:
```toml
[dependencies]
vectordb-core = { path = "../vectordb-core" }
axum = "0.7"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tower-http = { version = "0.5", features = ["trace", "cors"] }
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

### 3.2 Step 2: Implement Dynamic Routing and Logging in `Collection::search_with_filter`

In `vectordb-core/src/collection.rs`:

```rust
use crate::planner::{QueryPlanner, QueryStrategy};

impl Collection {
    pub fn search_with_filter(
        &self,
        query: &[f32],
        k: usize,
        filter: &FilterExpression,
    ) -> Result<Vec<SearchResult>> {
        let storage = self.storage.read();
        let plan = QueryPlanner::plan(&storage, Some(filter), k);

        tracing::info!(
            strategy = ?plan.strategy,
            selectivity = plan.selectivity,
            matching_count = plan.matching_count,
            total_count = plan.total_count,
            rationale = %plan.rationale,
            "Query planner decision executed"
        );

        match plan.strategy {
            QueryStrategy::BruteForceScan => {
                let all_bf = storage.search_brute_force(query, storage.len(), self.metric)?;
                let results = all_bf
                    .into_iter()
                    .filter(|r| {
                        if let Some(meta) = &r.metadata {
                            filter.matches(meta)
                        } else {
                            false
                        }
                    })
                    .take(k)
                    .collect();
                Ok(results)
            }
            QueryStrategy::FilteredScan => {
                let metric = self.metric;
                let mut heap = std::collections::BinaryHeap::with_capacity(k);

                for &id in storage.raw_idx_to_id() {
                    if storage.is_deleted(id) {
                        continue;
                    }
                    if filter.matches_id(&storage, id) {
                        if let Some(vec) = storage.get_vector(id) {
                            let dist = crate::hnsw::compute_distance(metric, query, vec);
                            let cand = crate::storage::Candidate { id, distance: dist };

                            if heap.len() < k {
                                heap.push(cand);
                            } else if let Some(top) = heap.peek() {
                                if cand.distance < top.distance {
                                    heap.pop();
                                    heap.push(cand);
                                }
                            }
                        }
                    }
                }

                let mut results: Vec<SearchResult> = heap
                    .into_sorted_vec()
                    .into_iter()
                    .map(|cand| SearchResult {
                        id: cand.id,
                        distance: cand.distance,
                        metadata: storage.get_metadata(cand.id).cloned(),
                    })
                    .collect();

                results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
                Ok(results)
            }
            QueryStrategy::HnswFiltered => {
                let hnsw = self.hnsw.read();
                let ef_search = hnsw.config.ef_search;
                hnsw.search_with_filter(query, k, ef_search, &storage, Some(filter))
            }
        }
    }
}
```

### 3.3 Step 3: Server Logging Initialization in `vectordb-server/src/main.rs`

In `vectordb-server/src/main.rs`:
```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("Initializing Production Vector Database Server...");

    let db = Arc::new(VectorDb::new());
    let router = app(db);
    ...
```

---

## 4. Verification Plan

1. **Unit & Fast Gate Tests**:
   - Run `cargo test --package vectordb-core --lib planner::tests` to verify `QueryPlanner` thresholds.
   - Run `cargo test --package vectordb-core --test flaw_audit_gate test_flaw_7_query_planner_routing` to verify routing through `Collection::search_with_filter`.

2. **Integration Tests**:
   - Run `cargo test --lib` across packages for fast execution.
   - Run `cargo test --release` when running full integration benchmarks (like `debug_recall`) to prevent unoptimized debug mode timeouts on 50,000-vector builds.

3. **Structured Log Verification**:
   - Run tests with `RUST_LOG=info` to inspect `tracing::info!` output containing `strategy`, `selectivity`, `matching_count`, `total_count`, and `rationale`.
