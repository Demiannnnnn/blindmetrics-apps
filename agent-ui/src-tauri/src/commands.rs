use crate::client::{self, DataDeletionDto, DataRequestDto, ServiceStatusDto};
use crate::error::CommandError;
use agent_ipc::{
    default_runtime_dir, AppSnapshot, DiagnosticsInfo, VerificationResult, WindowScreenHint,
};
use telemetry_crypto_core::ConsentFlags;

#[tauri::command]
pub async fn service_status() -> Result<ServiceStatusDto, CommandError> {
    let dir = default_runtime_dir();
    Ok(client::check_service_status(&dir).await)
}

#[tauri::command]
pub async fn ensure_service() -> Result<(), CommandError> {
    let dir = default_runtime_dir();
    client::ensure_service_running(&dir).await
}

#[tauri::command]
pub async fn fetch_snapshot() -> Result<AppSnapshot, CommandError> {
    let dir = default_runtime_dir();
    client::get_snapshot(&dir).await
}

#[tauri::command]
pub async fn enroll_worker(
    qr_payload: Option<String>,
    code: Option<String>,
) -> Result<VerificationResult, CommandError> {
    let dir = default_runtime_dir();
    client::enroll(&dir, qr_payload, code).await
}

#[tauri::command]
pub async fn verify_safety_code(matches: bool) -> Result<(), CommandError> {
    let dir = default_runtime_dir();
    client::confirm_safety_code(&dir, matches).await
}

#[tauri::command]
pub async fn submit_consent(accept: Option<ConsentFlags>) -> Result<(), CommandError> {
    let dir = default_runtime_dir();
    client::consent_decision(&dir, accept).await
}

#[tauri::command]
pub async fn withdraw_consent() -> Result<(), CommandError> {
    let dir = default_runtime_dir();
    client::withdraw(&dir).await
}

#[tauri::command]
pub async fn request_data(categories: Option<Vec<String>>) -> Result<DataRequestDto, CommandError> {
    let dir = default_runtime_dir();
    client::data_request(&dir, categories).await
}

#[tauri::command]
pub async fn request_deletion(
    categories: Option<Vec<String>>,
    reason: Option<String>,
) -> Result<DataDeletionDto, CommandError> {
    let dir = default_runtime_dir();
    client::data_deletion(&dir, categories, reason).await
}

#[tauri::command]
pub async fn unlink_device(confirm: String) -> Result<(), CommandError> {
    let dir = default_runtime_dir();
    client::unlink(&dir, confirm).await
}

#[tauri::command]
pub async fn get_diagnostics() -> Result<DiagnosticsInfo, CommandError> {
    let dir = default_runtime_dir();
    client::diagnostics(&dir).await
}

#[tauri::command]
pub async fn set_window_screen_hint(screen: WindowScreenHint) -> Result<(), CommandError> {
    let dir = default_runtime_dir();
    client::open_window_hint(&dir, screen).await
}

#[tauri::command]
pub async fn confirm_quit(nonce: String) -> Result<(), CommandError> {
    let dir = default_runtime_dir();
    client::confirm_quit(&dir, nonce).await
}

#[tauri::command]
pub async fn cancel_quit(nonce: String) -> Result<(), CommandError> {
    let dir = default_runtime_dir();
    client::cancel_quit(&dir, nonce).await
}
