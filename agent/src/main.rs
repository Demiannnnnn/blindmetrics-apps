pub mod api;
pub mod app;
pub mod background;
pub mod config;
pub mod core;
pub mod device;
pub mod errors;
pub mod io;
pub mod notifications;
pub mod pipeline;
pub mod ui;

use app::WorkerDesktopApp;
use background::BackgroundEngine;
use config::WorkerConfig;
use core::buffer::MetricsBuffer;
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
        config.worker_role
    );

    // 4. Initialize metrics buffer
    let buffer = Arc::new(MetricsBuffer::new(1000));

    // 5. Check if running in headless mode (--headless)
    let is_headless = std::env::args().any(|arg| arg == "--headless");

    // 6. Spawn the background engine in a dedicated tokio runtime
    let bg_config = config.clone();
    let bg_buffer = buffer.clone();
    std::thread::spawn(move || match tokio::runtime::Runtime::new() {
        Ok(rt) => {
            rt.block_on(async {
                let engine = BackgroundEngine::new(bg_config, bg_buffer);
                engine.run().await;
            });
        }
        Err(e) => {
            tracing::error!("Failed to initialize background Tokio runtime: {:?}", e);
        }
    });

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
            Box::new(|_cc| Box::new(WorkerDesktopApp::new(config, buffer))),
        )
        .map_err(|e| format!("eframe execution failed: {}", e))?;
    }

    Ok(())
}
