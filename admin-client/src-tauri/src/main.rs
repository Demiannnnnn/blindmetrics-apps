#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod client;
pub mod commands;
pub mod keystore;

use commands::{
    admin_decrypt_records, admin_generate_keypair, admin_get_public_key, admin_get_session,
    admin_lock_and_flush_memory, admin_login, admin_logout, admin_register,
    admin_simulate_and_decrypt,
};

fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    tracing::info!("Telemetry Admin Desktop Core initialized");

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            admin_login,
            admin_register,
            admin_logout,
            admin_get_session,
            admin_generate_keypair,
            admin_get_public_key,
            admin_decrypt_records,
            admin_simulate_and_decrypt,
            admin_lock_and_flush_memory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
