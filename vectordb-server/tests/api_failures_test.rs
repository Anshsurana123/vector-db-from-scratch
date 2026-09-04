use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::net::TcpListener;
use vectordb_core::{SearchResult, VectorDb};
use vectordb_server::app;

async fn spawn_server(db: Arc<VectorDb>) -> (String, tokio::task::JoinHandle<()>) {
    let router = app(db);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (format!("http://{}", addr), handle)
}

#[tokio::test]
async fn test_api_input_validation_and_error_codes() -> Result<(), Box<dyn std::error::Error>> {
    let db = Arc::new(VectorDb::new());
    let (base_url, _handle) = spawn_server(db).await;
    let client = reqwest::Client::new();

    // 1. Create collection with empty name -> 400 INVALID_PARAMETER
    let res = client
        .post(format!("{}/collections", base_url))
        .json(&serde_json::json!({
            "name": "   ",
            "dim": 4,
            "metric": "L2"
        }))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = res.json().await?;
    assert_eq!(body["code"], "INVALID_PARAMETER");

    // 2. Create collection with dim = 0 -> 400 INVALID_PARAMETER
    let res = client
        .post(format!("{}/collections", base_url))
        .json(&serde_json::json!({
            "name": "zero_dim_col",
            "dim": 0,
            "metric": "L2"
        }))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = res.json().await?;
    assert_eq!(body["code"], "INVALID_PARAMETER");

    // 3. Valid collection creation -> 201 CREATED
    let res = client
        .post(format!("{}/collections", base_url))
        .json(&serde_json::json!({
            "name": "test_col",
            "dim": 4,
            "metric": "L2"
        }))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::CREATED);

    // 4. Duplicate collection -> 409 COLLECTION_ALREADY_EXISTS
    let res = client
        .post(format!("{}/collections", base_url))
        .json(&serde_json::json!({
            "name": "test_col",
            "dim": 4,
            "metric": "L2"
        }))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::CONFLICT);
    let body: serde_json::Value = res.json().await?;
    assert_eq!(body["code"], "COLLECTION_ALREADY_EXISTS");

    // 5. Insert empty vector -> 400 INVALID_PARAMETER
    let res = client
        .post(format!("{}/collections/test_col/insert", base_url))
        .json(&serde_json::json!({
            "id": 1,
            "vector": []
        }))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = res.json().await?;
    assert_eq!(body["code"], "INVALID_PARAMETER");

    // 6. Insert dimension mismatch -> 400 DIMENSION_MISMATCH
    let res = client
        .post(format!("{}/collections/test_col/insert", base_url))
        .json(&serde_json::json!({
            "id": 1,
            "vector": [1.0, 2.0]
        }))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = res.json().await?;
    assert_eq!(body["code"], "DIMENSION_MISMATCH");

    // 7. Insert valid vector -> 200 OK
    let res = client
        .post(format!("{}/collections/test_col/insert", base_url))
        .json(&serde_json::json!({
            "id": 1,
            "vector": [1.0, 2.0, 3.0, 4.0]
        }))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::OK);

    // 8. Duplicate ID insertion -> 409 DUPLICATE_ID
    let res = client
        .post(format!("{}/collections/test_col/insert", base_url))
        .json(&serde_json::json!({
            "id": 1,
            "vector": [1.0, 2.0, 3.0, 4.0]
        }))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::CONFLICT);
    let body: serde_json::Value = res.json().await?;
    assert_eq!(body["code"], "DUPLICATE_ID");

    // 9. Get non-existent vector -> 404 VECTOR_NOT_FOUND
    let res = client
        .get(format!("{}/collections/test_col/vectors/999", base_url))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);
    let body: serde_json::Value = res.json().await?;
    assert_eq!(body["code"], "VECTOR_NOT_FOUND");

    // 10. Delete non-existent vector -> 404 VECTOR_NOT_FOUND
    let res = client
        .delete(format!("{}/collections/test_col/vectors/999", base_url))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);
    let body: serde_json::Value = res.json().await?;
    assert_eq!(body["code"], "VECTOR_NOT_FOUND");

    // 11. Search with empty query -> 400 INVALID_PARAMETER
    let res = client
        .post(format!("{}/collections/test_col/search", base_url))
        .json(&serde_json::json!({
            "query": [],
            "k": 5
        }))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = res.json().await?;
    assert_eq!(body["code"], "INVALID_PARAMETER");

    // 12. Search with k = 0 -> 400 INVALID_PARAMETER
    let res = client
        .post(format!("{}/collections/test_col/search", base_url))
        .json(&serde_json::json!({
            "query": [1.0, 2.0, 3.0, 4.0],
            "k": 0
        }))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = res.json().await?;
    assert_eq!(body["code"], "INVALID_PARAMETER");

    // 13. Search non-existent collection -> 404 COLLECTION_NOT_FOUND
    let res = client
        .post(format!("{}/collections/non_existent/search", base_url))
        .json(&serde_json::json!({
            "query": [1.0, 2.0, 3.0, 4.0],
            "k": 5
        }))
        .send()
        .await?;
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);
    let body: serde_json::Value = res.json().await?;
    assert_eq!(body["code"], "COLLECTION_NOT_FOUND");

    Ok(())
}

#[tokio::test]
async fn test_api_persistence_across_server_restarts() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let db_path = dir.path().join("server_persist_data");

    // Phase 1: Start Server 1, insert vectors via HTTP, take snapshot, insert more via WAL
    {
        let db = Arc::new(VectorDb::open(&db_path)?);
        let (base_url, handle) = spawn_server(db).await;
        let client = reqwest::Client::new();

        // Create collection
        let res = client
            .post(format!("{}/collections", base_url))
            .json(&serde_json::json!({
                "name": "persist_col",
                "dim": 3,
                "metric": "L2"
            }))
            .send()
            .await?;
        assert_eq!(res.status(), reqwest::StatusCode::CREATED);

        // Insert vectors 1, 2, 3
        for id in 1..=3 {
            let res = client
                .post(format!("{}/collections/persist_col/insert", base_url))
                .json(&serde_json::json!({
                    "id": id,
                    "vector": [id as f32, (id * 2) as f32, (id * 3) as f32],
                    "metadata": { "id": id }
                }))
                .send()
                .await?;
            assert_eq!(res.status(), reqwest::StatusCode::OK);
        }

        // Trigger snapshot via HTTP POST /snapshot
        let snap_res = client
            .post(format!("{}/snapshot", base_url))
            .send()
            .await?;
        assert_eq!(snap_res.status(), reqwest::StatusCode::OK);

        // Insert vector 4 post-snapshot (in WAL only)
        let res = client
            .post(format!("{}/collections/persist_col/insert", base_url))
            .json(&serde_json::json!({
                "id": 4,
                "vector": [4.0, 8.0, 12.0],
                "metadata": { "id": 4 }
            }))
            .send()
            .await?;
        assert_eq!(res.status(), reqwest::StatusCode::OK);

        // Delete vector 2 via HTTP
        let del_res = client
            .delete(format!("{}/collections/persist_col/vectors/2", base_url))
            .send()
            .await?;
        assert_eq!(del_res.status(), reqwest::StatusCode::OK);

        // Abruptly terminate server 1
        handle.abort();
    }

    // Phase 2: Start Server 2 from the same directory on disk
    {
        let db = Arc::new(VectorDb::open(&db_path)?);
        let (base_url, _handle) = spawn_server(db).await;
        let client = reqwest::Client::new();

        // Verify collection recovered
        let res = client
            .get(format!("{}/collections/persist_col", base_url))
            .send()
            .await?;
        assert_eq!(res.status(), reqwest::StatusCode::OK);
        let info: serde_json::Value = res.json().await?;
        assert_eq!(info["vector_count"], 3); // IDs 1, 3, 4 (2 was deleted)

        // Verify vector 1 exists with metadata
        let v1_res = client
            .get(format!("{}/collections/persist_col/vectors/1", base_url))
            .send()
            .await?;
        assert_eq!(v1_res.status(), reqwest::StatusCode::OK);
        let v1: serde_json::Value = v1_res.json().await?;
        assert_eq!(v1["metadata"]["id"], 1);

        // Verify vector 2 is absent
        let v2_res = client
            .get(format!("{}/collections/persist_col/vectors/2", base_url))
            .send()
            .await?;
        assert_eq!(v2_res.status(), reqwest::StatusCode::NOT_FOUND);

        // Verify vector 4 exists (WAL recovery post-snapshot)
        let v4_res = client
            .get(format!("{}/collections/persist_col/vectors/4", base_url))
            .send()
            .await?;
        assert_eq!(v4_res.status(), reqwest::StatusCode::OK);

        // Search vectors over HTTP
        let search_res = client
            .post(format!("{}/collections/persist_col/search", base_url))
            .json(&serde_json::json!({
                "query": [1.0, 2.0, 3.0],
                "k": 10
            }))
            .send()
            .await?;
        assert_eq!(search_res.status(), reqwest::StatusCode::OK);
        let results: Vec<SearchResult> = search_res.json().await?;
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].id, 1);
    }

    Ok(())
}
