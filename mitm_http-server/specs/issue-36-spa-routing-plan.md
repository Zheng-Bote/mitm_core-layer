# Architecture Plan: SPA Routing & Dynamic Tera Templates (Issue #36)

## 1. Context & Scope
The HTTP server currently renders Tera templates directly on the root (`/`) route and uses `ServeDir` as a naive fallback. To properly support an Angular Single Page Application (SPA), we need a fallback mechanism that intercepts `404 Not Found` for static assets and returns `index.html` instead, delegating the routing to the frontend. Tera templates will be moved to a dedicated `/template/:name` route.

## 2. Structural Changes

### 2.1 Router Configuration (`src/handlers/mod.rs`)
The `configure_routes` function is the entry point for all HTTP routing.
- **Current Behavior:** 
  - `.route("/", get(handle_index))`
  - `.fallback_service(serve_dir)`
- **Planned Behavior:**
  - Remove `.route("/", get(handle_index))`
  - Add `.route("/template/:name", get(handle_template))`
  - Modify `fallback_service` to use a `ServeDir` decorated with a `.not_found_service(ServeFile::new(...))`

### 2.2 Template Handler (`src/handlers/mod.rs`)
- Convert the existing `async fn handle_index` into `pub async fn handle_template`.
- It will extract `Path(name): Path<String>`.
- Format the path to `"{}.html"`, allowing requests to `/template/dashboard` to resolve to `dashboard.html`.

## 3. Dependencies
- **`tower-http`:** Requires the `fs` feature to be active for `ServeDir` and `ServeFile`. (Currently active in `Cargo.toml`).

## 4. Risk Analysis
- **Risk 1 (API Route Shadowing):** If the SPA fallback is accidentally placed before the API routes, it might swallow legitimate API 404 errors and return HTML instead of a JSON error.
  - *Mitigation:* In Axum, `.fallback_service()` only triggers if no other explicitly defined `.route()` or `.nest()` matches. It operates at the lowest precedence, guaranteeing API routes evaluate first.
- **Risk 2 (Missing `index.html`):** If the Angular build has not yet generated `html/public/index.html`, the `.not_found_service` might fail to read the file, resulting in an internal error instead of a graceful 404.
  - *Mitigation:* `ServeFile::new` handles missing files natively. If the target fallback file doesn't exist either, it will properly return a 404 instead of a panic.

## 5. Verification & Testing
1. **API Validation:** Perform a `GET` request to a valid API endpoint (e.g., `/api/system/v1/info`) and assert a `200 OK` JSON response.
2. **Template Validation:** Perform a `GET` request to `/template/index` and assert a `200 OK` HTML response rendered by Tera.
3. **Static File Validation:** Place a test file in `html/public/test.txt`. Request `GET /test.txt` and assert a `200 OK`.
4. **SPA Fallback Validation:** Request a non-existent URL (e.g., `GET /angular-route/123`). Assert that it returns an HTTP `200 OK` containing the contents of `index.html` (the SPA entry point), verifying the fallback mechanism.
