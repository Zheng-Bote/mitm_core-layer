/*
 * SPDX-License-Identifier: Apache-2.0
 */

use sqlx::Row;
use axum::{
    extract::{State, Query},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use crate::handlers::{AppState, ErrorResponse, JsonApiError};
use flatbuffers::FlatBufferBuilder;
use crate::schematas;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handle_dlq))
        .route("/requeue", post(handle_requeue))
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct DlqEntry {
    pub id: uuid::Uuid,
    pub package_id: Option<uuid::Uuid>,
    pub payload: serde_json::Value,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub failed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub resolved: Option<bool>,
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn handle_dlq(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let sql = "SELECT id, package_id, payload, error_code, error_message, failed_at, resolved, resolved_at FROM dead_letter_queue ORDER BY failed_at DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, DlqEntry>(sql).bind(limit).bind(offset).fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(entries) => (StatusCode::OK, Json(entries)).into_response(),
        Err(e) => {
            let err_msg = format!("Database Error: {}", e);
            log::error!("{}", err_msg);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Database Error".into(), detail: Some(e.to_string()) }] })).into_response()
        }
    }
}

pub async fn handle_dlq_bin(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let sql = "SELECT id, package_id, payload, error_code, error_message, failed_at, resolved, resolved_at FROM dead_letter_queue ORDER BY failed_at DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, DlqEntry>(sql).bind(limit).bind(offset).fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(entries) => {
            let mut builder = FlatBufferBuilder::new();
            let mut entry_offsets = Vec::new();

            for e in &entries {
                let id_str = builder.create_string(&e.id.to_string());
                let pkg_id_str = builder.create_string(&e.package_id.map_or("".to_string(), |u| u.to_string()));
                let payload_str = builder.create_string(&e.payload.to_string());
                let err_code_str = builder.create_string(&e.error_code.as_deref().unwrap_or(""));
                let err_msg_str = builder.create_string(&e.error_message.as_deref().unwrap_or(""));
                let failed_at_str = builder.create_string(&e.failed_at.map_or("".to_string(), |dt| dt.to_rfc3339()));
                
                let resolved_at_str = builder.create_string(&e.resolved_at.map_or("".to_string(), |dt| dt.to_rfc3339()));
                
                let log_off = schematas::DLQEntry::create(&mut builder, &schematas::DLQEntryArgs {
                    id: Some(id_str),
                    package_id: Some(pkg_id_str),
                    payload: Some(payload_str),
                    error_code: Some(err_code_str),
                    error_message: Some(err_msg_str),
                    failed_at: Some(failed_at_str),
                    failed_at_unix_ms: e.failed_at.map_or(0, |dt| dt.timestamp_millis()),
                    resolved: e.resolved.unwrap_or(false),
                    resolved_at: Some(resolved_at_str),
                    resolved_at_unix_ms: e.resolved_at.map_or(0, |dt| dt.timestamp_millis()),
                });
                entry_offsets.push(log_off);
            }

            let vec = builder.create_vector(&entry_offsets);
            let list_off = schematas::DLQEntryList::create(&mut builder, &schematas::DLQEntryListArgs {
                entries: Some(vec),
            });
            builder.finish(list_off, None);

            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/octet-stream"), (header::CONTENT_DISPOSITION, "attachment; filename=dlq.bin")],
                builder.finished_data().to_vec()
            ).into_response()
        },
        Err(e) => {
            let err_msg = format!("Database Error: {}", e);
            log::error!("{}", err_msg);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct RequeueRequest {
    pub ids: Vec<uuid::Uuid>,
}

async fn handle_requeue(
    State(state): State<AppState>,
    Json(payload): Json<RequeueRequest>,
) -> impl IntoResponse {
    let sql = "UPDATE dead_letter_queue SET resolved = true, resolved_at = NOW() WHERE id = ANY($1) RETURNING id";
    match sqlx::query(sql).bind(&payload.ids).fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(rows) => {
            let updated: Vec<uuid::Uuid> = rows.iter().map(|r| r.get(0)).collect();
            (StatusCode::OK, Json(serde_json::json!({ "requeued": updated }))).into_response()
        },
        Err(e) => {
            let err_msg = format!("Database Error: {}", e);
            log::error!("{}", err_msg);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
