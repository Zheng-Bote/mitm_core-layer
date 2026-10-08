/*
 * SPDX-License-Identifier: Apache-2.0
 */

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::handlers::{AppState, ErrorResponse, JsonApiError};







#[derive(Serialize, Deserialize)]
pub struct BackupPayload {
    pub version: String,
    pub data: HashMap<String, Vec<serde_json::Value>>,
}

pub async fn handle_backup(State(state): State<AppState>, axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>) -> impl IntoResponse {
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
        version: env!("CARGO_PKG_VERSION").to_string(),
        data,
    };

    (StatusCode::OK, Json(payload)).into_response()
}

pub async fn handle_restore(
    State(state): State<AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    Json(payload): Json<BackupPayload>,
) -> impl IntoResponse {
    if payload.version != env!("CARGO_PKG_VERSION") {
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

pub async fn handle_key_rotation(
    State(state): State<AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    Json(payload): Json<KeyRotationPayload>,
) -> impl IntoResponse {
    let nonce_bytes = match base64::engine::general_purpose::STANDARD.decode(&payload.nonce) {
        Ok(b) => b,
        Err(_) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError {
                    status: "400".into(),
                    title: "Invalid base64 encoding for nonce".into(),
                    detail: None,
                }],
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };
    let cipher_bytes = match base64::engine::general_purpose::STANDARD.decode(&payload.ciphertext) {
        Ok(b) => b,
        Err(_) => {
            let err = ErrorResponse {
                errors: vec![JsonApiError {
                    status: "400".into(),
                    title: "Invalid base64 encoding for ciphertext".into(),
                    detail: None,
                }],
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    // Combine nonce and ciphertext if the scheduler/iam expects them together,
    // or just insert the ciphertext. The previous `handle_get_storage_keys`
    // returns `wrapped_key` directly. In our Postgres schema:
    // `wrapped_key BYTEA NOT NULL`
    // Let's store nonce + ciphertext in wrapped_key for simplicity, or just
    // ciphertext if the frontend combined them? No, frontend sent them separate.
    // The previous code just did nothing.
    // Let's store nonce + ciphertext in wrapped_key.
    let mut wrapped_key = Vec::new();
    wrapped_key.extend_from_slice(&nonce_bytes);
    wrapped_key.extend_from_slice(&cipher_bytes);

    let mut tx = match state.repo.get().unwrap().pool.begin().await {
        Ok(t) => t,
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

    let deact = match sqlx::query("UPDATE storage_keys SET is_active = false WHERE is_active = true")
        .execute(&mut *tx)
        .await
    {
        Ok(res) => res.rows_affected(),
        Err(e) => {
            let _ = tx.rollback().await;
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

    if let Err(e) = sqlx::query("INSERT INTO storage_keys (wrapped_key, is_active) VALUES ($1, true)")
        .bind(&wrapped_key)
        .execute(&mut *tx)
        .await
    {
        let _ = tx.rollback().await;
        let err = ErrorResponse {
            errors: vec![JsonApiError {
                status: "500".into(),
                title: "Database error".into(),
                detail: Some(e.to_string()),
            }],
        };
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
    }

    if let Err(e) = tx.commit().await {
        let err = ErrorResponse {
            errors: vec![JsonApiError {
                status: "500".into(),
                title: "Database error".into(),
                detail: Some(e.to_string()),
            }],
        };
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
    }

    let _ = sqlx::query("INSERT INTO admin_audit_logs (username, action, details) VALUES ($1, $2, $3)")
        .bind(&auth.username)
        .bind("key_rotation_success")
        .bind(serde_json::json!({"deactivated_count": deact}))
        .execute(&state.repo.get().unwrap().pool)
        .await;

    (StatusCode::OK, "Key rotation successful").into_response()
}

pub async fn handle_get_storage_keys(State(state): State<AppState>) -> impl IntoResponse {
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




#[derive(serde::Serialize, serde::Deserialize)]
pub struct SourceCredentialResponse {
    pub id: Option<String>,
    pub source_name: String,
    pub connector_type: String,
    pub topic: String,
    pub config_payload: String,
    #[serde(default)]
    pub is_active: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DeliveryTargetResponse {
    pub id: Option<String>,
    pub topic: String,
    pub adapter_type: String,
    pub endpoint_url: String,
    pub config_payload: String,
    #[serde(default)]
    pub is_active: bool,
}

#[derive(sqlx::FromRow)]
struct SourceCredentialRow {
    pub id: uuid::Uuid,
    pub source_name: String,
    pub connector_type: String,
    pub topic: String,
    pub config_payload: Vec<u8>,
    pub nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
    pub is_active: Option<bool>,
}

#[derive(sqlx::FromRow)]
struct DeliveryTargetRow {
    pub id: uuid::Uuid,
    pub topic: String,
    pub adapter_type: String,
    pub endpoint_url: String,
    pub config_payload: Vec<u8>,
    pub nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
    pub is_active: Option<bool>,
}

pub async fn handle_credentials(State(state): axum::extract::State<crate::handlers::AppState>) -> impl axum::response::IntoResponse {
    let query = "
        SELECT c.id, c.source_name, c.connector_type, c.topic, c.config_payload, c.nonce, k.wrapped_key, c.is_active
        FROM source_credentials c
        JOIN storage_keys k ON c.dek_id = k.id
    ";
    let rows = match sqlx::query_as::<_, SourceCredentialRow>(query).fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(r) => r,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };
    
    let socket_path = std::path::PathBuf::from(&state.config.socket_dir).join("mitm_scheduler.sock");
    let mut results = Vec::new();
    for row in rows {
        let plaintext = match crate::ipc_client::crypto_decrypt(row.wrapped_key, row.nonce, row.config_payload, &socket_path).await {
            Ok(pt) => pt,
            Err(_) => Vec::new(),
        };
        let config_str = String::from_utf8(plaintext).unwrap_or_default();
        results.push(SourceCredentialResponse {
            id: Some(row.id.to_string()),
            source_name: row.source_name,
            connector_type: row.connector_type,
            topic: row.topic,
            config_payload: config_str,
            is_active: row.is_active.unwrap_or(true),
        });
    }

    (axum::http::StatusCode::OK, axum::Json(results)).into_response()
}

pub async fn handle_delivery_targets(State(state): axum::extract::State<crate::handlers::AppState>) -> impl axum::response::IntoResponse {
    let query = "
        SELECT d.id, d.topic, d.adapter_type, d.endpoint_url, d.config_payload, d.nonce, k.wrapped_key, d.is_active
        FROM delivery_targets d
        JOIN storage_keys k ON d.dek_id = k.id
    ";
    let rows = match sqlx::query_as::<_, DeliveryTargetRow>(query).fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(r) => r,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };
    
    let socket_path = std::path::PathBuf::from(&state.config.socket_dir).join("mitm_scheduler.sock");
    let mut results = Vec::new();
    for row in rows {
        let plaintext = match crate::ipc_client::crypto_decrypt(row.wrapped_key, row.nonce, row.config_payload, &socket_path).await {
            Ok(pt) => pt,
            Err(_) => Vec::new(),
        };
        let config_str = String::from_utf8(plaintext).unwrap_or_default();
        results.push(DeliveryTargetResponse {
            id: Some(row.id.to_string()),
            topic: row.topic,
            adapter_type: row.adapter_type,
            endpoint_url: row.endpoint_url,
            config_payload: config_str,
            is_active: row.is_active.unwrap_or(true),
        });
    }

    (axum::http::StatusCode::OK, axum::Json(results)).into_response()
}

pub async fn handle_post_credentials(
    State(state): axum::extract::State<crate::handlers::AppState>,
    axum::Json(payload): axum::Json<SourceCredentialResponse>
) -> impl axum::response::IntoResponse {
    let socket_path = std::path::PathBuf::from(&state.config.socket_dir).join("mitm_scheduler.sock");
    
    let key_row: Option<(uuid::Uuid, Vec<u8>)> = match sqlx::query_as("SELECT id, wrapped_key FROM storage_keys WHERE is_active = true LIMIT 1")
        .fetch_optional(&state.repo.get().unwrap().pool).await {
        Ok(r) => r,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };
    
    let (dek_id, wrapped_key) = match key_row {
        Some(k) => k,
        None => {
            match crate::ipc_client::crypto_generate_wrapped_dek(&socket_path).await {
                Ok(new_wrapped_dek) => {
                    let new_id: uuid::Uuid = match sqlx::query_scalar("INSERT INTO storage_keys (wrapped_key, is_active) VALUES ($1, true) RETURNING id")
                        .bind(&new_wrapped_dek)
                        .fetch_one(&state.repo.get().unwrap().pool).await {
                            Ok(id) => id,
                            Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": format!("DB Error: {}", e)}))).into_response(),
                        };
                    (new_id, new_wrapped_dek)
                },
                Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": format!("IPC Crypto Error: {}", e)}))).into_response(),
            }
        }
    };
 

    let (nonce, ciphertext) = match crate::ipc_client::crypto_encrypt(wrapped_key, payload.config_payload.into_bytes(), &socket_path).await {
        Ok(res) => res,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": e}))).into_response(),
    };
    
    if payload.id.is_none() || payload.id.as_ref().unwrap().is_empty() {
        let insert_query = "
            INSERT INTO source_credentials (source_name, connector_type, topic, config_payload, nonce, dek_id, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
        ";
        match sqlx::query(insert_query)
            .bind(&payload.source_name)
            .bind(&payload.connector_type)
            .bind(&payload.topic)
            .bind(ciphertext)
            .bind(nonce)
            .bind(dek_id)
            .bind(payload.is_active)
            .execute(&state.repo.get().unwrap().pool).await {
            Ok(_) => (axum::http::StatusCode::OK, axum::Json(serde_json::json!({"status": "created"}))).into_response(),
            Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": e.to_string()}))).into_response(),
        }
    } else {
        let id_uuid = match uuid::Uuid::parse_str(payload.id.as_ref().unwrap()) {
            Ok(u) => u,
            Err(_) => return (axum::http::StatusCode::BAD_REQUEST, axum::Json(serde_json::json!({"error": "Invalid UUID"}))).into_response(),
        };
        
        let update_query = "
            UPDATE source_credentials 
            SET source_name = $1, connector_type = $2, topic = $3, config_payload = $4, nonce = $5, dek_id = $6, is_active = $7
            WHERE id = $8
        ";
        match sqlx::query(update_query)
            .bind(&payload.source_name)
            .bind(&payload.connector_type)
            .bind(&payload.topic)
            .bind(ciphertext)
            .bind(nonce)
            .bind(dek_id)
            .bind(payload.is_active)
            .bind(id_uuid)
            .execute(&state.repo.get().unwrap().pool).await {
            Ok(_) => (axum::http::StatusCode::OK, axum::Json(serde_json::json!({"status": "updated"}))).into_response(),
            Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": e.to_string()}))).into_response(),
        }
    }
}

pub async fn handle_post_delivery_targets(
    State(state): axum::extract::State<crate::handlers::AppState>,
    axum::Json(payload): axum::Json<DeliveryTargetResponse>
) -> impl axum::response::IntoResponse {
    let socket_path = std::path::PathBuf::from(&state.config.socket_dir).join("mitm_scheduler.sock");
    
    let key_row: Option<(uuid::Uuid, Vec<u8>)> = match sqlx::query_as("SELECT id, wrapped_key FROM storage_keys WHERE is_active = true LIMIT 1")
        .fetch_optional(&state.repo.get().unwrap().pool).await {
        Ok(r) => r,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };
    
    let (dek_id, wrapped_key) = match key_row {
        Some(k) => k,
        None => {
            match crate::ipc_client::crypto_generate_wrapped_dek(&socket_path).await {
                Ok(new_wrapped_dek) => {
                    let new_id: uuid::Uuid = match sqlx::query_scalar("INSERT INTO storage_keys (wrapped_key, is_active) VALUES ($1, true) RETURNING id")
                        .bind(&new_wrapped_dek)
                        .fetch_one(&state.repo.get().unwrap().pool).await {
                            Ok(id) => id,
                            Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": format!("DB Error: {}", e)}))).into_response(),
                        };
                    (new_id, new_wrapped_dek)
                },
                Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": format!("IPC Crypto Error: {}", e)}))).into_response(),
            }
        }
    };
 

    let (nonce, ciphertext) = match crate::ipc_client::crypto_encrypt(wrapped_key, payload.config_payload.into_bytes(), &socket_path).await {
        Ok(res) => res,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": e}))).into_response(),
    };
    
    if payload.id.is_none() || payload.id.as_ref().unwrap().is_empty() {
        let insert_query = "
            INSERT INTO delivery_targets (topic, adapter_type, endpoint_url, config_payload, nonce, dek_id, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
        ";
        match sqlx::query(insert_query)
            .bind(&payload.topic)
            .bind(&payload.adapter_type)
            .bind(&payload.endpoint_url)
            .bind(ciphertext)
            .bind(nonce)
            .bind(dek_id)
            .bind(payload.is_active)
            .execute(&state.repo.get().unwrap().pool).await {
            Ok(_) => (axum::http::StatusCode::OK, axum::Json(serde_json::json!({"status": "created"}))).into_response(),
            Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": e.to_string()}))).into_response(),
        }
    } else {
        let id_uuid = match uuid::Uuid::parse_str(payload.id.as_ref().unwrap()) {
            Ok(u) => u,
            Err(_) => return (axum::http::StatusCode::BAD_REQUEST, axum::Json(serde_json::json!({"error": "Invalid UUID"}))).into_response(),
        };
        
        let update_query = "
            UPDATE delivery_targets 
            SET topic = $1, adapter_type = $2, endpoint_url = $3, config_payload = $4, nonce = $5, dek_id = $6, is_active = $7
            WHERE id = $8
        ";
        match sqlx::query(update_query)
            .bind(&payload.topic)
            .bind(&payload.adapter_type)
            .bind(&payload.endpoint_url)
            .bind(ciphertext)
            .bind(nonce)
            .bind(dek_id)
            .bind(payload.is_active)
            .bind(id_uuid)
            .execute(&state.repo.get().unwrap().pool).await {
            Ok(_) => (axum::http::StatusCode::OK, axum::Json(serde_json::json!({"status": "updated"}))).into_response(),
            Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": e.to_string()}))).into_response(),
        }
    }
}
