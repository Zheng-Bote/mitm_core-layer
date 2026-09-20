mod ipc_client;
mod db;
mod handlers;
#[allow(warnings, clippy::all)]
mod schematas;
use std::env;
use axum_server::tls_rustls::RustlsConfig;
use mitm_common::config::load_config;
use std::net::SocketAddr;


const APP_NAME: &str = "MitM HTTP Gateway";
const VERSION: &str = env!("CARGO_PKG_VERSION");

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
    let app_state = handlers::AppState { repo, config: std::sync::Arc::new(config.clone()) };

    let app = handlers::configure_routes()
        .layer(auth_layer)
        .with_state(app_state);
        
    let socket_dir = std::path::PathBuf::from(&config.socket_dir);
    let socket_path = socket_dir.join("mitm_iam.sock");

    // Log startup via IPC to iam-server
    tokio::spawn(async move {
        use tokio::net::UnixStream;
        use tokio::io::AsyncWriteExt;

        let req = serde_json::json!({
            "action": "log_system",
            "payload": {
                "level": "INFO",
                "component": "http-server",
                "message": format!("Starting {} v{}", APP_NAME, VERSION)
            }
        });

        if let Ok(mut stream) = UnixStream::connect(&socket_path).await {
            let mut out = serde_json::to_string(&req).unwrap_or_default();
            out.push('\n');
            let _ = stream.write_all(out.as_bytes()).await;
        }
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], config.http_port));
    
    if config.use_https {
        log::info!("Starting HTTPS Server on {}", addr);
        let tls_config = RustlsConfig::from_pem_file(&config.ssl_cert, &config.ssl_key)
            .await
            .unwrap_or_else(|e| panic!("Failed to load TLS certificates: {}", e));
            
        axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service())
            .await?;
    } else {
        log::info!("Starting HTTP Server on {}", addr);
        axum_server::bind(addr)
            .serve(app.into_make_service())
            .await?;
    }

    Ok(())
}
