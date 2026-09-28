# Agent Task Plan: API Versioning & Dynamic Policy Engine (AuthZ)

**Feature:** Issue #14
**Layer:** Core Layer
**Primary Module:** `core-layer/mitm_http-server`

---

### Task 1: Add Dependencies & Casbin Model Configuration
- **Repository/Module:** `core-layer/mitm_http-server`
- **Description:** Add `casbin` and `sqlx-adapter` to `Cargo.toml`. Create the RBAC `model.conf` file (or embed it as a string). Extend `AppState` to include a thread-safe instance of the Casbin `Enforcer`.
- **Goal:** Interfaces and contracts (Dependencies & State) are ready for the middlewares.

### Task 2: Initial AuthZ Policy Seeding (Bootstrap)
- **Repository/Module:** `core-layer/mitm_http-server`
- **Description:** Implement a bootstrap function that runs at startup to seed default Casbin policies (e.g., `ADMIN` role gets `*` access to `/api/admin/v1/*` and `/api/transformation/v1/*`) if they do not exist.
- **Goal:** Prevent lockout when the V1 API is deployed for the first time.

### Task 3: Implement AuthZ Middleware & Update AuthN Middleware
- **Repository/Module:** `core-layer/mitm_http-server`
- **Description:** 
  1. Modify `auth_middleware` (in `src/ipc_client.rs` or `mod.rs`) to also intercept requests starting with `/api/admin/v1` and `/api/transformation/v1`.
  2. Create a new `authz_middleware` that runs immediately after `auth_middleware`. It extracts the user's role, checks it against the `Enforcer` (`(role, path, method)`), and yields HTTP 403 if unauthorized.
- **Goal:** Identity verification and dynamic authorization are functioning in isolation.

### Task 4: Scaffold V1 API Router Structure
- **Repository/Module:** `core-layer/mitm_http-server`
- **Description:** Create a new module folder `src/handlers/api_v1/` with sub-modules `mod.rs`, `admin.rs`, `transformation.rs`, and `public.rs`. Wire `api_v1::routes()` into the main router in `src/handlers/mod.rs` under the `/api` prefix, attaching the two middlewares to the protected sub-routers.
- **Goal:** The structural foundation for the new namespaced endpoints is in place without breaking legacy endpoints.

### Task 5: Implement V1 Admin API Endpoints
- **Repository/Module:** `core-layer/mitm_http-server`
- **Description:** Implement the Admin domain endpoints in `src/handlers/api_v1/admin.rs`. Wrap existing DB logic but replace query parameters with RESTful Path variables (e.g., `/:id`). Apply Content Negotiation (`Accept` header) for audit logs to return JSON or FlatBuffers conditionally.
- **Goal:** `/api/admin/v1/*` is fully functional and RESTful.

### Task 6: Implement V1 Transformation API Endpoints
- **Repository/Module:** `core-layer/mitm_http-server`
- **Description:** Implement the Transformation domain endpoints in `src/handlers/api_v1/transformation.rs`. Convert parameters to Path variables. Apply Content Negotiation to eliminate `errors_bin` logic, handling it internally via the `Accept` header.
- **Goal:** `/api/transformation/v1/*` is fully functional and RESTful.

### Task 7: Implement V1 Public API Endpoints
- **Repository/Module:** `core-layer/mitm_http-server`
- **Description:** Implement public endpoints (e.g., DLQ) in `src/handlers/api_v1/public.rs`. Bypass AuthN/AuthZ middlewares entirely. Apply Content Negotiation for the payload.
- **Goal:** `/api/public/v1/*` is fully functional and RESTful.

### Task 8: Restructure API Documentation
- **Repository/Module:** `core-layer/mitm_http-server` (and `docs/` repo folder)
- **Description:** Rename `docs/API.md` (or equivalent) to `docs/api/api_v0.md`. Create a comprehensive `docs/api/api_v1.md` detailing the `/api/<domain>/v1` namespacing, Casbin AuthZ architecture, RESTful parameters, and Content Negotiation implementation.
- **Goal:** Documentation accurately reflects both the legacy and the new API structures.
