use axum::{Router, routing::get, extract::{State, Query}, http::HeaderMap};
use crate::handlers::AppState;
use axum::response::IntoResponse;
use crate::handlers::logs::LogQuery;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/system", get(handle_system_logs_v1))
        .route("/audit", get(handle_job_audit_logs_v1)) // combined or just map to job-audit for now
        .route("/transformation-errors", get(handle_errors_v1))
}

async fn handle_system_logs_v1(State(state): State<AppState>, Query(query): Query<LogQuery>, headers: HeaderMap) -> impl IntoResponse {
    let accept = headers.get(axum::http::header::ACCEPT).and_then(|v| v.to_str().ok()).unwrap_or("");
    if accept.contains("application/x-flatbuffers") {
        crate::handlers::logs::handle_system_logs_bin(State(state), Query(query)).await.into_response()
    } else {
        crate::handlers::logs::handle_system_logs(State(state), Query(query)).await.into_response()
    }
}

async fn handle_job_audit_logs_v1(State(state): State<AppState>, Query(query): Query<LogQuery>, headers: HeaderMap) -> impl IntoResponse {
    let accept = headers.get(axum::http::header::ACCEPT).and_then(|v| v.to_str().ok()).unwrap_or("");
    if accept.contains("application/x-flatbuffers") {
        crate::handlers::logs::handle_job_audit_logs_bin(State(state), Query(query)).await.into_response()
    } else {
        crate::handlers::logs::handle_job_audit_logs(State(state), Query(query)).await.into_response()
    }
}

async fn handle_errors_v1(State(state): State<AppState>, Query(query): Query<crate::handlers::transformation::PaginationQuery>, headers: HeaderMap) -> impl IntoResponse {
    let accept = headers.get(axum::http::header::ACCEPT).and_then(|v| v.to_str().ok()).unwrap_or("");
    if accept.contains("application/x-flatbuffers") {
        crate::handlers::transformation::handle_errors_bin(State(state), Query(query)).await.into_response()
    } else {
        crate::handlers::transformation::handle_errors(State(state), Query(query)).await.into_response()
    }
}
