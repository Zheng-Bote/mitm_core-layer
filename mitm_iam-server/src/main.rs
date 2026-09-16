mod db;

use std::env;
use std::fs;
use std::path::PathBuf;
use tokio::net::UnixListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use mitm_common::config::{load_config, DBConfig};
use mitm_common::ipc::{IpcRequest, IpcResponse, AuthResponse};
use subtle::ConstantTimeEq;

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

    let socket_path = PathBuf::from("/tmp/mitm_iam.sock");
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
                        
                        let response = match serde_json::from_str::<IpcRequest>(&line) {
                            Ok(IpcRequest::Authenticate(req)) => {
                                handle_authenticate(req, &config, &repo).await
                            }
                            Err(e) => IpcResponse::Error(format!("Invalid IPC JSON: {}", e))
                        };

                        if let Ok(resp_json) = serde_json::to_string(&response) {
                            let _ = writer.write_all(format!("{}\n", resp_json).as_bytes()).await;
                        }
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
    match repo.check_password(&req.username, &req.token).await {
        Ok(true) => {
            IpcResponse::AuthenticateResult(AuthResponse {
                success: true,
                username: req.username,
                roles: vec!["ADMIN".to_string()], // In a full implementation, we'd fetch actual roles
                error_message: None,
            })
        }
        Ok(false) => {
            IpcResponse::AuthenticateResult(AuthResponse {
                success: false,
                username: req.username,
                roles: vec![],
                error_message: Some("Invalid credentials".to_string()),
            })
        }
        Err(e) => {
            log::error!("Database check error: {}", e);
            IpcResponse::Error("Internal authentication error".to_string())
        }
    }
}
