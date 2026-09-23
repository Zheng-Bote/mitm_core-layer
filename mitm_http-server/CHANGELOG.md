# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.2] - 2026-09-23

### Fixed
- **Issue #30**: Removed hardcoded "admin" username from audit logging in `admin.rs`, properly extracting the authenticated user from the request extension.
- **Issue #30**: Fixed RBAC role reporting for in-memory config admins so they correctly receive the `ADMIN` role in the frontend.

## [1.0.1] - 2026-09-23

### Fixed
- Fixed encrypted config file loading by replacing hardcoded empty string with `MASTER_KEY` environment variable in `load_config` call.

## [1.0.0] - 2026-09-20

### Added
- Initial REST JSON:API Gateway for MitM-2 using Axum.

### Changed
- **Issue #3**: Integrated dynamic DB Configuration and IPC parameters via `AppState`. Extracted UDS paths directly from the global environment instead of using hardcoded paths.
- **Issue #3**: Enhanced authorization flow by injecting dynamic RBAC claims (fetched via the IAM UDS layer) into the Axum request extensions.
