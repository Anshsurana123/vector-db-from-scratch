use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use serde::Deserialize;

use vectordb_core::{MetricType, VectorDb};

#[derive(Deserialize)]
struct GroundTruthItem {
    id: u64,
    distance: f32,
}

#[derive(Deserialize)]
struct GroundTruthSpotCheck {
    l2: Vec<GroundTruthItem>,
    cosine: Vec<GroundTruthItem>,
    dot: Vec<GroundTruthItem>,
}

#[derive(Deserialize)]
struct DatasetData {
    vectors: Vec<Vec<f32>>,
    queries: Vec<Vec<f32>>,
}

fn get_bench_file_path(filename: &str) -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let manifest_path = PathBuf::from(&manifest_dir);

    if let Some(workspace_root) = manifest_path.parent() {
        let p1 = workspace_root.join(filename);
        if p1.exists() {
            return p1;
        }
        let p2 = workspace_root.join("vectordb-bench").join(filename);
        if p2.exists() {
            return p2;
        }
    }

    let p3 = PathBuf::from(filename);
    if p3.exists() {
        return p3;
    }

    manifest_path.join(filename)
}

#[test]
fn test_milestone1_gate() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MILESTONE 1 GATE VERIFICATION TEST ===");

    let dataset_path = get_bench_file_path("milestone1_dataset.json");
    let gt_path = get_bench_file_path("milestone1_ground_truth.json");

    println!("Loading dataset from: {:?}", dataset_path);
    println!("Loading ground truth from: {:?}", gt_path);

    let file = File::open(&dataset_path)
        .map_err(|e| format!("Failed to open dataset at {:?}: {}", dataset_path, e))?;
    let reader = BufReader::new(file);
    let dataset: DatasetData = serde_json::from_reader(reader)?;

    let num_vectors = dataset.vectors.len();
    let num_queries = dataset.queries.len();
    assert_eq!(num_vectors, 10000);
    assert_eq!(num_queries, 100);

    let gt_file = File::open(&gt_path)
        .map_err(|e| format!("Failed to open ground truth at {:?}: {}", gt_path, e))?;
    let gt_reader = BufReader::new(gt_file);
    let gt_map: std::collections::HashMap<String, GroundTruthSpotCheck> = serde_json::from_reader(gt_reader)?;

    // Create DB & Collections for L2, Cosine, DotProduct
    let db = VectorDb::new();
    let col_l2 = db.create_collection("test_l2", 128, MetricType::L2)?;
    let col_cosine = db.create_collection("test_cosine", 128, MetricType::Cosine)?;
    let col_dot = db.create_collection("test_dot", 128, MetricType::DotProduct)?;

    println!("Inserting 10,000 vectors (128-dim) into collections...");
    for (i, vec) in dataset.vectors.iter().enumerate() {
        let id = i as u64;
        let meta = serde_json::json!({"vec_idx": i});
        col_l2.insert(id, vec, Some(meta.clone()))?;
        col_cosine.insert(id, vec, Some(meta.clone()))?;
        col_dot.insert(id, vec, Some(meta.clone()))?;
    }

    assert_eq!(col_l2.len(), 10000);

    println!("Running 100 queries and spot-checking 10 queries against Python numpy ground truth...");
    let spot_checks = vec![0, 10, 20, 30, 40, 50, 60, 70, 80, 90];

    for &q_idx in &spot_checks {
        let query = &dataset.queries[q_idx];
        let gt = gt_map.get(&q_idx.to_string()).ok_or("Spot check key not found in ground truth")?;

        // 1. L2 Search (exact brute-force)
        let res_l2 = col_l2.search_brute_force(query, 10)?;
        assert_eq!(res_l2.len(), 10);
        for k in 0..10 {
            let expected_id = gt.l2[k].id;
            let expected_dist = gt.l2[k].distance;
            let actual_id = res_l2[k].id;
            let actual_dist = res_l2[k].distance;

            assert_eq!(actual_id, expected_id, "Query {q_idx} L2 top-{k} ID mismatch");
            assert!(
                (actual_dist - expected_dist).abs() < 1e-3,
                "Query {q_idx} L2 top-{k} distance mismatch: expected {expected_dist}, got {actual_dist}"
            );
        }

        // 2. Cosine Search (exact brute-force)
        let res_cosine = col_cosine.search_brute_force(query, 10)?;
        assert_eq!(res_cosine.len(), 10);
        for k in 0..10 {
            let expected_id = gt.cosine[k].id;
            let expected_dist = gt.cosine[k].distance;
            let actual_id = res_cosine[k].id;
            let actual_dist = res_cosine[k].distance;

            assert_eq!(actual_id, expected_id, "Query {q_idx} Cosine top-{k} ID mismatch");
            assert!(
                (actual_dist - expected_dist).abs() < 1e-3,
                "Query {q_idx} Cosine top-{k} distance mismatch: expected {expected_dist}, got {actual_dist}"
            );
        }

        // 3. Dot Product Search (exact brute-force)
        let res_dot = col_dot.search_brute_force(query, 10)?;
        assert_eq!(res_dot.len(), 10);
        for k in 0..10 {
            let expected_id = gt.dot[k].id;
            let expected_dist = gt.dot[k].distance;
            let actual_id = res_dot[k].id;
            let actual_dist = res_dot[k].distance;

            assert_eq!(actual_id, expected_id, "Query {q_idx} Dot top-{k} ID mismatch");
            assert!(
                (actual_dist - expected_dist).abs() < 1e-3,
                "Query {q_idx} Dot top-{k} distance mismatch: expected {expected_dist}, got {actual_dist}"
            );
        }
    }

    println!("SUCCESS: All 10 spot-checked queries matched Python numpy ground truth 100% exactly across all metrics!");

    println!("Executing API round-trip test: insert -> search -> delete -> search...");
    let test_col = db.create_collection("roundtrip", 128, MetricType::L2)?;
    let target_id = 99999u64;
    let target_vec = dataset.queries[0].clone();

    test_col.insert(target_id, &target_vec, Some(serde_json::json!({"test": "roundtrip"})))?;
    let search_res1 = test_col.search_brute_force(&target_vec, 1)?;
    assert_eq!(search_res1.len(), 1);
    assert_eq!(search_res1[0].id, target_id);
    assert_eq!(search_res1[0].distance, 0.0);

    // Delete
    let deleted = test_col.delete(target_id)?;
    assert!(deleted);
    assert_eq!(test_col.len(), 0);

    // Search again
    let search_res2 = test_col.search_brute_force(&target_vec, 1)?;
    assert_eq!(search_res2.len(), 0);

    println!("SUCCESS: API round-trip insert -> search -> delete -> search confirmed deleted vector is no longer returned.");

    Ok(())
}
