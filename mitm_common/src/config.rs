use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};
use std::fs;
use std::error::Error;
use crate::crypto;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdminUser {
    pub username: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DBConnectionConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    #[serde(default = "default_db_connect_delay")]
    pub db_connect_delay: u64,
    #[serde(default = "default_sslmode")]
    pub sslmode: bool,
    #[serde(default = "default_max_conns")]
    pub max_conns: u32,
}

fn default_db_connect_delay() -> u64 { 5 }
fn default_sslmode() -> bool { true }
fn default_max_conns() -> u32 { 5 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DBConfig {
    pub db: DBConnectionConfig,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub upload_dir: String,
    #[serde(default)]
    pub admins: Vec<AdminUser>,
    #[serde(default = "default_http_port")]
    pub http_port: u16,
    #[serde(default = "default_use_https")]
    pub use_https: bool,
    #[serde(default)]
    pub ssl_cert: String,
    #[serde(default)]
    pub ssl_key: String,
    #[serde(default)]
    pub socket_dir: String,
}

fn default_log_level() -> String { "INFO".to_string() }
fn default_http_port() -> u16 { 8443 }
fn default_use_https() -> bool { true }

fn get_env_str(key: &str, default_val: &str) -> String {
    env::var(key).unwrap_or_else(|_| default_val.to_string())
}

fn get_env_u16(key: &str, default_val: u16) -> u16 {
    env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default_val)
}

fn get_env_u32(key: &str, default_val: u32) -> u32 {
    env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default_val)
}

fn get_env_u64(key: &str, default_val: u64) -> u64 {
    env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default_val)
}

fn get_env_bool(key: &str, default_val: bool) -> bool {
    if let Ok(val) = env::var(key) {
        let lower = val.to_lowercase();
        lower == "true" || lower == "1" || lower == "yes"
    } else {
        default_val
    }
}

pub fn load_encrypted_config(file_path: &str, password: &str) -> Result<DBConfig, Box<dyn Error>> {
    let encrypted_data = fs::read(file_path)?;
    let decrypted_data = crypto::decrypt(&encrypted_data, password.as_bytes())?;
    let config: DBConfig = serde_json::from_slice(&decrypted_data)?;
    Ok(config)
}

fn load_from_env(exe_dir: &Path) -> DBConfig {
    let mut db_config = DBConfig {
        db: DBConnectionConfig {
            host: get_env_str("MITM_DB_HOST", ""),
            port: get_env_u16("MITM_DB_PORT", 5432),
            user: get_env_str("MITM_DB_USER", ""),
            password: get_env_str("MITM_DB_PASSWORD", ""),
            database: get_env_str("MITM_DB_NAME", ""),
            db_connect_delay: get_env_u64("MITM_DB_CONNECT_DELAY", 5),
            max_conns: get_env_u32("MITM_DB_MAX_CONNS", 50),
            sslmode: true,
        },
        log_level: get_env_str("MITM_LOG_LEVEL", "INFO"),
        upload_dir: get_env_str("MITM_UPLOAD_DIR", exe_dir.join("mitm_uploads").to_string_lossy().as_ref()),
        admins: Vec::new(),
        http_port: get_env_u16("MITM_HTTP_PORT", 8443),
        use_https: get_env_bool("MITM_USE_HTTPS", true),
        ssl_cert: get_env_str("MITM_SSL_CERT", get_env_str("MITM_SSL_CRT", exe_dir.join("certs").join("server.crt").to_string_lossy().as_ref()).as_ref()),
        ssl_key: get_env_str("MITM_SSL_KEY", exe_dir.join("certs").join("server.key").to_string_lossy().as_ref()),
        socket_dir: get_env_str("MITM_SOCKET_DIR", ""),
    };

    let ssl_mode_str = get_env_str("MITM_DB_SSLMODE", "").to_lowercase();
    if ssl_mode_str == "disable" || ssl_mode_str == "false" || ssl_mode_str == "0" || ssl_mode_str == "no" {
        db_config.db.sslmode = false;
    } else if ssl_mode_str == "require" || ssl_mode_str == "true" || ssl_mode_str == "1" || ssl_mode_str == "yes" {
        db_config.db.sslmode = true;
    } else {
        db_config.db.sslmode = get_env_bool("MITM_DB_SSL", true);
    }

    let admins_str = get_env_str("MITM_ADMINS", "");
    if !admins_str.is_empty() {
        for admin in admins_str.split(',') {
            let admin_trim = admin.trim();
            if !admin_trim.is_empty() {
                db_config.admins.push(AdminUser {
                    username: admin_trim.to_string(),
                    token: "cority".to_string(),
                });
            }
        }
    }

    db_config
}

fn apply_internal_defaults(exe_dir: &Path) -> DBConfig {
    DBConfig {
        db: DBConnectionConfig {
            host: "localhost".to_string(),
            port: 5432,
            user: "mitm_user".to_string(),
            password: "".to_string(),
            database: "mitm".to_string(),
            db_connect_delay: 5,
            max_conns: 50,
            sslmode: true,
        },
        log_level: "INFO".to_string(),
        upload_dir: exe_dir.join("mitm_uploads").to_string_lossy().to_string(),
        admins: Vec::new(),
        http_port: 8443,
        use_https: true,
        ssl_cert: exe_dir.join("certs").join("server.crt").to_string_lossy().to_string(),
        ssl_key: exe_dir.join("certs").join("server.key").to_string_lossy().to_string(),
        socket_dir: "".to_string(),
    }
}

fn apply_certificate_fallback(mut cfg: DBConfig, exe_dir: &Path) -> DBConfig {
    let check_fallback = |current_path: &str, filename: &str| -> String {
        if Path::new(current_path).exists() {
            return current_path.to_string();
        }
        let fallback_opts = [
            exe_dir.join(filename),
            exe_dir.join("certs").join(filename),
        ];
        for opt in &fallback_opts {
            if opt.exists() {
                return opt.to_string_lossy().into_owned();
            }
        }
        current_path.to_string()
    };

    let cert_name = Path::new(&cfg.ssl_cert)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("server.crt");
    
    let cert_name_final = if cert_name == "." || cert_name == "/" || cert_name.is_empty() {
        "server.crt"
    } else {
        cert_name
    };
    
    cfg.ssl_cert = check_fallback(&cfg.ssl_cert, cert_name_final);

    let key_name = Path::new(&cfg.ssl_key)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("server.key");
    
    let key_name_final = if key_name == "." || key_name == "/" || key_name.is_empty() {
        "server.key"
    } else {
        key_name
    };

    cfg.ssl_key = check_fallback(&cfg.ssl_key, key_name_final);

    cfg
}

fn apply_path_fallbacks(mut cfg: DBConfig, exe_dir: &Path) -> DBConfig {
    if cfg.socket_dir.is_empty() {
        cfg.socket_dir = exe_dir.join("run").to_string_lossy().to_string();
    }
    if cfg.upload_dir.is_empty() {
        cfg.upload_dir = exe_dir.join("mitm_uploads").to_string_lossy().to_string();
    }
    cfg
}

pub fn load_config(cli_param: Option<&str>, password: &str) -> Result<DBConfig, Box<dyn Error>> {
    let exe_path = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let exe_dir = exe_path.parent().unwrap_or_else(|| Path::new("."));

    // 1. Try Commandline parameter
    if let Some(path) = cli_param {
        if !path.is_empty() {
            if let Ok(cfg) = load_encrypted_config(path, password) {
                log::info!("Loaded config from parameter: {}", path);
                return Ok(apply_path_fallbacks(apply_certificate_fallback(cfg, exe_dir), exe_dir));
            }
            log::warn!("Failed to load config from parameter {}. Falling back.", path);
        }
    }

    // 2. Try ENVs if required ENV (MITM_DB_HOST) is present
    if !get_env_str("MITM_DB_HOST", "").is_empty() {
        let cfg = load_from_env(exe_dir);
        log::info!("Loaded config from Environment Variables.");
        return Ok(apply_path_fallbacks(apply_certificate_fallback(cfg, exe_dir), exe_dir));
    }

    // 3. Try Default files
    let default_path = exe_dir.join("config.enc");
    if let Ok(cfg) = load_encrypted_config(&default_path.to_string_lossy(), password) {
        log::info!("Loaded config from default path: {:?}", default_path);
        return Ok(apply_path_fallbacks(apply_certificate_fallback(cfg, exe_dir), exe_dir));
    }

    let fallback_path = exe_dir.join("cfg").join("config.enc");
    if let Ok(cfg) = load_encrypted_config(&fallback_path.to_string_lossy(), password) {
        log::info!("Loaded config from fallback path: {:?}", fallback_path);
        return Ok(apply_path_fallbacks(apply_certificate_fallback(cfg, exe_dir), exe_dir));
    }

    // 4. Fallback to internal defaults
    log::warn!("No config file or ENVs found. Falling back to internal defaults.");
    let cfg = apply_internal_defaults(exe_dir);
    Ok(apply_path_fallbacks(apply_certificate_fallback(cfg, exe_dir), exe_dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_internal_defaults() {
        let cfg = apply_internal_defaults(Path::new("/tmp"));
        assert_eq!(cfg.db.host, "localhost");
        assert_eq!(cfg.db.port, 5432);
        assert_eq!(cfg.db.max_conns, 50);
        assert_eq!(cfg.db.sslmode, true);
        assert_eq!(cfg.http_port, 8443);
        assert_eq!(cfg.use_https, true);
        assert_eq!(cfg.admins.len(), 0);
    }
}
