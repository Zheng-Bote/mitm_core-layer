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

    pub async fn log_job_event(&self, run_id: i32, component: &str, status: &str, message: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        sqlx::query(
            "INSERT INTO system_logs (run_id, component, log_level, message) VALUES ($1, $2, 'INFO', $3)",
        )
        .bind(run_id)
        .bind(component)
        .bind(format!("[{}] {}", status, message))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_enabled_programs(&self) -> Result<Vec<ScheduledProgram>, Box<dyn Error + Send + Sync>> {
        let records: Vec<(i32, String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, name, command, cron_expr, args FROM programs WHERE is_enabled = true",
        )
        .fetch_all(&self.pool)
        .await?;

        let programs = records.into_iter().map(|r| ScheduledProgram {
            id: r.0,
            name: r.1,
            command: r.2,
            cron_expr: r.3,
            args: r.4,
        }).collect();

        Ok(programs)
    }

    pub async fn create_program_run(&self, program_id: i32) -> Result<i32, Box<dyn Error + Send + Sync>> {
        let record: (i32,) = sqlx::query_as(
            "INSERT INTO program_runs (program_id, start_time, status) VALUES ($1, CURRENT_TIMESTAMP, 'RUNNING') RETURNING id",
        )
        .bind(program_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(record.0)
    }

    pub async fn update_program_run(&self, run_id: i32, exit_code: i32, success: bool, pid: u32) -> Result<(), Box<dyn Error + Send + Sync>> {
        let status = if success { "SUCCESS" } else { "FAILED" };
        sqlx::query(
            "UPDATE program_runs SET end_time = CURRENT_TIMESTAMP, exit_code = $1, status = $2, pid = $3 WHERE id = $4",
        )
        .bind(exit_code)
        .bind(status)
        .bind(pid as i32)
        .bind(run_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ScheduledProgram {
    pub id: i32,
    pub name: String,
    pub command: String,
    pub cron_expr: String,
    pub args: Option<String>,
}
