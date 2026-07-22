use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use vectordb_core::{MetricType, SearchResult, VectorDb};
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
async fn test_milestone6_gate() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MILESTONE 6 GATE VERIFICATION TEST ===");

    // 1. Launch Axum HTTP Server on a random local port
    let db = Arc::new(VectorDb::new());
    let router = app(db);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr: SocketAddr = listener.local_addr()?;
    println!("Server bound to local socket: http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let client = reqwest::Client::new();
    let base_url = format!("http://{}", addr);

    // 2. Create Collection via HTTP POST /collections
    println!("\n[1/4] Testing POST /collections...");
    let create_payload = serde_json::json!({
        "name": "http_test_col",
        "dim": 128,
        "metric": "L2"
    });

    let res = client
        .post(format!("{}/collections", base_url))
        .json(&create_payload)
        .send()
        .await?;

    println!("  HTTP Status: {}", res.status());
    assert_eq!(res.status(), reqwest::StatusCode::CREATED);

    // 3. Insert 1,000 Vectors via HTTP POST /collections/http_test_col/insert
    println!("\n[2/4] Testing POST /collections/http_test_col/insert on 1,000 vectors...");
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
            .post(format!("{}/collections/http_test_col/insert", base_url))
            .json(&insert_payload)
            .send()
            .await?;

        assert_eq!(res.status(), reqwest::StatusCode::OK);
    }
    println!("  1,000 vectors inserted successfully!");

    // 4. Vector Search via HTTP POST /collections/http_test_col/search
    println!("\n[3/4] Testing POST /collections/http_test_col/search...");
    let query_vec = generate_normalized_vector(&mut StdRng::seed_from_u64(999), dim);
    let search_payload = serde_json::json!({
        "query": query_vec,
        "k": 10,
        "ef_search": 100
    });

    let res = client
        .post(format!("{}/collections/http_test_col/search", base_url))
        .json(&search_payload)
        .send()
        .await?;

    println!("  HTTP Status: {}", res.status());
    assert_eq!(res.status(), reqwest::StatusCode::OK);

    let results: Vec<SearchResult> = res.json().await?;
    assert_eq!(results.len(), 10);
    println!("  Top Result ID: {}, Distance: {:.4}", results[0].id, results[0].distance);

    // 5. Delete Vector via HTTP DELETE /collections/http_test_col/vectors/42
    println!("\n[4/4] Testing DELETE /collections/http_test_col/vectors/42...");
    let res = client
        .delete(format!("{}/collections/http_test_col/vectors/42", base_url))
        .send()
        .await?;

    println!("  HTTP Status: {}", res.status());
    assert_eq!(res.status(), reqwest::StatusCode::OK);

    // Verify vector 42 is absent from search results
    let res = client
        .post(format!("{}/collections/http_test_col/search", base_url))
        .json(&search_payload)
        .send()
        .await?;

    let search_after_delete: Vec<SearchResult> = res.json().await?;
    assert!(search_after_delete.iter().all(|r| r.id != 42));

    println!("\nSUCCESS: Milestone 6 Gate Passed cleanly! Axum REST API verified across all HTTP endpoints.");

    Ok(())
}
