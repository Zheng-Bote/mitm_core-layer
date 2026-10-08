# Feature Spec: CRUD Operations for Transformations, Validations, and Rules

## 1. Problem Statement
The C++ Admin Frontend allows users with the `ADMIN` role to configure Mapping Rules, Transformations, and Validations. It attempts to persist and delete these configurations via HTTP `POST` and `DELETE` requests to the respective backend endpoints:
- `/api/v1/config/transformations`
- `/api/v1/config/transformations/rules`
- `/api/v1/config/transformations/validations`

However, the `mitm_http-server` backend currently only exposes `GET` routes for these endpoints. As a result, the backend rejects the requests with an HTTP 405 (Method Not Allowed) error, preventing admins from managing data transformations.

## 2. Scope
This feature is scoped entirely to the backend API (`core-layer/mitm_http-server`). It includes:
- Adding the missing `POST` (Create/Update) and `DELETE` routes for the three configuration types.
- Implementing the database handler functions in `transformation.rs` using `sqlx`.
- Updating the API Documentation (`docs/api/api_v1.md`).
- Ensuring `ADMIN` RBAC authorization applies to these modifying operations.

## 3. Goals
- **Enable Write Access:** The Admin frontend can successfully save and delete mapping rules, transformations, and validations without encountering 405 errors.
- **Maintain Consistency:** The payload accepted by the POST endpoints must match the flat JSON structure emitted by the frontend (`id`, `name`, `description`, `parameters`, `source_id`, `target_field_id`, `source_field`, `priority`, `transformation_chain`, `validation_chain`).
- **Upsert Semantic:** The `POST` endpoint must handle both creation (if `id` is omitted or empty) and updates (if `id` is provided).

## 4. Non-Goals
- We will not implement modifying actions for Data Sources or Data Targets (unless already missing, but they appear to be partially handled via credentials API).
- No frontend changes are required, as the frontend is already implemented correctly for this workflow.

## 5. Acceptance Criteria
- [ ] `POST /api/v1/config/transformations` creates or updates a Transformation.
- [ ] `DELETE /api/v1/config/transformations/:id` deletes a Transformation.
- [ ] `POST /api/v1/config/transformations/validations` creates or updates a Validation.
- [ ] `DELETE /api/v1/config/transformations/validations/:id` deletes a Validation.
- [ ] `POST /api/v1/config/transformations/rules` creates or updates a Rule.
- [ ] `DELETE /api/v1/config/transformations/rules/:id` deletes a Rule.
- [ ] The `docs/api/api_v1.md` reflects these new endpoints as `ADMIN` only.
- [ ] Sending a POST request to any of these endpoints successfully persists the data to the PostgreSQL database via `sqlx`.
