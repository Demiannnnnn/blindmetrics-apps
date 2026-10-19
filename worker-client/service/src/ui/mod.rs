pub mod tray;

pub use tray::{TrayAction, TrayMenuState};

use crate::config::WorkerConfig;

#[derive(Clone, Debug)]
pub struct WorkerUIState {
    pub is_connected: bool,
    pub is_in_work_hours: bool,
    pub worker_role: String,
    pub offline_items_count: usize,
    pub status_message: Option<String>,
}

impl WorkerUIState {
    pub fn new(config: &WorkerConfig) -> Self {
        Self {
            is_connected: true,
            is_in_work_hours: true,
            worker_role: config
                .worker_role
                .clone()
                .unwrap_or_else(|| "OFFICE".to_string()),
            offline_items_count: 0,
            status_message: None,
        }
    }

    /// Renders the visual traffic light indicator for the UI header:
    /// 🟢 Active | 🟡 Paused / Off-hours | 🔴 Disconnected
    pub fn get_traffic_light(&self) -> (&'static str, &'static str) {
        if !self.is_connected {
            ("🔴", "Desconectado")
        } else if !self.is_in_work_hours {
            ("🟡", "Fuera de Horario (GPS Pausado)")
        } else {
            ("🟢", "Monitoreo Activo y Protegido")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_traffic_light_states() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let config_path = temp_dir.path().join("config.json");
        let config = WorkerConfig::load_or_init_at(&config_path);
        let mut state = WorkerUIState::new(&config);

        assert_eq!(state.get_traffic_light().0, "🟢");

        state.is_in_work_hours = false;
        assert_eq!(state.get_traffic_light().0, "🟡");

        state.is_connected = false;
        assert_eq!(state.get_traffic_light().0, "🔴");
    }

    #[test]
    fn test_consent_flags_serialization() {
        use telemetry_crypto_core::ConsentFlags;

        let flags = ConsentFlags {
            system_activity: true,
            network_activity: false,
            productivity_basic: true,
            productivity_screenshots: false,
            file_activity: true,
            location: false,
        };

        let json = serde_json::to_string(&flags).expect("Serialize flags failed");
        let parsed: ConsentFlags = serde_json::from_str(&json).expect("Deserialize flags failed");
        assert_eq!(flags, parsed);
    }

    #[test]
    fn test_office_worker_cannot_enable_gps() {
        use crate::background::worker::{should_collect, CollectGate};
        use crate::state::RunMode;
        use telemetry_crypto_core::{Category, ConsentFlags};

        let all_true = ConsentFlags {
            system_activity: true,
            network_activity: true,
            productivity_basic: true,
            productivity_screenshots: true,
            file_activity: true,
            location: true,
        };

        let mode = RunMode::Collecting(all_true);
        let gate = |role| CollectGate {
            role_type: role,
            in_work_hours: true,
            in_disconnection: false,
            on_site: true,
            location_cause_declared: true,
        };

        // For an OFFICE worker, location collection is ALWAYS false, even if requested and during work hours (DP-04)
        let can_collect_office =
            should_collect(Category::Location, &mode, &gate("OFFICE"), &all_true);
        assert!(
            !can_collect_office,
            "OFFICE worker must never collect location"
        );

        // For a FIELD worker during work hours with consent and a declared cause, it is allowed
        let can_collect_field =
            should_collect(Category::Location, &mode, &gate("FIELD"), &all_true);
        assert!(
            can_collect_field,
            "FIELD worker during work hours with consent collects location"
        );
    }
}
