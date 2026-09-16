use axum::{Router, routing::get};
use crate::handlers::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
}
