# API v1 Documentation

The v1 API introduces a robust, RESTful architecture with Dynamic Authorization (AuthZ) using Casbin and Content Negotiation.

## 1. Authentication (AuthN) & Authorization (AuthZ)
All protected endpoints (under `/api/admin/v1` and `/api/transformation/v1`) are guarded by two layers of middleware:
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
- `GET /api/admin/v1/credentials` (List all credentials)
- `POST /api/admin/v1/credentials` (Create credential)
- `PUT /api/admin/v1/credentials/:id` (Update credential)
- `GET /api/admin/v1/system/backup`
- `POST /api/admin/v1/system/restore`
- `POST /api/admin/v1/system/key-rotation`
- `DELETE /api/admin/v1/jobs/:name`
- `POST /api/admin/v1/jobs/:name/stop`
- `POST /api/admin/v1/jobs/:name/execute`
- `DELETE /api/admin/v1/rbac/users/:id`

### 2.2 Transformation API
- `GET /api/transformation/v1/sources`
- `GET /api/transformation/v1/targets`
- `GET /api/transformation/v1/rules`
- `GET /api/transformation/v1/transformations`

### 2.3 Public API
- `GET /api/public/v1/dlq`
- `POST /api/public/v1/dlq/requeue`

### 2.4 System API
- `GET /api/system/v1/info` (Returns MitM components and database version metadata)
- `GET /api/system/v1/time` (Returns local and UTC time)

### 2.5 User API
Endpoints for the currently authenticated user's session.
- `POST /api/user/v1/session` (Establishes a new session. **Constraints:** 24h Absolute TTL, 2h Idle Timeout enforced via PostgreSQL.)
- `GET /api/user/v1/roles` (Retrieves the RBAC roles associated with the current session.)

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
