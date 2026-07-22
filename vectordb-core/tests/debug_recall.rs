use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::collections::HashSet;
use std::time::Instant;

use vectordb_core::{HnswConfig, MetricType, VectorDb};

#[test]
fn test_debug_recall_50k() -> Result<(), Box<dyn std::error::Error>> {
    let num_vectors = 50_000;
    let dim = 128;
    let num_queries = 50;
    let k = 10;

    println!("Generating {} random {}-dim vectors...", num_vectors, dim);
    let mut rng = StdRng::seed_from_u64(42);
    let mut vectors = Vec::with_capacity(num_vectors);
    for _ in 0..num_vectors {
        let mut v: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-10 {
            for el in v.iter_mut() { *el /= norm; }
        }
        vectors.push(v);
    }

    let mut query_rng = StdRng::seed_from_u64(12345);
    let mut queries = Vec::with_capacity(num_queries);
    for _ in 0..num_queries {
        let mut q: Vec<f32> = (0..dim).map(|_| query_rng.gen_range(-1.0..1.0)).collect();
        let norm: f32 = q.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-10 {
            for el in q.iter_mut() { *el /= norm; }
        }
        queries.push(q);
    }

    for &(m, ef_c) in &[(16, 200), (32, 200), (48, 200)] {
        let config = HnswConfig::new(m, ef_c, 100);
        let db = VectorDb::new();
        let name = format!("col_m{}_ef{}", m, ef_c);
        let collection = db.create_collection_with_config(&name, dim, MetricType::L2, config)?;

        let start_build = Instant::now();
        for (i, v) in vectors.iter().enumerate() {
            collection.insert(i as u64, v, None)?;
        }
        println!("Built M={}, efC={} in {:.2?}", m, ef_c, start_build.elapsed());

        let mut ground_truths = Vec::new();
        for q in &queries {
            let gt = collection.search_brute_force(q, k)?;
            let ids: HashSet<u64> = gt.into_iter().map(|r| r.id).collect();
            ground_truths.push(ids);
        }

        for &ef_s in &[100, 200, 300, 400] {
            let mut hits = 0;
            for (q_idx, q) in queries.iter().enumerate() {
                let res = collection.search_hnsw(q, k, ef_s)?;
                let gt = &ground_truths[q_idx];
                hits += res.iter().filter(|r| gt.contains(&r.id)).count();
            }
            let recall = hits as f64 / (num_queries * k) as f64;
            println!("Config M={}, efC={} | efS={:<3} => Recall@10 = {:.4}", m, ef_c, ef_s, recall);
        }
    }

    Ok(())
}
