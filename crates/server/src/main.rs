//! Binary entry point for the OpenBot-RS local HTTP/WebSocket server.
//!
//! Run with `cargo run -p server`. Configuration via env:
//! - `OPENBOT_DATA_DIR` (default `./data`)
//! - `OPENBOT_BIND` (default `127.0.0.1:3748`)

use app_core::OpenBot;
use server::{build_router, AppState};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let data_dir = std::env::var("OPENBOT_DATA_DIR").unwrap_or_else(|_| "./data".into());
    let bind = std::env::var("OPENBOT_BIND").unwrap_or_else(|_| "127.0.0.1:3748".into());
    let openbot = OpenBot::new(data_dir).expect("open OpenBot");
    openbot.start_routine_scheduler();
    let state = AppState { openbot };
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(&bind).await.expect("bind");
    tracing::info!("OpenBot-RS server listening on {bind}");
    axum::serve(listener, app).await.expect("serve");
}
