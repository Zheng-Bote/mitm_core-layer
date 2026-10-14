# API v1 Documentation

The v1 API introduces a robust, resource-oriented RESTful architecture with Dynamic Authorization (AuthZ) using Casbin and Content Negotiation.

> Applies to `mitm_http-server` **2.0.0** and later. Breaking change in 2.0.0: `POST /api/v1/auth/session` was removed; sessions are issued only by the WebAuthn ceremonies (see [2.7 Auth API](#27-auth-api)).

## 1. Authentication (AuthN) & Authorization (AuthZ)
All endpoints under `/api/v1/` (except public endpoints like `/api/v1/auth/webauthn/*`, `/api/v1/auth/bootstrap/status`, `/api/v1/system/info`, `/api/v1/system/time`) are guarded by two layers of middleware:
- **AuthN Middleware:** Validates the session with the IAM server via UDS, injecting an `AuthResponse` containing the user's roles.
- **AuthZ Middleware:** Uses Casbin to enforce dynamic RBAC policies stored in PostgreSQL. It evaluates the user's roles against the HTTP Path and HTTP Method. If denied, it yields `403 Forbidden`.

### Role Hierarchy
Casbin implements the following role hierarchy (Grouping):
- `ADMIN` inherits `USER`
- `USER` inherits `VIEWER`

## 2. API Namespacing & REST Semantics
The API is namespaced strictly by **resource domains**:
- `/api/v1/system`
- `/api/v1/jobs`
- `/api/v1/logs`
- `/api/v1/dlq`
- `/api/v1/config`
- `/api/v1/iam`
- `/api/v1/auth`

Path variables are used for identifying resources (e.g., `/:id` or `/:name`).

### 2.1 System API
System operations, health, and backups.

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/v1/system/info` | GET | None |
| `/api/v1/system/time` | GET | None |
| `/api/v1/system/dashboard` | GET | VIEWER |
| `/api/v1/system/backup` | GET | ADMIN |
| `/api/v1/system/restore` | POST | ADMIN |
| `/api/v1/system/key-rotation` | POST | ADMIN |
| `/api/v1/system/storage-keys` | GET | ADMIN |

### 2.2 Jobs API
Job listing and execution control.

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/v1/jobs` | GET, POST | VIEWER (GET), ADMIN (POST) |
| `/api/v1/jobs/:name` | DELETE | ADMIN |
| `/api/v1/jobs/:name/stop` | POST | USER |
| `/api/v1/jobs/:name/execute` | POST | USER |

### 2.3 Logs API
Access to audit trails and error logs (Supports Content Negotiation).

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/v1/logs/system` | GET | USER |
| `/api/v1/logs/audit` | GET | USER |
| `/api/v1/logs/transformation-errors` | GET | USER |

### 2.4 DLQ API
Dead Letter Queue management.

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/v1/dlq` | GET | USER |
| `/api/v1/dlq/requeue` | POST | USER |

### 2.5 Config API
Data source, transformation, and target configurations.

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/v1/config/credentials` | GET, POST | ADMIN |
| `/api/v1/config/credentials/:id` | PUT, DELETE | ADMIN |
| `/api/v1/config/targets` | GET, POST | ADMIN |
| `/api/v1/config/targets/:id` | PUT, DELETE | ADMIN |
| `/api/v1/config/transformations/sources` | GET, POST | ADMIN |
| `/api/v1/config/transformations/sources/:id` | DELETE | ADMIN |
| `/api/v1/config/transformations/targets` | GET, POST | ADMIN |
| `/api/v1/config/transformations/targets/:id` | DELETE | ADMIN |
| `/api/v1/config/transformations/rules` | GET, POST | ADMIN |
| `/api/v1/config/transformations/rules/:id` | DELETE | ADMIN |
| `/api/v1/config/transformations` | GET, POST | ADMIN |
| `/api/v1/config/transformations/:id` | DELETE | ADMIN |
| `/api/v1/config/transformations/validations` | GET, POST | ADMIN |
| `/api/v1/config/transformations/validations/:id` | DELETE | ADMIN |
| `/api/v1/config/transformations/topic-dependencies`| GET, POST, DELETE | ADMIN |
| `/api/v1/config/transformations/auto-map` | POST | ADMIN |

### 2.6 IAM API
Identity & Access Management (Users & Roles).

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/v1/iam/users` | GET, POST | ADMIN |
| `/api/v1/iam/users/:id` | GET, PUT, DELETE | ADMIN |
| `/api/v1/iam/users/:id/session` | DELETE | ADMIN |
| `/api/v1/iam/roles` | GET | ADMIN |
| `/api/v1/iam/assign-role` | POST | ADMIN |

### 2.7 Auth API
Endpoints for passkey (WebAuthn) authentication and the currently authenticated user's session.
Session tokens are issued only after a successful WebAuthn ceremony (`login/finish`, or `register/finish` for the bootstrap administrator).

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/v1/auth/bootstrap/status` | GET | None |
| `/api/v1/auth/webauthn/register/start` | POST | None (`bootstrap_token` required while no user exists) |
| `/api/v1/auth/webauthn/register/finish` | POST | None |
| `/api/v1/auth/webauthn/login/start` | POST | None |
| `/api/v1/auth/webauthn/login/finish` | POST | None |
| `/api/v1/auth/me` | GET | Valid Session |

- The public `/api/v1/auth/webauthn/*` and `/api/v1/auth/bootstrap/*` endpoints are rate-limited per client IP (20 requests per minute); excess requests receive `429 Too Many Requests` with a `Retry-After` header.
- The session's `client_ip` is determined server-side from the TCP peer address, or from the rightmost `X-Forwarded-For` entry when `MITM_TRUST_PROXY_HEADERS=true` (required behind a load balancer such as AWS ALB). A `client_ip` in a request payload is ignored.

#### Login Flow
1. `POST /api/v1/auth/webauthn/login/start` with `{"username": "..."}` returns `ceremony_id`, `public_key_options` and `rp_origin`.
2. The client signs the challenge with its passkey (FIDO2 security key or platform authenticator).
3. `POST /api/v1/auth/webauthn/login/finish` with `{"ceremony_id": "...", "credential": {...}}` returns `{"session_token": "..."}`.
4. Subsequent requests send `Authorization: Bearer <session_token>`. Sessions expire after 24 h (absolute) or 2 h without activity; `GET /api/v1/auth/me` keeps them alive.

#### Bootstrap (no user exists)
- `GET /api/v1/auth/bootstrap/status` returns `{"bootstrap_required": true}`.
- `register/start` requires `bootstrap_token` matching `INITIAL_ADMIN_TOKEN` (at least 32 characters; the server does not start without it while no user exists).
- `register/finish` creates the active `ADMIN` user and returns a `session_token`. Only one bootstrap administrator can be created.

#### Error Responses
| Status | Endpoint | Cause |
| --- | --- | --- |
| `400` | `register/start` | `bootstrap_token` missing while no user exists |
| `400` | `*/finish` | Ceremony expired or not found, registration verification failed |
| `400` | `login/start` | No passkey registered for the user |
| `401` | `login/finish` | Passkey verification failed |
| `403` | `register/start` | Invalid `bootstrap_token` |
| `403` | `login/start`, `login/finish` | Account inactive (pending administrator approval) |
| `404` | `login/start` | User not found |
| `409` | `register/finish` | System already initialized (concurrent or repeated bootstrap) |
| `429` | all public auth endpoints | Rate limit exceeded |

*Note: The root `/health` endpoint remains at the root level for load balancer / AWS health checks.*

## 3. Content Negotiation (FlatBuffers vs JSON)
Endpoints that handle large data sets (e.g., audit logs, transformation errors, DLQ) examine the `Accept` HTTP header:
- If `Accept: application/x-flatbuffers`, the server responds with a FlatBuffers binary payload.
- Otherwise (default), the server responds with a standard JSON payload.

### Negotiated Endpoints:
- `GET /api/v1/logs/system`
- `GET /api/v1/logs/audit`
- `GET /api/v1/logs/transformation-errors`
- `GET /api/v1/dlq`
