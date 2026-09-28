# Task Plan: SPA Routing & Dynamic Tera Templates (Issue #36)

## Phase 1: Implementation

- [ ] **Task 1: Rewrite Tera Handler**
  - **File:** `core-layer/mitm_http-server/src/handlers/mod.rs`
  - **Action:** Rename the `handle_index` function to `handle_template`.
  - **Details:** Add `Path(name): Path<String>` as a function parameter. Change the hardcoded `"index.html"` lookup in `state.tera.render` to `&format!("{}.html", name)`. If the template is not found in the `tera` engine, map the error to an HTTP `404 Not Found` response instead of an HTTP `500`.

- [ ] **Task 2: Configure SPA Fallback Service**
  - **File:** `core-layer/mitm_http-server/src/handlers/mod.rs`
  - **Action:** Modify the `serve_dir` definition inside `configure_routes`.
  - **Details:** Import `tower_http::services::ServeFile` if necessary. Chain `.not_found_service(tower_http::services::ServeFile::new(public_dir.join("index.html")))` onto the existing `tower_http::services::ServeDir::new(public_dir.clone())`.

- [ ] **Task 3: Update Router Configuration**
  - **File:** `core-layer/mitm_http-server/src/handlers/mod.rs`
  - **Action:** Replace the root Tera route and apply the new fallback logic in the `Router::new()` builder.
  - **Details:** 
    - Remove `.route("/", get(handle_index))`.
    - Add `.route("/template/:name", get(handle_template))`.
    - Ensure `.fallback_service(serve_dir)` correctly consumes the updated SPA fallback service.

## Phase 2: Documentation & Release

- [ ] **Task 4: Update Documentation**
  - **File:** `core-layer/mitm_http-server/CHANGELOG.md` & `core-layer/CHANGELOG.md`
  - **Action:** Document the routing paradigm shift. Mention that the HTTP server is now configured for Angular SPA client-side routing on fallback, and Tera templates have been namespaced to `/template/*`.

- [ ] **Task 5: Pull Request Assembly**
  - **Action:** Ensure the code compiles cleanly (`cargo check`). Push the code to the feature branch and create the Pull Request via the GitHub CLI.
