# Tasks: User Profile Fields, Session IP Tracking, and Conditional Roles

- **[x] Task 1:** Create migration script and update `setup.sql` to include `first_name` and `last_name`.
- **[x] Task 2:** Update `mitm_common::ipc` structs (`AuthRequest`, `AuthResponse`).
- **[x] Task 3:** Implement logic in `mitm_iam-server` to reject inactive users and return profile data.
- **[x] Task 4:** Update `mitm_http-server`'s `SessionRequest` and `create_session` to store `client_ip`.
- **[x] Task 5:** Update `mitm_http-server`'s `/api/v1/auth/me` to return `first_name`, `last_name`, `is_active`, and `client_ip`.
- **[x] Task 6:** Implement `DELETE /api/v1/iam/users/:id/session` and update User CRUD operations for profile fields.
- **[x] Task 7:** Update API documentation and Changelogs across all modified crates.
