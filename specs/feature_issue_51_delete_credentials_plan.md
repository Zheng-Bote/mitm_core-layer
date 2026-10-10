# Plan: Delete Endpoints for Source and Target Credentials

1. **Implement `DELETE` Handlers in `admin.rs`**
   - Create `handle_delete_credentials(State(state), Extension(auth), Path(id))` in `mitm_http-server/src/handlers/admin.rs`. It will:
     - Run `DELETE FROM source_credentials WHERE id = $1`.
     - Log to `admin_audit_logs` indicating the deleted credential ID and the action `DELETE_CREDENTIAL`.
     - Return `StatusCode::OK` on success, or `StatusCode::INTERNAL_SERVER_ERROR` on failure.
   - Create `handle_delete_delivery_targets(State(state), Extension(auth), Path(id))` in `mitm_http-server/src/handlers/admin.rs`. It will:
     - Run `DELETE FROM delivery_targets WHERE id = $1`.
     - Log to `admin_audit_logs` indicating the deleted target ID and the action `DELETE_TARGET`.
     - Return `StatusCode::OK` on success, or `StatusCode::INTERNAL_SERVER_ERROR` on failure.

2. **Register the Routes in `api_v1/config.rs`**
   - In `mitm_http-server/src/handlers/api_v1/config.rs`, modify the `routes()` function to add:
     - `.route("/credentials/:id", delete(crate::handlers::admin::handle_delete_credentials))`
     - `.route("/targets/:id", delete(crate::handlers::admin::handle_delete_delivery_targets))`

3. **Update Documentation**
   - Update `mitm_http-server/docs/api/api_v1.md` to indicate the new `DELETE` endpoints and their `ADMIN` requirement.

4. **Update CHANGELOG.md**
   - Add a note about the newly added endpoints to the changelog.
