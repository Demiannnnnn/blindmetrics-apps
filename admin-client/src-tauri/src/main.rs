#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod client;
pub mod commands;
pub mod keystore;

use commands::keys::{
    admin_generate_keypair, admin_get_public_key, keys_register_on_server, keys_status,
};
use commands::session::{
    admin_get_session, admin_lock_and_flush_memory, admin_login, admin_logout, admin_register,
};
use commands::telemetry::{
    admin_decrypt_records, admin_simulate_and_decrypt, fetch_and_decrypt_telemetry,
};
use commands::workers::{
    workers_create, workers_delete, workers_get, workers_get_consent_records, workers_list,
    workers_regenerate_token, workers_set_status, workers_update_consent_config,
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
            admin_lock_and_flush_memory,
            admin_generate_keypair,
            admin_get_public_key,
            keys_status,
            keys_register_on_server,
            admin_decrypt_records,
            admin_simulate_and_decrypt,
            fetch_and_decrypt_telemetry,
            workers_list,
            workers_get,
            workers_create,
            workers_set_status,
            workers_regenerate_token,
            workers_update_consent_config,
            workers_get_consent_records,
            workers_delete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
