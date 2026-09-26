/*
 * SPDX-License-Identifier: Apache-2.0
 */

use std::sync::Arc;
use tokio::time::{sleep, Duration};
use cron::Schedule;
use std::str::FromStr;
use chrono::Utc;
use crate::db::Repository;
use crate::job_runner::JobOrchestrator;

pub struct CronScheduler {
    repo: Arc<Repository>,
    orchestrator: Arc<JobOrchestrator>,
}

impl CronScheduler {
    pub fn new(repo: Arc<Repository>, orchestrator: Arc<JobOrchestrator>) -> Self {
        Self { repo, orchestrator }
    }

    pub async fn start(&self) {
        log::info!("Starting Cron Scheduler loop...");
        loop {
            let now = Utc::now();
            let programs = match self.repo.get_enabled_programs().await {
                Ok(p) => p,
                Err(e) => {
                    let err_msg = format!("Failed to fetch programs: {}", e);
                    log::error!("{}", err_msg);
                    sleep(Duration::from_secs(60)).await;
                    continue;
                }
            };

            for program in programs {
                let mut c_expr = program.cron_expr.clone();
                let parts: Vec<&str> = c_expr.split_whitespace().collect();
                if parts.len() == 5 {
                    c_expr = format!("0 {} *", c_expr);
                }
                if let Ok(schedule) = Schedule::from_str(&c_expr) {
                    if let Some(next) = schedule.upcoming(Utc).next() {
                        let diff = next.signed_duration_since(now).num_seconds();
                        // If the job is due within the next 60 seconds
                        if diff >= 0 && diff < 60 {
                            log::info!("Job {} is due at {}. Triggering orchestrator...", program.name, next);
                            let orch = self.orchestrator.clone();
                            let prog = program.clone();
                            let wait_time = diff as u64;
                            tokio::spawn(async move {
                                if wait_time > 0 {
                                    sleep(Duration::from_secs(wait_time)).await;
                                }
                                let _ = orch.run_job(prog).await;
                            });
                        }
                    }
                } else {
                    log::warn!("Invalid cron expression for job {}: {}", program.name, program.cron_expr);
                }
            }

            // Sleep until the top of the next minute
            let now_secs = Utc::now().timestamp();
            let sleep_secs = 60 - (now_secs % 60);
            sleep(Duration::from_secs(sleep_secs as u64)).await;
        }
    }
}
