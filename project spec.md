# Vector Database from Scratch — Technical Spec

## 1. Architecture Overview

```
┌─────────────────────────────────────────────┐
│                API Layer (gRPC/HTTP)          │
│   insert() / search() / delete() / filter()  │
└───────────────────┬───────────────────────────┘
                     │
┌────────────────────▼──────────────────────────┐
│              Query Planner                     │
│  decides: brute-force vs HNSW vs filtered path │
└──────┬─────────────────────────┬────────────────┘
       │                         │
┌──────▼─────────┐      ┌────────▼─────────┐
│  HNSW Index     │      │  Metadata Store   │
│ (in-memory graph)│     │ (inverted index / │
│                  │     │  bitset filters)  │
└──────┬───────────┘      └────────┬───────────┘
       │                            │
┌──────▼────────────────────────────▼───────────┐
│         Storage Engine (vectors + WAL)          │
│    - Raw/quantized vector storage               │
│    - Write-ahead log for durability             │
│    - Periodic snapshot compaction                │
└──────────────────────────────────────────────────┘
```

Four subsystems. Build them in this order: **storage → HNSW → persistence → filtering → quantization → API/planner**. Each one is independently testable before the next depends on it.

---

## 2. Storage Layer

Minimum viable design:
- Flat array of `f32` vectors, fixed dimensionality, contiguous memory (`Vec<Vec<f32>>` is fine to start, but move to a single flat `Vec<f32>` with `id * dim` offset indexing — cache locality matters a lot here once you're doing distance computations in a hot loop).
- A parallel `id -> metadata` map (start with a simple hashmap, JSON blob per id).
- Tombstone-based deletes (mark deleted, compact later) — real deletes in a graph index are expensive, don't implement eager deletion first.

---

## 3. HNSW Index (the core of the project)

This is Malkov & Yashunin's *Hierarchical Navigable Small World* graph. Implement it from the paper, not from a blog post summary — the blog posts skip the parts that matter.

**Structure:** a multi-layer graph. Layer 0 contains all points. Higher layers contain exponentially fewer points, acting as express lanes for approximate navigation.

**Key parameters:**
- `M` — max neighbors per node per layer (typically 12–48). Controls graph connectivity and memory.
- `efConstruction` — size of the dynamic candidate list during insertion (higher = better recall, slower build).
- `efSearch` — same idea at query time (higher = better recall, slower query). This is your main recall/latency knob and should be exposed in the API.
- `mL` — normalization factor for the random level assignment (`level = floor(-ln(uniform(0,1)) * mL)`).

**Insertion algorithm:**
1. Assign the new point a random max layer via the exponential decay formula above.
2. Starting from the top layer of the existing graph, greedily walk down layer by layer, at each layer finding the nearest entry point to descend from (`ef=1` search until you reach the point's assigned top layer).
3. From that layer down to layer 0, run a proper `ef=efConstruction` search to find candidate neighbors, then select `M` neighbors using a **heuristic neighbor selection** (not just "closest M" — the paper's heuristic prefers neighbors that improve graph diversity, avoiding clustering). This heuristic is the part most naive implementations skip, and it's why their recall is bad. Implement it properly.
4. Add bidirectional edges, and if a neighbor now exceeds `M` connections, prune it using the same heuristic.

**Search algorithm:**
1. Start at the top layer's entry point, greedy-search (`ef=1`) down to layer 1.
2. At layer 0, run a proper best-first search with a candidate priority queue of size `ef=efSearch`, expanding neighbors and maintaining the top-`ef` closest, until no closer candidates are found.
3. Return top-`k` from the final candidate set.

**Distance metric:** implement cosine, L2, and dot product — make it a runtime-selectable trait/interface, not hardcoded. This alone is a good abstraction-design signal in a code review.

---

## 4. Persistence

- **Write-ahead log (WAL):** every insert/delete is appended to an append-only log *before* being applied to the in-memory structure. Format: `[op_type][id][vector_bytes][metadata_bytes][checksum]`. On crash recovery, replay the WAL from the last snapshot.
- **Snapshotting:** periodically (size- or time-triggered) serialize the full HNSW graph + vector store to disk, then truncate the WAL. Snapshot format needs to preserve exact graph structure (adjacency lists per layer) — don't just dump vectors and rebuild the graph on load, that defeats the purpose of persistence for large datasets (rebuild time is exactly what you're trying to avoid).
- **Recovery path:** load latest snapshot, replay WAL entries after the snapshot's log offset.

This subsystem is what separates a "toy demo" from something a reviewer takes seriously — most portfolio vector search projects skip durability entirely.

---

## 5. Filtered Search

The hard part: combining `vector_search(query, k)` with `WHERE metadata.field = value` *without* falling back to brute-force scan-then-filter (which destroys your ANN speedup when filters are selective).

Approaches, in increasing sophistication:
1. **Post-filtering (naive):** run ANN search for `k * oversample_factor` results, then filter, then truncate to `k`. Simple but breaks down when the filter is highly selective (you might get zero valid results back).
2. **Pre-filtering with bitsets:** maintain a roaring-bitmap or bitset per indexed metadata field. During HNSW graph traversal, only consider candidate neighbors whose id is present in the filter bitset. This means threading the filter into the graph search itself, not applying it after.
3. **Hybrid (what production systems do):** estimate filter selectivity; if the filter is very selective, brute-force scan the filtered subset directly (skip the graph entirely — for a small enough candidate set, brute force is faster than graph traversal overhead); if the filter is broad, use in-search bitset filtering.

Implement #2 as your baseline, #3 as your query-planner's decision logic — that decision logic is a good thing to have a benchmark chart for.

---

## 6. Quantization

Once correctness is proven, add **Product Quantization (PQ)**:
- Split each vector into `m` subvectors.
- For each subspace, run k-means to learn a codebook of `k` centroids (typically 256, so each subvector index fits in a byte).
- Store each vector as `m` codebook indices instead of raw floats — massive memory reduction (e.g., 128-dim f32 vector = 512 bytes → PQ with m=16 = 16 bytes).
- Distance computation becomes an asymmetric distance calculation using precomputed distance tables (query vector to each subspace centroid), not full decompression.

This is optional for MVP but is what makes the project "not a toy" — it's the component that shows you understand the memory/accuracy tradeoff explicitly rather than just wrapping an algorithm.

---

## 7. API Layer + Query Planner

- gRPC (or HTTP/JSON if you want faster iteration) with endpoints: `Insert`, `Search`, `Delete`, `CreateCollection`.
- Query planner logic: given collection size and filter selectivity estimate, choose brute-force vs HNSW vs hybrid-filtered path. Log which path was chosen — this becomes a debug/observability feature that's genuinely useful in a demo.

---

## 8. Tech Stack Recommendation

**Rust** if you want the systems-credibility signal (this is what Qdrant is written in). You'll deal with manual memory layout for the flat vector array, `unsafe` for SIMD distance computations if you go that far, and real concurrency primitives (RwLock per layer, or lock-free graph updates if you're ambitious).

**Go** is a reasonable middle ground — easier concurrency model, still compiled/fast, less fighting the borrow checker while you're focused on algorithm correctness.

Don't do this in Python for the core engine — you can use Python for a client SDK on top, but the whole value proposition is "I understand systems-level performance," and an interpreted language undercuts that claim. If you want Python at all, use it only for the benchmarking harness.

---

## 9. Benchmark Plan (this is your portfolio centerpiece)

- **Dataset:** SIFT1M or GIST1M (standard ANN-benchmarks datasets, 1M–1M+ vectors with ground-truth nearest neighbors provided).
- **Metrics to report:**
  - Recall@k (10, 100) vs `efSearch`, plotted as a curve — this shows you understand the tunable tradeoff, not just a single number.
  - Queries per second (QPS) vs recall — the standard ANN-benchmarks-style Pareto curve.
  - p50/p95/p99 latency.
  - Memory footprint, with and without PQ.
  - Build time vs `efConstruction`.
- **Baseline comparison:** brute-force exact search (as your recall=100% reference), and ideally a comparison against FAISS's HNSW implementation on the same dataset/params — if your numbers are in the same ballpark, that's a strong, concrete portfolio claim ("achieves X% of FAISS's QPS at equivalent recall").

---

## 10. Common Pitfalls (things that will bite you)

- Skipping the heuristic neighbor selection during insertion → recall silently degrades as the graph grows, and it's hard to debug because insertion "succeeds," it just builds a bad graph.
- Not thread-safing the graph if you add concurrent inserts — HNSW insertion mutates shared adjacency lists; naive locking (global mutex) kills your build throughput, but incorrect fine-grained locking gives you data races that corrupt the graph silently.
- Rebuilding the whole index on every snapshot load instead of deserializing the graph structure directly — defeats the point of persistence at scale.
- Forgetting entry-point selection matters: if you don't track/update the graph's global entry point correctly on inserts, search quality degrades in ways that are annoying to trace back.

---

## 11. Stretch Goals (only after MVP + benchmarks are solid)

- Disk-backed index for datasets larger than RAM (mmap-based vector storage).
- Incremental/background compaction instead of blocking snapshot writes.
- Multi-tenant collections with per-collection HNSW graphs sharing a storage pool.
- Basic replication (leader + WAL shipping to replicas) if you want to gesture at distributed-systems chops.

---

## Milestone Order (suggested)

1. Flat storage + brute-force exact search + basic API — get end-to-end plumbing working first.
2. HNSW insert/search, single-threaded, no persistence. Validate recall against brute-force on a small dataset.
3. WAL + snapshot persistence + crash recovery test.
4. Metadata filtering (bitset-based).
5. Benchmark suite against SIFT1M + FAISS comparison — write this up properly, it's your README centerpiece.
6. Quantization (PQ) — optional but strong differentiator.
7. Stretch goals if time remains.

Steps 1–5 are the finishable, defensible core. Don't start step 6 until 1–5 produce clean benchmark numbers.
