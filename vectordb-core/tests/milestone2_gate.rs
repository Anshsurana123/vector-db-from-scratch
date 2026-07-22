use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::collections::HashSet;
use std::time::Instant;

use vectordb_core::{HnswConfig, MetricType, VectorDb};

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
fn test_milestone2_gate() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MILESTONE 2 GATE VERIFICATION TEST ===");

    let num_vectors = 100_000;
    let dim = 128;
    let num_queries = 100;
    let k = 10;

    println!("Generating {} normalized {}-dim vectors (seed = 42)...", num_vectors, dim);
    let mut rng = StdRng::seed_from_u64(42);
    let mut vectors = Vec::with_capacity(num_vectors);
    for _ in 0..num_vectors {
        vectors.push(generate_normalized_vector(&mut rng, dim));
    }

    println!("Generating {} query vectors (seed = 12345)...", num_queries);
    let mut query_rng = StdRng::seed_from_u64(12345);
    let mut queries = Vec::with_capacity(num_queries);
    for _ in 0..num_queries {
        queries.push(generate_normalized_vector(&mut query_rng, dim));
    }

    // High recall configuration for 100k vectors: M=48, efConstruction=200
    let config = HnswConfig::new(48, 200, 200);
    let db = VectorDb::new();
    let collection = db.create_collection_with_config("hnsw_100k", dim, MetricType::L2, config)?;

    println!("Building HNSW index on 100,000 vectors (M=48, efConstruction=200)...");
    let start_build = Instant::now();
    for (i, vec) in vectors.iter().enumerate() {
        collection.insert(i as u64, vec, None)?;
        if (i + 1) % 25_000 == 0 {
            println!("  Inserted {}/{} vectors...", i + 1, num_vectors);
        }
    }
    let build_duration = start_build.elapsed();
    println!("HNSW index build complete in {:.2?}", build_duration);

    println!("Computing brute-force ground truth top-10 for {} queries...", num_queries);
    let mut ground_truths: Vec<HashSet<u64>> = Vec::with_capacity(num_queries);
    let start_gt = Instant::now();
    for q in &queries {
        let gt_results = collection.search_brute_force(q, k)?;
        let gt_ids: HashSet<u64> = gt_results.into_iter().map(|r| r.id).collect();
        ground_truths.push(gt_ids);
    }
    println!("Ground truth computed in {:.2?}", start_gt.elapsed());

    let ef_search_values = vec![50, 100, 200, 300];
    let mut recall_results = Vec::new();

    println!("\nEvaluating Recall@10 across ef_search values:");
    println!("{:<12} | {:<12} | {:<15}", "efSearch", "Recall@10", "Avg Latency (ms)");
    println!("{:-<13}+{:-<14}+{:-<17}", "", "", "");

    for &ef in &ef_search_values {
        let mut total_hits = 0;
        let start_search = Instant::now();

        for (q_idx, q) in queries.iter().enumerate() {
            let hnsw_results = collection.search_hnsw(q, k, ef)?;
            let gt_ids = &ground_truths[q_idx];

            let hits = hnsw_results.iter().filter(|r| gt_ids.contains(&r.id)).count();
            total_hits += hits;
        }

        let total_possible = num_queries * k;
        let recall = total_hits as f64 / total_possible as f64;
        let avg_latency_ms = (start_search.elapsed().as_secs_f64() * 1000.0) / num_queries as f64;

        println!("{:<12} | {:<12.4} | {:<15.3}", ef, recall, avg_latency_ms);
        recall_results.push((ef, recall));
    }

    let recall_ef200 = recall_results.iter().find(|(ef, _)| *ef == 200).map(|(_, r)| *r).unwrap_or(0.0);
    println!("\nVerifying Gate Criteria:");
    println!("  1. Recall@10 at efSearch=200: {:.4} (Threshold >= 0.95)", recall_ef200);
    assert!(
        recall_ef200 >= 0.95,
        "GATE FAILURE: Recall@10 at efSearch=200 ({:.4}) is below threshold 0.95",
        recall_ef200
    );

    println!("  2. Monotonic recall growth across ef_search values:");
    for i in 1..recall_results.len() {
        let (prev_ef, prev_rec) = recall_results[i - 1];
        let (curr_ef, curr_rec) = recall_results[i];
        println!("     efSearch {} -> {}: recall {:.4} -> {:.4}", prev_ef, curr_ef, prev_rec, curr_rec);
        assert!(
            curr_rec >= prev_rec,
            "GATE FAILURE: Non-monotonic recall growth between efSearch {} ({:.4}) and {} ({:.4})",
            prev_ef, prev_rec, curr_ef, curr_rec
        );
    }

    println!("\nSUCCESS: Milestone 2 Gate Passed cleanly! Recall@10 = {:.4} >= 0.95 with monotonic growth.", recall_ef200);

    Ok(())
}
