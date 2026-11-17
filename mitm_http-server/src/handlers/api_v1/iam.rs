use axum::{Router, routing::{get, put}};
use crate::handlers::AppState;
use crate::handlers::rbac;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(rbac::handle_get_users).post(rbac::handle_create_user))
        .route("/users/:id", get(rbac::handle_get_user).put(rbac::handle_edit_user).delete(rbac::handle_delete_user))
        .route("/users/:id/roles", put(rbac::handle_set_user_roles))
        .route("/users/:id/session", axum::routing::delete(rbac::handle_terminate_session))
        .route("/roles", get(rbac::handle_get_roles))
}
