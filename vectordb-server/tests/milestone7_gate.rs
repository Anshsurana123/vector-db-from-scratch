use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use vectordb_core::{SearchResult, VectorDb};
use vectordb_server::app;

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

#[tokio::test]
async fn test_milestone7_gate_pq_endpoints() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MILESTONE 7 GATE VERIFICATION TEST ===");

    // Use a temp dir for VectorDb to support snapshotting
    let temp_dir = tempfile::tempdir()?;
    let db = Arc::new(VectorDb::open(temp_dir.path())?);
    let router = app(db);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr: SocketAddr = listener.local_addr()?;
    println!("Server bound to local socket: http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let client = reqwest::Client::new();
    let base_url = format!("http://{}", addr);

    // 1. Create Collection via HTTP POST /collections
    println!("\n[1/6] Testing POST /collections...");
    let create_payload = serde_json::json!({
        "name": "pq_test_col",
        "dim": 128,
        "metric": "L2"
    });

    let res = client
        .post(format!("{}/collections", base_url))
        .json(&create_payload)
        .send()
        .await?;

    assert_eq!(res.status(), reqwest::StatusCode::CREATED);

    // 2. Insert 1,000 Vectors via HTTP POST /collections/pq_test_col/insert
    println!("\n[2/6] Testing POST /collections/pq_test_col/insert on 1,000 vectors...");
    let dim = 128;
    let mut rng = StdRng::seed_from_u64(42);

    for i in 0..1000 {
        let vec = generate_normalized_vector(&mut rng, dim);
        let insert_payload = serde_json::json!({
            "id": i,
            "vector": vec,
            "metadata": { "index": i }
        });

        let res = client
            .post(format!("{}/collections/pq_test_col/insert", base_url))
            .json(&insert_payload)
            .send()
            .await?;

        assert_eq!(res.status(), reqwest::StatusCode::OK);
    }

    // 3. Compact via HTTP POST /collections/pq_test_col/compact
    println!("\n[3/6] Testing POST /collections/pq_test_col/compact...");
    let res = client
        .post(format!("{}/collections/pq_test_col/compact", base_url))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::OK);

    // 4. Train PQ via HTTP POST /collections/pq_test_col/train_pq
    println!("\n[4/6] Testing POST /collections/pq_test_col/train_pq...");
    let train_pq_payload = serde_json::json!({
        "num_subvectors": 32
    });
    let res = client
        .post(format!("{}/collections/pq_test_col/train_pq", base_url))
        .json(&train_pq_payload)
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::OK);

    // 5. Search with PQ via HTTP POST /collections/pq_test_col/search
    println!("\n[5/6] Testing POST /collections/pq_test_col/search with PQ...");
    let query_vec = generate_normalized_vector(&mut StdRng::seed_from_u64(999), dim);
    let search_payload = serde_json::json!({
        "query": query_vec,
        "k": 10,
        "use_pq": true
    });

    let res = client
        .post(format!("{}/collections/pq_test_col/search", base_url))
        .json(&search_payload)
        .send()
        .await?;

    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let results: Vec<SearchResult> = res.json().await?;
    assert_eq!(results.len(), 10);
    println!("  PQ Search Top Result ID: {}, Distance: {:.4}", results[0].id, results[0].distance);

    // 6. Snapshot via HTTP POST /snapshot
    println!("\n[6/6] Testing POST /snapshot...");
    let res = client
        .post(format!("{}/snapshot", base_url))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::OK);

    println!("\nSUCCESS: Milestone 7 Gate Passed cleanly! PQ endpoints verified.");
    Ok(())
}
