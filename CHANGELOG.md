# Changelog

All notable changes to the MitM-2 Core Layer project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **API V1 Extensions**: Added `DELETE /api/v1/config/credentials/:id` and `DELETE /api/v1/config/targets/:id` endpoints to allow `ADMIN` users to delete source credentials and delivery targets.

## [1.5.0] - 2026-10-05

### Added
- **API**: Added missing endpoints for system operations (`/api/v1/system/backup`, `/api/v1/system/restore`, `/api/v1/system/key-rotation`), user management (`GET /api/v1/users/:id`), and admin logs (`GET /api/v1/logs/admin-audit`).

### Changed
- **API Architecture**: Completely refactored API v1 routes to a RESTful resource-oriented design (`/api/v1/{resource}`).
- **RBAC**: Adjusted Casbin model and seeded policies to support role inheritance (`ADMIN` -> `USER` -> `VIEWER`), and mapped endpoints to appropriate roles.
- **Database**: Renamed the default `UPLOADER` role to `USER` in `migrations/setup.sql` and system configuration.
- **Documentation**: Updated `core-layer/mitm_http-server/docs/api/api_v1.md` to accurately reflect the new v1 API endpoints and backup parameters.

### Fixed
- **SPA Routing**: Fixed the Axum fallback router (`spa_service`) to correctly map `404 Not Found` to `200 OK` when serving the Angular `index.html` on browser refreshes.
- **Content Negotiation**: Fixed FlatBuffer vs JSON content negotiation by properly falling back to JSON when the `Accept: application/json` header is provided.

## [1.3.0] - 2026-09-28

### Added
- **Dynamic Component Versioning (Issue #38):** Extended the `mitm_common` IPC protocol with a new `GetInfo` request.
- **HTTP Info Refactoring:** The HTTP server's `/info` endpoint no longer hardcodes the component versions. It now fetches the runtime `CARGO_PKG_VERSION` from the `iam-server` and `scheduler-server` dynamically via UDS. If a service is offline, it gracefully reports `"offline"`.






## [1.0.2] - 2026-09-23

### Changed

- Replaced hardcoded "admin" username strings with dynamically loaded usernames in the HTTP server.
- Refactored `mitm_common::config::load_config` for better resilience.
- Distributed Tracing via `X-Request-ID` middleware over UDS implemented.
