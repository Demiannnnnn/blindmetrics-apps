use std::collections::HashMap;
use std::sync::RwLock;
use telemetry_crypto_core::ConsentFlags;

pub const SERVICE_NAME: &str = "TelemetryWorker";

pub const KEY_ACCESS_TOKEN: &str = "access_token";
pub const KEY_REFRESH_TOKEN: &str = "refresh_token";
pub const KEY_PINNED_ADMIN_PUBKEY: &str = "pinned_admin_public_key";
pub const KEY_PINNED_FINGERPRINT: &str = "pinned_fingerprint";
pub const KEY_WORKER_ID: &str = "worker_id";
pub const KEY_ACCEPTED_MANIFEST_HASH: &str = "accepted_manifest_hash";
pub const KEY_ACCEPTED_FLAGS: &str = "accepted_consent_flags";

/// Abstract secure credential storage trait to ensure isolation (SEC-05).
/// Concrete implementations exist for OS Keystore and in-memory mock.
pub trait CredentialStore: Send + Sync {
    fn get_access_token(&self) -> Option<String>;
    fn set_access_token(&self, token: Option<&str>) -> Result<(), String>;

    fn get_refresh_token(&self) -> Option<String>;
    fn set_refresh_token(&self, token: Option<&str>) -> Result<(), String>;

    fn get_pinned_admin_public_key(&self) -> Option<String>;
    fn set_pinned_admin_public_key(&self, key: Option<&str>) -> Result<(), String>;

    fn get_pinned_fingerprint(&self) -> Option<String>;
    fn set_pinned_fingerprint(&self, fp: Option<&str>) -> Result<(), String>;

    fn get_worker_id(&self) -> Option<String>;
    fn set_worker_id(&self, id: Option<&str>) -> Result<(), String>;

    fn get_accepted_manifest_hash(&self) -> Option<String>;
    fn set_accepted_manifest_hash(&self, hash: Option<&str>) -> Result<(), String>;

    fn get_accepted_flags(&self) -> Option<ConsentFlags>;
    fn set_accepted_flags(&self, flags: Option<&ConsentFlags>) -> Result<(), String>;

    fn wipe(&self) -> Result<(), String> {
        self.set_access_token(None)?;
        self.set_refresh_token(None)?;
        self.set_pinned_admin_public_key(None)?;
        self.set_pinned_fingerprint(None)?;
        self.set_worker_id(None)?;
        self.set_accepted_manifest_hash(None)?;
        self.set_accepted_flags(None)?;
        Ok(())
    }
}

/// In-memory implementation of CredentialStore for testing and fallback environments.
#[derive(Default)]
pub struct InMemoryCredentialStore {
    store: RwLock<HashMap<String, String>>,
}

impl InMemoryCredentialStore {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
        }
    }

    fn get_val(&self, key: &str) -> Option<String> {
        self.store
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(key)
            .cloned()
    }

    fn set_val(&self, key: &str, val: Option<&str>) -> Result<(), String> {
        let mut lock = self.store.write().unwrap_or_else(|p| p.into_inner());
        match val {
            Some(v) => {
                lock.insert(key.to_string(), v.to_string());
            }
            None => {
                lock.remove(key);
            }
        }
        Ok(())
    }
}

impl CredentialStore for InMemoryCredentialStore {
    fn get_access_token(&self) -> Option<String> {
        self.get_val(KEY_ACCESS_TOKEN)
    }

    fn set_access_token(&self, token: Option<&str>) -> Result<(), String> {
        self.set_val(KEY_ACCESS_TOKEN, token)
    }

    fn get_refresh_token(&self) -> Option<String> {
        self.get_val(KEY_REFRESH_TOKEN)
    }

    fn set_refresh_token(&self, token: Option<&str>) -> Result<(), String> {
        self.set_val(KEY_REFRESH_TOKEN, token)
    }

    fn get_pinned_admin_public_key(&self) -> Option<String> {
        self.get_val(KEY_PINNED_ADMIN_PUBKEY)
    }

    fn set_pinned_admin_public_key(&self, key: Option<&str>) -> Result<(), String> {
        self.set_val(KEY_PINNED_ADMIN_PUBKEY, key)
    }

    fn get_pinned_fingerprint(&self) -> Option<String> {
        self.get_val(KEY_PINNED_FINGERPRINT)
    }

    fn set_pinned_fingerprint(&self, fp: Option<&str>) -> Result<(), String> {
        self.set_val(KEY_PINNED_FINGERPRINT, fp)
    }

    fn get_worker_id(&self) -> Option<String> {
        self.get_val(KEY_WORKER_ID)
    }

    fn set_worker_id(&self, id: Option<&str>) -> Result<(), String> {
        self.set_val(KEY_WORKER_ID, id)
    }

    fn get_accepted_manifest_hash(&self) -> Option<String> {
        self.get_val(KEY_ACCEPTED_MANIFEST_HASH)
    }

    fn set_accepted_manifest_hash(&self, hash: Option<&str>) -> Result<(), String> {
        self.set_val(KEY_ACCEPTED_MANIFEST_HASH, hash)
    }

    fn get_accepted_flags(&self) -> Option<ConsentFlags> {
        self.get_val(KEY_ACCEPTED_FLAGS)
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    fn set_accepted_flags(&self, flags: Option<&ConsentFlags>) -> Result<(), String> {
        let serialized = flags.map(|f| serde_json::to_string(f).unwrap_or_default());
        self.set_val(KEY_ACCEPTED_FLAGS, serialized.as_deref())
    }
}

/// OS Native Keystore implementation backed by keyring (Apple Keychain / Windows Credential Manager / Secret Service).
#[derive(Default)]
pub struct KeyringCredentialStore;

impl KeyringCredentialStore {
    pub fn new() -> Self {
        Self
    }

    fn get_entry(account: &str) -> Option<String> {
        let entry = keyring::Entry::new(SERVICE_NAME, account).ok()?;
        entry.get_password().ok()
    }

    fn set_entry(account: &str, val: Option<&str>) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE_NAME, account)
            .map_err(|e| format!("Failed to access keyring: {}", e))?;

        match val {
            Some(v) => entry
                .set_password(v)
                .map_err(|e| format!("Failed to write to keyring: {}", e)),
            None => match entry.delete_credential() {
                Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(format!("Failed to delete from keyring: {}", e)),
            },
        }
    }
}

impl CredentialStore for KeyringCredentialStore {
    fn get_access_token(&self) -> Option<String> {
        Self::get_entry(KEY_ACCESS_TOKEN)
    }

    fn set_access_token(&self, token: Option<&str>) -> Result<(), String> {
        Self::set_entry(KEY_ACCESS_TOKEN, token)
    }

    fn get_refresh_token(&self) -> Option<String> {
        Self::get_entry(KEY_REFRESH_TOKEN)
    }

    fn set_refresh_token(&self, token: Option<&str>) -> Result<(), String> {
        Self::set_entry(KEY_REFRESH_TOKEN, token)
    }

    fn get_pinned_admin_public_key(&self) -> Option<String> {
        Self::get_entry(KEY_PINNED_ADMIN_PUBKEY)
    }

    fn set_pinned_admin_public_key(&self, key: Option<&str>) -> Result<(), String> {
        Self::set_entry(KEY_PINNED_ADMIN_PUBKEY, key)
    }

    fn get_pinned_fingerprint(&self) -> Option<String> {
        Self::get_entry(KEY_PINNED_FINGERPRINT)
    }

    fn set_pinned_fingerprint(&self, fp: Option<&str>) -> Result<(), String> {
        Self::set_entry(KEY_PINNED_FINGERPRINT, fp)
    }

    fn get_worker_id(&self) -> Option<String> {
        Self::get_entry(KEY_WORKER_ID)
    }

    fn set_worker_id(&self, id: Option<&str>) -> Result<(), String> {
        Self::set_entry(KEY_WORKER_ID, id)
    }

    fn get_accepted_manifest_hash(&self) -> Option<String> {
        Self::get_entry(KEY_ACCEPTED_MANIFEST_HASH)
    }

    fn set_accepted_manifest_hash(&self, hash: Option<&str>) -> Result<(), String> {
        Self::set_entry(KEY_ACCEPTED_MANIFEST_HASH, hash)
    }

    fn get_accepted_flags(&self) -> Option<ConsentFlags> {
        Self::get_entry(KEY_ACCEPTED_FLAGS).and_then(|s| serde_json::from_str(&s).ok())
    }

    fn set_accepted_flags(&self, flags: Option<&ConsentFlags>) -> Result<(), String> {
        let serialized = flags.map(|f| serde_json::to_string(f).unwrap_or_default());
        Self::set_entry(KEY_ACCEPTED_FLAGS, serialized.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorkerConfig;

    #[test]
    fn test_credentials_never_written_to_config_json() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let config_path = temp_dir.path().join("config.json");

        let config = WorkerConfig::load_or_init_at(&config_path);

        let cred_store = InMemoryCredentialStore::new();
        cred_store
            .set_access_token(Some("secret_jwt_access_token_12345"))
            .unwrap();
        cred_store
            .set_refresh_token(Some("secret_refresh_token_67890"))
            .unwrap();
        cred_store
            .set_pinned_admin_public_key(Some("pinned_admin_key_b64"))
            .unwrap();

        // Save config
        config.save_to(&config_path).expect("Failed to save config");

        let disk_content = std::fs::read_to_string(&config_path).expect("Failed to read config");
        assert!(!disk_content.contains("secret_jwt_access_token_12345"));
        assert!(!disk_content.contains("secret_refresh_token_67890"));
        assert!(!disk_content.contains("pinned_admin_key_b64"));
    }
}
