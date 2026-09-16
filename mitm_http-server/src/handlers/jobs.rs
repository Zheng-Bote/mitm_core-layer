use axum::{Router, routing::get};
use crate::handlers::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        // .route("/", get(handle_get_jobs))
}
