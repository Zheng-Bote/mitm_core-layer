import re

with open('mitm_http-server/src/handlers/rbac.rs', 'r') as f:
    rbac = f.read()

rbac = re.sub(
    r'async fn handle_assign_roles\(\n    State\(state\): State<AppState>,\n    Json\(payload\): Json<AssignRolesReq>,\n\) -> impl IntoResponse \{',
    r'async fn handle_assign_roles(\n    State(state): State<AppState>,\n    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,\n    Json(payload): Json<AssignRolesReq>,\n) -> impl IntoResponse {',
    rbac
)

# Only replace the one in handle_assign_roles
rbac = re.sub(
    r'\.execute\(&state\.repo\.get\(\)\.unwrap\(\)\.pool\)\n    \.await \{\n        Ok\(_\) => StatusCode::OK\.into_response\(\),\n        Err\(e\) => \{\n            let err = ErrorResponse \{ errors: vec!\[JsonApiError \{ status: "500"\.into\(\), title: "DB error"\.into\(\), detail: Some\(e\.to_string\(\)\) \}\] \};\n            \(StatusCode::INTERNAL_SERVER_ERROR, Json\(err\)\)\.into_response\(\)\n        \}\n    \}',
    r'.execute(&state.repo.get().unwrap().pool)\n    .await {\n        Ok(_) => {\n            let _ = sqlx::query("INSERT INTO admin_audit_logs (username, action, details) VALUES ($1, $2, $3)")\n                .bind(&auth.username)\n                .bind("assign_roles")\n                .bind(serde_json::json!({"user_id": payload.user_id, "roles": payload.role_ids}))\n                .execute(&state.repo.get().unwrap().pool)\n                .await;\n            StatusCode::OK.into_response()\n        },\n        Err(e) => {\n            let err = ErrorResponse { errors: vec![JsonApiError { status: "500".into(), title: "DB error".into(), detail: Some(e.to_string()) }] };\n            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()\n        }\n    }',
    rbac
)

with open('mitm_http-server/src/handlers/rbac.rs', 'w') as f:
    f.write(rbac)
