mod db;
mod job_runner;
mod scheduler;

use std::env;
use std::fs;
use std::path::PathBuf;
use tokio::net::UnixListener;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::sync::Arc;
use mitm_common::config::load_config;
use crate::job_runner::JobOrchestrator;
use crate::scheduler::CronScheduler;

const APP_NAME: &str = "MitM Scheduler Server";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let args: Vec<String> = env::args().collect();
    let config_param = args.get(1).map(|s| s.as_str());
    
    let password = env::var("MASTER_KEY").unwrap_or_else(|_| "".to_string());
    
    // Decode MASTER_KEY if it is exactly 44 characters (Base64 encoding of 32 bytes)
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

    let repo = db::Repository::new(&config).await.map_err(|e| e as Box<dyn std::error::Error>)?;
    log::info!("Scheduler connected to PostgreSQL at {}:{}", config.db.host, config.db.port);

    let socket_dir = PathBuf::from(&config.socket_dir);
    if !socket_dir.exists() {
        fs::create_dir_all(&socket_dir)?;
    }

    let socket_path = socket_dir.join("mitm_scheduler.sock");
    if socket_path.exists() {
        if std::os::unix::net::UnixStream::connect(&socket_path).is_ok() {
            log::error!("FATAL: Another instance of Scheduler server is already running! Exiting.");
            std::process::exit(1);
        }
        fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    log::info!("Scheduler listening for Job events on UDS {:?}", socket_path);

    let repo = Arc::new(repo);
    let success_msg = format!("Starting {} (v{})", APP_NAME, VERSION);
    log::info!("{}", success_msg);
    let _ = repo.log_system("INFO", "scheduler-server", &success_msg).await;

    let socket_path_str = socket_path.to_string_lossy().to_string();
    let orchestrator = Arc::new(JobOrchestrator::new(repo.clone(), socket_path_str));
    let cron_scheduler = CronScheduler::new(repo.clone(), orchestrator.clone());

    tokio::spawn(async move {
        cron_scheduler.start().await;
    });

    let master_key_str = password.clone();
    let db_config_json = serde_json::to_string(&config).unwrap_or_else(|_| "{}".to_string());

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                let shutdown_msg = "Shutting down...";
                log::info!("{}", shutdown_msg);
                let _ = repo.log_system("INFO", "scheduler-server", shutdown_msg).await;
                orchestrator.stop_all().await;
                // Wait briefly for jobs to receive the signal
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                break;
            }
            _ = sigterm.recv() => {
                let shutdown_msg = "Shutting down...";
                log::info!("{}", shutdown_msg);
                let _ = repo.log_system("INFO", "scheduler-server", shutdown_msg).await;
                orchestrator.stop_all().await;
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                break;
            }
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((mut stream, _)) => {
                        let repo = repo.clone();
                        let mk = master_key_str.clone();
                        let kek_clone = kek.clone();
                        let db_cfg = db_config_json.clone();
                        let orch = orchestrator.clone();
                        tokio::spawn(async move {
                            let (reader, mut writer) = stream.split();
                            let mut reader = BufReader::new(reader);
                            let mut line = String::new();

                            while let Ok(bytes) = reader.read_line(&mut line).await {
                                if bytes == 0 { break; }
                                
                                use mitm_common::ipc::SchedulerRequest;
                                match serde_json::from_str::<SchedulerRequest>(&line) {
                                    Ok(SchedulerRequest::RunImmediateJob(req)) => {
                                        log::info!("RunImmediateJob: {}", req.command);
                                        let orch = orch.clone();
                                        tokio::spawn(async move {
                                            use crate::db::ScheduledProgram;
                                            let prog = ScheduledProgram {
                                                id: -1,
                                                name: "Immediate_Trigger".to_string(),
                                                command: req.command,
                                                args: if req.args.is_empty() { None } else { serde_json::from_str(&req.args).ok() },
                                                cron_expr: "".to_string(),
                                                restart_on_exit: false,
                                            };
                                            let _ = orch.run_job(prog).await;
                                        });
                                    }
                                    Ok(SchedulerRequest::Status(event)) => {
                                        log::info!("Job Event [Run {}]: {} - {}", event.run_id, event.status, event.message);
                                        if let Err(e) = repo.log_job_event(event.run_id, &event.status, &event.message, event.progress).await {
                                            log::error!("Failed to log event to DB: {}", e);
                                        }
                                    }
                                    Ok(SchedulerRequest::Audit(event)) => {
                                        log::info!("AUDIT [Run {}]: {} - {}", event.run_id, event.component, event.message);
                                        if let Err(e) = repo.log_job_audit(event.run_id, &event.component, &event.message).await {
                                            log::error!("Failed to log audit event to DB: {}", e);
                                        }
                                    }
                                    Ok(SchedulerRequest::GetCredentials(req)) => {
                                        use mitm_common::ipc::CredentialsResponse;
                                        use tokio::io::AsyncWriteExt;
                                        log::info!("GetCredentials [Run {}]", req.run_id);
                                        let resp = CredentialsResponse {
                                            master_key: mk.clone(),
                                            db_config_json: db_cfg.clone(),
                                        };
                                        if let Ok(resp_json) = serde_json::to_string(&resp) {
                                            let mut out = resp_json;
                                            out.push('\n');
                                            let _ = writer.write_all(out.as_bytes()).await;
                                        }
                                    }
                                    Ok(SchedulerRequest::ExecuteJob { job_name }) => {
                                        log::info!("API requested ExecuteJob for job {}", job_name);
                                        let orch_clone = orch.clone();
                                        let repo_clone = repo.clone();
                                        tokio::spawn(async move {
                                            if let Ok(prog) = repo_clone.get_program_by_name(&job_name).await {
                                                if let Err(e) = orch_clone.run_job(prog).await {
                                                    let err_msg = format!("Failed to execute job {}: {}", job_name, e);
                                                    log::error!("{}", err_msg);
                                                    let _ = repo_clone.log_system("ERROR", "scheduler", &err_msg).await;
                                                }
                                            } else {
                                                let err_msg = format!("Failed to fetch job {}", job_name);
                                                log::error!("{}", err_msg);
                                                let _ = repo_clone.log_system("ERROR", "scheduler", &err_msg).await;
                                            }
                                        });
                                    }
                                    Ok(SchedulerRequest::StopJob { job_name }) => {
                                        log::info!("API requested StopJob for job {}", job_name);
                                        let repo_c = repo.clone(); let orch_c = orch.clone(); tokio::spawn(async move { if let Ok(prog) = repo_c.get_program_by_name(&job_name).await { orch_c.stop_job(prog.id).await; } });
                                    }
                                    Ok(SchedulerRequest::UpdateJobs) => {
                                        log::info!("API requested UpdateJobs, reloading scheduler config");
                                    }
                                    Ok(SchedulerRequest::CryptoEncrypt { wrapped_dek, plaintext }) => {
                                        use mitm_common::ipc::IpcResponse;
                                        use tokio::io::AsyncWriteExt;
                                        let resp = match mitm_common::crypto::envelope_encrypt(&kek_clone, &wrapped_dek, &plaintext) {
                                            Ok((nonce, ciphertext)) => IpcResponse::CryptoEncryptResult { nonce, ciphertext },
                                            Err(e) => IpcResponse::Error(e.to_string()),
                                        };
                                        if let Ok(resp_json) = serde_json::to_string(&resp) {
                                            let _ = writer.write_all(format!("{}\n", resp_json).as_bytes()).await;
                                        }
                                    }
                                    Ok(SchedulerRequest::CryptoDecrypt { wrapped_dek, nonce, ciphertext }) => {
                                        use mitm_common::ipc::IpcResponse;
                                        use tokio::io::AsyncWriteExt;
                                        let resp = match mitm_common::crypto::envelope_decrypt(&kek_clone, &wrapped_dek, &nonce, &ciphertext) {
                                            Ok(plaintext) => IpcResponse::CryptoDecryptResult { plaintext },
                                            Err(e) => IpcResponse::Error(e.to_string()),
                                        };
                                        if let Ok(resp_json) = serde_json::to_string(&resp) {
                                            let _ = writer.write_all(format!("{}\n", resp_json).as_bytes()).await;
                                        }
                                    }
                                    Err(e) => {
                                        let err_msg = format!("Invalid SchedulerRequest JSON: {}", e);
                                        log::error!("{}", err_msg);
                                        let _ = repo.log_system("ERROR", "scheduler", &err_msg).await;
                                    }
                                }
                                line.clear();
                            }
                        });
                    }
                    Err(e) => {
                        let err_msg = format!("Failed to accept connection: {}", e);
                        log::error!("{}", err_msg);
                        let _ = repo.log_system("ERROR", "scheduler", &err_msg).await;
                    }
                }
            }
        }
    }
    Ok(())
}
