/*
 * SPDX-License-Identifier: Apache-2.0
 */

//! OAuth2/OIDC sign-in (authorization code flow with PKCE). The http-server is the OIDC
//! client: it redeems the code, validates the ID token and issues its own session, which the
//! SPA picks up through a single-use handoff id.

use std::net::SocketAddr;
use axum::{
    extract::{ConnectInfo, Query, State, Json},
    http::{HeaderMap, StatusCode, header::{COOKIE, LOCATION, SET_COOKIE}},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use chrono::{Utc, Duration};
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    TokenResponse,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;
use crate::handlers::{AppState, ErrorResponse, JsonApiError};
use super::auth_guard;

const STATE_COOKIE: &str = "__Host-mitm_oidc";
const LOGIN_STATE_TTL_MINUTES: i64 = 10;
const HANDOFF_TTL_SECONDS: i64 = 60;

type OidcClient = CoreClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointMaybeSet, EndpointMaybeSet>;

/// OIDC relying party configuration from `OIDC_*` environment variables. Provider discovery
/// runs on the first login, so an unreachable IdP at startup does not disable SSO.
pub struct Oidc {
    issuer: String,
    client_id: String,
    client_secret: Option<Zeroizing<String>>,
    redirect_uri: String,
    http: reqwest::Client,
    client: tokio::sync::OnceCell<OidcClient>,
}

impl Oidc {
    /// Returns `None` unless `OIDC_ISSUER`, `OIDC_CLIENT_ID` and `OIDC_REDIRECT_URI` are set.
    pub fn from_env() -> Option<Self> {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        let issuer = var("OIDC_ISSUER")?;
        let client_id = var("OIDC_CLIENT_ID")?;
        let redirect_uri = var("OIDC_REDIRECT_URI")?;
        // No redirects: discovery and token requests must hit the configured IdP only.
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .ok()?;
        Some(Self {
            issuer,
            client_id,
            client_secret: var("OIDC_CLIENT_SECRET").map(Zeroizing::new),
            redirect_uri,
            http,
            client: tokio::sync::OnceCell::new(),
        })
    }

    async fn client(&self) -> Result<&OidcClient, String> {
        self.client.get_or_try_init(|| async {
            let issuer = IssuerUrl::new(self.issuer.clone()).map_err(|e| e.to_string())?;
            let metadata = CoreProviderMetadata::discover_async(issuer, &self.http).await.map_err(|e| e.to_string())?;
            let redirect = RedirectUrl::new(self.redirect_uri.clone()).map_err(|e| e.to_string())?;
            Ok(CoreClient::from_provider_metadata(
                metadata,
                ClientId::new(self.client_id.clone()),
                self.client_secret.as_ref().map(|s| ClientSecret::new(s.to_string())),
            ).set_redirect_uri(redirect))
        }).await
    }
}

/// Login state persisted in `webauthn_states` between `login` and `callback`.
#[derive(Serialize, Deserialize)]
struct LoginState {
    state: String,
    nonce: String,
    pkce_verifier: String,
}

#[derive(Serialize)]
pub struct OidcConfigResp {
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub iss: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct ExchangeReq {
    pub handoff: Uuid,
}

#[derive(Serialize)]
pub struct ExchangeResp {
    pub session_token: String,
}

fn json_error(status: StatusCode, title: &str, detail: &str) -> Response {
    let err = ErrorResponse { errors: vec![JsonApiError { status: status.as_str().into(), title: title.into(), detail: Some(detail.into()) }] };
    (status, Json(err)).into_response()
}

fn redirect(location: &str) -> Response {
    (StatusCode::FOUND, [(LOCATION, location.to_string())]).into_response()
}

/// Redirects the browser back to the login page and clears the login-state cookie.
fn fail(code: &str) -> Response {
    (
        StatusCode::FOUND,
        [
            (LOCATION, format!("/login?sso_error={}", code)),
            (SET_COOKIE, format!("{}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax", STATE_COOKIE)),
        ],
    ).into_response()
}

fn state_cookie(headers: &HeaderMap) -> Option<Uuid> {
    headers.get_all(COOKIE).iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(name, _)| *name == STATE_COOKIE)
        .and_then(|(_, value)| Uuid::parse_str(value).ok())
}

// GET /api/v1/auth/oidc/config
pub async fn get_config(State(state): State<AppState>) -> Response {
    Json(OidcConfigResp { enabled: state.oidc.is_some() }).into_response()
}

// GET /api/v1/auth/oidc/login
pub async fn login(State(state): State<AppState>) -> Response {
    let Some(oidc) = state.oidc.as_deref() else {
        return json_error(StatusCode::NOT_FOUND, "Not Found", "OIDC is not configured.");
    };
    let Some(repo) = state.repo.get() else {
        return json_error(StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable", "Database not ready");
    };
    let client = match oidc.client().await {
        Ok(c) => c,
        Err(e) => {
            log::error!("OIDC discovery for {} failed: {}", oidc.issuer, e);
            let _ = repo.log_system("ERROR", "http-server", &format!("OIDC discovery for {} failed: {}", oidc.issuer, e)).await;
            return fail("idp_unavailable");
        }
    };

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, csrf, nonce) = client
        .authorize_url(CoreAuthenticationFlow::AuthorizationCode, CsrfToken::new_random, Nonce::new_random)
        .add_scope(Scope::new("profile".into()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    let login_state = LoginState {
        state: csrf.secret().clone(),
        nonce: nonce.secret().clone(),
        pkce_verifier: pkce_verifier.secret().clone(),
    };
    let id = Uuid::new_v4();
    // Abandoned logins never reach the callback; drop their expired states here.
    let _ = sqlx::query("DELETE FROM webauthn_states WHERE challenge_type IN ('oidc', 'oidc_handoff') AND expires_at < NOW()")
        .execute(&repo.pool).await;
    if let Err(e) = sqlx::query(
        "INSERT INTO webauthn_states (id, challenge_type, state_data, expires_at) VALUES ($1, 'oidc', $2, $3)"
    )
    .bind(id)
    .bind(serde_json::to_value(&login_state).unwrap_or_default())
    .bind(Utc::now() + Duration::minutes(LOGIN_STATE_TTL_MINUTES))
    .execute(&repo.pool)
    .await {
        log::error!("Failed to persist OIDC login state: {}", e);
        return fail("server_error");
    }

    (
        StatusCode::FOUND,
        [
            (LOCATION, auth_url.to_string()),
            (SET_COOKIE, format!("{}={}; Path=/; Max-Age={}; HttpOnly; Secure; SameSite=Lax", STATE_COOKIE, id, LOGIN_STATE_TTL_MINUTES * 60)),
        ],
    ).into_response()
}

// GET /api/v1/auth/oidc/callback
pub async fn callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
    headers: HeaderMap,
    peer: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let Some(oidc) = state.oidc.as_deref() else {
        return json_error(StatusCode::NOT_FOUND, "Not Found", "OIDC is not configured.");
    };
    let Some(repo) = state.repo.get() else {
        return json_error(StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable", "Database not ready");
    };

    // Consume the login state first so that every callback, even a failing one, uses it up.
    let Some(state_id) = state_cookie(&headers) else {
        return fail("invalid_state");
    };
    let stored: Option<serde_json::Value> = sqlx::query_scalar(
        "DELETE FROM webauthn_states WHERE id = $1 AND challenge_type = 'oidc' AND expires_at > NOW() RETURNING state_data"
    )
    .bind(state_id)
    .fetch_optional(&repo.pool)
    .await
    .unwrap_or(None);
    let Some(login_state) = stored.and_then(|v| serde_json::from_value::<LoginState>(v).ok()) else {
        return fail("invalid_state");
    };
    if query.state.as_deref() != Some(login_state.state.as_str()) {
        return fail("invalid_state");
    }

    if let Some(error) = query.error {
        log::warn!("OIDC authorization failed at the IdP: {}", error);
        return fail("idp_error");
    }
    // RFC 9207: reject responses issued by another authorization server (mix-up attack).
    if query.iss.is_some_and(|iss| iss != oidc.issuer) {
        return fail("invalid_issuer");
    }
    let Some(code) = query.code else {
        return fail("idp_error");
    };

    let client = match oidc.client().await {
        Ok(c) => c,
        Err(e) => {
            log::error!("OIDC discovery for {} failed: {}", oidc.issuer, e);
            return fail("idp_unavailable");
        }
    };
    let request = match client.exchange_code(AuthorizationCode::new(code)) {
        Ok(r) => r,
        Err(e) => {
            log::error!("OIDC provider has no token endpoint: {}", e);
            return fail("idp_unavailable");
        }
    };
    let token_response = match request
        .set_pkce_verifier(PkceCodeVerifier::new(login_state.pkce_verifier))
        .request_async(&oidc.http)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            log::error!("OIDC token exchange failed: {:?}", e);
            return fail("token_exchange_failed");
        }
    };
    let subject = match token_response.id_token()
        .ok_or_else(|| "token response without id_token".to_string())
        .and_then(|t| t.claims(&client.id_token_verifier(), &Nonce::new(login_state.nonce)).map_err(|e| e.to_string()))
    {
        Ok(claims) => claims.subject().to_string(),
        Err(e) => {
            log::error!("OIDC ID token rejected: {}", e);
            return fail("invalid_id_token");
        }
    };

    // Only known, active users may sign in; OIDC never creates users.
    let is_active: Option<bool> = sqlx::query_scalar("SELECT is_active FROM admin_users WHERE username = $1")
        .bind(&subject)
        .fetch_optional(&repo.pool)
        .await
        .unwrap_or(None);
    match is_active {
        None => {
            let _ = repo.log_admin(&subject, "USER_LOGIN_OIDC", serde_json::json!({ "status": "rejected", "reason": "unknown_user" })).await;
            return fail("unknown_user");
        }
        Some(false) => {
            let _ = repo.log_admin(&subject, "USER_LOGIN_OIDC", serde_json::json!({ "status": "rejected", "reason": "inactive_user" })).await;
            return fail("inactive_user");
        }
        Some(true) => {}
    }

    // Issue session token (same lifetime as the WebAuthn login)
    let token_uuid = Uuid::new_v4();
    let now = Utc::now();
    let expires_at = now + Duration::hours(24);
    if let Err(e) = sqlx::query(
        "INSERT INTO user_sessions (session_token, os_user, created_at, expires_at, last_active_at, client_ip) VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(token_uuid)
    .bind(&subject)
    .bind(now)
    .bind(expires_at)
    .bind(now)
    .bind(auth_guard::client_ip(&headers, peer.as_ref(), state.trust_proxy_headers).map(|ip| ip.to_string()))
    .execute(&repo.pool)
    .await {
        log::error!("Failed to persist OIDC session: {}", e);
        return fail("server_error");
    }

    let handoff = Uuid::new_v4();
    if let Err(e) = sqlx::query(
        "INSERT INTO webauthn_states (id, challenge_type, state_data, expires_at) VALUES ($1, 'oidc_handoff', $2, $3)"
    )
    .bind(handoff)
    .bind(serde_json::json!({ "session_token": token_uuid }))
    .bind(now + Duration::seconds(HANDOFF_TTL_SECONDS))
    .execute(&repo.pool)
    .await {
        log::error!("Failed to persist OIDC handoff: {}", e);
        return fail("server_error");
    }

    let _ = repo.log_admin(&subject, "USER_LOGIN_OIDC", serde_json::json!({ "status": "success" })).await;

    let mut response = redirect(&format!("/auth/callback?handoff={}", handoff));
    if let Ok(cookie) = format!("{}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax", STATE_COOKIE).parse() {
        response.headers_mut().insert(SET_COOKIE, cookie);
    }
    response
}

// POST /api/v1/auth/oidc/exchange
pub async fn exchange(
    State(state): State<AppState>,
    Json(payload): Json<ExchangeReq>,
) -> Response {
    if state.oidc.is_none() {
        return json_error(StatusCode::NOT_FOUND, "Not Found", "OIDC is not configured.");
    }
    let Some(repo) = state.repo.get() else {
        return json_error(StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable", "Database not ready");
    };
    let stored: Option<serde_json::Value> = sqlx::query_scalar(
        "DELETE FROM webauthn_states WHERE id = $1 AND challenge_type = 'oidc_handoff' AND expires_at > NOW() RETURNING state_data"
    )
    .bind(payload.handoff)
    .fetch_optional(&repo.pool)
    .await
    .unwrap_or(None);
    match stored.as_ref().and_then(|v| v["session_token"].as_str()) {
        Some(token) => Json(ExchangeResp { session_token: token.to_string() }).into_response(),
        None => json_error(StatusCode::UNAUTHORIZED, "Unauthorized", "Sign-in link is invalid or expired. Please sign in again."),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/oidc/config", get(get_config))
        .route("/oidc/login", get(login))
        .route("/oidc/callback", get(callback))
        .route("/oidc/exchange", post(exchange))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_cookie_is_found_among_other_cookies() {
        let id = Uuid::new_v4();
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, format!("theme=dark; {}={}; other=1", STATE_COOKIE, id).parse().unwrap());
        assert_eq!(state_cookie(&headers), Some(id));
    }

    #[test]
    fn state_cookie_rejects_missing_or_malformed_value() {
        let mut headers = HeaderMap::new();
        assert_eq!(state_cookie(&headers), None);
        headers.insert(COOKIE, format!("{}=not-a-uuid", STATE_COOKIE).parse().unwrap());
        assert_eq!(state_cookie(&headers), None);
    }
}
