use axum::{Router, routing::get};
use crate::handlers::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/info", get(crate::handlers::handle_info))
        .route("/time", get(crate::handlers::handle_time))
}

pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard", get(handle_dashboard_stats))
        .route("/backup", get(crate::handlers::admin::handle_backup))
}

#[derive(serde::Serialize)]
pub struct TableStat {
    pub count: i64,
    pub oldest: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(serde::Serialize)]
pub struct DashboardStats {
    pub db_name: String,
    pub db_version: String,
    pub db_size: String,
    pub stats: DashboardStatsNested,
}

#[derive(serde::Serialize)]
pub struct DashboardStatsNested {
    pub dlq: TableStat,
    pub transformation_errors: TableStat,
    pub admin_audit_logs: TableStat,
    pub system_logs: TableStat,
    pub job_audit_logs: TableStat,
    pub total_scheduled_jobs: i64,
    pub total_registered_users: i64,
}

#[derive(sqlx::FromRow)]
struct DbInfoRow {
    current_database: String,
    version: String,
    pg_size_pretty: String,
}

pub async fn handle_dashboard_stats(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    use axum::Json;
    use axum::http::StatusCode;

    let db_info = match sqlx::query_as::<_, DbInfoRow>("SELECT current_database(), version(), pg_size_pretty(pg_database_size(current_database()))")
        .fetch_one(&state.repo.get().unwrap().pool)
        .await
    {
        Ok(info) => info,
        Err(e) => {
            log::error!("Failed to fetch db info: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "errors": [{"status": "500", "title": "DB Error", "detail": e.to_string()}]
            }))).into_response();
        }
    };

    let version = if db_info.version.starts_with("PostgreSQL ") {
        let parts: Vec<&str> = db_info.version.split_whitespace().collect();
        if parts.len() >= 2 {
            format!("{} {}", parts[0], parts[1])
        } else {
            db_info.version.clone()
        }
    } else {
        db_info.version.clone()
    };

    let dlq = sqlx::query_as::<_, (i64, Option<chrono::DateTime<chrono::Utc>>)>("SELECT COUNT(*), MIN(failed_at) FROM dead_letter_queue")
        .fetch_one(&state.repo.get().unwrap().pool)
        .await
        .unwrap_or((0, None));

    let transformation_errors = sqlx::query_as::<_, (i64, Option<chrono::DateTime<chrono::Utc>>)>("SELECT COUNT(*), MIN(created_at) FROM transformation_errors")
        .fetch_one(&state.repo.get().unwrap().pool)
        .await
        .unwrap_or((0, None));

    let admin_audit_logs = sqlx::query_as::<_, (i64, Option<chrono::DateTime<chrono::Utc>>)>("SELECT COUNT(*), MIN(ts) FROM admin_audit_logs")
        .fetch_one(&state.repo.get().unwrap().pool)
        .await
        .unwrap_or((0, None));

    let system_logs = sqlx::query_as::<_, (i64, Option<chrono::DateTime<chrono::Utc>>)>("SELECT COUNT(*), MIN(ts) FROM system_logs")
        .fetch_one(&state.repo.get().unwrap().pool)
        .await
        .unwrap_or((0, None));

    let job_audit_logs = sqlx::query_as::<_, (i64, Option<chrono::DateTime<chrono::Utc>>)>("SELECT COUNT(*), MIN(ts) FROM job_audit_logs")
        .fetch_one(&state.repo.get().unwrap().pool)
        .await
        .unwrap_or((0, None));

    let total_scheduled_jobs = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scheduled_programs")
        .fetch_one(&state.repo.get().unwrap().pool)
        .await
        .unwrap_or(0);

    let total_registered_users = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM admin_users")
        .fetch_one(&state.repo.get().unwrap().pool)
        .await
        .unwrap_or(0);

    let stats = DashboardStats {
        db_name: db_info.current_database,
        db_version: version,
        db_size: db_info.pg_size_pretty,
        stats: DashboardStatsNested {
            dlq: TableStat { count: dlq.0, oldest: dlq.1 },
            transformation_errors: TableStat { count: transformation_errors.0, oldest: transformation_errors.1 },
            admin_audit_logs: TableStat { count: admin_audit_logs.0, oldest: admin_audit_logs.1 },
            system_logs: TableStat { count: system_logs.0, oldest: system_logs.1 },
            job_audit_logs: TableStat { count: job_audit_logs.0, oldest: job_audit_logs.1 },
            total_scheduled_jobs,
            total_registered_users,
        },
    };

    (StatusCode::OK, Json(stats)).into_response()
}
