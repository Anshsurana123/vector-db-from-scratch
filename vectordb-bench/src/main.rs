use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;
use serde::Deserialize;

use vectordb_core::{HnswConfig, MetricType, ProductQuantizer, QuantizedVectorStorage, VectorDb, VectorStorage};

#[derive(Deserialize, Debug)]
struct SearchMetrics {
    recall: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    avg: f64,
}

#[derive(Deserialize, Debug)]
struct FaissResults {
    throughput: f64,
    search: HashMap<String, SearchMetrics>,
}

fn download_sift1m_if_needed() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let data_dir = PathBuf::from("vectordb-bench/data/sift");
    std::fs::create_dir_all(&data_dir)?;

    let base_file1 = data_dir.join("sift_base.fvecs");
    let base_file2 = data_dir.join("sift").join("sift_base.fvecs");

    if base_file1.exists() {
        return Ok(data_dir);
    }
    if base_file2.exists() {
        return Ok(data_dir.join("sift"));
    }

    println!("  Executing SIFT1M dataset downloader...");
    let _ = std::process::Command::new("python")
        .args(&["vectordb-bench/download_sift1m.py"])
        .status();

    if data_dir.join("sift").join("sift_base.fvecs").exists() {
        Ok(data_dir.join("sift"))
    } else {
        Ok(data_dir)
    }
}

fn read_fvecs(path: &Path, max_count: Option<usize>) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut vectors = Vec::new();

    loop {
        if let Some(max) = max_count {
            if vectors.len() >= max {
                break;
            }
        }

        let mut dim_buf = [0u8; 4];
        if reader.read_exact(&mut dim_buf).is_err() {
            break;
        }
        let dim = u32::from_le_bytes(dim_buf) as usize;

        let mut vec_buf = vec![0.0f32; dim];
        let mut byte_buf = vec![0u8; dim * 4];
        reader.read_exact(&mut byte_buf)?;

        for i in 0..dim {
            let bytes = [byte_buf[i * 4], byte_buf[i * 4 + 1], byte_buf[i * 4 + 2], byte_buf[i * 4 + 3]];
            vec_buf[i] = f32::from_le_bytes(bytes);
        }

        vectors.push(vec_buf);
    }

    Ok(vectors)
}

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

    println!("  Executing FAISS comparison benchmark...");
    let faiss_status = std::process::Command::new("python")
        .args(&["vectordb-bench/compare_faiss.py"])
        .status();
    
    let mut faiss_results: Option<FaissResults> = None;
    if let Ok(status) = faiss_status {
        if status.success() {
            if let Ok(json_str) = std::fs::read_to_string("vectordb-bench/faiss_results.json") {
                faiss_results = serde_json::from_str(&json_str).ok();
            }
        }
    }

    let data_dir = download_sift1m_if_needed().unwrap_or_else(|_| PathBuf::from("vectordb-bench/data/sift"));
    let base_fvecs = data_dir.join("sift_base.fvecs");
    let query_fvecs = data_dir.join("sift_query.fvecs");

    let (vectors, queries) = if base_fvecs.exists() && query_fvecs.exists() {
        println!("  Loading real SIFT1M dataset files...");
        let vecs = read_fvecs(&base_fvecs, Some(10_000))?;
        let q = read_fvecs(&query_fvecs, Some(1_000))?;
        (vecs, q)
    } else {
        println!("  Using synthetic normalized vectors for benchmark fallback...");
        let mut rng = StdRng::seed_from_u64(42);
        let mut vecs = Vec::with_capacity(10_000);
        for _ in 0..10_000 {
            vecs.push(generate_normalized_vector(&mut rng, 128));
        }
        let mut query_rng = StdRng::seed_from_u64(12345);
        let mut q = Vec::with_capacity(1_000);
        for _ in 0..1_000 {
            q.push(generate_normalized_vector(&mut query_rng, 128));
        }
        (vecs, q)
    };

    let num_vectors = vectors.len();
    let dim = vectors[0].len();
    let num_queries = queries.len();
    let k = 10;

    println!("\n[1/4] Running Indexing Throughput Benchmark ({} {}-dim vectors)...", num_vectors, dim);
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

    println!("\n[2/4] Computing Ground Truth Nearest Neighbors...");
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

    if let Some(faiss) = &faiss_results {
        println!("\n================== RUST VECTORDB VS FAISS HNSW ==================");
        println!("Indexing Throughput:");
        println!("  Rust VectorDB: {:.2} vectors/sec", indexing_throughput);
        println!("  FAISS HNSW:    {:.2} vectors/sec", faiss.throughput);
        println!("---------------------------------------------------------------------------------");
        println!("| efSearch  | Recall@10 (Rust / FAISS) | p95 Latency (Rust / FAISS)            |");
        println!("---------------------------------------------------------------------------------");
    } else {
        println!("\n+-----------+--------------+--------------+--------------+--------------+-------------+");
        println!("| efSearch  | Recall@10    | p50 Latency  | p95 Latency  | p99 Latency  | Avg Latency |");
        println!("+-----------+--------------+--------------+--------------+--------------+-------------+");
    }

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

        if let Some(faiss) = &faiss_results {
            if let Some(f_res) = faiss.search.get(&ef.to_string()) {
                println!(
                    "| {:<9} | {:<5.4} / {:<5.4}            | {:<6.3} ms / {:<6.3} ms          |",
                    ef, recall, f_res.recall, p95, f_res.p95
                );
            }
        } else {
            println!(
                "| {:<9} | {:<12.4} | {:<9.3} ms | {:<9.3} ms | {:<9.3} ms | {:<9.3} ms |",
                ef, recall, p50, p95, p99, avg
            );
        }
    }
    
    if faiss_results.is_some() {
        println!("---------------------------------------------------------------------------------");
    } else {
        println!("+-----------+--------------+--------------+--------------+--------------+-------------+");
    }

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
