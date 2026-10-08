# Tasks: CRUD Operations for Transformations, Validations, and Rules

## 1. Update API Documentation (Contract First)
- **Repository/Module:** `core-layer/mitm_http-server`
- **File:** `docs/api/api_v1.md`
- **Action:** Add the `POST` and `DELETE` HTTP methods for `/api/v1/config/transformations`, `/api/v1/config/transformations/rules`, and `/api/v1/config/transformations/validations` to the Config API table. Mark the required role as `ADMIN`.

## 2. Define Payload Structs for Deserialization
- **Repository/Module:** `core-layer/mitm_http-server`
- **File:** `src/handlers/transformation.rs`
- **Action:** Create `Deserialize` structs mapping to the frontend's JSON representation:
  - `TransformationPayload` (id, name, description, parameters)
  - `ValidationPayload` (id, name, description, parameters)
  - `RulePayload` (id, source_id, target_field_id, source_field, priority, transformation_chain, validation_chain)

## 3. Implement Transformations CRUD Handlers
- **Repository/Module:** `core-layer/mitm_http-server`
- **File:** `src/handlers/transformation.rs`
- **Action:** 
  - Write `handle_post_transformations` (upsert into `mapping_transformation`).
  - Write `handle_delete_transformations` (delete from `mapping_transformation` by ID).
  - Ensure an audit log entry (`admin_audit_logs`) is generated for both actions.

## 4. Implement Validations CRUD Handlers
- **Repository/Module:** `core-layer/mitm_http-server`
- **File:** `src/handlers/transformation.rs`
- **Action:** 
  - Write `handle_post_validations` (upsert into `mapping_validation`).
  - Write `handle_delete_validations` (delete from `mapping_validation` by ID).
  - Ensure an audit log entry (`admin_audit_logs`) is generated for both actions.

## 5. Implement Rules CRUD Handlers
- **Repository/Module:** `core-layer/mitm_http-server`
- **File:** `src/handlers/transformation.rs`
- **Action:** 
  - Write `handle_post_rules` (upsert into `mapping_rule`).
  - Write `handle_delete_rules` (delete from `mapping_rule` by ID).
  - Ensure an audit log entry (`admin_audit_logs`) is generated for both actions.

## 6. Register Routes in Config Router
- **Repository/Module:** `core-layer/mitm_http-server`
- **File:** `src/handlers/api_v1/config.rs`
- **Action:** 
  - Register the new `POST` and `DELETE` paths in the `routes()` function (e.g. `.route("/transformations", post(...))`, `.route("/transformations/:id", delete(...))`).
  - Add proxy functions if necessary (similar to `handle_put_credentials`) or directly reference the new handlers in `transformation.rs`.

## 7. Verify RBAC / IAM
- **Repository/Module:** `core-layer/mitm_http-server`
- **Action:** Confirm that Casbin policies in the PostgreSQL database correctly protect the new `POST` and `DELETE` methods for the `/api/v1/config/transformations*` endpoints, as Casbin handles this dynamically based on the URL. (No code change needed in the middleware, just logical validation).
