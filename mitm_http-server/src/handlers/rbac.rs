/*
 * SPDX-License-Identifier: Apache-2.0
 */

use axum::{
    extract::{State, Path},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::handlers::{AppState, response::{ApiError, ApiResponse}};
use mitm_common::crypto;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::{rngs::OsRng, RngCore};



#[derive(Serialize, sqlx::FromRow)]
pub struct Role {
    pub id: i32,
    pub name: String,
}

pub async fn handle_get_roles(State(state): State<AppState>) -> Response {
    match sqlx::query_as::<_, Role>("SELECT id, name FROM roles ORDER BY id ASC")
        .fetch_all(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(roles) => {
            let count = roles.len();
            ApiResponse::ok(roles).with_count(count).into_response()
        }
        Err(e) => ApiError::internal_logged("Failed to load roles", e).into_response(),
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub department: Option<String>,
    pub email: Option<String>,
    pub is_active: bool,
}

const USER_COLUMNS: &str = "id, username, first_name, last_name, department, email, is_active";

pub async fn handle_get_users(State(state): State<AppState>) -> Response {
    match sqlx::query_as::<_, User>(&format!("SELECT {USER_COLUMNS} FROM admin_users ORDER BY id ASC"))
        .fetch_all(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(users) => {
            let count = users.len();
            ApiResponse::ok(users).with_count(count).into_response()
        }
        Err(e) => ApiError::internal_logged("Failed to load users", e).into_response(),
    }
}

#[derive(Deserialize)]
pub struct CreateUserReq {
    pub username: String,
    pub password: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub department: Option<String>,
    pub email: Option<String>,
    pub is_active: Option<bool>,
}

/// Trimmed e-mail address; an empty value is stored as `NULL`.
fn email_value(email: &str) -> Option<&str> {
    Some(email.trim()).filter(|e| !e.is_empty())
}

pub async fn handle_create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserReq>,
) -> Response {
    let Some(username) = mitm_common::username::normalize(&payload.username) else {
        return ApiError::validation_failed("Username must match ^[A-Z0-9._-]{1,64}$ (case-insensitive).").into_response();
    };

    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    
    let hash = match crypto::derive_key(payload.password.as_bytes(), &salt) {
        Ok(h) => h,
        Err(e) => return ApiError::internal_logged("Failed to hash password", e).into_response(),
    };
    
    let hash_str = format!("{}:{}", BASE64.encode(salt), BASE64.encode(hash));
    let is_active = payload.is_active.unwrap_or(false);

    match sqlx::query_scalar::<_, i32>("INSERT INTO admin_users (username, password_hash, first_name, last_name, department, email, is_active) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id")
        .bind(&username)
        .bind(hash_str)
        .bind(&payload.first_name)
        .bind(&payload.last_name)
        .bind(&payload.department)
        .bind(payload.email.as_deref().and_then(email_value))
        .bind(is_active)
        .fetch_one(&state.repo.get().unwrap().pool)
        .await 
    {
        Ok(id) => ApiResponse::created_at(&format!("/api/v1/iam/users/{}", id), serde_json::json!({ "id": id })),
        Err(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()) => {
            ApiError::conflict(format!("User {} already exists.", username)).into_response()
        }
        Err(e) => ApiError::internal_logged("Failed to create user", e).into_response(),
    }
}

#[derive(Deserialize)]
pub struct EditUserReq {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub department: Option<String>,
    pub email: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn handle_edit_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<EditUserReq>,
) -> Response {
    let mut update_query = "UPDATE admin_users SET ".to_string();
    let mut param_idx = 1;
    let mut binds_str: Vec<String> = vec![];
    
    if payload.first_name.is_some() || payload.last_name.is_some() || payload.department.is_some() || payload.email.is_some() || payload.is_active.is_some() {
        if payload.first_name.is_some() { binds_str.push(format!("first_name = ${}", param_idx)); param_idx += 1; }
        if payload.last_name.is_some() { binds_str.push(format!("last_name = ${}", param_idx)); param_idx += 1; }
        if payload.department.is_some() { binds_str.push(format!("department = ${}", param_idx)); param_idx += 1; }
        if payload.email.is_some() { binds_str.push(format!("email = ${}", param_idx)); param_idx += 1; }
        if payload.is_active.is_some() { binds_str.push(format!("is_active = ${}", param_idx)); param_idx += 1; }
        
        update_query.push_str(&binds_str.join(", "));
        update_query.push_str(&format!(" WHERE id = ${}", param_idx));

        let mut q = sqlx::query(&update_query);
        if let Some(ref f) = payload.first_name { q = q.bind(f); }
        if let Some(ref l) = payload.last_name { q = q.bind(l); }
        if let Some(ref d) = payload.department { q = q.bind(d); }
        if let Some(ref e) = payload.email { q = q.bind(email_value(e)); }
        if let Some(a) = payload.is_active { q = q.bind(a); }
        q = q.bind(id);

        match q.execute(&state.repo.get().unwrap().pool).await {
            Ok(r) if r.rows_affected() == 0 => ApiError::not_found(format!("User {} not found.", id)).into_response(),
            Ok(_) => ApiResponse::updated(format!("User {} updated", id), None).into_response(),
            Err(e) => ApiError::internal_logged("Failed to update user", e).into_response(),
        }
    } else {
        ApiResponse::updated(format!("User {} unchanged", id), None).into_response()
    }
}

pub async fn handle_terminate_session(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Response {
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
            Ok(_) => ApiResponse::deleted(format!("Sessions of user {} terminated", id)).into_response(),
            Err(e) => ApiError::internal_logged("Failed to terminate sessions", e).into_response(),
        }
    } else {
        ApiError::not_found(format!("User {} not found.", id)).into_response()
    }
}

pub async fn handle_delete_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Response {
    match sqlx::query("DELETE FROM admin_users WHERE id = $1")
        .bind(id)
        .execute(&state.repo.get().unwrap().pool)
        .await 
    {
        Ok(r) if r.rows_affected() == 0 => ApiError::not_found(format!("User {} not found.", id)).into_response(),
        Ok(_) => ApiResponse::deleted(format!("User {} deleted", id)).into_response(),
        Err(e) => ApiError::internal_logged("Failed to delete user", e).into_response(),
    }
}

/// Body of `PUT /api/v1/iam/users/:id/roles`.
#[derive(Deserialize)]
pub struct SetRolesReq {
    pub role_ids: Vec<i32>,
}

/// `PUT /api/v1/iam/users/:id/roles`: replaces the user's role assignment.
pub async fn handle_set_user_roles(
    State(state): State<AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    Path(user_id): Path<i32>,
    Json(payload): Json<SetRolesReq>,
) -> Response {
    match sqlx::query_scalar::<_, i32>("SELECT id FROM admin_users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => return ApiError::not_found(format!("User {} not found.", user_id)).into_response(),
        Err(e) => return ApiError::internal_logged("Failed to load user", e).into_response(),
    }
    let roles_json = match serde_json::to_vec(&payload.role_ids) {
        Ok(v) => v,
        Err(e) => return ApiError::internal_logged("Failed to serialize roles", e).into_response(),
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
        Err(e) => return ApiError::internal_logged("Failed to load storage key", e).into_response(),
    };

    let wrapped_dek = match active_dek_row {
        Some(dek) => dek,
        None => {
            match crate::ipc_client::crypto_generate_wrapped_dek(&socket_path).await {
                Ok(dek) => dek,
                Err(e) => return ApiError::internal_logged("Failed to generate wrapped DEK", e).into_response(),
            }
        }
    };

    let (nonce, ciphertext) = match crate::ipc_client::crypto_encrypt(wrapped_dek.clone(), roles_json, &socket_path).await {
        Ok(res) => res,
        Err(e) => return ApiError::internal_logged("Failed to encrypt roles", e).into_response(),
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
    .bind(user_id)
    .bind(wrapped_dek)
    .bind(nonce)
    .bind(ciphertext)
    .execute(&state.repo.get().unwrap().pool)
    .await {
        Ok(_) => {
            state.repo.get().unwrap().audit(&auth.username, crate::audit::action::ASSIGN_ROLES, serde_json::json!({"user_id": user_id, "roles": payload.role_ids})).await;
            ApiResponse::updated(format!("Roles of user {} updated", user_id), Some(serde_json::json!({ "role_ids": payload.role_ids }))).into_response()
        },
        Err(e) => ApiError::internal_logged("Failed to store roles", e).into_response(),
    }
}

#[derive(Serialize)]
pub struct UserDetail {
    #[serde(flatten)]
    pub user: User,
    pub role_ids: Vec<i32>,
}

pub async fn handle_get_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Response {
    let user = match sqlx::query_as::<_, User>(&format!("SELECT {USER_COLUMNS} FROM admin_users WHERE id = $1"))
        .bind(id)
        .fetch_optional(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(Some(u)) => u,
        Ok(None) => return ApiError::not_found(format!("User {} not found.", id)).into_response(),
        Err(e) => return ApiError::internal_logged("Failed to load user", e).into_response(),
    };

    let role_ids = load_role_ids(&state, id).await;
    ApiResponse::ok(UserDetail { user, role_ids }).into_response()
}

/// Decrypts the user's role IDs; an unreadable or missing assignment yields no roles.
async fn load_role_ids(state: &AppState, user_id: i32) -> Vec<i32> {
    let row: Option<(Vec<u8>, Vec<u8>, Vec<u8>)> = sqlx::query_as("SELECT wrapped_dek, nonce, encrypted_roles FROM user_roles_encrypted WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&state.repo.get().unwrap().pool)
        .await
        .unwrap_or(None);

    let Some((wrapped_dek, nonce, encrypted_roles)) = row else {
        return Vec::new();
    };

    let socket_path = std::path::PathBuf::from(&state.config.socket_dir).join("mitm_scheduler.sock");
    match crate::ipc_client::crypto_decrypt(wrapped_dek, nonce, encrypted_roles, &socket_path).await {
        Ok(plaintext) => serde_json::from_slice(&plaintext).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}




