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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEvent {
    pub run_id: i32,
    pub component: String,
    pub status: String,
    pub message: String,
    pub progress: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub run_id: i32,
    pub component: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCredentialsRequest {
    pub run_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SchedulerRequest {
    #[serde(rename = "status")]
    Status(StatusEvent),
    #[serde(rename = "audit")]
    Audit(AuditEvent),
    #[serde(rename = "get_credentials")]
    GetCredentials(GetCredentialsRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialsResponse {
    pub master_key: String,
    pub db_config_json: String,
}
