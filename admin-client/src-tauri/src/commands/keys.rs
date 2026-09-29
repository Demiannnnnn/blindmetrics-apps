use super::CommandError;
use crate::client::get_api_client;
use crate::keystore::{load_admin_private_key, save_admin_private_key};
use serde::{Deserialize, Serialize};
use telemetry_crypto_core::enrollment::public_key_fingerprint;
use telemetry_crypto_core::{
    derive_public_key, export_encrypted_admin_key, generate_admin_keypair,
    import_encrypted_admin_key,
};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct KeyStatus {
    pub has_local_key: bool,
    pub local_public_key: Option<String>,
    pub local_fingerprint: Option<String>,
    pub server_fingerprint: Option<String>,
    pub is_registered_on_server: bool,
    pub matches_server: bool,
}

/// Generates a new X25519 keypair and securely saves the private key in the OS Keystore (SEC-04).
/// Returns the Base64 public key.
pub fn generate_and_save_master_keys() -> Result<String, CommandError> {
    let (priv_b64, pub_b64) = generate_admin_keypair();
    save_admin_private_key(&priv_b64).map_err(|e| CommandError {
        code: "KEYSTORE_ERROR".into(),
        message: format!("Failed to save admin key in OS keystore: {}", e),
    })?;
    Ok(pub_b64)
}

/// Loads the admin public key derived from the private key in the OS Keystore.
pub fn get_admin_public_key() -> Result<String, CommandError> {
    let priv_b64 = load_admin_private_key().map_err(|e| CommandError {
        code: "KEYSTORE_ERROR".into(),
        message: format!("Failed to load admin key from OS keystore: {}", e),
    })?;
    derive_public_key(&priv_b64).map_err(|e| CommandError {
        code: "CRYPTO_ERROR".into(),
        message: format!("Crypto error: {}", e),
    })
}

#[tauri::command]
pub fn admin_generate_keypair() -> Result<String, CommandError> {
    generate_and_save_master_keys()
}

#[tauri::command]
pub fn admin_get_public_key() -> Result<String, CommandError> {
    get_admin_public_key()
}

#[tauri::command]
pub fn admin_export_backup(passphrase: String) -> Result<Vec<u8>, CommandError> {
    let priv_b64 = load_admin_private_key().map_err(|e| CommandError {
        code: "KEYSTORE_ERROR".into(),
        message: format!("Failed to load admin key from OS keystore: {}", e),
    })?;

    let backup = export_encrypted_admin_key(&priv_b64, &passphrase).map_err(|e| CommandError {
        code: "CRYPTO_ERROR".into(),
        message: format!("Failed to export encrypted backup: {}", e),
    })?;

    tracing::info!("Admin master key backup successfully exported with passphrase (RF-ADMIN-01-3)");
    Ok(backup)
}

#[tauri::command]
pub fn admin_import_backup(backup_data: Vec<u8>, passphrase: String) -> Result<(), CommandError> {
    let priv_b64 =
        import_encrypted_admin_key(&backup_data, &passphrase).map_err(|e| CommandError {
            code: "CRYPTO_ERROR".into(),
            message: format!("Failed to decrypt and import backup: {}", e),
        })?;

    save_admin_private_key(&priv_b64).map_err(|e| CommandError {
        code: "KEYSTORE_ERROR".into(),
        message: format!("Failed to save imported admin key to OS keystore: {}", e),
    })?;

    tracing::info!(
        "Admin master key backup successfully imported into OS keystore (RF-ADMIN-01-3)"
    );
    Ok(())
}

pub async fn keys_register_on_server_with_client(
    client: &crate::client::AdminApiClient,
) -> Result<String, CommandError> {
    let pub_key = get_admin_public_key()?;
    let resp = client
        .register_public_key(&pub_key)
        .await
        .map_err(|e| CommandError {
            code: "REGISTRATION_FAILED".into(),
            message: e,
        })?;

    client.set_current_user_public_key(resp.public_key.clone(), resp.public_key_fingerprint);
    tracing::info!("Registered admin master public key on server (RF-ADMIN-08-1)");
    Ok(resp.public_key)
}

#[tauri::command]
pub async fn keys_register_on_server() -> Result<String, CommandError> {
    keys_register_on_server_with_client(get_api_client()).await
}

#[tauri::command]
pub async fn keys_status() -> Result<KeyStatus, CommandError> {
    let local_pub = get_admin_public_key().ok();
    let local_fp = local_pub
        .as_deref()
        .and_then(|k| public_key_fingerprint(k).ok());

    let client = get_api_client();
    let me = client.get_admin_me().await.ok();

    let server_fp = me.as_ref().and_then(|m| m.public_key_fingerprint.clone());
    let is_registered = server_fp.is_some();

    let matches_server = match (&local_fp, &server_fp) {
        (Some(loc), Some(srv)) => loc == srv,
        _ => true, // Not registered yet or no local key
    };

    Ok(KeyStatus {
        has_local_key: local_pub.is_some(),
        local_public_key: local_pub,
        local_fingerprint: local_fp,
        server_fingerprint: server_fp,
        is_registered_on_server: is_registered,
        matches_server,
    })
}

/// Checks key registration and registers local key if missing on the server (RF-ADMIN-08-1).
pub async fn check_and_register_key_if_missing_with_client(
    client: &crate::client::AdminApiClient,
) -> Result<(), CommandError> {
    if let Ok(me) = client.get_admin_me().await {
        if me.public_key.is_none() {
            if get_admin_public_key().is_ok() {
                let _ = keys_register_on_server_with_client(client).await?;
                tracing::info!("Automatically registered local admin public key upon login");
            }
        } else if let Ok(local_pub) = get_admin_public_key() {
            let local_fp = public_key_fingerprint(&local_pub)?;
            if let Some(srv_fp) = me.public_key_fingerprint {
                if local_fp != srv_fp {
                    tracing::warn!("Server public key differs from local key (K2 mismatch)");
                    return Err(CommandError {
                        code: "PUBLIC_KEY_MISMATCH".into(),
                        message: "Server master public key does not match local key. Worker creation blocked."
                            .into(),
                    });
                }
            }
        }
    }
    Ok(())
}

pub async fn check_and_register_key_if_missing() -> Result<(), CommandError> {
    check_and_register_key_if_missing_with_client(get_api_client()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::AdminApiClient;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_register_public_key_on_login_when_missing() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();

        let server = MockServer::start().await;
        let local_pub = generate_and_save_master_keys().unwrap();
        let local_fp = public_key_fingerprint(&local_pub).unwrap();

        // 1. Mock GET /admins/me returning no public key yet
        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "admin-123",
                    "email": "admin@acme.cl",
                    "name": "Admin",
                    "organization": "Acme",
                    "is_active": true,
                    "worker_count": 0,
                    "public_key": null,
                    "public_key_fingerprint": null,
                    "created_at": "2026-09-28T00:00:00Z",
                    "updated_at": "2026-09-28T00:00:00Z"
                }
            })))
            .mount(&server)
            .await;

        // 2. Mock PUT /admins/me/public-key registering our key
        Mock::given(method("PUT"))
            .and(path("/admins/me/public-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "public_key": local_pub,
                    "public_key_fingerprint": local_fp
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        let res = check_and_register_key_if_missing_with_client(&client).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_admin_export_and_import_backup_roundtrip() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();

        // 1. Generate master keys
        let original_pub = generate_and_save_master_keys().expect("Generate keys failed");
        let passphrase = "safe-passphrase-with-enough-entropy-2026";

        // 2. Export backup
        let backup_bytes =
            admin_export_backup(passphrase.to_string()).expect("Export backup failed");
        assert!(!backup_bytes.is_empty());

        // 3. Clear keystore
        crate::keystore::delete_admin_private_key().expect("Delete key failed");
        assert!(get_admin_public_key().is_err());

        // 4. Import backup
        admin_import_backup(backup_bytes, passphrase.to_string()).expect("Import backup failed");

        // 5. Restored key should derive identical public key
        let restored_pub = get_admin_public_key().expect("Restored key not found");
        assert_eq!(original_pub, restored_pub);
    }

    #[tokio::test]
    async fn test_backup_encryption_invalid_passphrase() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();

        let _ = generate_and_save_master_keys().expect("Generate keys failed");
        let passphrase = "correct-passphrase-123456";
        let wrong_passphrase = "wrong-passphrase-654321";

        let backup_bytes =
            admin_export_backup(passphrase.to_string()).expect("Export backup failed");

        let err = admin_import_backup(backup_bytes, wrong_passphrase.to_string())
            .expect_err("Should fail");
        assert_eq!(err.code, "CRYPTO_ERROR");
    }

    #[tokio::test]
    async fn test_backup_passphrase_too_short_rejected() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();

        let _ = generate_and_save_master_keys().expect("Generate keys failed");
        let short_passphrase = "short";

        let err = admin_export_backup(short_passphrase.to_string())
            .expect_err("Export short should fail");
        assert_eq!(err.code, "CRYPTO_ERROR");
        assert!(err.message.contains("at least 12 characters"));
    }
}
