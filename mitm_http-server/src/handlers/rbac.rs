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
        .fetch_all(&state.repo.pool)
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
        .fetch_all(&state.repo.pool)
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
        .execute(&state.repo.pool)
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
        .execute(&state.repo.pool)
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
    Json(payload): Json<AssignRolesReq>,
) -> impl IntoResponse {
    let mut tx = match state.repo.pool.begin().await {
        Ok(t) => t,
        Err(e) => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "DB error".into(), detail: Some(e.to_string()) }] };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };

    if let Err(e) = sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
        .bind(payload.user_id)
        .execute(&mut *tx).await 
    {
        let _ = tx.rollback().await;
        let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "DB error".into(), detail: Some(e.to_string()) }] };
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
    }

    for role_id in payload.role_ids {
        if let Err(e) = sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)")
            .bind(payload.user_id)
            .bind(role_id)
            .execute(&mut *tx).await 
        {
            let _ = tx.rollback().await;
            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "DB error".into(), detail: Some(e.to_string()) }] };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    }

    let _ = tx.commit().await;
    StatusCode::OK.into_response()
}

#[derive(Deserialize)]
pub struct GetUserRolesQuery {
    pub user_id: i32,
}

async fn handle_get_user_roles(
    State(state): State<AppState>,
    Query(query): Query<GetUserRolesQuery>,
) -> impl IntoResponse {
    let mut roles = Vec::new();
    if let Ok(rows) = sqlx::query("SELECT role_id FROM user_roles WHERE user_id = $1")
        .bind(query.user_id)
        .fetch_all(&state.repo.pool)
        .await
    {
        for row in rows {
            use sqlx::Row;
            if let Ok(role_id) = row.try_get::<i32, _>(0) {
                roles.push(role_id);
            }
        }
    }
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
    let mut roles = Vec::new();
    let sql = "SELECT r.name FROM roles r JOIN user_roles ur ON r.id = ur.role_id JOIN admin_users u ON ur.user_id = u.id WHERE u.username = $1 AND u.is_active = true";
    if let Ok(rows) = sqlx::query(sql)
        .bind(&query.os_user)
        .fetch_all(&state.repo.pool)
        .await
    {
        for row in rows {
            use sqlx::Row;
            if let Ok(role_name) = row.try_get::<String, _>(0) {
                roles.push(role_name);
            }
        }
    }
    (StatusCode::OK, Json(roles)).into_response()
}
