use axum::{Router, routing::{get, post}, extract::{State, Path, Query}};
use crate::handlers::AppState;
use axum::response::IntoResponse;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(crate::handlers::rbac::handle_get_users))
        .route("/users", post(crate::handlers::rbac::handle_create_user))
        .route("/users/:id", get(handle_get_user_roles_v1).delete(handle_delete_user_v1).put(handle_edit_user_v1))
        .route("/users/:id/session", axum::routing::delete(handle_terminate_session_v1))
        .route("/roles", get(crate::handlers::rbac::handle_get_roles))
        .route("/assign-role", post(crate::handlers::rbac::handle_assign_roles))
}

async fn handle_get_user_roles_v1(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    crate::handlers::rbac::handle_get_user_roles(State(state), Query(crate::handlers::rbac::GetUserRolesQuery { user_id: id })).await
}

async fn handle_delete_user_v1(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    crate::handlers::rbac::handle_delete_user(State(state), Query(crate::handlers::rbac::UserIdQuery { id })).await
}

async fn handle_edit_user_v1(State(state): State<AppState>, Path(id): Path<i32>, axum::extract::Json(payload): axum::extract::Json<crate::handlers::rbac::EditUserReq>) -> impl IntoResponse {
    crate::handlers::rbac::handle_edit_user(State(state), Path(id), axum::extract::Json(payload)).await
}

async fn handle_terminate_session_v1(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    crate::handlers::rbac::handle_terminate_session(State(state), Path(id)).await
}
