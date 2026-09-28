use axum::{Router, routing::{get, post}, extract::{State, Query}, http::HeaderMap};
use crate::handlers::AppState;
use axum::response::IntoResponse;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sources", get(crate::handlers::transformation::handle_sources))
        .route("/targets", get(crate::handlers::transformation::handle_targets))
        .route("/rules", get(crate::handlers::transformation::handle_rules))
        .route("/transformations", get(crate::handlers::transformation::handle_transformations))
        .route("/validations", get(crate::handlers::transformation::handle_validations))
        .route("/topic-dependencies", get(crate::handlers::transformation::handle_topic_dependencies))
        .route("/auto-map", post(crate::handlers::transformation::handle_auto_map))
        // Content negotiated errors endpoint
        .route("/errors", get(handle_errors_v1))
}

async fn handle_errors_v1(
    State(state): State<AppState>,
    Query(query): Query<crate::handlers::transformation::PaginationQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let accept = headers.get(axum::http::header::ACCEPT).and_then(|v| v.to_str().ok()).unwrap_or("");
    if accept.contains("application/x-flatbuffers") {
        crate::handlers::transformation::handle_errors_bin(State(state), Query(query)).await.into_response()
    } else {
        crate::handlers::transformation::handle_errors(State(state), Query(query)).await.into_response()
    }
}
