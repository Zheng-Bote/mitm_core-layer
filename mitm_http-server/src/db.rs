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
}
