# Changelog

All notable changes to the MitM-2 Core Layer project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0] - 2026-09-27

### Added

- **Configuration**: The configuration loader now prioritizes explicitly provided command-line config files over Environment Variables. If no CLI parameter is provided, it gracefully falls back to ENVs before checking default locations.
- **Logging**: All core components (`http-server`, `iam-server`, `scheduler-server`) now initialize `env_logger` using the `MITM_LOG_LEVEL` environment variable (defaulting to `INFO` if not set), ensuring consistent log outputs across the stack.
- **Admin Authentication**: Implemented "Trust Proxy" (Option A) authentication for frontend users. The IAM server now skips explicit password validation for admin users defined via `MITM_ADMINS` or frontend requests, instead verifying user existence and loading the assigned RBAC roles.
- **Documentation**: Added an Apache-2.0 `NOTICE` file.
- **Tasks**: Completed and verified tasks for DB startup/shutdown logging across all services.

### Fixed

- **Process Orchestration**: The `http-server` supervisor no longer defaults to passing a hardcoded `"config.json"` string to child processes if it was started without parameters. It now dynamically inherits the parameter behavior, ensuring ENV-only startups function correctly.
- Removed unused imports and fixed all compiler warnings (e.g., `sqlx::postgres` in `db.rs`).

## [1.0.2] - 2026-09-23

### Changed

- Replaced hardcoded "admin" username strings with dynamically loaded usernames in the HTTP server.
- Refactored `mitm_common::config::load_config` for better resilience.
- Distributed Tracing via `X-Request-ID` middleware over UDS implemented.
