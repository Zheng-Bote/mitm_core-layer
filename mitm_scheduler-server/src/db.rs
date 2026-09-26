/*
 * SPDX-License-Identifier: Apache-2.0
 */

use sqlx::postgres;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::error::Error;
use mitm_common::config::DBConfig;

#[derive(Clone)]
pub struct Repository {
    pub pool: PgPool,
}

impl Repository {
    pub async fn new(config: &DBConfig) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let db_url = format!(
            "postgres://{}:{}@{}:{}/{}?sslmode={}",
            config.db.user,
            config.db.password,
            config.db.host,
            config.db.port,
            config.db.database,
            if config.db.sslmode { "require" } else { "disable" }
        );

        let pool = PgPoolOptions::new()
            .max_connections(config.db.max_conns)
            .connect(&db_url)
            .await?;

        Ok(Self { pool })
    }

    pub async fn log_system(&self, level: &str, component: &str, message: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        sqlx::query(
            "INSERT INTO system_logs (level, component, message) VALUES ($1, $2, $3)",
        )
        .bind(level)
        .bind(component)
        .bind(message)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn log_job_event(&self, run_id: i32, status: &str, message: &str, progress: i32) -> Result<(), Box<dyn Error + Send + Sync>> {
        sqlx::query(
            "INSERT INTO job_status_events (run_id, status, message, progress) VALUES ($1, $2, $3, $4)",
        )
        .bind(run_id)
        .bind(status)
        .bind(message)
        .bind(progress)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn log_job_audit(&self, run_id: i32, component: &str, message: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        sqlx::query(
            "INSERT INTO job_audit_logs (run_id, component, message) VALUES ($1, $2, $3)",
        )
        .bind(run_id)
        .bind(component)
        .bind(message)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_enabled_programs(&self) -> Result<Vec<ScheduledProgram>, Box<dyn Error + Send + Sync>> {
        let records: Vec<(i32, String, String, Option<serde_json::Value>, String, bool, bool)> = sqlx::query_as(
            "SELECT id, name, command, args, cron_expr, enabled, restart_on_exit FROM scheduled_programs WHERE enabled = true",
        )
        .fetch_all(&self.pool)
        .await?;

        let programs = records.into_iter().map(|r| ScheduledProgram {
            id: r.0,
            name: r.1,
            command: r.2,
            args: r.3,
            cron_expr: r.4,
            restart_on_exit: r.6,
        }).collect();

        Ok(programs)
    }

    pub async fn create_program_run(&self, program_id: i32) -> Result<i32, Box<dyn Error + Send + Sync>> {
        let record: (i32,) = if program_id <= 0 {
            sqlx::query_as(
                "INSERT INTO program_runs (pid, started_at) VALUES (0, CURRENT_TIMESTAMP) RETURNING id",
            )
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                "INSERT INTO program_runs (program_id, pid, started_at) VALUES ($1, 0, CURRENT_TIMESTAMP) RETURNING id",
            )
            .bind(program_id)
            .fetch_one(&self.pool)
            .await?
        };
        Ok(record.0)
    }
    
    pub async fn update_run_pid(&self, run_id: i32, pid: u32) -> Result<(), Box<dyn Error + Send + Sync>> {
        sqlx::query("UPDATE program_runs SET pid = $1 WHERE id = $2")
            .bind(pid as i32)
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_program_run(&self, run_id: i32, exit_code: i32, success: bool, pid: u32) -> Result<(), Box<dyn Error + Send + Sync>> {
        sqlx::query(
            "UPDATE program_runs SET finished_at = CURRENT_TIMESTAMP, exit_code = $1, success = $2, pid = $3 WHERE id = $4",
        )
        .bind(exit_code)
        .bind(success)
        .bind(pid as i32)
        .bind(run_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }



    pub async fn get_program_by_name(&self, name: &str) -> Result<ScheduledProgram, Box<dyn Error + Send + Sync>> {
        let r: (i32, String, String, Option<serde_json::Value>, String, bool, bool) = sqlx::query_as(
            "SELECT id, name, command, args, cron_expr, enabled, restart_on_exit FROM scheduled_programs WHERE name = $1",
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await?;

        Ok(ScheduledProgram {
            id: r.0,
            name: r.1,
            command: r.2,
            args: r.3,
            cron_expr: r.4,
            restart_on_exit: r.6,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ScheduledProgram {
    pub id: i32,
    pub name: String,
    pub command: String,
    pub cron_expr: String,
    pub args: Option<serde_json::Value>,
    pub restart_on_exit: bool,
}
