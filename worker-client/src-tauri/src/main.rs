// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    if let Err(e) = agent_ui_core::run() {
        tracing::error!("Telemetry Worker window failed: {}", e);
        std::process::exit(1);
    }
}
