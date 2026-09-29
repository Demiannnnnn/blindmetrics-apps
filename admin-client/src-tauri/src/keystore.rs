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

/// Securely saves the Base64-encoded Admin private key into the OS Keystore
/// (Apple Keychain / Windows Credential Manager / Linux Secret Service).
pub fn save_admin_private_key(priv_key_b64: &str) -> Result<(), KeystoreError> {
    let entry = Entry::new(SERVICE_NAME, KEY_ACCOUNT)?;
    entry.set_password(priv_key_b64)?;
    Ok(())
}

/// Retrieves the Admin private key from the OS Keystore.
pub fn load_admin_private_key() -> Result<String, KeystoreError> {
    let entry = Entry::new(SERVICE_NAME, KEY_ACCOUNT)?;
    match entry.get_password() {
        Ok(k) => Ok(k),
        Err(keyring::Error::NoEntry) => Err(KeystoreError::KeyNotFound),
        Err(e) => Err(KeystoreError::Keyring(e)),
    }
}

/// Deletes the Admin private key from the OS Keystore (on logout or vault reset).
pub fn delete_admin_private_key() -> Result<(), KeystoreError> {
    let entry = Entry::new(SERVICE_NAME, KEY_ACCOUNT)?;
    entry.delete_credential().map_err(KeystoreError::Keyring)
}

/// Securely saves the admin refresh token in the OS Keystore.
pub fn save_admin_refresh_token(refresh_token: &str) -> Result<(), KeystoreError> {
    let entry = Entry::new(SERVICE_NAME, REFRESH_TOKEN_ACCOUNT)?;
    entry.set_password(refresh_token)?;
    Ok(())
}

/// Retrieves the admin refresh token from the OS Keystore.
pub fn load_admin_refresh_token() -> Result<String, KeystoreError> {
    let entry = Entry::new(SERVICE_NAME, REFRESH_TOKEN_ACCOUNT)?;
    match entry.get_password() {
        Ok(t) => Ok(t),
        Err(keyring::Error::NoEntry) => Err(KeystoreError::TokenNotFound),
        Err(e) => Err(KeystoreError::Keyring(e)),
    }
}

/// Deletes the admin refresh token from the OS Keystore.
pub fn delete_admin_refresh_token() -> Result<(), KeystoreError> {
    let entry = Entry::new(SERVICE_NAME, REFRESH_TOKEN_ACCOUNT)?;
    match entry.delete_credential() {
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(KeystoreError::Keyring(e)),
    }
}
