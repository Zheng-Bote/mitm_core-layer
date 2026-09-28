# Feature Spec: API Versioning & Dynamic Policy Engine (AuthZ) (Issue #14)

## 1. Feature Intent
Currently, authentication is manually invoked in almost every protected handler, which is highly error-prone. Concurrently, the API lacks versioning, relies on RPC-style query parameters, and mixes public/private namespaces.

This feature refactors the API to use a **Domain-driven API Versioning** scheme (e.g., `/api/admin/v1`, `/api/transformation/v1`, `/api/public/v1`). 

Crucially, it separates **Authentication (AuthN)** from **Authorization (AuthZ)** to ensure scalability and regulatory compliance:
- A centralized `AuthMiddleware` strictly guarantees the identity of the user across all non-public domains (AuthN).
- A dynamic **Policy Engine (e.g., Casbin)** acts as the global Authorization (AuthZ) middleware. By decoupling rules from code and moving them to the database, we prevent role-based coupling.

It also modernizes the endpoints towards RESTful resource paths and Content Negotiation, preparing the API for future stability.

**Important Constraint (Backwards Compatibility):** The existing API paths (without version numbers) must remain intact and functional for now to ensure backwards compatibility with the current frontend. The new `/api/<domain>/v1` endpoints will be introduced alongside them.

## 2. Requirements (EARS Syntax)
1. **Ubiquitous:** The `HTTP Server` shall group all *new* endpoints under a domain-driven versioning pattern: `/api/<domain>/<version>/`.
2. **Event-driven:** When a request targets any route under `/api/admin/v1` or `/api/transformation/v1`, the `HTTP Server` shall enforce identity verification (AuthN) via a centralized `AuthMiddleware`, attaching the user to the request context.
3. **Event-driven:** When an authenticated request reaches a protected router, the `HTTP Server` shall evaluate the request `(Subject, Object, Action)` against a dynamic **Policy Engine** (AuthZ), rejecting unauthorized access with HTTP 403 Forbidden.
4. **Ubiquitous:** The `HTTP Server` shall process strictly public (unauthenticated) endpoints exclusively under the `/api/public/v1` namespace.
5. **State-driven:** While operating on specific resources under the new `/v1/` APIs, the `HTTP Server` shall use RESTful path variables (e.g., `DELETE /api/admin/v1/jobs/{name}`) instead of query parameters.
6. **Event-driven:** When a client requests log or export data via the new APIs, the `HTTP Server` shall evaluate the HTTP `Accept` header to dynamically return either `application/json` or `application/x-flatbuffers`, eliminating the need for `_bin` endpoints in the `v1` namespace.
7. **Ubiquitous:** The `HTTP Server` shall maintain all existing legacy endpoints exactly as they are (including their current Auth rules and `_bin` suffix behaviors) to prevent breaking the current frontend.

## 3. Scope
`core-layer/mitm_http-server` (Rust Axum HTTP Server component)

## 4. SpecDD Architecture Alignment (Drift Control)
- [x] **Architecture:** The layered architecture is maintained.
- [x] **Architecture:** Feature affects architecture: SpecKit feature forces update of the SpecDD `.sdd`
- [x] **Security:** Envelope Encryption (AES-GCM) is NOT bypassed for PII data.
- [x] **Data Model:** Core PostgreSQL schemas remain intact (feature-specific tables are allowed for Casbin policies).
- [x] **Standards:** SPDX headers, English documentation.

## 5. Acceptance Criteria
- [ ] Existing API endpoints (without `/api/.../v1`) are fully preserved and still functional.
- [ ] `AuthMiddleware` is implemented and attached to the new sub-routers (`/api/admin/v1`, `/api/transformation/v1`) to handle AuthN.
- [ ] A dynamic Policy Engine (e.g., Casbin) is integrated as a global AuthZ middleware for the new `v1` routes.
- [ ] Authorization policies (Rules) for the `v1` API are loaded dynamically from the PostgreSQL database.
- [ ] New `v1` API Endpoints use Path Variables natively.
- [ ] New `v1` API Endpoints utilize Content Negotiation (`Accept` header) instead of `_bin` suffixes.
- [ ] API Documentation is restructured: The existing `docs/API.md` is moved to `docs/api/api_v0.md` (representing the legacy API), and a new `docs/api/api_v1.md` is created to document the new paths, REST semantics, and dynamic policy-based access model for the V1 API.
