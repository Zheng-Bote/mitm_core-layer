use axum::{
    extract::{State, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use crate::handlers::{AppState, ErrorResponse, JsonApiError};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handle_dlq))
        .route("/requeue", post(handle_requeue))
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct DlqEntry {
    pub id: i32,
    pub run_id: i32,
    pub source: String,
    pub payload: serde_json::Value,
    pub error_msg: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub requeued: bool,
}

async fn handle_dlq(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let sql = "SELECT id, run_id, source, payload, error_msg, created_at, requeued FROM dead_letter_queue ORDER BY created_at DESC LIMIT $1 OFFSET $2";
    match sqlx::query_as::<_, DlqEntry>(sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.repo.pool)
        .await
    {
        Ok(entries) => (StatusCode::OK, Json(entries)).into_response(),
        Err(e) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError {
                    status: "500".into(),
                    title: "Database Error".into(),
                    detail: Some(e.to_string()),
                }],
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct RequeuePayload {
    pub dlq_ids: Vec<i32>,
}

async fn handle_requeue(
    State(state): State<AppState>,
    Json(payload): Json<RequeuePayload>,
) -> impl IntoResponse {
    if payload.dlq_ids.is_empty() {
        return StatusCode::OK.into_response();
    }

    let mut tx = match state.repo.pool.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }]
        })).into_response()
    };

    for id in payload.dlq_ids {
        if let Err(e) = sqlx::query("UPDATE dead_letter_queue SET requeued = true WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
        {
            let _ = tx.rollback().await;
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }]
            })).into_response()
        }
    }

    let _ = tx.commit().await;
    StatusCode::OK.into_response()
}
