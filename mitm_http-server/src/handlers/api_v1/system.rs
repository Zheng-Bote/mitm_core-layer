use axum::{Router, routing::get};
use crate::handlers::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/info", get(crate::handlers::handle_info))
        .route("/time", get(crate::handlers::handle_time))
}
