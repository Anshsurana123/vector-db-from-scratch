use axum::{body::Body, http::{self, Request, StatusCode}};
use serde_json::json;
use tower::ServiceExt;
use vectordb_server::api::app;
use vectordb_core::VectorDb;
use std::sync::Arc;

#[tokio::test]
async fn test_pq_endpoints() {
    let db = Arc::new(VectorDb::new());
    let app = app(db);

    // 1. Create collection with PQ
    let req = Request::builder()
        .method(http::Method::POST)
        .uri("/collections")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name": "pq_test",
                "dim": 4,
                "metric": "L2",
                "is_quantized": true,
                "num_subvectors": 2
            })
            .to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 2. Train PQ explicitly
    let req = Request::builder()
        .method(http::Method::POST)
        .uri("/collections/pq_test/train_pq")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"num_subvectors": 2}).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. Compact collection
    let req = Request::builder()
        .method(http::Method::POST)
        .uri("/collections/pq_test/compact")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Snapshot
    let req = Request::builder()
        .method(http::Method::POST)
        .uri("/snapshot")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
