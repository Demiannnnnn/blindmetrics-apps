//! Session commands (TASK-INT-01, TASK-INT-02, TASK-ADMIN-07, TASK-ADMIN-10).

use super::CommandError;
use crate::client::{get_api_client, AdminApiClient, AdminMeResponse, AdminProfile};
use crate::google_oauth::{self, GoogleConfig, GoogleError, SignInEndpoints};
use serde::Deserialize;

const MIN_PROFILE_FIELD_CHARS: usize = 2;
const MAX_PROFILE_FIELD_CHARS: usize = 255;

fn apply_api_url(client: &AdminApiClient, api_url: Option<String>) -> Result<(), CommandError> {
    if let Some(url) = api_url
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
    {
        let allowed =
            url.starts_with("https://") || (cfg!(debug_assertions) && url.starts_with("http://"));
        if !allowed {
            // SEC-10: release builds only talk to https:// servers.
            return Err(CommandError::validation(
                "La dirección del servidor debe comenzar con https://",
            ));
        }
        client.set_base_url(url);
    }
    Ok(())
}

/// After any sign-in: register the local key if the server has none (RF-ADMIN-08-1). A
/// failure is logged; the key screens (ADMIN-11) handle every other state.
async fn after_sign_in(client: &AdminApiClient) -> Result<AdminProfile, CommandError> {
    if let Err(e) = super::keys::check_and_register_key_if_missing_with_client(client).await {
        tracing::warn!("Key registration check after sign-in failed: {}", e);
    }
    client.get_current_user().ok_or_else(|| {
        CommandError::new(
            "NOT_AUTHENTICATED",
            "La sesión se cerró; inicia sesión nuevamente.",
        )
    })
}

#[tauri::command]
pub async fn admin_login(
    email: String,
    password: String,
    api_url: Option<String>,
) -> Result<AdminProfile, CommandError> {
    let client = get_api_client();
    apply_api_url(client, api_url)?;
    let password = zeroize::Zeroizing::new(password);
    client.login(&email, &password).await?;
    after_sign_in(client).await
}

#[tauri::command]
pub async fn admin_register(
    name: String,
    email: String,
    password: String,
    organization: String,
    api_url: Option<String>,
) -> Result<AdminProfile, CommandError> {
    let client = get_api_client();
    apply_api_url(client, api_url)?;
    let password = zeroize::Zeroizing::new(password);
    client
        .register(&name, &email, &password, &organization)
        .await?;
    after_sign_in(client).await
}

/// `true` when this build/environment has a Google desktop client id (RF-ADMIN-10-1).
#[tauri::command]
pub fn google_sign_in_available() -> bool {
    GoogleConfig::from_env().is_some()
}

/// Google sign-in through the system browser (loopback + PKCE + nonce, RF-ADMIN-10-2).
#[tauri::command]
pub async fn admin_login_google(api_url: Option<String>) -> Result<AdminProfile, CommandError> {
    let client = get_api_client();
    apply_api_url(client, api_url)?;
    let cfg = GoogleConfig::from_env().ok_or(GoogleError::NotConfigured)?;
    let id_token = google_oauth::sign_in(
        client.http(),
        &cfg,
        &SignInEndpoints::default(),
        google_oauth::open_system_browser,
    )
    .await?;
    client.login_google(&id_token).await?;
    tracing::info!("google sign-in completed");
    after_sign_in(client).await
}

/// Revokes the refresh token on the server (best effort) and clears the local session.
#[tauri::command]
pub async fn admin_logout() -> Result<(), CommandError> {
    get_api_client().logout_remote().await;
    Ok(())
}

#[tauri::command]
pub fn admin_get_session() -> Result<Option<AdminProfile>, CommandError> {
    Ok(get_api_client().get_current_user())
}

#[derive(Deserialize, Debug, Default)]
pub struct ProfileUpdate {
    pub name: Option<String>,
    pub organization: Option<String>,
}

fn validate_profile_field(label: &str, value: &str) -> Result<String, CommandError> {
    let trimmed = value.trim().to_string();
    let chars = trimmed.chars().count();
    if !(MIN_PROFILE_FIELD_CHARS..=MAX_PROFILE_FIELD_CHARS).contains(&chars) {
        return Err(CommandError::validation(format!(
            "{label} debe tener entre {MIN_PROFILE_FIELD_CHARS} y {MAX_PROFILE_FIELD_CHARS} caracteres"
        )));
    }
    Ok(trimmed)
}

/// `PUT /admins/me` (RF-ADMIN-09-5: organization missing after Google sign-up).
pub async fn admin_update_profile_with_client(
    client: &AdminApiClient,
    update: ProfileUpdate,
) -> Result<AdminProfile, CommandError> {
    let mut body = serde_json::Map::new();
    if let Some(name) = update.name.as_deref() {
        body.insert(
            "name".into(),
            validate_profile_field("El nombre", name)?.into(),
        );
    }
    if let Some(org) = update.organization.as_deref() {
        body.insert(
            "organization".into(),
            validate_profile_field("La organización", org)?.into(),
        );
    }
    if body.is_empty() {
        return Err(CommandError::validation("No hay cambios que guardar"));
    }
    let _: AdminMeResponse = client
        .put_json("/admins/me", &serde_json::Value::Object(body))
        .await?;
    Ok(client.reload_profile().await?)
}

#[tauri::command]
pub async fn admin_update_profile(update: ProfileUpdate) -> Result<AdminProfile, CommandError> {
    admin_update_profile_with_client(get_api_client(), update).await
}

#[tauri::command]
pub fn admin_lock_and_flush_memory() -> Result<(), CommandError> {
    tracing::info!("Admin lock invoked: In-memory session flushed (SEC-11)");
    get_api_client().lock_and_flush();
    Ok(())
}

#[tauri::command]
pub async fn admin_unlock(password: String) -> Result<AdminProfile, CommandError> {
    let password = zeroize::Zeroizing::new(password);
    Ok(get_api_client().unlock_with_password(&password).await?)
}

#[tauri::command]
pub async fn admin_unlock_with_biometric() -> Result<AdminProfile, CommandError> {
    Ok(get_api_client().unlock_with_refresh().await?)
}

#[tauri::command]
pub fn admin_is_locked() -> Result<bool, CommandError> {
    Ok(get_api_client().is_locked())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::test_support::{admin_me_json, auth_json};
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // logout() deletes the refresh token: hold the keystore mutex and use the
    // in-memory vault so these tests neither race others nor touch the OS keystore.
    #[tokio::test]
    async fn test_admin_session_lifecycle() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let client = get_api_client();
        client.logout();
        assert!(admin_get_session().unwrap().is_none());
        assert!(admin_logout().await.is_ok());
    }

    #[tokio::test]
    async fn test_admin_lock_and_flush() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let client = get_api_client();
        client.set_access_token_for_test(Some("sensitive-access-token-12345".to_string()));

        assert!(admin_lock_and_flush_memory().is_ok());

        // Token must be wiped from memory (zeroized)
        let token_opt = client.get_access_token_for_test();
        assert!(token_opt.is_none(), "Access token was not cleared from RAM");
    }

    #[tokio::test]
    async fn test_login_google_posts_id_token_and_device() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/auth/google"))
            .respond_with(ResponseTemplate::new(200).set_body_json(auth_json("ga", "gr", true)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(admin_me_json("", None)))
            .mount(&server)
            .await;
        let client = AdminApiClient::new(server.uri());
        let profile = client.login_google("header.payload.sig").await.unwrap();
        assert!(profile.is_new_user);
        assert!(profile.organization.is_empty());
        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["id_token"], "header.payload.sig");
        assert_eq!(body["role"], "ADMIN");
        assert!(body["device"]["fingerprint"]
            .as_str()
            .unwrap()
            .starts_with("inst-"));
    }

    #[tokio::test]
    async fn test_login_google_maps_disabled_and_mismatch() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/auth/google"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": { "code": "FORBIDDEN", "message": "disabled",
                           "details": { "reason": "GOOGLE_AUTH_DISABLED" } }
            })))
            .mount(&server)
            .await;
        let client = AdminApiClient::new(server.uri());
        let err = CommandError::from(client.login_google("a.b.c").await.unwrap_err());
        assert!(err.is("GOOGLE_AUTH_DISABLED"));
    }

    #[tokio::test]
    async fn test_admin_update_profile_sets_organization() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/admins/me"))
            .and(body_json(serde_json::json!({ "organization": "Acme SpA" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(admin_me_json("Acme SpA", None)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(admin_me_json("Acme SpA", None)))
            .mount(&server)
            .await;
        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("a".into()));
        let profile = admin_update_profile_with_client(
            &client,
            ProfileUpdate {
                name: None,
                organization: Some("  Acme SpA ".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(profile.organization, "Acme SpA");

        for bad in [
            ProfileUpdate::default(),
            ProfileUpdate {
                name: None,
                organization: Some(" A ".into()),
            },
        ] {
            let err = admin_update_profile_with_client(&client, bad)
                .await
                .unwrap_err();
            assert_eq!(err.code, "VALIDATION_ERROR");
        }
    }

    #[test]
    fn test_google_available_follows_env() {
        // Only asserts the function does not panic without configuration.
        let _ = google_sign_in_available();
    }
}
