# Architecture Plan: API Versioning & Dynamic Policy Engine (AuthZ)

## 1. Context & Scope
This plan details the implementation of Issue #14 in `mitm_http-server` (Core Layer). It introduces `/api/<domain>/v1` namespacing, separates AuthN from AuthZ, integrates Casbin for dynamic authorization, and shifts towards RESTful Content Negotiation. 

**Legacy Constraint:** Existing non-versioned endpoints (e.g., `/admin/*`, `/transformation/*`, `*_bin`) will remain entirely untouched and actively routed to preserve backward compatibility.

## 2. Architecture & Design (The "How")

### 2.1 Routing & Namespacing (Axum)
- **Legacy Routes:** The existing `configure_routes` in `mod.rs` will keep the legacy routes exactly as they are.
- **New V1 Routes:** We will create a new top-level nest `/api` containing:
  - `/api/admin/v1`
  - `/api/transformation/v1`
  - `/api/public/v1`
- **Implementation Strategy:** Create new handler modules (e.g., `admin_v1.rs`, `transformation_v1.rs`) or a sub-folder `api_v1/` to cleanly separate V1 logic from legacy logic. V1 handlers will wrap the same underlying database queries but expose them via the new RESTful interfaces.

### 2.2 AuthN (Authentication Middleware)
- **Current State:** `auth_middleware` intercepts requests starting with `/admin`.
- **Target State:** The middleware will be updated to also intercept `/api/admin/v1` and `/api/transformation/v1`.
- It will continue to authenticate against the IAM server via UDS and inject `AuthResponse` into the request extensions.
- Public routes (`/api/public/v1`) will bypass this middleware.

### 2.3 AuthZ (Casbin Dynamic Policy Engine)
- **Dependencies:** Add `casbin` and `sqlx-adapter` to `Cargo.toml`.
- **Model:** We will use a standard RESTful RBAC model (`model.conf`) stored in the `mitm_http-server` configuration directory or as a constant string in the binary:
  ```ini
  [request_definition]
  r = sub, obj, act
  
  [policy_definition]
  p = sub, obj, act
  
  [role_definition]
  g = _, _
  
  [policy_effect]
  e = some(where (p.eft == allow))
  
  [matchers]
  m = g(r.sub, p.sub) && keyMatch2(r.obj, p.obj) && regexMatch(r.act, p.act)
  ```
- **Storage:** The `sqlx-adapter` will connect using the existing PostgreSQL connection pool (`state.repo.get().unwrap().pool`). This fulfills the requirement of dynamic database-driven policies. Casbin will automatically manage its `casbin_rule` table.
- **Middleware:** A new `authz_middleware` will be placed immediately *after* the `auth_middleware` on protected `/api/*/v1` routes.
  - It extracts the user's assigned role(s) from `AuthResponse`.
  - It evaluates `enforcer.enforce((role, request.uri().path(), request.method().as_str()))`.
  - Returns `HTTP 403 Forbidden` if denied, otherwise `next.run()`.

### 2.4 RESTful Semantics & Path Variables
- Handlers in the V1 namespace will replace query-based resource identification with Path variables.
  - *Example:* Instead of `DELETE /api/admin/v1/jobs?name=foo`, use `DELETE /api/admin/v1/jobs/:name` mapped via `axum::extract::Path`.

### 2.5 Content Negotiation (Eliminating `_bin`)
- For data-heavy endpoints (like logs and transformations) in the V1 namespace, the `_bin` suffix is dropped.
- The handler will inject `axum::extract::TypedHeader<headers::Accept>`.
- **Logic:**
  - If `Accept: application/x-flatbuffers`, return the flatbuffer binary payload.
  - If `Accept: application/json` (or default), return the standard JSON payload.

## 3. SpecDD Validation Checklist
- [x] **No interference with Legacy:** Legacy paths remain unaffected.
- [x] **Layer Boundaries:** No bypassing of layers; we only change the HTTP exposure.
- [x] **PostgreSQL Pooling:** We reuse the existing `sqlx::PgPool` for the Casbin adapter.
- [x] **Security:** AuthN is strictly enforced before AuthZ. AES-GCM logic is untouched.
