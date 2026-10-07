use axum::{Router, routing::{get, post, delete}, extract::{State, Path, Query}};
use crate::handlers::AppState;
use axum::response::IntoResponse;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(crate::handlers::jobs::handle_get_jobs))
        .route("/", post(crate::handlers::jobs::handle_update_jobs))
        .route("/:name", delete(handle_delete_job_v1))
        .route("/:name/stop", post(handle_stop_job_v1))
        .route("/:name/execute", post(handle_execute_job_v1))
        .route("/upload/source_file", post(crate::handlers::jobs::handle_upload_file))
}

async fn handle_delete_job_v1(State(state): State<AppState>, Path(name): Path<String>) -> impl IntoResponse {
    crate::handlers::jobs::handle_delete_job(State(state), Query(crate::handlers::jobs::JobNameQuery { name })).await
}

async fn handle_stop_job_v1(State(state): State<AppState>, Path(name): Path<String>) -> impl IntoResponse {
    crate::handlers::jobs::handle_stop_job(State(state), Query(crate::handlers::jobs::JobNameQuery { name })).await
}

async fn handle_execute_job_v1(State(state): State<AppState>, Path(name): Path<String>) -> impl IntoResponse {
    crate::handlers::jobs::handle_execute_job(State(state), Query(crate::handlers::jobs::JobNameQuery { name })).await
}
