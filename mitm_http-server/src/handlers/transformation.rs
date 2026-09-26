/*
 * SPDX-License-Identifier: Apache-2.0
 */

use axum::{
    extract::{State, Query},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
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
        .route("/auto-map", post(handle_auto_map))
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MappingSource { pub id: uuid::Uuid, pub name: String, pub r#type: String, pub topic: String, pub version: i32, }
#[derive(Serialize, sqlx::FromRow, Clone)]
pub struct MappingTargetField { pub id: uuid::Uuid, pub topic: String, pub field_name: String, pub data_type: String, pub is_required: bool, pub encrypted: bool, pub version: i32, }
#[derive(Serialize, sqlx::FromRow)]
pub struct MappingRule { pub id: uuid::Uuid, pub source_id: uuid::Uuid, pub target_field_id: uuid::Uuid, pub source_field: String, pub priority: i32, pub transformation_chain: Option<serde_json::Value>, pub validation_chain: Option<serde_json::Value>, pub version: i32, }
#[derive(Serialize, sqlx::FromRow)]
pub struct MappingTransformation { pub id: uuid::Uuid, pub name: String, pub description: Option<String>, pub parameters: Option<serde_json::Value>, pub version: i32, }
#[derive(Serialize, sqlx::FromRow)]
pub struct MappingValidation { pub id: uuid::Uuid, pub name: String, pub description: Option<String>, pub parameters: Option<serde_json::Value>, pub version: i32, }
#[derive(Serialize, sqlx::FromRow)]
pub struct TransformationError { pub id: uuid::Uuid, pub raw_ingestion_id: uuid::Uuid, pub failed_field: String, pub rule_name: String, pub error_message: String, pub created_at: chrono::DateTime<chrono::Utc>, }
#[derive(Serialize, sqlx::FromRow)]
pub struct TopicDependency { pub topic: String, pub required_sources: Vec<String>, }

async fn handle_sources(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let sql = "SELECT id, name, type, topic, version FROM mapping_source ORDER BY name LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, MappingSource>(sql).bind(query.limit.unwrap_or(100)).bind(query.offset.unwrap_or(0)).fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
async fn handle_targets(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let sql = "SELECT id, topic, field_name, data_type, is_required, encrypted, version FROM mapping_target_field ORDER BY topic LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, MappingTargetField>(sql).bind(query.limit.unwrap_or(100)).bind(query.offset.unwrap_or(0)).fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
async fn handle_rules(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let sql = "SELECT id, source_id, target_field_id, source_field, priority, transformation_chain, validation_chain, version FROM mapping_rule ORDER BY priority LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, MappingRule>(sql).bind(query.limit.unwrap_or(100)).bind(query.offset.unwrap_or(0)).fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
async fn handle_transformations(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let sql = "SELECT id, name, description, parameters, version FROM mapping_transformation ORDER BY name LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, MappingTransformation>(sql).bind(query.limit.unwrap_or(100)).bind(query.offset.unwrap_or(0)).fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
async fn handle_validations(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let sql = "SELECT id, name, description, parameters, version FROM mapping_validation ORDER BY name LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, MappingValidation>(sql).bind(query.limit.unwrap_or(100)).bind(query.offset.unwrap_or(0)).fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn handle_errors(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let mut builder = sqlx::QueryBuilder::new("SELECT id, raw_ingestion_id, failed_field, rule_name, error_message, created_at FROM transformation_errors WHERE 1=1 ");
    
    if let Some(ref from) = query.from {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(from) {
            builder.push(" AND created_at >= ");
            builder.push_bind(dt.with_timezone(&chrono::Utc));
        }
    }
    if let Some(ref to) = query.to {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(to) {
            builder.push(" AND created_at <= ");
            builder.push_bind(dt.with_timezone(&chrono::Utc));
        }
    }
    
    builder.push(" ORDER BY created_at DESC LIMIT ");
    builder.push_bind(query.limit.unwrap_or(100));
    builder.push(" OFFSET ");
    builder.push_bind(query.offset.unwrap_or(0));

    match builder.build_query_as::<TransformationError>().fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
async fn handle_errors_bin(State(state): State<AppState>, Query(query): Query<PaginationQuery>) -> impl IntoResponse {
    let mut builder = sqlx::QueryBuilder::new("SELECT id, raw_ingestion_id, failed_field, rule_name, error_message, created_at FROM transformation_errors WHERE 1=1 ");
    
    if let Some(ref from) = query.from {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(from) {
            builder.push(" AND created_at >= ");
            builder.push_bind(dt.with_timezone(&chrono::Utc));
        }
    }
    if let Some(ref to) = query.to {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(to) {
            builder.push(" AND created_at <= ");
            builder.push_bind(dt.with_timezone(&chrono::Utc));
        }
    }
    
    builder.push(" ORDER BY created_at DESC LIMIT ");
    builder.push_bind(query.limit.unwrap_or(1000));
    builder.push(" OFFSET ");
    builder.push_bind(query.offset.unwrap_or(0));

    match builder.build_query_as::<TransformationError>().fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(res) => {
            let mut fb = FlatBufferBuilder::new();
            let mut err_offsets = Vec::new();

            for e in &res {
                let id_str = fb.create_string(&e.id.to_string());
                let corr_str = fb.create_string(&e.raw_ingestion_id.to_string());
                let field_str = fb.create_string(&e.failed_field);
                let rule_str = fb.create_string(&e.rule_name);
                let msg_str = fb.create_string(&e.error_message);
                let ts_str = fb.create_string(&e.created_at.to_rfc3339());
                
                let err_off = schematas::TransformationError::create(&mut fb, &schematas::TransformationErrorArgs {
                    id: Some(id_str),
                    correlation_id: Some(corr_str), // Using correlation_id mapping to raw_ingestion_id for flatbuffer
                    topic: None,
                    failed_field: Some(field_str),
                    rule_name: Some(rule_str),
                    error_message: Some(msg_str),
                    created_at: Some(ts_str),
                    created_at_unix_ms: e.created_at.timestamp_millis(),
                });
                err_offsets.push(err_off);
            }

            let vec = fb.create_vector(&err_offsets);
            let list_off = schematas::TransformationErrorList::create(&mut fb, &schematas::TransformationErrorListArgs {
                errors: Some(vec),
            });
            fb.finish(list_off, None);

            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/octet-stream"), (header::CONTENT_DISPOSITION, "attachment; filename=transformation_errors.bin")],
                fb.finished_data().to_vec()
            ).into_response()
        },
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
async fn handle_topic_dependencies(State(state): State<AppState>) -> impl IntoResponse {
    let sql = "SELECT topic, required_sources FROM topic_dependencies ORDER BY topic";
    match sqlx::query_as::<_, TopicDependency>(sql).fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

fn normalize_for_match(s: &str) -> String {
    s.to_lowercase().replace('_', "").replace('-', "").replace(' ', "")
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let mut d = vec![vec![0; b_chars.len() + 1]; a_chars.len() + 1];

    for i in 0..=a_chars.len() { d[i][0] = i; }
    for j in 0..=b_chars.len() { d[0][j] = j; }

    for i in 1..=a_chars.len() {
        for j in 1..=b_chars.len() {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
        }
    }
    d[a_chars.len()][b_chars.len()]
}

#[derive(Deserialize)]
pub struct AutoMapRequest {
    pub source_id: uuid::Uuid,
    pub source_fields: Vec<String>,
}

pub async fn handle_auto_map(
    State(state): State<AppState>,
    Json(payload): Json<AutoMapRequest>,
) -> impl IntoResponse {
    if payload.source_fields.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Missing source_fields"}))).into_response();
    }

    let targets = match sqlx::query_as::<_, MappingTargetField>("SELECT id, topic, field_name, data_type, is_required, encrypted, version FROM mapping_target_field")
        .fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
    };

    let mut created = 0;
    for sf in payload.source_fields {
        let norm_sf = normalize_for_match(&sf);
        let mut best_target: Option<&MappingTargetField> = None;
        let mut best_dist = 9999;
        
        for tgt in &targets {
            let norm_tgt = normalize_for_match(&tgt.field_name);
            let dist = levenshtein(&norm_sf, &norm_tgt);
            if dist < best_dist && dist <= 3 {
                best_dist = dist;
                best_target = Some(tgt);
            }
        }
        
        if let Some(target) = best_target {
            let id = uuid::Uuid::new_v4();
            let sql = "INSERT INTO mapping_rule (id, source_id, target_field_id, source_field, priority, transformation_chain, validation_chain, version) VALUES ($1, $2, $3, $4, 1, '[]'::jsonb, '[]'::jsonb, 1)";
            if let Ok(_) = sqlx::query(sql)
                .bind(id)
                .bind(payload.source_id)
                .bind(target.id)
                .bind(sf)
                .execute(&state.repo.get().unwrap().pool).await {
                created += 1;
            }
        }
    }

    (StatusCode::OK, Json(serde_json::json!({ "created": created }))).into_response()
}
