use axum::{Router, routing::{get, post, put}};
use crate::handlers::AppState;
use crate::handlers::jobs;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(jobs::handle_get_jobs).post(jobs::handle_create_job))
        .route("/:name", put(jobs::handle_update_job).delete(jobs::handle_delete_job))
        .route("/:name/stop", post(jobs::handle_stop_job))
        .route("/:name/execute", post(jobs::handle_execute_job))
        .route("/upload/source_file", post(jobs::handle_upload_file))
}
