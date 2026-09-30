#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod client;
pub mod commands;
pub mod device;
pub mod google_oauth;
pub mod keystore;

use client::{get_api_client, ClientEvents, SubscriptionBlock};
use commands::devices::{devices_list, devices_rename, devices_revoke};
use commands::keys::{admin_get_public_key, keys_register_on_server, keys_status};
use commands::session::{
    admin_get_session, admin_is_locked, admin_lock_and_flush_memory, admin_login,
    admin_login_google, admin_logout, admin_register, admin_unlock, admin_unlock_with_biometric,
    admin_update_profile, google_sign_in_available,
};
use commands::subscription::{open_billing_portal, subscription_status};
use commands::telemetry::{
    admin_decrypt_records, admin_simulate_and_decrypt, fetch_and_decrypt_telemetry,
};
use commands::vault::{
    vault_create_initial_key, vault_delete_server_backup, vault_export_tkb, vault_generate_new_key,
    vault_import_backup, vault_recover_from_server, vault_setup_server_backup, vault_status,
};
use commands::workers::{
    workers_create, workers_delete, workers_get, workers_get_consent_records, workers_list,
    workers_regenerate_token, workers_set_status, workers_update, workers_update_consent_config,
};
use std::sync::Arc;
use tauri::Emitter;

/// Forwards client events to the WebView (`session-expired`, `subscription-blocked`).
struct TauriEvents(tauri::AppHandle);

impl ClientEvents for TauriEvents {
    fn session_expired(&self, reason: &str) {
        if let Err(e) = self
            .0
            .emit("session-expired", serde_json::json!({ "reason": reason }))
        {
            tracing::warn!("session-expired event not delivered: {}", e);
        }
    }

    fn subscription_blocked(&self, block: &SubscriptionBlock) {
        if let Err(e) = self.0.emit("subscription-blocked", block.clone()) {
            tracing::warn!("subscription-blocked event not delivered: {}", e);
        }
    }
}

fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    tracing::info!("Telemetry Admin Desktop Core initialized");

    let result = tauri::Builder::default()
        .setup(|app| {
            get_api_client().set_event_sink(Arc::new(TauriEvents(app.handle().clone())));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            admin_login,
            admin_register,
            admin_login_google,
            google_sign_in_available,
            admin_logout,
            admin_get_session,
            admin_update_profile,
            admin_lock_and_flush_memory,
            admin_unlock,
            admin_unlock_with_biometric,
            admin_is_locked,
            admin_get_public_key,
            keys_status,
            keys_register_on_server,
            vault_status,
            vault_create_initial_key,
            vault_export_tkb,
            vault_setup_server_backup,
            vault_delete_server_backup,
            vault_import_backup,
            vault_recover_from_server,
            vault_generate_new_key,
            devices_list,
            devices_rename,
            devices_revoke,
            subscription_status,
            open_billing_portal,
            admin_decrypt_records,
            admin_simulate_and_decrypt,
            fetch_and_decrypt_telemetry,
            workers_list,
            workers_get,
            workers_create,
            workers_set_status,
            workers_regenerate_token,
            workers_update,
            workers_update_consent_config,
            workers_get_consent_records,
            workers_delete,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        tracing::error!("Tauri application failed: {}", e);
        std::process::exit(1);
    }
}
