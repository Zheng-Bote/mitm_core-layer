/*
 * SPDX-License-Identifier: Apache-2.0
 */

use tokio::process::Command;
use std::process::Stdio;
use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use std::collections::{HashMap, HashSet};
use std::env;
use std::os::unix::process::ExitStatusExt;
use crate::db::{Repository, ScheduledProgram};

pub struct JobOrchestrator {
    repo: Arc<Repository>,
    running_jobs: Arc<Mutex<HashMap<i32, u32>>>,
    socket_path: String,
    shutting_down: Arc<AtomicBool>,
}

impl JobOrchestrator {
    pub fn new(repo: Arc<Repository>, socket_path: String) -> Self {
        Self {
            repo,
            running_jobs: Arc::new(Mutex::new(HashMap::new())),
            socket_path,
            shutting_down: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn stop_job(&self, program_id: i32) {
        let running = self.running_jobs.lock().await;
        let pid = running.get(&program_id).copied();
        drop(running);

        if let Some(pid) = pid {
            if pid > 0 {
                log::info!("Sending SIGTERM to job {} (PID: {})", program_id, pid);
                unsafe { libc::kill(pid as i32, libc::SIGTERM); }

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
                        unsafe { libc::kill(pid as i32, libc::SIGKILL); }
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
        self.shutting_down.store(true, Ordering::SeqCst);
        stop_running_jobs(&self.running_jobs, tokio::time::Duration::from_secs(10)).await;
    }

    pub async fn run_job(&self, program: ScheduledProgram) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.shutting_down.load(Ordering::SeqCst) {
            log::info!("Scheduler is shutting down, skipping execution of job {}", program.name);
            return Ok(());
        }
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
        let shutting_down = self.shutting_down.clone();

        tokio::spawn(async move {
            loop {
                if shutting_down.load(Ordering::SeqCst) {
                    running_jobs.lock().await.remove(&program.id);
                    return;
                }
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

                // Read the pipes until EOF; an unread pipe blocks the job after 64 KiB of output (#96).
                // Not awaited: a descendant keeping a pipe open must not delay the run's completion.
                if let Some(out) = child.stdout.take() {
                    tokio::spawn(drain_job_output(out, program.name.clone(), run_id, "stdout"));
                }
                if let Some(err) = child.stderr.take() {
                    tokio::spawn(drain_job_output(err, program.name.clone(), run_id, "stderr"));
                }

                let pid = child.id().unwrap_or(0);
                running_jobs.lock().await.insert(program.id, pid);
                if shutting_down.load(Ordering::SeqCst) && pid > 0 {
                    unsafe { libc::kill(pid as i32, libc::SIGTERM); }
                }
                let _ = repo.update_run_pid(run_id, pid).await;
                let _ = repo.log_system("DEBUG", "Scheduler", &format!("Job {} started (RunID {}, PID {})", program.name, run_id, pid)).await;
                
                let status = match child.wait().await {
                    Ok(s) => s,
                    Err(e) => {
                        let err_msg = format!("Error waiting for job {}: {}", program.name, e);
                        log::error!("{}", err_msg);
                        let _ = repo.log_system("ERROR", "Scheduler", &err_msg).await;
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
                    let err_msg = format!("Job {} failed with code {} (RunID: {})", program.name, exit_code, run_id);
                    log::error!("{}", err_msg);
                    let _ = repo.log_system("ERROR", "Scheduler", &err_msg).await;
                }

                let _ = repo.update_program_run(run_id, exit_code, success, pid).await;

                if program.restart_on_exit && !success && !was_killed && !shutting_down.load(Ordering::SeqCst) {
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

/// Maximum number of bytes of a single job output line written to the scheduler log.
const MAX_JOB_OUTPUT_LINE: usize = 4096;

/// Reads a job's stdout or stderr until EOF and writes each line to the scheduler log at DEBUG level.
async fn drain_job_output<R>(reader: R, job_name: String, run_id: i32, stream: &'static str)
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncBufReadExt;
    let mut reader = tokio::io::BufReader::new(reader);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) => break,
            Ok(_) => {
                let line = &buf[..buf.len().min(MAX_JOB_OUTPUT_LINE)];
                let line = String::from_utf8_lossy(line);
                log::debug!("[job {} run {} {}] {}", job_name, run_id, stream, line.trim_end());
            }
            Err(e) => {
                log::warn!("Reading {} of job {} (RunID: {}) failed: {}", stream, job_name, run_id, e);
                break;
            }
        }
    }
}

async fn stop_running_jobs(
    running_jobs: &Arc<Mutex<HashMap<i32, u32>>>,
    grace_timeout: tokio::time::Duration,
) {
    let mut signaled_pids: HashSet<u32> = HashSet::new();
    let deadline = tokio::time::Instant::now() + grace_timeout;

    loop {
        let running: Vec<(i32, u32)> = {
            let lock = running_jobs.lock().await;
            lock.iter().map(|(&id, &pid)| (id, pid)).collect()
        };

        if running.is_empty() {
            log::info!("All running jobs have terminated gracefully.");
            return;
        }

        for (program_id, pid) in &running {
            if *pid > 0 && signaled_pids.insert(*pid) {
                log::info!("Graceful shutdown: Sending SIGTERM to job {} (PID: {})", program_id, pid);
                unsafe { libc::kill(*pid as i32, libc::SIGTERM); }
            }
        }

        if tokio::time::Instant::now() >= deadline {
            for (program_id, pid) in running {
                if pid > 0 {
                    log::warn!(
                        "Job {} (PID: {}) did not exit within {:?} during shutdown, sending SIGKILL",
                        program_id,
                        pid,
                        grace_timeout
                    );
                    unsafe { libc::kill(pid as i32, libc::SIGKILL); }
                }
            }
            break;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    let kill_deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(2);
    while tokio::time::Instant::now() < kill_deadline {
        if running_jobs.lock().await.is_empty() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{drain_job_output, stop_running_jobs};
    use std::process::Stdio;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::time::Duration;

    #[tokio::test]
    async fn stop_running_jobs_terminates_and_waits_for_drain() {
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("trap 'exit 0' TERM; while :; do sleep 0.1; done")
            .spawn()
            .unwrap();
        let pid = child.id().unwrap();

        let running_jobs = Arc::new(Mutex::new(HashMap::from([(42, pid)])));
        let running_jobs_reaper = running_jobs.clone();
        tokio::spawn(async move {
            let _ = child.wait().await;
            running_jobs_reaper.lock().await.remove(&42);
        });

        stop_running_jobs(&running_jobs, Duration::from_secs(2)).await;
        assert!(running_jobs.lock().await.is_empty(), "running_jobs must be drained after stop_running_jobs");
    }

    /// Spawns a child that writes 200 KiB to stdout and stderr (more than a 64 KiB pipe buffer).
    fn spawn_chatty_child() -> tokio::process::Child {
        tokio::process::Command::new("sh")
            .arg("-c")
            .arg("i=0; while [ $i -lt 2000 ]; do echo \"line $i $(printf '%090d' 0)\"; echo \"err $i $(printf '%090d' 0)\" >&2; i=$((i+1)); done; exit 3")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }

    #[tokio::test]
    async fn drained_job_with_large_output_runs_to_completion() {
        let mut child = spawn_chatty_child();
        tokio::spawn(drain_job_output(child.stdout.take().unwrap(), "chatty".to_string(), 1, "stdout"));
        tokio::spawn(drain_job_output(child.stderr.take().unwrap(), "chatty".to_string(), 1, "stderr"));

        let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
            .await
            .expect("job must not block on its output pipes")
            .unwrap();
        assert_eq!(status.code(), Some(3), "exit code must be preserved");
    }

    #[tokio::test]
    async fn undrained_job_with_large_output_blocks() {
        // Documents the bug of #96: without draining, the child blocks once the pipe buffer is full.
        let mut child = spawn_chatty_child();
        let result = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
        assert!(result.is_err(), "undrained child unexpectedly finished");
        child.kill().await.unwrap();
    }
}
