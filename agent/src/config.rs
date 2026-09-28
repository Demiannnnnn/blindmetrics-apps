use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::device::get_device_identifier;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsentSettings {
    pub system_activity: bool,
    pub network_activity: bool,
    pub productivity_basic: bool,
    pub productivity_screenshots: bool,
    pub file_activity: bool,
    pub location: bool,
}

impl Default for ConsentSettings {
    fn default() -> Self {
        Self {
            system_activity: true,
            network_activity: true,
            productivity_basic: true,
            productivity_screenshots: false,
            file_activity: false,
            location: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub api_base_url: String,
    pub worker_id: Option<String>,
    pub admin_public_key: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub device_identifier: String,
    pub worker_role: String, // "OFFICE" or "FIELD"
    pub work_hours_start: String, // "09:00"
    pub work_hours_end: String, // "18:00"
    pub timezone: String, // "America/Santiago"
    pub flush_interval_secs: u64,
    pub consents: ConsentSettings,
}

impl WorkerConfig {
    pub fn config_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("telemetry-worker");
        std::fs::create_dir_all(&path).ok();
        path.push("config.json");
        path
    }

    pub fn load_or_init() -> Self {
        let _ = dotenvy::dotenv();
        let path = Self::config_path();
        let mut cfg = if let Ok(data) = std::fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_else(|_| Self::default_config())
        } else {
            Self::default_config()
        };

        // Override or initialize from environment variables if present
        if let Ok(env_url) = std::env::var("TELEMETRY_API_URL") {
            if !env_url.trim().is_empty() {
                cfg.api_base_url = env_url.trim().to_string();
            }
        }
        if let Ok(env_role) = std::env::var("WORKER_ROLE") {
            let role_upper = env_role.trim().to_uppercase();
            if role_upper == "FIELD" || role_upper == "OFFICE" {
                cfg.worker_role = role_upper;
            }
        }
        if let Ok(env_flush) = std::env::var("WORKER_FLUSH_INTERVAL") {
            if let Ok(secs) = env_flush.trim().parse::<u64>() {
                if secs >= 5 {
                    cfg.flush_interval_secs = secs;
                }
            }
        }

        cfg.save().ok();
        cfg
    }

    fn default_config() -> Self {
        Self {
            api_base_url: std::env::var("TELEMETRY_API_URL")
                .unwrap_or_else(|_| "http://localhost:8080/api/v1".to_string()),
            worker_id: None,
            admin_public_key: None,
            access_token: None,
            refresh_token: None,
            device_identifier: get_device_identifier(),
            worker_role: std::env::var("WORKER_ROLE").unwrap_or_else(|_| "OFFICE".to_string()),
            work_hours_start: "09:00".to_string(),
            work_hours_end: "18:00".to_string(),
            timezone: "America/Santiago".to_string(),
            flush_interval_secs: 30,
            consents: ConsentSettings::default(),
        }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(Self::config_path(), data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = WorkerConfig {
            api_base_url: "https://test.telemetry.com/api/v1".into(),
            worker_id: Some("worker-123".into()),
            admin_public_key: Some("dummy-b64-key".into()),
            access_token: None,
            refresh_token: None,
            device_identifier: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            worker_role: "FIELD".into(),
            work_hours_start: "08:30".into(),
            work_hours_end: "17:30".into(),
            timezone: "America/Santiago".into(),
            flush_interval_secs: 60,
            consents: ConsentSettings::default(),
        };

        let json = serde_json::to_string(&config).expect("Serialize failed");
        let deserialized: WorkerConfig = serde_json::from_str(&json).expect("Deserialize failed");

        assert_eq!(config.api_base_url, deserialized.api_base_url);
        assert_eq!(config.worker_role, deserialized.worker_role);
        assert_eq!(config.consents, deserialized.consents);
    }
}
