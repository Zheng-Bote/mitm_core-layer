use axum::{Router, routing::{get, post, put, delete}};
use crate::handlers::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/credentials", get(crate::handlers::admin::handle_credentials))
        .route("/credentials", post(crate::handlers::admin::handle_post_credentials))
        .route("/credentials/:id", put(handle_put_credentials))
        .route("/targets", get(crate::handlers::admin::handle_delivery_targets))
        .route("/targets", post(crate::handlers::admin::handle_post_delivery_targets))
        .route("/targets/:id", put(handle_put_delivery_targets))
        
        .route("/transformations/sources", get(crate::handlers::transformation::handle_sources))
        .route("/transformations/targets", get(crate::handlers::transformation::handle_targets))
        .route("/transformations/rules", get(crate::handlers::transformation::handle_rules))
        .route("/transformations/rules", post(crate::handlers::transformation::handle_post_rules))
        .route("/transformations/rules/:id", delete(crate::handlers::transformation::handle_delete_rules))
        .route("/transformations", get(crate::handlers::transformation::handle_transformations))
        .route("/transformations", post(crate::handlers::transformation::handle_post_transformations))
        .route("/transformations/:id", delete(crate::handlers::transformation::handle_delete_transformations))
        .route("/transformations/validations", get(crate::handlers::transformation::handle_validations))
        .route("/transformations/validations", post(crate::handlers::transformation::handle_post_validations))
        .route("/transformations/validations/:id", delete(crate::handlers::transformation::handle_delete_validations))
        .route("/transformations/topic-dependencies", get(crate::handlers::transformation::handle_topic_dependencies))
        .route("/transformations/topic-dependencies", delete(crate::handlers::transformation::handle_delete_topic_dependencies))
        .route("/transformations/auto-map", post(crate::handlers::transformation::handle_auto_map))
}

use axum::{extract::{State, Path, Json}, response::IntoResponse};
use crate::handlers::admin::SourceCredentialResponse;

pub async fn handle_put_credentials(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut payload): Json<SourceCredentialResponse>
) -> impl IntoResponse {
    payload.id = Some(id);
    crate::handlers::admin::handle_post_credentials(State(state), Json(payload)).await
}

pub async fn handle_put_delivery_targets(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut payload): Json<crate::handlers::admin::DeliveryTargetResponse>
) -> impl IntoResponse {
    payload.id = Some(id);
    crate::handlers::admin::handle_post_delivery_targets(State(state), Json(payload)).await
}
