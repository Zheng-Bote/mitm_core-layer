use axum::Router;
use axum::routing::get;
use std::sync::Arc;
use axum::extract::State;
use crate::db::Repository;

pub mod admin;
pub mod jobs;
pub mod rbac;
pub mod logs;
pub mod dlq;
pub mod transformation;

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<Repository>,
    pub config: Arc<mitm_common::config::DBConfig>,
    pub kek: Arc<Vec<u8>>,
}

#[derive(serde::Serialize)]
pub struct JsonApiError {
    pub status: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ErrorResponse {
    pub errors: Vec<JsonApiError>,
}

async fn handle_info() -> axum::response::Response {
    let info = serde_json::json!({
        "name": "MitM HTTP Gateway",
        "description": "Backend scheduler for the MitM project",
        "version": env!("CARGO_PKG_VERSION"),
    });
    axum::response::IntoResponse::into_response(axum::Json(info))
}

async fn handle_health(State(state): State<AppState>) -> axum::response::Response {
    match sqlx::query("SELECT 1").execute(&state.repo.pool).await {
        Ok(_) => axum::response::IntoResponse::into_response((axum::http::StatusCode::OK, "OK")),
        Err(e) => axum::response::IntoResponse::into_response((axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("DB Error: {}", e))),
    }
}

async fn handle_time() -> axum::response::Response {
    let now = chrono::Local::now();
    let res = serde_json::json!({
        "local_time": now.to_rfc3339(),
        "timestamp": now.timestamp(),
        "timezone": now.offset().to_string(),
    });
    axum::response::IntoResponse::into_response(axum::Json(res))
}

pub fn configure_routes() -> Router<AppState> {
    Router::new()
        .route("/info", get(handle_info))
        .route("/health", get(handle_health))
        .route("/time", get(handle_time))
        .nest("/admin", admin::routes())
        .nest("/admin", jobs::routes())
        .nest("/admin/rbac", rbac::routes())
        .nest("/admin/logs", logs::routes())
        .nest("/admin/dlq", dlq::routes())
        .route("/admin/dlq_bin", get(dlq::handle_dlq_bin))
        .nest("/admin/transformation", transformation::routes())
}
