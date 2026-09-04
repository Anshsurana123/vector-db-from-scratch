use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tempfile::tempdir;
use vectordb_core::{HnswConfig, MetricType, Result, VectorDb};

fn random_vec(rng: &mut impl Rng, dim: usize) -> Vec<f32> {
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
fn test_concurrency_insert_and_search() -> Result<()> {
    let db = Arc::new(VectorDb::new());
    let dim = 16;
    let config = HnswConfig::new(8, 50, 50);
    let col = db.create_collection_with_config("concurrent_rw", dim, MetricType::L2, config)?;

    // Pre-populate with 200 vectors
    let mut rng = StdRng::seed_from_u64(100);
    for i in 0..200 {
        let v = random_vec(&mut rng, dim);
        col.insert(i, &v, None)?;
    }

    let is_running = Arc::new(AtomicBool::new(true));
    let read_counter = Arc::new(AtomicU64::new(0));
    let write_counter = Arc::new(AtomicU64::new(0));

    // Spawn 6 search reader threads
    let mut reader_handles = Vec::new();
    for t in 0..6 {
        let col_clone = col.clone();
        let running = Arc::clone(&is_running);
        let r_count = Arc::clone(&read_counter);
        let mut t_rng = StdRng::seed_from_u64(200 + t);

        reader_handles.push(thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                let q = random_vec(&mut t_rng, dim);
                let res = col_clone.search_hnsw(&q, 5, 50).expect("Search must succeed");
                // Invariants:
                assert!(!res.is_empty(), "Search should find neighbors in non-empty index");
                assert!(res.len() <= 5);
                // Distances must be sorted ascending
                for window in res.windows(2) {
                    assert!(window[0].distance <= window[1].distance + 1e-5);
                }
                r_count.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    // Spawn 4 writer threads inserting distinct IDs
    let mut writer_handles = Vec::new();
    for t in 0..4 {
        let col_clone = col.clone();
        let w_count = Arc::clone(&write_counter);

        writer_handles.push(thread::spawn(move || {
            let mut t_rng = StdRng::seed_from_u64(500 + t);
            let base_id = 1000 + t * 200;
            for i in 0..200 {
                let id = base_id + i;
                let v = random_vec(&mut t_rng, dim);
                col_clone.insert(id, &v, None).expect("Insert must succeed");
                w_count.fetch_add(1, Ordering::Relaxed);
                thread::yield_now();
            }
        }));
    }

    // Wait for all writers to complete
    for h in writer_handles {
        h.join().unwrap();
    }

    // Stop readers
    is_running.store(false, Ordering::Relaxed);
    for h in reader_handles {
        h.join().unwrap();
    }

    let total_written = write_counter.load(Ordering::SeqCst);
    let total_read = read_counter.load(Ordering::SeqCst);

    assert_eq!(total_written, 800);
    assert!(total_read > 50, "Readers must complete queries: got {}", total_read);
    assert_eq!(col.len(), 1000);

    Ok(())
}

#[test]
fn test_concurrency_search_and_delete() -> Result<()> {
    let db = Arc::new(VectorDb::new());
    let dim = 8;
    let config = HnswConfig::new(8, 50, 50);
    let col = db.create_collection_with_config("concurrent_del", dim, MetricType::L2, config)?;

    // Insert 500 vectors
    let mut rng = StdRng::seed_from_u64(42);
    for i in 0..500 {
        let v = random_vec(&mut rng, dim);
        col.insert(i, &v, None)?;
    }

    let is_running = Arc::new(AtomicBool::new(true));
    let deleted_ids = Arc::new(parking_lot::RwLock::new(std::collections::HashSet::new()));

    // Spawn 4 readers checking that deleted vectors NEVER appear after confirmed deletion
    let mut readers = Vec::new();
    for t in 0..4 {
        let col_clone = col.clone();
        let running = Arc::clone(&is_running);
        let del_set = Arc::clone(&deleted_ids);
        let mut t_rng = StdRng::seed_from_u64(1000 + t);

        readers.push(thread::spawn(move || {
            let mut searches = 0;
            while running.load(Ordering::Relaxed) {
                let q = random_vec(&mut t_rng, dim);
                let res = col_clone.search_hnsw(&q, 10, 50).expect("Search must succeed");
                let del_guard = del_set.read();
                for r in res {
                    assert!(
                        !del_guard.contains(&r.id),
                        "Search returned deleted vector ID {}",
                        r.id
                    );
                }
                searches += 1;
            }
            searches
        }));
    }

    // Spawn deleter thread deleting vectors 0..250
    let col_del = col.clone();
    let del_set_clone = Arc::clone(&deleted_ids);
    let deleter = thread::spawn(move || {
        for id in 0..250 {
            col_del.delete(id).expect("Delete must succeed");
            del_set_clone.write().insert(id);
            thread::sleep(Duration::from_millis(1));
        }
    });

    deleter.join().unwrap();
    is_running.store(false, Ordering::Relaxed);

    let mut total_searches = 0;
    for r in readers {
        total_searches += r.join().unwrap();
    }

    assert_eq!(col.len(), 250);
    assert!(total_searches > 100);

    Ok(())
}

#[test]
fn test_concurrency_multi_collection_operations() -> Result<()> {
    let db = Arc::new(VectorDb::new());
    let mut handles = Vec::new();

    // 8 threads concurrently creating, inserting, searching, and dropping their own collections
    for t in 0..8 {
        let db_clone = Arc::clone(&db);
        handles.push(thread::spawn(move || {
            let col_name = format!("worker_col_{}", t);
            let dim = 4;
            let col = db_clone
                .create_collection(&col_name, dim, MetricType::L2)
                .expect("Create must succeed");

            for i in 0..50 {
                col.insert(i, &[i as f32, (i * 2) as f32, 0.0, 1.0], None)
                    .expect("Insert must succeed");
            }
            assert_eq!(col.len(), 50);

            let res = col.search(&[10.0, 20.0, 0.0, 1.0], 5).expect("Search must succeed");
            assert_eq!(res[0].id, 10);

            // Drop collection
            let dropped = db_clone.drop_collection(&col_name).expect("Drop must succeed");
            assert!(dropped);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(db.list_collections().len(), 0);
    Ok(())
}

#[test]
fn test_concurrency_repeated_reopens_under_workload() -> Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path();
    let dim = 4;

    for cycle in 0..4 {
        let db = Arc::new(VectorDb::open(db_path)?);
        if cycle == 0 {
            db.create_collection("reopen_col", dim, MetricType::L2)?;
        }
        let col = db.get_collection("reopen_col")?;

        let start_len = col.len();
        let base_id = (cycle * 100) as u64;

        // Concurrent inserts across 4 threads through VectorDb API (concurrent WAL writes)
        let mut handles = Vec::new();
        for t in 0..4 {
            let db_clone = Arc::clone(&db);
            handles.push(thread::spawn(move || {
                for i in 0..25 {
                    let id = base_id + (t * 25 + i) as u64;
                    db_clone
                        .insert_vector("reopen_col", id, &[id as f32, id as f32, 0.0, 0.0], None)
                        .unwrap();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(col.len(), start_len + 100);

        if cycle % 2 == 1 {
            db.save_snapshot()?;
        }
    }

    // Final reopen verification
    let db_final = VectorDb::open(db_path)?;
    let col = db_final.get_collection("reopen_col")?;
    assert_eq!(col.len(), 400);

    Ok(())
}
