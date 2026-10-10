# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.4.0] - 2026-10-10
### Added
- **RBAC Enhancements (Issue #53):** Included `get_user` method in `db.rs` to fetch user profile (`first_name`, `last_name`, `is_active`). Inactive users are now properly rejected during authentication.

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
- **Issue #28**: Decoded `MASTER_KEY` from Base64 (if 44 chars) for Envelope Encryption to fix key mismatch with Go collectors.
- **Security**: Updated password hash checking in `db.rs` to use `subtle::ConstantTimeEq` to prevent compiler optimizations bypassing constant-time guarantees.

## [1.0.0] - 2026-09-20

### Added
- Envelope Encryption engine via AES-GCM and Argon2id.
- Initial UDS (Unix Domain Socket) IPC API for Authentication.

### Fixed
- **Issue #2**: Integrated PostgreSQL for RBAC. Hardcoded `ADMIN` roles were replaced with dynamic database queries using `sqlx` in `db.rs` to fetch actual roles (`get_user_roles`).
