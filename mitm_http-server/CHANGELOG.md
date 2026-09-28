# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.1] - 2026-09-28

### Added
- **SPA Routing:** Configured the HTTP Server to act as a proper host for Single Page Applications (e.g. Angular). The fallback route now delegates `404 Not Found` requests to the static `index.html`, enabling client-side routing.

### Changed
- **Tera Templates:** Moved the dynamic Tera template rendering from the root (`/`) to the `/template/:name` namespace to avoid conflicts with static SPA assets.

## [1.2.0] - 2026-09-28

### Added
- **API Versioning (Issue #14):** Introduced a new API namespace `v1` (`/api/admin/v1`, `/api/transformation/v1`, `/api/public/v1`) while retaining full backward compatibility for the legacy `v0` API.
- **Dynamic Policy Engine (Casbin):** Implemented a new AuthZ middleware using `casbin` and `sqlx-adapter`. Policies are now dynamically evaluated against the PostgreSQL database.
- **Content Negotiation:** V1 endpoints for data-heavy operations (e.g., audit logs, transformations, DLQ) no longer use the `_bin` suffix for FlatBuffers. They now use the `Accept: application/x-flatbuffers` HTTP header.
- **REST Semantics:** Query-based resource identifiers in V1 routes are upgraded to Path variables (e.g., `/api/admin/v1/jobs/:name/stop`).
- **Documentation:** Added detailed API documentation `api_v1.md` and archived `api_v0.md`.

## [1.1.1] - 2026-09-28

### Fixed
- Replaced all mock endpoints in `admin.rs` with actual database queries and UDS IPC.
- Fixed version logic in `backup` / `restore`.
- Fixed `/key-rotation` endpoint.
- Fixed date-range parsing for `?from` & `?to` query params in logs to support frontend `yyyy-MM-dd` formats.
- Fixed nullable UUID parse crash in `errors_bin`.

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
