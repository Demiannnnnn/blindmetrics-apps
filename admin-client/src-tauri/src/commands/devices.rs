//! Authorized admin devices (TASK-ADMIN-10, API `/admins/me/devices`).

use super::CommandError;
use crate::client::{get_api_client, AdminApiClient};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AdminDevice {
    pub id: String,
    pub device_name: String,
    pub device_fingerprint: String,
    #[serde(default)]
    pub public_key_fingerprint: Option<String>,
    pub is_active: bool,
    #[serde(default)]
    pub is_current: bool,
    #[serde(default)]
    pub last_seen_at: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub revoked_at: Option<String>,
}

const MAX_DEVICE_NAME_CHARS: usize = 255;

fn validate_device_id(id: &str) -> Result<(), CommandError> {
    uuid::Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| CommandError::validation("Identificador de dispositivo inválido"))
}

pub async fn devices_list_with_client(
    client: &AdminApiClient,
) -> Result<Vec<AdminDevice>, CommandError> {
    let devices = client
        .get_collection::<AdminDevice>("/admins/me/devices")
        .await?;
    Ok(devices.data)
}

#[tauri::command]
pub async fn devices_list() -> Result<Vec<AdminDevice>, CommandError> {
    devices_list_with_client(get_api_client()).await
}

pub async fn devices_rename_with_client(
    client: &AdminApiClient,
    id: &str,
    name: &str,
) -> Result<AdminDevice, CommandError> {
    validate_device_id(id)?;
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_DEVICE_NAME_CHARS {
        return Err(CommandError::validation(
            "El nombre del dispositivo debe tener entre 1 y 255 caracteres",
        ));
    }
    let device: AdminDevice = client
        .patch_json(
            &format!("/admins/me/devices/{id}"),
            &serde_json::json!({ "device_name": name }),
        )
        .await?;
    tracing::info!("device renamed");
    Ok(device)
}

#[tauri::command]
pub async fn devices_rename(id: String, name: String) -> Result<AdminDevice, CommandError> {
    devices_rename_with_client(get_api_client(), &id, &name).await
}

/// Revokes a device. Revoking the current one also ends the local session (RF-ADMIN-10-7).
pub async fn devices_revoke_with_client(
    client: &AdminApiClient,
    id: &str,
) -> Result<bool, CommandError> {
    validate_device_id(id)?;
    let is_current = devices_list_with_client(client)
        .await?
        .iter()
        .any(|d| d.id == id && d.is_current);
    client.delete(&format!("/admins/me/devices/{id}")).await?;
    tracing::info!(is_current, "device revoked");
    if is_current {
        client.logout();
    }
    Ok(is_current)
}

/// Returns whether the revoked device was this one (the UI then returns to the login).
#[tauri::command]
pub async fn devices_revoke(id: String) -> Result<bool, CommandError> {
    devices_revoke_with_client(get_api_client(), &id).await
}

/// Records on the server which admin key this device holds (RF-ADMIN-10-8). Best effort: a
/// failure is logged and never blocks key registration, import or recovery.
pub async fn record_key_on_current_device(client: &AdminApiClient, public_key_fingerprint: &str) {
    let devices = match devices_list_with_client(client).await {
        Ok(devices) => devices,
        Err(e) => {
            tracing::warn!(
                "device list unavailable, key fingerprint not recorded: {}",
                e
            );
            return;
        }
    };
    let Some(current) = devices.iter().find(|d| d.is_current && d.is_active) else {
        return;
    };
    if current.public_key_fingerprint.as_deref() == Some(public_key_fingerprint) {
        return;
    }
    let result: Result<AdminDevice, _> = client
        .patch_json(
            &format!("/admins/me/devices/{}", current.id),
            &serde_json::json!({ "public_key_fingerprint": public_key_fingerprint }),
        )
        .await;
    if let Err(e) = result {
        tracing::warn!("device key fingerprint not recorded: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const CURRENT: &str = "01920000-0000-7000-8000-000000000001";
    const OTHER: &str = "01920000-0000-7000-8000-000000000002";

    fn device(id: &str, current: bool, fp: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "id": id, "device_name": "MacBook de Ana", "device_fingerprint": "inst-abc",
            "public_key_fingerprint": fp, "is_active": true, "is_current": current,
            "last_seen_at": "2026-09-30T12:00:00Z", "created_at": "2026-09-01T10:00:00Z",
            "revoked_at": null
        })
    }

    async fn setup() -> (MockServer, AdminApiClient) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/admins/me/devices"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [device(CURRENT, true, None), device(OTHER, false, Some("ff"))]
            })))
            .mount(&server)
            .await;
        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("a".into()));
        (server, client)
    }

    #[tokio::test]
    async fn test_devices_list_rename_revoke() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let (server, client) = setup().await;
        Mock::given(method("PATCH"))
            .and(path(format!("/admins/me/devices/{OTHER}")))
            .and(body_json(
                serde_json::json!({ "device_name": "Notebook oficina" }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": device(OTHER, false, Some("ff"))
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path(format!("/admins/me/devices/{OTHER}")))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let list = devices_list_with_client(&client).await.unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].is_current);
        devices_rename_with_client(&client, OTHER, "  Notebook oficina ")
            .await
            .unwrap();
        assert!(!devices_revoke_with_client(&client, OTHER).await.unwrap());
        assert!(
            client.get_access_token_for_test().is_some(),
            "other device: session kept"
        );

        assert!(devices_rename_with_client(&client, OTHER, "   ")
            .await
            .is_err());
        assert!(devices_revoke_with_client(&client, "not-a-uuid")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_devices_revoke_current_logs_out() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let (server, client) = setup().await;
        Mock::given(method("DELETE"))
            .and(path(format!("/admins/me/devices/{CURRENT}")))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        assert!(devices_revoke_with_client(&client, CURRENT).await.unwrap());
        assert!(client.get_access_token_for_test().is_none());
    }

    #[tokio::test]
    async fn test_devices_revoke_last_active_maps_reason() {
        let (server, client) = setup().await;
        Mock::given(method("DELETE"))
            .and(path(format!("/admins/me/devices/{CURRENT}")))
            .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                "error": { "code": "CONFLICT", "message": "last device",
                           "details": { "reason": "LAST_ACTIVE_DEVICE" } }
            })))
            .mount(&server)
            .await;
        let err = devices_revoke_with_client(&client, CURRENT)
            .await
            .unwrap_err();
        assert_eq!(err.reason.as_deref(), Some("LAST_ACTIVE_DEVICE"));
        assert!(client.get_access_token_for_test().is_some());
    }

    #[tokio::test]
    async fn test_record_key_on_current_device_patches_only_current() {
        let (server, client) = setup().await;
        Mock::given(method("PATCH"))
            .and(path(format!("/admins/me/devices/{CURRENT}")))
            .and(body_json(
                serde_json::json!({ "public_key_fingerprint": "ab".repeat(32) }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": device(CURRENT, true, Some(&"ab".repeat(32)))
            })))
            .expect(1)
            .mount(&server)
            .await;
        record_key_on_current_device(&client, &"ab".repeat(32)).await;
    }
}
