use tokio::net::UnixStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use mitm_common::ipc::{IpcRequest, IpcResponse, AuthRequest, AuthResponse};

pub async fn authenticate_via_ipc(username: &str, token: &str) -> Result<AuthResponse, String> {
    let mut stream = UnixStream::connect("/tmp/mitm_iam.sock").await
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
    }
}
