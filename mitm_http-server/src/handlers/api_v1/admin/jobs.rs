use axum::{Router, routing::{get, post, delete}, extract::{State, Path, Query}};
use crate::handlers::AppState;
use axum::response::IntoResponse;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/jobs", get(crate::handlers::jobs::handle_get_jobs))
        .route("/jobs", post(crate::handlers::jobs::handle_update_jobs))
        .route("/jobs/:name", delete(handle_delete_job_v1))
        .route("/jobs/:name/stop", post(handle_stop_job_v1))
        .route("/jobs/:name/execute", post(handle_execute_job_v1))
        // V1 does not have upload source file here, maybe later or keep in v0
}

async fn handle_delete_job_v1(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let q = crate::handlers::jobs::JobNameQuery { name };
    crate::handlers::jobs::handle_delete_job(State(state), Query(q)).await
}

async fn handle_stop_job_v1(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let q = crate::handlers::jobs::JobNameQuery { name };
    crate::handlers::jobs::handle_stop_job(State(state), Query(q)).await
}

async fn handle_execute_job_v1(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let q = crate::handlers::jobs::JobNameQuery { name };
    crate::handlers::jobs::handle_execute_job(State(state), Query(q)).await
}
