use axum::{Router, middleware};
use crate::handlers::AppState;

pub mod admin;
pub mod transformation;
pub mod public;
pub mod system;
pub mod user;

pub fn routes(state: AppState) -> Router<AppState> {
    let authz_layer = middleware::from_fn_with_state(state.clone(), crate::handlers::authz_middleware);
    
    let admin_routes = admin::routes().layer(authz_layer.clone());
    let transformation_routes = transformation::routes().layer(authz_layer);
    let public_routes = public::routes();
    let system_routes = system::routes();
    let user_routes = user::routes(); // Unprotected (auth happens inside handlers)

    Router::new()
        .nest("/user/v1", user_routes)
        .nest("/admin/v1", admin_routes)
        .nest("/transformation/v1", transformation_routes)
        .nest("/public/v1", public_routes)
        .nest("/system/v1", system_routes)
}
