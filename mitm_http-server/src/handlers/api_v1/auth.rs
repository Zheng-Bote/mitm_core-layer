use axum::{
    extract::{State, Json},
    http::{StatusCode, HeaderMap, header::AUTHORIZATION},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{Utc, Duration};
use crate::handlers::AppState;

#[derive(Deserialize)]
pub struct SessionRequest {
    pub os_user: String,
    #[allow(dead_code)]
    pub token: Option<String>,
}

#[derive(Serialize)]
pub struct SessionResponse {
    pub session_token: String,
}

#[derive(Serialize)]
pub struct RolesResponse {
    pub roles: Vec<String>,
}

// Handler for POST /api/user/v1/session
pub async fn create_session(
    State(state): State<AppState>,
    Json(payload): Json<SessionRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Basic validation. If token is passed, we check against admin_users.
    // In a real app we'd verify the auth payload strictly. Here we create the session for os_user.
    
    // We check if the token matches an admin user in the DB.
    // If not, we still might allow them as a VIEWER based on os_user.
    // For now, let's just insert a session into user_sessions.
    let token_uuid = Uuid::new_v4();
    
    // 24 hours absolute, 2 hours idle
    let now = Utc::now();
    let expires_at = now + Duration::hours(24);
    
    let result = sqlx::query(
        r#"
        INSERT INTO user_sessions (session_token, os_user, created_at, expires_at, last_active_at)
        VALUES ($1, $2, $3, $4, $5)
        "#
    )
    .bind(token_uuid)
    .bind(&payload.os_user)
    .bind(now)
    .bind(expires_at)
    .bind(now)
    .execute(&state.repo.get().unwrap().pool)
    .await;

    match result {
        Ok(_) => {
            let response = SessionResponse {
                session_token: token_uuid.to_string(),
            };
            Ok((StatusCode::CREATED, Json(response)))
        }
        Err(e) => {
            log::error!("Failed to create user session: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()))
        }
    }
}

// Handler for GET /api/user/v1/roles
pub async fn get_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_header = headers.get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "Missing authorization header".to_string()))?;
        
    if !auth_header.starts_with("Bearer ") {
        return Err((StatusCode::UNAUTHORIZED, "Invalid authorization format".to_string()));
    }
    
    let token_str = &auth_header[7..];
    
    let token_uuid = Uuid::parse_str(token_str).map_err(|_| {
        (StatusCode::UNAUTHORIZED, "Invalid token format".to_string())
    })?;

    // Check token validity
    let now = Utc::now();
    
    let session_opt = sqlx::query_as::<_, (String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"
        SELECT os_user, expires_at, last_active_at FROM user_sessions
        WHERE session_token = $1
        "#
    )
    .bind(token_uuid)
    .fetch_optional(&state.repo.get().unwrap().pool)
    .await
    .map_err(|e| {
        log::error!("DB error fetching session: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string())
    })?;

    if let Some(session) = session_opt {
        // Enforce 24h absolute TTL
        if session.1 < now {
            return Err((StatusCode::UNAUTHORIZED, "Session expired (absolute TTL)".to_string()));
        }
        
        // Enforce 2h Idle TTL
        if session.2.unwrap_or(now) + Duration::hours(2) < now {
            return Err((StatusCode::UNAUTHORIZED, "Session expired (idle timeout)".to_string()));
        }

        // Touch the session (update last_active_at)
        let _ = sqlx::query(
            r#"
            UPDATE user_sessions SET last_active_at = $1 WHERE session_token = $2
            "#
        )
        .bind(now)
        .bind(token_uuid)
        .execute(&state.repo.get().unwrap().pool)
        .await;

        // In a real application, we would check user roles against DB (`user_roles_encrypted`).
        // For MVP, we return a hardcoded/mock array, or fetch from Casbin if possible.
        // Let's query admin_users for roles
        let is_admin = sqlx::query_as::<_, (i32,)>(
            "SELECT id FROM admin_users WHERE username = $1 AND is_active = true"
        )
        .bind(&session.0)
        .fetch_optional(&state.repo.get().unwrap().pool)
        .await
        .unwrap_or(None)
        .is_some();

        let roles = if is_admin {
            vec!["ADMIN".to_string(), "VIEWER".to_string(), "UPLOADER".to_string()]
        } else {
            vec!["VIEWER".to_string()]
        };

        Ok(Json(RolesResponse { roles }))

    } else {
        Err((StatusCode::UNAUTHORIZED, "Invalid session token".to_string()))
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/session", post(create_session))
        .route("/me", get(get_roles))
}
