//! Admin key status and registration (TASK-ADMIN-01, TASK-ADMIN-08, TASK-ADMIN-11).
//!
//! Key creation, backups, recovery and rotation live in [`super::vault`]. Nothing here
//! generates a key implicitly (SEC-13): a key appears only through an explicit vault flow.

use super::CommandError;
use crate::client::{get_api_client, AdminApiClient};
use crate::keystore::{load_admin_private_key, save_admin_private_key};
use serde::{Deserialize, Serialize};
use telemetry_crypto_core::enrollment::public_key_fingerprint;
use telemetry_crypto_core::{derive_public_key, fingerprints_equal, generate_admin_keypair};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct KeyStatus {
    pub has_local_key: bool,
    pub local_public_key: Option<String>,
    pub local_fingerprint: Option<String>,
    pub server_fingerprint: Option<String>,
    pub is_registered_on_server: bool,
    pub matches_server: bool,
}

/// Generates a new X25519 keypair and saves the private key in the OS Keystore (SEC-04).
/// Returns the Base64 public key. Callers own the rollback if a later step fails.
pub(crate) fn generate_and_save_master_keys() -> Result<String, CommandError> {
    let (priv_b64, pub_b64) = generate_admin_keypair();
    let priv_b64 = zeroize::Zeroizing::new(priv_b64);
    save_admin_private_key(&priv_b64).map_err(|e| {
        CommandError::new(
            "KEYSTORE_ERROR",
            format!("No se pudo guardar la llave en el llavero del sistema: {e}"),
        )
    })?;
    Ok(pub_b64)
}

/// Public key derived from the private key in the OS Keystore.
pub fn get_admin_public_key() -> Result<String, CommandError> {
    let priv_b64 = load_admin_private_key().map_err(|e| {
        CommandError::new(
            "KEY_SETUP_REQUIRED",
            format!("No hay llave maestra en este equipo: {e}"),
        )
    })?;
    derive_public_key(&priv_b64)
        .map_err(|e| CommandError::new("CRYPTO_ERROR", format!("Llave local inválida: {e}")))
}

#[tauri::command]
pub fn admin_get_public_key() -> Result<String, CommandError> {
    get_admin_public_key()
}

/// Registers the local public key (`PUT /admins/me/public-key`) and records it on this device.
pub async fn keys_register_on_server_with_client(
    client: &AdminApiClient,
) -> Result<String, CommandError> {
    let pub_key = get_admin_public_key()?;
    let resp = client.register_public_key(&pub_key).await?;
    client
        .set_current_user_public_key(resp.public_key.clone(), resp.public_key_fingerprint.clone());
    super::devices::record_key_on_current_device(client, &resp.public_key_fingerprint).await;
    tracing::info!("Registered admin master public key on server (RF-ADMIN-08-1)");
    Ok(resp.public_key)
}

#[tauri::command]
pub async fn keys_register_on_server() -> Result<String, CommandError> {
    keys_register_on_server_with_client(get_api_client()).await
}

pub async fn keys_status_with_client(client: &AdminApiClient) -> Result<KeyStatus, CommandError> {
    let local_pub = get_admin_public_key().ok();
    let local_fp = local_pub
        .as_deref()
        .and_then(|k| public_key_fingerprint(k).ok());
    let me = client.get_admin_me().await?;
    let server_fp = me.public_key_fingerprint.clone();
    let matches_server = match (&local_fp, &server_fp) {
        (Some(loc), Some(srv)) => fingerprints_equal(loc, srv),
        _ => false,
    };
    Ok(KeyStatus {
        has_local_key: local_pub.is_some(),
        local_public_key: local_pub,
        local_fingerprint: local_fp,
        is_registered_on_server: server_fp.is_some(),
        server_fingerprint: server_fp,
        matches_server,
    })
}

#[tauri::command]
pub async fn keys_status() -> Result<KeyStatus, CommandError> {
    keys_status_with_client(get_api_client()).await
}

/// After login: registers the local key when the server has none (RF-ADMIN-08-1). A different
/// registered key is reported (K2) and never overwritten.
pub async fn check_and_register_key_if_missing_with_client(
    client: &AdminApiClient,
) -> Result<(), CommandError> {
    let me = client.get_admin_me().await?;
    let Ok(local_pub) = get_admin_public_key() else {
        return Ok(());
    };
    match me.public_key_fingerprint {
        None => {
            keys_register_on_server_with_client(client).await?;
            tracing::info!("Automatically registered local admin public key upon login");
        }
        Some(srv_fp) if !fingerprints_equal(&srv_fp, &public_key_fingerprint(&local_pub)?) => {
            tracing::warn!("Server public key differs from local key (K2 mismatch)");
            return Err(CommandError::new(
                "PUBLIC_KEY_MISMATCH",
                "La llave registrada en el servidor es distinta a la de este equipo.",
            ));
        }
        Some(_) => {}
    }
    Ok(())
}

pub async fn check_and_register_key_if_missing() -> Result<(), CommandError> {
    check_and_register_key_if_missing_with_client(get_api_client()).await
}

/// RF-ADMIN-08-1 gate before issuing enrollments: the local key must exist and be the one
/// registered on the server; registers it if the server has none. Returns the local public
/// key (the only one ever put in a manifest, SEC-07).
pub async fn ensure_server_key_with_client(
    client: &AdminApiClient,
) -> Result<String, CommandError> {
    let local_pub = get_admin_public_key().map_err(|_| {
        CommandError::new(
            "KEY_SETUP_REQUIRED",
            "No hay llave maestra en este equipo: créala o restáurala en Bóveda antes de continuar.",
        )
    })?;
    let me = client.get_admin_me().await?;
    match me.public_key_fingerprint {
        None => {
            keys_register_on_server_with_client(client).await?;
        }
        Some(srv_fp) if !fingerprints_equal(&srv_fp, &public_key_fingerprint(&local_pub)?) => {
            tracing::warn!("Server public key differs from local key (K2 mismatch)");
            return Err(CommandError::new(
                "PUBLIC_KEY_MISMATCH",
                "La llave registrada en el servidor es distinta a la de este equipo.",
            ));
        }
        Some(_) => {}
    }
    Ok(local_pub)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::test_support::admin_me_json;
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

        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(admin_me_json("Acme", None)))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/admins/me/public-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "public_key": local_pub, "public_key_fingerprint": local_fp }
            })))
            .expect(1)
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
        check_and_register_key_if_missing_with_client(&client)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_ensure_server_key_without_local_key_requires_setup() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let client = AdminApiClient::new("http://127.0.0.1:9".into());
        let err = ensure_server_key_with_client(&client).await.unwrap_err();
        assert_eq!(err.code, "KEY_SETUP_REQUIRED");
    }

    #[tokio::test]
    async fn test_ensure_server_key_mismatch_blocks() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        generate_and_save_master_keys().unwrap();
        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(admin_me_json("Acme", Some(&"a".repeat(64)))),
            )
            .mount(&server)
            .await;
        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("a".into()));
        let err = ensure_server_key_with_client(&client).await.unwrap_err();
        assert_eq!(err.code, "PUBLIC_KEY_MISMATCH");
        let status = keys_status_with_client(&client).await.unwrap();
        assert!(status.has_local_key && status.is_registered_on_server && !status.matches_server);
    }
}
