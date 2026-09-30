//! Admin device descriptor sent with register / login / Google sign-in (TASK-INT-02,
//! RF-INT-02-5; API `AdminDeviceInfo`).
//!
//! The installation identifier is random (`inst-<uuid v4 simple>`), stored in the OS keystore
//! and never derived from hardware or personal data. Deleting it makes this installation a new
//! device for the server.

use crate::keystore::{self, KeystoreError, DEVICE_ID_ACCOUNT};
use serde::Serialize;

pub const INSTALLATION_ID_PREFIX: &str = "inst-";
pub const DEFAULT_DEVICE_NAME: &str = "Telemetry Admin";
const MAX_DEVICE_NAME_CHARS: usize = 255;

/// `device` object of the API (`fingerprint` 16–512 chars of `[A-Za-z0-9._:-]`, `name` 1–255).
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AdminDeviceDescriptor {
    pub fingerprint: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key_fingerprint: Option<String>,
}

/// `inst-` followed by 32 lowercase hex digits.
pub fn is_valid_installation_id(value: &str) -> bool {
    value
        .strip_prefix(INSTALLATION_ID_PREFIX)
        .is_some_and(|hex| {
            hex.len() == 32
                && hex
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        })
}

pub fn new_installation_id() -> String {
    format!(
        "{}{}",
        INSTALLATION_ID_PREFIX,
        uuid::Uuid::new_v4().simple()
    )
}

/// Loads the installation id, creating it on first use. `None` when the keystore is unusable
/// (the API accepts sessions without a device; they just cannot be listed or revoked per device).
pub fn installation_id() -> Option<String> {
    match keystore::load_secret(DEVICE_ID_ACCOUNT) {
        Ok(id) if is_valid_installation_id(&id) => Some(id.to_string()),
        Ok(_) | Err(KeystoreError::NotFound) => {
            let id = new_installation_id();
            match keystore::save_secret(DEVICE_ID_ACCOUNT, &id) {
                Ok(()) => {
                    tracing::info!(
                        device = &id[..INSTALLATION_ID_PREFIX.len() + 8],
                        "New admin installation id created"
                    );
                    Some(id)
                }
                Err(e) => {
                    tracing::warn!("Installation id could not be stored: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            tracing::warn!("Installation id could not be read: {}", e);
            None
        }
    }
}

/// Removes control characters, trims and bounds the device name; empty ⇒ default.
pub fn sanitize_device_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_DEVICE_NAME_CHARS)
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        DEFAULT_DEVICE_NAME.to_string()
    } else {
        cleaned
    }
}

/// Human-readable name of this computer (hostname).
pub fn device_name() -> String {
    sanitize_device_name(&gethostname::gethostname().to_string_lossy())
}

/// Fingerprint of the admin key stored on this device, if any.
pub fn local_key_fingerprint() -> Option<String> {
    let public_key = crate::commands::keys::get_admin_public_key().ok()?;
    telemetry_crypto_core::public_key_fingerprint(&public_key).ok()
}

/// Descriptor for the current installation (`None` if the installation id is unavailable).
pub fn descriptor() -> Option<AdminDeviceDescriptor> {
    Some(AdminDeviceDescriptor {
        fingerprint: installation_id()?,
        name: device_name(),
        public_key_fingerprint: local_key_fingerprint(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_installation_id_format() {
        let id = new_installation_id();
        assert!(is_valid_installation_id(&id), "{id}");
        assert_eq!(id.len(), 37);
        assert!(!is_valid_installation_id("inst-XYZ"));
        assert!(!is_valid_installation_id(&id.to_uppercase()));
        assert!(!is_valid_installation_id(&id[1..]));
    }

    #[tokio::test]
    async fn test_device_descriptor_is_stable_and_valid() {
        let _guard = keystore::KEYSTORE_TEST_MUTEX.lock().await;
        keystore::init_test_vault();

        let first = descriptor().expect("descriptor");
        let second = descriptor().expect("descriptor");
        assert_eq!(first.fingerprint, second.fingerprint, "stable across calls");
        assert!(is_valid_installation_id(&first.fingerprint));
        assert!(first.public_key_fingerprint.is_none(), "no key yet");

        // A corrupted stored id is replaced.
        keystore::save_secret(DEVICE_ID_ACCOUNT, "garbage").unwrap();
        let replaced = descriptor().unwrap();
        assert!(is_valid_installation_id(&replaced.fingerprint));
        assert_ne!(replaced.fingerprint, first.fingerprint);

        // With a key in the keystore its fingerprint is included.
        let (private_key, public_key) = telemetry_crypto_core::generate_admin_keypair();
        keystore::save_admin_private_key(&private_key).unwrap();
        let with_key = descriptor().unwrap();
        assert_eq!(
            with_key.public_key_fingerprint,
            Some(telemetry_crypto_core::public_key_fingerprint(&public_key).unwrap())
        );
        let json = serde_json::to_value(&with_key).unwrap();
        assert!(json.get("fingerprint").is_some() && json.get("name").is_some());
    }

    #[test]
    fn test_sanitize_device_name() {
        assert_eq!(
            sanitize_device_name("  MacBook-de-Ana \n"),
            "MacBook-de-Ana"
        );
        assert_eq!(
            sanitize_device_name("\u{0007}\u{0000}"),
            DEFAULT_DEVICE_NAME
        );
        assert_eq!(sanitize_device_name(&"x".repeat(300)).chars().count(), 255);
        assert!(!device_name().is_empty());
    }
}
