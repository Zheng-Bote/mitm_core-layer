use axum::{Router, routing::{get, post}};
use crate::handlers::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/system/backup", get(crate::handlers::admin::handle_backup))
        .route("/system/restore", post(crate::handlers::admin::handle_restore))
        .route("/system/key-rotation", post(crate::handlers::admin::handle_key_rotation))
        .route("/system/storage-keys", get(crate::handlers::admin::handle_get_storage_keys))
}
