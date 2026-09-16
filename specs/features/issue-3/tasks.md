# Feature: HTTP API Gateway with Axum & IAM IPC

## Goal
Implement `mitm_http-server` as the API Gateway for the core-layer. It routes external HTTP/HTTPS traffic using `axum`, extracts Basic Auth headers, and validates them by communicating with the `mitm_iam-server` over Unix Domain Sockets (IPC).

## Context
The previous Go implementation handled HTTP and auth in one monolithic process. In the Rust port, this microservice focuses exclusively on HTTP/HTTPS multiplexing and API logic, delegating authorization to the IAM server.

## Tasks

### 1. HTTP Server Setup
- [ ] Add `tokio`, `axum`, `rustls` (or `axum-server` for TLS), `serde_json`, `base64` dependencies in `Cargo.toml`.
- [ ] Setup `main.rs` to load the config via `mitm_common::config::load_config`.
- [ ] Start an `axum` HTTPS server bound to `config.http_port` using the certificates from `ssl_cert`/`ssl_key`.

### 2. IAM IPC Client Integration
- [ ] Create `ipc_client.rs` to abstract the Unix Domain Socket connection to `/tmp/mitm_iam.sock`.
- [ ] Implement a function to send `IpcRequest::Authenticate` and parse `IpcResponse::AuthenticateResult`.

### 3. API Endpoints & Auth Middleware
- [ ] Implement `/admin/action` (POST) parsing strict JSON payloads.
- [ ] Implement an Axum middleware or extractor for Basic Auth that invokes the IPC client.
- [ ] Return `401 Unauthorized` strictly adhering to the JSON error schema `{"error": "Unauthorized", "code": 401}` if IPC auth fails.
