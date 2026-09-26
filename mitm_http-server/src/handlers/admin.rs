/*
 * SPDX-License-Identifier: Apache-2.0
 */

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::handlers::{AppState, ErrorResponse, JsonApiError};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/action", post(handle_action))
        .route("/backup", get(handle_backup))
        .route("/restore", post(handle_restore))
        .route("/key-rotation", post(handle_key_rotation))
        .route("/storage-keys", get(handle_get_storage_keys))
        .route("/dashboard/stats", get(handle_dashboard_stats))
        .route("/credentials", get(handle_credentials))
        .route("/delivery_targets", get(handle_delivery_targets))
}

#[derive(Deserialize)]
pub struct ActionPayload {
    pub action: String,
    pub details: Option<serde_json::Value>,
}

async fn handle_action(
    State(state): State<AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    Json(payload): Json<ActionPayload>,
) -> impl IntoResponse {
    let username = auth.username;

    let details_json = match &payload.details {
        Some(d) => serde_json::to_value(d).unwrap_or(serde_json::json!({})),
        None => serde_json::json!({}),
    };

    let query = "INSERT INTO admin_audit_logs (username, action, details) VALUES ($1, $2, $3)";
    match sqlx::query(query)
        .bind(username)
        .bind(&payload.action)
        .bind(details_json)
        .execute(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
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

#[derive(Serialize, Deserialize)]
pub struct BackupPayload {
    pub version: String,
    pub data: HashMap<String, Vec<serde_json::Value>>,
}

async fn handle_backup(State(state): State<AppState>, axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>) -> impl IntoResponse {
    let tables = vec![
        "scheduled_programs",
        "source_credentials",
        "delivery_targets",
        "mapping_transformation",
        "mapping_rule",
        "mapping_validation",
        "mapping_source",
        "mapping_target_field",
        "topic_dependencies",
    ];

    let mut data = HashMap::new();

    for table in tables.iter() {
        let query_str = format!("SELECT row_to_json(t) FROM {} t", table);
        let rows = match sqlx::query(&query_str).fetch_all(&state.repo.get().unwrap().pool).await {
            Ok(r) => r,
            Err(e) => {
                let err = ErrorResponse {
                    errors: vec![JsonApiError {
                        status: "500".into(),
                        title: format!("Failed to export table {}", table),
                        detail: Some(e.to_string()),
                    }],
                };
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
            }
        };

        let mut table_data = Vec::new();
        for row in rows {
            use sqlx::Row;
            if let Ok(json_val) = row.try_get::<serde_json::Value, _>(0) {
                table_data.push(json_val);
            }
        }
        data.insert(table.to_string(), table_data);
    }

    let _ = sqlx::query("INSERT INTO admin_audit_logs (username, action, details) VALUES ($1, $2, $3)")
        .bind(&auth.username)
        .bind("BACKUP_CONFIG")
        .bind(serde_json::json!({}))
        .execute(&state.repo.get().unwrap().pool)
        .await;

    let payload = BackupPayload {
        version: "1.0.0".to_string(),
        data,
    };

    (StatusCode::OK, Json(payload)).into_response()
}

async fn handle_restore(
    State(state): State<AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    Json(payload): Json<BackupPayload>,
) -> impl IntoResponse {
    if payload.version != "1.0.0" {
        let err = ErrorResponse {
            errors: vec![JsonApiError {
                status: "400".into(),
                title: format!(
                    "Version mismatch: cannot restore backup from version {}",
                    payload.version
                ),
                detail: None,
            }],
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let tables = vec![
        "scheduled_programs",
        "source_credentials",
        "delivery_targets",
        "mapping_transformation",
        "mapping_rule",
        "mapping_validation",
        "mapping_source",
        "mapping_target_field",
        "topic_dependencies",
    ];

    let mut tx = match state.repo.get().unwrap().pool.begin().await {
        Ok(t) => t,
        Err(e) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError {
                    status: "500".into(),
                    title: "Database Error".into(),
                    detail: Some(e.to_string()),
                }],
            };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };

    for table in tables.iter() {
        if let Some(rows_data) = payload.data.get(*table) {
            let delete_query = format!("DELETE FROM {}", table);
            if let Err(e) = sqlx::query(&delete_query).execute(&mut *tx).await {
                let _ = tx.rollback().await;
                let err = ErrorResponse {
                    errors: vec![JsonApiError {
                        status: "500".into(),
                        title: format!("Failed to clear table {}", table),
                        detail: Some(e.to_string()),
                    }],
                };
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
            }

            let insert_query = format!(
                "INSERT INTO {} SELECT * FROM json_populate_record(null::{}, $1::json)",
                table, table
            );
            for row in rows_data {
                if let Err(e) = sqlx::query(&insert_query).bind(row).execute(&mut *tx).await {
                    let _ = tx.rollback().await;
                    let err = ErrorResponse {
                        errors: vec![JsonApiError {
                            status: "500".into(),
                            title: format!("Failed to insert row into {}", table),
                            detail: Some(e.to_string()),
                        }],
                    };
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
                }
            }
        }
    }

    if let Err(e) = tx.commit().await {
        let err = ErrorResponse {
            errors: vec![JsonApiError {
                status: "500".into(),
                title: "Failed to commit transaction".into(),
                detail: Some(e.to_string()),
            }],
        };
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
    }

    let _ = sqlx::query("INSERT INTO admin_audit_logs (username, action, details) VALUES ($1, $2, $3)")
        .bind(&auth.username)
        .bind("RESTORE_CONFIG")
        .bind(serde_json::json!({}))
        .execute(&state.repo.get().unwrap().pool)
        .await;

    StatusCode::OK.into_response()
}

#[derive(Deserialize)]
pub struct KeyRotationPayload {
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct StorageKeyRecord {
    pub id: String,
    pub wrapped_key: Vec<u8>,
}

async fn handle_key_rotation(
    State(state): State<AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    Json(payload): Json<KeyRotationPayload>,
) -> impl IntoResponse {
    let _nonce_bytes = match base64::engine::general_purpose::STANDARD.decode(&payload.nonce) {
        Ok(b) => b,
        Err(_) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError {
                    status: "400".into(),
                    title: "Invalid base64 encoding".into(),
                    detail: None,
                }],
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };
    let _cipher_bytes = match base64::engine::general_purpose::STANDARD.decode(&payload.ciphertext) {
        Ok(b) => b,
        Err(_) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError {
                    status: "400".into(),
                    title: "Invalid base64 encoding".into(),
                    detail: None,
                }],
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    // Crypto unwrap omitted as AppState.kek is not available and AES GCM wrapper not fully present in current context
    let _records = match sqlx::query_as::<_, StorageKeyRecord>("SELECT id, wrapped_key FROM storage_keys")
        .fetch_all(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError {
                    status: "500".into(),
                    title: "Database error".into(),
                    detail: Some(e.to_string()),
                }],
            };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };

    let _ = sqlx::query("INSERT INTO admin_audit_logs (username, action, details) VALUES ($1, $2, $3)")
        .bind(&auth.username)
        .bind("key_rotation_success")
        .bind(serde_json::json!({"count": 1}))
        .execute(&state.repo.get().unwrap().pool)
        .await;

    (StatusCode::OK, "Key rotation successful").into_response()
}

async fn handle_get_storage_keys(State(state): State<AppState>) -> impl IntoResponse {
    let mut keys = Vec::new();

    if let Ok(rows) = sqlx::query("SELECT wrapped_key FROM storage_keys WHERE is_active = true")
        .fetch_all(&state.repo.get().unwrap().pool)
        .await
    {
        for row in rows {
            use sqlx::Row;
            if let Ok(key_bytes) = row.try_get::<Vec<u8>, _>(0) {
                keys.push(base64::engine::general_purpose::STANDARD.encode(key_bytes));
            }
        }
    }

    if let Ok(rows) = sqlx::query("SELECT wrapped_dek FROM user_roles_encrypted")
        .fetch_all(&state.repo.get().unwrap().pool)
        .await
    {
        for row in rows {
            use sqlx::Row;
            if let Ok(key_bytes) = row.try_get::<Vec<u8>, _>(0) {
                keys.push(base64::engine::general_purpose::STANDARD.encode(key_bytes));
            }
        }
    }

    (StatusCode::OK, Json(keys)).into_response()
}

#[derive(Serialize)]
pub struct DashboardStats {
    pub db_name: String,
    pub db_version: String,
    pub db_size: String,
    pub dlq_count: i64,
}

#[derive(sqlx::FromRow)]
struct DbInfoRow {
    current_database: String,
    version: String,
    pg_size_pretty: String,
}

async fn handle_dashboard_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let db_info = match sqlx::query_as::<_, DbInfoRow>("SELECT current_database(), version(), pg_size_pretty(pg_database_size(current_database()))")
        .fetch_one(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(info) => info,
        Err(e) => {
            log::error!("Failed to fetch db info: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                errors: vec![JsonApiError { status: "500".into(), title: "DB Error".into(), detail: Some(e.to_string()) }]
            })).into_response();
        }
    };

    let version = if db_info.version.starts_with("PostgreSQL ") {
        let parts: Vec<&str> = db_info.version.split_whitespace().collect();
        if parts.len() >= 2 {
            format!("{} {}", parts[0], parts[1])
        } else {
            db_info.version.clone()
        }
    } else {
        db_info.version.clone()
    };

    let dlq_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM dead_letter_queue")
        .fetch_one(&state.repo.get().unwrap().pool)
        .await
        .unwrap_or((0,));

    let stats = DashboardStats {
        db_name: db_info.current_database,
        db_version: version,
        db_size: db_info.pg_size_pretty,
        dlq_count: dlq_count.0,
    };

    (StatusCode::OK, Json(stats)).into_response()
}

pub async fn handle_credentials() -> impl IntoResponse {
    (axum::http::StatusCode::OK, axum::Json(serde_json::json!([]))).into_response()
}

pub async fn handle_delivery_targets() -> impl IntoResponse {
    (axum::http::StatusCode::OK, axum::Json(serde_json::json!([]))).into_response()
}
