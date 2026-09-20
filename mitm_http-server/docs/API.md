# API Documentation

This document outlines the REST JSON:API endpoints provided by the `mitm_http-server`.

**API Version:** `v0` (Current Legacy Routing)  
**Content-Type:** `application/vnd.api+json` (JSON:API Standard)  
**Authentication:** JWT / Bearer Token expected via `Authorization` Header (intercepted by AuthMiddleware)

## Endpoints Overview

The endpoints are currently grouped logically by theme. All routes are currently nested under the `/admin/` namespace.

### Theme: Job Management (Orchestration)
Endpoints for scheduling, triggering, and managing background worker jobs.

| Method | Endpoint | Description | Auth Required |
| :--- | :--- | :--- | :--- |
| `GET` | `/admin/jobs/` | List all available and active jobs | Yes (User/Admin) |
| `POST` | `/admin/jobs/update-jobs` | Reload job configuration and schedules | Yes (Admin) |
| `DELETE`| `/admin/jobs/delete-job` | Delete a specific job definition | Yes (Admin) |
| `POST` | `/admin/jobs/stop-job` | Halt a running job (sends `SIGTERM`/`SIGKILL`) | Yes (Admin) |
| `POST` | `/admin/jobs/execute-job` | Immediately trigger a job via UDS Scheduler | Yes (Admin) |

### Theme: Identity & Access Management (RBAC)
Endpoints for managing users, roles, and permissions. Encryption keys are managed via IAM.

| Method | Endpoint | Description | Auth Required |
| :--- | :--- | :--- | :--- |
| `GET` | `/admin/rbac/roles` | Fetch all available roles from the database | Yes (Admin) |
| `GET` | `/admin/rbac/users` | Fetch all registered users | Yes (Admin) |
| `POST` | `/admin/rbac/user/create` | Register a new user | Yes (Admin) |
| `DELETE`| `/admin/rbac/user/delete` | Delete a user by ID/Name | Yes (Admin) |
| `POST` | `/admin/rbac/assign` | Assign roles to a specific user | Yes (Admin) |
| `GET` | `/admin/rbac/user_roles` | Get roles for the current or specified user | Yes (Admin) |
| `GET` | `/admin/rbac/os_user_roles` | Get system OS role mappings | Yes (Admin) |

### Theme: System Administration & Keys
Endpoints for system maintenance, backup, and cryptographic key rotation.

| Method | Endpoint | Description | Auth Required |
| :--- | :--- | :--- | :--- |
| `POST` | `/admin/action` | Trigger a generic system action | Yes (Admin) |
| `GET` | `/admin/backup` | Create a system backup snapshot | Yes (Admin) |
| `POST` | `/admin/restore` | Restore system from a backup snapshot | Yes (Admin) |
| `POST` | `/admin/key-rotation` | Trigger AES-GCM DEK key rotation via IAM | Yes (Admin) |
| `GET` | `/admin/storage-keys` | Retrieve public storage keys | Yes (Admin) |

### Theme: Audit & System Logs
Endpoints for fetching execution history and compliance logs.

| Method | Endpoint | Description | Auth Required |
| :--- | :--- | :--- | :--- |
| `GET` | `/admin/logs/system` | Fetch general system output logs | Yes (Admin) |
| `GET` | `/admin/logs/job-audit` | Fetch job execution audit logs | Yes (User/Admin) |
| `GET` | `/admin/logs/admin-audit`| Fetch administrative action audit logs | Yes (Admin) |

### Theme: Data Transformation & Mapping
Endpoints for managing rules that map Source systems to Target SaaS solutions.

| Method | Endpoint | Description | Auth Required |
| :--- | :--- | :--- | :--- |
| `GET` | `/admin/transformation/sources` | List all configured data sources | Yes (User/Admin) |
| `GET` | `/admin/transformation/targets` | List all configured data targets | Yes (User/Admin) |
| `GET` | `/admin/transformation/rules` | Fetch transformation mapping rules | Yes (User/Admin) |
| `GET` | `/admin/transformation/transformations`| Fetch active transformation processes | Yes (User/Admin) |
| `GET` | `/admin/transformation/validations` | List data validation rules | Yes (User/Admin) |
| `GET` | `/admin/transformation/errors` | List transformation history errors | Yes (User/Admin) |
| `GET` | `/admin/transformation/topic-dependencies` | Get Kafka/Queue topic dependencies | Yes (User/Admin) |

### Theme: Dead Letter Queue (DLQ)
Endpoints for monitoring and handling failed message deliveries.

| Method | Endpoint | Description | Auth Required |
| :--- | :--- | :--- | :--- |
| `GET` | `/admin/dlq/` | Fetch all messages currently in the DLQ | Yes (Admin) |
| `POST` | `/admin/dlq/requeue` | Requeue a DLQ message for processing | Yes (Admin) |
