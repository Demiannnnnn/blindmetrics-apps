use crate::credentials::CredentialStore;
use crate::device::get_device_identifier;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsentSettings {
    pub system_activity: bool,
    pub network_activity: bool,
    pub productivity_basic: bool,
    pub productivity_screenshots: bool,
    pub file_activity: bool,
    pub location: bool,
}

impl Default for ConsentSettings {
    /// DP-08: Default consent settings are ALL FALSE.
    /// No categories are enabled by default.
    fn default() -> Self {
        Self {
            system_activity: false,
            network_activity: false,
            productivity_basic: false,
            productivity_screenshots: false,
            file_activity: false,
            location: false,
        }
    }
}

/// Server the agent is built for, fixed at compile time by `build.rs` from `TELEMETRY_API_URL`
/// (`agent/.env` or the environment). The worker never types or changes it.
pub const API_BASE_URL: &str = env!("TELEMETRY_API_URL");

fn compiled_api_base_url() -> String {
    API_BASE_URL.to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Always `API_BASE_URL` in the app. Never read from or written to disk (an old config file's
    /// value is ignored); tests point it at a local mock server.
    #[serde(skip, default = "compiled_api_base_url")]
    pub api_base_url: String,
    pub device_identifier: String,
    pub flush_interval_secs: u64,
    #[serde(default)]
    pub consents: ConsentSettings,
    #[serde(default)]
    pub corporate_watched_dir: Option<PathBuf>,

    // Sensitive and server-managed fields: deserialized on read for backward-compat migration,
    // but never serialized back out to config.json (SEC-05).
    #[serde(default, skip_serializing)]
    pub worker_id: Option<String>,
    #[serde(default, skip_serializing)]
    pub admin_public_key: Option<String>,
    #[serde(default, skip_serializing)]
    pub access_token: Option<String>,
    #[serde(default, skip_serializing)]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing)]
    pub worker_role: Option<String>,
    #[serde(default, skip_serializing)]
    pub work_hours_start: Option<String>,
    #[serde(default, skip_serializing)]
    pub work_hours_end: Option<String>,
    #[serde(default, skip_serializing)]
    pub timezone: Option<String>,
}

impl WorkerConfig {
    pub fn corporate_watched_dir(&self) -> PathBuf {
        if let Some(ref dir) = self.corporate_watched_dir {
            dir.clone()
        } else {
            dirs::document_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Corporate")
        }
    }

    pub fn config_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("telemetry-worker");
        std::fs::create_dir_all(&path).ok();
        path.push("config.json");
        path
    }

    pub fn load_or_init() -> Self {
        Self::load_or_init_at(Self::config_path())
    }

    pub fn load_or_init_at<P: AsRef<std::path::Path>>(path: P) -> Self {
        let path = path.as_ref();
        let _ = dotenvy::dotenv();
        let (mut cfg, is_new) = if let Ok(data) = std::fs::read_to_string(path) {
            if let Ok(loaded) = serde_json::from_str::<Self>(&data) {
                // Check if old sensitive fields were present on disk
                let had_sensitive_data = loaded.admin_public_key.is_some()
                    || loaded.access_token.is_some()
                    || loaded.refresh_token.is_some()
                    || loaded.worker_id.is_some();
                if had_sensitive_data {
                    let keystore = crate::credentials::KeyringCredentialStore::new();
                    if loaded.admin_public_key.is_some()
                        && keystore.get_pinned_admin_public_key().is_none()
                    {
                        let _ = keystore
                            .set_pinned_admin_public_key(loaded.admin_public_key.as_deref());
                    }
                    if loaded.access_token.is_some() && keystore.get_access_token().is_none() {
                        let _ = keystore.set_access_token(loaded.access_token.as_deref());
                    }
                    if loaded.refresh_token.is_some() && keystore.get_refresh_token().is_none() {
                        let _ = keystore.set_refresh_token(loaded.refresh_token.as_deref());
                    }
                    if loaded.worker_id.is_some() && keystore.get_worker_id().is_none() {
                        let _ = keystore.set_worker_id(loaded.worker_id.as_deref());
                    }
                    // Re-save without sensitive fields to remove them from disk
                    let _ = loaded.save_to(path);
                }
                (loaded, false)
            } else {
                (Self::default_config(), true)
            }
        } else {
            (Self::default_config(), true)
        };

        if is_new {
            if let Ok(env_flush) = std::env::var("WORKER_FLUSH_INTERVAL") {
                if let Ok(secs) = env_flush.trim().parse::<u64>() {
                    if secs >= 5 {
                        cfg.flush_interval_secs = secs;
                    }
                }
            }

            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = cfg.save_to(path);
        }

        cfg
    }

    fn default_config() -> Self {
        Self {
            api_base_url: compiled_api_base_url(),
            device_identifier: get_device_identifier(),
            flush_interval_secs: 30,
            consents: ConsentSettings::default(),
            corporate_watched_dir: None,
            worker_id: None,

            admin_public_key: None,
            access_token: None,
            refresh_token: None,
            worker_role: None,
            work_hours_start: None,
            work_hours_end: None,
            timezone: None,
        }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        self.save_to(Self::config_path())
    }

    pub fn save_to<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_consent_is_all_false() {
        let consents = ConsentSettings::default();
        assert!(!consents.system_activity);
        assert!(!consents.network_activity);
        assert!(!consents.productivity_basic);
        assert!(!consents.productivity_screenshots);
        assert!(!consents.file_activity);
        assert!(!consents.location);
    }

    #[test]
    fn test_config_serialization_roundtrip_never_writes_credentials() {
        let config = WorkerConfig {
            api_base_url: "https://test.telemetry.com/api/v1".into(),
            device_identifier: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .into(),
            flush_interval_secs: 60,
            consents: ConsentSettings::default(),
            corporate_watched_dir: None,
            worker_id: Some("worker-123".into()),

            admin_public_key: Some("dummy-b64-key".into()),
            access_token: Some("secret_access".into()),
            refresh_token: Some("secret_refresh".into()),
            worker_role: Some("FIELD".into()),
            work_hours_start: Some("08:30".into()),
            work_hours_end: Some("17:30".into()),
            timezone: Some("America/Santiago".into()),
        };

        let json = serde_json::to_string(&config).expect("Serialize failed");
        assert!(!json.contains("worker-123"));
        assert!(!json.contains("dummy-b64-key"));
        assert!(!json.contains("secret_access"));
        assert!(!json.contains("secret_refresh"));

        // The server URL is compiled in: never persisted, and ignored if an old file has one
        assert!(!json.contains("api_base_url"));
        let deserialized: WorkerConfig = serde_json::from_str(&json).expect("Deserialize failed");
        assert_eq!(deserialized.api_base_url, API_BASE_URL);
        let legacy: WorkerConfig = serde_json::from_str(&json.replacen(
            '{',
            r#"{"api_base_url":"https://evil.example/api/v1","#,
            1,
        ))
        .expect("Deserialize legacy failed");
        assert_eq!(legacy.api_base_url, API_BASE_URL);
        assert_eq!(config.device_identifier, deserialized.device_identifier);
        assert_eq!(config.flush_interval_secs, deserialized.flush_interval_secs);
        assert_eq!(config.consents, deserialized.consents);
    }

    #[test]
    fn test_config_load_uses_injected_path() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let config_path = temp_dir.path().join("injected_config.json");

        assert!(!config_path.exists());
        let mut config = WorkerConfig::load_or_init_at(&config_path);
        assert!(config_path.exists());

        config.flush_interval_secs = 45;
        config
            .save_to(&config_path)
            .expect("Failed to save to injected path");

        let reloaded = WorkerConfig::load_or_init_at(&config_path);
        assert_eq!(reloaded.flush_interval_secs, 45);
    }
}
