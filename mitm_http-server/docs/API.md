# API Documentation

This document outlines the REST JSON:API endpoints provided by the `mitm_http-server`.

**API Version:** `v0` (Current Legacy Routing)  
**Content-Type:** `application/vnd.api+json` (JSON:API Standard)  
**Authentication:** JWT / Bearer Token expected via `Authorization` Header (intercepted by AuthMiddleware)

## Standardized Responses

In compliance with the JSON:API standard, responses follow a strict schema:

**Success (2xx):**
Returns `200 OK` (data retrieval) or `202 Accepted` / `204 No Content` (mutations/actions) with the payload encapsulated in a `data` array/object.
```json
{
  "data": [ ... ]
}
```

**Errors (4xx, 5xx):**
If a request fails, the server returns an HTTP error status code along with an `errors` array:
```json
{
  "errors": [
    {
      "status": "401",
      "title": "Unauthorized",
      "detail": "Missing or invalid authorization credentials."
    }
  ]
}
```

## Endpoints Overview

The endpoints are grouped logically by theme. All routes are currently nested under the `/admin/` namespace.

### Theme: Job Management (Orchestration)
Endpoints for scheduling, triggering, and managing background worker jobs.

| Method | Endpoint | Description | Auth Required | Expected Returns |
| :--- | :--- | :--- | :--- | :--- |
| `GET` | `/admin/jobs/` | List all available and active jobs | Yes (User/Admin) | `200 OK` (Array of Jobs) |
| `POST` | `/admin/jobs/update-jobs` | Reload job configuration and schedules | Yes (Admin) | `202 Accepted` |
| `DELETE`| `/admin/jobs/delete-job` | Delete a specific job definition | Yes (Admin) | `204 No Content` |
| `POST` | `/admin/jobs/stop-job` | Halt a running job (sends `SIGTERM`/`SIGKILL`) | Yes (Admin) | `202 Accepted` |
| `POST` | `/admin/jobs/execute-job` | Immediately trigger a job via UDS Scheduler | Yes (Admin) | `202 Accepted` |

### Theme: Identity & Access Management (RBAC)
Endpoints for managing users, roles, and permissions. Encryption keys are managed via IAM.

| Method | Endpoint | Description | Auth Required | Expected Returns |
| :--- | :--- | :--- | :--- | :--- |
| `GET` | `/admin/rbac/roles` | Fetch all available roles from the database | Yes (Admin) | `200 OK` (Array of Roles) |
| `GET` | `/admin/rbac/users` | Fetch all registered users | Yes (Admin) | `200 OK` (Array of Users) |
| `POST` | `/admin/rbac/user/create` | Register a new user | Yes (Admin) | `201 Created` |
| `DELETE`| `/admin/rbac/user/delete` | Delete a user by ID/Name | Yes (Admin) | `204 No Content` |
| `POST` | `/admin/rbac/assign` | Assign roles to a specific user | Yes (Admin) | `200 OK` |
| `GET` | `/admin/rbac/user_roles` | Get roles for the current or specified user | Yes (Admin) | `200 OK` (Array of Roles) |
| `GET` | `/admin/rbac/os_user_roles` | Get system OS role mappings | Yes (Admin) | `200 OK` (Array of OS Roles) |

### Theme: System Administration & Keys
Endpoints for system maintenance, backup, and cryptographic key rotation.

| Method | Endpoint | Description | Auth Required | Expected Returns |
| :--- | :--- | :--- | :--- | :--- |
| `POST` | `/admin/action` | Trigger a generic system action | Yes (Admin) | `202 Accepted` |
| `GET` | `/admin/backup` | Create a system backup snapshot | Yes (Admin) | `200 OK` (Binary/Zip Stream) |
| `POST` | `/admin/restore` | Restore system from a backup snapshot | Yes (Admin) | `202 Accepted` |
| `POST` | `/admin/key-rotation` | Trigger AES-GCM DEK key rotation via IAM | Yes (Admin) | `200 OK` (Status Details) |
| `GET` | `/admin/storage-keys` | Retrieve public storage keys | Yes (Admin) | `200 OK` (Keys Payload) |

### Theme: Audit & System Logs
Endpoints for fetching execution history and compliance logs.

| Method | Endpoint | Description | Auth Required | Expected Returns |
| :--- | :--- | :--- | :--- | :--- |
| `GET` | `/admin/logs/system` | Fetch general system output logs | Yes (Admin) | `200 OK` (Log Array) |
| `GET` | `/admin/logs/job-audit` | Fetch job execution audit logs | Yes (User/Admin) | `200 OK` (Audit Records) |
| `GET` | `/admin/logs/admin-audit`| Fetch administrative action audit logs | Yes (Admin) | `200 OK` (Audit Records) |

### Theme: Data Transformation & Mapping
Endpoints for managing rules that map Source systems to Target SaaS solutions.

| Method | Endpoint | Description | Auth Required | Expected Returns |
| :--- | :--- | :--- | :--- | :--- |
| `GET` | `/admin/transformation/sources` | List all configured data sources | Yes (User/Admin) | `200 OK` (Sources Array) |
| `GET` | `/admin/transformation/targets` | List all configured data targets | Yes (User/Admin) | `200 OK` (Targets Array) |
| `GET` | `/admin/transformation/rules` | Fetch transformation mapping rules | Yes (User/Admin) | `200 OK` (Rules Array) |
| `GET` | `/admin/transformation/transformations`| Fetch active transformation processes | Yes (User/Admin) | `200 OK` (Transforms) |
| `GET` | `/admin/transformation/validations` | List data validation rules | Yes (User/Admin) | `200 OK` (Validations) |
| `GET` | `/admin/transformation/errors` | List transformation history errors | Yes (User/Admin) | `200 OK` (Error History) |
| `GET` | `/admin/transformation/topic-dependencies` | Get Kafka/Queue topic dependencies | Yes (User/Admin) | `200 OK` (Dependencies) |

### Theme: Dead Letter Queue (DLQ)
Endpoints for monitoring and handling failed message deliveries.

| Method | Endpoint | Description | Auth Required | Expected Returns |
| :--- | :--- | :--- | :--- | :--- |
| `GET` | `/admin/dlq/` | Fetch all messages currently in the DLQ | Yes (Admin) | `200 OK` (DLQ Messages) |
| `POST` | `/admin/dlq/requeue` | Requeue a DLQ message for processing | Yes (Admin) | `202 Accepted` |
