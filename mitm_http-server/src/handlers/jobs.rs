use axum::{
    extract::{State, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::UnixStream;
use tokio::io::AsyncWriteExt;
use mitm_common::ipc::SchedulerRequest;

use crate::handlers::{AppState, ErrorResponse, JsonApiError};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handle_get_jobs))
        .route("/update-jobs", post(handle_update_jobs))
        .route("/delete-job", delete(handle_delete_job))
        .route("/stop-job", post(handle_stop_job))
        .route("/execute-job", post(handle_execute_job))
        // .route("/upload/source_file", post(handle_upload_file))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ScheduledProgram {
    pub id: i32,
    pub name: String,
    pub command: String,
    pub args: Option<serde_json::Value>,
    pub cron_expr: Option<String>,
    pub enabled: bool,
    pub restart_on_exit: bool,
}

async fn handle_get_jobs(State(state): State<AppState>) -> impl IntoResponse {
    let query = "SELECT id, name, command, args, cron_expr, enabled, restart_on_exit FROM scheduled_programs ORDER BY name ASC";
    match sqlx::query_as::<_, ScheduledProgram>(query)
        .fetch_all(&state.repo.pool)
        .await
    {
        Ok(jobs) => (StatusCode::OK, Json(jobs)).into_response(),
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

async fn send_scheduler_ipc(req: SchedulerRequest) -> Result<(), String> {
    // We assume socket is /var/run/mitm_scheduler.sock for now
    // A production version should read from config
    let socket_path = "/var/run/mitm_scheduler.sock";
    let mut stream = UnixStream::connect(socket_path).await.map_err(|e| e.to_string())?;
    let mut out = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    out.push('\n');
    stream.write_all(out.as_bytes()).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn handle_update_jobs() -> impl IntoResponse {
    match send_scheduler_ipc(SchedulerRequest::UpdateJobs).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError {
                    status: "500".into(),
                    title: "IPC Error".into(),
                    detail: Some(e),
                }],
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct JobIdQuery {
    pub id: i32,
}

async fn handle_delete_job(
    State(state): State<AppState>,
    Query(query): Query<JobIdQuery>,
) -> impl IntoResponse {
    match sqlx::query("DELETE FROM scheduled_programs WHERE id = $1")
        .bind(query.id)
        .execute(&state.repo.pool)
        .await
    {
        Ok(_) => {
            let _ = send_scheduler_ipc(SchedulerRequest::UpdateJobs).await;
            StatusCode::OK.into_response()
        },
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

#[derive(Deserialize)]
pub struct JobIdPayload {
    pub id: i32,
}

async fn handle_stop_job(
    Json(payload): Json<JobIdPayload>,
) -> impl IntoResponse {
    match send_scheduler_ipc(SchedulerRequest::StopJob(payload.id)).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError {
                    status: "500".into(),
                    title: "IPC Error".into(),
                    detail: Some(e),
                }],
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
    }
}

async fn handle_execute_job(
    Json(payload): Json<JobIdPayload>,
) -> impl IntoResponse {
    match send_scheduler_ipc(SchedulerRequest::ExecuteJob(payload.id)).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError {
                    status: "500".into(),
                    title: "IPC Error".into(),
                    detail: Some(e),
                }],
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
    }
}
