use keyring::Entry;
use thiserror::Error;

const SERVICE_NAME: &str = "TelemetryAdminVault";
const KEY_ACCOUNT: &str = "admin_master_key_v1";
const REFRESH_TOKEN_ACCOUNT: &str = "admin_refresh_token";

#[derive(Error, Debug)]
pub enum KeystoreError {
    #[error("Keyring failure: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("Private key not found in OS Keystore")]
    KeyNotFound,
    #[error("Refresh token not found in OS Keystore")]
    TokenNotFound,
}

#[cfg(test)]
static TEST_VAULT: std::sync::RwLock<Option<std::collections::HashMap<String, String>>> =
    std::sync::RwLock::new(None);

#[cfg(test)]
pub static KEYSTORE_TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(test)]
pub fn init_test_vault() {
    let mut lock = TEST_VAULT.write().unwrap();
    *lock = Some(std::collections::HashMap::new());
}

/// Securely saves the Base64-encoded Admin private key into the OS Keystore
/// (Apple Keychain / Windows Credential Manager / Linux Secret Service).
pub fn save_admin_private_key(priv_key_b64: &str) -> Result<(), KeystoreError> {
    #[cfg(test)]
    if let Some(ref mut map) = *TEST_VAULT.write().unwrap() {
        map.insert(KEY_ACCOUNT.to_string(), priv_key_b64.to_string());
        return Ok(());
    }

    let entry = Entry::new(SERVICE_NAME, KEY_ACCOUNT)?;
    entry.set_password(priv_key_b64)?;
    Ok(())
}

/// Retrieves the Admin private key from the OS Keystore.
pub fn load_admin_private_key() -> Result<String, KeystoreError> {
    #[cfg(test)]
    if let Some(ref map) = *TEST_VAULT.read().unwrap() {
        return match map.get(KEY_ACCOUNT) {
            Some(k) => Ok(k.clone()),
            None => Err(KeystoreError::KeyNotFound),
        };
    }

    let entry = Entry::new(SERVICE_NAME, KEY_ACCOUNT)?;
    match entry.get_password() {
        Ok(k) => Ok(k),
        Err(keyring::Error::NoEntry) => Err(KeystoreError::KeyNotFound),
        Err(e) => Err(KeystoreError::Keyring(e)),
    }
}

/// Deletes the Admin private key from the OS Keystore (on logout or vault reset).
pub fn delete_admin_private_key() -> Result<(), KeystoreError> {
    #[cfg(test)]
    if let Some(ref mut map) = *TEST_VAULT.write().unwrap() {
        map.remove(KEY_ACCOUNT);
        return Ok(());
    }

    let entry = Entry::new(SERVICE_NAME, KEY_ACCOUNT)?;
    entry.delete_credential().map_err(KeystoreError::Keyring)
}

/// Securely saves the admin refresh token in the OS Keystore.
pub fn save_admin_refresh_token(refresh_token: &str) -> Result<(), KeystoreError> {
    #[cfg(test)]
    if let Some(ref mut map) = *TEST_VAULT.write().unwrap() {
        map.insert(REFRESH_TOKEN_ACCOUNT.to_string(), refresh_token.to_string());
        return Ok(());
    }

    let entry = Entry::new(SERVICE_NAME, REFRESH_TOKEN_ACCOUNT)?;
    entry.set_password(refresh_token)?;
    Ok(())
}

/// Retrieves the admin refresh token from the OS Keystore.
pub fn load_admin_refresh_token() -> Result<String, KeystoreError> {
    #[cfg(test)]
    if let Some(ref map) = *TEST_VAULT.read().unwrap() {
        return match map.get(REFRESH_TOKEN_ACCOUNT) {
            Some(t) => Ok(t.clone()),
            None => Err(KeystoreError::TokenNotFound),
        };
    }

    let entry = Entry::new(SERVICE_NAME, REFRESH_TOKEN_ACCOUNT)?;
    match entry.get_password() {
        Ok(t) => Ok(t),
        Err(keyring::Error::NoEntry) => Err(KeystoreError::TokenNotFound),
        Err(e) => Err(KeystoreError::Keyring(e)),
    }
}

/// Deletes the admin refresh token from the OS Keystore.
pub fn delete_admin_refresh_token() -> Result<(), KeystoreError> {
    #[cfg(test)]
    if let Some(ref mut map) = *TEST_VAULT.write().unwrap() {
        map.remove(REFRESH_TOKEN_ACCOUNT);
        return Ok(());
    }

    let entry = Entry::new(SERVICE_NAME, REFRESH_TOKEN_ACCOUNT)?;
    match entry.delete_credential() {
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(KeystoreError::Keyring(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_keystore_mock_save_and_load() {
        let _guard = KEYSTORE_TEST_MUTEX.lock().await;
        init_test_vault();

        // 1. Initial state should be KeyNotFound
        let load_initial = load_admin_private_key();
        assert!(matches!(load_initial, Err(KeystoreError::KeyNotFound)));

        // 2. Save master key
        let test_key = "dGVzdC1wcml2YXRlLWtleS0zMi1ieXRlcy1sb25n";
        save_admin_private_key(test_key).expect("Save admin private key failed");

        // 3. Load master key
        let loaded = load_admin_private_key().expect("Load admin private key failed");
        assert_eq!(loaded, test_key);

        // 4. Delete master key
        delete_admin_private_key().expect("Delete admin private key failed");
        let after_delete = load_admin_private_key();
        assert!(matches!(after_delete, Err(KeystoreError::KeyNotFound)));

        // 5. Test refresh token lifecycle
        save_admin_refresh_token("refresh_tok_123").expect("Save refresh token failed");
        let tok = load_admin_refresh_token().expect("Load refresh token failed");
        assert_eq!(tok, "refresh_tok_123");
        delete_admin_refresh_token().expect("Delete refresh token failed");
        let tok_after = load_admin_refresh_token();
        assert!(matches!(tok_after, Err(KeystoreError::TokenNotFound)));
    }
}
