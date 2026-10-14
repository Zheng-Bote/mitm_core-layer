/*
 * SPDX-License-Identifier: Apache-2.0
 */

//! Guards for the unauthenticated WebAuthn endpoints: server-side client IP resolution,
//! per-IP rate limiting and bootstrap token validation.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, StatusCode, header::RETRY_AFTER},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use zeroize::Zeroizing;
use crate::handlers::{AppState, ErrorResponse, JsonApiError};

pub const MIN_BOOTSTRAP_TOKEN_LEN: usize = 32;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const RATE_LIMIT_MAX_REQUESTS: u32 = 20;

/// Resolves the client IP from the connection, never from the request payload.
/// With `trust_proxy_headers` the rightmost `X-Forwarded-For` entry wins, which is the
/// address the load balancer (e.g. AWS ALB) received the request from.
pub fn client_ip(headers: &HeaderMap, peer: Option<&ConnectInfo<SocketAddr>>, trust_proxy_headers: bool) -> Option<IpAddr> {
    if trust_proxy_headers {
        let forwarded = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit(',').next())
            .and_then(|ip| ip.trim().parse().ok());
        if forwarded.is_some() {
            return forwarded;
        }
    }
    peer.map(|ConnectInfo(addr)| addr.ip())
}

/// Fixed-window request counter per client IP.
#[derive(Default)]
pub struct RateLimiter {
    windows: Mutex<HashMap<IpAddr, (Instant, u32)>>,
}

impl RateLimiter {
    /// Counts a request from `ip`; returns false once the budget of the current window is exceeded.
    fn allow(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut windows = self.windows.lock().unwrap_or_else(|e| e.into_inner());
        windows.retain(|_, (start, _)| now.duration_since(*start) < RATE_LIMIT_WINDOW);
        let (_, count) = windows.entry(ip).or_insert((now, 0));
        *count += 1;
        *count <= RATE_LIMIT_MAX_REQUESTS
    }
}

pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    peer: Option<ConnectInfo<SocketAddr>>,
    req: Request,
    next: Next,
) -> Response {
    // Requests without a resolvable IP share one bucket instead of bypassing the limit.
    let ip = client_ip(req.headers(), peer.as_ref(), state.trust_proxy_headers)
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    if state.auth_rate_limiter.allow(ip) {
        return next.run(req).await;
    }
    log::warn!("Rate limit exceeded on {} for client {}", req.uri().path(), ip);
    let err = ErrorResponse { errors: vec![JsonApiError { status: "429".into(), title: "Too Many Requests".into(), detail: Some("Too many authentication requests. Please retry later.".into()) }] };
    (StatusCode::TOO_MANY_REQUESTS, [(RETRY_AFTER, RATE_LIMIT_WINDOW.as_secs().to_string())], Json(err)).into_response()
}

/// Returns `INITIAL_ADMIN_TOKEN` if it is set and has at least `MIN_BOOTSTRAP_TOKEN_LEN` characters.
pub fn bootstrap_token() -> Option<Zeroizing<String>> {
    std::env::var("INITIAL_ADMIN_TOKEN")
        .ok()
        .map(Zeroizing::new)
        .filter(|t| t.chars().count() >= MIN_BOOTSTRAP_TOKEN_LEN)
}

/// Constant-time comparison of `candidate` against the configured bootstrap token.
pub fn bootstrap_token_matches(candidate: &str) -> bool {
    match bootstrap_token() {
        Some(expected) => expected.len() == candidate.len()
            && openssl::memcmp::eq(expected.as_bytes(), candidate.as_bytes()),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(ip: &str) -> ConnectInfo<SocketAddr> {
        ConnectInfo(SocketAddr::new(ip.parse().unwrap(), 443))
    }

    fn forwarded(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", value.parse().unwrap());
        headers
    }

    #[test]
    fn client_ip_ignores_forwarded_header_unless_trusted() {
        let headers = forwarded("1.1.1.1");
        assert_eq!(client_ip(&headers, Some(&peer("10.0.0.5")), false), Some("10.0.0.5".parse().unwrap()));
    }

    #[test]
    fn client_ip_uses_rightmost_forwarded_entry_when_trusted() {
        let headers = forwarded("6.6.6.6, 203.0.113.7");
        assert_eq!(client_ip(&headers, Some(&peer("10.0.0.5")), true), Some("203.0.113.7".parse().unwrap()));
    }

    #[test]
    fn client_ip_falls_back_to_peer_without_forwarded_header() {
        assert_eq!(client_ip(&HeaderMap::new(), Some(&peer("10.0.0.5")), true), Some("10.0.0.5".parse().unwrap()));
    }

    #[test]
    fn rate_limiter_rejects_after_budget_per_ip() {
        let limiter = RateLimiter::default();
        let a: IpAddr = "192.0.2.1".parse().unwrap();
        let b: IpAddr = "192.0.2.2".parse().unwrap();
        for _ in 0..RATE_LIMIT_MAX_REQUESTS {
            assert!(limiter.allow(a));
        }
        assert!(!limiter.allow(a));
        assert!(limiter.allow(b));
    }

    #[test]
    fn bootstrap_token_requires_minimum_length_and_exact_match() {
        let valid = "a".repeat(MIN_BOOTSTRAP_TOKEN_LEN);
        // SAFETY: this is the only test that touches INITIAL_ADMIN_TOKEN.
        unsafe { std::env::set_var("INITIAL_ADMIN_TOKEN", &valid) };
        assert!(bootstrap_token_matches(&valid));
        assert!(!bootstrap_token_matches(&"b".repeat(MIN_BOOTSTRAP_TOKEN_LEN)));
        assert!(!bootstrap_token_matches("a"));

        unsafe { std::env::set_var("INITIAL_ADMIN_TOKEN", "a".repeat(MIN_BOOTSTRAP_TOKEN_LEN - 1)) };
        assert!(bootstrap_token().is_none());
        assert!(!bootstrap_token_matches(&"a".repeat(MIN_BOOTSTRAP_TOKEN_LEN - 1)));

        unsafe { std::env::remove_var("INITIAL_ADMIN_TOKEN") };
        assert!(bootstrap_token().is_none());
    }
}
