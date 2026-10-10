# Feature Specification: User Profile Fields, Session IP Tracking, and Conditional Roles

## Feature Intent
To support advanced RBAC management and auditing in the frontend, the backend needs to track additional user metadata and active session details. Specifically, administrators must be able to configure a user's first name, last name, and active status (`is_active`). Inactive users must be rejected upon login with an appropriate error message. Furthermore, the authentication flow must capture the client's local IP address and store it in their session. Lastly, an endpoint is needed to allow `ADMIN` users to remotely terminate (kill) active user sessions.

## Requirements (EARS Syntax)

1. The system shall provide a PostgreSQL migration script in `../migrations/` adding `first_name` (VARCHAR) and `last_name` (VARCHAR) to the `admin_users` table. (Note: `is_active` in `admin_users` and `client_ip` in `user_sessions` already exist).
2. When processing `POST /api/v1/auth/session`, the system shall extract the client's IP address from the request payload (if provided by the client) and store it in `user_sessions.client_ip`.
3. When processing `POST /api/v1/auth/session`, if the authenticated user's `is_active` flag is false, the system shall reject the login attempt and return an appropriate error message (e.g., "Account is inactive").
4. When processing `GET /api/v1/auth/me`, the system shall return the user's `first_name`, `last_name`, `client_ip`, and `is_active` status.
5. The system shall provide an endpoint `DELETE /api/v1/iam/users/:id/session` allowing `ADMIN` users to delete a user's active session from the `user_sessions` table.
6. The `POST /api/v1/iam/users` and `PUT /api/v1/iam/users/:id` endpoints (or corresponding handlers) shall be updated to accept, validate, and store `first_name` and `last_name`, and correctly update `is_active`.

## Scope
**Core-Layer**:
- Database Migrations (`../migrations/003_admin_users_profile.sql`)
- `mitm_iam-server/src/db.rs` and `mitm_iam-server/src/ipc_server.rs` (Auth Logic, Conditional Roles)
- `mitm_http-server/src/handlers/api_v1/iam.rs`, `mitm_http-server/src/handlers/api_v1/auth.rs`, `mitm_http-server/src/handlers/rbac.rs` (CRUD, Session endpoints)
- `mitm_common/src/ipc.rs` (IPC Model updates)
- API documentation (`mitm_http-server/docs/api/api_v1.md`)

## SpecDD Architecture Alignment (Drift Control)
- [x] **Architecture:** The layered architecture is maintained.
- [x] **Security:** Envelope Encryption (AES-GCM) is NOT bypassed for PII data.
- [x] **Data Model:** Core PostgreSQL schemas remain intact (additive feature-specific tables/columns are allowed).
- [x] **Standards:** SPDX headers, English documentation, and independent `Cargo.toml` per layer will be maintained.

## Acceptance Criteria
- [ ] Database migrations successfully apply the new columns without destroying existing data.
- [ ] Authentication is rejected if `is_active` is false.
- [ ] Authentication successfully parses and tracks `client_ip`.
- [ ] `/api/v1/auth/me` includes the new profile fields.
- [ ] `DELETE /api/v1/iam/users/:id/session` functions correctly and requires `ADMIN`.
- [ ] `POST / PUT` endpoints for user management correctly save the new profile fields.
- [ ] `CHANGELOG.md` is updated.
