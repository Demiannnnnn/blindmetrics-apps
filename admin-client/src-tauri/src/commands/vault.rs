//! Key vault of the admin app (TASK-ADMIN-11 = TASK-APP-02).
//!
//! * The Master Password never leaves this process and Argon2id runs only here (SEC-12);
//!   the server receives the encrypted blob, the Argon2id parameters and the fingerprint.
//! * A restored key is stored only if it is the key registered on the server, and no error
//!   path leaves a local key that differs from the registered one (SEC-13).
//! * Generating a new key when one is registered is destructive and needs two confirmations
//!   validated here, not only in the UI (SEC-15).
//! * Passwords are moved into `Zeroizing` on entry; keys and KEKs are zeroized by
//!   `telemetry-crypto-core` (criterion "zeroización" of TASK-APP-02).

use super::keys::get_admin_public_key;
use super::CommandError;
use crate::client::{get_api_client, AdminApiClient};
use crate::keystore::{delete_admin_private_key, load_admin_private_key, save_admin_private_key};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use telemetry_crypto_core::{
    decrypt_private_key, encrypt_private_key, fingerprints_equal, generate_admin_keypair,
    import_any_backup, normalize_master_password, public_key_fingerprint,
    suggested_backup_file_name, Argon2Params, BackupFormat, CryptoError, KeyBackupFile,
    MAX_BACKUP_FILE_BYTES,
};
use zeroize::Zeroizing;

/// Exact phrase the admin must type to generate a new key (RF-ADMIN-11-7).
pub const NEW_KEY_CONFIRMATION_PHRASE: &str = "GENERAR NUEVA LLAVE";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KeyState {
    /// No key on this device and none registered (new account): K1.
    NoKey,
    /// The server has a key but this device does not (new device / reinstall): K3.
    MissingLocal,
    /// The local key differs from the registered one: K2.
    Mismatch,
    /// The device has a key and the server none: register it.
    LocalOnly,
    Ok,
}

/// `GET /admins/me/vault` — never the blob.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ServerBackupStatus {
    pub key_backup_enabled: bool,
    #[serde(default)]
    pub public_key_fingerprint: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub matches_registered_key: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VaultStatus {
    pub key_state: KeyState,
    pub has_local_key: bool,
    pub local_fingerprint: Option<String>,
    pub registered_fingerprint: Option<String>,
    pub server_backup: Option<ServerBackupStatus>,
    pub organization: String,
}

/// `GET /admins/me/vault/backup`.
#[derive(Deserialize, Clone)]
struct ServerVaultBackup {
    encrypted_private_key: String,
    argon2_params: Argon2Params,
    public_key_fingerprint: String,
}

/// Body of `POST /admins/me/vault/setup`: a closed struct, so a Master Password can never be
/// sent by accident (the API also rejects unknown fields).
#[derive(Serialize)]
struct VaultSetupRequest<'a> {
    encrypted_private_key: &'a str,
    public_key_fingerprint: &'a str,
    argon2_params: &'a Argon2Params,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ServerBackupConfigured {
    pub key_backup_enabled: bool,
    pub public_key_fingerprint: String,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub replaced: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExportedBackup {
    pub path: String,
    pub file_name: String,
    pub public_key_fingerprint: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct KeyInstalled {
    pub public_key_fingerprint: String,
    /// `V1_LEGACY` / `V2` for files, `SERVER` for the vault, `GENERATED` for new keys.
    pub source: String,
    /// Workers revoked by a rotation (only for `vault_generate_new_key`).
    pub revoked_workers: i64,
}

#[derive(Deserialize)]
struct RotateResponse {
    public_key_fingerprint: String,
    #[serde(default)]
    revoked_workers: i64,
}

fn short(fingerprint: &str) -> String {
    fingerprint.chars().take(16).collect()
}

/// Maps backup crypto errors without distinguishing wrong password from tampering.
fn map_backup_error(e: CryptoError) -> CommandError {
    match e {
        CryptoError::AeadError => CommandError::new(
            "WRONG_MASTER_PASSWORD",
            "Master Password incorrecta o respaldo dañado.",
        ),
        CryptoError::InsecureKdfParams(_) => CommandError::new(
            "INSECURE_BACKUP_PARAMS",
            "El respaldo usa parámetros de derivación no aceptados.",
        ),
        CryptoError::InvalidBackup(_) | CryptoError::Base64Error(_) => CommandError::new(
            "INVALID_BACKUP_FILE",
            "El archivo no es un respaldo válido (.tkb o .vault).",
        ),
        CryptoError::BackupKeyMismatch => CommandError::new(
            "BACKUP_KEY_MISMATCH",
            "El respaldo corresponde a otra llave.",
        ),
        CryptoError::InvalidPassphrase(msg) => CommandError::validation(msg),
        other => CommandError::new("CRYPTO_ERROR", other.to_string()),
    }
}

fn mismatch_error(backup_fp: &str, registered_fp: &str) -> CommandError {
    let mut err = CommandError::new(
        "BACKUP_KEY_MISMATCH",
        format!(
            "Este respaldo corresponde a la llave {}… y no a la registrada ({}…).",
            short(backup_fp),
            short(registered_fp)
        ),
    );
    err.details = Some(Box::new(serde_json::json!({
        "backup_fingerprint": short(backup_fp),
        "registered_fingerprint": short(registered_fp),
    })));
    err
}

/// New Master Password: NFC, 12+ characters, ≤ 1024 bytes, equal to its confirmation.
fn validate_new_master_password(
    password: &str,
    confirmation: Option<&str>,
) -> Result<(), CommandError> {
    let normalized = normalize_master_password(password).map_err(map_backup_error)?;
    let Some(confirmation) = confirmation else {
        return Err(CommandError::validation(
            "Confirma la Master Password escribiéndola dos veces.",
        ));
    };
    let confirmed = normalize_master_password(confirmation).map_err(|_| {
        CommandError::validation("La confirmación no coincide con la Master Password.")
    })?;
    if normalized.as_str() != confirmed.as_str() {
        return Err(CommandError::validation(
            "La confirmación no coincide con la Master Password.",
        ));
    }
    Ok(())
}

/// Runs CPU-heavy Argon2id work outside the async runtime threads.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| CommandError::new("THREAD_JOIN_ERROR", e.to_string()))?
}

pub fn classify(local_fp: Option<&str>, registered_fp: Option<&str>) -> KeyState {
    match (local_fp, registered_fp) {
        (None, None) => KeyState::NoKey,
        (None, Some(_)) => KeyState::MissingLocal,
        (Some(_), None) => KeyState::LocalOnly,
        (Some(local), Some(registered)) if fingerprints_equal(local, registered) => KeyState::Ok,
        (Some(_), Some(_)) => KeyState::Mismatch,
    }
}

fn local_fingerprint() -> Option<String> {
    get_admin_public_key()
        .ok()
        .and_then(|k| public_key_fingerprint(&k).ok())
}

pub async fn vault_status_with_client(
    client: &AdminApiClient,
) -> Result<VaultStatus, CommandError> {
    let me = client.get_admin_me().await?;
    let local_fp = local_fingerprint();
    let server_backup: ServerBackupStatus = client.get_json("/admins/me/vault").await?;
    Ok(VaultStatus {
        key_state: classify(local_fp.as_deref(), me.public_key_fingerprint.as_deref()),
        has_local_key: local_fp.is_some(),
        local_fingerprint: local_fp,
        registered_fingerprint: me.public_key_fingerprint,
        server_backup: Some(server_backup),
        organization: me.organization,
    })
}

#[tauri::command]
pub async fn vault_status() -> Result<VaultStatus, CommandError> {
    vault_status_with_client(get_api_client()).await
}

/// K1: first key of the account (RF-ADMIN-11-2). Rolls back the local key if the server
/// refuses it.
pub async fn vault_create_initial_key_with_client(
    client: &AdminApiClient,
) -> Result<KeyInstalled, CommandError> {
    let me = client.get_admin_me().await?;
    if me.public_key_fingerprint.is_some() {
        return Err(CommandError::new(
            "PUBLIC_KEY_ALREADY_REGISTERED",
            "Tu cuenta ya tiene una llave registrada: recupérala con tu respaldo.",
        ));
    }
    if local_fingerprint().is_some() {
        return Err(CommandError::new(
            "LOCAL_KEY_PRESENT",
            "Este equipo ya tiene una llave: regístrala en lugar de crear otra.",
        ));
    }
    let public_key = super::keys::generate_and_save_master_keys()?;
    let registered = match client.register_public_key(&public_key).await {
        Ok(resp) => resp,
        Err(e) => {
            if let Err(rollback) = delete_admin_private_key() {
                tracing::error!("rollback of the new local key failed: {}", rollback);
            }
            return Err(e.into());
        }
    };
    client.set_current_user_public_key(
        registered.public_key,
        registered.public_key_fingerprint.clone(),
    );
    super::devices::record_key_on_current_device(client, &registered.public_key_fingerprint).await;
    tracing::info!(fingerprint = %short(&registered.public_key_fingerprint), "initial admin key created");
    Ok(KeyInstalled {
        public_key_fingerprint: registered.public_key_fingerprint,
        source: "GENERATED".into(),
        revoked_workers: 0,
    })
}

#[tauri::command]
pub async fn vault_create_initial_key() -> Result<KeyInstalled, CommandError> {
    vault_create_initial_key_with_client(get_api_client()).await
}

async fn require_ok_state(client: &AdminApiClient) -> Result<VaultStatus, CommandError> {
    let status = vault_status_with_client(client).await?;
    if status.key_state != KeyState::Ok {
        return Err(CommandError::new(
            "KEY_NOT_READY",
            "La llave de este equipo debe ser la registrada en el servidor.",
        ));
    }
    Ok(status)
}

/// Encrypts the local key with fresh salt and nonce (Argon2id + AES-256-GCM).
async fn encrypt_local_key(
    password: Zeroizing<String>,
) -> Result<telemetry_crypto_core::VaultBlob, CommandError> {
    let private_key = load_admin_private_key()?;
    blocking(move || {
        encrypt_private_key(&private_key, &password, Argon2Params::generate_default())
            .map_err(map_backup_error)
    })
    .await
}

/// RF-ADMIN-11-4: enables or replaces the zero-knowledge backup on the server.
pub async fn vault_setup_server_backup_with_client(
    client: &AdminApiClient,
    password: Zeroizing<String>,
    confirmation: Zeroizing<String>,
) -> Result<ServerBackupConfigured, CommandError> {
    validate_new_master_password(&password, Some(&confirmation))?;
    require_ok_state(client).await?;
    let blob = encrypt_local_key(password).await?;
    let configured: ServerBackupConfigured = client
        .post_json(
            "/admins/me/vault/setup",
            &VaultSetupRequest {
                encrypted_private_key: &blob.encrypted_private_key,
                public_key_fingerprint: &blob.public_key_fingerprint,
                argon2_params: &blob.argon2_params,
            },
        )
        .await?;
    client.update_current_user(|p| p.key_backup_enabled = true);
    tracing::info!(
        replaced = configured.replaced,
        "vault server backup configured"
    );
    Ok(configured)
}

#[tauri::command]
pub async fn vault_setup_server_backup(
    master_password: String,
    confirm_password: String,
) -> Result<ServerBackupConfigured, CommandError> {
    vault_setup_server_backup_with_client(
        get_api_client(),
        Zeroizing::new(master_password),
        Zeroizing::new(confirm_password),
    )
    .await
}

pub async fn vault_delete_server_backup_with_client(
    client: &AdminApiClient,
) -> Result<(), CommandError> {
    client.delete("/admins/me/vault").await?;
    client.update_current_user(|p| p.key_backup_enabled = false);
    tracing::info!("vault server backup deleted");
    Ok(())
}

#[tauri::command]
pub async fn vault_delete_server_backup() -> Result<(), CommandError> {
    vault_delete_server_backup_with_client(get_api_client()).await
}

/// Downloads folder (fallback Documents, then home).
pub fn backup_directory() -> PathBuf {
    dirs::download_dir()
        .or_else(dirs::document_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Writes `bytes` as a new file (`create_new`, never overwrites; `-2`, `-3`… suffixes) with
/// permissions `0600` on Unix.
pub fn write_backup_file(
    dir: &Path,
    file_name: &str,
    bytes: &[u8],
) -> Result<PathBuf, CommandError> {
    use std::io::Write;
    std::fs::create_dir_all(dir).map_err(|e| {
        CommandError::new(
            "FILE_WRITE_ERROR",
            format!("No se pudo crear la carpeta: {e}"),
        )
    })?;
    let (stem, extension) = file_name.rsplit_once('.').unwrap_or((file_name, "tkb"));
    for attempt in 1..=99 {
        let candidate = if attempt == 1 {
            dir.join(file_name)
        } else {
            dir.join(format!("{stem}-{attempt}.{extension}"))
        };
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&candidate) {
            Ok(mut file) => {
                file.write_all(bytes)
                    .and_then(|_| file.sync_all())
                    .map_err(|e| {
                        CommandError::new(
                            "FILE_WRITE_ERROR",
                            format!("No se pudo escribir el respaldo: {e}"),
                        )
                    })?;
                return Ok(candidate);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(CommandError::new(
                    "FILE_WRITE_ERROR",
                    format!("No se pudo crear el archivo de respaldo: {e}"),
                ))
            }
        }
    }
    Err(CommandError::new(
        "FILE_WRITE_ERROR",
        "Hay demasiados respaldos con el mismo nombre en la carpeta de descargas.",
    ))
}

/// RF-ADMIN-11-3: exports a `.tkb` of the key stored on this device. When the device holds
/// the registered key and a server backup exists, the Master Password is verified by
/// decrypting that backup; otherwise (no backup, or K2 where the local key is not the
/// registered one, RF-ADMIN-11-8) it is a new password and must be confirmed.
pub async fn vault_export_tkb_with_client(
    client: &AdminApiClient,
    password: Zeroizing<String>,
    confirmation: Option<Zeroizing<String>>,
    directory: &Path,
) -> Result<ExportedBackup, CommandError> {
    normalize_master_password(&password).map_err(map_backup_error)?;
    let status = vault_status_with_client(client).await?;
    if !status.has_local_key {
        return Err(CommandError::new(
            "KEY_NOT_READY",
            "No hay una llave en este equipo que exportar.",
        ));
    }
    let local_fp = status.local_fingerprint.clone().unwrap_or_default();

    let verifiable = status.key_state == KeyState::Ok
        && status
            .server_backup
            .as_ref()
            .is_some_and(|b| b.key_backup_enabled);
    if verifiable {
        let remote: ServerVaultBackup = client.get_json("/admins/me/vault/backup").await?;
        let verify_password = password.clone();
        let local_fp_check = local_fp.clone();
        blocking(move || {
            decrypt_private_key(
                &remote.encrypted_private_key,
                &verify_password,
                &remote.argon2_params,
                Some(&local_fp_check),
            )
            .map(|_| ())
            .map_err(map_backup_error)
        })
        .await
        .map_err(|e| {
            tracing::warn!("vault password verification failed");
            e
        })?;
    } else {
        validate_new_master_password(&password, confirmation.as_ref().map(|c| c.as_str()))?;
    }

    let blob = encrypt_local_key(password).await?;
    let file = KeyBackupFile::from_blob(&blob, &status.organization, chrono::Utc::now());
    let bytes = file.to_json_bytes().map_err(map_backup_error)?;
    let file_name =
        suggested_backup_file_name(&status.organization, chrono::Local::now().date_naive());
    let path = write_backup_file(directory, &file_name, &bytes)?;
    let written_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or(file_name);
    tracing::info!(fingerprint = %short(&blob.public_key_fingerprint), file = %written_name, "vault backup exported");
    Ok(ExportedBackup {
        path: path.to_string_lossy().to_string(),
        file_name: written_name,
        public_key_fingerprint: blob.public_key_fingerprint,
    })
}

#[tauri::command]
pub async fn vault_export_tkb(
    master_password: String,
    confirm_password: Option<String>,
) -> Result<ExportedBackup, CommandError> {
    vault_export_tkb_with_client(
        get_api_client(),
        Zeroizing::new(master_password),
        confirm_password.map(Zeroizing::new),
        &backup_directory(),
    )
    .await
}

/// Stores a recovered key (A/B) following SEC-13: same fingerprint as the registered key (or
/// registers it when the account has none), explicit replacement of a different local key,
/// rollback when the server rejects the registration.
async fn install_recovered_key(
    client: &AdminApiClient,
    private_key: Zeroizing<String>,
    fingerprint: &str,
    registered_fp: Option<&str>,
    replace_local: bool,
    source: &str,
) -> Result<KeyInstalled, CommandError> {
    if let Some(registered) = registered_fp {
        if !fingerprints_equal(fingerprint, registered) {
            return Err(mismatch_error(fingerprint, registered));
        }
    }
    let previous = load_admin_private_key().ok();
    if let Some(previous_key) = &previous {
        let previous_fp = telemetry_crypto_core::derive_public_key(previous_key)
            .ok()
            .and_then(|k| public_key_fingerprint(&k).ok());
        let same = previous_fp
            .as_deref()
            .is_some_and(|fp| fingerprints_equal(fp, fingerprint));
        if !same && !replace_local {
            return Err(CommandError::new(
                "LOCAL_KEY_PRESENT",
                "Este equipo tiene otra llave. Expórtala primero y confirma el reemplazo.",
            ));
        }
    }
    save_admin_private_key(&private_key)?;

    if registered_fp.is_none() {
        let public_key = telemetry_crypto_core::derive_public_key(&private_key)?;
        if let Err(e) = client.register_public_key(&public_key).await {
            let rollback = match &previous {
                Some(old) => save_admin_private_key(old),
                None => delete_admin_private_key(),
            };
            if let Err(rollback) = rollback {
                tracing::error!("rollback of the restored key failed: {}", rollback);
            }
            return Err(e.into());
        }
        client.set_current_user_public_key(public_key, fingerprint.to_string());
    }
    super::devices::record_key_on_current_device(client, fingerprint).await;
    tracing::info!(source, fingerprint = %short(fingerprint), "key recovered");
    Ok(KeyInstalled {
        public_key_fingerprint: fingerprint.to_string(),
        source: source.to_string(),
        revoked_workers: 0,
    })
}

/// A: `.tkb` (v2) or legacy `.vault` (v1) file (RF-ADMIN-11-5).
pub async fn vault_import_backup_with_client(
    client: &AdminApiClient,
    file_bytes: Vec<u8>,
    password: Zeroizing<String>,
    replace_local: bool,
) -> Result<KeyInstalled, CommandError> {
    if file_bytes.is_empty() || file_bytes.len() > MAX_BACKUP_FILE_BYTES {
        return Err(CommandError::new(
            "INVALID_BACKUP_FILE",
            "El archivo no es un respaldo válido (.tkb o .vault).",
        ));
    }
    let imported =
        blocking(move || import_any_backup(&file_bytes, &password).map_err(map_backup_error))
            .await?;
    let me = client.get_admin_me().await?;
    let source = match imported.format {
        BackupFormat::V2 => "V2",
        BackupFormat::V1Legacy => "V1_LEGACY",
    };
    install_recovered_key(
        client,
        imported.private_key_b64,
        &imported.public_key_fingerprint,
        me.public_key_fingerprint.as_deref(),
        replace_local,
        source,
    )
    .await
}

#[tauri::command]
pub async fn vault_import_backup(
    file_bytes: Vec<u8>,
    master_password: String,
    replace_local: bool,
) -> Result<KeyInstalled, CommandError> {
    vault_import_backup_with_client(
        get_api_client(),
        file_bytes,
        Zeroizing::new(master_password),
        replace_local,
    )
    .await
}

/// B: recovery from the server backup with the Master Password (RF-ADMIN-11-6).
pub async fn vault_recover_from_server_with_client(
    client: &AdminApiClient,
    password: Zeroizing<String>,
    replace_local: bool,
) -> Result<KeyInstalled, CommandError> {
    normalize_master_password(&password).map_err(map_backup_error)?;
    let me = client.get_admin_me().await?;
    let Some(registered) = me.public_key_fingerprint.clone() else {
        return Err(CommandError::new(
            "ADMIN_PUBLIC_KEY_MISSING",
            "Tu cuenta no tiene una llave registrada.",
        ));
    };
    let backup: ServerVaultBackup = client.get_json("/admins/me/vault/backup").await?;
    if !fingerprints_equal(&backup.public_key_fingerprint, &registered) {
        return Err(mismatch_error(&backup.public_key_fingerprint, &registered));
    }
    let expected = registered.clone();
    let private_key = blocking(move || {
        decrypt_private_key(
            &backup.encrypted_private_key,
            &password,
            &backup.argon2_params,
            Some(&expected),
        )
        .map_err(map_backup_error)
    })
    .await?;
    install_recovered_key(
        client,
        private_key,
        &registered,
        Some(&registered),
        replace_local,
        "SERVER",
    )
    .await
}

#[tauri::command]
pub async fn vault_recover_from_server(
    master_password: String,
    replace_local: bool,
) -> Result<KeyInstalled, CommandError> {
    vault_recover_from_server_with_client(
        get_api_client(),
        Zeroizing::new(master_password),
        replace_local,
    )
    .await
}

/// C: new key with double confirmation (RF-ADMIN-11-7). Rotates the server key when one is
/// registered (revokes every worker) and restores the previous local key on any failure.
pub async fn vault_generate_new_key_with_client(
    client: &AdminApiClient,
    acknowledge_data_loss: bool,
    confirmation_phrase: &str,
) -> Result<KeyInstalled, CommandError> {
    if !acknowledge_data_loss || confirmation_phrase.trim() != NEW_KEY_CONFIRMATION_PHRASE {
        return Err(CommandError::new(
            "CONFIRMATION_REQUIRED",
            "Confirma la pérdida de datos y escribe exactamente GENERAR NUEVA LLAVE.",
        ));
    }
    let me = client.get_admin_me().await?;
    let previous = load_admin_private_key().ok();
    let (private_key, public_key) = generate_admin_keypair();
    let private_key = Zeroizing::new(private_key);
    save_admin_private_key(&private_key)?;

    let result: Result<(String, i64), CommandError> = match me.public_key_fingerprint.as_deref() {
        Some(previous_fp) => client
            .post_json::<_, RotateResponse>(
                "/admins/me/public-key/rotate",
                &serde_json::json!({
                    "public_key": public_key,
                    "previous_public_key_fingerprint": previous_fp,
                    "confirm_data_loss": true,
                }),
            )
            .await
            .map(|r| (r.public_key_fingerprint, r.revoked_workers))
            .map_err(CommandError::from),
        None => client
            .register_public_key(&public_key)
            .await
            .map(|r| (r.public_key_fingerprint, 0))
            .map_err(CommandError::from),
    };

    let (fingerprint, revoked_workers) = match result {
        Ok(ok) => ok,
        Err(e) => {
            let rollback = match &previous {
                Some(old) => save_admin_private_key(old),
                None => delete_admin_private_key(),
            };
            if let Err(rollback) = rollback {
                tracing::error!("rollback after failed rotation failed: {}", rollback);
            }
            return Err(e);
        }
    };
    client.set_current_user_public_key(public_key, fingerprint.clone());
    client.update_current_user(|p| p.key_backup_enabled = false);
    super::devices::record_key_on_current_device(client, &fingerprint).await;
    tracing::warn!(revoked_workers, fingerprint = %short(&fingerprint), "key rotated");
    Ok(KeyInstalled {
        public_key_fingerprint: fingerprint,
        source: "GENERATED".into(),
        revoked_workers,
    })
}

#[tauri::command]
pub async fn vault_generate_new_key(
    acknowledge_data_loss: bool,
    confirmation_phrase: String,
) -> Result<KeyInstalled, CommandError> {
    vault_generate_new_key_with_client(
        get_api_client(),
        acknowledge_data_loss,
        &confirmation_phrase,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::test_support::admin_me_json;
    use crate::keystore::{init_test_vault, KEYSTORE_TEST_MUTEX};
    use base64::Engine;
    use telemetry_crypto_core::{
        decrypt_telemetry_record, encrypt_telemetry_batch, MIN_ARGON2_MEMORY_KIB,
        MIN_ARGON2_TIME_COST,
    };
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const PASSWORD: &str = "correct horse battery staple";

    fn pw(value: &str) -> Zeroizing<String> {
        Zeroizing::new(value.to_string())
    }

    fn fast_params() -> Argon2Params {
        Argon2Params::with_salt(MIN_ARGON2_MEMORY_KIB, MIN_ARGON2_TIME_COST, 1, &[7u8; 16])
    }

    fn fp_of(public_key: &str) -> String {
        public_key_fingerprint(public_key).unwrap()
    }

    async fn server_with(
        registered_fp: Option<&str>,
        backup: Option<&str>,
    ) -> (MockServer, AdminApiClient) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(admin_me_json("Acme SpA", registered_fp)),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/admins/me/vault"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "key_backup_enabled": backup.is_some(),
                    "public_key_fingerprint": backup.and(registered_fp),
                    "updated_at": backup.map(|_| "2026-09-30T12:00:00Z"),
                    "matches_registered_key": backup.is_some()
                }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/admins/me/devices"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": [] })),
            )
            .mount(&server)
            .await;
        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("a".into()));
        (server, client)
    }

    fn mount_backup(blob: &telemetry_crypto_core::VaultBlob) -> Mock {
        Mock::given(method("GET"))
            .and(path("/admins/me/vault/backup"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "encrypted_private_key": blob.encrypted_private_key,
                    "argon2_params": blob.argon2_params,
                    "public_key_fingerprint": blob.public_key_fingerprint,
                    "updated_at": "2026-09-30T12:00:00Z"
                }
            })))
    }

    #[test]
    fn test_vault_status_key_states() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        assert_eq!(classify(None, None), KeyState::NoKey);
        assert_eq!(classify(None, Some(&a)), KeyState::MissingLocal);
        assert_eq!(classify(Some(&a), None), KeyState::LocalOnly);
        assert_eq!(classify(Some(&a), Some(&a.to_uppercase())), KeyState::Ok);
        assert_eq!(classify(Some(&a), Some(&b)), KeyState::Mismatch);
    }

    #[tokio::test]
    async fn test_vault_status_reports_state_and_backup() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();
        let (_s, client) = server_with(Some(&"c".repeat(64)), Some("yes")).await;
        let status = vault_status_with_client(&client).await.unwrap();
        assert_eq!(status.key_state, KeyState::MissingLocal);
        assert!(status.server_backup.unwrap().key_backup_enabled);
        assert_eq!(status.organization, "Acme SpA");
    }

    #[tokio::test]
    async fn test_vault_create_initial_key_rolls_back_on_server_error() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();
        let (server, client) = server_with(None, None).await;
        Mock::given(method("PUT"))
            .and(path("/admins/me/public-key"))
            .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                "error": { "code": "CONFLICT", "message": "registered",
                           "details": { "reason": "PUBLIC_KEY_ALREADY_REGISTERED" } }
            })))
            .mount(&server)
            .await;
        let err = vault_create_initial_key_with_client(&client)
            .await
            .unwrap_err();
        assert!(err.is("PUBLIC_KEY_ALREADY_REGISTERED"));
        assert!(
            load_admin_private_key().is_err(),
            "no orphan local key (SEC-13)"
        );
    }

    #[tokio::test]
    async fn test_vault_setup_server_backup_sends_only_blob_params_and_fingerprint() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();
        let (private_key, public_key) = generate_admin_keypair();
        save_admin_private_key(&private_key).unwrap();
        let fp = fp_of(&public_key);
        let (server, client) = server_with(Some(&fp), None).await;
        Mock::given(method("POST"))
            .and(path("/admins/me/vault/setup"))
            .and(body_partial_json(
                serde_json::json!({ "public_key_fingerprint": fp }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "key_backup_enabled": true, "public_key_fingerprint": fp,
                          "updated_at": "2026-09-30T12:00:00Z", "replaced": false }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let mismatch =
            vault_setup_server_backup_with_client(&client, pw(PASSWORD), pw("other password!"))
                .await;
        assert_eq!(mismatch.unwrap_err().code, "VALIDATION_ERROR");
        let configured = vault_setup_server_backup_with_client(&client, pw(PASSWORD), pw(PASSWORD))
            .await
            .unwrap();
        assert!(configured.key_backup_enabled);

        let requests = server.received_requests().await.unwrap();
        let setup = requests
            .iter()
            .find(|r| r.url.path() == "/admins/me/vault/setup")
            .unwrap();
        let body = String::from_utf8(setup.body.clone()).unwrap();
        assert!(
            !body.contains(PASSWORD),
            "the Master Password never leaves the device"
        );
        assert!(
            !body.contains(&private_key),
            "the private key never leaves in clear"
        );
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys.len(), 3, "{keys:?}");
        assert_eq!(json["argon2_params"]["memory_cost"], 65536);
        assert_eq!(json["argon2_params"]["algorithm"], "argon2id");
        let blob = base64::engine::general_purpose::STANDARD
            .decode(json["encrypted_private_key"].as_str().unwrap())
            .unwrap();
        assert_eq!(blob.len(), 60);
    }

    #[tokio::test]
    async fn test_vault_export_tkb_roundtrip_decrypts_telemetry() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();
        let (private_key, public_key) = generate_admin_keypair();
        save_admin_private_key(&private_key).unwrap();
        let fp = fp_of(&public_key);
        let (_server, client) = server_with(Some(&fp), None).await;
        let envelope =
            encrypt_telemetry_batch(&serde_json::json!({ "cpu": 42 }), &public_key).unwrap();
        let dir = tempfile::tempdir().unwrap();

        let missing_confirmation =
            vault_export_tkb_with_client(&client, pw(PASSWORD), None, dir.path()).await;
        assert_eq!(missing_confirmation.unwrap_err().code, "VALIDATION_ERROR");

        let exported =
            vault_export_tkb_with_client(&client, pw(PASSWORD), Some(pw(PASSWORD)), dir.path())
                .await
                .unwrap();
        assert!(exported.file_name.starts_with("telemetry_key_acme-spa_"));
        assert!(exported.file_name.ends_with(".tkb"));
        assert_eq!(exported.public_key_fingerprint, fp);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&exported.path)
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        // Lose the local key, import the file, decrypt old telemetry (TASK-APP-02 round-trip).
        delete_admin_private_key().unwrap();
        let bytes = std::fs::read(&exported.path).unwrap();
        let installed = vault_import_backup_with_client(&client, bytes, pw(PASSWORD), false)
            .await
            .unwrap();
        assert_eq!(installed.source, "V2");
        let restored = load_admin_private_key().unwrap();
        let plain: serde_json::Value = decrypt_telemetry_record(
            &restored,
            &envelope.ephemeral_public_key,
            &envelope.encrypted_payload,
        )
        .unwrap();
        assert_eq!(plain["cpu"], 42);
    }

    #[tokio::test]
    async fn test_vault_export_verifies_password_against_server_backup() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();
        let (private_key, public_key) = generate_admin_keypair();
        save_admin_private_key(&private_key).unwrap();
        let fp = fp_of(&public_key);
        let (server, client) = server_with(Some(&fp), Some("yes")).await;
        let blob = encrypt_private_key(&private_key, PASSWORD, fast_params()).unwrap();
        mount_backup(&blob).mount(&server).await;
        let dir = tempfile::tempdir().unwrap();

        let wrong =
            vault_export_tkb_with_client(&client, pw("wrong password 123"), None, dir.path())
                .await
                .unwrap_err();
        assert_eq!(wrong.code, "WRONG_MASTER_PASSWORD");
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            0,
            "nothing written"
        );

        // Verified against the server backup: no confirmation needed.
        let exported = vault_export_tkb_with_client(&client, pw(PASSWORD), None, dir.path())
            .await
            .unwrap();
        let file = KeyBackupFile::parse(&std::fs::read(&exported.path).unwrap()).unwrap();
        assert_eq!(file.decrypt(PASSWORD).unwrap().as_str(), private_key);
        assert_ne!(
            file.kdf.salt, blob.argon2_params.salt,
            "every export uses a fresh salt"
        );
    }

    #[tokio::test]
    async fn test_vault_export_allowed_for_mismatched_local_key_with_new_password() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();
        let (private_key, _) = generate_admin_keypair();
        save_admin_private_key(&private_key).unwrap();
        // Registered key is another one, and a server backup exists for it (K2).
        let (_s, client) = server_with(Some(&"9".repeat(64)), Some("yes")).await;
        let dir = tempfile::tempdir().unwrap();
        let err = vault_export_tkb_with_client(&client, pw(PASSWORD), None, dir.path())
            .await
            .unwrap_err();
        assert_eq!(
            err.code, "VALIDATION_ERROR",
            "new password needs confirmation"
        );
        let exported =
            vault_export_tkb_with_client(&client, pw(PASSWORD), Some(pw(PASSWORD)), dir.path())
                .await
                .unwrap();
        let file = KeyBackupFile::parse(&std::fs::read(&exported.path).unwrap()).unwrap();
        assert_eq!(file.decrypt(PASSWORD).unwrap().as_str(), private_key);

        init_test_vault();
        let (_s2, client) = server_with(Some(&"9".repeat(64)), None).await;
        let err =
            vault_export_tkb_with_client(&client, pw(PASSWORD), Some(pw(PASSWORD)), dir.path())
                .await
                .unwrap_err();
        assert_eq!(err.code, "KEY_NOT_READY");
    }

    #[tokio::test]
    async fn test_vault_import_legacy_v1_file() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();
        // Fixture shared with telemetry-crypto-core (TASK-CRYPTO-03 §8).
        let legacy_key = "QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl8=";
        let legacy_file = br#"{"format":"telemetry-vault-v1","iterations":100000,"salt":"EBESExQVFhcYGRobHB0eHw==","encrypted_payload":"MDEyMzQ1Njc4OTo7uwwaxxwcYTcQbq/Gh3MpJqYmU3tKMVNKpR49dU4Xwpjo2C0qTWkeOqhl+4ttxRRoB90e4PZPo57hEKMY"}"#;
        let fp = fp_of(&telemetry_crypto_core::derive_public_key(legacy_key).unwrap());
        let (_server, client) = server_with(Some(&fp), None).await;
        let installed = vault_import_backup_with_client(
            &client,
            legacy_file.to_vec(),
            pw("legacy-vault-passphrase"),
            false,
        )
        .await
        .unwrap();
        assert_eq!(installed.source, "V1_LEGACY");
        assert_eq!(load_admin_private_key().unwrap().as_str(), legacy_key);
    }

    #[tokio::test]
    async fn test_vault_import_rejects_foreign_key() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();
        let (private_key, _) = generate_admin_keypair();
        let blob = encrypt_private_key(&private_key, PASSWORD, fast_params()).unwrap();
        let bytes = KeyBackupFile::from_blob(&blob, "Otra", chrono::Utc::now())
            .to_json_bytes()
            .unwrap();
        let (_server, client) = server_with(Some(&"d".repeat(64)), None).await;
        let err = vault_import_backup_with_client(&client, bytes, pw(PASSWORD), false)
            .await
            .unwrap_err();
        assert_eq!(err.code, "BACKUP_KEY_MISMATCH");
        assert!(load_admin_private_key().is_err());

        let err = vault_import_backup_with_client(&client, b"{}".to_vec(), pw(PASSWORD), false)
            .await
            .unwrap_err();
        assert_eq!(err.code, "INVALID_BACKUP_FILE");
    }

    #[tokio::test]
    async fn test_vault_import_requires_explicit_replacement_of_other_local_key() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();
        let (other_key, _) = generate_admin_keypair();
        save_admin_private_key(&other_key).unwrap();
        let (private_key, public_key) = generate_admin_keypair();
        let fp = fp_of(&public_key);
        let blob = encrypt_private_key(&private_key, PASSWORD, fast_params()).unwrap();
        let bytes = KeyBackupFile::from_blob(&blob, "Acme", chrono::Utc::now())
            .to_json_bytes()
            .unwrap();
        let (_server, client) = server_with(Some(&fp), None).await;
        let err = vault_import_backup_with_client(&client, bytes.clone(), pw(PASSWORD), false)
            .await
            .unwrap_err();
        assert_eq!(err.code, "LOCAL_KEY_PRESENT");
        assert_eq!(load_admin_private_key().unwrap().as_str(), other_key);
        vault_import_backup_with_client(&client, bytes, pw(PASSWORD), true)
            .await
            .unwrap();
        assert_eq!(load_admin_private_key().unwrap().as_str(), private_key);
    }

    #[tokio::test]
    async fn test_vault_recover_from_server_restores_key() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();
        let (private_key, public_key) = generate_admin_keypair();
        let fp = fp_of(&public_key);
        let (server, client) = server_with(Some(&fp), Some("yes")).await;
        let blob = encrypt_private_key(&private_key, PASSWORD, fast_params()).unwrap();
        mount_backup(&blob).mount(&server).await;

        let wrong = vault_recover_from_server_with_client(&client, pw("wrong password 123"), false)
            .await
            .unwrap_err();
        assert_eq!(wrong.code, "WRONG_MASTER_PASSWORD");
        assert!(load_admin_private_key().is_err());

        let installed = vault_recover_from_server_with_client(&client, pw(PASSWORD), false)
            .await
            .unwrap();
        assert_eq!(installed.source, "SERVER");
        assert_eq!(load_admin_private_key().unwrap().as_str(), private_key);
    }

    #[tokio::test]
    async fn test_vault_recover_without_server_backup_maps_reason() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();
        let (server, client) = server_with(Some(&"e".repeat(64)), None).await;
        Mock::given(method("GET"))
            .and(path("/admins/me/vault/backup"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "error": { "code": "RESOURCE_NOT_FOUND", "message": "none",
                           "details": { "reason": "VAULT_NOT_CONFIGURED" } }
            })))
            .mount(&server)
            .await;
        let err = vault_recover_from_server_with_client(&client, pw(PASSWORD), false)
            .await
            .unwrap_err();
        assert!(err.is("VAULT_NOT_CONFIGURED"));
    }

    #[tokio::test]
    async fn test_vault_generate_new_key_requires_double_confirmation() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();
        let (_server, client) = server_with(Some(&"f".repeat(64)), None).await;
        for (ack, phrase) in [
            (false, NEW_KEY_CONFIRMATION_PHRASE),
            (true, "generar nueva llave"),
            (true, "GENERAR NUEVA"),
            (true, ""),
        ] {
            let err = vault_generate_new_key_with_client(&client, ack, phrase)
                .await
                .unwrap_err();
            assert_eq!(err.code, "CONFIRMATION_REQUIRED");
        }
        assert!(load_admin_private_key().is_err());
    }

    #[tokio::test]
    async fn test_vault_generate_new_key_rotates_and_rolls_back_on_server_error() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();
        let (old_key, old_public) = generate_admin_keypair();
        save_admin_private_key(&old_key).unwrap();
        let old_fp = fp_of(&old_public);
        let (server, client) = server_with(Some(&old_fp), None).await;
        Mock::given(method("POST"))
            .and(path("/admins/me/public-key/rotate"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        let err = vault_generate_new_key_with_client(&client, true, NEW_KEY_CONFIRMATION_PHRASE)
            .await
            .unwrap_err();
        assert_eq!(err.code, "HTTP_503");
        assert_eq!(
            load_admin_private_key().unwrap().as_str(),
            old_key,
            "old key restored"
        );

        Mock::given(method("POST"))
            .and(path("/admins/me/public-key/rotate"))
            .and(body_partial_json(serde_json::json!({
                "previous_public_key_fingerprint": old_fp, "confirm_data_loss": true
            })))
            .respond_with(move |req: &wiremock::Request| {
                let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
                let new_fp = public_key_fingerprint(body["public_key"].as_str().unwrap()).unwrap();
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": { "public_key": body["public_key"], "public_key_fingerprint": new_fp,
                              "public_key_registered_at": "2026-09-30T12:00:00Z",
                              "revoked_workers": 7, "key_backup_enabled": false }
                }))
            })
            .expect(1)
            .mount(&server)
            .await;
        let installed = vault_generate_new_key_with_client(&client, true, " GENERAR NUEVA LLAVE ")
            .await
            .unwrap();
        assert_eq!(installed.revoked_workers, 7);
        let new_key = load_admin_private_key().unwrap();
        assert_ne!(new_key.as_str(), old_key);
        assert_eq!(
            fp_of(&telemetry_crypto_core::derive_public_key(&new_key).unwrap()),
            installed.public_key_fingerprint
        );
    }

    #[test]
    fn test_backup_file_written_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let first =
            write_backup_file(dir.path(), "telemetry_key_acme_2026-09-30.tkb", b"one").unwrap();
        let second =
            write_backup_file(dir.path(), "telemetry_key_acme_2026-09-30.tkb", b"two").unwrap();
        assert_ne!(first, second);
        assert!(second.ends_with("telemetry_key_acme_2026-09-30-2.tkb"));
        assert_eq!(std::fs::read(&first).unwrap(), b"one");
        assert_eq!(std::fs::read(&second).unwrap(), b"two");
    }
}
