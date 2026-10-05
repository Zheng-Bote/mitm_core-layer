use axum::{Router, middleware};
use crate::handlers::AppState;

pub mod system;
pub mod jobs;
pub mod logs;
pub mod dlq;
pub mod config;
pub mod iam;
pub mod auth;

pub fn routes(state: AppState) -> Router<AppState> {
    let authz_layer = middleware::from_fn_with_state(state.clone(), crate::handlers::authz_middleware);

    let protected = Router::new()
        .nest("/system", system::protected_routes())
        .nest("/jobs", jobs::routes())
        .nest("/logs", logs::routes())
        .nest("/dlq", dlq::routes())
        .nest("/config", config::routes())
        .nest("/iam", iam::routes())
        .layer(authz_layer);

    Router::new()
        .nest("/auth", auth::routes())
        .nest("/system", system::routes())
        .merge(protected)
}
