use axum::Router;
use crate::handlers::AppState;

pub mod jobs;
pub mod logs;
pub mod config;
pub mod system;
pub mod rbac;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(jobs::routes())
        .merge(logs::routes())
        .merge(config::routes())
        .merge(system::routes())
        .merge(rbac::routes())
}
