#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayAction {
    OpenConsentDashboard,
    PauseMonitoring15Min,
    RequestDataExport,
    Quit,
}

pub struct TrayMenuState {
    pub is_monitoring_paused: bool,
    pub tooltip: String,
}

impl TrayMenuState {
    pub fn new() -> Self {
        Self {
            is_monitoring_paused: false,
            tooltip: "Telemetry Worker — Monitoreo Protegido".to_string(),
        }
    }

    pub fn handle_action(&mut self, action: TrayAction) -> Option<String> {
        match action {
            TrayAction::OpenConsentDashboard => Some("SHOW_WINDOW".into()),
            TrayAction::PauseMonitoring15Min => {
                self.is_monitoring_paused = true;
                self.tooltip = "Telemetry Worker — Pausado (15 min)".to_string();
                Some("PAUSE_15M".into())
            }
            TrayAction::RequestDataExport => Some("REQUEST_EXPORT".into()),
            TrayAction::Quit => Some("QUIT".into()),
        }
    }
}

impl Default for TrayMenuState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_menu_actions() {
        let mut tray = TrayMenuState::new();
        assert!(!tray.is_monitoring_paused);

        let res = tray.handle_action(TrayAction::PauseMonitoring15Min);
        assert_eq!(res, Some("PAUSE_15M".into()));
        assert!(tray.is_monitoring_paused);
    }
}
