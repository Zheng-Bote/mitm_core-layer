use axum::{Router, routing::{get, post}, extract::{State, Query}, http::HeaderMap};
use crate::handlers::AppState;
use axum::response::IntoResponse;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dlq", get(handle_dlq_v1))
        .route("/dlq/requeue", post(crate::handlers::dlq::handle_requeue))
}

async fn handle_dlq_v1(
    State(state): State<AppState>,
    Query(query): Query<crate::handlers::dlq::PaginationQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let accept = headers.get(axum::http::header::ACCEPT).and_then(|v| v.to_str().ok()).unwrap_or("");
    if accept.contains("application/x-flatbuffers") {
        crate::handlers::dlq::handle_dlq_bin(State(state), Query(query)).await.into_response()
    } else {
        crate::handlers::dlq::handle_dlq(State(state), Query(query)).await.into_response()
    }
}
