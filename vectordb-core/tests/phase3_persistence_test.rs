use tempfile::tempdir;
use vectordb_core::{MetricType, VectorDb};

#[test]
fn test_phase3_per_collection_wal_and_auto_snapshot() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let db = VectorDb::open(dir.path())?;
    
    // Set auto-snapshot threshold to 5 operations
    db.set_auto_snapshot_threshold(5);

    // Create 2 separate collections
    let col1 = db.create_collection("col_alpha", 4, MetricType::L2)?;
    let col2 = db.create_collection("col_beta", 4, MetricType::Cosine)?;

    // Insert 3 vectors into col1, 3 into col2 (total 6 ops -> auto snapshot triggers)
    db.insert_vector("col_alpha", 1, &[1.0, 0.0, 0.0, 0.0], None)?;
    db.insert_vector("col_alpha", 2, &[0.0, 1.0, 0.0, 0.0], None)?;
    db.insert_vector("col_beta", 10, &[0.0, 0.0, 1.0, 0.0], None)?;
    db.insert_vector("col_beta", 20, &[0.0, 0.0, 0.0, 1.0], None)?;
    db.insert_vector("col_alpha", 3, &[0.5, 0.5, 0.0, 0.0], None)?;
    db.insert_vector("col_beta", 30, &[0.1, 0.1, 0.1, 0.1], None)?;

    // Verify vectors present
    assert_eq!(col1.len(), 3);
    assert_eq!(col2.len(), 3);

    // Close and reopen DB from same directory (triggers recovery)
    drop(col1);
    drop(col2);
    drop(db);

    let db_reopened = VectorDb::open(dir.path())?;
    let col1_reopened = db_reopened.get_collection("col_alpha")?;
    let col2_reopened = db_reopened.get_collection("col_beta")?;

    assert_eq!(col1_reopened.len(), 3);
    assert_eq!(col2_reopened.len(), 3);
    assert!(col1_reopened.get_vector(1).is_some());
    assert!(col2_reopened.get_vector(20).is_some());

    Ok(())
}
