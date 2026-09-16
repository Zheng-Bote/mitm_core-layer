# Feature: IAM Server with IPC & RBAC Database Access

## Goal
Implement the `mitm_iam-server` microservice in Rust. It runs independently, connects to PostgreSQL to manage roles and identities, and exposes its services via Unix Domain Sockets (IPC) to other components (like `mitm_http-server`).

## Context
The IAM server is the source of truth for authentication and authorization. It utilizes `tokio` for async runtime and `sqlx` for async database access. Communication is strictly internal via IPC.

## Tasks

### 1. Define IPC Protocol (`mitm_common`)
- [ ] Define shared IPC JSON request/response structures in `mitm_common::ipc` using `serde`.
- [ ] Define `AuthRequest` and `AuthResponse`.

### 2. IAM Server Dependencies & DB Bootstrap
- [ ] Update `mitm_iam-server/Cargo.toml` with `tokio`, `sqlx` (PostgreSQL), `serde_json`.
- [ ] Implement database connection pooling (`sqlx::PgPool`).
- [ ] Implement `bootstrap_admins()` logic porting from Go (loads `s.Admins` from config, creates them in DB with `ADMIN` role if they do not exist).

### 3. IAM IPC Server
- [ ] Implement a `tokio::net::UnixListener` that accepts incoming UDS connections.
- [ ] Parse incoming JSON lines into IPC request structures.
- [ ] Implement the `authenticate` handler (In-Memory check first -> DB fallback) and return JSON responses.
