mod db;

use std::env;
use std::fs;
use std::path::PathBuf;
use tokio::net::UnixListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::signal;
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
    
    // Decode MASTER_KEY if it is exactly 44 characters (Base64 encoding of 32 bytes), matching Go collectors
    let kek = if password.len() == 44 {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
        BASE64.decode(&password).unwrap_or_else(|_| password.as_bytes().to_vec())
    } else {
        password.as_bytes().to_vec()
    };
    
    let config = match load_config(config_param, &password) {
        Ok(cfg) => cfg,
        Err(e) => {
            log::error!("Failed to load configuration: {}", e);
            return Err(e);
        }
    };

    let repo = db::Repository::new(&config).await?;
    log::info!("Connected to PostgreSQL at {}:{}", config.db.host, config.db.port);

    db::bootstrap_admins(&repo, &config, &kek).await;

    // Log startup
    let success_msg = format!("Starting {} (v{})", APP_NAME, VERSION);
    let _ = repo.log_system("INFO", "iam-server", &success_msg).await;
    log::info!("{}", success_msg);

    // Ensure socket directory exists
    let socket_dir = PathBuf::from(&config.socket_dir);
    if !socket_dir.exists() {
        fs::create_dir_all(&socket_dir)?;
    }

    let socket_path = socket_dir.join("mitm_iam.sock");
    if socket_path.exists() {
        if std::os::unix::net::UnixStream::connect(&socket_path).is_ok() {
            log::error!("FATAL: Another instance of IAM server is already running! Exiting.");
            std::process::exit(1);
        }
        fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    log::info!("IAM Server listening on UDS {:?}", socket_path);

    let config = std::sync::Arc::new(config);
    let repo_arc = std::sync::Arc::new(repo);
    let repo_clone = repo_arc.clone();
    let kek_arc = std::sync::Arc::new(kek);

    let _server_task = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut stream, _)) => {
                    let config = config.clone();
                    let repo = repo_clone.clone();
                    let kek = kek_arc.clone();
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
                                    let response = handle_authenticate(req, &config, &repo, &kek).await;
                                    if let Ok(resp_json) = serde_json::to_string(&response) {
                                        let _ = writer.write_all(format!("{}\n", resp_json).as_bytes()).await;
                                    }
                                }

                                Err(e) => {
                                    let err_msg = format!("Invalid IPC JSON: {}", e);
                                    log::error!("{}", err_msg);
                drop(e);
                                    let _ = repo.log_system("ERROR", "iam-server", &err_msg).await;
                                    let response = IpcResponse::Error(err_msg);
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
                    let err_msg = format!("Failed to accept connection: {}", e);
                    log::error!("{}", err_msg);
                drop(e);
                    let _ = repo_clone.log_system("ERROR", "iam-server", &err_msg).await;
                }
            }
        }
    });

    // Wait for termination
    tokio::select! {
        _ = signal::ctrl_c() => {},
        _ = async {
            #[cfg(unix)]
            {
                if let Ok(mut sig) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
                    sig.recv().await;
                }
            }
            #[cfg(not(unix))]
            std::future::pending::<()>().await;
        } => {},
    }

    let shutdown_msg = "Shutting down...";
    log::info!("{}", shutdown_msg);
    let _ = repo_arc.log_system("INFO", "iam-server", shutdown_msg).await;

    Ok(())
}

async fn handle_authenticate(req: mitm_common::ipc::AuthRequest, config: &DBConfig, repo: &db::Repository, kek: &[u8]) -> IpcResponse {
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
    let is_valid = match repo.check_password(&req.username, &req.token).await.map_err(|e| e.to_string()) {
        Ok(valid) => valid,
        Err(e) => {
            let err_msg = format!("Database check error: {}", e);
            log::error!("{}", err_msg);
                drop(e);
            let _ = repo.log_system("ERROR", "iam-server", &err_msg).await;
            return IpcResponse::Error("Internal authentication error".to_string());
        }
    };

    if is_valid {
        let roles = match repo.get_user_roles(&req.username, kek).await.map_err(|e| e.to_string()) {
            Ok(r) => r,
            Err(e) => {
                let err_msg = format!("Failed to fetch roles for {}: {}", req.username, e);
                log::error!("{}", err_msg);
                drop(e);
                let _ = repo.log_system("ERROR", "iam-server", &err_msg).await;
                vec![]
            }
        };
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
