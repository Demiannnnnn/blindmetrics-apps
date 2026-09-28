pub mod background;
pub mod config;
pub mod core;
pub mod device;
pub mod errors;
pub mod io;
pub mod notifications;
pub mod ui;

use std::sync::Arc;
use config::WorkerConfig;
use core::buffer::MetricsBuffer;
use background::BackgroundEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    // 2. Load configuration and hardware fingerprint
    let config = WorkerConfig::load_or_init();
    tracing::info!(
        "Telemetry Worker starting. Device ID: {}, Role: {}",
        &config.device_identifier[..8],
        config.worker_role
    );

    // 3. Initialize metrics buffer
    let buffer = Arc::new(MetricsBuffer::new(1000));

    // 4. Start background collection, periodic notification, and dispatch loop
    let engine = BackgroundEngine::new(config, buffer);
    engine.run().await;

    Ok(())
}
