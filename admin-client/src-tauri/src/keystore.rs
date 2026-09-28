use keyring::Entry;
use thiserror::Error;

const SERVICE_NAME: &str = "TelemetryAdminVault";
const KEY_ACCOUNT: &str = "admin_master_key_v1";

#[derive(Error, Debug)]
pub enum KeystoreError {
    #[error("Keyring failure: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("Private key not found in OS Keystore")]
    KeyNotFound,
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
