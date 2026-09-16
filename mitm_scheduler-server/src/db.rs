use sqlx::postgres::{PgPool, PgPoolOptions};
use std::error::Error;
use mitm_common::config::DBConfig;

#[derive(Clone)]
pub struct Repository {
    pub pool: PgPool,
}

impl Repository {
    pub async fn new(config: &DBConfig) -> Result<Self, Box<dyn Error>> {
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

    pub async fn log_job_event(&self, run_id: i32, component: &str, status: &str, message: &str) -> Result<(), Box<dyn Error>> {
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
}
