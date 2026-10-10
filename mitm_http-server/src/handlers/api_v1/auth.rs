use axum::{
    extract::{State, Json},
    http::{StatusCode, HeaderMap, header::AUTHORIZATION},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{Utc, Duration};
use crate::handlers::{AppState, ErrorResponse, JsonApiError};

#[derive(Deserialize)]
pub struct SessionRequest {
    pub os_user: String,
    #[allow(dead_code)]
    pub token: Option<String>,
    pub client_ip: Option<String>,
}

#[derive(Serialize)]
pub struct SessionResponse {
    pub session_token: String,
}

#[derive(Serialize)]
pub struct RolesResponse {
    pub roles: Vec<String>,
    pub os_user: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_active: bool,
    pub client_ip: Option<String>,
}

// Handler for POST /api/user/v1/session
pub async fn create_session(
    State(state): State<AppState>,
    Json(payload): Json<SessionRequest>,
) -> Response {
    if let Some(repo) = state.repo.get() {
        match sqlx::query_scalar::<_, bool>("SELECT is_active FROM admin_users WHERE username = $1")
            .bind(&payload.os_user)
            .fetch_optional(&repo.pool)
            .await
        {
            Ok(Some(active)) => {
                if !active {
                    let err = ErrorResponse { errors: vec![JsonApiError { status: "403".into(), title: "Forbidden".into(), detail: Some("Login Rejected: User account is inactive.".into()) }] };
                    return (StatusCode::FORBIDDEN, Json(err)).into_response();
                }
            }
            Ok(None) => {}
            Err(e) => log::error!("DB error checking is_active: {}", e),
        }
    }
    
    let token_uuid = Uuid::new_v4();
    let now = Utc::now();
    let expires_at = now + Duration::hours(24);
    
    let result = sqlx::query(
        r#"
        INSERT INTO user_sessions (session_token, os_user, created_at, expires_at, last_active_at, client_ip)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#
    )
    .bind(token_uuid)
    .bind(&payload.os_user)
    .bind(now)
    .bind(expires_at)
    .bind(now)
    .bind(&payload.client_ip)
    .execute(&state.repo.get().unwrap().pool)
    .await;

    match result {
        Ok(_) => {
            if let Some(repo) = state.repo.get() {
                let details = serde_json::json!({
                    "action": "login",
                    "status": "success"
                });
                let _ = repo.log_admin(&payload.os_user, "USER_LOGIN", details).await;
            }

            let response = SessionResponse {
                session_token: token_uuid.to_string(),
            };
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => {
            log::error!("Failed to create user session: {}", e);
            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Internal Server Error".into(), detail: Some("Database error".into()) }] };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
    }
}

// Handler for GET /api/user/v1/roles
pub async fn get_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let auth_header = match headers.get(AUTHORIZATION).and_then(|h| h.to_str().ok()) {
        Some(h) => h,
        None => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "401".into(), title: "Unauthorized".into(), detail: Some("Missing authorization header".into()) }] };
            return (StatusCode::UNAUTHORIZED, Json(err)).into_response();
        }
    };
        
    if !auth_header.starts_with("Bearer ") {
        let err = ErrorResponse { errors: vec![JsonApiError { status: "401".into(), title: "Unauthorized".into(), detail: Some("Invalid authorization format".into()) }] };
        return (StatusCode::UNAUTHORIZED, Json(err)).into_response();
    }
    
    let token_str = &auth_header[7..];
    
    let token_uuid = match Uuid::parse_str(token_str) {
        Ok(u) => u,
        Err(_) => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "401".into(), title: "Unauthorized".into(), detail: Some("Invalid token format".into()) }] };
            return (StatusCode::UNAUTHORIZED, Json(err)).into_response();
        }
    };

    let now = Utc::now();
    
    let session_opt = match sqlx::query_as::<_, (String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>, Option<String>)>(
        r#"
        SELECT os_user, expires_at, last_active_at, client_ip FROM user_sessions
        WHERE session_token = $1
        "#
    )
    .bind(token_uuid)
    .fetch_optional(&state.repo.get().unwrap().pool)
    .await {
        Ok(s) => s,
        Err(e) => {
            log::error!("DB error fetching session: {}", e);
            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Internal Server Error".into(), detail: Some("Database error".into()) }] };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };

    if let Some(session) = session_opt {
        if session.1 < now {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "401".into(), title: "Unauthorized".into(), detail: Some("Session expired (absolute TTL)".into()) }] };
            return (StatusCode::UNAUTHORIZED, Json(err)).into_response();
        }
        
        if session.2.unwrap_or(now) + Duration::hours(2) < now {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "401".into(), title: "Unauthorized".into(), detail: Some("Session expired (idle timeout)".into()) }] };
            return (StatusCode::UNAUTHORIZED, Json(err)).into_response();
        }

        let _ = sqlx::query(
            r#"
            UPDATE user_sessions SET last_active_at = $1 WHERE session_token = $2
            "#
        )
        .bind(now)
        .bind(token_uuid)
        .execute(&state.repo.get().unwrap().pool)
        .await;

        let pool = &state.repo.get().unwrap().pool;
        let mut roles = vec![];
        let mut first_name = None;
        let mut last_name = None;
        let mut is_active = false;

        if let Ok(Some((user_id, f_name, l_name, active))) = sqlx::query_as::<_, (i32, Option<String>, Option<String>, bool)>(
            "SELECT id, first_name, last_name, is_active FROM admin_users WHERE username = $1"
        )
            .bind(&session.0)
            .fetch_optional(pool).await 
        {
            first_name = f_name;
            last_name = l_name;
            is_active = active;

            let row = sqlx::query_as::<_, (Vec<u8>, Vec<u8>, Vec<u8>)>(
                "SELECT wrapped_dek, nonce, encrypted_roles FROM user_roles_encrypted WHERE user_id = $1"
            )
            .bind(user_id)
            .fetch_optional(pool).await.unwrap_or(None);

            if let Some((wrapped_dek, nonce, encrypted_roles)) = row {
                let socket_dir = std::path::PathBuf::from(&state.config.socket_dir);
                let socket_path = socket_dir.join("mitm_scheduler.sock");

                if let Ok(plaintext) = crate::ipc_client::crypto_decrypt(wrapped_dek, nonce, encrypted_roles, &socket_path).await {
                    if let Ok(role_ids) = serde_json::from_slice::<Vec<i32>>(&plaintext) {
                        for rid in role_ids {
                            if let Ok(Some(name)) = sqlx::query_scalar::<_, String>("SELECT name FROM roles WHERE id = $1")
                                .bind(rid)
                                .fetch_optional(pool).await 
                            {
                                roles.push(name);
                            }
                        }
                    }
                }
            }
        }

        Json(RolesResponse { 
            roles, 
            os_user: session.0,
            first_name,
            last_name,
            is_active,
            client_ip: session.3,
        }).into_response()

    } else {
        let err = ErrorResponse { errors: vec![JsonApiError { status: "401".into(), title: "Unauthorized".into(), detail: Some("Invalid session token".into()) }] };
        (StatusCode::UNAUTHORIZED, Json(err)).into_response()
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/session", post(create_session))
        .route("/me", get(get_roles))
}
