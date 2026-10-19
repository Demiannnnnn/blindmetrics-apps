use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Open,
    /// *Marcar asistencia…*: opens the window on Asistencia (RF-AGENT-12-14).
    MarkAttendance,
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

/// Menu item ids returned by muda; used to map `MenuEvent`s to `TrayAction`s.
#[derive(Clone, Debug)]
struct MenuIds {
    open: String,
    mark_attendance: String,
    whats_collected: String,
    withdraw: String,
    quit: String,
}

impl MenuIds {
    /// Ids used when no tray exists: no `MenuEvent` will ever carry them.
    fn unavailable() -> Self {
        Self {
            open: "tray-unavailable-open".to_string(),
            mark_attendance: "tray-unavailable-mark-attendance".to_string(),
            whats_collected: "tray-unavailable-whats-collected".to_string(),
            withdraw: "tray-unavailable-withdraw".to_string(),
            quit: "tray-unavailable-quit".to_string(),
        }
    }
}

/// Updates forwarded to a tray owned by another thread (Linux GTK thread).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
enum TrayCommand {
    Color(u8, u8, u8),
    Tooltip(String),
}

pub struct SystemTrayManager {
    /// Tray owned by the UI thread (Windows, macOS main thread).
    pub tray_icon: Option<TrayIcon>,
    /// Channel to the GTK thread that owns the tray on Linux.
    commands: Option<std::sync::mpsc::Sender<TrayCommand>>,
    pub open_id: String,
    pub mark_attendance_id: String,
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

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn is_main_thread() -> bool {
    true
}

/// Builds the tray and its menu on the calling thread.
fn build_tray() -> Option<(TrayIcon, MenuIds)> {
    let menu = Menu::new();
    // Labels of docs/worker-app-spec.md §7.
    let open_item = MenuItem::new("Abrir", true, None);
    let mark_item = MenuItem::new("Marcar asistencia…", true, None);
    let whats_collected_item = MenuItem::new("Qué se recolecta", true, None);
    let withdraw_item = MenuItem::new("Retirar consentimiento...", true, None);
    let quit_item = MenuItem::new("Salir", true, None);

    let ids = MenuIds {
        open: open_item.id().0.clone(),
        mark_attendance: mark_item.id().0.clone(),
        whats_collected: whats_collected_item.id().0.clone(),
        withdraw: withdraw_item.id().0.clone(),
        quit: quit_item.id().0.clone(),
    };

    let _ = menu.append_items(&[
        &open_item,
        &mark_item,
        &whats_collected_item,
        &PredefinedMenuItem::separator(),
        &withdraw_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ]);

    let icon = create_tray_icon(46, 204, 113).ok()?; // Default green
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Telemetry — Asistencia")
        .with_icon(icon)
        .build()
        .map_err(|e| tracing::warn!("System tray unavailable: {}", e))
        .ok()?;
    Some((tray, ids))
}

/// Linux: tray-icon/muda create GTK widgets, but eframe runs a winit loop
/// without GTK. GTK is initialized and pumped on a dedicated thread that owns
/// the tray; updates arrive through a channel. Without a display or
/// AppIndicator the tray degrades to unavailable instead of panicking (PLAT-01).
#[cfg(target_os = "linux")]
fn spawn_gtk_tray() -> Option<(MenuIds, std::sync::mpsc::Sender<TrayCommand>)> {
    use std::sync::mpsc::{self, TryRecvError};
    use std::time::Duration;

    let (ids_tx, ids_rx) = mpsc::channel::<Option<MenuIds>>();
    let (cmd_tx, cmd_rx) = mpsc::channel::<TrayCommand>();

    std::thread::Builder::new()
        .name("tray-gtk".into())
        .spawn(move || {
            if let Err(e) = gtk::init() {
                tracing::warn!("System tray unavailable: GTK init failed: {}", e);
                let _ = ids_tx.send(None);
                return;
            }
            let Some((tray, ids)) = build_tray() else {
                let _ = ids_tx.send(None);
                return;
            };
            let _ = ids_tx.send(Some(ids));

            gtk::glib::timeout_add_local(Duration::from_millis(200), move || loop {
                match cmd_rx.try_recv() {
                    Ok(TrayCommand::Color(r, g, b)) => {
                        if let Ok(icon) = create_tray_icon(r, g, b) {
                            let _ = tray.set_icon(Some(icon));
                        }
                    }
                    Ok(TrayCommand::Tooltip(t)) => {
                        let _ = tray.set_tooltip(Some(t));
                    }
                    Err(TryRecvError::Empty) => return gtk::glib::ControlFlow::Continue,
                    Err(TryRecvError::Disconnected) => {
                        // Manager dropped: the app is exiting.
                        gtk::main_quit();
                        return gtk::glib::ControlFlow::Break;
                    }
                }
            });
            gtk::main();
        })
        .map_err(|e| tracing::warn!("System tray thread could not start: {}", e))
        .ok()?;

    let ids = ids_rx.recv_timeout(Duration::from_secs(5)).ok().flatten()?;
    Some((ids, cmd_tx))
}

impl SystemTrayManager {
    fn from_parts(
        tray_icon: Option<TrayIcon>,
        commands: Option<std::sync::mpsc::Sender<TrayCommand>>,
        ids: MenuIds,
    ) -> Self {
        Self {
            tray_icon,
            commands,
            open_id: ids.open,
            mark_attendance_id: ids.mark_attendance,
            whats_collected_id: ids.whats_collected,
            withdraw_id: ids.withdraw,
            quit_id: ids.quit,
        }
    }

    #[cfg(target_os = "linux")]
    pub fn new() -> Self {
        match spawn_gtk_tray() {
            Some((ids, commands)) => Self::from_parts(None, Some(commands), ids),
            None => Self::from_parts(None, None, MenuIds::unavailable()),
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn new() -> Self {
        if !is_main_thread() {
            // muda / Cocoa requires Menu creation strictly on macOS main thread.
            // When invoked in tests or non-main threads, gracefully return a stub manager.
            return Self::from_parts(None, None, MenuIds::unavailable());
        }
        match build_tray() {
            Some((tray, ids)) => Self::from_parts(Some(tray), None, ids),
            None => Self::from_parts(None, None, MenuIds::unavailable()),
        }
    }

    /// True when a tray icon is actually shown.
    pub fn is_available(&self) -> bool {
        self.tray_icon.is_some() || self.commands.is_some()
    }

    pub fn set_icon_color(&mut self, r: u8, g: u8, b: u8) {
        if let Some(ref mut tray) = self.tray_icon {
            if let Ok(icon) = create_tray_icon(r, g, b) {
                let _ = tray.set_icon(Some(icon));
            }
        } else if let Some(ref tx) = self.commands {
            let _ = tx.send(TrayCommand::Color(r, g, b));
        }
    }

    pub fn set_tooltip(&mut self, tooltip: &str) {
        if let Some(ref mut tray) = self.tray_icon {
            let _ = tray.set_tooltip(Some(tooltip));
        } else if let Some(ref tx) = self.commands {
            let _ = tx.send(TrayCommand::Tooltip(tooltip.to_string()));
        }
    }

    pub fn poll_action(&self) -> Option<TrayAction> {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = &event.id.0;
            if id == &self.open_id {
                return Some(TrayAction::Open);
            } else if id == &self.mark_attendance_id {
                return Some(TrayAction::MarkAttendance);
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

/// Telemetry line of the tray tooltip (spec §7, line 2).
pub fn telemetry_line(mode: &crate::state::RunMode) -> &'static str {
    use crate::state::RunMode;
    match mode {
        RunMode::Collecting(_) => "Telemetría activa",
        RunMode::Paused => "Telemetría en pausa",
        RunMode::EmployerSuspended { .. } => "Telemetría suspendida",
        RunMode::NotRequested => "Sin telemetría",
        RunMode::Declined => "Telemetría rechazada",
        RunMode::AwaitingConsent { .. } => "Telemetría: decisión pendiente",
        RunMode::Halted(_) => "Telemetría detenida",
        RunMode::Unenrolled => "Telemetría inactiva",
    }
}

/// Tooltip: attendance state on line 1, telemetry state on line 2 (RF-AGENT-12-14).
pub fn tray_tooltip(attendance_line: &str, mode: &crate::state::RunMode) -> String {
    format!("{attendance_line}\n{}", telemetry_line(mode))
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
            TrayAction::MarkAttendance => Some("MARK_ATTENDANCE"),
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
            tray.handle_action(TrayAction::MarkAttendance),
            Some("MARK_ATTENDANCE")
        );
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
    fn test_tray_tooltip_has_attendance_then_telemetry() {
        use crate::state::RunMode;
        let t = tray_tooltip("En jornada desde 09:02", &RunMode::Paused);
        assert_eq!(t, "En jornada desde 09:02\nTelemetría en pausa");
        assert_eq!(
            tray_tooltip("Fuera de jornada", &RunMode::NotRequested),
            "Fuera de jornada\nSin telemetría"
        );
    }

    #[test]
    fn test_create_tray_icon() {
        let icon = create_tray_icon(46, 204, 113);
        assert!(icon.is_ok());
    }

    #[test]
    fn test_system_tray_manager_initialization_never_panics() {
        // Off the main thread / without a display the tray must degrade, not
        // panic (release builds abort on panic).
        let mut manager = SystemTrayManager::new();
        assert!(!manager.open_id.is_empty());
        assert!(!manager.mark_attendance_id.is_empty());
        assert!(!manager.whats_collected_id.is_empty());
        assert!(!manager.withdraw_id.is_empty());
        assert!(!manager.quit_id.is_empty());
        manager.set_icon_color(241, 196, 15);
        manager.set_tooltip("test");
    }

    #[test]
    fn test_system_tray_unavailable_ids_never_match_actions() {
        let manager = SystemTrayManager::from_parts(None, None, MenuIds::unavailable());
        assert!(!manager.is_available());
        assert_eq!(manager.poll_action(), None);
    }
}
