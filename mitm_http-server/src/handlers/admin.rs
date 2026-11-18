/*
 * SPDX-License-Identifier: Apache-2.0
 */

use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use base64::Engine;
use serde::Deserialize;

use crate::handlers::{AppState, response::{ApiError, ApiResponse}};







#[derive(Deserialize)]
pub struct BackupRequest {
    pub passphrase: String,
}

#[derive(Deserialize)]
pub struct RestoreRequest {
    pub passphrase: String,
    pub backup: serde_json::Value,
}

fn backup_error(err: crate::handlers::backup::BackupError) -> ApiError {
    use crate::handlers::backup::{BackupError, MIN_PASSPHRASE_CHARS};
    use axum::http::StatusCode;
    match err {
        BackupError::PassphraseTooShort => {
            ApiError::validation_failed(format!("The passphrase must have at least {} characters.", MIN_PASSPHRASE_CHARS))
        }
        BackupError::UnsupportedFormat(detail) => {
            ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "BACKUP_FORMAT_UNSUPPORTED", "Unprocessable Entity", detail)
        }
        BackupError::DecryptionFailed => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "BACKUP_DECRYPTION_FAILED",
            "Unprocessable Entity",
            "Wrong passphrase or modified backup file.",
        ),
        BackupError::Internal(e) => ApiError::internal_logged("Backup encryption failed", e),
    }
}

/// `POST /api/v1/system/backup`: exports the configuration encrypted with the given passphrase (#99).
pub async fn handle_backup(
    State(state): State<AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    Json(req): Json<BackupRequest>,
) -> impl IntoResponse {
    use crate::handlers::backup;
    let passphrase = zeroize::Zeroizing::new(req.passphrase);
    if let Err(e) = backup::check_passphrase(&passphrase) {
        return backup_error(e).into_response();
    }

    let repo = state.repo.get().unwrap();
    // One consistent snapshot of all tables.
    let mut tx = match repo.pool.begin().await {
        Ok(t) => t,
        Err(e) => return ApiError::internal_logged("Database Error", e).into_response(),
    };
    if let Err(e) = sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY").execute(&mut *tx).await {
        return ApiError::internal_logged("Database Error", e).into_response();
    }
    let tables = match backup::export_tables(&mut tx).await {
        Ok(t) => t,
        Err(e) => return ApiError::internal_logged(&format!("Failed to export table {}", e.table), e.error).into_response(),
    };
    let _ = tx.rollback().await;

    let counts = backup::row_counts(&tables);
    let created_at = chrono::Utc::now().to_rfc3339();
    let document = match tokio::task::spawn_blocking(move || {
        backup::encrypt(&tables, &passphrase, env!("CARGO_PKG_VERSION"), &created_at)
    })
    .await
    {
        Ok(Ok(doc)) => doc,
        Ok(Err(e)) => return backup_error(e).into_response(),
        Err(e) => return ApiError::internal_logged("Backup encryption failed", e).into_response(),
    };

    repo.audit(
        &auth.username,
        crate::audit::action::BACKUP_CONFIG,
        serde_json::json!({ "format": backup::FORMAT, "tables": counts }),
    )
    .await;

    ApiResponse::ok(document).into_response()
}

/// `POST /api/v1/system/restore`: decrypts a backup and restores it in one transaction (#99).
pub async fn handle_restore(
    State(state): State<AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    Json(req): Json<RestoreRequest>,
) -> impl IntoResponse {
    use crate::handlers::backup::{self, UserAssignment};
    use axum::http::StatusCode;
    let passphrase = zeroize::Zeroizing::new(req.passphrase);
    let document = req.backup;

    let tables = match tokio::task::spawn_blocking(move || backup::decrypt(&document, &passphrase)).await {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => return backup_error(e).into_response(),
        Err(e) => return ApiError::internal_logged("Backup decryption failed", e).into_response(),
    };

    // Lock-out check: the current user must keep restore access with the backup's users, roles and policies.
    let lockout = |detail: &str| {
        ApiError::new(StatusCode::CONFLICT, "RESTORE_LOCKOUT", "Conflict", format!("Restore rejected: {}", detail)).into_response()
    };
    let roles = match backup::user_assignment(&tables, &auth.username) {
        UserAssignment::Missing => return lockout(&format!("user {} is not contained in the backup.", auth.username)),
        UserAssignment::Inactive => return lockout(&format!("user {} is inactive in the backup.", auth.username)),
        UserAssignment::NoRoles => backup::role_names(&tables, &[]),
        UserAssignment::Encrypted { wrapped_dek, nonce, encrypted_roles } => {
            let socket_path = std::path::PathBuf::from(&state.config.socket_dir).join("mitm_scheduler.sock");
            match crate::ipc_client::crypto_decrypt(wrapped_dek, nonce, encrypted_roles, &socket_path).await {
                Ok(plaintext) => backup::role_names(&tables, &serde_json::from_slice::<Vec<i32>>(&plaintext).unwrap_or_default()),
                Err(e) => {
                    log::error!("Restore: cannot decrypt the role assignment of {}: {}", auth.username, e);
                    return ApiError::service_unavailable("Cannot verify the role assignment in the backup (scheduler crypto unavailable).")
                        .into_response();
                }
            }
        }
    };
    let allowed = if tables.contains_key("casbin_rule") {
        match backup::policies_allow(&tables, &roles, "/api/v1/system/restore", "POST").await {
            Ok(a) => a,
            Err(e) => return ApiError::internal_logged("Restore: cannot evaluate the backup's RBAC policies", e).into_response(),
        }
    } else {
        use casbin::CoreApi;
        let enforcer = state.enforcer.read().await;
        roles.iter().any(|r| enforcer.enforce((r.as_str(), "/api/v1/system/restore", "POST")).unwrap_or(false))
    };
    if !allowed {
        return lockout(&format!("with the backup's roles ({}) user {} could no longer restore.", roles.join(", "), auth.username));
    }

    let repo = state.repo.get().unwrap();
    let mut tx = match repo.pool.begin().await {
        Ok(t) => t,
        Err(e) => return ApiError::internal_logged("Database Error", e).into_response(),
    };
    let report = match backup::restore_tables(&mut tx, &tables).await {
        Ok(r) => r,
        Err(e) => return ApiError::internal_logged(&format!("Restore failed at table {}", e.table), e.error).into_response(),
    };
    if let Err(e) = tx.commit().await {
        return ApiError::internal_logged("Failed to commit transaction", e).into_response();
    }

    {
        use casbin::CoreApi;
        if let Err(e) = state.enforcer.write().await.load_policy().await {
            log::error!("Restore: reloading the RBAC policies failed: {}", e);
        }
    }
    let scheduler_reloaded = crate::handlers::jobs::send_scheduler_ipc(mitm_common::ipc::SchedulerRequest::UpdateJobs, &state.config.socket_dir)
        .await
        .map_err(|e| log::warn!("Restore: scheduler reload failed: {}", e))
        .is_ok();

    repo.audit(
        &auth.username,
        crate::audit::action::RESTORE_CONFIG,
        serde_json::json!({
            "format": backup::FORMAT,
            "tables": report.tables,
            "deactivated_programs": report.deactivated_programs,
            "scheduler_reloaded": scheduler_reloaded,
        }),
    )
    .await;

    ApiResponse::updated(
        "Configuration restored",
        Some(serde_json::json!({
            "tables": report.tables,
            "deactivated_programs": report.deactivated_programs,
            "scheduler_reloaded": scheduler_reloaded,
        })),
    )
    .into_response()
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
            return ApiError::validation_failed("Invalid base64 encoding for nonce").into_response();
        }
    };
    let cipher_bytes = match base64::engine::general_purpose::STANDARD.decode(&payload.ciphertext) {
        Ok(b) => b,
        Err(_) => {
            return ApiError::validation_failed("Invalid base64 encoding for ciphertext").into_response();
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
            return ApiError::internal_logged("Database error", e).into_response();
        }
    };

    let deact = match sqlx::query("UPDATE storage_keys SET is_active = false WHERE is_active = true")
        .execute(&mut *tx)
        .await
    {
        Ok(res) => res.rows_affected(),
        Err(e) => {
            let _ = tx.rollback().await;
            return ApiError::internal_logged("Database error", e).into_response();
        }
    };

    if let Err(e) = sqlx::query("INSERT INTO storage_keys (wrapped_key, is_active) VALUES ($1, true)")
        .bind(&wrapped_key)
        .execute(&mut *tx)
        .await
    {
        let _ = tx.rollback().await;
        return ApiError::internal_logged("Database error", e).into_response();
    }

    if let Err(e) = tx.commit().await {
        return ApiError::internal_logged("Database error", e).into_response();
    }

    state.repo.get().unwrap().audit(&auth.username, crate::audit::action::KEY_ROTATION_SUCCESS, serde_json::json!({"deactivated_count": deact})).await;

    ApiResponse::updated("Key rotation successful", Some(serde_json::json!({ "deactivated_count": deact }))).into_response()
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

    let count = keys.len();
    ApiResponse::ok(keys).with_count(count).into_response()
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
        Err(e) => return ApiError::internal_logged("Database error", e).into_response(),
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

    let count = results.len();
    ApiResponse::ok(results).with_count(count).into_response()
}

pub async fn handle_delivery_targets(State(state): axum::extract::State<crate::handlers::AppState>) -> impl axum::response::IntoResponse {
    let query = "
        SELECT d.id, d.topic, d.adapter_type, d.endpoint_url, d.config_payload, d.nonce, k.wrapped_key, d.is_active
        FROM delivery_targets d
        JOIN storage_keys k ON d.dek_id = k.id
    ";
    let rows = match sqlx::query_as::<_, DeliveryTargetRow>(query).fetch_all(&state.repo.get().unwrap().pool).await {
        Ok(r) => r,
        Err(e) => return ApiError::internal_logged("Database error", e).into_response(),
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

    let count = results.len();
    ApiResponse::ok(results).with_count(count).into_response()
}

pub async fn handle_post_credentials(
    State(state): axum::extract::State<crate::handlers::AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    axum::Json(mut payload): axum::Json<SourceCredentialResponse>
) -> impl axum::response::IntoResponse {
    let socket_path = std::path::PathBuf::from(&state.config.socket_dir).join("mitm_scheduler.sock");
    
    let key_row: Option<(uuid::Uuid, Vec<u8>)> = match sqlx::query_as("SELECT id, wrapped_key FROM storage_keys WHERE is_active = true LIMIT 1")
        .fetch_optional(&state.repo.get().unwrap().pool).await {
        Ok(r) => r,
        Err(e) => return ApiError::internal_logged("Database error", e).into_response(),
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
                            Err(e) => return ApiError::internal_logged("Database error", e).into_response(),
                        };
                    (new_id, new_wrapped_dek)
                },
                Err(e) => return ApiError::internal_logged("Encryption failed", e).into_response(),
            }
        }
    };
 

    let (nonce, ciphertext) = match crate::ipc_client::crypto_encrypt(wrapped_key, std::mem::take(&mut payload.config_payload).into_bytes(), &socket_path).await {
        Ok(res) => res,
        Err(e) => return ApiError::internal_logged("Encryption failed", e).into_response(),
    };
    
    if payload.id.is_none() || payload.id.as_ref().unwrap().is_empty() {
        let insert_query = "
            INSERT INTO source_credentials (source_name, connector_type, topic, config_payload, nonce, dek_id, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
        ";
        match sqlx::query_scalar::<_, uuid::Uuid>(insert_query)
            .bind(&payload.source_name)
            .bind(&payload.connector_type)
            .bind(&payload.topic)
            .bind(ciphertext)
            .bind(nonce)
            .bind(dek_id)
            .bind(payload.is_active)
            .fetch_one(&state.repo.get().unwrap().pool).await {
            Ok(id) => {
                state.repo.get().unwrap().audit(&auth.username, crate::audit::action::UPSERT_CREDENTIAL, credential_audit_details(id, &payload, "create")).await;
                ApiResponse::created_at(&format!("/api/v1/config/credentials/{}", id), serde_json::json!({ "id": id }))
            },
            Err(e) => ApiError::internal_logged("Database error", e).into_response(),
        }
    } else {
        let id_uuid = match uuid::Uuid::parse_str(payload.id.as_ref().unwrap()) {
            Ok(u) => u,
            Err(_) => return ApiError::bad_request("Invalid UUID").into_response(),
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
            Ok(r) if r.rows_affected() == 0 => ApiError::not_found(format!("Credential {} not found", id_uuid)).into_response(),
            Ok(_) => {
                state.repo.get().unwrap().audit(&auth.username, crate::audit::action::UPSERT_CREDENTIAL, credential_audit_details(id_uuid, &payload, "update")).await;
                ApiResponse::updated(format!("Credential {} updated", id_uuid), Some(serde_json::json!({ "id": id_uuid }))).into_response()
            },
            Err(e) => ApiError::internal_logged("Database error", e).into_response(),
        }
    }
}

/// Audit details of a source credential change; never contains `config_payload`.
fn credential_audit_details(id: uuid::Uuid, payload: &SourceCredentialResponse, operation: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id.to_string(),
        "source_name": payload.source_name,
        "connector_type": payload.connector_type,
        "topic": payload.topic,
        "is_active": payload.is_active,
        "operation": operation,
    })
}

/// Audit details of a delivery target change; the endpoint URL without user info, query and fragment.
fn target_audit_details(id: uuid::Uuid, payload: &DeliveryTargetResponse, operation: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id.to_string(),
        "topic": payload.topic,
        "adapter_type": payload.adapter_type,
        "endpoint_url": crate::audit::redact_url(&payload.endpoint_url),
        "is_active": payload.is_active,
        "operation": operation,
    })
}

pub async fn handle_post_delivery_targets(
    State(state): axum::extract::State<crate::handlers::AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    axum::Json(mut payload): axum::Json<DeliveryTargetResponse>
) -> impl axum::response::IntoResponse {
    let socket_path = std::path::PathBuf::from(&state.config.socket_dir).join("mitm_scheduler.sock");
    
    let key_row: Option<(uuid::Uuid, Vec<u8>)> = match sqlx::query_as("SELECT id, wrapped_key FROM storage_keys WHERE is_active = true LIMIT 1")
        .fetch_optional(&state.repo.get().unwrap().pool).await {
        Ok(r) => r,
        Err(e) => return ApiError::internal_logged("Database error", e).into_response(),
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
                            Err(e) => return ApiError::internal_logged("Database error", e).into_response(),
                        };
                    (new_id, new_wrapped_dek)
                },
                Err(e) => return ApiError::internal_logged("Encryption failed", e).into_response(),
            }
        }
    };
 

    let (nonce, ciphertext) = match crate::ipc_client::crypto_encrypt(wrapped_key, std::mem::take(&mut payload.config_payload).into_bytes(), &socket_path).await {
        Ok(res) => res,
        Err(e) => return ApiError::internal_logged("Encryption failed", e).into_response(),
    };
    
    if payload.id.is_none() || payload.id.as_ref().unwrap().is_empty() {
        let insert_query = "
            INSERT INTO delivery_targets (topic, adapter_type, endpoint_url, config_payload, nonce, dek_id, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
        ";
        match sqlx::query_scalar::<_, uuid::Uuid>(insert_query)
            .bind(&payload.topic)
            .bind(&payload.adapter_type)
            .bind(&payload.endpoint_url)
            .bind(ciphertext)
            .bind(nonce)
            .bind(dek_id)
            .bind(payload.is_active)
            .fetch_one(&state.repo.get().unwrap().pool).await {
            Ok(id) => {
                state.repo.get().unwrap().audit(&auth.username, crate::audit::action::UPSERT_TARGET_CREDENTIAL, target_audit_details(id, &payload, "create")).await;
                ApiResponse::created_at(&format!("/api/v1/config/targets/{}", id), serde_json::json!({ "id": id }))
            },
            Err(e) => ApiError::internal_logged("Database error", e).into_response(),
        }
    } else {
        let id_uuid = match uuid::Uuid::parse_str(payload.id.as_ref().unwrap()) {
            Ok(u) => u,
            Err(_) => return ApiError::bad_request("Invalid UUID").into_response(),
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
            Ok(r) if r.rows_affected() == 0 => ApiError::not_found(format!("Delivery target {} not found", id_uuid)).into_response(),
            Ok(_) => {
                state.repo.get().unwrap().audit(&auth.username, crate::audit::action::UPSERT_TARGET_CREDENTIAL, target_audit_details(id_uuid, &payload, "update")).await;
                ApiResponse::updated(format!("Delivery target {} updated", id_uuid), Some(serde_json::json!({ "id": id_uuid }))).into_response()
            },
            Err(e) => ApiError::internal_logged("Database error", e).into_response(),
        }
    }
}

pub async fn handle_delete_credentials(
    State(state): State<AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> impl IntoResponse {
    match sqlx::query("DELETE FROM source_credentials WHERE id = $1")
        .bind(id)
        .execute(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(result) => {
            if result.rows_affected() > 0 {
                state.repo.get().unwrap().audit(&auth.username, crate::audit::action::DELETE_CREDENTIAL, serde_json::json!({"id": id.to_string()})).await;
                ApiResponse::deleted(format!("Credential {} deleted", id)).into_response()
            } else {
                ApiError::not_found(format!("Credential {} not found", id)).into_response()
            }
        },
        Err(e) => ApiError::internal_logged("Database error", e).into_response()
    }
}

pub async fn handle_delete_delivery_targets(
    State(state): State<AppState>,
    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> impl IntoResponse {
    match sqlx::query("DELETE FROM delivery_targets WHERE id = $1")
        .bind(id)
        .execute(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(result) => {
            if result.rows_affected() > 0 {
                state.repo.get().unwrap().audit(&auth.username, crate::audit::action::DELETE_TARGET_CREDENTIAL, serde_json::json!({"id": id.to_string()})).await;
                ApiResponse::deleted(format!("Delivery target {} deleted", id)).into_response()
            } else {
                ApiError::not_found(format!("Delivery target {} not found", id)).into_response()
            }
        },
        Err(e) => ApiError::internal_logged("Database error", e).into_response()
    }
}
