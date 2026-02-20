use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing_subscriber;

use gdml_studio_backend::api;
use gdml_studio_backend::config;
use gdml_studio_backend::state;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let shared_state = state::app_state::create_shared_state();

    // API routes
    let api_router = api::routes::create_router(shared_state.clone());

    // CORS for dev mode (Vite runs on different port)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Try to serve static frontend files
    let cwd = std::env::current_dir().unwrap_or_default();
    let frontend_dir = if cwd.join("frontend/dist").exists() {
        cwd.join("frontend/dist")
    } else {
        cwd.join("../frontend/dist")
    };

    let app = if frontend_dir.exists() {
        api_router
            .fallback_service(ServeDir::new(frontend_dir))
            .layer(cors)
    } else {
        tracing::info!("No frontend/dist found, serving API only (use Vite dev server for frontend)");
        api_router.layer(cors)
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], config::DEFAULT_PORT));
    tracing::info!("GDML Studio backend starting on http://{}", addr);

    // Try to open browser
    let url = format!("http://{}", addr);
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        let _ = open::that(&url);
    });

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
