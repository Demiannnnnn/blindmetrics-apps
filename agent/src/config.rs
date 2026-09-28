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
        let path = Self::config_path();
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str(&data) {
                return cfg;
            }
        }

        let default_cfg = Self {
            api_base_url: "https://telemetry.local/api/v1".to_string(),
            worker_id: None,
            admin_public_key: None,
            access_token: None,
            refresh_token: None,
            device_identifier: get_device_identifier(),
            worker_role: "OFFICE".to_string(),
            work_hours_start: "09:00".to_string(),
            work_hours_end: "18:00".to_string(),
            timezone: "America/Santiago".to_string(),
            flush_interval_secs: 120,
            consents: ConsentSettings::default(),
        };

        default_cfg.save().ok();
        default_cfg
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
