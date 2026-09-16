# Feature: Full Scheduler Functionality

## Goal
Parity with the Go implementation of the Job Scheduler engine. Implement database polling for scheduled jobs, execution tracking, concurrency locking, and secure environment variable injection for IPC.

## Context
The scheduler engine manages child processes. It ensures jobs are not run concurrently if already running, logs start/end times and exit codes to the `program_runs` table, and injects critical environment variables (`RUN_ID`, `SCHEDULER_SOCKET_PATH`) so child jobs can authenticate back via Unix Domain Sockets.

## Tasks

### 1. Database Access Layer
- [ ] Add `chrono` dependency (or use `chrono` feature in `sqlx`).
- [ ] Implement `get_enabled_programs` in `db.rs` to fetch jobs from the `programs` table.
- [ ] Implement `create_program_run` and `update_program_run` in `db.rs`.

### 2. Job Orchestrator & Concurrency
- [ ] Implement an in-memory active job tracker (using `Arc<Mutex<HashMap>>` or `DashMap`) to prevent concurrent execution of the same job.
- [ ] Inject `RUN_ID` and `SCHEDULER_SOCKET_PATH` into the child process environment, explicitly clearing other non-whitelisted environment variables.

### 3. Cron Processing Loop
- [ ] Implement a background Tokio task in `main.rs` that uses `tokio-cron-scheduler` (or polls DB and calculates time offsets via `cron` crate) to trigger jobs based on their `cron_expr`.
