mod db;
mod job_runner;

use std::env;
use std::fs;
use std::path::PathBuf;
use tokio::net::UnixListener;
use tokio::io::{AsyncBufReadExt, BufReader};
use mitm_common::config::load_config;
use mitm_common::ipc::StatusEvent;

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
    log::info!("Scheduler connected to PostgreSQL at {}:{}", config.db.host, config.db.port);

    let socket_path = PathBuf::from("/tmp/mitm.sock");
    if socket_path.exists() {
        fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    log::info!("Scheduler listening for Job events on UDS {:?}", socket_path);

    let repo = std::sync::Arc::new(repo);

    loop {
        match listener.accept().await {
            Ok((mut stream, _)) => {
                let repo = repo.clone();
                tokio::spawn(async move {
                    let (reader, _writer) = stream.split();
                    let mut reader = BufReader::new(reader);
                    let mut line = String::new();

                    while let Ok(bytes) = reader.read_line(&mut line).await {
                        if bytes == 0 { break; }
                        
                        match serde_json::from_str::<StatusEvent>(&line) {
                            Ok(event) => {
                                log::info!("Job Event [Run {}]: {} - {}", event.run_id, event.status, event.message);
                                if let Err(e) = repo.log_job_event(event.run_id, &event.component, &event.status, &event.message).await {
                                    log::error!("Failed to log event to DB: {}", e);
                                }
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
