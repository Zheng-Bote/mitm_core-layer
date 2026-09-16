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
        .route("/system", get(handle_system_logs))
        .route("/job-audit", get(handle_job_audit_logs))
        .route("/admin-audit", get(handle_admin_audit_logs))
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
    let limit = query.limit.unwrap_or(100).min(1000); // Max 1000 per page
    let offset = query.offset.unwrap_or(0);

    let sql = "SELECT id, created_at, level, component, message FROM system_logs ORDER BY created_at DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, SystemLog>(sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.repo.pool)
        .await
    {
        Ok(logs) => (StatusCode::OK, Json(logs)).into_response(),
        Err(e) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError {
                    status: "500".into(),
                    title: "Database Error".into(),
                    detail: Some(e.to_string()),
                }],
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
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
    match sqlx::query_as::<_, JobAuditLog>(sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.repo.pool)
        .await
    {
        Ok(logs) => (StatusCode::OK, Json(logs)).into_response(),
        Err(e) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError {
                    status: "500".into(),
                    title: "Database Error".into(),
                    detail: Some(e.to_string()),
                }],
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
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
    match sqlx::query_as::<_, AdminAuditLog>(sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.repo.pool)
        .await
    {
        Ok(logs) => (StatusCode::OK, Json(logs)).into_response(),
        Err(e) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError {
                    status: "500".into(),
                    title: "Database Error".into(),
                    detail: Some(e.to_string()),
                }],
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
    }
}
