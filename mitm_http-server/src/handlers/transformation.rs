use axum::{
    extract::{State, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::handlers::{AppState, ErrorResponse, JsonApiError};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sources", get(handle_sources))
        .route("/targets", get(handle_targets))
        .route("/rules", get(handle_rules))
        .route("/transformations", get(handle_transformations))
        .route("/validations", get(handle_validations))
        .route("/errors", get(handle_errors))
        .route("/topic-dependencies", get(handle_topic_dependencies))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MappingSource {
    pub id: i32,
    pub source_type: String,
    pub name: String,
    pub schema_json: serde_json::Value,
}

async fn handle_sources(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_as::<_, MappingSource>("SELECT id, source_type, name, schema_json FROM mapping_source")
        .fetch_all(&state.repo.pool).await
    {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }]
        })).into_response()
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MappingTarget {
    pub id: i32,
    pub name: String,
    pub data_type: String,
    pub required: bool,
}

async fn handle_targets(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_as::<_, MappingTarget>("SELECT id, name, data_type, required FROM mapping_target_field")
        .fetch_all(&state.repo.pool).await
    {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }]
        })).into_response()
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MappingRule {
    pub id: i32,
    pub source_id: Option<i32>,
    pub target_id: Option<i32>,
    pub map_type: String,
}

async fn handle_rules(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_as::<_, MappingRule>("SELECT id, source_id, target_id, map_type FROM mapping_rule")
        .fetch_all(&state.repo.pool).await
    {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }]
        })).into_response()
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MappingTransformation {
    pub id: i32,
    pub rule_id: i32,
    pub trans_type: String,
    pub params: serde_json::Value,
}

async fn handle_transformations(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_as::<_, MappingTransformation>("SELECT id, rule_id, trans_type, params FROM mapping_transformation")
        .fetch_all(&state.repo.pool).await
    {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }]
        })).into_response()
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MappingValidation {
    pub id: i32,
    pub rule_id: i32,
    pub val_type: String,
    pub params: serde_json::Value,
}

async fn handle_validations(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_as::<_, MappingValidation>("SELECT id, rule_id, val_type, params FROM mapping_validation")
        .fetch_all(&state.repo.pool).await
    {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }]
        })).into_response()
    }
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TransformationError {
    pub id: i32,
    pub run_id: i32,
    pub mapping_rule_id: Option<i32>,
    pub source_data: serde_json::Value,
    pub error_msg: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn handle_errors(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let sql = "SELECT id, run_id, mapping_rule_id, source_data, error_msg, created_at FROM transformation_errors ORDER BY created_at DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, TransformationError>(sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.repo.pool)
        .await
    {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }]
        })).into_response()
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TopicDependency {
    pub id: i32,
    pub topic_name: String,
    pub depends_on: String,
}

async fn handle_topic_dependencies(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_as::<_, TopicDependency>("SELECT id, topic_name, depends_on FROM topic_dependencies")
        .fetch_all(&state.repo.pool).await
    {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }]
        })).into_response()
    }
}
