use super::CommandError;
use crate::client::{get_api_client, AdminProfile};

#[tauri::command]
pub async fn admin_login(
    email: String,
    password: String,
    api_url: Option<String>,
) -> Result<AdminProfile, CommandError> {
    let client = get_api_client();
    if let Some(url) = api_url {
        client.set_base_url(url);
    }
    let profile = client
        .login(&email, &password)
        .await
        .map_err(|e| CommandError {
            code: "LOGIN_FAILED".into(),
            message: e,
        })?;

    // Auto-register key if missing or check match (RF-ADMIN-08-1)
    let _ = crate::commands::keys::check_and_register_key_if_missing().await;

    Ok(profile)
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
    if let Some(url) = api_url {
        client.set_base_url(url);
    }
    client
        .register(&name, &email, &password, &organization)
        .await
        .map_err(|e| CommandError {
            code: "REGISTER_FAILED".into(),
            message: e,
        })
}

#[tauri::command]
pub fn admin_logout() -> Result<(), CommandError> {
    get_api_client().logout();
    Ok(())
}

#[tauri::command]
pub fn admin_get_session() -> Result<Option<AdminProfile>, CommandError> {
    Ok(get_api_client().get_current_user())
}

#[tauri::command]
pub fn admin_lock_and_flush_memory() -> Result<(), CommandError> {
    tracing::info!("Admin lock invoked: In-memory session flushed (SEC-11)");
    get_api_client().logout();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_session_lifecycle() {
        let client = get_api_client();
        client.logout();
        assert!(admin_get_session().unwrap().is_none());
        assert!(admin_logout().is_ok());
    }

    #[test]
    fn test_admin_lock_and_flush() {
        assert!(admin_lock_and_flush_memory().is_ok());
    }
}
