/*
 * SPDX-License-Identifier: Apache-2.0
 */

use axum::{
    extract::{State, Query, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::handlers::{AppState, ErrorResponse, JsonApiError};
use mitm_common::crypto;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::{rngs::OsRng, RngCore};



#[derive(Serialize, sqlx::FromRow)]
pub struct Role {
    pub id: i32,
    pub name: String,
}

pub async fn handle_get_roles(State(state): State<AppState>) -> impl IntoResponse {
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
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_active: bool,
}

pub async fn handle_get_users(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_as::<_, User>("SELECT id, username, first_name, last_name, is_active FROM admin_users ORDER BY id ASC")
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
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn handle_create_user(
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
    let is_active = payload.is_active.unwrap_or(true);

    match sqlx::query("INSERT INTO admin_users (username, password_hash, first_name, last_name, is_active) VALUES ($1, $2, $3, $4, $5)")
        .bind(&payload.username)
        .bind(hash_str)
        .bind(&payload.first_name)
        .bind(&payload.last_name)
        .bind(is_active)
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
pub struct EditUserReq {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn handle_edit_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<EditUserReq>,
) -> impl IntoResponse {
    let mut update_query = "UPDATE admin_users SET ".to_string();
    let mut param_idx = 1;
    let mut binds_str: Vec<String> = vec![];
    
    if payload.first_name.is_some() || payload.last_name.is_some() || payload.is_active.is_some() {
        if payload.first_name.is_some() { binds_str.push(format!("first_name = ${}", param_idx)); param_idx += 1; }
        if payload.last_name.is_some() { binds_str.push(format!("last_name = ${}", param_idx)); param_idx += 1; }
        if payload.is_active.is_some() { binds_str.push(format!("is_active = ${}", param_idx)); param_idx += 1; }
        
        update_query.push_str(&binds_str.join(", "));
        update_query.push_str(&format!(" WHERE id = ${}", param_idx));

        let mut q = sqlx::query(&update_query);
        if let Some(ref f) = payload.first_name { q = q.bind(f); }
        if let Some(ref l) = payload.last_name { q = q.bind(l); }
        if let Some(a) = payload.is_active { q = q.bind(a); }
        q = q.bind(id);

        match q.execute(&state.repo.get().unwrap().pool).await {
            Ok(_) => StatusCode::OK.into_response(),
            Err(e) => {
                let err = ErrorResponse {
                    errors: vec![JsonApiError { status: "500".into(), title: "Database error".into(), detail: Some(e.to_string()) }]
                };
                (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
            }
        }
    } else {
        StatusCode::OK.into_response()
    }
}

pub async fn handle_terminate_session(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    // Delete session for user id. We need the username first.
    let user_res: Result<Option<(String,)>, _> = sqlx::query_as("SELECT username FROM admin_users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.repo.get().unwrap().pool)
        .await;

    if let Ok(Some((username,))) = user_res {
        match sqlx::query("DELETE FROM user_sessions WHERE os_user = $1")
            .bind(username)
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
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[derive(Deserialize)]
pub struct UserIdQuery {
    pub id: i32,
}

pub async fn handle_delete_user(
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

pub async fn handle_assign_roles(
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
            Some(row.get::<Vec<u8>, _>("wrapped_key"))
        },
        Ok(None) => None,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }] })).into_response(),
    };

    let wrapped_dek = match active_dek_row {
        Some(dek) => dek,
        None => {
            match crate::ipc_client::crypto_generate_wrapped_dek(&socket_path).await {
                Ok(dek) => dek,
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Failed to generate wrapped DEK".into(), detail: Some(e.to_string()) }] })).into_response(),
            }
        }
    };

    let (nonce, ciphertext) = match crate::ipc_client::crypto_encrypt(wrapped_dek.clone(), roles_json, &socket_path).await {
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

pub async fn handle_get_user_roles(
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




