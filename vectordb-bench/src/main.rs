use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::collections::HashSet;
use std::time::Instant;

use vectordb_core::{HnswConfig, MetricType, ProductQuantizer, QuantizedVectorStorage, VectorDb, VectorStorage};

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=========================================================================");
    println!("       PRODUCTION VECTOR DATABASE IN RUST — BENCHMARK SUITE");
    println!("=========================================================================");

    let num_vectors = 10_000;
    let dim = 128;
    let num_queries = 1_000;
    let k = 10;

    println!("\n[1/4] Running Indexing Throughput Benchmark (10,000 128-dim vectors)...");
    let mut rng = StdRng::seed_from_u64(42);
    let mut vectors = Vec::with_capacity(num_vectors);
    for _ in 0..num_vectors {
        vectors.push(generate_normalized_vector(&mut rng, dim));
    }

    let db = VectorDb::new();
    let config = HnswConfig::new(16, 80, 100);

    let start_index = Instant::now();
    let collection = db.create_collection_with_config("bench_col", dim, MetricType::L2, config)?;

    for (i, vec) in vectors.iter().enumerate() {
        collection.insert(i as u64, vec, None)?;
    }

    let index_duration = start_index.elapsed();
    let indexing_throughput = num_vectors as f64 / index_duration.as_secs_f64();

    println!("  Total Indexing Duration: {:.2?}", index_duration);
    println!("  Indexing Throughput: {:.2} vectors / sec", indexing_throughput);

    println!("\n[2/4] Generating {} Query Vectors & Ground Truth...", num_queries);
    let mut query_rng = StdRng::seed_from_u64(12345);
    let mut queries = Vec::with_capacity(num_queries);
    for _ in 0..num_queries {
        queries.push(generate_normalized_vector(&mut query_rng, dim));
    }

    // Compute ground truth for a sample of 100 queries
    let sample_queries = 100;
    let mut raw_storage = VectorStorage::new(dim);
    for (i, vec) in vectors.iter().enumerate() {
        raw_storage.insert(i as u64, vec, None)?;
    }

    let mut ground_truths: Vec<HashSet<u64>> = Vec::with_capacity(sample_queries);
    for q in queries.iter().take(sample_queries) {
        let gt_res = raw_storage.search_brute_force(q, k, MetricType::L2)?;
        let gt_ids: HashSet<u64> = gt_res.into_iter().map(|r| r.id).collect();
        ground_truths.push(gt_ids);
    }

    println!("\n[3/4] Running Search Latency Distribution & Recall Curve Benchmarks...");
    let ef_values = vec![10, 50, 100, 200, 300];

    println!("\n+-----------+--------------+--------------+--------------+--------------+-------------+");
    println!("| efSearch  | Recall@10    | p50 Latency  | p95 Latency  | p99 Latency  | Avg Latency |");
    println!("+-----------+--------------+--------------+--------------+--------------+-------------+");

    let mut gate_p50 = 0.0;
    let mut gate_p95 = 0.0;
    let mut gate_recall_200 = 0.0;

    for &ef in &ef_values {
        let mut latencies = Vec::with_capacity(num_queries);

        for q in &queries {
            let start_q = Instant::now();
            let _res = collection.search_hnsw(q, k, ef)?;
            let elapsed_ms = start_q.elapsed().as_secs_f64() * 1000.0;
            latencies.push(elapsed_ms);
        }

        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let p50 = latencies[(num_queries as f64 * 0.50) as usize];
        let p95 = latencies[(num_queries as f64 * 0.95) as usize];
        let p99 = latencies[(num_queries as f64 * 0.99) as usize];
        let avg = latencies.iter().sum::<f64>() / num_queries as f64;

        // Evaluate recall on 100 sample queries
        let mut total_hits = 0;
        for (q_idx, q) in queries.iter().take(sample_queries).enumerate() {
            let res = collection.search_hnsw(q, k, ef)?;
            let gt_ids = &ground_truths[q_idx];
            let hits = res.iter().filter(|r| gt_ids.contains(&r.id)).count();
            total_hits += hits;
        }

        let recall = total_hits as f64 / (sample_queries * k) as f64;

        if ef == 100 {
            gate_p50 = p50;
            gate_p95 = p95;
        }
        if ef == 200 {
            gate_recall_200 = recall;
        }

        println!(
            "| {:<9} | {:<12.4} | {:<9.3} ms | {:<9.3} ms | {:<9.3} ms | {:<9.3} ms |",
            ef, recall, p50, p95, p99, avg
        );
    }
    println!("+-----------+--------------+--------------+--------------+--------------+-------------+");

    println!("\n[4/4] Running Product Quantization Memory Footprint Benchmark...");
    let raw_bytes = num_vectors * dim * std::mem::size_of::<f32>();
    let m = 64;
    let train_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let quantizer = ProductQuantizer::train(&train_refs, dim, m, 256, 15, MetricType::L2)?;

    let mut q_storage = QuantizedVectorStorage::new(quantizer);
    for (i, vec) in vectors.iter().enumerate() {
        q_storage.insert(i as u64, vec)?;
    }

    let quantized_bytes = num_vectors * m * std::mem::size_of::<u8>();
    let compression_ratio = raw_bytes as f64 / quantized_bytes as f64;

    println!("  Raw Vector Storage RAM: {:.2} MB", raw_bytes as f64 / (1024.0 * 1024.0));
    println!("  Product Quantized Codes RAM: {:.2} MB", quantized_bytes as f64 / (1024.0 * 1024.0));
    println!("  Memory Compression Ratio: {:.2}x", compression_ratio);

    println!("\n=========================================================================");
    println!("                    VERIFYING BENCHMARK GATES");
    println!("=========================================================================");

    println!("  1. Indexing Throughput: {:.2} vecs/sec (Target >= 500)", indexing_throughput);
    assert!(indexing_throughput >= 500.0, "GATE FAILURE: Indexing throughput below 500 vecs/sec");

    println!("  2. p50 Search Latency: {:.3} ms (Target < 1.0 ms)", gate_p50);
    assert!(gate_p50 < 1.0, "GATE FAILURE: p50 latency above 1.0 ms");

    println!("  3. p95 Search Latency: {:.3} ms (Target < 5.0 ms)", gate_p95);
    assert!(gate_p95 < 5.0, "GATE FAILURE: p95 latency above 5.0 ms");

    println!("  4. Recall@10 at ef=200: {:.4} (Target >= 0.90)", gate_recall_200);
    assert!(gate_recall_200 >= 0.90, "GATE FAILURE: Recall@10 at ef=200 below 0.90");

    println!("\nSUCCESS: Milestone 7 Benchmark Gate Passed cleanly across all performance metrics!");

    Ok(())
}
