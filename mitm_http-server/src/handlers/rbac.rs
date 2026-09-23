use axum::{
    extract::{State, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::handlers::{AppState, ErrorResponse, JsonApiError};
use mitm_common::crypto;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::{rngs::OsRng, RngCore};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/roles", get(handle_get_roles))
        .route("/users", get(handle_get_users))
        .route("/user/create", post(handle_create_user))
        .route("/user/delete", delete(handle_delete_user))
        .route("/assign", post(handle_assign_roles))
        .route("/user_roles", get(handle_get_user_roles))
        .route("/os_user_roles", get(handle_get_os_user_roles))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Role {
    pub id: i32,
    pub name: String,
}

async fn handle_get_roles(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_as::<_, Role>("SELECT id, name FROM roles ORDER BY id ASC")
        .fetch_all(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(roles) => (StatusCode::OK, Json(roles)).into_response(),
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

#[derive(Serialize, sqlx::FromRow)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub is_active: bool,
}

async fn handle_get_users(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_as::<_, User>("SELECT id, username, is_active FROM admin_users ORDER BY id ASC")
        .fetch_all(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(users) => (StatusCode::OK, Json(users)).into_response(),
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
pub struct CreateUserReq {
    pub username: String,
    pub password: String,
}

async fn handle_create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserReq>,
) -> impl IntoResponse {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    
    let hash = match crypto::derive_key(payload.password.as_bytes(), &salt) {
        Ok(h) => h,
        Err(e) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError { status: "500".into(), title: "Crypto error".into(), detail: Some(e.to_string()) }]
            };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };
    
    let hash_str = format!("{}:{}", BASE64.encode(salt), BASE64.encode(hash));

    match sqlx::query("INSERT INTO admin_users (username, password_hash, is_active) VALUES ($1, $2, true)")
        .bind(&payload.username)
        .bind(hash_str)
        .execute(&state.repo.get().unwrap().pool)
        .await 
    {
        Ok(_) => StatusCode::CREATED.into_response(),
        Err(e) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError { status: "500".into(), title: "Database error".into(), detail: Some(e.to_string()) }]
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct UserIdQuery {
    pub id: i32,
}

async fn handle_delete_user(
    State(state): State<AppState>,
    Query(query): Query<UserIdQuery>,
) -> impl IntoResponse {
    match sqlx::query("DELETE FROM admin_users WHERE id = $1")
        .bind(query.id)
        .execute(&state.repo.get().unwrap().pool)
        .await 
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError { status: "500".into(), title: "Database error".into(), detail: Some(e.to_string()) }]
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct AssignRolesReq {
    pub user_id: i32,
    pub role_ids: Vec<i32>,
}

async fn handle_assign_roles(
    State(state): State<AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    Json(payload): Json<AssignRolesReq>,
) -> impl IntoResponse {
    let roles_json = match serde_json::to_vec(&payload.role_ids) {
        Ok(v) => v,
        Err(e) => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "JSON error".into(), detail: Some(e.to_string()) }] };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };
    let socket_dir = std::path::PathBuf::from(&state.config.socket_dir);
    let socket_path = socket_dir.join("mitm_scheduler.sock");

    let active_dek_row = match sqlx::query("SELECT wrapped_key FROM storage_keys WHERE is_active = true LIMIT 1")
        .fetch_optional(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(Some(row)) => {
            use sqlx::Row;
            row.get::<Vec<u8>, _>("wrapped_key")
        },
        Ok(None) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "No active key".into(), detail: None }] })).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }] })).into_response(),
    };

    let wrapped_dek = active_dek_row.clone();
    let (nonce, ciphertext) = match crate::ipc_client::crypto_encrypt(active_dek_row, roles_json, &socket_path).await {
        Ok(res) => res,
        Err(e) => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "IPC Crypto error".into(), detail: Some(e.to_string()) }] };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };

    match sqlx::query(
        "INSERT INTO user_roles_encrypted (user_id, wrapped_dek, nonce, encrypted_roles) 
         VALUES ($1, $2, $3, $4) 
         ON CONFLICT (user_id) DO UPDATE SET 
         wrapped_dek = EXCLUDED.wrapped_dek, 
         nonce = EXCLUDED.nonce, 
         encrypted_roles = EXCLUDED.encrypted_roles, 
         updated_at = CURRENT_TIMESTAMP"
    )
    .bind(payload.user_id)
    .bind(wrapped_dek)
    .bind(nonce)
    .bind(ciphertext)
    .execute(&state.repo.get().unwrap().pool)
    .await {
        Ok(_) => {
            let _ = sqlx::query("INSERT INTO admin_audit_logs (username, action, details) VALUES ($1, $2, $3)")
                .bind(&auth.username)
                .bind("assign_roles")
                .bind(serde_json::json!({"user_id": payload.user_id, "roles": payload.role_ids}))
                .execute(&state.repo.get().unwrap().pool)
                .await;
            StatusCode::OK.into_response()
        },
        Err(e) => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "DB error".into(), detail: Some(e.to_string()) }] };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct GetUserRolesQuery {
    pub user_id: i32,
}

async fn handle_get_user_roles(
    State(state): State<AppState>,
    Query(query): Query<GetUserRolesQuery>,
) -> impl IntoResponse {
    let row: Option<(Vec<u8>, Vec<u8>, Vec<u8>)> = match sqlx::query_as("SELECT wrapped_dek, nonce, encrypted_roles FROM user_roles_encrypted WHERE user_id = $1")
        .bind(query.user_id)
        .fetch_optional(&state.repo.get().unwrap().pool)
        .await {
            Ok(r) => r,
            Err(_) => return (StatusCode::OK, Json(Vec::<i32>::new())).into_response(),
        };

    let (wrapped_dek, nonce, encrypted_roles) = match row {
        Some(r) => r,
        None => return (StatusCode::OK, Json(Vec::<i32>::new())).into_response(),
    };

    let socket_dir = std::path::PathBuf::from(&state.config.socket_dir);
    let socket_path = socket_dir.join("mitm_scheduler.sock");

    let plaintext = match crate::ipc_client::crypto_decrypt(wrapped_dek, nonce, encrypted_roles, &socket_path).await {
        Ok(p) => p,
        Err(_) => return (StatusCode::OK, axum::Json(Vec::<i32>::new())).into_response(),
    };

    let roles: Vec<i32> = serde_json::from_slice(&plaintext).unwrap_or_default();
    (StatusCode::OK, Json(roles)).into_response()
}

#[derive(Deserialize)]
pub struct OsUserQuery {
    pub os_user: String,
}

async fn handle_get_os_user_roles(
    State(state): State<AppState>,
    Query(query): Query<OsUserQuery>,
) -> impl IntoResponse {
    for admin in &state.config.admins {
        if admin.username == query.os_user {
            return (StatusCode::OK, Json(vec!["ADMIN".to_string()])).into_response();
        }
    }

    let record: Option<(i32,)> = match sqlx::query_as("SELECT id FROM admin_users WHERE username = $1 AND is_active = true")
        .bind(&query.os_user)
        .fetch_optional(&state.repo.get().unwrap().pool)
        .await {
            Ok(r) => r,
            Err(_) => return (StatusCode::OK, Json(Vec::<String>::new())).into_response(),
        };

    let user_id = match record {
        Some(r) => r.0,
        None => return (StatusCode::OK, Json(Vec::<String>::new())).into_response(),
    };

    let row: Option<(Vec<u8>, Vec<u8>, Vec<u8>)> = match sqlx::query_as("SELECT wrapped_dek, nonce, encrypted_roles FROM user_roles_encrypted WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&state.repo.get().unwrap().pool)
        .await {
            Ok(r) => r,
            Err(_) => return (StatusCode::OK, Json(Vec::<String>::new())).into_response(),
        };

    let (wrapped_dek, nonce, encrypted_roles) = match row {
        Some(r) => r,
        None => return (StatusCode::OK, Json(Vec::<String>::new())).into_response(),
    };

    let socket_dir = std::path::PathBuf::from(&state.config.socket_dir);
    let socket_path = socket_dir.join("mitm_scheduler.sock");

    let plaintext = match crate::ipc_client::crypto_decrypt(wrapped_dek, nonce, encrypted_roles, &socket_path).await {
        Ok(p) => p,
        Err(_) => return (StatusCode::OK, axum::Json(Vec::<String>::new())).into_response(),
    };

    let role_ids: Vec<i32> = serde_json::from_slice(&plaintext).unwrap_or_default();

    if role_ids.is_empty() {
        return (StatusCode::OK, Json(Vec::<String>::new())).into_response();
    }

    let mut role_names = Vec::new();
    if let Ok(rows) = sqlx::query("SELECT name FROM roles WHERE id = ANY($1)")
        .bind(&role_ids)
        .fetch_all(&state.repo.get().unwrap().pool)
        .await {
            for row in rows {
                use sqlx::Row;
                if let Ok(name) = row.try_get::<String, _>(0) {
                    role_names.push(name);
                }
            }
        }

    (StatusCode::OK, Json(role_names)).into_response()
}
