# Plan: User Profile Fields, Session IP Tracking, and Conditional Roles

1. **Database Migration**
   - Create `../migrations/003_admin_users_profile.sql` to add `first_name` and `last_name` to `admin_users`.
   - Update `../migrations/setup.sql` to include `first_name` and `last_name` in the initial `CREATE TABLE` block.

2. **IPC Updates (`mitm_common`)**
   - Add `first_name`, `last_name`, `is_active` to `AuthResponse`.
   - Update `AuthRequest` to include an optional `client_ip`.

3. **IAM Server Updates (`mitm_iam-server`)**
   - Update `db::authenticate_user` to query `first_name`, `last_name`, and `is_active`.
   - If `is_active` is `false`, return an `Err("Account is inactive")` immediately before checking roles.
   - Map `first_name` and `last_name` into the `AuthResponse`.

4. **HTTP Server Updates (`mitm_http-server`)**
   - **`SessionRequest`**: Add `client_ip: Option<String>`.
   - **`auth::create_session`**: When inserting into `user_sessions`, bind the `client_ip`.
   - **`auth::me` (`GET /api/v1/auth/me`)**: Query `first_name`, `last_name`, `is_active` from `admin_users` and `client_ip` from `user_sessions` and return them.
   - **Session Deletion**: Create `DELETE /api/v1/iam/users/:id/session` handler in `rbac.rs` / `iam.rs` to delete a session matching the user's `username`.
   - **User CRUD**: Update `UserResponse`, `CreateUserRequest`, and `UpdateUserRequest` in `rbac.rs` to include `first_name` and `last_name`. Update the `INSERT` and `UPDATE` statements to handle these fields (and ensure `is_active` is properly bound).

5. **Documentation & Changelog**
   - Update `api_v1.md` with the new endpoint and payload changes.
   - Update `CHANGELOG.md` of `mitm_common`, `mitm_iam-server`, and `mitm_http-server`.
