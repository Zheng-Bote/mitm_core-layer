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
    #[serde(rename = "log_system")]
    LogSystem { level: String, component: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", content = "payload")]
pub enum IpcResponse {
    #[serde(rename = "authenticate_result")]
    AuthenticateResult(AuthResponse),
    #[serde(rename = "error")]
    Error(String),
    #[serde(rename = "crypto_encrypt_result")]
    CryptoEncryptResult { nonce: Vec<u8>, ciphertext: Vec<u8> },
    #[serde(rename = "crypto_decrypt_result")]
    CryptoDecryptResult { plaintext: Vec<u8> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
#[serde(default)]
pub struct StatusEvent {
    pub run_id: i32,
    pub component: String,
    pub status: String,
    pub message: String,
    pub progress: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
#[serde(default)]
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
    #[serde(rename = "run_immediate_job")]
    RunImmediateJob(RunImmediateJob),

    #[serde(rename = "status")]
    Status(StatusEvent),
    #[serde(rename = "audit")]
    Audit(AuditEvent),
    #[serde(rename = "get_credentials")]
    GetCredentials(GetCredentialsRequest),
    #[serde(rename = "execute_job")]
    ExecuteJob { job_name: String },
    #[serde(rename = "stop_job")]
    StopJob { job_name: String },
    #[serde(rename = "update_jobs")]
    UpdateJobs,
    #[serde(rename = "crypto_encrypt")]
    CryptoEncrypt { wrapped_dek: Vec<u8>, plaintext: Vec<u8> },
    #[serde(rename = "crypto_decrypt")]
    CryptoDecrypt { wrapped_dek: Vec<u8>, nonce: Vec<u8>, ciphertext: Vec<u8> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialsResponse {
    pub master_key: String,
    pub db_config_json: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_serde() {
        let req = SchedulerRequest::ExecuteJob { job_name: "clean up".to_string() };
        println!("{}", serde_json::to_string(&req).unwrap());
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[derive(Clone)]
pub struct RunImmediateJob {
    pub command: String,
    pub args: String,
}
