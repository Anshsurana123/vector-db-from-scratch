use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::collections::HashSet;
use std::time::Instant;

use vectordb_core::{FilterExpression, HnswConfig, MetricType, VectorDb};

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
fn test_milestone5_gate() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MILESTONE 5 GATE VERIFICATION TEST ===");

    let num_vectors = 10_000;
    let dim = 128;
    let num_queries = 100;
    let k = 10;
    let categories = vec!["electronics", "clothing", "books", "home"];

    println!("Generating {} normalized {}-dim vectors with JSON metadata (seed = 42)...", num_vectors, dim);
    let mut rng = StdRng::seed_from_u64(42);
    let db = VectorDb::new();
    let collection = db.create_collection_with_config(
        "filtered_col",
        dim,
        MetricType::L2,
        HnswConfig::new(32, 100, 200),
    )?;

    for i in 0..num_vectors {
        let vec = generate_normalized_vector(&mut rng, dim);
        let cat = categories[i % categories.len()];
        let price = (i % 500) as f64 + 10.0; // 10.0 to 509.0
        let rating = 1.0 + (i % 5) as f64 * 0.9; // 1.0 to 4.6

        let meta = serde_json::json!({
            "category": cat,
            "price": price,
            "rating": rating,
            "item_id": i
        });

        collection.insert(i as u64, &vec, Some(meta))?;
    }

    println!("Generating {} query vectors (seed = 12345)...", num_queries);
    let mut query_rng = StdRng::seed_from_u64(12345);
    let mut queries = Vec::with_capacity(num_queries);
    for _ in 0..num_queries {
        queries.push(generate_normalized_vector(&mut query_rng, dim));
    }

    // Define Filter: category == "electronics" AND price <= 150.0 AND rating >= 4.0
    let filter = FilterExpression::And(vec![
        FilterExpression::Eq("category".into(), serde_json::json!("electronics")),
        FilterExpression::Lte("price".into(), 150.0),
        FilterExpression::Gte("rating".into(), 4.0),
    ]);

    println!("\nEvaluating Filtered Search Accuracy & Recall:");
    println!("Filter Expression: category == 'electronics' AND price <= 150.0 AND rating >= 4.0");

    let mut total_hits = 0;
    let mut total_false_positives = 0;
    let start_search = Instant::now();

    for (q_idx, q) in queries.iter().enumerate() {
        // 1. Compute Filtered Brute-Force Ground Truth
        let all_bf = collection.search_brute_force(q, num_vectors)?;
        let filtered_gt: Vec<_> = all_bf
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

        let gt_ids: HashSet<u64> = filtered_gt.iter().map(|r| r.id).collect();

        // 2. Compute Filtered HNSW Search
        let hnsw_results = collection.search_with_filter(q, k, &filter)?;

        // Check for False Positives (results that violate filter)
        for r in &hnsw_results {
            if let Some(meta) = &r.metadata {
                if !filter.matches(meta) {
                    total_false_positives += 1;
                }
            } else {
                total_false_positives += 1;
            }
        }

        let hits = hnsw_results.iter().filter(|r| gt_ids.contains(&r.id)).count();
        total_hits += hits;
    }

    let search_duration = start_search.elapsed();
    let recall = total_hits as f64 / (num_queries * k) as f64;
    let avg_latency_ms = (search_duration.as_secs_f64() * 1000.0) / num_queries as f64;

    println!("\nVerifying Filter Correctness (Zero False Positives):");
    println!("  Total False Positives: {}", total_false_positives);
    assert_eq!(
        total_false_positives, 0,
        "GATE FAILURE: Found {} non-matching vectors in search results",
        total_false_positives
    );

    println!("\nVerifying Filtered Recall@10:");
    println!("  Filtered Search Latency: {:.3} ms / query", avg_latency_ms);
    println!("  Recall@10: {:.4} (Threshold >= 0.95)", recall);

    assert!(
        recall >= 0.95,
        "GATE FAILURE: Filtered Recall@10 ({:.4}) is below threshold 0.95",
        recall
    );

    println!("\nSUCCESS: Milestone 5 Gate Passed cleanly! Filtered Recall@10 = {:.4} >= 0.95 with 0 false positives.", recall);

    Ok(())
}
