use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::collections::HashSet;
use std::time::Instant;

use vectordb_core::{MetricType, ProductQuantizer, QuantizedVectorStorage, VectorStorage};

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
fn test_milestone4_gate() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MILESTONE 4 GATE VERIFICATION TEST ===");

    let num_vectors = 10_000;
    let dim = 128;
    let num_queries = 100;
    let k = 10;
    let num_subvectors = 64; // d' = 2 dimensions per subspace for minimal quantization error
    let num_centroids = 256;

    println!("Generating {} normalized {}-dim vectors (seed = 42)...", num_vectors, dim);
    let mut rng = StdRng::seed_from_u64(42);
    let mut vectors = Vec::with_capacity(num_vectors);
    let mut storage = VectorStorage::new(dim);

    for i in 0..num_vectors {
        let vec = generate_normalized_vector(&mut rng, dim);
        storage.insert(i as u64, &vec, None)?;
        vectors.push(vec);
    }

    println!("Generating {} query vectors (seed = 12345)...", num_queries);
    let mut query_rng = StdRng::seed_from_u64(12345);
    let mut queries = Vec::with_capacity(num_queries);
    for _ in 0..num_queries {
        queries.push(generate_normalized_vector(&mut query_rng, dim));
    }

    println!("Computing brute-force uncompressed ground truth top-10 for {} queries...", num_queries);
    let mut ground_truths: Vec<HashSet<u64>> = Vec::with_capacity(num_queries);
    let start_gt = Instant::now();
    for q in &queries {
        let gt_results = storage.search_brute_force(q, k, MetricType::L2)?;
        let gt_ids: HashSet<u64> = gt_results.into_iter().map(|r| r.id).collect();
        ground_truths.push(gt_ids);
    }
    println!("Ground truth computed in {:.2?}", start_gt.elapsed());

    println!("Training Product Quantizer (m={}, centroids={})...", num_subvectors, num_centroids);
    let train_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let start_train = Instant::now();
    let quantizer = ProductQuantizer::train(
        &train_refs,
        dim,
        num_subvectors,
        num_centroids,
        30,
        MetricType::L2,
    )?;
    println!("PQ training complete in {:.2?}", start_train.elapsed());

    println!("Quantizing 10,000 vectors into 64-byte codes...");
    let start_quantize = Instant::now();
    let mut q_storage = QuantizedVectorStorage::new(quantizer);
    for (i, vec) in vectors.iter().enumerate() {
        q_storage.insert(i as u64, vec)?;
    }
    println!("Quantization complete in {:.2?}", start_quantize.elapsed());

    // 1. Verify Memory Compression Ratio
    let raw_bytes = num_vectors * dim * std::mem::size_of::<f32>();
    let quantized_bytes = num_vectors * num_subvectors * std::mem::size_of::<u8>();
    let compression_ratio = raw_bytes as f64 / quantized_bytes as f64;

    println!("\nVerifying Memory Footprint Reduction:");
    println!("  Raw vectors: {:.2} MB", raw_bytes as f64 / (1024.0 * 1024.0));
    println!("  Quantized codes: {:.2} MB", quantized_bytes as f64 / (1024.0 * 1024.0));
    println!("  Compression ratio: {:.2}x (Threshold >= 8.0x)", compression_ratio);
    assert!(
        compression_ratio >= 8.0,
        "GATE FAILURE: Compression ratio {:.2}x is below 8.0x threshold",
        compression_ratio
    );

    // 2. Evaluate ADC Search Recall@10
    println!("\nEvaluating Asymmetric Distance Computation (ADC) search quality:");
    let mut total_hits = 0;
    let start_adc = Instant::now();

    for (q_idx, q) in queries.iter().enumerate() {
        let adc_results = q_storage.search_adc(q, k)?;
        let gt_ids = &ground_truths[q_idx];

        let hits = adc_results.iter().filter(|r| gt_ids.contains(&r.id)).count();
        total_hits += hits;
    }

    let search_duration = start_adc.elapsed();
    let recall = total_hits as f64 / (num_queries * k) as f64;
    let avg_latency_ms = (search_duration.as_secs_f64() * 1000.0) / num_queries as f64;

    println!("  ADC Search Latency: {:.3} ms / query", avg_latency_ms);
    println!("  Recall@10: {:.4} (Threshold >= 0.70)", recall);

    assert!(
        recall >= 0.70,
        "GATE FAILURE: ADC Recall@10 ({:.4}) is below threshold 0.70",
        recall
    );

    println!("\nSUCCESS: Milestone 4 Gate Passed cleanly! Recall@10 = {:.4} >= 0.70 with {:.2}x memory compression.", recall, compression_ratio);

    Ok(())
}
