# API v1 Documentation

The v1 API introduces a robust, RESTful architecture with Dynamic Authorization (AuthZ) using Casbin and Content Negotiation.

## 1. Authentication (AuthN) & Authorization (AuthZ)
All protected endpoints (under `/api/admin/v1`, `/api/transformation/v1`, and `/api/system/v1/dashboard`) are guarded by two layers of middleware:
- **AuthN Middleware:** Validates the session with the IAM server via UDS, injecting an `AuthResponse` containing the user's roles.
- **AuthZ Middleware:** Uses Casbin to enforce dynamic RBAC policies stored in PostgreSQL. It evaluates the user's roles against the HTTP Path and HTTP Method. If denied, it yields `403 Forbidden`.

## 2. API Namespacing & REST Semantics
The API is namespaced by domain:
- `/api/admin/v1`
- `/api/transformation/v1`
- `/api/public/v1`
- `/api/system/v1`
- `/api/user/v1`

Query parameters for identifying resources have been replaced with RESTful Path Variables (e.g., `/:id` or `/:name`).

### 2.1 Admin API

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/admin/v1/system/backup` | GET | ADMIN |
| `/api/admin/v1/system/restore` | POST | ADMIN |
| `/api/admin/v1/system/key-rotation` | POST | ADMIN |
| `/api/admin/v1/system/storage-keys` | GET | ADMIN |
| `/api/admin/v1/logs/system` | GET | ADMIN |
| `/api/admin/v1/logs/job-audit` | GET | ADMIN |
| `/api/admin/v1/logs/admin-audit` | GET | ADMIN |
| `/api/admin/v1/credentials` | GET, POST | ADMIN |
| `/api/admin/v1/credentials/:id` | PUT | ADMIN |
| `/api/admin/v1/delivery_targets` | GET, POST | ADMIN |
| `/api/admin/v1/delivery_targets/:id` | PUT | ADMIN |
| `/api/admin/v1/jobs` | GET, POST | ADMIN |
| `/api/admin/v1/jobs/:name` | DELETE | ADMIN |
| `/api/admin/v1/jobs/:name/stop` | POST | ADMIN |
| `/api/admin/v1/jobs/:name/execute` | POST | ADMIN |
| `/api/admin/v1/rbac/roles` | GET | ADMIN |
| `/api/admin/v1/rbac/users` | GET, POST | ADMIN |
| `/api/admin/v1/rbac/users/:id` | DELETE | ADMIN |
| `/api/admin/v1/rbac/assign` | POST | ADMIN |
| `/api/admin/v1/rbac/user_roles` | GET | ADMIN |
| `/api/admin/v1/rbac/os_user_roles` | GET | ADMIN |

### 2.2 Transformation API

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/transformation/v1/sources` | GET | ADMIN |
| `/api/transformation/v1/targets` | GET | ADMIN |
| `/api/transformation/v1/rules` | GET | ADMIN |
| `/api/transformation/v1/transformations` | GET | ADMIN |
| `/api/transformation/v1/validations` | GET | ADMIN |
| `/api/transformation/v1/topic-dependencies` | GET | ADMIN |
| `/api/transformation/v1/auto-map` | POST | ADMIN |
| `/api/transformation/v1/errors` | GET | ADMIN |

### 2.3 Public API

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/public/v1/dlq` | GET | None |
| `/api/public/v1/dlq/requeue` | POST | None |

### 2.4 System API

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/system/v1/info` | GET | None |
| `/api/system/v1/time` | GET | None |
| `/api/system/v1/dashboard` | GET | VIEWER |

### 2.5 User API
Endpoints for the currently authenticated user's session.

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/user/v1/session` | POST | None |
| `/api/user/v1/roles` | GET | None (Valid Session required) |

*Note: The root `/health` endpoint remains at the root level for load balancer / AWS health checks.*

## 3. Content Negotiation (FlatBuffers vs JSON)
Endpoints that handle large data sets (e.g., audit logs, transformation errors, DLQ) no longer use a `_bin` suffix. Instead, they examine the `Accept` HTTP header:
- If `Accept: application/x-flatbuffers`, the server responds with a FlatBuffers binary payload.
- Otherwise (default), the server responds with a standard JSON payload.

### Negotiated Endpoints:
- `GET /api/admin/v1/logs/system`
- `GET /api/admin/v1/logs/job-audit`
- `GET /api/admin/v1/logs/admin-audit`
- `GET /api/transformation/v1/errors`
- `GET /api/public/v1/dlq`
