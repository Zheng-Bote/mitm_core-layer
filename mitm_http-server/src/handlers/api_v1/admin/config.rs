use axum::{Router, routing::{get, post, put}, extract::{State, Path, Json}, response::IntoResponse};
use crate::handlers::AppState;
use crate::handlers::admin::SourceCredentialResponse;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/credentials", get(crate::handlers::admin::handle_credentials))
        .route("/credentials", post(crate::handlers::admin::handle_post_credentials))
        // We also want to support PUT /credentials/:id for RESTful updates if needed, 
        // but since handle_post_credentials already handles update via payload.id, we can map PUT as well,
        // intercepting the path ID and injecting it into the payload.
        .route("/credentials/:id", put(handle_put_credentials))
        
        .route("/delivery_targets", get(crate::handlers::admin::handle_delivery_targets))
        .route("/delivery_targets", post(crate::handlers::admin::handle_post_delivery_targets))
        .route("/delivery_targets/:id", put(handle_put_delivery_targets))
}

async fn handle_put_credentials(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut payload): Json<SourceCredentialResponse>
) -> impl IntoResponse {
    payload.id = Some(id);
    crate::handlers::admin::handle_post_credentials(State(state), Json(payload)).await
}

async fn handle_put_delivery_targets(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut payload): Json<crate::handlers::admin::DeliveryTargetResponse>
) -> impl IntoResponse {
    payload.id = Some(id);
    crate::handlers::admin::handle_post_delivery_targets(State(state), Json(payload)).await
}
