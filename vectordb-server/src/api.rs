use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use vectordb_core::{FilterExpression, MetricType, VectorDb, VectorDbError};

#[derive(Debug)]
pub struct AppState {
    pub db: Arc<VectorDb>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCollectionRequest {
    pub name: String,
    pub dim: usize,
    pub metric: MetricType,
    pub is_quantized: Option<bool>,
    pub num_subvectors: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InsertVectorRequest {
    pub id: u64,
    pub vector: Vec<f32>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: Vec<f32>,
    pub k: usize,
    pub ef_search: Option<usize>,
    pub filter: Option<FilterExpression>,
    pub use_pq: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionInfoResponse {
    pub name: String,
    pub dim: usize,
    pub metric: MetricType,
    pub vector_count: usize,
}

pub struct AppError(VectorDbError);

impl From<VectorDbError> for AppError {
    fn from(err: VectorDbError) -> Self {
        AppError(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self.0 {
            VectorDbError::CollectionNotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            VectorDbError::VectorNotFound(id) => (StatusCode::NOT_FOUND, format!("Vector ID {} not found", id)),
            VectorDbError::CollectionAlreadyExists(msg) => (StatusCode::CONFLICT, msg.clone()),
            VectorDbError::DuplicateId(id) => (StatusCode::CONFLICT, format!("Vector ID {} already exists", id)),
            VectorDbError::DimensionMismatch { expected, actual } => (
                StatusCode::BAD_REQUEST,
                format!("Dimension mismatch: expected {}, got {}", expected, actual),
            ),
            err => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        };

        let body = Json(json!({ "error": message }));
        (status, body).into_response()
    }
}

pub fn app(db: Arc<VectorDb>) -> Router {
    let state = Arc::new(AppState { db });

    Router::new()
        .route("/collections", post(create_collection).get(list_collections))
        .route("/collections/:name", get(get_collection).delete(drop_collection))
        .route("/collections/:name/insert", post(insert_vector))
        .route("/collections/:name/search", post(search_vectors))
        .route("/collections/:name/vectors/:id", delete(delete_vector))
        .route("/collections/:name/compact", post(compact_collection))
        .route("/snapshot", post(create_snapshot))
        .route("/collections/:name/snapshot", post(create_snapshot))
        .route("/collections/:name/train_pq", post(train_pq))
        .with_state(state)
}

async fn create_collection(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateCollectionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let col = state.db.create_collection(req.name, req.dim, req.metric)?;
    
    if req.is_quantized.unwrap_or(false) {
        if let Some(num_sub) = req.num_subvectors {
            col.enable_pq(num_sub)?;
        }
    }
    
    let res = CollectionInfoResponse {
        name: col.name().to_string(),
        dim: col.dim(),
        metric: col.metric(),
        vector_count: col.len(),
    };
    Ok((StatusCode::CREATED, Json(res)))
}

async fn list_collections(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let names = state.db.list_collections();
    Ok((StatusCode::OK, Json(names)))
}

async fn get_collection(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let col = state.db.get_collection(&name)?;
    let res = CollectionInfoResponse {
        name: col.name().to_string(),
        dim: col.dim(),
        metric: col.metric(),
        vector_count: col.len(),
    };
    Ok((StatusCode::OK, Json(res)))
}

async fn drop_collection(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let dropped = state.db.drop_collection(&name)?;
    if dropped {
        Ok((StatusCode::OK, Json(json!({ "status": "dropped", "name": name }))))
    } else {
        Err(AppError(VectorDbError::CollectionNotFound(name)))
    }
}

async fn insert_vector(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(req): Json<InsertVectorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let col = state.db.get_collection(&name)?;
    col.insert(req.id, &req.vector, req.metadata)?;
    Ok((StatusCode::OK, Json(json!({ "status": "inserted", "id": req.id }))))
}

async fn search_vectors(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(req): Json<SearchRequest>,
) -> Result<impl IntoResponse, AppError> {
    let col = state.db.get_collection(&name)?;
    let ef_search = req.ef_search.unwrap_or(32);
    
    let results = if req.use_pq.unwrap_or(false) {
        col.search_pq(&req.query, req.k, ef_search)?
    } else if let Some(filter) = req.filter {
        col.search_with_filter(&req.query, req.k, &filter)?
    } else {
        col.search_hnsw(&req.query, req.k, ef_search)?
    };

    Ok((StatusCode::OK, Json(results)))
}

async fn delete_vector(
    State(state): State<Arc<AppState>>,
    Path((name, id)): Path<(String, u64)>,
) -> Result<impl IntoResponse, AppError> {
    let col = state.db.get_collection(&name)?;
    let deleted = col.delete(id)?;
    if deleted {
        Ok((StatusCode::OK, Json(json!({ "status": "deleted", "id": id }))))
    } else {
        Err(AppError(VectorDbError::VectorNotFound(id)))
    }
}

#[derive(Debug, Deserialize)]
pub struct TrainPqRequest {
    pub num_subvectors: usize,
}

async fn compact_collection(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state.db.compact_collection(&name)?;
    Ok(StatusCode::OK)
}

async fn create_snapshot(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    state.db.save_snapshot()?;
    Ok(StatusCode::OK)
}

async fn train_pq(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(req): Json<TrainPqRequest>,
) -> Result<impl IntoResponse, AppError> {
    state.db.train_pq(&name, req.num_subvectors)?;
    Ok(StatusCode::OK)
}
