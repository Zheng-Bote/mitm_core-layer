mod ipc_client;
mod db;
mod handlers;
#[allow(warnings, clippy::all)]
mod schematas;
use std::env;
use axum_server::tls_rustls::RustlsConfig;
use mitm_common::config::load_config;
use std::net::SocketAddr;
use tokio::signal;
use axum::middleware;

const APP_NAME: &str = "MitM HTTP Gateway";
const VERSION: &str = env!("CARGO_PKG_VERSION");

async fn send_log(socket_path: &std::path::Path, message: &str) {
    use tokio::net::UnixStream;
    use tokio::io::AsyncWriteExt;

    let req = serde_json::json!({
        "action": "log_system",
        "payload": {
            "level": "INFO",
            "component": "http-server",
            "message": message
        }
    });

    if let Ok(mut stream) = UnixStream::connect(socket_path).await {
        let mut out = serde_json::to_string(&req).unwrap_or_default();
        out.push('\n');
        let _ = stream.write_all(out.as_bytes()).await;
    }
}

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
        log::error!("{}", err_msg);
        let _ = state.repo.log_system("ERROR", "http-server", &err_msg).await;
    }
    res
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let config_param = args.get(1).map(|s| s.as_str());
    let password = env::var("MASTER_KEY").unwrap_or_else(|_| "".to_string());
    
    let config = match load_config(config_param, &password) {
        Ok(cfg) => cfg,
        Err(e) => {
            log::error!("Failed to load configuration: {}", e);
            return Err(e);
        }
    };

    let config_clone = config.clone();
    let auth_layer = axum::middleware::from_fn(move |req, next| {
        ipc_client::auth_middleware(req, next, config_clone.clone())
    });

    let repo = std::sync::Arc::new(db::Repository::new(&config).await?);
    let kek = std::sync::Arc::new(password.as_bytes().to_vec());
    let app_state = handlers::AppState { repo, config: std::sync::Arc::new(config.clone()), kek };
    
    let error_layer = middleware::from_fn_with_state(app_state.clone(), error_logging_middleware);

    let app = handlers::configure_routes()
        .layer(error_layer)
        .layer(auth_layer)
        .with_state(app_state);
        
    let socket_dir = std::path::PathBuf::from(&config.socket_dir);
    let socket_path = socket_dir.join("mitm_iam.sock");

    let success_msg = format!("{} ({}) started successfully", APP_NAME, VERSION);
    log::info!("{}", success_msg);

    let socket_path_clone = socket_path.clone();
    let success_msg_clone = success_msg.clone();
    tokio::spawn(async move {
        send_log(&socket_path_clone, &success_msg_clone).await;
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], config.http_port));
    
    let handle = axum_server::Handle::new();
    let handle_clone = handle.clone();

    let server_task = tokio::spawn(async move {
        if config.use_https {
            log::info!("HTTP Server listening on HTTPS {}", addr);
            let tls_config = RustlsConfig::from_pem_file(&config.ssl_cert, &config.ssl_key)
                .await
                .unwrap_or_else(|e| panic!("Failed to load TLS certificates: {}", e));
                
            let _ = axum_server::bind_rustls(addr, tls_config)
                .handle(handle_clone)
                .serve(app.into_make_service())
                .await;
        } else {
            log::info!("HTTP Server listening on HTTP {}", addr);
            let _ = axum_server::bind(addr)
                .handle(handle_clone)
                .serve(app.into_make_service())
                .await;
        }
    });

    // Wait for termination
    tokio::select! {
        _ = signal::ctrl_c() => {},
        _ = async {
            #[cfg(unix)]
            {
                if let Ok(mut sig) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
                    sig.recv().await;
                }
            }
            #[cfg(not(unix))]
            std::future::pending::<()>().await;
        } => {},
    }

    let shutdown_msg = "Shutting down...";
    log::info!("{}", shutdown_msg);
    send_log(&socket_path, shutdown_msg).await;
    handle.graceful_shutdown(Some(std::time::Duration::from_secs(30)));
    let _ = server_task.await;

    Ok(())
}
