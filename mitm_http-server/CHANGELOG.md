# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.6.1] - 2026-10-08

### Added
- **API V1 Extensions**: Added missing `POST` (Create/Update) and `DELETE` endpoints for `/api/v1/config/transformations`, `/api/v1/config/transformations/rules`, and `/api/v1/config/transformations/validations` to allow the Admin UI to persist configuration data.

## [1.6.0] - 2026-10-07
### Added
- **Auth & Audit**: Implemented Admin Auth Audit logging (`USER_LOGIN` events) into the database on successful authentication.
- **Session Context**: The `/api/v1/auth/me` endpoint now returns the `os_user` field in the `RolesResponse`, allowing client UIs to display the logged-in user dynamically.

### Changed
- **API Clean-up**: Removed legacy `v0` API endpoints (`/admin/*`) completely in favor of the new `/api/v1/*` REST structure.
- **API V1 Extensions**: Restored previously missing endpoints into `/api/v1/*` (e.g., file upload, system actions, topic dependencies deletion) to support C++ client migration.
- **RBAC**: Enforced explicit Casbin policies on `/api/v1/jobs` endpoints to restrict execution/stopping strictly to `USER` or `ADMIN`, while `VIEWER` is restricted to read-only access.

## [1.5.1] - 2026-10-06

### Fixed
- **Dependencies**: Downgraded `sqlx` to `=0.7.3` due to a known regression in `0.7.4` causing `PoolTimedOut` errors in AWS environments.

## [1.5.0] - 2026-10-05

### Added
- **API Architecture:** Refactored the entire API from module-based paths (`/api/admin/v1`, `/api/transformation/v1`) to a strict resource-oriented RESTful layout under `/api/v1/*` (e.g., `/api/v1/system`, `/api/v1/jobs`, `/api/v1/logs`, `/api/v1/dlq`, `/api/v1/config`, `/api/v1/iam`, `/api/v1/auth`).
- **Dashboard Stats:** Expanded the `/api/v1/system/dashboard` endpoint to include nested metrics (count and oldest timestamp) for DLQ, Transformation Errors, System Logs, Audit Logs, Jobs, and Users.
- **Hierarchical RBAC:** Reconfigured Casbin with the grouping matcher (`g(r.sub, p.sub)`) to support hierarchical role inheritance, allowing `ADMIN` to inherit `USER`, and `USER` to inherit `VIEWER`. Updated the seeded Casbin policies to reflect this hierarchy.

### Changed
- **Auth Middleware:** Updated the authentication middleware bypass rules to cleanly exclude public endpoints (`/api/v1/auth/session`, `/api/v1/system/info`, `/api/v1/system/time`).
- **Documentation:** Extensively updated `api_v1.md` to document the new resource-oriented endpoints and their corresponding minimum role requirements.

## [1.4.0] - 2026-10-03

### Added
- **API Versioning**: Added the `### 2.5 User API` namespace (`/api/user/v1`) to the `api_v1.md` documentation, formally introducing the `POST /api/user/v1/session` and `GET /api/user/v1/roles` endpoints for managing the currently authenticated user's session.
- **Security Policy**: Officially documented a maximum 24h Absolute TTL and a 2h Idle Timeout for user sessions managed via PostgreSQL.

## [1.3.1] - 2026-09-30

### Added
- **System Info:** Added `database.size` property to the `/api/system/v1/info` endpoint output.
- **Audit Logging:** Access denied events (`403 Forbidden`) by Casbin are now properly logged to `admin_audit_logs` including username and details.

### Fixed
- **SPA Routing:** Bypassed the `readiness_middleware` for the root path `/` to return a `200 OK` (via SPA fallback) immediately upon startup without yielding `503 Service Unavailable`.
- **Casbin AuthZ:** Fixed the Casbin matcher to use explicit role matching and `keyMatch` for wildcards. Also fixed Axum `req.uri().path()` truncation in nested routers by relying on `axum::extract::OriginalUri` so Casbin correctly evaluates the full URI.
- **System Info:** Simplified the database version string in `/api/system/v1/info` to match the format seen in `/admin/dashboard/stats` (e.g. `PostgreSQL 18.2`).

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

- **Admin API**: Replaced all remaining mock endpoints in `admin.rs` (`/admin/credentials`, `/admin/delivery_targets`) with actual database queries and IAM IPC integration (AES-GCM encryption/decryption).
- **Admin API**: Fixed mocked backup version logic. `/admin/backup` and `/admin/restore` now dynamically use and validate against the current application build version (`env!("CARGO_PKG_VERSION")`).
- **Admin API**: Fixed `/admin/key-rotation` which was previously mocked. The endpoint now correctly rotates the Master-Key, marks old DEKs as inactive, inserts the new encrypted DEK, and stores a correct audit log count.
- **Admin API**: Fixed HTTP 500 Internal Server Error on `/admin/transformation/errors_bin` by allowing `raw_ingestion_id` to safely parse `NULL` values from PostgreSQL.
- **Log API**: Fixed date-range filtering (`?from` & `?to`) across all log endpoints (`system_logs`, `job_audit_logs`, `admin_audit_logs`). Added fallback parser for simple date formats (`yyyy-MM-dd`) sent by the frontend, properly adjusting the time boundaries to cover the full day.
- Replaced all mock endpoints in `admin.rs` with actual database queries and UDS IPC.
- Fixed version logic in `backup` / `restore`.
- Fixed `/key-rotation` endpoint.
- Fixed date-range parsing for `?from` & `?to` query params in logs to support frontend `yyyy-MM-dd` formats.
- Fixed nullable UUID parse crash in `errors_bin`.

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
