use axum::{
    extract::{State, Query},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use crate::handlers::AppState;
use crate::schematas;
use flatbuffers::FlatBufferBuilder;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sources", get(handle_sources))
        .route("/targets", get(handle_targets))
        .route("/rules", get(handle_rules))
        .route("/transformations", get(handle_transformations))
        .route("/validations", get(handle_validations))
        .route("/errors", get(handle_errors))
        .route("/errors_bin", get(handle_errors_bin))
        .route("/topic-dependencies", get(handle_topic_dependencies))
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct DataSource { pub id: i32, pub name: String, pub connection_type: String, pub config: serde_json::Value, }
#[derive(Serialize, sqlx::FromRow)]
pub struct DataTarget { pub id: i32, pub name: String, pub target_type: String, pub config: serde_json::Value, }
#[derive(Serialize, sqlx::FromRow)]
pub struct MappingRule { pub id: i32, pub name: String, pub source_id: i32, pub target_id: i32, pub mapping_config: serde_json::Value, }
#[derive(Serialize, sqlx::FromRow)]
pub struct ActiveTransformation { pub id: i32, pub mapping_rule_id: i32, pub status: String, }
#[derive(Serialize, sqlx::FromRow)]
pub struct ValidationRule { pub id: i32, pub mapping_rule_id: i32, pub field_name: String, pub rule_type: String, pub parameters: serde_json::Value, }
#[derive(Serialize, sqlx::FromRow)]
pub struct TransformationError { pub id: i32, pub run_id: i32, pub mapping_rule_id: Option<i32>, pub source_data: serde_json::Value, pub error_msg: String, pub created_at: chrono::DateTime<chrono::Utc>, }

async fn handle_sources(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let sql = "SELECT id, name, connection_type, config FROM data_sources ORDER BY name LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, DataSource>(sql).bind(query.limit.unwrap_or(100)).bind(query.offset.unwrap_or(0)).fetch_all(&state.repo.pool).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
async fn handle_targets(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let sql = "SELECT id, name, target_type, config FROM data_targets ORDER BY name LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, DataTarget>(sql).bind(query.limit.unwrap_or(100)).bind(query.offset.unwrap_or(0)).fetch_all(&state.repo.pool).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
async fn handle_rules(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let sql = "SELECT id, name, source_id, target_id, mapping_config FROM mapping_rules ORDER BY name LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, MappingRule>(sql).bind(query.limit.unwrap_or(100)).bind(query.offset.unwrap_or(0)).fetch_all(&state.repo.pool).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
async fn handle_transformations(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let sql = "SELECT id, mapping_rule_id, status FROM active_transformations LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, ActiveTransformation>(sql).bind(query.limit.unwrap_or(100)).bind(query.offset.unwrap_or(0)).fetch_all(&state.repo.pool).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
async fn handle_validations(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let sql = "SELECT id, mapping_rule_id, field_name, rule_type, parameters FROM validation_rules LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, ValidationRule>(sql).bind(query.limit.unwrap_or(100)).bind(query.offset.unwrap_or(0)).fetch_all(&state.repo.pool).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
async fn handle_errors(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let sql = "SELECT id, run_id, mapping_rule_id, source_data, error_msg, created_at FROM transformation_errors ORDER BY created_at DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, TransformationError>(sql).bind(query.limit.unwrap_or(100)).bind(query.offset.unwrap_or(0)).fetch_all(&state.repo.pool).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
async fn handle_errors_bin(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let sql = "SELECT id, run_id, mapping_rule_id, source_data, error_msg, created_at FROM transformation_errors ORDER BY created_at DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, TransformationError>(sql).bind(query.limit.unwrap_or(100)).bind(query.offset.unwrap_or(0)).fetch_all(&state.repo.pool).await {
        Ok(res) => {
            let mut builder = FlatBufferBuilder::new();
            let mut err_offsets = Vec::new();

            for e in &res {
                let id_str = builder.create_string(&e.id.to_string());
                let corr_str = builder.create_string(&e.run_id.to_string());
                let rule_str = builder.create_string(&e.mapping_rule_id.map_or("".to_string(), |v| v.to_string()));
                let msg_str = builder.create_string(&e.error_msg);
                let ts_str = builder.create_string(&e.created_at.to_rfc3339());
                
                let err_off = schematas::TransformationError::create(&mut builder, &schematas::TransformationErrorArgs {
                    id: Some(id_str),
                    correlation_id: Some(corr_str),
                    topic: None,
                    failed_field: None,
                    rule_name: Some(rule_str),
                    error_message: Some(msg_str),
                    created_at: Some(ts_str),
                    created_at_unix_ms: e.created_at.timestamp_millis(),
                });
                err_offsets.push(err_off);
            }

            let vec = builder.create_vector(&err_offsets);
            let list_off = schematas::TransformationErrorList::create(&mut builder, &schematas::TransformationErrorListArgs {
                errors: Some(vec),
            });
            builder.finish(list_off, None);

            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/octet-stream"), (header::CONTENT_DISPOSITION, "attachment; filename=transformation_errors.bin")],
                builder.finished_data().to_vec()
            ).into_response()
        },
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
async fn handle_topic_dependencies() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}
