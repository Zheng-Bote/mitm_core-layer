# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
