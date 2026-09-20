use axum::{
    extract::{State, Query},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::handlers::{AppState, ErrorResponse, JsonApiError};
use crate::schematas;
use flatbuffers::FlatBufferBuilder;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/system", get(handle_system_logs))
        .route("/system_bin", get(handle_system_logs_bin))
        .route("/job-audit", get(handle_job_audit_logs))
        .route("/job-audit_bin", get(handle_job_audit_logs_bin))
        .route("/admin-audit", get(handle_admin_audit_logs))
        .route("/admin-audit_bin", get(handle_admin_audit_logs_bin))
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct SystemLog {
    pub id: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub component: String,
    pub message: String,
}

async fn handle_system_logs(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let sql = "SELECT id, created_at, level, component, message FROM system_logs ORDER BY created_at DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, SystemLog>(sql).bind(limit).bind(offset).fetch_all(&state.repo.pool).await {
        Ok(logs) => (StatusCode::OK, Json(logs)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Database Error".into(), detail: Some(e.to_string()) }] })).into_response()
    }
}

async fn handle_system_logs_bin(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let sql = "SELECT id, created_at, level, component, message FROM system_logs ORDER BY created_at DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, SystemLog>(sql).bind(limit).bind(offset).fetch_all(&state.repo.pool).await {
        Ok(logs) => {
            let mut builder = FlatBufferBuilder::new();
            let mut log_offsets = Vec::new();

            for log in &logs {
                let level_str = builder.create_string(&log.level);
                let comp_str = builder.create_string(&log.component);
                let msg_str = builder.create_string(&log.message);
                let ts_str = builder.create_string(&log.created_at.to_rfc3339());
                
                let log_off = schematas::SystemLog::create(&mut builder, &schematas::SystemLogArgs {
                    id: log.id as i64,
                    level: Some(level_str),
                    component: Some(comp_str),
                    message: Some(msg_str),
                    ts: Some(ts_str),
                    ts_unix_ms: log.created_at.timestamp_millis(),
                });
                log_offsets.push(log_off);
            }

            let logs_vec = builder.create_vector(&log_offsets);
            let list_off = schematas::SystemLogList::create(&mut builder, &schematas::SystemLogListArgs {
                logs: Some(logs_vec),
            });
            builder.finish(list_off, None);

            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/octet-stream"), (header::CONTENT_DISPOSITION, "attachment; filename=system_logs.bin")],
                builder.finished_data().to_vec()
            ).into_response()
        },
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct JobAuditLog {
    pub id: i32,
    pub run_id: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub component: String,
    pub message: String,
}

async fn handle_job_audit_logs(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let sql = "SELECT id, run_id, created_at, component, message FROM job_audit_logs ORDER BY created_at DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, JobAuditLog>(sql).bind(limit).bind(offset).fetch_all(&state.repo.pool).await {
        Ok(logs) => (StatusCode::OK, Json(logs)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Database Error".into(), detail: Some(e.to_string()) }] })).into_response()
    }
}

async fn handle_job_audit_logs_bin(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let sql = "SELECT id, run_id, created_at, component, message FROM job_audit_logs ORDER BY created_at DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, JobAuditLog>(sql).bind(limit).bind(offset).fetch_all(&state.repo.pool).await {
        Ok(logs) => {
            let mut builder = FlatBufferBuilder::new();
            let mut log_offsets = Vec::new();

            for log in &logs {
                let comp_str = builder.create_string(&log.component);
                let msg_str = builder.create_string(&log.message);
                let ts_str = builder.create_string(&log.created_at.to_rfc3339());
                
                let log_off = schematas::JobAuditLog::create(&mut builder, &schematas::JobAuditLogArgs {
                    id: log.id as i64,
                    run_id: log.run_id as i64,
                    component: Some(comp_str),
                    message: Some(msg_str),
                    ts: Some(ts_str),
                    ts_unix_ms: log.created_at.timestamp_millis(),
                });
                log_offsets.push(log_off);
            }

            let logs_vec = builder.create_vector(&log_offsets);
            let list_off = schematas::JobAuditLogList::create(&mut builder, &schematas::JobAuditLogListArgs {
                logs: Some(logs_vec),
            });
            builder.finish(list_off, None);

            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/octet-stream"), (header::CONTENT_DISPOSITION, "attachment; filename=job_audit_logs.bin")],
                builder.finished_data().to_vec()
            ).into_response()
        },
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AdminAuditLog {
    pub id: i32,
    pub username: String,
    pub action: String,
    pub details: Option<serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

async fn handle_admin_audit_logs(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let sql = "SELECT id, username, action, details, timestamp FROM admin_audit_logs ORDER BY timestamp DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, AdminAuditLog>(sql).bind(limit).bind(offset).fetch_all(&state.repo.pool).await {
        Ok(logs) => (StatusCode::OK, Json(logs)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Database Error".into(), detail: Some(e.to_string()) }] })).into_response()
    }
}

async fn handle_admin_audit_logs_bin(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let sql = "SELECT id, username, action, details, timestamp FROM admin_audit_logs ORDER BY timestamp DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, AdminAuditLog>(sql).bind(limit).bind(offset).fetch_all(&state.repo.pool).await {
        Ok(logs) => {
            let mut builder = FlatBufferBuilder::new();
            let mut log_offsets = Vec::new();

            for log in &logs {
                let user_str = builder.create_string(&log.username);
                let action_str = builder.create_string(&log.action);
                let det_str = builder.create_string(&log.details.as_ref().map_or("".to_string(), |v| v.to_string()));
                let ts_str = builder.create_string(&log.timestamp.to_rfc3339());
                
                let log_off = schematas::AdminAuditLog::create(&mut builder, &schematas::AdminAuditLogArgs {
                    id: log.id as i64,
                    username: Some(user_str),
                    action: Some(action_str),
                    details: Some(det_str),
                    ts: Some(ts_str),
                    ts_unix_ms: log.timestamp.timestamp_millis(),
                });
                log_offsets.push(log_off);
            }

            let logs_vec = builder.create_vector(&log_offsets);
            let list_off = schematas::AdminAuditLogList::create(&mut builder, &schematas::AdminAuditLogListArgs {
                logs: Some(logs_vec),
            });
            builder.finish(list_off, None);

            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/octet-stream"), (header::CONTENT_DISPOSITION, "attachment; filename=admin_audit_logs.bin")],
                builder.finished_data().to_vec()
            ).into_response()
        },
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
