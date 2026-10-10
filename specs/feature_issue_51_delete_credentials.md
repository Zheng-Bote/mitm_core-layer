# Feature Specification: Delete Endpoints for Source and Target Credentials

## Feature Intent
The Admin Frontend requires the ability to delete selected Source Credentials and Delivery Targets from the UI. Currently, the `core-layer` provides endpoints to get, create (POST), and update (PUT) these resources, but lacks the endpoints to delete them. This feature will add the missing `DELETE /api/v1/config/credentials/:id` and `DELETE /api/v1/config/targets/:id` endpoints, ensuring that only users with the `ADMIN` role can perform these destructive actions.

## Requirements (EARS Syntax)

1. The system shall provide a `DELETE /api/v1/config/credentials/:id` endpoint that deletes a credential from the `source_credentials` table based on its ID.
2. The system shall provide a `DELETE /api/v1/config/targets/:id` endpoint that deletes a target from the `delivery_targets` table based on its ID.
3. The system shall verify the user's authorization before executing the deletion, restricting access exclusively to users with the `ADMIN` role.
4. When a deletion is successful, the system shall respond with a `204 No Content` or `200 OK` status, conforming to existing API patterns.
5. The system shall log the delete actions in the audit log table (`audit_log`) identifying the user and the resource that was deleted.

## Scope
**Core-Layer (mitm_http-server)**:
- `mitm_http-server/src/handlers/api_v1/config.rs`
- `mitm_http-server/src/handlers/admin.rs`
- API documentation (`mitm_http-server/docs/api/api_v1.md`)

## SpecDD Architecture Alignment (Drift Control)
- [x] **Architecture:** The layered architecture is maintained (no direct bypass from Collector to Delivery).
- [ ] **Architecture:** Feature affects architecture: SpecKit feature forces update of the SpecDD .sdd
- [x] **Security:** Envelope Encryption (AES-GCM) is NOT bypassed for PII data.
- [x] **Data Model:** Core PostgreSQL schemas remain intact.
- [x] **Standards:** SPDX headers, English documentation, and independent `Cargo.toml` / `go.mod` per layer will be maintained.

## Acceptance Criteria
- [ ] `DELETE /api/v1/config/credentials/:id` endpoint is implemented and accessible.
- [ ] `DELETE /api/v1/config/targets/:id` endpoint is implemented and accessible.
- [ ] Both endpoints successfully delete the corresponding database records.
- [ ] Both endpoints require `ADMIN` authorization; requests from users without the `ADMIN` role are rejected.
- [ ] Delete operations generate audit log entries.
- [ ] API documentation (`docs/api/api_v1.md`) is updated to include the new endpoints.
- [ ] `CHANGELOG.md` is updated.
