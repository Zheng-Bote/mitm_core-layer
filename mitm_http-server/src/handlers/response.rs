/**
 * SPDX-FileComment: Standardized JSON API Response Envelope and Request-ID Middleware
 * SPDX-FileType: SOURCE
 * SPDX-FileContributor: ZHENG Robert
 * SPDX-FileCopyrightText: 2026 ZHENG Robert
 * SPDX-License-Identifier: Apache-2.0
 *
 * @file response.rs
 * @brief Implements the JSON:API-derived response envelope (id, status, code, title, detail, meta, data)
 *        and X-Request-ID middleware for RMM Level 2 compliance.
 * @version 3.5.0
 * @date 2026-11-17
 *
 * @author ZHENG Robert (robert@hase-zheng.net)
 * @copyright Copyright (c) 2026 ZHENG Robert
 * @license Apache-2.0
 */

use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::{HeaderName, HeaderValue, StatusCode, header::CONTENT_TYPE},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
const API_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_ENVELOPE_BODY_BYTES: usize = 64 * 1024 * 1024;

/// Per-request UUID stored in request extensions and returned in `X-Request-ID` and `ApiResponse.id`.
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

impl RequestId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

/// Metadata block of every standardized API response.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ApiMeta {
    pub timestamp: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
}

impl Default for ApiMeta {
    fn default() -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: API_VERSION.to_string(),
            count: None,
            limit: None,
            offset: None,
        }
    }
}

/// Standardized JSON:API-derived response object for both `2xx` and `4xx`/`5xx` responses.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApiResponse<T = Value> {
    pub id: String,
    pub status: u16,
    pub code: String,
    pub title: String,
    pub detail: Option<String>,
    pub meta: ApiMeta,
    pub data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn new(
        http_status: StatusCode,
        code: impl Into<String>,
        title: impl Into<String>,
        detail: Option<String>,
        data: Option<T>,
    ) -> Self {
        Self {
            id: String::new(),
            status: http_status.as_u16(),
            code: code.into(),
            title: title.into(),
            detail,
            meta: ApiMeta::default(),
            data,
        }
    }

    pub fn ok(data: T) -> Self {
        Self::new(StatusCode::OK, "OK", "Success", None, Some(data))
    }

    pub fn created(data: T) -> Self {
        Self::new(StatusCode::CREATED, "CREATED", "Resource Created", None, Some(data))
    }

    pub fn accepted(detail: impl Into<String>, data: Option<T>) -> Self {
        Self::new(StatusCode::ACCEPTED, "ACCEPTED", "Accepted", Some(detail.into()), data)
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_count(mut self, count: usize) -> Self {
        self.meta.count = Some(count);
        self
    }
}

impl ApiResponse<Value> {
    pub fn updated(detail: impl Into<String>, data: Option<Value>) -> Self {
        Self::new(StatusCode::OK, "UPDATED", "Resource Updated", Some(detail.into()), data)
    }

    pub fn deleted(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::OK, "DELETED", "Resource Deleted", Some(detail.into()), None)
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(mut self) -> Response {
        if self.id.is_empty() {
            self.id = Uuid::new_v4().to_string();
        }
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::OK);
        (status, Json(self)).into_response()
    }
}

/// Standardized error builder that serializes into the `ApiResponse<Value>` envelope (`data: None`).
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: String,
    pub title: String,
    pub detail: Option<String>,
}

impl ApiError {
    pub fn new(
        status: StatusCode,
        code: impl Into<String>,
        title: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        let d = detail.into();
        Self {
            status,
            code: code.into(),
            title: title.into(),
            detail: if d.is_empty() { None } else { Some(d) },
        }
    }

    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "BAD_REQUEST", "Bad Request", detail)
    }

    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "AUTH_REQUIRED", "Unauthorized", detail)
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "RBAC_FORBIDDEN", "Forbidden", detail)
    }

    pub fn user_inactive(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "USER_INACTIVE", "Forbidden", detail)
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", "Not Found", detail)
    }

    pub fn conflict(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "CONFLICT", "Conflict", detail)
    }

    pub fn validation_failed(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_FAILED",
            "Validation Failed",
            detail,
        )
    }

    pub fn too_many_requests(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "Too Many Requests",
            detail,
        )
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Internal Server Error",
            detail,
        )
    }

    pub fn service_unavailable(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "Service Unavailable",
            detail,
        )
    }

    pub fn into_envelope(self, request_id: String) -> ApiResponse<Value> {
        ApiResponse {
            id: request_id,
            status: self.status.as_u16(),
            code: self.code,
            title: self.title,
            detail: self.detail,
            meta: ApiMeta::default(),
            data: None,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status;
        let envelope = self.into_envelope(Uuid::new_v4().to_string());
        (status, Json(envelope)).into_response()
    }
}

/// Default application `code` and `title` for a given HTTP status code.
pub fn default_code_and_title(status: StatusCode) -> (&'static str, &'static str) {
    match status {
        StatusCode::OK => ("OK", "Success"),
        StatusCode::CREATED => ("CREATED", "Resource Created"),
        StatusCode::ACCEPTED => ("ACCEPTED", "Accepted"),
        StatusCode::NO_CONTENT => ("OK", "No Content"),
        StatusCode::BAD_REQUEST => ("BAD_REQUEST", "Bad Request"),
        StatusCode::UNAUTHORIZED => ("AUTH_REQUIRED", "Unauthorized"),
        StatusCode::FORBIDDEN => ("RBAC_FORBIDDEN", "Forbidden"),
        StatusCode::NOT_FOUND => ("NOT_FOUND", "Not Found"),
        StatusCode::METHOD_NOT_ALLOWED => ("METHOD_NOT_ALLOWED", "Method Not Allowed"),
        StatusCode::CONFLICT => ("CONFLICT", "Conflict"),
        StatusCode::UNPROCESSABLE_ENTITY => ("VALIDATION_FAILED", "Validation Failed"),
        StatusCode::TOO_MANY_REQUESTS => ("RATE_LIMITED", "Too Many Requests"),
        StatusCode::SERVICE_UNAVAILABLE => ("SERVICE_UNAVAILABLE", "Service Unavailable"),
        s if s.is_client_error() => ("CLIENT_ERROR", s.canonical_reason().unwrap_or("Client Error")),
        _ => ("INTERNAL_ERROR", status.canonical_reason().unwrap_or("Internal Server Error")),
    }
}

fn is_envelope_json(val: &Value) -> bool {
    matches!(val, Value::Object(map)
        if map.contains_key("id")
            && map.contains_key("status")
            && map.contains_key("code")
            && map.contains_key("title")
            && map.contains_key("meta")
            && map.contains_key("data"))
}

fn is_binary_or_file_content(content_type: Option<&str>, has_attachment: bool) -> bool {
    if has_attachment {
        if let Some(ct) = content_type {
            if ct.starts_with("application/octet-stream")
                || ct.starts_with("application/x-flatbuffers")
                || ct.starts_with("application/sql")
            {
                return true;
            }
        }
    }
    matches!(
        content_type,
        Some(ct)
            if ct.starts_with("application/octet-stream")
                || ct.starts_with("application/x-flatbuffers")
                || ct.starts_with("application/sql")
    )
}

/// Normalizes any JSON or plain-text `/api/v1/*` response payload into the standardized `ApiResponse` envelope.
pub fn normalize_to_envelope(status: StatusCode, request_id: &str, body_bytes: &[u8]) -> Value {
    let (default_code, default_title) = default_code_and_title(status);

    if let Ok(mut parsed) = serde_json::from_slice::<Value>(body_bytes) {
        // 1. Already an ApiResponse envelope: ensure `id` and `status` match the request/response.
        if is_envelope_json(&parsed) {
            if let Some(obj) = parsed.as_object_mut() {
                obj.insert("id".to_string(), Value::String(request_id.to_string()));
                obj.insert("status".to_string(), Value::Number(status.as_u16().into()));
            }
            return parsed;
        }

        // 2. Legacy `{"errors": [{"status", "title", "detail"}]}` payload.
        if let Some(errors) = parsed.get("errors").and_then(|v| v.as_array()) {
            if let Some(first) = errors.first() {
                let title = first
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or(default_title)
                    .to_string();
                let detail = first
                    .get("detail")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let code = first
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or(default_code)
                    .to_string();
                let env = ApiResponse::<Value> {
                    id: request_id.to_string(),
                    status: status.as_u16(),
                    code,
                    title,
                    detail,
                    meta: ApiMeta::default(),
                    data: None,
                };
                return serde_json::to_value(env).unwrap_or(Value::Null);
            }
        }

        // 3. Error status with custom JSON body.
        if status.is_client_error() || status.is_server_error() {
            let detail = parsed
                .get("detail")
                .or_else(|| parsed.get("message"))
                .or_else(|| parsed.get("error"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let env = ApiResponse::<Value> {
                id: request_id.to_string(),
                status: status.as_u16(),
                code: default_code.to_string(),
                title: default_title.to_string(),
                detail,
                meta: ApiMeta::default(),
                data: None,
            };
            return serde_json::to_value(env).unwrap_or(Value::Null);
        }

        // 4. 2xx JSON payload (object or array): wrap in `data` and populate `meta.count` for arrays.
        let count = parsed.as_array().map(|a| a.len());
        let detail = if parsed.is_object()
            && parsed.as_object().map(|m| m.len() == 1 && m.contains_key("message")).unwrap_or(false)
        {
            parsed.get("message").and_then(|v| v.as_str()).map(|s| s.to_string())
        } else {
            None
        };
        let mut meta = ApiMeta::default();
        meta.count = count;
        let env = ApiResponse::<Value> {
            id: request_id.to_string(),
            status: status.as_u16(),
            code: default_code.to_string(),
            title: default_title.to_string(),
            detail,
            meta,
            data: Some(parsed),
        };
        return serde_json::to_value(env).unwrap_or(Value::Null);
    }

    // 5. Empty or plain-text body (e.g. `200 OK` with empty body or Axum extractor rejection text).
    let text = String::from_utf8_lossy(body_bytes).trim().to_string();
    let detail = if text.is_empty() { None } else { Some(text) };
    let status_out = if status == StatusCode::NO_CONTENT {
        StatusCode::OK
    } else {
        status
    };
    let (code, title) = default_code_and_title(status_out);
    let env = ApiResponse::<Value> {
        id: request_id.to_string(),
        status: status_out.as_u16(),
        code: code.to_string(),
        title: title.to_string(),
        detail,
        meta: ApiMeta::default(),
        data: None,
    };
    serde_json::to_value(env).unwrap_or(Value::Null)
}

/// Middleware that assigns a UUID `RequestId` (`X-Request-ID`) to every request and guarantees
/// that all `/api/v1/*` JSON responses conform to the `ApiResponse` envelope.
pub async fn request_id_and_envelope_middleware(mut req: Request, next: Next) -> Response {
    let request_id = req
        .headers()
        .get(&X_REQUEST_ID)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .map(|u| u.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    req.extensions_mut().insert(RequestId(request_id.clone()));
    let path = req.uri().path().to_string();

    let mut res = next.run(req).await;
    if let Ok(header_val) = HeaderValue::from_str(&request_id) {
        res.headers_mut().insert(X_REQUEST_ID, header_val);
    }

    if !path.starts_with("/api/v1") {
        return res;
    }

    let status = res.status();
    // Browser OIDC navigation endpoints respond with 3xx redirects.
    if status.is_redirection() {
        return res;
    }

    let content_type = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let has_attachment = res
        .headers()
        .get(axum::http::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("attachment"))
        .unwrap_or(false);

    // Binary FlatBuffers streams on 200 OK stay binary.
    if status.is_success() && is_binary_or_file_content(content_type.as_deref(), has_attachment) {
        return res;
    }

    let (mut parts, body) = res.into_parts();
    let bytes = match to_bytes(body, MAX_ENVELOPE_BODY_BYTES).await {
        Ok(b) => b,
        Err(_) => {
            return ApiError::internal("Response payload exceeded maximum buffer size")
                .into_envelope(request_id)
                .into_response();
        }
    };

    let normalized_status = if status == StatusCode::NO_CONTENT {
        StatusCode::OK
    } else {
        status
    };
    let envelope = normalize_to_envelope(normalized_status, &request_id, &bytes);
    let body_out = serde_json::to_vec(&envelope).unwrap_or_default();

    parts.status = normalized_status;
    parts.headers.remove(axum::http::header::CONTENT_LENGTH);
    parts.headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );

    Response::from_parts(parts, Body::from(body_out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, routing::{get, post}};
    use tower::ServiceExt;

    async fn read_json(res: Response) -> Value {
        let bytes = to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn wraps_success_array_with_request_id_and_count() {
        let app = Router::new()
            .route("/api/v1/items", get(|| async { Json(serde_json::json!([{"id": 1}, {"id": 2}])) }))
            .layer(axum::middleware::from_fn(request_id_and_envelope_middleware));

        let res = app
            .oneshot(Request::builder().uri("/api/v1/items").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        let header_id = res.headers().get(&X_REQUEST_ID).unwrap().to_str().unwrap().to_string();
        let body = read_json(res).await;

        assert_eq!(body["id"], header_id);
        assert_eq!(body["status"], 200);
        assert_eq!(body["code"], "OK");
        assert_eq!(body["title"], "Success");
        assert!(body["detail"].is_null());
        assert_eq!(body["meta"]["count"], 2);
        assert_eq!(body["data"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn converts_legacy_errors_and_api_errors_into_envelope() {
        let app = Router::new()
            .route(
                "/api/v1/legacy-err",
                get(|| async {
                    let err = crate::handlers::ErrorResponse {
                        errors: vec![crate::handlers::JsonApiError {
                            status: "403".into(),
                            title: "Forbidden".into(),
                            detail: Some("Access denied".into()),
                        }],
                    };
                    (StatusCode::FORBIDDEN, Json(err))
                }),
            )
            .route(
                "/api/v1/typed-err",
                post(|| async { ApiError::validation_failed("Invalid cron expression") }),
            )
            .layer(axum::middleware::from_fn(request_id_and_envelope_middleware));

        let res = app
            .clone()
            .oneshot(Request::builder().uri("/api/v1/legacy-err").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
        let body = read_json(res).await;
        assert_eq!(body["status"], 403);
        assert_eq!(body["code"], "RBAC_FORBIDDEN");
        assert_eq!(body["title"], "Forbidden");
        assert_eq!(body["detail"], "Access denied");
        assert!(body["data"].is_null());

        let res2 = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/typed-err")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res2.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body2 = read_json(res2).await;
        assert_eq!(body2["status"], 422);
        assert_eq!(body2["code"], "VALIDATION_FAILED");
        assert_eq!(body2["detail"], "Invalid cron expression");
        assert!(body2["data"].is_null());
    }

    #[tokio::test]
    async fn preserves_flatbuffers_binary_and_oidc_redirect() {
        let app = Router::new()
            .route(
                "/api/v1/logs/system",
                get(|| async {
                    (
                        StatusCode::OK,
                        [(CONTENT_TYPE, "application/octet-stream")],
                        vec![1_u8, 2, 3, 4],
                    )
                }),
            )
            .route(
                "/api/v1/auth/oidc/login",
                get(|| async { axum::response::Redirect::to("https://idp.example.com/auth") }),
            )
            .layer(axum::middleware::from_fn(request_id_and_envelope_middleware));

        let bin_res = app
            .clone()
            .oneshot(Request::builder().uri("/api/v1/logs/system").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(bin_res.status(), StatusCode::OK);
        assert!(bin_res.headers().contains_key(&X_REQUEST_ID));
        let bytes = to_bytes(bin_res.into_body(), 1024).await.unwrap();
        assert_eq!(&bytes[..], &[1, 2, 3, 4]);

        let redir_res = app
            .oneshot(Request::builder().uri("/api/v1/auth/oidc/login").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(redir_res.status(), StatusCode::SEE_OTHER);
    }
}
