/*
 * SPDX-License-Identifier: Apache-2.0
 */

//! Admin audit log (`admin_audit_logs`): action names and helpers (issue #86).
//!
//! Every server-side action name is an UPPER_SNAKE_CASE constant of [`action`]. Actions reported
//! by clients via `POST /api/v1/system/action` carry the [`CLIENT_PREFIX`], so a client can never
//! write a server action. `details` is always a JSON object and never contains secrets.

/// Server-side action names.
pub mod action {
    pub const ASSIGN_ROLES: &str = "ASSIGN_ROLES";
    pub const BACKUP_CONFIG: &str = "BACKUP_CONFIG";
    pub const RESTORE_CONFIG: &str = "RESTORE_CONFIG";
    pub const KEY_ROTATION_SUCCESS: &str = "KEY_ROTATION_SUCCESS";
    pub const UPSERT_CREDENTIAL: &str = "UPSERT_CREDENTIAL";
    pub const DELETE_CREDENTIAL: &str = "DELETE_CREDENTIAL";
    pub const UPSERT_TARGET_CREDENTIAL: &str = "UPSERT_TARGET_CREDENTIAL";
    pub const DELETE_TARGET_CREDENTIAL: &str = "DELETE_TARGET_CREDENTIAL";
    pub const FORBIDDEN_ACCESS: &str = "FORBIDDEN_ACCESS";
    pub const USER_LOGIN_WEBAUTHN: &str = "USER_LOGIN_WEBAUTHN";
    pub const USER_LOGIN_OIDC: &str = "USER_LOGIN_OIDC";
    pub const USER_LOGIN_FAILED: &str = "USER_LOGIN_FAILED";
    pub const BOOTSTRAP_ADMIN_REGISTERED: &str = "BOOTSTRAP_ADMIN_REGISTERED";
    pub const UPSERT_TOPIC_DEPENDENCY: &str = "UPSERT_TOPIC_DEPENDENCY";
    pub const UPSERT_TRANSFORMATION: &str = "UPSERT_TRANSFORMATION";
    pub const DELETE_TRANSFORMATION: &str = "DELETE_TRANSFORMATION";
    pub const UPSERT_VALIDATION: &str = "UPSERT_VALIDATION";
    pub const DELETE_VALIDATION: &str = "DELETE_VALIDATION";
    pub const UPSERT_RULE: &str = "UPSERT_RULE";
    pub const DELETE_RULE: &str = "DELETE_RULE";
    pub const UPSERT_SOURCE: &str = "UPSERT_SOURCE";
    pub const DELETE_SOURCE: &str = "DELETE_SOURCE";
    pub const UPSERT_TARGET: &str = "UPSERT_TARGET";
    pub const DELETE_TARGET: &str = "DELETE_TARGET";
}

/// Prefix of client-reported actions.
pub const CLIENT_PREFIX: &str = "CLIENT_";
const MAX_CLIENT_ACTION_LEN: usize = 64;

/// Turns a client-reported action into `CLIENT_<ACTION>`: upper case, every character other than
/// A-Z and 0-9 replaced by `_`, at most 64 characters.
pub fn client_action(raw: &str) -> String {
    let mut name: String = raw
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' })
        .take(MAX_CLIENT_ACTION_LEN)
        .collect();
    if name.trim_matches('_').is_empty() {
        name = "UNKNOWN".to_string();
    }
    format!("{CLIENT_PREFIX}{name}")
}

/// Details of a `USER_LOGIN_FAILED` entry.
pub fn login_failed(method: &str, reason: &str, client_ip: Option<std::net::IpAddr>) -> serde_json::Value {
    serde_json::json!({
        "method": method,
        "reason": reason,
        "client_ip": client_ip.map(|ip| ip.to_string()),
    })
}

/// Removes user info, query and fragment from a URL, which may carry credentials or tokens.
pub fn redact_url(raw: &str) -> String {
    match reqwest::Url::parse(raw) {
        Ok(mut url) => {
            let _ = url.set_username("");
            let _ = url.set_password(None);
            url.set_query(None);
            url.set_fragment(None);
            url.to_string()
        }
        Err(_) => "<invalid url>".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_actions_are_prefixed_and_sanitized() {
        assert_eq!(client_action("frontend_startup"), "CLIENT_FRONTEND_STARTUP");
        assert_eq!(client_action("DELETE_CREDENTIAL"), "CLIENT_DELETE_CREDENTIAL");
        assert_eq!(client_action(" get jobs/ä'; DROP "), "CLIENT_GET_JOBS_____DROP");
        assert_eq!(client_action(""), "CLIENT_UNKNOWN");
        assert_eq!(client_action("___"), "CLIENT_UNKNOWN");
        assert_eq!(client_action(&"x".repeat(200)).len(), CLIENT_PREFIX.len() + MAX_CLIENT_ACTION_LEN);
    }

    #[test]
    fn server_actions_are_upper_snake_case() {
        for name in [action::ASSIGN_ROLES, action::KEY_ROTATION_SUCCESS, action::USER_LOGIN_FAILED, action::UPSERT_CREDENTIAL] {
            assert!(name.chars().all(|c| c.is_ascii_uppercase() || c == '_'), "{name}");
            assert!(!name.starts_with(CLIENT_PREFIX));
        }
    }

    #[test]
    fn login_failed_details() {
        let details = login_failed("webauthn", "verification_failed", "10.0.0.7".parse().ok());
        assert_eq!(details, serde_json::json!({"method": "webauthn", "reason": "verification_failed", "client_ip": "10.0.0.7"}));
        assert_eq!(login_failed("oidc", "invalid_state", None)["client_ip"], serde_json::Value::Null);
    }

    #[test]
    fn urls_are_redacted() {
        assert_eq!(redact_url("https://user:secret@api.example.com/v1/push?token=abc#x"), "https://api.example.com/v1/push");
        assert_eq!(redact_url("https://api.example.com/v1"), "https://api.example.com/v1");
        assert_eq!(redact_url("not a url"), "<invalid url>");
    }
}
