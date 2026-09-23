# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
