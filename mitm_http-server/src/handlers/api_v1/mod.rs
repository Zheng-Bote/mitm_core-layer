use axum::{Router, middleware};
use crate::handlers::AppState;

pub mod system;
pub mod jobs;
pub mod logs;
pub mod dlq;
pub mod config;
pub mod iam;
pub mod auth;
pub mod webauthn;
pub mod oidc;
pub mod auth_guard;

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

    let rate_limit_layer = middleware::from_fn_with_state(state.clone(), auth_guard::rate_limit_middleware);

    let api_v1_router = Router::new()
        .nest("/auth", auth::routes().merge(webauthn::routes().merge(oidc::routes()).route_layer(rate_limit_layer)))
        .nest("/system", system::routes())
        .merge(protected);
        
    Router::new().nest("/v1", api_v1_router)
}
