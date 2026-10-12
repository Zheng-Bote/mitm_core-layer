/*
 * SPDX-License-Identifier: Apache-2.0
 */

use axum::{
    extract::{State, Json},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{Utc, Duration};
use base64::{Engine as _, engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD}};
use webauthn_rs::prelude::*;
use crate::handlers::{AppState, ErrorResponse, JsonApiError};

#[derive(Serialize)]
pub struct BootstrapStatusResponse {
    pub bootstrap_required: bool,
}

#[derive(Deserialize)]
pub struct StartRegistrationReq {
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub department: Option<String>,
    pub bootstrap_token: Option<String>,
}

#[derive(Serialize)]
pub struct StartRegistrationResp {
    pub ceremony_id: Uuid,
    pub public_key_options: CreationChallengeResponse,
    pub is_bootstrap: bool,
}

#[derive(Deserialize)]
pub struct FinishRegistrationReq {
    pub ceremony_id: Uuid,
    pub credential: RegisterPublicKeyCredential,
    pub attachment_type: Option<String>,
    pub yubikey_serial: Option<String>,
}

#[derive(Serialize)]
pub struct FinishRegistrationResp {
    pub status: String, // "active" or "pending_approval"
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_token: Option<String>,
}

#[derive(Deserialize)]
pub struct StartLoginReq {
    pub username: String,
}

#[derive(Serialize)]
pub struct StartLoginResp {
    pub ceremony_id: Uuid,
    pub public_key_options: RequestChallengeResponse,
}

#[derive(Deserialize)]
pub struct FinishLoginReq {
    pub ceremony_id: Uuid,
    pub credential: PublicKeyCredential,
}

#[derive(Serialize)]
pub struct FinishLoginResp {
    pub session_token: String,
}

// GET /api/v1/auth/bootstrap/status
pub async fn get_bootstrap_status(State(state): State<AppState>) -> Response {
    let repo = match state.repo.get() {
        Some(r) => r,
        None => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "503".into(), title: "Service Unavailable".into(), detail: Some("Database not ready".into()) }] };
            return (StatusCode::SERVICE_UNAVAILABLE, Json(err)).into_response();
        }
    };

    let user_count: i64 = match sqlx::query_scalar("SELECT COUNT(*) FROM admin_users")
        .fetch_one(&repo.pool)
        .await 
    {
        Ok(c) => c,
        Err(e) => {
            log::error!("DB error checking bootstrap status: {}", e);
            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Internal Server Error".into(), detail: Some("DB query failed".into()) }] };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };

    (StatusCode::OK, Json(BootstrapStatusResponse {
        bootstrap_required: user_count == 0,
    })).into_response()
}

// POST /api/v1/auth/webauthn/register/start
pub async fn start_registration(
    State(state): State<AppState>,
    Json(payload): Json<StartRegistrationReq>,
) -> Response {
    let repo = match state.repo.get() {
        Some(r) => r,
        None => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "503".into(), title: "Service Unavailable".into(), detail: Some("Database not ready".into()) }] };
            return (StatusCode::SERVICE_UNAVAILABLE, Json(err)).into_response();
        }
    };

    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_users")
        .fetch_one(&repo.pool)
        .await
        .unwrap_or(0);

    let is_bootstrap = user_count == 0;

    if is_bootstrap {
        // Validate bootstrap token
        let expected_token = std::env::var("INITIAL_ADMIN_TOKEN").unwrap_or_else(|_| "wba_boot_init".to_string());
        if let Some(token) = &payload.bootstrap_token {
            if token != &expected_token {
                let err = ErrorResponse { errors: vec![JsonApiError { status: "403".into(), title: "Forbidden".into(), detail: Some("Invalid Bootstrap Token.".into()) }] };
                return (StatusCode::FORBIDDEN, Json(err)).into_response();
            }
        } else {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "400".into(), title: "Bad Request".into(), detail: Some("Bootstrap Token is required when system is uninitialized.".into()) }] };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    } else {
        // Check if user already exists
        let exists: Option<i32> = sqlx::query_scalar("SELECT id FROM admin_users WHERE username = $1")
            .bind(&payload.username)
            .fetch_optional(&repo.pool)
            .await
            .unwrap_or(None);

        if exists.is_some() {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "409".into(), title: "Conflict".into(), detail: Some("Username/LoginID already registered.".into()) }] };
            return (StatusCode::CONFLICT, Json(err)).into_response();
        }
    }

    let user_uuid = Uuid::new_v4();
    let display_name = format!("{} {}", payload.first_name.as_deref().unwrap_or(""), payload.last_name.as_deref().unwrap_or("")).trim().to_string();
    let display_name = if display_name.is_empty() { payload.username.clone() } else { display_name };

    let (ccr, reg_state) = match state.webauthn.start_passkey_registration(
        user_uuid,
        &payload.username,
        &display_name,
        None,
    ) {
        Ok(res) => res,
        Err(e) => {
            log::error!("WebAuthn start_passkey_registration error: {:?}", e);
            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "WebAuthn Error".into(), detail: Some(format!("{:?}", e)) }] };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };

    let ceremony_id = Uuid::new_v4();
    let state_data = serde_json::json!({
        "reg_state": reg_state,
        "username": payload.username,
        "first_name": payload.first_name,
        "last_name": payload.last_name,
        "department": payload.department,
        "is_bootstrap": is_bootstrap,
    });

    let expires_at = Utc::now() + Duration::minutes(10);
    let _ = sqlx::query(
        "INSERT INTO webauthn_states (id, challenge_type, state_data, expires_at) VALUES ($1, 'register', $2, $3)"
    )
    .bind(ceremony_id)
    .bind(state_data)
    .bind(expires_at)
    .execute(&repo.pool)
    .await;

    (StatusCode::OK, Json(StartRegistrationResp {
        ceremony_id,
        public_key_options: ccr,
        is_bootstrap,
    })).into_response()
}

// POST /api/v1/auth/webauthn/register/finish
pub async fn finish_registration(
    State(state): State<AppState>,
    Json(payload): Json<FinishRegistrationReq>,
) -> Response {
    let repo = match state.repo.get() {
        Some(r) => r,
        None => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "503".into(), title: "Service Unavailable".into(), detail: Some("Database not ready".into()) }] };
            return (StatusCode::SERVICE_UNAVAILABLE, Json(err)).into_response();
        }
    };

    let row = match sqlx::query_as::<_, (serde_json::Value, chrono::DateTime<chrono::Utc>)>(
        "SELECT state_data, expires_at FROM webauthn_states WHERE id = $1 AND challenge_type = 'register'"
    )
    .bind(payload.ceremony_id)
    .fetch_optional(&repo.pool)
    .await {
        Ok(Some(r)) => r,
        _ => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "400".into(), title: "Bad Request".into(), detail: Some("Ceremony expired or not found.".into()) }] };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    if row.1 < Utc::now() {
        let _ = sqlx::query("DELETE FROM webauthn_states WHERE id = $1").bind(payload.ceremony_id).execute(&repo.pool).await;
        let err = ErrorResponse { errors: vec![JsonApiError { status: "400".into(), title: "Bad Request".into(), detail: Some("Ceremony expired.".into()) }] };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let state_data = row.0;
    let reg_state_val = match state_data.get("reg_state") {
        Some(v) => v,
        None => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Internal Server Error".into(), detail: Some("Corrupt ceremony state".into()) }] };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };

    let reg_state: PasskeyRegistration = match serde_json::from_value(reg_state_val.clone()) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to deserialize PasskeyRegistration: {}", e);
            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Internal Server Error".into(), detail: Some(e.to_string()) }] };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };

    let passkey = match state.webauthn.finish_passkey_registration(&payload.credential, &reg_state) {
        Ok(pk) => pk,
        Err(e) => {
            log::error!("WebAuthn finish_passkey_registration error: {:?}", e);
            let err = ErrorResponse { errors: vec![JsonApiError { status: "400".into(), title: "Registration Verification Failed".into(), detail: Some(format!("{:?}", e)) }] };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    let username = state_data["username"].as_str().unwrap_or("unknown");
    let first_name = state_data["first_name"].as_str();
    let last_name = state_data["last_name"].as_str();
    let department = state_data["department"].as_str();
    let is_bootstrap = state_data["is_bootstrap"].as_bool().unwrap_or(false);

    // Check if user already exists
    let existing_user = sqlx::query_as::<_, (i32, bool)>(
        "SELECT id, is_active FROM admin_users WHERE username = $1"
    )
    .bind(username)
    .fetch_optional(&repo.pool)
    .await
    .unwrap_or(None);

    let (user_id, is_new_user) = if let Some((uid, _)) = existing_user {
        (uid, false)
    } else {
        // Initial Bootstrap Admin is immediately is_active=true; standard new user registration is is_active=false
        let is_active = is_bootstrap;

        let uid: i32 = match sqlx::query_scalar(
            r#"
            INSERT INTO admin_users (username, password_hash, first_name, last_name, department, is_active)
            VALUES ($1, 'webauthn_passkey', $2, $3, $4, $5)
            RETURNING id
            "#
        )
        .bind(username)
        .bind(first_name)
        .bind(last_name)
        .bind(department)
        .bind(is_active)
        .fetch_one(&repo.pool)
        .await {
            Ok(id) => id,
            Err(e) => {
                log::error!("Failed to insert user {}: {}", username, e);
                let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Database Error".into(), detail: Some(e.to_string()) }] };
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
            }
        };
        (uid, true)
    };

    // If bootstrap, assign ADMIN role
    if is_bootstrap {
        if let Ok(Some(admin_role_id)) = sqlx::query_scalar::<_, i32>("SELECT id FROM roles WHERE name = 'ADMIN'").fetch_optional(&repo.pool).await {
            let socket_dir = std::path::PathBuf::from(&state.config.socket_dir);
            let socket_path = socket_dir.join("mitm_scheduler.sock");
            if let Ok(creds) = crate::ipc_client::get_credentials(&socket_path).await {
                if let Ok(kek) = STANDARD.decode(&creds.master_key) {
                    let roles_json = serde_json::to_vec(&vec![admin_role_id]).unwrap_or_default();
                    let enc_data: Option<(Vec<u8>, Vec<u8>, Vec<u8>)> = (|| {
                        let wrapped_dek = mitm_common::crypto::generate_wrapped_dek(&kek).ok()?;
                        let (ciphertext, nonce) = mitm_common::crypto::envelope_encrypt(&kek, &wrapped_dek, &roles_json).ok()?;
                        Some((wrapped_dek, nonce, ciphertext))
                    })();
                    if let Some((wrapped_dek, nonce, ciphertext)) = enc_data {
                        let _ = sqlx::query(
                            "INSERT INTO user_roles_encrypted (user_id, wrapped_dek, nonce, encrypted_roles) VALUES ($1, $2, $3, $4) ON CONFLICT (user_id) DO NOTHING"
                        )
                        .bind(user_id)
                        .bind(wrapped_dek)
                        .bind(nonce)
                        .bind(ciphertext)
                        .execute(&repo.pool)
                        .await;
                    }
                }
            }
        }
        let _ = repo.log_admin("system", "BOOTSTRAP_ADMIN_REGISTERED", serde_json::json!({ "username": username })).await;
    }

    // Save Passkey Credential
    let cred_id_str = URL_SAFE_NO_PAD.encode(passkey.cred_id().as_ref());
    let passkey_json = serde_json::to_value(&passkey).unwrap_or(serde_json::Value::Null);
    let aaguid_str = None::<String>;
    let sign_count = 0i64;
    let attachment_type = payload.attachment_type.unwrap_or_else(|| "cross-platform".to_string());

    let _ = sqlx::query(
        r#"
        INSERT INTO webauthn_credentials (user_id, credential_id, passkey_json, aaguid, sign_count, attachment_type, yubikey_serial)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#
    )
    .bind(user_id)
    .bind(cred_id_str)
    .bind(passkey_json)
    .bind(aaguid_str)
    .bind(sign_count)
    .bind(attachment_type)
    .bind(payload.yubikey_serial)
    .execute(&repo.pool)
    .await;

    // Clean up ceremony state
    let _ = sqlx::query("DELETE FROM webauthn_states WHERE id = $1").bind(payload.ceremony_id).execute(&repo.pool).await;

    if is_bootstrap {
        // Automatically establish session for the bootstrap admin
        let token_uuid = Uuid::new_v4();
        let now = Utc::now();
        let expires_at = now + Duration::hours(24);
        let _ = sqlx::query(
            "INSERT INTO user_sessions (session_token, os_user, created_at, expires_at, last_active_at) VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(token_uuid)
        .bind(username)
        .bind(now)
        .bind(expires_at)
        .bind(now)
        .execute(&repo.pool)
        .await;

        (StatusCode::CREATED, Json(FinishRegistrationResp {
            status: "active".into(),
            message: "Bootstrap admin registered and active.".into(),
            session_token: Some(token_uuid.to_string()),
        })).into_response()
    } else if !is_new_user {
        // Additional key registered for an existing user
        (StatusCode::CREATED, Json(FinishRegistrationResp {
            status: "active".into(),
            message: "Additional hardware token registered successfully.".into(),
            session_token: None,
        })).into_response()
    } else {
        (StatusCode::CREATED, Json(FinishRegistrationResp {
            status: "pending_approval".into(),
            message: "User registered successfully. Your account is inactive pending administrator approval.".into(),
            session_token: None,
        })).into_response()
    }
}

// POST /api/v1/auth/webauthn/login/start
pub async fn start_login(
    State(state): State<AppState>,
    Json(payload): Json<StartLoginReq>,
) -> Response {
    let repo = match state.repo.get() {
        Some(r) => r,
        None => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "503".into(), title: "Service Unavailable".into(), detail: Some("Database not ready".into()) }] };
            return (StatusCode::SERVICE_UNAVAILABLE, Json(err)).into_response();
        }
    };

    let user_opt: Option<(i32, bool)> = match sqlx::query_as("SELECT id, is_active FROM admin_users WHERE username = $1")
        .bind(&payload.username)
        .fetch_optional(&repo.pool)
        .await {
            Ok(u) => u,
            Err(e) => {
                log::error!("DB error looking up user {}: {}", payload.username, e);
                let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Internal Server Error".into(), detail: Some("DB query error".into()) }] };
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
            }
        };

    let (user_id, is_active) = match user_opt {
        Some((id, active)) => (id, active),
        None => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "404".into(), title: "Not Found".into(), detail: Some("User not found.".into()) }] };
            return (StatusCode::NOT_FOUND, Json(err)).into_response();
        }
    };

    if !is_active {
        let err = ErrorResponse { errors: vec![JsonApiError { status: "403".into(), title: "Forbidden".into(), detail: Some("Account is inactive pending administrator approval.".into()) }] };
        return (StatusCode::FORBIDDEN, Json(err)).into_response();
    }

    // Fetch credentials for user
    let cred_rows: Vec<(serde_json::Value,)> = match sqlx::query_as("SELECT passkey_json FROM webauthn_credentials WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(&repo.pool)
        .await {
            Ok(rows) => rows,
            Err(e) => {
                log::error!("DB error fetching credentials for user {}: {}", user_id, e);
                let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Internal Server Error".into(), detail: Some("DB error".into()) }] };
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
            }
        };

    if cred_rows.is_empty() {
        let err = ErrorResponse { errors: vec![JsonApiError { status: "400".into(), title: "Bad Request".into(), detail: Some("No WebAuthn passkeys registered for this user.".into()) }] };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let mut passkeys: Vec<Passkey> = Vec::new();
    for (json_val,) in cred_rows {
        if let Ok(pk) = serde_json::from_value::<Passkey>(json_val) {
            passkeys.push(pk);
        }
    }

    let (rcr, auth_state) = match state.webauthn.start_passkey_authentication(&passkeys) {
        Ok(res) => res,
        Err(e) => {
            log::error!("WebAuthn start_passkey_authentication error: {:?}", e);
            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "WebAuthn Error".into(), detail: Some(format!("{:?}", e)) }] };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };

    let ceremony_id = Uuid::new_v4();
    let state_data = serde_json::json!({
        "auth_state": auth_state,
        "username": payload.username,
        "user_id": user_id,
    });
    let expires_at = Utc::now() + Duration::minutes(5);

    let _ = sqlx::query(
        "INSERT INTO webauthn_states (id, challenge_type, state_data, user_id, expires_at) VALUES ($1, 'login', $2, $3, $4)"
    )
    .bind(ceremony_id)
    .bind(state_data)
    .bind(user_id)
    .bind(expires_at)
    .execute(&repo.pool)
    .await;

    (StatusCode::OK, Json(StartLoginResp {
        ceremony_id,
        public_key_options: rcr,
    })).into_response()
}

// POST /api/v1/auth/webauthn/login/finish
pub async fn finish_login(
    State(state): State<AppState>,
    Json(payload): Json<FinishLoginReq>,
) -> Response {
    let repo = match state.repo.get() {
        Some(r) => r,
        None => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "503".into(), title: "Service Unavailable".into(), detail: Some("Database not ready".into()) }] };
            return (StatusCode::SERVICE_UNAVAILABLE, Json(err)).into_response();
        }
    };

    let row = match sqlx::query_as::<_, (serde_json::Value, chrono::DateTime<chrono::Utc>)>(
        "SELECT state_data, expires_at FROM webauthn_states WHERE id = $1 AND challenge_type = 'login'"
    )
    .bind(payload.ceremony_id)
    .fetch_optional(&repo.pool)
    .await {
        Ok(Some(r)) => r,
        _ => {
            let err = ErrorResponse { errors: vec![JsonApiError { status: "400".into(), title: "Bad Request".into(), detail: Some("Ceremony expired or not found.".into()) }] };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    if row.1 < Utc::now() {
        let _ = sqlx::query("DELETE FROM webauthn_states WHERE id = $1").bind(payload.ceremony_id).execute(&repo.pool).await;
        let err = ErrorResponse { errors: vec![JsonApiError { status: "400".into(), title: "Bad Request".into(), detail: Some("Ceremony expired.".into()) }] };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let state_data = row.0;
    let auth_state: PasskeyAuthentication = match serde_json::from_value(state_data["auth_state"].clone()) {
        Ok(a) => a,
        Err(e) => {
            log::error!("Failed to deserialize PasskeyAuthentication: {}", e);
            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "Internal Server Error".into(), detail: Some("State parse error".into()) }] };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };

    let auth_result = match state.webauthn.finish_passkey_authentication(&payload.credential, &auth_state) {
        Ok(r) => r,
        Err(e) => {
            log::error!("WebAuthn finish_passkey_authentication failed: {:?}", e);
            let err = ErrorResponse { errors: vec![JsonApiError { status: "401".into(), title: "Unauthorized".into(), detail: Some("Passkey verification failed.".into()) }] };
            return (StatusCode::UNAUTHORIZED, Json(err)).into_response();
        }
    };

    let user_id = state_data["user_id"].as_i64().unwrap_or(0) as i32;
    let username = state_data["username"].as_str().unwrap_or("unknown");

    // Check again if still active
    let is_active: bool = sqlx::query_scalar("SELECT is_active FROM admin_users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&repo.pool)
        .await
        .unwrap_or(Some(false))
        .unwrap_or(false);

    if !is_active {
        let err = ErrorResponse { errors: vec![JsonApiError { status: "403".into(), title: "Forbidden".into(), detail: Some("Account is inactive.".into()) }] };
        return (StatusCode::FORBIDDEN, Json(err)).into_response();
    }

    // Update credential counter and last_used_at
    let cred_id_str = URL_SAFE_NO_PAD.encode(auth_result.cred_id().as_ref());
    let _ = sqlx::query(
        "UPDATE webauthn_credentials SET sign_count = $1, last_used_at = NOW() WHERE credential_id = $2"
    )
    .bind(auth_result.counter() as i64)
    .bind(cred_id_str)
    .execute(&repo.pool)
    .await;

    // Clean up ceremony state
    let _ = sqlx::query("DELETE FROM webauthn_states WHERE id = $1").bind(payload.ceremony_id).execute(&repo.pool).await;

    // Issue session token
    let token_uuid = Uuid::new_v4();
    let now = Utc::now();
    let expires_at = now + Duration::hours(24);
    let _ = sqlx::query(
        "INSERT INTO user_sessions (session_token, os_user, created_at, expires_at, last_active_at) VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(token_uuid)
    .bind(username)
    .bind(now)
    .bind(expires_at)
    .bind(now)
    .execute(&repo.pool)
    .await;

    let _ = repo.log_admin(username, "USER_LOGIN_WEBAUTHN", serde_json::json!({ "status": "success" })).await;

    (StatusCode::OK, Json(FinishLoginResp {
        session_token: token_uuid.to_string(),
    })).into_response()
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/bootstrap/status", get(get_bootstrap_status))
        .route("/webauthn/register/start", post(start_registration))
        .route("/webauthn/register/finish", post(finish_registration))
        .route("/webauthn/login/start", post(start_login))
        .route("/webauthn/login/finish", post(finish_login))
}
