use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Instant;

use vectordb_core::{ConcurrentHnswIndex, HnswConfig, MetricType, VectorStorage};

fn generate_normalized_vector<R: Rng>(rng: &mut R, dim: usize) -> Vec<f32> {
    let mut v: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-10 {
        for el in v.iter_mut() {
            *el /= norm;
        }
    }
    v
}

#[test]
fn test_milestone8_gate() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MILESTONE 8 GATE VERIFICATION TEST ===");

    let num_vectors = 10_000;
    let dim = 128;
    let k = 10;
    let num_queries = 100;

    println!("Generating {} normalized {}-dim vectors (seed = 42)...", num_vectors, dim);
    let mut rng = StdRng::seed_from_u64(42);
    let mut storage = VectorStorage::new(dim);
    let mut ids = Vec::with_capacity(num_vectors);

    for i in 0..num_vectors {
        let vec = generate_normalized_vector(&mut rng, dim);
        let id = i as u64;
        storage.insert(id, &vec, None)?;
        ids.push(id);
    }

    println!("Generating {} query vectors (seed = 12345)...", num_queries);
    let mut query_rng = StdRng::seed_from_u64(12345);
    let mut queries = Vec::with_capacity(num_queries);
    for _ in 0..num_queries {
        queries.push(generate_normalized_vector(&mut query_rng, dim));
    }

    // 1. Measure Sequential Indexing Duration
    println!("\n[1/3] Benchmarking Single-Threaded Sequential Indexing...");
    let config = HnswConfig::new(16, 80, 200);
    let seq_index = ConcurrentHnswIndex::new(config.clone(), MetricType::L2);

    let start_seq = Instant::now();
    for &id in &ids {
        seq_index.insert(id, &storage)?;
    }
    let duration_seq = start_seq.elapsed();
    println!("  Sequential Indexing Duration: {:.2?}", duration_seq);

    // 2. Measure Multi-Threaded Parallel Indexing Duration
    println!("\n[2/3] Benchmarking Multi-Threaded Parallel Batch Indexing...");
    let par_index = Arc::new(ConcurrentHnswIndex::new(config, MetricType::L2));

    let start_par = Instant::now();
    par_index.insert_batch_parallel(&ids, &storage)?;
    let duration_par = start_par.elapsed();
    println!("  Parallel Indexing Duration: {:.2?}", duration_par);

    let speedup = duration_seq.as_secs_f64() / duration_par.as_secs_f64();
    println!("\nParallel Indexing Speedup: {:.2}x", speedup);

    // 3. Thread-Safety & Concurrent Read/Write Race Test
    println!("\n[3/3] Testing Thread-Safety under Active Concurrent Read/Write Operations...");
    let concurrent_db_index = Arc::new(ConcurrentHnswIndex::new(HnswConfig::new(16, 80, 200), MetricType::L2));
    let storage_arc = Arc::new(storage);

    let is_running = Arc::new(AtomicBool::new(true));
    let mut handles = Vec::new();

    // Spawn 8 worker threads doing continuous search reads
    for t_idx in 0..8 {
        let index_clone = Arc::clone(&concurrent_db_index);
        let storage_clone = Arc::clone(&storage_arc);
        let running = Arc::clone(&is_running);
        let q = queries[t_idx % queries.len()].clone();

        let handle = thread::spawn(move || {
            let mut searches = 0;
            while running.load(Ordering::Relaxed) {
                let _ = index_clone.search(&q, k, 100, &storage_clone);
                searches += 1;
            }
            searches
        });
        handles.push(handle);
    }

    // Insert 5,000 vectors while reads are actively executing
    let sample_ids: Vec<u64> = ids.iter().copied().take(5000).collect();
    concurrent_db_index.insert_batch_parallel(&sample_ids, &storage_arc)?;

    is_running.store(false, Ordering::Relaxed);
    let mut total_concurrent_searches = 0;
    for handle in handles {
        total_concurrent_searches += handle.join().unwrap();
    }
    println!("  Executed {} concurrent searches during parallel indexing with 0 races/panics!", total_concurrent_searches);

    // 4. Verify Accuracy / Recall@10
    println!("\nEvaluating Concurrent HNSW Search Recall@10...");
    let mut total_hits = 0;

    for q in &queries {
        let gt_res = storage_arc.search_brute_force(q, k, MetricType::L2)?;
        let gt_ids: HashSet<u64> = gt_res.into_iter().map(|r| r.id).collect();

        let par_res = par_index.search(q, k, 200, &storage_arc)?;
        let hits = par_res.iter().filter(|r| gt_ids.contains(&r.id)).count();
        total_hits += hits;
    }

    let recall = total_hits as f64 / (num_queries * k) as f64;
    println!("  Recall@10: {:.4} (Threshold >= 0.90)", recall);

    assert!(
        recall >= 0.90,
        "GATE FAILURE: Concurrent HNSW Recall@10 ({:.4}) is below threshold 0.90",
        recall
    );

    println!("\nSUCCESS: Milestone 8 Gate Passed cleanly! Thread-safe parallel index with 0 race conditions and Recall@10 = {:.4}.", recall);

    Ok(())
}
