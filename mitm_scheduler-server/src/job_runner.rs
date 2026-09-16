use tokio::process::Command;
use std::process::Stdio;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashSet;
use std::env;
use crate::db::{Repository, ScheduledProgram};

pub struct JobOrchestrator {
    repo: Arc<Repository>,
    running_jobs: Arc<Mutex<HashSet<i32>>>,
    socket_path: String,
}

impl JobOrchestrator {
    pub fn new(repo: Arc<Repository>, socket_path: String) -> Self {
        Self {
            repo,
            running_jobs: Arc::new(Mutex::new(HashSet::new())),
            socket_path,
        }
    }

    pub async fn run_job(&self, program: ScheduledProgram) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut running = self.running_jobs.lock().await;
        if running.contains(&program.id) {
            let msg = format!("Job {} is already running, skipping execution", program.name);
            log::info!("{}", msg);
            let _ = self.repo.log_system("WARN", "Scheduler", &msg).await;
            let _ = self.repo.log_job_audit(0, "Scheduler", &msg).await;
            return Ok(());
        }
        running.insert(program.id);
        drop(running);

        let repo = self.repo.clone();
        let running_jobs = self.running_jobs.clone();
        let socket_path = self.socket_path.clone();

        tokio::spawn(async move {
            let run_id = match repo.create_program_run(program.id).await {
                Ok(id) => id,
                Err(e) => {
                    let err_msg = format!("Failed to create run for {}: {}", program.name, e);
                    log::error!("{}", err_msg);
                    let _ = repo.log_system("ERROR", "Scheduler", &err_msg).await;
                    running_jobs.lock().await.remove(&program.id);
                    return;
                }
            };

            log::info!("Starting job {} (RunID: {})", program.name, run_id);
            let _ = repo.log_system("INFO", "Scheduler", &format!("Starting job {}", program.name)).await;

            let args_json = program.args.unwrap_or_else(|| "{}".to_string());
            let mut cmd = Command::new(&program.command);
            cmd.arg(&args_json);
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            
            let whitelist = ["PATH", "SYSTEMROOT", "USERPROFILE", "HOME", "TEMP", "TMP"];
            cmd.env_clear();
            for (key, val) in env::vars() {
                let key_upper = key.to_uppercase();
                if whitelist.contains(&key_upper.as_str()) {
                    cmd.env(key, val);
                }
            }
            
            cmd.env("RUN_ID", run_id.to_string());
            cmd.env("SCHEDULER_SOCKET_PATH", &socket_path);

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    let err_msg = format!("Failed to spawn {}: {}", program.command, e);
                    log::error!("{}", err_msg);
                    let _ = repo.update_program_run(run_id, -1, false, 0).await;
                    let _ = repo.log_system("ERROR", "Scheduler", &err_msg).await;
                    running_jobs.lock().await.remove(&program.id);
                    return;
                }
            };

            let pid = child.id().unwrap_or(0);
            let _ = repo.log_system("DEBUG", "Scheduler", &format!("Job {} started (RunID {}, PID {})", program.name, run_id, pid)).await;
            
            let status = match child.wait().await {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Error waiting for job {}: {}", program.name, e);
                    let _ = repo.update_program_run(run_id, -1, false, pid).await;
                    running_jobs.lock().await.remove(&program.id);
                    return;
                }
            };

            let exit_code = status.code().unwrap_or(-1);
            let success = status.success();

            if success {
                log::info!("Job {} completed successfully (RunID: {})", program.name, run_id);
            } else {
                log::error!("Job {} failed with code {} (RunID: {})", program.name, exit_code, run_id);
            }

            let _ = repo.update_program_run(run_id, exit_code, success, pid).await;
            running_jobs.lock().await.remove(&program.id);
        });

        Ok(())
    }
}
