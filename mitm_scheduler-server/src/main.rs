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
        fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    log::info!("Scheduler listening for Job events on UDS {:?}", socket_path);

    let repo = Arc::new(repo);
    let _ = repo.log_system("INFO", "scheduler-server", &format!("Starting {} v{}", APP_NAME, VERSION)).await;
    let socket_path_str = socket_path.to_string_lossy().to_string();
    let orchestrator = Arc::new(JobOrchestrator::new(repo.clone(), socket_path_str));
    let cron_scheduler = CronScheduler::new(repo.clone(), orchestrator.clone());

    tokio::spawn(async move {
        cron_scheduler.start().await;
    });

    let master_key_str = password.clone();
    let db_config_json = serde_json::to_string(&config.db).unwrap_or_else(|_| "{}".to_string());

    loop {
        match listener.accept().await {
            Ok((mut stream, _)) => {
                let repo = repo.clone();
                let mk = master_key_str.clone();
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
                            Ok(SchedulerRequest::ExecuteJob(job_id)) => {
                                log::info!("API requested ExecuteJob for job {}", job_id);
                                let orch_clone = orch.clone();
                                let repo_clone = repo.clone();
                                tokio::spawn(async move {
                                    if let Ok(prog) = repo_clone.get_program_by_id(job_id).await {
                                        if let Err(e) = orch_clone.run_job(prog).await {
                                            log::error!("Failed to execute job {}: {}", job_id, e);
                                        }
                                    } else {
                                        log::error!("Failed to fetch job {}", job_id);
                                    }
                                });
                            }
                            Ok(SchedulerRequest::StopJob(job_id)) => {
                                log::info!("API requested StopJob for job {}", job_id);
                                // For a full implementation, we would send SIGTERM to the child's PID.
                            }
                            Ok(SchedulerRequest::UpdateJobs) => {
                                log::info!("API requested UpdateJobs, reloading scheduler config");
                            }
                            Err(e) => {
                                log::error!("Invalid Job Status JSON: {}", e);
                            }
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
