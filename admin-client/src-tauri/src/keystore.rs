//! OS keystore access (Apple Keychain / Windows Credential Manager / Linux Secret Service).
//!
//! Service `TelemetryAdminVault` (SEC-04, SEC-05):
//!
//! | Account | Value |
//! |:---|:---|
//! | `admin_master_key_v1` | Base64 `K_admin_priv` |
//! | `admin_refresh_token` | Current refresh token (single use, rotated) |
//! | `admin_device_id` | Installation identifier `inst-<32 hex>` (TASK-INT-02) |
//!
//! Tests use an in-memory vault (`init_test_vault`) guarded by `KEYSTORE_TEST_MUTEX`, never
//! the real keystore.

use keyring::Entry;
use thiserror::Error;
use zeroize::Zeroizing;

const SERVICE_NAME: &str = "TelemetryAdminVault";
const KEY_ACCOUNT: &str = "admin_master_key_v1";
const REFRESH_TOKEN_ACCOUNT: &str = "admin_refresh_token";
pub const DEVICE_ID_ACCOUNT: &str = "admin_device_id";

#[derive(Error, Debug)]
pub enum KeystoreError {
    #[error("Keyring failure: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("Private key not found in OS Keystore")]
    KeyNotFound,
    #[error("Refresh token not found in OS Keystore")]
    TokenNotFound,
    #[error("Entry not found in OS Keystore")]
    NotFound,
}

#[cfg(test)]
static TEST_VAULT: std::sync::RwLock<Option<std::collections::HashMap<String, String>>> =
    std::sync::RwLock::new(None);

#[cfg(test)]
pub static KEYSTORE_TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(test)]
pub fn init_test_vault() {
    let mut lock = TEST_VAULT.write().unwrap_or_else(|p| p.into_inner());
    *lock = Some(std::collections::HashMap::new());
}

/// Stores `value` under `account`.
pub fn save_secret(account: &str, value: &str) -> Result<(), KeystoreError> {
    #[cfg(test)]
    if let Some(ref mut map) = *TEST_VAULT.write().unwrap_or_else(|p| p.into_inner()) {
        map.insert(account.to_string(), value.to_string());
        return Ok(());
    }

    let entry = Entry::new(SERVICE_NAME, account)?;
    entry.set_password(value)?;
    Ok(())
}

/// Reads `account`; [`KeystoreError::NotFound`] when absent. The value is zeroized on drop.
pub fn load_secret(account: &str) -> Result<Zeroizing<String>, KeystoreError> {
    #[cfg(test)]
    if let Some(ref map) = *TEST_VAULT.read().unwrap_or_else(|p| p.into_inner()) {
        return match map.get(account) {
            Some(v) => Ok(Zeroizing::new(v.clone())),
            None => Err(KeystoreError::NotFound),
        };
    }

    let entry = Entry::new(SERVICE_NAME, account)?;
    match entry.get_password() {
        Ok(v) => Ok(Zeroizing::new(v)),
        Err(keyring::Error::NoEntry) => Err(KeystoreError::NotFound),
        Err(e) => Err(KeystoreError::Keyring(e)),
    }
}

/// Deletes `account` (idempotent).
pub fn delete_secret(account: &str) -> Result<(), KeystoreError> {
    #[cfg(test)]
    if let Some(ref mut map) = *TEST_VAULT.write().unwrap_or_else(|p| p.into_inner()) {
        map.remove(account);
        return Ok(());
    }

    let entry = Entry::new(SERVICE_NAME, account)?;
    match entry.delete_credential() {
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(KeystoreError::Keyring(e)),
    }
}

/// Securely saves the Base64-encoded Admin private key into the OS Keystore.
pub fn save_admin_private_key(priv_key_b64: &str) -> Result<(), KeystoreError> {
    save_secret(KEY_ACCOUNT, priv_key_b64)
}

/// Retrieves the Admin private key from the OS Keystore (zeroized on drop, SEC-03).
pub fn load_admin_private_key() -> Result<Zeroizing<String>, KeystoreError> {
    load_secret(KEY_ACCOUNT).map_err(|e| match e {
        KeystoreError::NotFound => KeystoreError::KeyNotFound,
        other => other,
    })
}

/// Deletes the Admin private key from the OS Keystore (idempotent).
pub fn delete_admin_private_key() -> Result<(), KeystoreError> {
    delete_secret(KEY_ACCOUNT)
}

/// Securely saves the admin refresh token in the OS Keystore.
pub fn save_admin_refresh_token(refresh_token: &str) -> Result<(), KeystoreError> {
    save_secret(REFRESH_TOKEN_ACCOUNT, refresh_token)
}

/// Retrieves the admin refresh token from the OS Keystore.
pub fn load_admin_refresh_token() -> Result<Zeroizing<String>, KeystoreError> {
    load_secret(REFRESH_TOKEN_ACCOUNT).map_err(|e| match e {
        KeystoreError::NotFound => KeystoreError::TokenNotFound,
        other => other,
    })
}

/// Deletes the admin refresh token from the OS Keystore (idempotent).
pub fn delete_admin_refresh_token() -> Result<(), KeystoreError> {
    delete_secret(REFRESH_TOKEN_ACCOUNT)
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
        assert_eq!(loaded.as_str(), test_key);

        // 4. Delete master key (twice: idempotent)
        delete_admin_private_key().expect("Delete admin private key failed");
        delete_admin_private_key().expect("Second delete must succeed");
        let after_delete = load_admin_private_key();
        assert!(matches!(after_delete, Err(KeystoreError::KeyNotFound)));

        // 5. Test refresh token lifecycle
        save_admin_refresh_token("refresh_tok_123").expect("Save refresh token failed");
        let tok = load_admin_refresh_token().expect("Load refresh token failed");
        assert_eq!(tok.as_str(), "refresh_tok_123");
        delete_admin_refresh_token().expect("Delete refresh token failed");
        let tok_after = load_admin_refresh_token();
        assert!(matches!(tok_after, Err(KeystoreError::TokenNotFound)));

        // 6. Generic accounts
        assert!(matches!(
            load_secret(DEVICE_ID_ACCOUNT),
            Err(KeystoreError::NotFound)
        ));
        save_secret(DEVICE_ID_ACCOUNT, "inst-abc").unwrap();
        assert_eq!(load_secret(DEVICE_ID_ACCOUNT).unwrap().as_str(), "inst-abc");
    }
}
