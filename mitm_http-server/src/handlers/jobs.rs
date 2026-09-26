/*
 * SPDX-License-Identifier: Apache-2.0
 */

use axum::{
    extract::{State, Query, Multipart},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use tokio::net::UnixStream;
use tokio::io::AsyncWriteExt;
use std::path::Path;
use mitm_common::ipc::SchedulerRequest;
use std::str::FromStr;

use crate::handlers::{AppState, ErrorResponse, JsonApiError};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/jobs", get(handle_get_jobs))
        .route("/update-jobs", post(handle_update_jobs))
        .route("/delete-job", delete(handle_delete_job))
        .route("/stop-job", post(handle_stop_job))
        .route("/execute-job", post(handle_execute_job))
        .route("/upload/source_file", post(handle_upload_file))
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct ScheduledProgram {
    #[serde(default)]
    pub id: i32,
    pub name: String,
    pub command: String,
    pub args: Option<serde_json::Value>,
    pub cron_expr: Option<String>,
    pub enabled: bool,
    pub restart_on_exit: bool,
}

#[derive(Serialize)]
pub struct JobResponse {
    #[serde(flatten)]
    pub program: ScheduledProgram,
    pub next_run: String,
    pub is_running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_pid: Option<i32>,
}

#[derive(sqlx::FromRow)]
struct JobRow {
    id: i32,
    name: String,
    command: String,
    args: Option<serde_json::Value>,
    cron_expr: Option<String>,
    enabled: bool,
    restart_on_exit: bool,
    active_pid: Option<i32>,
    is_running: Option<bool>,
}

async fn handle_get_jobs(State(state): State<AppState>) -> impl IntoResponse {
    let query = "SELECT sp.id, sp.name, sp.command, sp.args, sp.cron_expr, sp.enabled, sp.restart_on_exit, pr.pid as active_pid, CASE WHEN pr.id IS NOT NULL THEN true ELSE false END as is_running FROM scheduled_programs sp LEFT JOIN program_runs pr ON sp.id = pr.program_id AND pr.finished_at IS NULL ORDER BY sp.name ASC";
    match sqlx::query_as::<_, JobRow>(query)
        .fetch_all(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(rows) => {
            // let now = chrono::Utc::now();
            let mut jobs = Vec::new();
            for r in rows {
                let mut next_run = String::new();
                if r.enabled {
                    if let Some(ref cron) = r.cron_expr {
                        let mut cron_str = cron.clone();
                        let parts: Vec<&str> = cron_str.split_whitespace().collect();
                        if parts.len() == 5 {
                            cron_str = format!("0 {} *", cron_str);
                        }
                        if let Ok(schedule) = cron::Schedule::from_str(&cron_str) {
                            if let Some(next) = schedule.upcoming(chrono::Utc).next() {
                                next_run = next.to_rfc3339();
                            }
                        }
                    }
                }

                jobs.push(JobResponse {
                    program: ScheduledProgram {
                        id: r.id,
                        name: r.name,
                        command: r.command,
                        args: r.args,
                        cron_expr: r.cron_expr,
                        enabled: r.enabled,
                        restart_on_exit: r.restart_on_exit,
                    },
                    next_run,
                    is_running: r.is_running.unwrap_or(false),
                    active_pid: r.active_pid,
                });
            }
            (StatusCode::OK, Json(jobs)).into_response()
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

async fn send_scheduler_ipc(req: SchedulerRequest, socket_dir: &str) -> Result<(), String> {
    let socket_path = std::path::Path::new(socket_dir).join("mitm_scheduler.sock");
    let mut stream = UnixStream::connect(socket_path).await.map_err(|e| e.to_string())?;
    let mut out = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    out.push('\n');
    stream.write_all(out.as_bytes()).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn handle_update_jobs(
    State(state): State<AppState>,
    Json(jobs): Json<Vec<ScheduledProgram>>,
) -> impl IntoResponse {
    let mut tx = match state.repo.get().unwrap().pool.begin().await {
        Ok(tx) => tx,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }] })).into_response()
    };

    for job in jobs {
        let sql = "INSERT INTO scheduled_programs (name, command, args, cron_expr, enabled, restart_on_exit, updated_at)
                   VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP)
                   ON CONFLICT (name) DO UPDATE SET
                   command = EXCLUDED.command, args = EXCLUDED.args,
                   cron_expr = EXCLUDED.cron_expr, enabled = EXCLUDED.enabled, restart_on_exit = EXCLUDED.restart_on_exit,
                   updated_at = CURRENT_TIMESTAMP";
        if let Err(e) = sqlx::query(sql)
            .bind(&job.name)
            .bind(&job.command)
            .bind(&job.args)
            .bind(&job.cron_expr)
            .bind(job.enabled)
            .bind(job.restart_on_exit)
            .execute(&mut *tx)
            .await
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }] })).into_response();
        }
    }

    if let Err(e) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }] })).into_response();
    }

    match send_scheduler_ipc(SchedulerRequest::UpdateJobs, &state.config.socket_dir).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "message": "Jobs updated and scheduler reloaded" }))).into_response(),
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
pub struct JobNameQuery {
    pub name: String,
}

async fn handle_delete_job(
    State(state): State<AppState>,
    Query(query): Query<JobNameQuery>,
) -> impl IntoResponse {
    match sqlx::query("DELETE FROM scheduled_programs WHERE name = $1")
        .bind(query.name)
        .execute(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(_) => {
            let _ = send_scheduler_ipc(SchedulerRequest::UpdateJobs, &state.config.socket_dir).await;
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

async fn handle_stop_job(
    State(state): State<AppState>,
    Query(query): Query<JobNameQuery>,
) -> impl IntoResponse {
    match send_scheduler_ipc(SchedulerRequest::StopJob { job_name: query.name.clone() }, &state.config.socket_dir).await {
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
    State(state): State<AppState>,
    Query(query): Query<JobNameQuery>,
) -> impl IntoResponse {
    match send_scheduler_ipc(SchedulerRequest::ExecuteJob { job_name: query.name.clone() }, &state.config.socket_dir).await {
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

pub async fn handle_upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let _ = std::fs::create_dir_all(&state.config.upload_dir);
    
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name = "upload".to_string();
    let mut topic = "upload".to_string();

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();
        if name == "topic" {
            if let Ok(text) = field.text().await {
                topic = text;
            }
        } else if name == "file" {
            file_name = field.file_name().unwrap_or("upload").to_string();
            if let Ok(data) = field.bytes().await {
                file_data = Some(data.to_vec());
            }
        }
    }

    if let Some(data) = file_data {
        let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let dest_path = Path::new(&state.config.upload_dir).join(format!("{}_{}", timestamp, file_name));
        
        if let Err(e) = std::fs::write(&dest_path, &data) {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": format!("Failed to save file: {}", e) }))).into_response();
        }
        
        // Trigger collector
        let args_json = serde_json::json!({
            "file": dest_path.to_string_lossy().to_string(),
            "topic": topic
        }).to_string();
        
        use mitm_common::ipc::{SchedulerRequest, RunImmediateJob};
        let req = SchedulerRequest::RunImmediateJob(RunImmediateJob {
            command: "mitm-collector-csv-xls".to_string(),
            args: args_json,
        });
        
        let _ = send_scheduler_ipc(req, &state.config.socket_dir).await;

        return (StatusCode::OK, Json(serde_json::json!({ "message": "File uploaded" }))).into_response();
    }

    (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "No file field found" }))).into_response()
}
