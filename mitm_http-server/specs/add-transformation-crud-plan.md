# Technical Spec & Plan: CRUD Operations for Transformations, Validations, and Rules

## 1. Architecture Check & Inherited Constraints
According to the SpecDD inheritance chain (`mitm-2.sdd` -> `core-layer.sdd` -> `mitm_http-server.sdd`):
- **RBAC:** These endpoints modify configuration, so they must be bound to the `ADMIN` role. This is handled dynamically via Casbin middleware if we map it in the DB, but statically we need to ensure the routing fits the pattern.
- **Auditing:** As per `mitm-2.sdd`, any manual configuration changes must trigger an `admin_audit_logs` event. (We will verify how existing config POST endpoints implement this and replicate it).
- **Encryption:** Unlike `credentials` or `targets`, `mapping_rule`, `mapping_transformation`, and `mapping_validation` do not store sensitive secrets (no `wrapped_key`/`nonce`), so Envelope Encryption is not required here.
- **Language/License:** All comments in English, existing files retain their SPDX headers.

## 2. Technical Design
We need to extend `core-layer/mitm_http-server/src/handlers/transformation.rs` with the following HTTP handlers:

### 2.1 Transformations
- `handle_post_transformations(State, Json)`: Upserts into `mapping_transformation`.
  - Checks if `id` is present. If yes, runs `UPDATE`. If no, generates a UUIDv4 and runs `INSERT`.
- `handle_delete_transformations(State, Path)`: Deletes from `mapping_transformation` by `id`.

### 2.2 Validations
- `handle_post_validations(State, Json)`: Upserts into `mapping_validation`.
  - Similar upsert logic based on `id`.
- `handle_delete_validations(State, Path)`: Deletes from `mapping_validation` by `id`.

### 2.3 Rules
- `handle_post_rules(State, Json)`: Upserts into `mapping_rule`.
  - Maps `id`, `source_id`, `target_field_id`, `source_field`, `priority`, `transformation_chain`, `validation_chain`.
- `handle_delete_rules(State, Path)`: Deletes from `mapping_rule` by `id`.

### 2.4 Routing (`core-layer/mitm_http-server/src/handlers/api_v1/config.rs`)
- Register the `POST` and `DELETE` routes for:
  - `/transformations`
  - `/transformations/:id` (for DELETE)
  - `/transformations/validations`
  - `/transformations/validations/:id`
  - `/transformations/rules`
  - `/transformations/rules/:id`

### 2.5 API Documentation
- Update `core-layer/mitm_http-server/docs/api/api_v1.md` to list `POST` and `DELETE` for `/api/v1/config/transformations`, `/api/v1/config/transformations/rules`, and `/api/v1/config/transformations/validations` with `ADMIN` requirement.

## 3. Data Structures
We need to define `Deserialize` structs for the POST payloads, mirroring the C++ frontend JSON:
- `TransformationPayload`: `id` (Option), `name`, `description` (Option), `parameters` (Option).
- `ValidationPayload`: `id` (Option), `name`, `description` (Option), `parameters` (Option).
- `RulePayload`: `id` (Option), `source_id`, `target_field_id`, `source_field`, `priority`, `transformation_chain` (Option), `validation_chain` (Option).
