use axum::{
    extract::{State, Query},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use crate::handlers::{AppState, ErrorResponse, JsonApiError};
use crate::schematas;
use flatbuffers::FlatBufferBuilder;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handle_dlq))
        .route("/dlq_bin", get(handle_dlq_bin))
        .route("/requeue", post(handle_requeue))
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct DlqEntry {
    pub id: i32,
    pub run_id: i32,
    pub source: String,
    pub payload: serde_json::Value,
    pub error_msg: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn handle_dlq(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let sql = "SELECT id, run_id, source, payload, error_msg, created_at FROM dlq ORDER BY created_at DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, DlqEntry>(sql).bind(limit).bind(offset).fetch_all(&state.repo.pool).await {
        Ok(entries) => (StatusCode::OK, Json(entries)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Database Error".into(), detail: Some(e.to_string()) }] })).into_response()
    }
}

async fn handle_dlq_bin(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let sql = "SELECT id, run_id, source, payload, error_msg, created_at FROM dlq ORDER BY created_at DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, DlqEntry>(sql).bind(limit).bind(offset).fetch_all(&state.repo.pool).await {
        Ok(entries) => {
            let mut builder = FlatBufferBuilder::new();
            let mut entry_offsets = Vec::new();

            for e in &entries {
                let id_str = builder.create_string(&e.id.to_string());
                let pkg_id_str = builder.create_string(&e.run_id.to_string());
                let payload_str = builder.create_string(&e.payload.to_string());
                let err_msg_str = builder.create_string(&e.error_msg);
                let failed_at_str = builder.create_string(&e.created_at.to_rfc3339());
                
                let log_off = schematas::DLQEntry::create(&mut builder, &schematas::DLQEntryArgs {
                    id: Some(id_str),
                    package_id: Some(pkg_id_str),
                    payload: Some(payload_str),
                    error_code: None,
                    error_message: Some(err_msg_str),
                    failed_at: Some(failed_at_str),
                    failed_at_unix_ms: e.created_at.timestamp_millis(),
                    resolved: false,
                    resolved_at: None,
                    resolved_at_unix_ms: 0,
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
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn handle_requeue() -> impl IntoResponse {
    (StatusCode::ACCEPTED, Json(serde_json::json!({"status": "accepted"})))
}
