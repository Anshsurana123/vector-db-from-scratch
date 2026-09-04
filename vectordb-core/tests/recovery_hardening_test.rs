use std::fs::{self, OpenOptions};
use std::io::Write;
use tempfile::tempdir;
use vectordb_core::{MetricType, Result, VectorDb, VectorDbError};

#[test]
fn test_recovery_1_truncated_final_record_at_eof() -> Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path();

    // Initialize DB and write 2 collections and several vectors
    {
        let db = VectorDb::open(db_path)?;
        db.create_collection("col1", 3, MetricType::L2)?;
        db.insert_vector("col1", 1, &[1.0, 2.0, 3.0], None)?;
        db.insert_vector("col1", 2, &[4.0, 5.0, 6.0], None)?;
    }

    // Now corrupt the end of the collection's WAL by appending a truncated partial record (e.g. crash mid-write)
    let wal_path = db_path.join("wal_col1.wal");
    assert!(wal_path.exists());
    let mut f = OpenOptions::new().append(true).open(&wal_path)?;
    // Write partial magic and incomplete header (simulate sudden power cut)
    f.write_all(b"VWA")?;
    f.sync_all()?;
    drop(f);

    // Reopen DB: must safely truncate partial EOF bytes and recover all prior valid records
    let db_recovered = VectorDb::open(db_path)?;
    let col = db_recovered.get_collection("col1")?;
    assert_eq!(col.len(), 2);
    assert_eq!(col.get_vector(1).unwrap(), vec![1.0, 2.0, 3.0]);
    assert_eq!(col.get_vector(2).unwrap(), vec![4.0, 5.0, 6.0]);

    Ok(())
}

#[test]
fn test_recovery_2_malformed_wal_record_fails_explicitly() -> Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path();

    {
        let db = VectorDb::open(db_path)?;
        db.create_collection("col_bad", 3, MetricType::L2)?;
        db.insert_vector("col_bad", 1, &[1.0, 2.0, 3.0], None)?;
    }

    // Append a fully framed record with intentional bitflip / bad CRC
    let wal_path = db_path.join("wal_col_bad.wal");
    let mut f = OpenOptions::new().append(true).open(&wal_path)?;
    // MAGIC(4) + TYPE(1) + SEQ(8) + LEN(4) + PAYLOAD(4) + BAD_CRC(4) = complete byte count, but mismatched CRC
    f.write_all(b"VWAL\x02\x05\x00\x00\x00\x00\x00\x00\x00\x04\x00\x00\x00data\xff\xff\xff\xff")?;
    f.sync_all()?;
    drop(f);

    // Reopen DB: must fail explicitly with CRC mismatch rather than silently pretending nothing happened
    let res = VectorDb::open(db_path);
    assert!(res.is_err(), "Recovery should fail on corrupted WAL with bad CRC");
    match res.err().unwrap() {
        VectorDbError::WalCrcMismatch { .. } => {}
        other => panic!("Expected WalCrcMismatch error, got: {:?}", other),
    }

    Ok(())
}

#[test]
fn test_recovery_3_snapshot_exists_with_post_snapshot_wal_writes() -> Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path();

    {
        let db = VectorDb::open(db_path)?;
        db.create_collection("snap_col", 2, MetricType::L2)?;

        // Pre-snapshot writes: IDs 1..5
        for i in 1..=5 {
            db.insert_vector("snap_col", i, &[i as f32, i as f32], None)?;
        }

        // Take atomic snapshot
        db.save_snapshot()?;

        // Post-snapshot writes: IDs 6..10
        for i in 6..=10 {
            db.insert_vector("snap_col", i, &[i as f32, i as f32], None)?;
        }
        // Delete ID 2 post-snapshot
        db.delete_vector("snap_col", 2)?;
    }

    // Recover from disk
    let db_recovered = VectorDb::open(db_path)?;
    let col = db_recovered.get_collection("snap_col")?;

    // Total vectors should be 9 (1, 3..10, since 2 was deleted)
    assert_eq!(col.len(), 9);
    assert!(col.get_vector(2).is_none(), "Vector 2 must remain deleted post-recovery");
    for i in [1, 3, 4, 5, 6, 7, 8, 9, 10] {
        assert!(col.get_vector(i).is_some(), "Vector {} must exist", i);
    }

    // Verify search works correctly on recovered state
    let results = col.search_hnsw(&[10.0, 10.0], 3, 50)?;
    assert_eq!(results[0].id, 10);

    Ok(())
}

#[test]
fn test_recovery_4_snapshot_replacement_interrupted_leaves_tmp_file() -> Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path();

    // Step 1: establish a valid snapshot
    {
        let db = VectorDb::open(db_path)?;
        db.create_collection("col", 2, MetricType::L2)?;
        db.insert_vector("col", 100, &[1.0, 1.0], None)?;
        db.save_snapshot()?;
    }

    // Step 2: simulate an interrupted snapshot creation leaving an incomplete snapshot.snap.tmp
    let tmp_snap = db_path.join("snapshot.snap.tmp");
    fs::write(&tmp_snap, b"incomplete corrupted snapshot data")?;

    // Step 3: Reopen database. It must clean up the orphan .tmp and load the valid snapshot.snap
    let db_recovered = VectorDb::open(db_path)?;
    let col = db_recovered.get_collection("col")?;
    assert_eq!(col.len(), 1);
    assert!(col.get_vector(100).is_some());

    // The orphan .tmp file should be removed
    assert!(!tmp_snap.exists(), "Interrupted .tmp snapshot must be cleaned up on recovery");

    Ok(())
}

#[test]
fn test_recovery_5_empty_database_recovery() -> Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path();

    // Open an empty directory
    let db = VectorDb::open(db_path)?;
    assert_eq!(db.list_collections().len(), 0);

    // Create and delete a collection, then restart
    db.create_collection("transient", 2, MetricType::L2)?;
    db.drop_collection("transient")?;
    drop(db);

    let db_reopened = VectorDb::open(db_path)?;
    assert_eq!(db_reopened.list_collections().len(), 0);

    Ok(())
}

#[test]
fn test_recovery_6_multiple_collections_recovering_independently() -> Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path();

    {
        let db = VectorDb::open(db_path)?;
        db.create_collection("alpha", 2, MetricType::L2)?;
        db.create_collection("beta", 4, MetricType::Cosine)?;
        db.create_collection("gamma", 3, MetricType::DotProduct)?;

        db.insert_vector("alpha", 1, &[1.0, 2.0], None)?;
        db.insert_vector("beta", 10, &[1.0, 0.0, 0.0, 0.0], None)?;
        db.insert_vector("gamma", 100, &[0.5, 0.5, 0.5], None)?;
    }

    let db_reopened = VectorDb::open(db_path)?;
    let mut cols = db_reopened.list_collections();
    cols.sort();
    assert_eq!(cols, vec!["alpha", "beta", "gamma"]);

    let alpha = db_reopened.get_collection("alpha")?;
    let beta = db_reopened.get_collection("beta")?;
    let gamma = db_reopened.get_collection("gamma")?;

    assert_eq!(alpha.dim(), 2);
    assert_eq!(beta.dim(), 4);
    assert_eq!(gamma.dim(), 3);

    assert_eq!(alpha.len(), 1);
    assert_eq!(beta.len(), 1);
    assert_eq!(gamma.len(), 1);

    Ok(())
}

#[test]
fn test_recovery_7_delete_tombstone_state_survives_restart() -> Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path();

    {
        let db = VectorDb::open(db_path)?;
        db.create_collection("tombstones", 2, MetricType::L2)?;
        db.insert_vector("tombstones", 1, &[1.0, 1.0], None)?;
        db.insert_vector("tombstones", 2, &[2.0, 2.0], None)?;
        db.insert_vector("tombstones", 3, &[3.0, 3.0], None)?;

        // Delete 2
        db.delete_vector("tombstones", 2)?;
        // Do NOT snapshot: rely purely on WAL replay
    }

    let db_reopened = VectorDb::open(db_path)?;
    let col = db_reopened.get_collection("tombstones")?;

    assert_eq!(col.len(), 2);
    assert!(col.get_vector(1).is_some());
    assert!(col.get_vector(2).is_none(), "Tombstoned ID 2 must remain deleted after WAL replay");
    assert!(col.get_vector(3).is_some());

    // Verify search does not return tombstone
    let results = col.search_hnsw(&[2.0, 2.0], 3, 50)?;
    for r in results {
        assert_ne!(r.id, 2, "Search must never return deleted vector 2");
    }

    Ok(())
}

#[test]
fn test_recovery_8_missing_persisted_artifact_scenarios() -> Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path();

    // Scenario A: Missing snapshot file entirely (WAL-only recovery without snapshot)
    {
        let db = VectorDb::open(db_path)?;
        db.create_collection("col_wal_only", 2, MetricType::L2)?;
        db.insert_vector("col_wal_only", 1, &[10.0, 20.0], None)?;
        db.insert_vector("col_wal_only", 2, &[30.0, 40.0], None)?;
    }
    // Verify snapshot.snap doesn't exist
    assert!(!db_path.join("snapshot.snap").exists());
    let db_reopened = VectorDb::open(db_path)?;
    let col = db_reopened.get_collection("col_wal_only")?;
    assert_eq!(col.len(), 2);
    assert_eq!(col.get_vector(1).unwrap(), vec![10.0, 20.0]);

    // Scenario B: Missing WAL file for one collection while another collection's WAL is intact
    db_reopened.create_collection("col_lost_wal", 2, MetricType::L2)?;
    db_reopened.insert_vector("col_lost_wal", 99, &[9.0, 9.0], None)?;
    drop(db_reopened);

    // Delete the second collection's WAL file
    let lost_wal = db_path.join("wal_col_lost_wal.wal");
    if lost_wal.exists() {
        fs::remove_file(&lost_wal)?;
    }

    // Reopen: col_wal_only is fully intact, col_lost_wal recovers empty since its WAL was lost
    let db_recovered = VectorDb::open(db_path)?;
    let col_intact = db_recovered.get_collection("col_wal_only")?;
    assert_eq!(col_intact.len(), 2);
    let col_lost = db_recovered.get_collection("col_lost_wal")?;
    assert_eq!(col_lost.len(), 0);

    // Scenario C: Snapshot exists, all WAL files missing -> recovers clean snapshot baseline
    db_recovered.insert_vector("col_wal_only", 3, &[50.0, 60.0], None)?;
    db_recovered.save_snapshot()?;
    drop(db_recovered);

    // Remove all .wal files from directory
    for entry in fs::read_dir(db_path)? {
        let entry = entry?;
        if entry.path().extension().map(|e| e == "wal").unwrap_or(false) {
            fs::remove_file(entry.path())?;
        }
    }

    let db_from_snap = VectorDb::open(db_path)?;
    let col_snap = db_from_snap.get_collection("col_wal_only")?;
    assert_eq!(col_snap.len(), 3);
    assert_eq!(col_snap.get_vector(3).unwrap(), vec![50.0, 60.0]);

    Ok(())
}

#[test]
fn test_recovery_9_consecutive_restarts_and_cycles() -> Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path();

    for cycle in 0..5 {
        let db = VectorDb::open(db_path)?;
        if cycle == 0 {
            db.create_collection("cycle_col", 2, MetricType::L2)?;
        }
        let _col = db.get_collection("cycle_col")?;
        let base_id = (cycle * 10) as u64;
        for i in 0..10 {
            let id = base_id + i;
            db.insert_vector("cycle_col", id, &[id as f32, id as f32], None)?;
        }

        // Snapshot on even cycles
        if cycle % 2 == 0 {
            db.save_snapshot()?;
        }
    }

    // Final verification after 5 consecutive restarts
    let db_final = VectorDb::open(db_path)?;
    let col = db_final.get_collection("cycle_col")?;
    assert_eq!(col.len(), 50);

    for id in 0..50 {
        assert!(col.get_vector(id).is_some(), "Vector {} missing after 5 restart cycles", id);
    }

    Ok(())
}
