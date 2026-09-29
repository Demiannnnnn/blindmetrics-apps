#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Open,
    WhatsCollected,
    WithdrawConsent,
    Quit,
}

pub struct TrayMenuState {
    pub tooltip: String,
}

impl TrayMenuState {
    pub fn new() -> Self {
        Self {
            tooltip: "Telemetry Worker — Transparencia Activa".to_string(),
        }
    }

    pub fn handle_action(&mut self, action: TrayAction) -> Option<&'static str> {
        match action {
            TrayAction::Open => Some("SHOW_WINDOW"),
            TrayAction::WhatsCollected => Some("WHATS_COLLECTED"),
            TrayAction::WithdrawConsent => Some("WITHDRAW_CONSENT"),
            TrayAction::Quit => Some("QUIT"),
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
        assert_eq!(tray.handle_action(TrayAction::Open), Some("SHOW_WINDOW"));
        assert_eq!(
            tray.handle_action(TrayAction::WhatsCollected),
            Some("WHATS_COLLECTED")
        );
        assert_eq!(
            tray.handle_action(TrayAction::WithdrawConsent),
            Some("WITHDRAW_CONSENT")
        );
        assert_eq!(tray.handle_action(TrayAction::Quit), Some("QUIT"));
    }
}
