# Feature Specification: SPA Routing & Dynamic Tera Templates (Issue #36)

## 1. Overview
The HTTP Server currently handles the root (`/`) route by rendering a Tera template (`index.html`) and uses a simple `ServeDir` as a fallback for static assets. To support a modern Single Page Application (SPA) frontend like Angular, the backend routing must be restructured. SPAs require that unknown routes (like `/dashboard`) fall back to serving the static `index.html`, allowing the client-side router to handle the view. Concurrently, dynamic Tera template rendering must be relocated to a dedicated namespace.

## 2. Technical Objectives
1. **Relocate Tera Rendering:** Move the current `handle_index` logic (which renders `index.html` via Tera) to a dynamic endpoint `GET /template/:name`.
2. **Implement SPA Static Serving:** Map the fallback service to use `tower_http::services::ServeDir` pointed at the `html/public` directory.
3. **Configure SPA Fallback:** Enhance the `ServeDir` with `.not_found_service(ServeFile::new(...))` to ensure that any 404 response for a static asset is intercepted and returns the static `index.html` instead.

## 3. Affected Components
- **File:** `core-layer/mitm_http-server/src/handlers/mod.rs`
- **Functions:** `configure_routes`, `handle_index` (to be renamed/replaced).

## 4. Implementation Details
- **Dynamic Template Handler:** Create `handle_template(State(state), Path(name))`. The handler will format the requested name (e.g., append `.html`), inject standard context variables (like `CARGO_PKG_VERSION`), and render it via `state.tera`.
- **Axum SPA Router setup:**
  ```rust
  let public_dir = std::path::Path::new(&mitm_dir).join("html").join("public");
  let spa_service = tower_http::services::ServeDir::new(public_dir.clone())
      .not_found_service(tower_http::services::ServeFile::new(public_dir.join("index.html")));
  ```
- **Route Registration:** 
  - Register `.route("/template/:name", get(handle_template))`
  - Register `.fallback_service(spa_service)`

## 5. Drift Control & Architecture Constraints
- **Layer Separation:** This change strictly affects the Presentation/Delivery layer logic within the HTTP Server and maintains clear separation.
- **Security:** Modifying static file serving does not compromise the envelope encryption or data security layers.
- **Compatibility:** Legacy API endpoints (`/admin/*`) and V1 API endpoints (`/api/*`) are explicit routes and will take precedence over the `fallback_service`, ensuring existing functionality remains intact.

## 6. Acceptance Criteria
1. `GET /template/index` returns the Tera-rendered dynamic template.
2. `GET /` serves the static `index.html` via `ServeDir` (as `index.html` is the default file returned for the root directory by `ServeDir`).
3. `GET /random-unknown-route` returns the static `index.html` with an HTTP 200 OK, rather than a 404 Not Found.
4. Existing API routes (`/api/system/v1/info`, `/api/admin/v1/credentials`, etc.) continue to function normally.
