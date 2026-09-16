mod ipc_client;

use std::env;
use axum::{
    routing::{get, post},
    Router, Json, http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use axum_server::tls_rustls::RustlsConfig;
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use mitm_common::config::load_config;
use std::net::SocketAddr;

#[derive(Serialize)]
struct JsonApiError {
    status: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Serialize)]
struct ErrorResponse {
    errors: Vec<JsonApiError>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdminActionPayload {
    action: String,
    details: serde_json::Value,
}

#[derive(Serialize)]
struct AdminActionResponse {
    success: bool,
    message: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let config_param = args.get(1).map(|s| s.as_str());
    let password = env::var("MASTER_KEY").unwrap_or_else(|_| "".to_string());
    
    let config = match load_config(config_param, &password) {
        Ok(cfg) => cfg,
        Err(e) => {
            log::error!("Failed to load configuration: {}", e);
            return Err(e);
        }
    };

    let app = Router::new()
        .route("/admin/action", post(handle_admin_action))
        .route("/admin/rbac/os_user_roles", get(handle_get_roles))
        .route_layer(middleware::from_fn(auth_middleware));

    let addr = SocketAddr::from(([0, 0, 0, 0], config.http_port));
    
    if config.use_https {
        log::info!("Starting HTTPS Server on {}", addr);
        let tls_config = RustlsConfig::from_pem_file(&config.ssl_cert, &config.ssl_key)
            .await
            .unwrap_or_else(|e| panic!("Failed to load TLS certificates: {}", e));
            
        axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service())
            .await?;
    } else {
        log::info!("Starting HTTP Server on {}", addr);
        axum_server::bind(addr)
            .serve(app.into_make_service())
            .await?;
    }

    Ok(())
}

async fn auth_middleware(mut request: axum::extract::Request, next: Next) -> Response {
    let mut extracted_creds = None;
    if let Some(auth_header) = request.headers().get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Basic ") {
                let encoded = &auth_str[6..];
                if let Ok(decoded) = BASE64.decode(encoded) {
                    if let Ok(credentials) = String::from_utf8(decoded) {
                        let parts: Vec<&str> = credentials.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            extracted_creds = Some((parts[0].to_string(), parts[1].to_string()));
                        }
                    }
                }
            }
        }
    }

    if let Some((username, token)) = extracted_creds {
        match ipc_client::authenticate_via_ipc(&username, &token).await {
            Ok(auth_result) if auth_result.success => {
                request.extensions_mut().insert(auth_result);
                return next.run(request).await;
            }
            _ => {
                // Fall through to unauthorized
            }
        }
    }

    let err_resp = ErrorResponse {
        errors: vec![JsonApiError {
            status: "401".to_string(),
            title: "Unauthorized".to_string(),
            detail: Some("Missing or invalid authorization credentials.".to_string()),
        }],
    };
    
    (StatusCode::UNAUTHORIZED, Json(err_resp)).into_response()
}

async fn handle_admin_action(
    axum::extract::Extension(user): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    Json(payload): Json<AdminActionPayload>,
) -> impl IntoResponse {
    log::info!("User {} performed action: {}", user.username, payload.action);
    (StatusCode::OK, Json(AdminActionResponse { success: true, message: "Action logged".to_string() }))
}

#[derive(Deserialize)]
struct RolesQuery {
    os_user: String,
}

async fn handle_get_roles(
    axum::extract::Extension(user): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    axum::extract::Query(query): axum::extract::Query<RolesQuery>,
) -> impl IntoResponse {
    log::info!("Fetching roles for os_user: {} requested by {}", query.os_user, user.username);
    // In a full implementation, we'd query the DB for the roles of `os_user`.
    // For now, return a placeholder as the Go version would.
    (StatusCode::OK, Json(vec!["ADMIN".to_string()]))
}
