/*
 * SPDX-License-Identifier: Apache-2.0
 */

use tokio::process::Command;
use std::process::Stdio;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use std::env;
use std::os::unix::process::ExitStatusExt;
use crate::db::{Repository, ScheduledProgram};

pub struct JobOrchestrator {
    repo: Arc<Repository>,
    running_jobs: Arc<Mutex<HashMap<i32, u32>>>,
    socket_path: String,
}

impl JobOrchestrator {
    pub fn new(repo: Arc<Repository>, socket_path: String) -> Self {
        Self {
            repo,
            running_jobs: Arc::new(Mutex::new(HashMap::new())),
            socket_path,
        }
    }

    pub async fn stop_job(&self, program_id: i32) {
        let running = self.running_jobs.lock().await;
        let pid = running.get(&program_id).copied();
        drop(running);

        if let Some(pid) = pid {
            if pid > 0 {
                log::info!("Sending SIGTERM to job {} (PID: {})", program_id, pid);
                let _ = Command::new("kill").arg("-15").arg(pid.to_string()).output().await;

                // Start 5 second SIGKILL timeout task
                let running_jobs_clone = self.running_jobs.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    let is_still_running = {
                        let lock = running_jobs_clone.lock().await;
                        lock.get(&program_id) == Some(&pid)
                    };
                    if is_still_running {
                        log::warn!("Job {} (PID: {}) did not terminate after 5s, sending SIGKILL", program_id, pid);
                        let _ = Command::new("kill").arg("-9").arg(pid.to_string()).output().await;
                    }
                });
            } else {
                log::warn!("Job {} is running but PID is not yet known", program_id);
            }
        } else {
            log::warn!("Job {} is not currently running", program_id);
        }
    }

    pub async fn stop_all(&self) {
        let running: Vec<(i32, u32)> = {
            let lock = self.running_jobs.lock().await;
            lock.iter().map(|(&id, &pid)| (id, pid)).collect()
        };

        for (program_id, pid) in running {
            if pid > 0 {
                log::info!("Graceful shutdown: Sending SIGTERM to job {} (PID: {})", program_id, pid);
                let _ = Command::new("kill").arg("-15").arg(pid.to_string()).output().await;
            }
        }
    }

    pub async fn run_job(&self, program: ScheduledProgram) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut running = self.running_jobs.lock().await;
        if running.contains_key(&program.id) {
            let msg = format!("Job {} is already running, skipping execution", program.name);
            log::info!("{}", msg);
            let _ = self.repo.log_system("WARN", "Scheduler", &msg).await;
            let _ = self.repo.log_job_audit(0, "Scheduler", &msg).await;
            return Ok(());
        }
        running.insert(program.id, 0);
        drop(running);

        let repo = self.repo.clone();
        let running_jobs = self.running_jobs.clone();
        let socket_path = self.socket_path.clone();

        tokio::spawn(async move {
            loop {
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

                let args_json = program.args.as_ref().map(|v| v.to_string()).unwrap_or_else(|| "{}".to_string());
                let exe_path = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let exe_dir = exe_path.parent().unwrap_or_else(|| std::path::Path::new("."));
                let cmd_path = exe_dir.join(&program.command);
                let mut cmd = Command::new(&cmd_path);
                cmd.current_dir(&exe_dir);
                cmd.arg(&args_json);
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());
                
                let whitelist = ["PATH", "SYSTEMROOT", "USERPROFILE", "HOME", "TEMP", "TMP"];
                cmd.env_clear();
                for (key, val) in env::vars() {
                    let key_upper = key.to_uppercase();
                    if whitelist.contains(&key_upper.as_str()) || key_upper.starts_with("MITM_") {
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
                running_jobs.lock().await.insert(program.id, pid);
                let _ = repo.update_run_pid(run_id, pid).await;
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
                let was_killed = exit_code == -1 || status.core_dumped();

                if success {
                    log::info!("Job {} completed successfully (RunID: {})", program.name, run_id);
                } else {
                    log::error!("Job {} failed with code {} (RunID: {})", program.name, exit_code, run_id);
                }

                let _ = repo.update_program_run(run_id, exit_code, success, pid).await;

                if program.restart_on_exit && !success && !was_killed {
                    log::info!("Restarting job {} due to failure (restart_on_exit=true)", program.name);
                    // Do not remove from running_jobs, just loop again
                } else {
                    running_jobs.lock().await.remove(&program.id);
                    break;
                }
            }
        });

        Ok(())
    }
}
