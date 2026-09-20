mod db;

use std::env;
use std::fs;
use std::path::PathBuf;
use tokio::net::UnixListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use mitm_common::config::{load_config, DBConfig};
use mitm_common::ipc::{IpcRequest, IpcResponse, AuthResponse};
use subtle::ConstantTimeEq;

const APP_NAME: &str = "MitM IAM Server";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let args: Vec<String> = env::args().collect();
    let config_param = args.get(1).map(|s| s.as_str());
    
    let password = env::var("MASTER_KEY").unwrap_or_else(|_| "".to_string());
    
    let config = match load_config(config_param, &password) {
        Ok(cfg) => cfg,
        Err(e) => {
            log::error!("Failed to load configuration: {}", e);
            return Err(e);
        }
    };

    let repo = db::Repository::new(&config).await?;
    log::info!("Connected to PostgreSQL at {}:{}", config.db.host, config.db.port);

    db::bootstrap_admins(&repo, &config).await;

    // Log startup
    let _ = repo.log_system("INFO", "iam-server", &format!("Starting {} v{}", APP_NAME, VERSION)).await;

    // Ensure socket directory exists
    let socket_dir = PathBuf::from(&config.socket_dir);
    if !socket_dir.exists() {
        fs::create_dir_all(&socket_dir)?;
    }

    let socket_path = socket_dir.join("mitm_iam.sock");
    if socket_path.exists() {
        fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    log::info!("IAM Server listening on UDS {:?}", socket_path);

    let config = std::sync::Arc::new(config);
    let repo = std::sync::Arc::new(repo);

    loop {
        match listener.accept().await {
            Ok((mut stream, _)) => {
                let config = config.clone();
                let repo = repo.clone();
                tokio::spawn(async move {
                    let (reader, mut writer) = stream.split();
                    let mut reader = BufReader::new(reader);
                    let mut line = String::new();

                    while let Ok(bytes) = reader.read_line(&mut line).await {
                        if bytes == 0 { break; }
                        
                        match serde_json::from_str::<IpcRequest>(&line) {
                            Ok(IpcRequest::LogSystem { level, component, message }) => {
                                if let Err(e) = repo.log_system(&level, &component, &message).await {
                                    log::error!("Failed to save LogSystem IPC: {}", e);
                                }
                            }
                            Ok(IpcRequest::Authenticate(req)) => {
                                let response = handle_authenticate(req, &config, &repo).await;
                                if let Ok(resp_json) = serde_json::to_string(&response) {
                                    let _ = writer.write_all(format!("{}\n", resp_json).as_bytes()).await;
                                }
                            }
                            Err(e) => {
                                let response = IpcResponse::Error(format!("Invalid IPC JSON: {}", e));
                                if let Ok(resp_json) = serde_json::to_string(&response) {
                                    let _ = writer.write_all(format!("{}\n", resp_json).as_bytes()).await;
                                }
                            }
                        };
                        line.clear();
                    }
                });
            }
            Err(e) => {
                log::error!("Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_authenticate(req: mitm_common::ipc::AuthRequest, config: &DBConfig, repo: &db::Repository) -> IpcResponse {
    // 1. In-Memory Check
    for admin in &config.admins {
        if admin.username.as_bytes().ct_eq(req.username.as_bytes()).unwrap_u8() == 1
            && admin.token.as_bytes().ct_eq(req.token.as_bytes()).unwrap_u8() == 1 {
            return IpcResponse::AuthenticateResult(AuthResponse {
                success: true,
                username: req.username,
                roles: vec!["ADMIN".to_string()],
                error_message: None,
            });
        }
    }

    // 2. DB Fallback Check
    let is_valid = match repo.check_password(&req.username, &req.token).await {
        Ok(valid) => valid,
        Err(e) => {
            log::error!("Database check error: {}", e);
            return IpcResponse::Error("Internal authentication error".to_string());
        }
    };

    if is_valid {
        let roles = repo.get_user_roles(&req.username).await.unwrap_or_else(|e| {
            log::error!("Failed to fetch roles for {}: {}", req.username, e);
            vec![]
        });
        IpcResponse::AuthenticateResult(AuthResponse {
            success: true,
            username: req.username,
            roles,
            error_message: None,
        })
    } else {
        IpcResponse::AuthenticateResult(AuthResponse {
            success: false,
            username: req.username,
            roles: vec![],
            error_message: Some("Invalid credentials".to_string()),
        })
    }
}
