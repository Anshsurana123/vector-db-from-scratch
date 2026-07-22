use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::time::Instant;
use tempfile::tempdir;

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
fn test_milestone3_wal_and_snapshot_recovery() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MILESTONE 3 GATE VERIFICATION TEST ===");

    let dir = tempdir()?;
    let db_path = dir.path().join("vectordb_data");

    // -------------------------------------------------------------------------
    // TEST 1: WAL-only Crash Recovery (10,000 vectors)
    // -------------------------------------------------------------------------
    println!("\n[1/2] Testing WAL-only crash recovery on 10,000 vectors...");
    let dim = 128;
    let mut rng = StdRng::seed_from_u64(42);

    {
        let db = VectorDb::open(&db_path)?;
        let collection = db.create_collection_with_config(
            "wal_col",
            dim,
            MetricType::L2,
            HnswConfig::new(16, 100, 100),
        )?;

        for i in 0..10_000 {
            let vec = generate_normalized_vector(&mut rng, dim);
            db.insert_vector("wal_col", i, &vec, None)?;
        }

        // Delete IDs 500..510
        for id in 500..510 {
            db.delete_vector("wal_col", id)?;
        }

        assert_eq!(collection.len(), 9990);
        // Abruptly drop db without saving snapshot
    }

    println!("Simulating crash... Reopening database from WAL file...");
    let start_wal_recovery = Instant::now();
    let recovered_db = VectorDb::open(&db_path)?;
    let wal_recovery_dur = start_wal_recovery.elapsed();

    let recovered_col = recovered_db.get_collection("wal_col")?;
    println!("WAL Recovery finished in {:.2?}", wal_recovery_dur);
    println!("Recovered vector count: {}", recovered_col.len());
    assert_eq!(recovered_col.len(), 9990);

    // Verify deleted IDs are absent
    for id in 500..510 {
        assert!(recovered_col.get_vector(id).is_none());
    }

    // -------------------------------------------------------------------------
    // TEST 2: Snapshot + WAL Crash Recovery (100,000 vectors)
    // -------------------------------------------------------------------------
    println!("\n[2/2] Testing Snapshot + WAL crash recovery on 100,000 vectors...");
    let snap_dir = dir.path().join("vectordb_snapshot_data");

    let query_vector = generate_normalized_vector(&mut StdRng::seed_from_u64(999), dim);

    {
        let db = VectorDb::open(&snap_dir)?;
        let collection = db.create_collection_with_config(
            "snap_col",
            dim,
            MetricType::L2,
            HnswConfig::new(16, 100, 100),
        )?;

        println!("Building initial 99,500 vectors for Snapshot...");
        for i in 0..99_500 {
            let vec = generate_normalized_vector(&mut rng, dim);
            db.insert_vector("snap_col", i, &vec, None)?;
        }

        println!("Saving atomic Bincode snapshot...");
        let start_snap = Instant::now();
        db.save_snapshot()?;
        println!("Snapshot saved atomically in {:.2?}", start_snap.elapsed());

        println!("Appending 500 more vectors to WAL...");
        for i in 99_500..100_000 {
            let vec = generate_normalized_vector(&mut rng, dim);
            db.insert_vector("snap_col", i, &vec, None)?;
        }

        assert_eq!(collection.len(), 100_000);
        // Abruptly drop db handle
    }

    println!("\nSimulating un-flushed crash... Recovering database from Snapshot + WAL...");
    let start_recovery = Instant::now();
    let recovered_snap_db = VectorDb::open(&snap_dir)?;
    let recovery_dur = start_recovery.elapsed();

    let recovered_snap_col = recovered_snap_db.get_collection("snap_col")?;
    println!("Database Recovery Complete in {:.4} seconds!", recovery_dur.as_secs_f64());
    println!("Recovered vector count: {} / 100,000", recovered_snap_col.len());

    assert_eq!(recovered_snap_col.len(), 100_000);

    // Verify recovery time < 2.0 seconds
    println!("Verifying recovery time criterion (< 2.0 seconds):");
    println!("  Recovery Time: {:.4}s (Target < 2.00s)", recovery_dur.as_secs_f64());
    assert!(
        recovery_dur.as_secs_f64() < 2.0,
        "GATE FAILURE: Recovery took {:.4}s, exceeding 2.0s limit",
        recovery_dur.as_secs_f64()
    );

    // Search query test
    let results = recovered_snap_col.search_hnsw(&query_vector, 10, 100)?;
    assert_eq!(results.len(), 10);
    println!("Top result ID: {}, Distance: {:.4}", results[0].id, results[0].distance);

    println!("\nSUCCESS: Milestone 3 Gate Passed cleanly! 100k vector state recovered in {:.4}s.", recovery_dur.as_secs_f64());

    Ok(())
}
