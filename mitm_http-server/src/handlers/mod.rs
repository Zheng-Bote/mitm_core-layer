/*
 * SPDX-License-Identifier: Apache-2.0
 */

use axum::Router;
use axum::routing::get;
use std::sync::Arc;
use axum::extract::State;


pub mod admin;
pub mod jobs;
pub mod rbac;
pub mod logs;
pub mod api_v1;
pub mod dlq;
pub mod transformation;

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<tokio::sync::OnceCell<crate::db::Repository>>,
    pub config: Arc<mitm_common::config::DBConfig>,
    pub tera: Arc<tera::Tera>,
    pub enforcer: Arc<tokio::sync::RwLock<casbin::Enforcer>>,
}

#[derive(serde::Serialize)]
pub struct JsonApiError {
    pub status: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ErrorResponse {
    pub errors: Vec<JsonApiError>,
}

pub async fn handle_info(axum::extract::State(state): axum::extract::State<AppState>) -> axum::response::Response {
    let db_version: String = sqlx::query_scalar("SELECT version()")
        .fetch_one(&state.repo.get().unwrap().pool).await.unwrap_or_else(|_| "Unknown".into());
    let db_name: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&state.repo.get().unwrap().pool).await.unwrap_or_else(|_| "Unknown".into());

    let info = serde_json::json!({
        "name": "MitM Core Layer",
        "description": "Backend services for the MitM project",
        "database": {
            "name": db_name,
            "version": db_version
        },
        "core_components": [
            { "name": "mitm_http-server", "version": env!("CARGO_PKG_VERSION") },
            { "name": "mitm_iam-server", "version": env!("CARGO_PKG_VERSION") },
            { "name": "mitm_scheduler-server", "version": env!("CARGO_PKG_VERSION") }
        ]
    });
    axum::response::IntoResponse::into_response(axum::Json(info))
}

async fn handle_health(State(state): State<AppState>) -> axum::response::Response {
    if state.repo.get().is_some() {
        axum::response::IntoResponse::into_response((axum::http::StatusCode::OK, r#"{"status":"ready"}"#))
    } else {
        axum::response::IntoResponse::into_response((axum::http::StatusCode::OK, r#"{"status":"booting"}"#))
    }
}

pub async fn handle_time() -> axum::response::Response {
    let now = chrono::Local::now();
    let res = serde_json::json!({
        "local_time": now.to_rfc3339(),
        "timestamp": now.timestamp(),
        "timezone": now.offset().to_string(),
    });
    axum::response::IntoResponse::into_response(axum::Json(res))
}

pub fn configure_routes(mitm_dir: String, state: AppState) -> Router<AppState> {
    let public_dir = std::path::Path::new(&mitm_dir).join("html").join("public");
    let serve_dir = tower_http::services::ServeDir::new(public_dir);

    Router::new()
        .route("/", get(handle_index))
        .route("/info", get(handle_info))
        .route("/health", get(handle_health))
        .route("/time", get(handle_time))
                .nest("/admin", admin::routes())
        .nest("/admin", jobs::routes())
        .nest("/admin/rbac", rbac::routes())
        .nest("/admin/logs", logs::routes())
        .nest("/api", api_v1::routes(state.clone()))
        .nest("/admin/dlq", dlq::routes())
        .route("/admin/dlq_bin", get(dlq::handle_dlq_bin))
        .nest("/admin/transformation", transformation::routes())
        .fallback_service(serve_dir)
}

async fn handle_index(State(state): State<AppState>) -> axum::response::Response {
    use axum::response::IntoResponse;
    let mut context = tera::Context::new();
    context.insert("version", env!("CARGO_PKG_VERSION"));
    
    match state.tera.render("index.html", &context) {
        Ok(html) => axum::response::Html(html).into_response(),
        Err(e) => {
            log::error!("Template render error: {}", e);
            axum::response::IntoResponse::into_response((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Template rendering failed",
            ))
        }
    }
}


use casbin::CoreApi;

pub async fn authz_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let path = req.uri().path().to_string();
    let method = req.method().as_str().to_string();
    
    let mut allowed = false;
    let enforcer = state.enforcer.read().await;
    
    for role in auth.roles {
        if let Ok(true) = enforcer.enforce((role.clone(), path.clone(), method.clone())) {
            allowed = true;
            break;
        }
    }
    
    if allowed {
        next.run(req).await
    } else {
        let err = ErrorResponse {
            errors: vec![JsonApiError {
                status: "403".into(),
                title: "Forbidden".into(),
                detail: Some("You do not have permission to access this resource".into()),
            }],
        };
        (axum::http::StatusCode::FORBIDDEN, axum::Json(err)).into_response()
    }
}
