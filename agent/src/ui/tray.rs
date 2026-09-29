use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Open,
    WhatsCollected,
    WithdrawConsent,
    Quit,
}

pub fn create_tray_icon(r: u8, g: u8, b: u8) -> Result<Icon, tray_icon::BadIcon> {
    let width = 16u32;
    let height = 16u32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    let center = 7.5f32;
    let radius = 6.0f32;
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= radius {
                rgba.extend_from_slice(&[r, g, b, 255]);
            } else if dist <= radius + 1.0 {
                let alpha = ((radius + 1.0 - dist) * 255.0) as u8;
                rgba.extend_from_slice(&[r, g, b, alpha]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Icon::from_rgba(rgba, width, height)
}

pub struct SystemTrayManager {
    pub tray_icon: Option<TrayIcon>,
    pub open_id: String,
    pub whats_collected_id: String,
    pub withdraw_id: String,
    pub quit_id: String,
}

#[cfg(target_os = "macos")]
fn is_main_thread() -> bool {
    extern "C" {
        fn pthread_main_np() -> i32;
    }
    unsafe { pthread_main_np() != 0 }
}

#[cfg(not(target_os = "macos"))]
fn is_main_thread() -> bool {
    true
}

impl SystemTrayManager {
    pub fn new() -> Self {
        if !is_main_thread() {
            // muda / Cocoa requires Menu creation strictly on macOS main thread.
            // When invoked in tests or non-main threads, gracefully return a stub manager.
            return Self {
                tray_icon: None,
                open_id: "open_item".to_string(),
                whats_collected_id: "whats_collected_item".to_string(),
                withdraw_id: "withdraw_item".to_string(),
                quit_id: "quit_item".to_string(),
            };
        }

        let menu = Menu::new();
        let open_item = MenuItem::new("Abrir Panel de Telemetría", true, None);
        let whats_collected_item = MenuItem::new("Qué se recopila", true, None);
        let withdraw_item = MenuItem::new("Retirar consentimiento...", true, None);
        let quit_item = MenuItem::new("Salir", true, None);

        let open_id = open_item.id().0.clone();
        let whats_collected_id = whats_collected_item.id().0.clone();
        let withdraw_id = withdraw_item.id().0.clone();
        let quit_id = quit_item.id().0.clone();

        let _ = menu.append_items(&[
            &open_item,
            &whats_collected_item,
            &PredefinedMenuItem::separator(),
            &withdraw_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ]);

        let icon_res = create_tray_icon(46, 204, 113); // Default green
        let tray_icon = if let Ok(icon) = icon_res {
            TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip("Telemetry Worker — Transparencia Activa")
                .with_icon(icon)
                .build()
                .ok()
        } else {
            None
        };

        Self {
            tray_icon,
            open_id,
            whats_collected_id,
            withdraw_id,
            quit_id,
        }
    }

    pub fn set_icon_color(&mut self, r: u8, g: u8, b: u8) {
        if let Some(ref mut tray) = self.tray_icon {
            if let Ok(icon) = create_tray_icon(r, g, b) {
                let _ = tray.set_icon(Some(icon));
            }
        }
    }

    pub fn set_tooltip(&mut self, tooltip: &str) {
        if let Some(ref mut tray) = self.tray_icon {
            let _ = tray.set_tooltip(Some(tooltip));
        }
    }

    pub fn poll_action(&self) -> Option<TrayAction> {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = &event.id.0;
            if id == &self.open_id {
                return Some(TrayAction::Open);
            } else if id == &self.whats_collected_id {
                return Some(TrayAction::WhatsCollected);
            } else if id == &self.withdraw_id {
                return Some(TrayAction::WithdrawConsent);
            } else if id == &self.quit_id {
                return Some(TrayAction::Quit);
            }
        }

        if let Ok(TrayIconEvent::Click { .. }) = TrayIconEvent::receiver().try_recv() {
            return Some(TrayAction::Open);
        }

        None
    }
}

impl Default for SystemTrayManager {
    fn default() -> Self {
        Self::new()
    }
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

    #[test]
    fn test_create_tray_icon() {
        let icon = create_tray_icon(46, 204, 113);
        assert!(icon.is_ok());
    }

    #[test]
    fn test_system_tray_manager_initialization() {
        let manager = SystemTrayManager::new();
        assert!(!manager.open_id.is_empty());
        assert!(!manager.whats_collected_id.is_empty());
        assert!(!manager.withdraw_id.is_empty());
        assert!(!manager.quit_id.is_empty());
    }
}
