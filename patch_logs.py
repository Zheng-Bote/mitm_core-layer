import re

# 1. mitm_http-server/src/main.rs: Log success_msg to DB when initialized
with open('mitm_http-server/src/main.rs', 'r') as f:
    http_main = f.read()

http_main = http_main.replace(
    'log::info!("Database initialized successfully. 503 Middleware lifted!");',
    'log::info!("Database initialized successfully. 503 Middleware lifted!");\n                                        let success_msg = format!("Starting {} (v{})", APP_NAME, VERSION);\n                                        let _ = repo_cell_for_uds.get().unwrap().log_system("INFO", "http-server", &success_msg).await;'
)

with open('mitm_http-server/src/main.rs', 'w') as f:
    f.write(http_main)


# 2. mitm_http-server/src/handlers/rbac.rs: Log role assignment to admin_audit_logs
with open('mitm_http-server/src/handlers/rbac.rs', 'r') as f:
    rbac = f.read()

# We need to get `auth` extension to log the username.
# Currently handle_assign_roles signature:
# async fn handle_assign_roles(
#     State(state): State<AppState>,
#     Json(payload): Json<AssignRolesReq>,
# ) -> impl IntoResponse {

rbac = re.sub(
    r'async fn handle_assign_roles\(\n    State\(state\): State<AppState>,\n    Json\(payload\): Json<AssignRolesReq>,\n\) -> impl IntoResponse \{',
    r'async fn handle_assign_roles(\n    State(state): State<AppState>,\n    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,\n    Json(payload): Json<AssignRolesReq>,\n) -> impl IntoResponse {',
    rbac
)

rbac = re.sub(
    r'        Ok\(_\) => StatusCode::OK\.into_response\(\),',
    r'        Ok(_) => {\n            let _ = sqlx::query("INSERT INTO admin_audit_logs (username, action, details) VALUES ($1, $2, $3)")\n                .bind(&auth.username)\n                .bind("assign_roles")\n                .bind(serde_json::json!({"user_id": payload.user_id, "roles": payload.role_ids}))\n                .execute(&state.repo.get().unwrap().pool)\n                .await;\n            StatusCode::OK.into_response()\n        },',
    rbac
)

with open('mitm_http-server/src/handlers/rbac.rs', 'w') as f:
    f.write(rbac)

# 3. mitm_iam-server/src/db.rs: Log missing admin creation
with open('mitm_iam-server/src/db.rs', 'r') as f:
    iam_db = f.read()

iam_db = re.sub(
    r'                        log::info!\("Created initial admin user: \{\}", admin_cfg\.username\);\n                        id',
    r'                        log::info!("Created initial admin user: {}", admin_cfg.username);\n                        let _ = sqlx::query("INSERT INTO admin_audit_logs (username, action, details) VALUES ($1, $2, $3)")\n                            .bind("system")\n                            .bind("create_admin")\n                            .bind(serde_json::json!({"created_user": admin_cfg.username}))\n                            .execute(&repo.pool)\n                            .await;\n                        id',
    iam_db
)

with open('mitm_iam-server/src/db.rs', 'w') as f:
    f.write(iam_db)

