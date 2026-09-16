# Feature: Scheduler Server & Job Orchestration

## Goal
Implement the `mitm_scheduler-server` microservice in Rust. This service acts as the orchestration engine, executing collector/transformation/delivery jobs and receiving status updates from them via Unix Domain Sockets (IPC).

## Context
In the Go implementation, `scheduler` was a monolithic daemon handling HTTP, IAM, and jobs. In this microservice architecture, it is dedicated to job lifecycle management, interacting with `sqlx` to update job states and launching external binaries as child processes.

## Tasks

### 1. Scheduler Setup & DB Integration
- [ ] Add `tokio` (with `process` feature), `sqlx` (PostgreSQL), `serde_json`, `log` to `Cargo.toml`.
- [ ] Load DB configuration using `mitm_common::config::load_config` and initialize a PostgreSQL connection pool.

### 2. Job IPC Listener (Collectors)
- [ ] Implement a `UnixListener` on `/tmp/mitm_scheduler.sock` to listen for job status events.
- [ ] Define `StatusEvent` and `CredentialsResponse` JSON structures mirroring the Go `ipc.go` definitions.
- [ ] Implement a simple event loop processing incoming JSON payloads from spawned jobs.

### 3. Job Execution Engine
- [ ] Create a basic asynchronous job runner using `tokio::process::Command`.
- [ ] Ensure the scheduler can launch external binaries (e.g. `mitm_collector_pg`) and capture their completion status.
