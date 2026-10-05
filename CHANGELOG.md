# Changelog

All notable changes to the MitM-2 Core Layer project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- Moved dashboard stats endpoint from `/api/admin/v1/system/dashboard/stats` to `/api/system/v1/dashboard` and expanded it with advanced counts and oldest timestamps for all audit logs and DLQ.
- Adjusted RBAC policy to allow `VIEWER` access to the dashboard endpoint.

## [1.3.0] - 2026-09-28

### Added
- **Dynamic Component Versioning (Issue #38):** Extended the `mitm_common` IPC protocol with a new `GetInfo` request.
- **HTTP Info Refactoring:** The HTTP server's `/info` endpoint no longer hardcodes the component versions. It now fetches the runtime `CARGO_PKG_VERSION` from the `iam-server` and `scheduler-server` dynamically via UDS. If a service is offline, it gracefully reports `"offline"`.






## [1.0.2] - 2026-09-23

### Changed

- Replaced hardcoded "admin" username strings with dynamically loaded usernames in the HTTP server.
- Refactored `mitm_common::config::load_config` for better resilience.
- Distributed Tracing via `X-Request-ID` middleware over UDS implemented.
