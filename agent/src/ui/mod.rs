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
}
