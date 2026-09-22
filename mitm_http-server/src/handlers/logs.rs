use axum::{
    extract::{State, Query},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::QueryBuilder;

use crate::handlers::AppState;
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
pub struct LogQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct SystemLog {
    pub id: i32,
    pub ts: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub component: String,
    pub message: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct JobAuditLog {
    pub id: i32,
    pub run_id: i32,
    pub ts: chrono::DateTime<chrono::Utc>,
    pub component: String,
    pub message: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AdminAuditLog {
    pub id: i32,
    pub username: String,
    pub action: String,
    pub details: Option<serde_json::Value>,
    pub ts: chrono::DateTime<chrono::Utc>,
}

async fn get_system_logs(state: &AppState, query: &LogQuery) -> Result<Vec<SystemLog>, sqlx::Error> {
    let mut qb = QueryBuilder::new("SELECT id, ts, level, component, message FROM system_logs WHERE 1=1");
    if let Some(from_str) = &query.from {
        if let Ok(from) = chrono::DateTime::parse_from_rfc3339(from_str) {
            qb.push(" AND ts >= ");
            qb.push_bind(from.with_timezone(&chrono::Utc));
        }
    }
    if let Some(to_str) = &query.to {
        if let Ok(to) = chrono::DateTime::parse_from_rfc3339(to_str) {
            qb.push(" AND ts <= ");
            qb.push_bind(to.with_timezone(&chrono::Utc));
        }
    }
    qb.push(" ORDER BY ts DESC");
    if let Some(limit) = query.limit {
        qb.push(" LIMIT ");
        qb.push_bind(limit);
    } else {
        qb.push(" LIMIT 1000"); // default
    }
    if let Some(offset) = query.offset {
        qb.push(" OFFSET ");
        qb.push_bind(offset);
    }
    qb.build_query_as::<SystemLog>().fetch_all(&state.repo.get().unwrap().pool).await
}

async fn handle_system_logs(
    State(state): State<AppState>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    match get_system_logs(&state, &query).await {
        Ok(logs) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json"), (header::CONTENT_DISPOSITION, "attachment; filename=system_logs.json")],
            Json(logs).into_response()
        ).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn handle_system_logs_bin(
    State(state): State<AppState>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    match get_system_logs(&state, &query).await {
        Ok(logs) => {
            let mut builder = FlatBufferBuilder::new();
            let mut log_offsets = Vec::new();
            for log in &logs {
                let level_str = builder.create_string(&log.level);
                let comp_str = builder.create_string(&log.component);
                let msg_str = builder.create_string(&log.message);
                let ts_str = builder.create_string(&log.ts.to_rfc3339());
                
                let log_off = schematas::SystemLog::create(&mut builder, &schematas::SystemLogArgs {
                    id: log.id as i64,
                    level: Some(level_str),
                    component: Some(comp_str),
                    message: Some(msg_str),
                    ts: Some(ts_str),
                    ts_unix_ms: log.ts.timestamp_millis(),
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

async fn get_job_audit_logs(state: &AppState, query: &LogQuery) -> Result<Vec<JobAuditLog>, sqlx::Error> {
    let mut qb = QueryBuilder::new("SELECT id, run_id, ts, component, message FROM job_audit_logs WHERE 1=1");
    if let Some(from_str) = &query.from {
        if let Ok(from) = chrono::DateTime::parse_from_rfc3339(from_str) {
            qb.push(" AND ts >= ");
            qb.push_bind(from.with_timezone(&chrono::Utc));
        }
    }
    if let Some(to_str) = &query.to {
        if let Ok(to) = chrono::DateTime::parse_from_rfc3339(to_str) {
            qb.push(" AND ts <= ");
            qb.push_bind(to.with_timezone(&chrono::Utc));
        }
    }
    qb.push(" ORDER BY ts DESC");
    if let Some(limit) = query.limit {
        qb.push(" LIMIT ");
        qb.push_bind(limit);
    } else {
        qb.push(" LIMIT 1000"); // default
    }
    if let Some(offset) = query.offset {
        qb.push(" OFFSET ");
        qb.push_bind(offset);
    }
    qb.build_query_as::<JobAuditLog>().fetch_all(&state.repo.get().unwrap().pool).await
}

async fn handle_job_audit_logs(
    State(state): State<AppState>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    match get_job_audit_logs(&state, &query).await {
        Ok(logs) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json"), (header::CONTENT_DISPOSITION, "attachment; filename=job_audit_logs.json")],
            Json(logs).into_response()
        ).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn handle_job_audit_logs_bin(
    State(state): State<AppState>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    match get_job_audit_logs(&state, &query).await {
        Ok(logs) => {
            let mut builder = FlatBufferBuilder::new();
            let mut log_offsets = Vec::new();
            for log in &logs {
                let comp_str = builder.create_string(&log.component);
                let msg_str = builder.create_string(&log.message);
                let ts_str = builder.create_string(&log.ts.to_rfc3339());
                
                let log_off = schematas::JobAuditLog::create(&mut builder, &schematas::JobAuditLogArgs {
                    id: log.id as i64,
                    run_id: log.run_id as i64,
                    component: Some(comp_str),
                    message: Some(msg_str),
                    ts: Some(ts_str),
                    ts_unix_ms: log.ts.timestamp_millis(),
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

async fn get_admin_audit_logs(state: &AppState, query: &LogQuery) -> Result<Vec<AdminAuditLog>, sqlx::Error> {
    let mut qb = QueryBuilder::new("SELECT id, username, action, details, ts FROM admin_audit_logs WHERE 1=1");
    if let Some(from_str) = &query.from {
        if let Ok(from) = chrono::DateTime::parse_from_rfc3339(from_str) {
            qb.push(" AND ts >= ");
            qb.push_bind(from.with_timezone(&chrono::Utc));
        }
    }
    if let Some(to_str) = &query.to {
        if let Ok(to) = chrono::DateTime::parse_from_rfc3339(to_str) {
            qb.push(" AND ts <= ");
            qb.push_bind(to.with_timezone(&chrono::Utc));
        }
    }
    qb.push(" ORDER BY ts DESC");
    if let Some(limit) = query.limit {
        qb.push(" LIMIT ");
        qb.push_bind(limit);
    } else {
        qb.push(" LIMIT 1000"); // default
    }
    if let Some(offset) = query.offset {
        qb.push(" OFFSET ");
        qb.push_bind(offset);
    }
    qb.build_query_as::<AdminAuditLog>().fetch_all(&state.repo.get().unwrap().pool).await
}

async fn handle_admin_audit_logs(
    State(state): State<AppState>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    match get_admin_audit_logs(&state, &query).await {
        Ok(logs) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json"), (header::CONTENT_DISPOSITION, "attachment; filename=admin_audit_logs.json")],
            Json(logs).into_response()
        ).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn handle_admin_audit_logs_bin(
    State(state): State<AppState>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    match get_admin_audit_logs(&state, &query).await {
        Ok(logs) => {
            let mut builder = FlatBufferBuilder::new();
            let mut log_offsets = Vec::new();
            for log in &logs {
                let user_str = builder.create_string(&log.username);
                let action_str = builder.create_string(&log.action);
                let details_json = serde_json::to_string(&log.details).unwrap_or_else(|_| "{}".to_string());
                let details_str = builder.create_string(&details_json);
                let ts_str = builder.create_string(&log.ts.to_rfc3339());
                
                let log_off = schematas::AdminAuditLog::create(&mut builder, &schematas::AdminAuditLogArgs {
                    id: log.id as i64,
                    username: Some(user_str),
                    action: Some(action_str),
                    details: Some(details_str),
                    ts: Some(ts_str),
                    ts_unix_ms: log.ts.timestamp_millis(),
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
