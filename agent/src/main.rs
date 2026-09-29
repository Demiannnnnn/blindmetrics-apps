pub mod api;
pub mod app;
pub mod background;
pub mod config;
pub mod consent;
pub mod core;
pub mod credentials;
pub mod device;
pub mod enrollment;
pub mod errors;
pub mod io;
pub mod notifications;
pub mod pipeline;
pub mod session;
pub mod state;
pub mod ui;

use app::WorkerDesktopApp;
use background::worker::EngineHandle;
use background::BackgroundEngine;
use config::WorkerConfig;
use core::buffer::MetricsBuffer;
use credentials::{CredentialStore, KeyringCredentialStore};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load environment variables from .env
    dotenvy::dotenv().ok();

    // 2. Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    // 3. Load configuration and hardware fingerprint
    let config = WorkerConfig::load_or_init();
    tracing::info!(
        "Telemetry Worker starting. Device ID: {}, Role: {}",
        &config.device_identifier[..8],
        config.worker_role.as_deref().unwrap_or("OFFICE")
    );

    // 4. Initialize metrics buffer
    let buffer = Arc::new(MetricsBuffer::new(1000));

    // 5. Check if running in headless mode (--headless)
    let is_headless = std::env::args().any(|arg| arg == "--headless");

    // 6. Spawn the background engine in a dedicated tokio runtime. The UI gets
    //    an EngineHandle to spawn work on that runtime; it never blocks on it.
    let cred_store: Arc<dyn CredentialStore> = Arc::new(KeyringCredentialStore::new());
    let (handle_tx, handle_rx) = std::sync::mpsc::channel::<EngineHandle>();
    let bg_config = config.clone();
    let bg_buffer = buffer.clone();
    let bg_creds = cred_store.clone();
    std::thread::Builder::new()
        .name("telemetry-engine".into())
        .spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to initialize background Tokio runtime: {:?}", e);
                    return;
                }
            };
            let engine = match BackgroundEngine::new(bg_config, bg_buffer, bg_creds) {
                Ok(engine) => engine,
                Err(e) => {
                    tracing::error!("Failed to initialize background engine: {:?}", e);
                    return;
                }
            };
            let _ = handle_tx.send(engine.handle(rt.handle().clone()));
            rt.block_on(engine.run());
        })?;
    let engine = handle_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .map_err(|e| tracing::error!("Background engine unavailable: {}", e))
        .ok();

    if is_headless {
        tracing::info!(
            "Telemetry Worker running in headless daemon mode. Press Ctrl+C to terminate."
        );
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
    } else {
        tracing::info!("Launching Telemetry Worker Desktop UI (egui/eframe)...");
        let native_options = eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([500.0, 680.0])
                .with_min_inner_size([420.0, 520.0])
                .with_title("Telemetry Worker — Transparencia Activa"),
            ..Default::default()
        };

        eframe::run_native(
            "Telemetry Worker",
            native_options,
            Box::new(move |_cc| {
                Box::new(WorkerDesktopApp::new(config, buffer, cred_store, engine))
            }),
        )
        .map_err(|e| format!("eframe execution failed: {}", e))?;
    }

    Ok(())
}
