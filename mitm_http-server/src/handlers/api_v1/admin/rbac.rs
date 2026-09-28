use axum::{Router, routing::{get, post, delete}, extract::{State, Path, Query}};
use crate::handlers::AppState;
use axum::response::IntoResponse;


pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/rbac/roles", get(crate::handlers::rbac::handle_get_roles))
        .route("/rbac/users", get(crate::handlers::rbac::handle_get_users))
        .route("/rbac/users", post(crate::handlers::rbac::handle_create_user))
        .route("/rbac/users/:id", delete(handle_delete_user_v1))
        .route("/rbac/assign", post(crate::handlers::rbac::handle_assign_roles))
        .route("/rbac/user_roles", get(crate::handlers::rbac::handle_get_user_roles))
        .route("/rbac/os_user_roles", get(crate::handlers::rbac::handle_get_os_user_roles))
}

async fn handle_delete_user_v1(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let q = crate::handlers::rbac::UserIdQuery { id };
    crate::handlers::rbac::handle_delete_user(State(state), Query(q)).await
}
