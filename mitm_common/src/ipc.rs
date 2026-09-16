use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub success: bool,
    pub username: String,
    pub roles: Vec<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", content = "payload")]
pub enum IpcRequest {
    #[serde(rename = "authenticate")]
    Authenticate(AuthRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", content = "payload")]
pub enum IpcResponse {
    #[serde(rename = "authenticate_result")]
    AuthenticateResult(AuthResponse),
    #[serde(rename = "error")]
    Error(String),
}
