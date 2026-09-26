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
    req: axum::extract::Request,
    next: axum::middleware::Next,
    config: mitm_common::config::DBConfig,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    use axum::http::StatusCode;
    use axum::Json;
    use serde::Serialize;
    
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
    
    let make_error = || {
        let err_resp = ErrorResponse {
            errors: vec![JsonApiError {
                status: "401".to_string(),
                title: "Unauthorized".to_string(),
                detail: Some("Missing or invalid authorization credentials.".to_string()),
            }],
        };
        (StatusCode::UNAUTHORIZED, Json(err_resp)).into_response()
    };
    
    let auth_header = match req.headers().get(axum::http::header::AUTHORIZATION) {
        Some(h) => h.to_str().unwrap_or(""),
        None => return make_error(),
    };
    
    if !auth_header.starts_with("Basic ") {
        return make_error();
    }
    
    let encoded = &auth_header[6..];
    let decoded = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded) {
        Ok(d) => d,
        Err(_) => return make_error(),
    };
    
    let credentials = String::from_utf8_lossy(&decoded);
    let parts: Vec<&str> = credentials.splitn(2, ':').collect();
    if parts.len() != 2 {
        return make_error();
    }
    
    let username = parts[0];
    let token = parts[1];
    let socket_dir = std::path::PathBuf::from(&config.socket_dir);
    let socket_path = socket_dir.join("mitm_iam.sock");

    let auth_resp = match authenticate_via_ipc(&username, &token, &socket_path).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("Auth IPC Error: {}", e);
            return make_error();
        }
    };
    
    if !auth_resp.success {
        return make_error();
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
