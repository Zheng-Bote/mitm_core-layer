use axum::{Router, middleware};
use crate::handlers::AppState;

pub mod admin;
pub mod transformation;
pub mod public;

pub fn routes(state: AppState) -> Router<AppState> {
    let authz_layer = middleware::from_fn_with_state(state.clone(), crate::handlers::authz_middleware);
    
    let admin_routes = admin::routes().layer(authz_layer.clone());
    let transformation_routes = transformation::routes().layer(authz_layer);
    let public_routes = public::routes();

    Router::new()
        .nest("/admin/v1", admin_routes)
        .nest("/transformation/v1", transformation_routes)
        .nest("/public/v1", public_routes)
}
