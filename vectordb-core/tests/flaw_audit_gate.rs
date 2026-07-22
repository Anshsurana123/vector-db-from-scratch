use std::fs;
use tempfile::tempdir;
use vectordb_core::{
    FilterExpression, HnswConfig, MetricType, VectorDb, Result,
};

#[test]
fn test_flaw_1_and_4_http_wal_and_snapshot_truncation() -> Result<()> {
    let dir = tempdir()?;
    let db_dir = dir.path();

    let db = VectorDb::open(db_dir)?;
    db.create_collection("test_col", 4, MetricType::L2)?;

    // Insert 10 vectors via VectorDb API (simulating HTTP endpoint calls)
    for i in 0..10 {
        db.insert_vector("test_col", i, &[i as f32, 0.0, 0.0, 0.0], None)?;
    }

    let wal_path = db_dir.join("wal.wal");
    assert!(wal_path.exists());
    let wal_len_before = fs::metadata(&wal_path)?.len();
    assert!(wal_len_before > 0, "WAL file should contain appended frames");

    // Save snapshot -> WAL should be truncated to 0 bytes
    db.save_snapshot()?;
    let wal_len_after = fs::metadata(&wal_path)?.len();
    assert_eq!(wal_len_after, 0, "WAL file should be truncated to 0 bytes after snapshot");

    // Add 5 more vectors after snapshot
    for i in 10..15 {
        db.insert_vector("test_col", i, &[i as f32, 0.0, 0.0, 0.0], None)?;
    }
    drop(db);

    // Re-open DB and verify all 15 vectors are recovered cleanly from snapshot + WAL replay
    let db_reopened = VectorDb::open(db_dir)?;
    let col = db_reopened.get_collection("test_col")?;
    assert_eq!(col.len(), 15);
    for i in 0..15 {
        assert!(col.get_vector(i).is_some());
    }

    Ok(())
}

#[test]
fn test_flaw_2_reinsertion_and_compaction() -> Result<()> {
    let dir = tempdir()?;
    let db_dir = dir.path();

    let db = VectorDb::open(db_dir)?;
    let col = db.create_collection_with_config("col", 2, MetricType::L2, HnswConfig::new(4, 16, 16))?;

    // Insert IDs 1, 2, 3
    col.insert(1, &[1.0, 1.0], None)?;
    col.insert(2, &[2.0, 2.0], None)?;
    col.insert(3, &[3.0, 3.0], None)?;

    // Delete ID 2
    col.delete(2)?;
    assert_eq!(col.len(), 2);

    // Re-insert ID 2 with new vector
    col.insert(2, &[10.0, 10.0], None)?;
    assert_eq!(col.len(), 3);

    // Execute compaction
    col.compact();
    assert_eq!(col.len(), 3);

    // Search and verify correct results
    let results = col.search_hnsw(&[9.9, 9.9], 1, 16)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 2);

    Ok(())
}

#[test]
fn test_flaw_7_query_planner_routing() -> Result<()> {
    let db = VectorDb::new();
    let col = db.create_collection("products", 2, MetricType::L2)?;

    // 1. Empty collection -> BruteForceScan strategy
    let empty_filter = FilterExpression::Eq("category".into(), serde_json::json!("electronics"));
    let empty_results = col.search_with_filter(&[1.0, 1.0], 5, &empty_filter)?;
    assert_eq!(empty_results.len(), 0);

    for i in 0..100 {
        let category = if i < 5 { "electronics" } else { "clothing" };
        col.insert(
            i as u64,
            &[i as f32, i as f32],
            Some(serde_json::json!({ "category": category, "price": i * 10 })),
        )?;
    }

    // 2. High selectivity filter: category == "electronics" (5% match) -> FilteredScan strategy
    let selective = FilterExpression::Eq("category".into(), serde_json::json!("electronics"));
    let storage_guard = col.get_vector(0);
    assert!(storage_guard.is_some());

    let results = col.search_with_filter(&[1.0, 1.0], 10, &selective)?;
    assert_eq!(results.len(), 5);
    for res in results {
        let meta = res.metadata.unwrap();
        assert_eq!(meta.get("category").unwrap(), "electronics");
    }

    // 3. Broad selectivity filter: category == "clothing" (95% match) -> HnswFiltered strategy
    let broad = FilterExpression::Eq("category".into(), serde_json::json!("clothing"));
    let broad_results = col.search_with_filter(&[1.0, 1.0], 10, &broad)?;
    assert_eq!(broad_results.len(), 10);
    for res in broad_results {
        let meta = res.metadata.unwrap();
        assert_eq!(meta.get("category").unwrap(), "clothing");
    }

    Ok(())
}

