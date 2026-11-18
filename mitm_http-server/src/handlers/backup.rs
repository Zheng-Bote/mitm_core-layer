/*
 * SPDX-License-Identifier: Apache-2.0
 */

//! Encrypted configuration backup and restore (issue #99).
//!
//! The backup document carries a plaintext header (format, KDF parameters, salt, nonce) and the
//! AES-256-GCM ciphertext of the exported tables. The key is derived from an admin passphrase with
//! Argon2id; the header is authenticated as associated data.

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgConnection;
use std::collections::BTreeMap;
use zeroize::Zeroizing;

/// Current backup format version.
pub const FORMAT: u32 = 1;
/// Minimum passphrase length in characters.
pub const MIN_PASSPHRASE_CHARS: usize = 12;

const KDF_ALGORITHM: &str = "argon2id";
const KDF_VERSION: u32 = 0x13;
const M_COST_KIB: u32 = 64 * 1024;
const T_COST: u32 = 3;
const P_COST: u32 = 1;
const CIPHER_ALGORITHM: &str = "AES-256-GCM";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// Tables contained in a backup.
pub const BACKUP_TABLES: [&str; 16] = [
    "storage_keys",
    "scheduler_config",
    "roles",
    "casbin_rule",
    "admin_users",
    "user_roles_encrypted",
    "webauthn_credentials",
    "scheduled_programs",
    "source_credentials",
    "delivery_targets",
    "mapping_source",
    "mapping_target_field",
    "mapping_rule",
    "mapping_transformation",
    "mapping_validation",
    "topic_dependencies",
];

/// Tables replaced on restore, in delete order (children before the tables they reference).
/// They are inserted in the reverse order.
const REPLACE_DELETE_ORDER: [&str; 14] = [
    "mapping_rule",
    "mapping_transformation",
    "mapping_validation",
    "topic_dependencies",
    "mapping_target_field",
    "mapping_source",
    "source_credentials",
    "delivery_targets",
    "casbin_rule",
    "webauthn_credentials",
    "user_roles_encrypted",
    "admin_users",
    "roles",
    "scheduler_config",
];

/// Exported rows per table.
pub type Tables = BTreeMap<String, Vec<Value>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KdfHeader {
    pub algorithm: String,
    pub version: u32,
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
    pub salt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CipherHeader {
    pub algorithm: String,
    pub nonce: String,
}

/// The backup document as stored by the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackupDocument {
    pub format: u32,
    pub server_version: String,
    pub created_at: String,
    pub kdf: KdfHeader,
    pub cipher: CipherHeader,
    pub ciphertext: String,
}

/// Header fields authenticated as AES-GCM associated data.
#[derive(Serialize)]
struct AssociatedData<'a> {
    format: u32,
    server_version: &'a str,
    created_at: &'a str,
    kdf: &'a KdfHeader,
    cipher: &'a CipherHeader,
}

#[derive(Serialize, Deserialize)]
struct Content {
    tables: Tables,
}

#[derive(Debug, PartialEq)]
pub enum BackupError {
    PassphraseTooShort,
    UnsupportedFormat(String),
    DecryptionFailed,
    Internal(String),
}

pub fn check_passphrase(passphrase: &str) -> Result<(), BackupError> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(BackupError::PassphraseTooShort);
    }
    Ok(())
}

fn associated_data(doc: &BackupDocument) -> Vec<u8> {
    serde_json::to_vec(&AssociatedData {
        format: doc.format,
        server_version: &doc.server_version,
        created_at: &doc.created_at,
        kdf: &doc.kdf,
        cipher: &doc.cipher,
    })
    .unwrap_or_default()
}

fn derive_key(passphrase: &str, salt: &[u8], kdf: &KdfHeader) -> Result<Zeroizing<[u8; 32]>, BackupError> {
    let params = Params::new(kdf.m_cost_kib, kdf.t_cost, kdf.p_cost, Some(32))
        .map_err(|e| BackupError::Internal(format!("invalid Argon2 parameters: {}", e)))?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, key.as_mut())
        .map_err(|e| BackupError::Internal(format!("key derivation failed: {}", e)))?;
    Ok(key)
}

/// Encrypts the exported tables into a backup document.
pub fn encrypt(tables: &Tables, passphrase: &str, server_version: &str, created_at: &str) -> Result<BackupDocument, BackupError> {
    check_passphrase(passphrase)?;
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);

    let mut doc = BackupDocument {
        format: FORMAT,
        server_version: server_version.to_string(),
        created_at: created_at.to_string(),
        kdf: KdfHeader {
            algorithm: KDF_ALGORITHM.to_string(),
            version: KDF_VERSION,
            m_cost_kib: M_COST_KIB,
            t_cost: T_COST,
            p_cost: P_COST,
            salt: B64.encode(salt),
        },
        cipher: CipherHeader { algorithm: CIPHER_ALGORITHM.to_string(), nonce: B64.encode(nonce) },
        ciphertext: String::new(),
    };

    let key = derive_key(passphrase, &salt, &doc.kdf)?;
    let plaintext = Zeroizing::new(
        serde_json::to_vec(&Content { tables: tables.clone() }).map_err(|e| BackupError::Internal(e.to_string()))?,
    );
    let cipher = Aes256Gcm::new_from_slice(key.as_ref()).map_err(|e| BackupError::Internal(e.to_string()))?;
    let aad = associated_data(&doc);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), Payload { msg: &plaintext, aad: &aad })
        .map_err(|_| BackupError::Internal("encryption failed".to_string()))?;
    doc.ciphertext = B64.encode(ciphertext);
    Ok(doc)
}

/// Decrypts a backup document; rejects unencrypted backups of older servers and unknown formats.
pub fn decrypt(document: &Value, passphrase: &str) -> Result<Tables, BackupError> {
    let format = match document.get("format").and_then(Value::as_u64) {
        Some(f) => f,
        None if document.get("data").is_some() || document.get("version").is_some() => {
            return Err(BackupError::UnsupportedFormat(
                "unencrypted backup of mitm_http-server < 3.6.0; create a new backup".to_string(),
            ))
        }
        None => return Err(BackupError::UnsupportedFormat("not a MitM backup document".to_string())),
    };
    if format != FORMAT as u64 {
        return Err(BackupError::UnsupportedFormat(format!("backup format {} is not supported", format)));
    }
    let doc: BackupDocument = serde_json::from_value(document.clone())
        .map_err(|e| BackupError::UnsupportedFormat(format!("invalid backup document: {}", e)))?;
    if doc.kdf.algorithm != KDF_ALGORITHM || doc.kdf.version != KDF_VERSION || doc.cipher.algorithm != CIPHER_ALGORITHM {
        return Err(BackupError::UnsupportedFormat("unsupported key derivation or cipher".to_string()));
    }
    // Bound the KDF cost taken from the (untrusted) header.
    if !(8 * 1024..=1024 * 1024).contains(&doc.kdf.m_cost_kib) || !(1..=10).contains(&doc.kdf.t_cost) || !(1..=8).contains(&doc.kdf.p_cost) {
        return Err(BackupError::UnsupportedFormat("key derivation parameters out of range".to_string()));
    }

    let salt = B64.decode(&doc.kdf.salt).map_err(|_| BackupError::DecryptionFailed)?;
    let nonce = B64.decode(&doc.cipher.nonce).map_err(|_| BackupError::DecryptionFailed)?;
    let ciphertext = B64.decode(&doc.ciphertext).map_err(|_| BackupError::DecryptionFailed)?;
    if salt.len() < 8 || nonce.len() != NONCE_LEN {
        return Err(BackupError::DecryptionFailed);
    }

    let key = derive_key(passphrase, &salt, &doc.kdf)?;
    let cipher = Aes256Gcm::new_from_slice(key.as_ref()).map_err(|e| BackupError::Internal(e.to_string()))?;
    let aad = associated_data(&doc);
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(Nonce::from_slice(&nonce), Payload { msg: &ciphertext, aad: &aad })
            .map_err(|_| BackupError::DecryptionFailed)?,
    );
    let content: Content = serde_json::from_slice(&plaintext).map_err(|_| BackupError::DecryptionFailed)?;
    Ok(content.tables)
}

/// Row counts per table.
pub fn row_counts(tables: &Tables) -> BTreeMap<String, usize> {
    tables.iter().map(|(t, rows)| (t.clone(), rows.len())).collect()
}

/// Database error together with the table it occurred in.
#[derive(Debug)]
pub struct TableError {
    pub table: String,
    pub error: sqlx::Error,
}

fn at(table: &str) -> impl FnOnce(sqlx::Error) -> TableError + '_ {
    move |error| TableError { table: table.to_string(), error }
}

/// Exports all backup tables (call inside a REPEATABLE READ transaction for a consistent snapshot).
pub async fn export_tables(conn: &mut PgConnection) -> Result<Tables, TableError> {
    let mut tables = Tables::new();
    for table in BACKUP_TABLES {
        let query = format!("SELECT COALESCE(json_agg(row_to_json(t)), '[]'::json) FROM {} t", table);
        let rows: Value = sqlx::query_scalar(&query).fetch_one(&mut *conn).await.map_err(at(table))?;
        tables.insert(table.to_string(), rows.as_array().cloned().unwrap_or_default());
    }
    Ok(tables)
}

/// Result of a restore.
#[derive(Debug, Default, Serialize)]
pub struct RestoreReport {
    pub tables: BTreeMap<String, usize>,
    pub deactivated_programs: Vec<String>,
}

fn rows_json(tables: &Tables, table: &str) -> Option<Value> {
    tables.get(table).map(|rows| Value::Array(rows.clone()))
}

/// Restores the backup tables; must run inside a transaction (the caller commits or rolls back).
pub async fn restore_tables(conn: &mut PgConnection, tables: &Tables) -> Result<RestoreReport, TableError> {
    let mut report = RestoreReport { tables: row_counts(tables), ..Default::default() };

    // 1. Keys are only added: existing keys may encrypt raw_ingestion data and are never deleted.
    if let Some(rows) = rows_json(tables, "storage_keys") {
        sqlx::query("INSERT INTO storage_keys SELECT * FROM json_populate_recordset(null::storage_keys, $1::json) ON CONFLICT (id) DO NOTHING")
            .bind(rows)
            .execute(&mut *conn)
            .await
            .map_err(at("storage_keys"))?;
    }

    // 2. Clear replaced tables children-first, 3. insert them parents-first.
    for table in REPLACE_DELETE_ORDER.iter().filter(|t| tables.contains_key(**t)) {
        sqlx::query(&format!("DELETE FROM {}", table)).execute(&mut *conn).await.map_err(at(table))?;
    }
    for table in REPLACE_DELETE_ORDER.iter().rev() {
        if let Some(rows) = rows_json(tables, table) {
            let query = format!("INSERT INTO {t} SELECT * FROM json_populate_recordset(null::{t}, $1::json)", t = table);
            sqlx::query(&query).bind(rows).execute(&mut *conn).await.map_err(at(table))?;
        }
    }

    // 4. Programs are upserted (keeps program_runs / job audit logs); missing ones are deactivated.
    if let Some(rows) = rows_json(tables, "scheduled_programs") {
        let table = "scheduled_programs";
        let columns: Vec<String> = sqlx::query_scalar(
            "SELECT column_name::text FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = $1 ORDER BY ordinal_position",
        )
        .bind(table)
        .fetch_all(&mut *conn)
        .await
        .map_err(at(table))?;
        let cols = columns.iter().map(|c| format!("\"{}\"", c)).collect::<Vec<_>>().join(", ");
        let updates = columns
            .iter()
            .filter(|c| c.as_str() != "id")
            .map(|c| format!("\"{c}\" = EXCLUDED.\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!(
            "INSERT INTO {table} ({cols}) SELECT {cols} FROM json_populate_recordset(null::{table}, $1::json) ON CONFLICT (id) DO UPDATE SET {updates}"
        );
        sqlx::query(&query).bind(rows).execute(&mut *conn).await.map_err(at(table))?;

        let ids: Vec<i32> = tables[table]
            .iter()
            .filter_map(|r| r.get("id").and_then(Value::as_i64))
            .map(|id| id as i32)
            .collect();
        report.deactivated_programs = sqlx::query_scalar(
            "UPDATE scheduled_programs SET enabled = false, updated_at = CURRENT_TIMESTAMP WHERE NOT (id = ANY($1)) AND enabled RETURNING name",
        )
        .bind(&ids)
        .fetch_all(&mut *conn)
        .await
        .map_err(at(table))?;
        report.deactivated_programs.sort();
    }

    // 5. Sequences follow the restored IDs.
    for table in BACKUP_TABLES.iter().filter(|t| tables.contains_key(**t)) {
        let serials: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT column_name::text, pg_get_serial_sequence(quote_ident(table_name), column_name)
             FROM information_schema.columns
             WHERE table_schema = current_schema() AND table_name = $1 AND column_default LIKE 'nextval(%'",
        )
        .bind(table)
        .fetch_all(&mut *conn)
        .await
        .map_err(at(table))?;
        for (column, sequence) in serials {
            let Some(sequence) = sequence else { continue };
            // setval is not transactional: only ever move a sequence forward.
            let query = format!(
                "SELECT setval($1, GREATEST(COALESCE((SELECT MAX(\"{c}\") FROM {t}), 0), COALESCE(pg_sequence_last_value($1::regclass), 0), 1), true)",
                c = column,
                t = table
            );
            sqlx::query(&query).bind(sequence).execute(&mut *conn).await.map_err(at(table))?;
        }
    }

    Ok(report)
}

/// Role assignment of a user as contained in a backup.
#[derive(Debug, PartialEq)]
pub enum UserAssignment {
    Missing,
    Inactive,
    NoRoles,
    Encrypted { wrapped_dek: Vec<u8>, nonce: Vec<u8>, encrypted_roles: Vec<u8> },
}

/// Parses a bytea value exported by row_to_json ("\\x0a1b…").
fn bytea(value: Option<&Value>) -> Option<Vec<u8>> {
    let hex = value?.as_str()?.strip_prefix("\\x")?;
    (0..hex.len())
        .step_by(2)
        .map(|i| hex.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok()))
        .collect()
}

/// Looks up the user and their encrypted role assignment in the backup.
pub fn user_assignment(tables: &Tables, username: &str) -> UserAssignment {
    let Some(user) = tables
        .get("admin_users")
        .and_then(|rows| rows.iter().find(|u| u.get("username").and_then(Value::as_str).is_some_and(|n| n.eq_ignore_ascii_case(username))))
    else {
        return UserAssignment::Missing;
    };
    if !user.get("is_active").and_then(Value::as_bool).unwrap_or(false) {
        return UserAssignment::Inactive;
    }
    let user_id = user.get("id").and_then(Value::as_i64);
    let assignment = tables
        .get("user_roles_encrypted")
        .and_then(|rows| rows.iter().find(|r| r.get("user_id").and_then(Value::as_i64) == user_id));
    match assignment {
        None => UserAssignment::NoRoles,
        Some(row) => match (bytea(row.get("wrapped_dek")), bytea(row.get("nonce")), bytea(row.get("encrypted_roles"))) {
            (Some(wrapped_dek), Some(nonce), Some(encrypted_roles)) => UserAssignment::Encrypted { wrapped_dek, nonce, encrypted_roles },
            _ => UserAssignment::NoRoles,
        },
    }
}

/// Role names for the given role IDs; like the session check, no roles means ADMIN.
pub fn role_names(tables: &Tables, role_ids: &[i32]) -> Vec<String> {
    let names: Vec<String> = tables
        .get("roles")
        .map(|rows| {
            rows.iter()
                .filter(|r| r.get("id").and_then(Value::as_i64).is_some_and(|id| role_ids.contains(&(id as i32))))
                .filter_map(|r| r.get("name").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if names.is_empty() { vec!["ADMIN".to_string()] } else { names }
}

/// Checks with the backup's Casbin policies whether one of the roles may call method on path.
pub async fn policies_allow(tables: &Tables, roles: &[String], path: &str, method: &str) -> Result<bool, String> {
    use casbin::{CoreApi, MgmtApi, RbacApi};
    let model = casbin::DefaultModel::from_str(crate::handlers::RBAC_MODEL).await.map_err(|e| e.to_string())?;
    let mut enforcer = casbin::Enforcer::new(model, casbin::MemoryAdapter::default()).await.map_err(|e| e.to_string())?;
    let field = |r: &Value, k: &str| r.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    for rule in tables.get("casbin_rule").into_iter().flatten() {
        match rule.get("ptype").and_then(Value::as_str) {
            Some("p") => {
                enforcer.add_policy(vec![field(rule, "v0"), field(rule, "v1"), field(rule, "v2")]).await.map_err(|e| e.to_string())?;
            }
            Some("g") => {
                enforcer.add_role_for_user(&field(rule, "v0"), &field(rule, "v1"), None).await.map_err(|e| e.to_string())?;
            }
            _ => {}
        }
    }
    for role in roles {
        if enforcer.enforce((role.as_str(), path, method)).map_err(|e| e.to_string())? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_tables() -> Tables {
        let mut t = Tables::new();
        t.insert("roles".into(), vec![json!({"id": 1, "name": "ADMIN"}), json!({"id": 3, "name": "USER"}), json!({"id": 4, "name": "BACKUP-RESTORE"})]);
        t.insert(
            "casbin_rule".into(),
            vec![
                json!({"ptype": "p", "v0": "ADMIN", "v1": "/api/v1/*", "v2": ".*"}),
                json!({"ptype": "g", "v0": "ADMIN", "v1": "USER", "v2": null}),
                json!({"ptype": "p", "v0": "USER", "v1": "/api/v1/jobs/*", "v2": "POST"}),
            ],
        );
        t.insert(
            "admin_users".into(),
            vec![
                json!({"id": 1, "username": "ALICE", "is_active": true}),
                json!({"id": 2, "username": "BOB", "is_active": false}),
                json!({"id": 3, "username": "CAROL", "is_active": true}),
            ],
        );
        t.insert(
            "user_roles_encrypted".into(),
            vec![json!({"user_id": 3, "wrapped_dek": "\\x0a0b", "nonce": "\\x0c", "encrypted_roles": "\\xff00"})],
        );
        t
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let tables = sample_tables();
        let doc = encrypt(&tables, "correct horse battery", "3.6.0", "2026-11-18T12:00:00Z").unwrap();
        assert_eq!(doc.format, FORMAT);
        assert!(!doc.ciphertext.contains("ALICE"));
        let value = serde_json::to_value(&doc).unwrap();
        assert_eq!(decrypt(&value, "correct horse battery").unwrap(), tables);
    }

    #[test]
    fn wrong_passphrase_and_tampering_fail() {
        let doc = encrypt(&sample_tables(), "correct horse battery", "3.6.0", "2026-11-18T12:00:00Z").unwrap();
        let value = serde_json::to_value(&doc).unwrap();
        assert_eq!(decrypt(&value, "wrong passphrase!!"), Err(BackupError::DecryptionFailed));

        let mut header = value.clone();
        header["server_version"] = json!("9.9.9");
        assert_eq!(decrypt(&header, "correct horse battery"), Err(BackupError::DecryptionFailed));

        let mut body = doc.clone();
        let mut raw = B64.decode(&body.ciphertext).unwrap();
        raw[0] ^= 1;
        body.ciphertext = B64.encode(raw);
        assert_eq!(decrypt(&serde_json::to_value(&body).unwrap(), "correct horse battery"), Err(BackupError::DecryptionFailed));
    }

    #[test]
    fn short_passphrase_is_rejected() {
        assert_eq!(check_passphrase("short"), Err(BackupError::PassphraseTooShort));
        assert!(encrypt(&Tables::new(), "elevenchars", "3.6.0", "x").is_err());
        assert!(check_passphrase("twelve chars").is_ok());
    }

    #[test]
    fn old_and_unknown_formats_are_unsupported() {
        let old = json!({"version": "3.5.0", "data": {"scheduled_programs": []}});
        assert!(matches!(decrypt(&old, "correct horse battery"), Err(BackupError::UnsupportedFormat(m)) if m.contains("unencrypted")));
        let future = json!({"format": 2});
        assert!(matches!(decrypt(&future, "correct horse battery"), Err(BackupError::UnsupportedFormat(_))));
    }

    #[test]
    fn user_assignment_cases() {
        let t = sample_tables();
        assert_eq!(user_assignment(&t, "alice"), UserAssignment::NoRoles);
        assert_eq!(user_assignment(&t, "BOB"), UserAssignment::Inactive);
        assert_eq!(user_assignment(&t, "DAVE"), UserAssignment::Missing);
        assert_eq!(
            user_assignment(&t, "CAROL"),
            UserAssignment::Encrypted { wrapped_dek: vec![0x0a, 0x0b], nonce: vec![0x0c], encrypted_roles: vec![0xff, 0x00] }
        );
    }

    #[test]
    fn role_names_fall_back_to_admin() {
        let t = sample_tables();
        assert_eq!(role_names(&t, &[3]), vec!["USER".to_string()]);
        assert_eq!(role_names(&t, &[]), vec!["ADMIN".to_string()]);
    }

    #[tokio::test]
    async fn policies_from_backup_decide_restore_access() {
        let t = sample_tables();
        assert!(policies_allow(&t, &["ADMIN".into()], "/api/v1/system/restore", "POST").await.unwrap());
        assert!(!policies_allow(&t, &["USER".into()], "/api/v1/system/restore", "POST").await.unwrap());
        // No policy for BACKUP-RESTORE in this backup → no access.
        assert!(!policies_allow(&t, &["BACKUP-RESTORE".into()], "/api/v1/system/restore", "POST").await.unwrap());
    }

    /// Integration test against PostgreSQL. Set MITM_TEST_DATABASE_URL and MITM_TEST_SCHEMA
    /// (a schema created from migrations/setup.sql). Everything runs in a transaction that is rolled back.
    #[tokio::test]
    async fn restore_against_postgres() {
        use sqlx::{Connection, Executor};
        let (Ok(url), Ok(schema)) = (std::env::var("MITM_TEST_DATABASE_URL"), std::env::var("MITM_TEST_SCHEMA")) else {
            eprintln!("skipped: MITM_TEST_DATABASE_URL / MITM_TEST_SCHEMA not set");
            return;
        };
        let mut conn = sqlx::PgConnection::connect(&url).await.unwrap();
        let mut tx = conn.begin().await.unwrap();
        tx.execute(format!("SET LOCAL search_path TO {}, public", schema).as_str()).await.unwrap();

        // Fixture: one key, two programs with history, mapping rule referencing source and field, a user.
        tx.execute(
            r#"
            DELETE FROM program_runs; DELETE FROM scheduled_programs;
            DELETE FROM mapping_rule; DELETE FROM mapping_source; DELETE FROM mapping_target_field;
            DELETE FROM source_credentials; DELETE FROM delivery_targets;
            INSERT INTO storage_keys (id, wrapped_key) VALUES ('11111111-1111-1111-1111-111111111111', '\x0102');
            INSERT INTO source_credentials (id, source_name, connector_type, config_payload, nonce, dek_id, topic)
              VALUES ('22222222-2222-2222-2222-222222222222', 'src', 'PG', '\x03', '\x04', '11111111-1111-1111-1111-111111111111', 'T');
            INSERT INTO scheduled_programs (id, name, command, cron_expr) VALUES (101, 'job_a', 'a', '0 * * * * *'), (102, 'job_b', 'b', '0 * * * * *');
            INSERT INTO program_runs (program_id) VALUES (101), (101);
            INSERT INTO job_audit_logs (run_id, message) SELECT id, 'ERROR: x' FROM program_runs;
            "#,
        )
        .await
        .unwrap();
        tx.execute(
            r#"
            INSERT INTO mapping_source (id, name, type, topic) VALUES ('e0000000-0000-0000-0000-000000000001', 'S', 'PG', 'T');
            INSERT INTO mapping_target_field (id, topic, field_name, data_type, is_required) VALUES ('f0000000-0000-0000-0000-000000000001', 'T', 'F', 'string', false);
            INSERT INTO mapping_rule (id, source_id, target_field_id, source_field) VALUES ('a0000000-0000-0000-0000-000000000001', 'e0000000-0000-0000-0000-000000000001', 'f0000000-0000-0000-0000-000000000001', 'col');
            "#,
        )
        .await
        .unwrap();

        let backup = export_tables(&mut tx).await.unwrap();
        let runs_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM program_runs").fetch_one(&mut *tx).await.unwrap();
        let logs_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM job_audit_logs").fetch_one(&mut *tx).await.unwrap();

        // Changes after the backup: a new program, a changed program.
        tx.execute("INSERT INTO scheduled_programs (id, name, command, cron_expr) VALUES (103, 'job_c', 'c', '0 * * * * *'); UPDATE scheduled_programs SET command = 'changed' WHERE id = 102;")
            .await
            .unwrap();

        // Same-DB restore (mapping rules present): must succeed and keep the history.
        let report = restore_tables(&mut tx, &backup).await.unwrap();
        assert_eq!(report.deactivated_programs, vec!["job_c".to_string()]);
        let runs_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM program_runs").fetch_one(&mut *tx).await.unwrap();
        let logs_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM job_audit_logs").fetch_one(&mut *tx).await.unwrap();
        assert_eq!((runs_after, logs_after), (runs_before, logs_before), "restore must keep program_runs and job_audit_logs");
        let command: String = sqlx::query_scalar("SELECT command FROM scheduled_programs WHERE id = 102").fetch_one(&mut *tx).await.unwrap();
        assert_eq!(command, "b");
        let enabled_c: bool = sqlx::query_scalar("SELECT enabled FROM scheduled_programs WHERE id = 103").fetch_one(&mut *tx).await.unwrap();
        assert!(!enabled_c);
        let rules: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mapping_rule").fetch_one(&mut *tx).await.unwrap();
        assert_eq!(rules as usize, backup["mapping_rule"].len());

        // Restore into emptied tables with reset sequences (simulates a new database).
        tx.execute(
            r#"
            DELETE FROM program_runs; DELETE FROM scheduled_programs; DELETE FROM mapping_rule; DELETE FROM mapping_source;
            DELETE FROM mapping_target_field; DELETE FROM source_credentials; DELETE FROM delivery_targets;
            DELETE FROM webauthn_credentials; DELETE FROM user_roles_encrypted; DELETE FROM admin_users; DELETE FROM casbin_rule;
            DELETE FROM storage_keys WHERE id = '11111111-1111-1111-1111-111111111111';
            ALTER SEQUENCE scheduled_programs_id_seq RESTART WITH 1;
            "#,
        )
        .await
        .unwrap();
        let report = restore_tables(&mut tx, &backup).await.unwrap();
        assert!(report.deactivated_programs.is_empty());
        for table in BACKUP_TABLES {
            let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {}", table)).fetch_one(&mut *tx).await.unwrap();
            if table == "storage_keys" {
                assert!(count as usize >= backup[table].len());
            } else {
                assert_eq!(count as usize, backup[table].len(), "row count of {}", table);
            }
        }
        // Sequence follows the restored IDs: a new program gets a fresh ID.
        let new_id: i32 = sqlx::query_scalar("INSERT INTO scheduled_programs (name, command, cron_expr) VALUES ('job_new', 'n', '0 * * * * *') RETURNING id")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert!(new_id > 102);

        tx.rollback().await.unwrap();
    }
}
