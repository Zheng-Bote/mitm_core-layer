use axum::Router;
use std::sync::Arc;
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

pub fn configure_routes() -> Router<AppState> {
    Router::new()
        .nest("/admin", admin::routes())
        .nest("/admin/jobs", jobs::routes())
        .nest("/admin/rbac", rbac::routes())
        .nest("/admin/logs", logs::routes())
        .nest("/admin/dlq", dlq::routes())
        .nest("/admin/transformation", transformation::routes())
}
