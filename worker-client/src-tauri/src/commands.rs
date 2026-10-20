use crate::client::{self, DataDeletionDto, DataRequestDto, MarksPageDto, ServiceStatusDto};
use crate::error::CommandError;
use agent_ipc::{
    default_runtime_dir, AppSnapshot, Correction, DaySummary, DiagnosticsInfo, LegalDocument,
    MarkRecord, MarkType, SecretPin, TodayView, VerificationResult, WindowScreenHint,
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

#[tauri::command]
pub async fn legal_documents() -> Result<Vec<LegalDocument>, CommandError> {
    client::legal_documents(&default_runtime_dir()).await
}

#[tauri::command]
pub async fn accept_terms(
    version: String,
    hash: String,
    privacy_version: String,
    privacy_hash: String,
) -> Result<String, CommandError> {
    client::accept_terms(
        &default_runtime_dir(),
        version,
        hash,
        privacy_version,
        privacy_hash,
    )
    .await
}

/// The PIN is deserialized straight into a zeroizing [`SecretPin`] (SEC-19).
#[tauri::command]
pub async fn set_pin(pin: SecretPin) -> Result<(), CommandError> {
    client::set_pin(&default_runtime_dir(), pin).await
}

#[tauri::command]
pub async fn change_pin(current: SecretPin, new: SecretPin) -> Result<(), CommandError> {
    client::change_pin(&default_runtime_dir(), current, new).await
}

#[tauri::command]
pub async fn request_pin_reset() -> Result<(), CommandError> {
    client::request_pin_reset(&default_runtime_dir()).await
}

#[tauri::command]
pub async fn verify_pin_reset(code: String) -> Result<u64, CommandError> {
    client::verify_pin_reset(&default_runtime_dir(), code).await
}

/// Only path from the window to a mark: explicit type + PIN typed in the dialog (ATT-01).
#[tauri::command]
pub async fn mark_attendance(
    mark_type: MarkType,
    pin: SecretPin,
) -> Result<MarkRecord, CommandError> {
    client::mark(&default_runtime_dir(), mark_type, pin).await
}

#[tauri::command]
pub async fn attendance_today() -> Result<TodayView, CommandError> {
    client::today(&default_runtime_dir()).await
}

#[tauri::command]
pub async fn attendance_days(from: String, to: String) -> Result<Vec<DaySummary>, CommandError> {
    client::days(&default_runtime_dir(), from, to).await
}

#[tauri::command]
pub async fn attendance_marks(
    from: String,
    to: String,
    page: Option<u32>,
) -> Result<MarksPageDto, CommandError> {
    client::marks(&default_runtime_dir(), from, to, page).await
}

#[tauri::command]
pub async fn list_corrections() -> Result<Vec<Correction>, CommandError> {
    client::corrections(&default_runtime_dir()).await
}

#[tauri::command]
pub async fn respond_correction(id: String, accept: bool) -> Result<Correction, CommandError> {
    client::respond_correction(&default_runtime_dir(), id, accept).await
}

/// Decodes an enrollment QR from image bytes without blocking the main async thread (PERF-06).
#[tauri::command]
pub async fn decode_enrollment_qr_image(image_bytes: Vec<u8>) -> Result<String, CommandError> {
    tokio::task::spawn_blocking(move || crate::qr::decode_qr_from_image_bytes(&image_bytes))
        .await
        .map_err(|e| CommandError::new("TASK_ERROR", format!("Fallo en tarea asíncrona: {}", e)))?
}
