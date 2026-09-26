/*
 * SPDX-License-Identifier: Apache-2.0
 */

use sqlx::postgres;
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
        let record: Option<(i32,)> = sqlx::query_as("SELECT id FROM roles WHERE name = 'ADMIN'")
            .fetch_optional(&self.pool)
            .await?;
        Ok(record.map(|r| r.0))
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

    pub async fn create_user(&self, username: &str, password: &str) -> Result<i32, Box<dyn Error>> {
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);
        
        let hash = crypto::derive_key(password.as_bytes(), &salt)?;
        let hash_str = format!("{}:{}", BASE64.encode(salt), BASE64.encode(hash));

        let record: (i32,) = sqlx::query_as(
            "INSERT INTO admin_users (username, password_hash, is_active) VALUES ($1, $2, true) RETURNING id",
        )
        .bind(username)
        .bind(hash_str)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(record.0)
    }

    pub async fn assign_role(&self, user_id: i32, role_id: i32, kek: &[u8]) -> Result<(), Box<dyn Error>> {
        let mut role_ids: Vec<i32> = match sqlx::query_as::<_, (Vec<u8>, Vec<u8>, Vec<u8>)>("SELECT wrapped_dek, nonce, encrypted_roles FROM user_roles_encrypted WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool).await? {
                Some((wrapped_dek, nonce, encrypted_roles)) => {
                    let plaintext = crypto::envelope_decrypt(kek, &wrapped_dek, &nonce, &encrypted_roles)?;
                    serde_json::from_slice(&plaintext)?
                },
                None => vec![]
            };

        if !role_ids.contains(&role_id) {
            role_ids.push(role_id);
            let roles_json = serde_json::to_vec(&role_ids)?;

            let wrapped_dek = crypto::generate_wrapped_dek(kek)?;
            let (ciphertext, nonce) = crypto::envelope_encrypt(kek, &wrapped_dek, &roles_json)?;

            sqlx::query(
                "INSERT INTO user_roles_encrypted (user_id, wrapped_dek, nonce, encrypted_roles) 
                 VALUES ($1, $2, $3, $4) 
                 ON CONFLICT (user_id) DO UPDATE SET 
                 wrapped_dek = EXCLUDED.wrapped_dek, 
                 nonce = EXCLUDED.nonce, 
                 encrypted_roles = EXCLUDED.encrypted_roles, 
                 updated_at = CURRENT_TIMESTAMP"
            )
            .bind(user_id)
            .bind(wrapped_dek)
            .bind(nonce)
            .bind(ciphertext)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn get_user_id(&self, username: &str) -> Result<Option<i32>, Box<dyn Error>> {
        let record: Option<(i32,)> = sqlx::query_as("SELECT id FROM admin_users WHERE username = $1")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;
        Ok(record.map(|r| r.0))
    }

    pub async fn check_password(&self, username: &str, password: &str) -> Result<bool, Box<dyn Error>> {
        let record: Option<(String,)> = sqlx::query_as(
            "SELECT password_hash FROM admin_users WHERE username = $1 AND is_active = true",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = record {
            let parts: Vec<&str> = row.0.split(':').collect();
            if parts.len() != 2 {
                return Ok(false);
            }
            let salt = BASE64.decode(parts[0])?;
            let stored_hash = BASE64.decode(parts[1])?;

            let derived_hash = crypto::derive_key(password.as_bytes(), &salt)?;
            
            // Constant time compare using subtle
            use subtle::ConstantTimeEq;
            if derived_hash.len() == stored_hash.len() {
                return Ok(derived_hash.ct_eq(&stored_hash).unwrap_u8() == 1);
            }
        }
        Ok(false)
    }

    pub async fn get_user_roles(&self, username: &str, kek: &[u8]) -> Result<Vec<String>, Box<dyn Error>> {
        let record: Option<(i32,)> = sqlx::query_as("SELECT id FROM admin_users WHERE username = $1 AND is_active = true")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;

        let user_id = match record {
            Some(r) => r.0,
            None => return Ok(vec![]),
        };

        let row: Option<(Vec<u8>, Vec<u8>, Vec<u8>)> = sqlx::query_as("SELECT wrapped_dek, nonce, encrypted_roles FROM user_roles_encrypted WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        let (wrapped_dek, nonce, encrypted_roles) = match row {
            Some(r) => r,
            None => return Ok(vec![]),
        };

        let plaintext = crypto::envelope_decrypt(kek, &wrapped_dek, &nonce, &encrypted_roles)?;
        let role_ids: Vec<i32> = serde_json::from_slice(&plaintext)?;

        if role_ids.is_empty() {
            return Ok(vec![]);
        }

        let records: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM roles WHERE id = ANY($1)"
        )
        .bind(&role_ids)
        .fetch_all(&self.pool)
        .await?;

        Ok(records.into_iter().map(|r| r.0).collect())
    }
}

pub async fn bootstrap_admins(repo: &Repository, config: &DBConfig, kek: &[u8]) {
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
                        let _ = sqlx::query("INSERT INTO admin_audit_logs (username, action, details) VALUES ($1, $2, $3)")
                            .bind("system")
                            .bind("create_admin")
                            .bind(serde_json::json!({"created_user": admin_cfg.username}))
                            .execute(&repo.pool)
                            .await;
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

        if let Err(e) = repo.assign_role(user_id, admin_role_id, kek).await {
            log::error!("Failed to assign ADMIN role to user {}: {}", admin_cfg.username, e);
        }
    }
}
