use sqlx::postgres::{PgPool, PgPoolOptions};
use std::error::Error;
use mitm_common::config::DBConfig;
use mitm_common::crypto;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::{rngs::OsRng, RngCore};

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

    pub async fn get_admin_role_id(&self) -> Result<Option<i32>, Box<dyn Error>> {
        let record = sqlx::query!("SELECT id FROM roles WHERE name = 'ADMIN'")
            .fetch_optional(&self.pool)
            .await?;
        Ok(record.map(|r| r.id))
    }

    pub async fn create_user(&self, username: &str, password: &str) -> Result<i32, Box<dyn Error>> {
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);
        
        let hash = crypto::derive_key(password.as_bytes(), &salt)?;
        let hash_str = format!("{}:{}", BASE64.encode(salt), BASE64.encode(hash));

        let record = sqlx::query!(
            "INSERT INTO admin_users (username, password_hash, is_active) VALUES ($1, $2, true) RETURNING id",
            username,
            hash_str
        )
        .fetch_one(&self.pool)
        .await?;
        
        Ok(record.id)
    }

    pub async fn assign_role(&self, user_id: i32, role_id: i32) -> Result<(), Box<dyn Error>> {
        sqlx::query!(
            "INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            user_id,
            role_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_user_id(&self, username: &str) -> Result<Option<i32>, Box<dyn Error>> {
        let record = sqlx::query!("SELECT id FROM admin_users WHERE username = $1", username)
            .fetch_optional(&self.pool)
            .await?;
        Ok(record.map(|r| r.id))
    }

    pub async fn check_password(&self, username: &str, password: &str) -> Result<bool, Box<dyn Error>> {
        let record = sqlx::query!(
            "SELECT password_hash FROM admin_users WHERE username = $1 AND is_active = true",
            username
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = record {
            let parts: Vec<&str> = row.password_hash.split(':').collect();
            if parts.len() != 2 {
                return Ok(false);
            }
            let salt = BASE64.decode(parts[0])?;
            let stored_hash = BASE64.decode(parts[1])?;

            let derived_hash = crypto::derive_key(password.as_bytes(), &salt)?;
            
            // Constant time compare
            let mut result = 0;
            if derived_hash.len() == stored_hash.len() {
                for (a, b) in derived_hash.iter().zip(stored_hash.iter()) {
                    result |= a ^ b;
                }
                return Ok(result == 0);
            }
        }
        Ok(false)
    }
}

pub async fn bootstrap_admins(repo: &Repository, config: &DBConfig) {
    let admin_role_id = match repo.get_admin_role_id().await {
        Ok(Some(id)) => id,
        Ok(None) => {
            log::warn!("ADMIN role not found in database for bootstrap");
            return;
        }
        Err(e) => {
            log::error!("Failed to get roles for bootstrap: {}", e);
            return;
        }
    };

    for admin_cfg in &config.admins {
        let user_id = match repo.get_user_id(&admin_cfg.username).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                match repo.create_user(&admin_cfg.username, &admin_cfg.token).await {
                    Ok(id) => {
                        log::info!("Created initial admin user: {}", admin_cfg.username);
                        id
                    }
                    Err(e) => {
                        log::error!("Failed to create admin user {}: {}", admin_cfg.username, e);
                        continue;
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to get user {}: {}", admin_cfg.username, e);
                continue;
            }
        };

        if let Err(e) = repo.assign_role(user_id, admin_role_id).await {
            log::error!("Failed to assign ADMIN role to user {}: {}", admin_cfg.username, e);
        }
    }
}
