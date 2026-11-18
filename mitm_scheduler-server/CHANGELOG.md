# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.3.7] - 2026-11-18

### Fixed
- **Jobs Blocking on Unread Output ([#96](https://github.com/Zheng-Bote/mitm_core-layer/issues/96)):** Jobs were started with piped `stdout`/`stderr` that were never read, so a job blocked forever once it had written 64 KiB (e.g. `mitm_delivery_cority` hanging after its last successful delivery, ignoring `max_runtime_minutes`). The scheduler now reads both pipes until EOF and writes each line to its own log at level `DEBUG` (`[job <name> run <id> stdout|stderr] <line>`, max. 4096 bytes per line). The `program_runs` update does not wait for the readers.

## [1.3.6] - 2026-11-17

### Added
- **Startup Cleanup of Orphaned `program_runs` ([#91](https://github.com/Zheng-Bote/mitm_core-layer/issues/91)):** On startup (after passing the singleton UDS check and before starting `CronScheduler`), `mitm-scheduler-server` updates any unfinished `program_runs` entries (`exit_code IS NULL OR success IS NULL`) to `exit_code = 2`, `success = false`, and `finished_at = COALESCE(finished_at, CURRENT_TIMESTAMP)`, logging the count to `system_logs`.

## [1.3.5] - 2026-11-17

### Fixed
- **Graceful Job Termination on Shutdown ([#89](https://github.com/Zheng-Bote/mitm_core-layer/issues/89)):** `JobOrchestrator::stop_all()` now sets a `shutting_down` flag (preventing new cron jobs and `restart_on_exit` restarts during shutdown), sends `SIGTERM` to all active job PIDs, waits up to 10s for `running_jobs` to drain while keeping the UDS IPC accept loop active (so final `cancelled` status/audit events and `program_runs` updates are persisted), and escalates to `SIGKILL` for any remaining job PIDs.

## [1.3.4] - 2026-11-10

### Fixed
- **Logging**: A job exiting with a non-zero exit code (`Job <name> failed with code <code>`) and errors while waiting for a job process are now also written to `system_logs` with level `ERROR` (Feature spec `job-failure-logging`).


## [1.3.3] - 2026-10-08

### Added
- **Crypto**: Implemented `CryptoGenerateWrappedDek` IPC handler to allow the HTTP server to request a freshly generated DEK when the key rotation table is empty.


## [1.3.2] - 2026-10-08

### Fixed
- **Job Orchestrator**: Replaced external `/bin/kill` command with direct `libc::kill` syscalls. This ensures signals (`SIGTERM` and `SIGKILL`) are properly delivered to child processes (like `mitm_delivery_cority`) in distroless or minimal container environments where the `kill` binary is absent.

## [1.3.1] - 2026-10-06

### Fixed
- **Database**: Added a retry loop for PostgreSQL connection on startup to prevent `PoolTimedOut` crashes in environments with slow database provisioning (like AWS ECS + RDS). Respects `MITM_DB_CONNECT_DELAY` configuration.
- **Dependencies**: Downgraded `sqlx` to `=0.7.3` due to a known regression in `0.7.4` causing `PoolTimedOut` errors in AWS environments.

## [1.1.0] - 2026-09-27

### Added

- **Configuration**: The configuration loader now prioritizes explicitly provided command-line config files over Environment Variables. If no CLI parameter is provided, it gracefully falls back to ENVs before checking default locations.
- **Logging**: All core components (`http-server`, `iam-server`, `scheduler-server`) now initialize `env_logger` using the `MITM_LOG_LEVEL` environment variable (defaulting to `INFO` if not set), ensuring consistent log outputs across the stack.
- **Admin Authentication**: Implemented "Trust Proxy" (Option A) authentication for frontend users. The IAM server now skips explicit password validation for admin users defined via `MITM_ADMINS` or frontend requests, instead verifying user existence and loading the assigned RBAC roles.
- **Documentation**: Added an Apache-2.0 `NOTICE` file.
- **Tasks**: Completed and verified tasks for DB startup/shutdown logging across all services.

## [1.0.1] - 2026-09-23

### Fixed
- **Issue #30**: Decoded `MASTER_KEY` from Base64 (if 44 chars) to properly decrypt the DEK during IPC `CryptoEncrypt`/`CryptoDecrypt` requests from the HTTP server.
- **Issue #30**: Fixed swapped `nonce` and `ciphertext` variables in `CryptoEncrypt` IPC response, which caused the DB to store invalid nonces and subsequently crash during decryption.

## [1.0.0] - 2026-09-20

### Added
- **Issue #4**: Implemented `ExecuteJob` IPC integration to orchestrate jobs dynamically via the API Gateway.
- **Issue #5**: Implemented full Job Lifecycle tracking. Added `StopJob` IPC handling, mapping active PIDs in `JobOrchestrator`, and utilizing `SIGTERM` for process termination.
- **Issue #13**: Ported missing backward-compatibility features from the legacy Go scheduler:
  - Added `restart_on_exit` loop functionality to restart crashed child jobs.
  - Added 5-second `SIGKILL` timeout escalation for frozen tasks.
  - Integrated real-time PID synchronization with PostgreSQL upon process spawn.
  - Added Graceful Shutdown logic (trapping `SIGINT`/`SIGTERM`) to terminate active children on exit.
