# API v1 Documentation

The v1 API introduces a robust, resource-oriented RESTful architecture with Dynamic Authorization (AuthZ) using Casbin and Content Negotiation.

## 1. Authentication (AuthN) & Authorization (AuthZ)
All endpoints under `/api/v1/` (except public endpoints like `/api/v1/auth/session`, `/api/v1/system/info`, `/api/v1/system/time`) are guarded by two layers of middleware:
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
| `/api/v1/config/credentials/:id` | PUT | ADMIN |
| `/api/v1/config/targets` | GET, POST | ADMIN |
| `/api/v1/config/targets/:id` | PUT | ADMIN |
| `/api/v1/config/transformations/sources` | GET | ADMIN |
| `/api/v1/config/transformations/targets` | GET | ADMIN |
| `/api/v1/config/transformations/rules` | GET, POST | ADMIN |
| `/api/v1/config/transformations/rules/:id` | DELETE | ADMIN |
| `/api/v1/config/transformations` | GET, POST | ADMIN |
| `/api/v1/config/transformations/:id` | DELETE | ADMIN |
| `/api/v1/config/transformations/validations` | GET, POST | ADMIN |
| `/api/v1/config/transformations/validations/:id` | DELETE | ADMIN |
| `/api/v1/config/transformations/topic-dependencies`| GET | ADMIN |
| `/api/v1/config/transformations/auto-map` | POST | ADMIN |

### 2.6 IAM API
Identity & Access Management (Users & Roles).

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/v1/iam/users` | GET, POST | ADMIN |
| `/api/v1/iam/users/:id` | GET, DELETE | ADMIN |
| `/api/v1/iam/roles` | GET | ADMIN |
| `/api/v1/iam/assign-role` | POST | ADMIN |

### 2.7 Auth API
Endpoints for the currently authenticated user's session.

| API-Endpoint | Method | required role |
| --- | --- | --- |
| `/api/v1/auth/session` | POST | None |
| `/api/v1/auth/me` | GET | Valid Session |

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
