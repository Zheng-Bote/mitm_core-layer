/*
 * SPDX-License-Identifier: Apache-2.0
 */

use tokio::net::UnixStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use mitm_common::ipc::{IpcRequest, IpcResponse, AuthRequest, AuthResponse};

pub async fn authenticate_via_ipc(username: &str, token: &str, socket_path: &std::path::Path) -> Result<AuthResponse, String> {
    let mut stream = UnixStream::connect(socket_path).await
        .map_err(|e| format!("Failed to connect to UDS: {}", e))?;
    
    let req = IpcRequest::Authenticate(AuthRequest {
        username: username.to_string(),
        token: token.to_string(),
    });
    
    let mut json_req = serde_json::to_string(&req).unwrap();
    json_req.push('\n');
    
    stream.write_all(json_req.as_bytes()).await
        .map_err(|e| format!("Failed to write: {}", e))?;
    
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await
        .map_err(|e| format!("Failed to read: {}", e))?;
    
    let resp: IpcResponse = serde_json::from_str(&line)
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    match resp {
        IpcResponse::AuthenticateResult(result) => Ok(result),
        IpcResponse::Error(e) => Err(e),
        _ => Err("Unexpected IPC response".to_string()),
    }
}

pub async fn auth_middleware(
    axum::extract::State(state): axum::extract::State<crate::handlers::AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    use axum::http::StatusCode;
    use axum::Json;
    use serde::Serialize;
    
    let path = req.uri().path();
    // Protect these paths. We also need to allow /api/user/v1 through without auth for creating sessions, but wait, the check is explicitly allowing anything else!
    if !path.starts_with("/admin") && !path.starts_with("/api/admin/v1") && !path.starts_with("/api/transformation/v1") && !path.starts_with("/api/system/v1/dashboard") {
        return next.run(req).await;
    }

    #[derive(Serialize)]
    struct JsonApiError {
        status: String,
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    }

    #[derive(Serialize)]
    struct ErrorResponse {
        errors: Vec<JsonApiError>,
    }
    
    let make_error = |detail: &str| {
        let err_resp = ErrorResponse {
            errors: vec![JsonApiError {
                status: "401".to_string(),
                title: "Unauthorized".to_string(),
                detail: Some(detail.to_string()),
            }],
        };
        (StatusCode::UNAUTHORIZED, Json(err_resp)).into_response()
    };
    
    let auth_header = match req.headers().get(axum::http::header::AUTHORIZATION) {
        Some(h) => h.to_str().unwrap_or(""),
        None => return make_error("Missing or invalid authorization credentials."),
    };
    
    let mut auth_resp = mitm_common::ipc::AuthResponse {
        success: false,
        username: String::new(),
        roles: vec![],
        error_message: None,
    };

    if auth_header.starts_with("Bearer ") {
        let token_str = &auth_header[7..];
        let token_uuid = match uuid::Uuid::parse_str(token_str) {
            Ok(u) => u,
            Err(_) => return make_error("Invalid Bearer token format."),
        };

        let now = chrono::Utc::now();
        
        let pool = &state.repo.get().unwrap().pool;

        let session_opt = match sqlx::query_as::<_, (String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)>(
            "SELECT os_user, expires_at, last_active_at FROM user_sessions WHERE session_token = $1"
        )
        .bind(token_uuid)
        .fetch_optional(pool).await {
            Ok(s) => s,
            Err(_) => return make_error("Database error verifying session."),
        };

        if let Some(session) = session_opt {
            if session.1 < now {
                return make_error("Session expired (absolute TTL).");
            }
            if session.2.unwrap_or(now) + chrono::Duration::hours(2) < now {
                return make_error("Session expired (idle timeout).");
            }
            
            // Touch session
            let _ = sqlx::query(
                "UPDATE user_sessions SET last_active_at = $1 WHERE session_token = $2"
            )
            .bind(now)
            .bind(token_uuid)
            .execute(pool).await;

            auth_resp.success = true;
            auth_resp.username = session.0.clone();
            
            // Query DB for actual roles, for now we mock based on admin_users check
            let is_admin = sqlx::query_as::<_, (i32,)>(
                "SELECT id FROM admin_users WHERE username = $1 AND is_active = true"
            )
            .bind(&session.0)
            .fetch_optional(pool).await.unwrap_or(None).is_some();

            if is_admin {
                auth_resp.roles = vec!["ADMIN".to_string(), "VIEWER".to_string(), "UPLOADER".to_string()];
            } else {
                auth_resp.roles = vec!["VIEWER".to_string()];
            }

        } else {
            return make_error("Invalid session token.");
        }
    } else if auth_header.starts_with("Basic ") {
        // Legacy Basic Auth fallback via IAM UDS
        let encoded = &auth_header[6..];
        let decoded = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded) {
            Ok(d) => d,
            Err(_) => return make_error("Invalid Basic encoding."),
        };
        
        let credentials = String::from_utf8_lossy(&decoded);
        let parts: Vec<&str> = credentials.splitn(2, ':').collect();
        if parts.len() != 2 {
            return make_error("Invalid Basic format.");
        }
        
        let username = parts[0];
        let token = parts[1];
        let socket_dir = std::path::PathBuf::from(&state.config.socket_dir);
        let socket_path = socket_dir.join("mitm_iam.sock");

        match authenticate_via_ipc(&username, &token, &socket_path).await {
            Ok(r) => {
                auth_resp = r;
            },
            Err(e) => {
                log::error!("Auth IPC Error: {}", e);
                return make_error("IAM Backend error.");
            }
        };
    } else {
        return make_error("Unsupported authorization scheme.");
    }

    if !auth_resp.success {
        return make_error("Authentication failed.");
    }
    
    let mut req = req;
    req.extensions_mut().insert(auth_resp);
    
    next.run(req).await
}

pub async fn crypto_encrypt(wrapped_dek: Vec<u8>, plaintext: Vec<u8>, socket_path: &std::path::Path) -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut stream = UnixStream::connect(socket_path).await
        .map_err(|e| format!("Failed to connect to UDS: {}", e))?;
    
    let req = mitm_common::ipc::SchedulerRequest::CryptoEncrypt { wrapped_dek, plaintext };
    let mut json_req = serde_json::to_string(&req).unwrap();
    json_req.push('\n');
    
    stream.write_all(json_req.as_bytes()).await
        .map_err(|e| format!("Failed to write: {}", e))?;
    
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await
        .map_err(|e| format!("Failed to read: {}", e))?;
    
    let resp: IpcResponse = serde_json::from_str(&line)
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    match resp {
        IpcResponse::CryptoEncryptResult { nonce, ciphertext } => Ok((nonce, ciphertext)),
        IpcResponse::Error(e) => Err(e),
        _ => Err("Unexpected IPC response".to_string()),
    }
}

pub async fn crypto_decrypt(wrapped_dek: Vec<u8>, nonce: Vec<u8>, ciphertext: Vec<u8>, socket_path: &std::path::Path) -> Result<Vec<u8>, String> {
    let mut stream = UnixStream::connect(socket_path).await
        .map_err(|e| format!("Failed to connect to UDS: {}", e))?;
    
    let req = mitm_common::ipc::SchedulerRequest::CryptoDecrypt { wrapped_dek, nonce, ciphertext };
    let mut json_req = serde_json::to_string(&req).unwrap();
    json_req.push('\n');
    
    stream.write_all(json_req.as_bytes()).await
        .map_err(|e| format!("Failed to write: {}", e))?;
    
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await
        .map_err(|e| format!("Failed to read: {}", e))?;
    
    let resp: IpcResponse = serde_json::from_str(&line)
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    match resp {
        IpcResponse::CryptoDecryptResult { plaintext } => Ok(plaintext),
        IpcResponse::Error(e) => Err(e),
        _ => Err("Unexpected IPC response".to_string()),
    }
}

pub async fn get_credentials(socket_path: &std::path::Path) -> Result<mitm_common::ipc::CredentialsResponse, String> {
    let mut stream = UnixStream::connect(socket_path).await
        .map_err(|e| format!("Failed to connect to UDS: {}", e))?;
    
    let req = mitm_common::ipc::SchedulerRequest::GetCredentials(mitm_common::ipc::GetCredentialsRequest { run_id: 0 });
    let mut json_req = serde_json::to_string(&req).unwrap();
    json_req.push('\n');
    
    stream.write_all(json_req.as_bytes()).await
        .map_err(|e| format!("Failed to write: {}", e))?;
    
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await
        .map_err(|e| format!("Failed to read: {}", e))?;
    
    let resp: mitm_common::ipc::CredentialsResponse = serde_json::from_str(&line)
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    Ok(resp)
}

pub async fn query_iam_info(socket_path: &std::path::Path) -> String {
    let timeout = tokio::time::Duration::from_millis(500);
    let fut = async {
        let mut stream = UnixStream::connect(socket_path).await.ok()?;
        let req = mitm_common::ipc::IpcRequest::GetInfo;
        let mut json_req = serde_json::to_string(&req).unwrap();
        json_req.push('\n');
        stream.write_all(json_req.as_bytes()).await.ok()?;
        
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.ok()?;
        
        match serde_json::from_str::<mitm_common::ipc::IpcResponse>(&line) {
            Ok(mitm_common::ipc::IpcResponse::GetInfoResult(info)) => Some(info.version),
            _ => None,
        }
    };
    
    match tokio::time::timeout(timeout, fut).await {
        Ok(Some(v)) => v,
        _ => "offline".to_string(),
    }
}

pub async fn query_scheduler_info(socket_path: &std::path::Path) -> String {
    let timeout = tokio::time::Duration::from_millis(500);
    let fut = async {
        let mut stream = UnixStream::connect(socket_path).await.ok()?;
        let req = mitm_common::ipc::SchedulerRequest::GetInfo;
        let mut json_req = serde_json::to_string(&req).unwrap();
        json_req.push('\n');
        stream.write_all(json_req.as_bytes()).await.ok()?;
        
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.ok()?;
        
        match serde_json::from_str::<mitm_common::ipc::InfoResponse>(&line) {
            Ok(info) => Some(info.version),
            _ => None,
        }
    };
    
    match tokio::time::timeout(timeout, fut).await {
        Ok(Some(v)) => v,
        _ => "offline".to_string(),
    }
}
