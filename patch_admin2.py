import re

with open('mitm_http-server/src/handlers/admin.rs', 'r') as f:
    content = f.read()

# Replace handle_backup
content = re.sub(
    r'async fn handle_backup\(State\(state\): State<AppState>\) -> impl IntoResponse {',
    r'async fn handle_backup(State(state): State<AppState>, axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>) -> impl IntoResponse {',
    content
)
content = re.sub(
    r'\.bind\("admin"\)\n\s*\.bind\("BACKUP_CONFIG"\)',
    r'.bind(&auth.username)\n        .bind("BACKUP_CONFIG")',
    content
)

# Replace handle_restore
content = re.sub(
    r'async fn handle_restore\(\n\s*State\(state\): State<AppState>,\n\s*Json\(payload\): Json<BackupPayload>,\n\) -> impl IntoResponse {',
    r'async fn handle_restore(\n    State(state): State<AppState>,\n    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,\n    Json(payload): Json<BackupPayload>,\n) -> impl IntoResponse {',
    content
)
content = re.sub(
    r'\.bind\("admin"\)\n\s*\.bind\("RESTORE_CONFIG"\)',
    r'.bind(&auth.username)\n        .bind("RESTORE_CONFIG")',
    content
)

# Replace handle_key_rotation
content = re.sub(
    r'async fn handle_key_rotation\(\n\s*State\(state\): State<AppState>,\n\s*Json\(payload\): Json<KeyRotationPayload>,\n\) -> impl IntoResponse {',
    r'async fn handle_key_rotation(\n    State(state): State<AppState>,\n    axum::extract::Extension(auth): axum::extract::Extension<mitm_common::ipc::AuthResponse>,\n    Json(payload): Json<KeyRotationPayload>,\n) -> impl IntoResponse {',
    content
)
content = re.sub(
    r'\.bind\("admin"\)\n\s*\.bind\("key_rotation_success"\)',
    r'.bind(&auth.username)\n        .bind("key_rotation_success")',
    content
)

with open('mitm_http-server/src/handlers/admin.rs', 'w') as f:
    f.write(content)
