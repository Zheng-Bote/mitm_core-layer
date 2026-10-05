use casbin::MgmtApi;
use casbin::CoreApi;
/*
 * SPDX-License-Identifier: Apache-2.0
 */

mod ipc_client;
mod db;
mod handlers;
#[allow(warnings, clippy::all)]
mod schematas;
use std::env;

use mitm_common::config::load_config;

use tokio::signal;
use axum::middleware;

const APP_NAME: &str = "MitM HTTP Gateway";
const VERSION: &str = env!("CARGO_PKG_VERSION");

async fn error_logging_middleware(
    axum::extract::State(state): axum::extract::State<handlers::AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let res = next.run(req).await;
    
    if res.status().is_server_error() || res.status().is_client_error() {
        let status = res.status();
        let err_msg = format!("HTTP {} - {} {}", status, method, uri);
        
        // Log to STDOUT immediately
        log::error!("{}", err_msg);
        
        // Only log to DB if UDS/DB connection is fully established
        if let Some(repo) = state.repo.get() {
            let _ = repo.log_system("ERROR", "http-server", &err_msg).await;
        }
    }
    res
}

async fn readiness_middleware(
    axum::extract::State(state): axum::extract::State<handlers::AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = req.uri().path();
    
    // Exclude basic health, info, and SPA root from the blockade
    if path == "/health" || path == "/info" || path == "/" {
        return next.run(req).await;
    }

    // Block functional routes if UDS initialization is not complete
    if state.repo.get().is_none() {
        let err_msg = serde_json::json!({
            "errors": [{
                "status": "503",
                "title": "Service Unavailable",
                "detail": "Core components are still initializing. Please wait."
            }]
        });
        return axum::response::IntoResponse::into_response((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(err_msg),
        ));
    }

    next.run(req).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().filter_or("MITM_LOG_LEVEL", "info")).init();

    let args: Vec<String> = env::args().collect();
    let config_param = args.get(1).map(|s| s.as_str());
    
    // Read MASTER_KEY to decrypt config.enc (fallback to empty string for unencrypted json)
    let password = env::var("MASTER_KEY").unwrap_or_else(|_| "".to_string());
    let config = match load_config(config_param, &password) {
        Ok(cfg) => cfg,
        Err(e) => {
            log::error!("Failed to load configuration: {}", e);
            return Err(e);
        }
    };

    let success_msg = format!("Starting {} (v{})", APP_NAME, VERSION);
    log::info!("{}", success_msg);

    // TASK 3: ECS Supervisor Engine -> Child-Prozesse spawnen
    
    // Dynamischer Pfad: Suche die Sub-Prozesse im selben Ordner wie den HTTP-Core
    let current_exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let bin_dir = current_exe.parent().unwrap_or(std::path::Path::new("."));
    
    let iam_path = bin_dir.join("mitm-core-iam");
    let scheduler_path = bin_dir.join("mitm-core-scheduler");

    let mut iam_command = tokio::process::Command::new(&iam_path);
    if let Some(cfg) = config_param {
        iam_command.arg(cfg);
    }
    let mut iam_child = iam_command
        .kill_on_drop(true)
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to spawn IAM server child process from {:?}: {}", iam_path, e));
        
    let mut scheduler_command = tokio::process::Command::new(&scheduler_path);
    if let Some(cfg) = config_param {
        scheduler_command.arg(cfg);
    }
    let mut scheduler_child = scheduler_command
        .kill_on_drop(true)
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to spawn Scheduler child process from {:?}: {}", scheduler_path, e));

    let repo_cell = std::sync::Arc::new(tokio::sync::OnceCell::new());
    let handle = axum_server::Handle::new();
    let handle_clone = handle.clone();

    // TASK 3: UDS Retry Loop (Warten auf Scheduler & DB Credentials)
    let config_for_uds = config.clone();
    let repo_cell_for_uds = repo_cell.clone();

    // ECS Health-Binding wird verzögert, bis wir die Config vom Scheduler haben.
    // Das dauert typischerweise nur 1-2 Sekunden, was für den AWS ECS Health Check schnell genug ist.

    tokio::spawn(async move {
        let socket_dir = std::path::PathBuf::from(&config_for_uds.socket_dir);
        let scheduler_sock = socket_dir.join("mitm_scheduler.sock");
        let mut attempts = 0;
        
        loop {
            if scheduler_sock.exists() {
                log::info!("Scheduler UDS socket found! Fetching credentials via IPC...");
                match crate::ipc_client::get_credentials(&scheduler_sock).await {
                    Ok(creds_resp) => {
                        match serde_json::from_str::<mitm_common::config::DBConfig>(&creds_resp.db_config_json) {
                            Ok(mut parsed_cfg) => {
                                parsed_cfg.socket_dir = config_for_uds.socket_dir.clone();
                                if let Some(repo) = db::Repository::new(&parsed_cfg).await.ok() {
                                    if repo_cell_for_uds.set(repo).is_ok() {
                                        log::info!("Database initialized successfully. 503 Middleware lifted!");
                                        let success_msg = format!("Starting {} (v{})", APP_NAME, VERSION);
                                        let _ = repo_cell_for_uds.get().unwrap().log_system("INFO", "http-server", &success_msg).await;
                                        
                                        // Build true app state with the correct config
                                        let template_dir = std::path::Path::new(&parsed_cfg.mitm_dir).join("html").join("templates");
                                        let tera = match tera::Tera::new(&format!("{}/**/*", template_dir.to_string_lossy())) {
                                            Ok(t) => t,
                                            Err(e) => {
                                                log::error!("Failed to parse templates: {}", e);
                                                // Create a dummy Tera if it fails so it doesn't crash the server hard, but log the error
                                                tera::Tera::default()
                                            }
                                        };
                                        


                                        // Initialize Casbin Enforcer with inline model
                                        let model_text = "
[request_definition]
r = sub, obj, act

[policy_definition]
p = sub, obj, act

[role_definition]
g = _, _

[policy_effect]
e = some(where (p.eft == allow))

[matchers]
m = g(r.sub, p.sub) && (keyMatch(r.obj, p.obj) || keyMatch2(r.obj, p.obj) || r.obj == p.obj) && (p.act == \".*\" || r.act == p.act || regexMatch(r.act, p.act))
";
                                        let m = casbin::DefaultModel::from_str(model_text).await.unwrap();
                                        let a = sqlx_adapter::SqlxAdapter::new_with_pool(repo_cell_for_uds.get().unwrap().pool.clone()).await.unwrap();
                                        let mut enforcer = casbin::Enforcer::new(m, a).await.unwrap();
                                        
                                        // Seed default policies (Task 2)
                                        let _ = enforcer.add_grouping_policy(vec!["ADMIN".to_string(), "USER".to_string()]).await;
                                        let _ = enforcer.add_grouping_policy(vec!["USER".to_string(), "VIEWER".to_string()]).await;

                                        if !enforcer.has_policy(vec!["VIEWER".to_string(), "/api/v1/system/dashboard".to_string(), "GET".to_string()]) {
                                            let _ = enforcer.add_policy(vec!["VIEWER".to_string(), "/api/v1/system/dashboard".to_string(), "GET".to_string()]).await;
                                        }
                                        if !enforcer.has_policy(vec!["VIEWER".to_string(), "/api/v1/jobs".to_string(), "GET".to_string()]) {
                                            let _ = enforcer.add_policy(vec!["VIEWER".to_string(), "/api/v1/jobs".to_string(), "GET".to_string()]).await;
                                        }

                                        if !enforcer.has_policy(vec!["USER".to_string(), "/api/v1/jobs/*".to_string(), "POST".to_string()]) {
                                            let _ = enforcer.add_policy(vec!["USER".to_string(), "/api/v1/jobs/*".to_string(), "POST".to_string()]).await;
                                        }
                                        if !enforcer.has_policy(vec!["USER".to_string(), "/api/v1/logs/*".to_string(), "GET".to_string()]) {
                                            let _ = enforcer.add_policy(vec!["USER".to_string(), "/api/v1/logs/*".to_string(), "GET".to_string()]).await;
                                        }
                                        if !enforcer.has_policy(vec!["USER".to_string(), "/api/v1/dlq".to_string(), "GET".to_string()]) {
                                            let _ = enforcer.add_policy(vec!["USER".to_string(), "/api/v1/dlq".to_string(), "GET".to_string()]).await;
                                        }
                                        if !enforcer.has_policy(vec!["USER".to_string(), "/api/v1/dlq/requeue".to_string(), "POST".to_string()]) {
                                            let _ = enforcer.add_policy(vec!["USER".to_string(), "/api/v1/dlq/requeue".to_string(), "POST".to_string()]).await;
                                        }
                                        if !enforcer.has_policy(vec!["USER".to_string(), "/api/v1/uploads/*".to_string(), "POST".to_string()]) {
                                            let _ = enforcer.add_policy(vec!["USER".to_string(), "/api/v1/uploads/*".to_string(), "POST".to_string()]).await;
                                        }

                                        if !enforcer.has_policy(vec!["ADMIN".to_string(), "/api/v1/*".to_string(), ".*".to_string()]) {
                                            let _ = enforcer.add_policy(vec!["ADMIN".to_string(), "/api/v1/*".to_string(), ".*".to_string()]).await;
                                        }

                                        let app_state = handlers::AppState { 
                                            repo: repo_cell_for_uds.clone(), 
                                            config: std::sync::Arc::new(parsed_cfg.clone()),
                                            tera: std::sync::Arc::new(tera),
                                            enforcer: std::sync::Arc::new(tokio::sync::RwLock::new(enforcer)),
                                        };

                                        
                                        let auth_layer = axum::middleware::from_fn_with_state(
                                            app_state.clone(),
                                            crate::ipc_client::auth_middleware
                                        );

                                        let ready_layer = middleware::from_fn_with_state(app_state.clone(), readiness_middleware);
                                        let error_layer = middleware::from_fn_with_state(app_state.clone(), error_logging_middleware);

                                        let app = handlers::configure_routes(parsed_cfg.mitm_dir.clone(), app_state.clone())
                                            .layer(ready_layer)
                                            .layer(error_layer)
                                            .layer(auth_layer)
                                            .with_state(app_state);
                                            
                                        // Start HTTP Server dynamically with true config
                                        let real_addr = std::net::SocketAddr::from(([0, 0, 0, 0], parsed_cfg.http_port));
                                        let app_clone = app.clone();
                                        let handle_c = handle_clone.clone();
                                        
                                        if parsed_cfg.use_https {
                                            log::info!("Starting HTTP listener on HTTPS {}", real_addr);
                                            tokio::spawn(async move {
                                                let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(&parsed_cfg.ssl_cert, &parsed_cfg.ssl_key)
                                                    .await
                                                    .unwrap_or_else(|e| panic!("Failed to load TLS certificates: {}", e));
                                                let _ = axum_server::bind_rustls(real_addr, tls_config)
                                                    .handle(handle_c)
                                                    .serve(app_clone.into_make_service())
                                                    .await;
                                            });
                                        } else {
                                            log::info!("Starting HTTP listener on HTTP {}", real_addr);
                                            tokio::spawn(async move {
                                                let _ = axum_server::bind(real_addr)
                                                    .handle(handle_c)
                                                    .serve(app_clone.into_make_service())
                                                    .await;
                                            });
                                        }
                                        break;
                                    }
                                } else {
                                    log::error!("Failed to initialize DB with credentials from Scheduler. Retrying...");
                                }
                            }
                            Err(e) => log::error!("Failed to parse DB config from Scheduler: {}", e),
                        }
                    }
                    Err(e) => log::error!("IPC GetCredentials failed: {}. Retrying...", e),
                }
            }
            attempts += 1;
            log::info!("Waiting for UDS socket to become ready (Attempt {})...", attempts);
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });

    // Supervisor Monitoring Loop & SIGTERM Handling
    let exit_code = tokio::select! {
        _ = signal::ctrl_c() => {
            log::info!("Received Ctrl-C, shutting down gracefully...");
            0
        },
        _ = async {
            #[cfg(unix)]
            {
                if let Ok(mut sig) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
                    sig.recv().await;
                    log::info!("Received SIGTERM from AWS ECS, shutting down gracefully...");
                }
            }
            #[cfg(not(unix))]
            std::future::pending::<()>().await;
        } => {
            0
        },
        // Wait on IAM process crash
        status = iam_child.wait() => {
            log::error!("FATAL: IAM server child exited unexpectedly with {:?}", status);
            1
        },
        // Wait on Scheduler crash
        status = scheduler_child.wait() => {
            log::error!("FATAL: Scheduler server child exited unexpectedly with {:?}", status);
            1
        }
    };

    let shutdown_msg = "Shutting down...";
    log::info!("{}", shutdown_msg);
    if let Some(repo) = repo_cell.get() {
        let _ = repo.log_system("INFO", "http-server", shutdown_msg).await;
    }
    log::info!("Forwarding shutdown signals to children...");
    
    // Child-Prozesse beenden
    let _ = iam_child.kill().await;
    let _ = scheduler_child.kill().await;

    // Graceful Shutdown für den Webserver
    handle.graceful_shutdown(Some(std::time::Duration::from_secs(30)));

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

