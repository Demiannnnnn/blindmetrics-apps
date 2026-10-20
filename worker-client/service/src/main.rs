pub mod api;
pub mod attendance;
pub mod background;
pub mod config;
pub mod consent;
pub mod core;
pub mod credentials;
pub mod device;
pub mod enrollment;
pub mod errors;
pub mod io;
pub mod ipc_server;
pub mod legal;
pub mod notifications;
pub mod pipeline;
pub mod session;
pub mod state;
pub mod ui;

#[cfg(all(test, feature = "live-api"))]
mod live_api_tests;

use agent_ipc::{
    default_runtime_dir, socket_endpoint, token_path, IpcClient, SocketEndpoint, WindowScreenHint,
};
use attendance::{
    AttendanceService, DesktopNotifier, MarkJournal, SystemClock, UnavailableLocationProvider,
};
use background::BackgroundEngine;
use config::WorkerConfig;
use core::buffer::MetricsBuffer;
use credentials::{CredentialStore, KeyringCredentialStore};
use notifications::NotificationDispatcher;
use state::RunMode;
use ui::tray::{SystemTrayManager, TrayAction};

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Pure function to resolve the UI binary path (RF-AGENT-10-9).
/// Priority:
/// 1. `override_bin`: if `TELEMETRY_WORKER_UI_BIN` is provided (dev override, non-empty).
/// 2. Sibling executable `telemetry-worker` (`telemetry-worker.exe` on Windows) in the same directory as `current_exe`.
pub fn resolve_ui_binary_path(
    override_bin: Option<&str>,
    current_exe: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(dev_override) = override_bin {
        let trimmed = dev_override.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }

    #[cfg(windows)]
    let bin_name = "telemetry-worker.exe";
    #[cfg(not(windows))]
    let bin_name = "telemetry-worker";

    current_exe
        .and_then(|p| p.parent())
        .map(|dir| dir.join(bin_name))
}

fn launch_ui_bin() {
    let override_var = std::env::var("TELEMETRY_WORKER_UI_BIN").ok();
    let current_exe = std::env::current_exe().ok();
    let resolved = resolve_ui_binary_path(override_var.as_deref(), current_exe.as_deref());

    let Some(bin_path) = resolved else {
        tracing::warn!("Could not determine UI binary path (RF-AGENT-10-9)");
        NotificationDispatcher::notify_ui_not_found();
        return;
    };

    if !bin_path.is_file() {
        tracing::warn!("UI binary does not exist at {:?} (RF-AGENT-10-9)", bin_path);
        NotificationDispatcher::notify_ui_not_found();
        return;
    }

    match std::process::Command::new(&bin_path).spawn() {
        Ok(_) => {
            tracing::info!("Launched worker UI: {:?}", bin_path);
        }
        Err(e) => {
            tracing::warn!("Could not launch worker UI {:?}: {}", bin_path, e);
            NotificationDispatcher::notify_ui_not_found();
        }
    }
}

/// Tray icon colour = telemetry light (spec §7).
fn tray_color(mode: &RunMode) -> (u8, u8, u8) {
    match mode {
        RunMode::Collecting(_) => (46, 204, 113),
        RunMode::Paused | RunMode::EmployerSuspended { .. } => (241, 196, 15),
        RunMode::AwaitingConsent { .. }
        | RunMode::Unenrolled
        | RunMode::NotRequested
        | RunMode::Declined => (180, 180, 180),
        RunMode::Halted(_) => (231, 76, 60),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Prohibit --headless mode without tray (PROC-02 / RF-AGENT-10-10)
    if std::env::args().any(|arg| arg == "--headless") {
        eprintln!(
            "Error: El modo --headless sin bandeja está prohibido (PROC-02). \
             El servicio requiere bandeja del sistema para operar con transparencia."
        );
        std::process::exit(1);
    }

    // 2. Load environment variables from .env
    dotenvy::dotenv().ok();

    // 3. Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    // 4. Determine runtime directory and check for single instance (RF-AGENT-10-09)
    let runtime_dir = default_runtime_dir();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let is_already_running = rt.block_on(async {
        match IpcClient::connect(&runtime_dir).await {
            Ok(_client) => true,
            Err(_) => false,
        }
    });

    if is_already_running {
        tracing::info!(
            "Telemetry Worker service is already running on this machine. Exiting cleanly (RF-AGENT-10-09)."
        );
        std::process::exit(0);
    }

    // 5. Load configuration and hardware fingerprint
    let config = WorkerConfig::load_or_init();
    tracing::info!(
        "Telemetry Worker starting. Device ID: {}, Role: {}",
        &config.device_identifier[..8.min(config.device_identifier.len())],
        config.worker_role.as_deref().unwrap_or("OFFICE")
    );

    // 6. Initialize system tray (PROC-02)
    let mut tray = SystemTrayManager::new();
    let tray_available = tray.is_available();
    if !tray_available {
        tracing::warn!(
            "Bandeja del sistema no disponible en este entorno gráfico. \
             Los recolectores de telemetría no se iniciarán (PROC-02), pero el servicio continuará."
        );
    }

    // 7. Initialize credentials, metrics buffer, and background engine
    let cred_store: Arc<dyn CredentialStore> = Arc::new(KeyringCredentialStore::new());
    let buffer = Arc::new(MetricsBuffer::new(1000));

    let mut engine = BackgroundEngine::new(config.clone(), buffer.clone(), cred_store.clone())?;
    engine.set_tray_available(tray_available);
    let engine_handle = engine.handle(rt.handle().clone());
    let tray_available_flag = engine.tray_available_flag();

    // 7b. Attendance module (TASK-AGENT-12): mark journal in offline.db (table `marks`).
    let db_path = io::transport::default_offline_db_path();
    let journal = match MarkJournal::open(&db_path) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Cannot open the mark journal ({e}); using memory for this session");
            MarkJournal::open_in_memory()?
        }
    };
    let attendance = AttendanceService::new(
        &config.api_base_url,
        attendance::service::AttendanceDeps {
            creds: cred_store.clone(),
            journal,
            db_path: Some(db_path),
            clock: Arc::new(SystemClock),
            notifier: Arc::new(DesktopNotifier),
            location: Arc::new(UnavailableLocationProvider),
        },
        engine_handle.state.clone(),
        Some(engine_handle.commands.clone()),
    )?;
    engine.set_attendance(attendance.clone());

    // 8. Initialize IPC server (RF-AGENT-10-1)
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let ipc_server = Arc::new(ipc_server::IpcServer::new(ipc_server::IpcServerDeps {
        config: config.clone(),
        cred_store: cred_store.clone(),
        buffer: buffer.clone(),
        engine: engine_handle.clone(),
        runtime_dir: runtime_dir.clone(),
        tray_available: tray_available_flag.clone(),
        shutdown_notify: shutdown_notify.clone(),
        attendance: attendance.clone(),
    })?);

    // 9. Spawn engine and IPC server on Tokio runtime
    let engine_task = rt.spawn(engine.run());
    let attendance_task = rt.spawn(attendance.clone().run(shutdown_notify.clone()));
    let ipc_server_for_run = ipc_server.clone();
    let ipc_task = rt.spawn(async move {
        if let Err(e) = ipc_server_for_run.run().await {
            tracing::error!("IPC server error: {:?}", e);
        }
    });

    // 10. Listen for OS signals (SIGINT / SIGTERM)
    let shutdown_signal_notify = shutdown_notify.clone();
    rt.spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate()).ok();
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("SIGINT received");
                }
                _ = async {
                    if let Some(ref mut s) = sigterm {
                        s.recv().await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    tracing::info!("SIGTERM received");
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("Ctrl+C received");
        }
        shutdown_signal_notify.notify_waiters();
    });

    // 11. Shutdown observer flag
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let shutdown_req_clone = shutdown_requested.clone();
    let shutdown_watch = shutdown_notify.clone();
    rt.spawn(async move {
        shutdown_watch.notified().await;
        shutdown_req_clone.store(true, Ordering::SeqCst);
    });

    // 12. Main loop: tray polling and color updates
    let mut quit_deadline: Option<Instant> = None;
    let mut last_run_mode = engine_handle.state.borrow().run_mode.clone();

    // Initial tray icon color and tooltip (spec §7)
    let (r, g, b) = tray_color(&last_run_mode);
    tray.set_icon_color(r, g, b);
    let mut last_tooltip = String::new();
    let mut tooltip_tick: u32 = 0;

    loop {
        if shutdown_requested.load(Ordering::Relaxed) {
            break;
        }

        if let Some(action) = tray.poll_action() {
            match action {
                TrayAction::Open => {
                    tracing::info!("Tray action: Abrir");
                    let has_subs = rt.block_on(ipc_server.context.has_subscribers());
                    if !has_subs {
                        launch_ui_bin();
                    } else {
                        rt.block_on(
                            ipc_server
                                .context
                                .set_suggested_screen(Some(WindowScreenHint::Home)),
                        );
                    }
                }
                TrayAction::MarkAttendance => {
                    tracing::info!("Tray action: Marcar asistencia");
                    rt.block_on(
                        ipc_server
                            .context
                            .set_suggested_screen(Some(WindowScreenHint::Attendance)),
                    );
                    let has_subs = rt.block_on(ipc_server.context.has_subscribers());
                    if !has_subs {
                        launch_ui_bin();
                    }
                }
                TrayAction::WhatsCollected => {
                    tracing::info!("Tray action: Qué se recolecta");
                    rt.block_on(
                        ipc_server
                            .context
                            .set_suggested_screen(Some(WindowScreenHint::Telemetry)),
                    );
                    let has_subs = rt.block_on(ipc_server.context.has_subscribers());
                    if !has_subs {
                        launch_ui_bin();
                    }
                }
                TrayAction::WithdrawConsent => {
                    tracing::info!("Tray action: Retirar consentimiento...");
                    rt.block_on(
                        ipc_server
                            .context
                            .set_suggested_screen(Some(WindowScreenHint::Withdraw)),
                    );
                    let has_subs = rt.block_on(ipc_server.context.has_subscribers());
                    if !has_subs {
                        launch_ui_bin();
                    }
                }
                TrayAction::Quit => {
                    // PROC-04: with a window open, ask it to show the confirmation dialog. The
                    // window answers with a one-shot nonce valid for 30 s; only then does the
                    // IPC server notify the shutdown (no IPC client can stop the service alone).
                    if rt.block_on(ipc_server.context.request_quit()) {
                        tracing::info!(
                            "Tray action: Salir requested. Awaiting confirmation in the window."
                        );
                        quit_deadline = None;
                    } else {
                        // No window: fall back to pressing Salir twice within 10 s.
                        let now = Instant::now();
                        if let Some(deadline) = quit_deadline {
                            if now <= deadline {
                                tracing::info!("Tray action: Salir confirmed.");
                                break;
                            }
                        }
                        tracing::info!(
                            "Tray action: Salir requested. Awaiting confirmation within 10s."
                        );
                        NotificationDispatcher::notify_confirm_quit();
                        quit_deadline = Some(now + Duration::from_secs(10));
                    }
                }
            }
        }

        // Expire quit confirmation deadline
        if let Some(deadline) = quit_deadline {
            if Instant::now() > deadline {
                quit_deadline = None;
            }
        }

        // Sync tray icon color with RunMode transitions
        let current_mode = engine_handle.state.borrow().run_mode.clone();
        if current_mode != last_run_mode {
            let (r, g, b) = tray_color(&current_mode);
            tray.set_icon_color(r, g, b);
            last_run_mode = current_mode;
        }

        // Tooltip: attendance line + telemetry line, refreshed every second. On Linux the update
        // goes through the channel to the GTK thread that owns the tray.
        tooltip_tick = tooltip_tick.wrapping_add(1);
        if tooltip_tick % 10 == 0 {
            let tooltip = ui::tray::tray_tooltip(&attendance.tray_line(), &last_run_mode);
            if tooltip != last_tooltip {
                tray.set_tooltip(&tooltip);
                last_tooltip = tooltip;
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    // 13. Clean shutdown sequence (RF-AGENT-10-12)
    tracing::info!("Initiating clean shutdown sequence (RF-AGENT-10-12)...");
    engine_handle.shutdown();
    shutdown_notify.notify_waiters();

    let _ = rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(3), async {
            let _ = tokio::join!(engine_task, ipc_task, attendance_task);
        })
        .await
    });

    let _ = std::fs::remove_file(token_path(&runtime_dir));
    if let SocketEndpoint::Path(ref p) = socket_endpoint(&runtime_dir) {
        let _ = std::fs::remove_file(p);
    }

    tracing::info!("Clean shutdown complete. Exiting with code 0.");
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_ui_binary_path_with_dev_override() {
        let dev = Some("/custom/bin/telemetry-worker-dev");
        let current_exe = Some(Path::new("/opt/telemetry/bin/telemetry-agent"));
        let resolved = resolve_ui_binary_path(dev, current_exe);
        assert_eq!(
            resolved,
            Some(PathBuf::from("/custom/bin/telemetry-worker-dev"))
        );
    }

    #[test]
    fn test_resolve_ui_binary_path_sibling_to_current_exe() {
        let current_exe = Some(Path::new("/opt/telemetry/bin/telemetry-agent"));
        let resolved = resolve_ui_binary_path(None, current_exe);
        #[cfg(windows)]
        assert_eq!(
            resolved,
            Some(PathBuf::from(r"/opt/telemetry/bin\telemetry-worker.exe"))
        );
        #[cfg(not(windows))]
        assert_eq!(
            resolved,
            Some(PathBuf::from("/opt/telemetry/bin/telemetry-worker"))
        );
    }

    #[test]
    fn test_resolve_ui_binary_path_empty_override_falls_back_to_sibling() {
        let dev = Some("   ");
        let current_exe = Some(Path::new("/usr/local/bin/telemetry-agent"));
        let resolved = resolve_ui_binary_path(dev, current_exe);
        #[cfg(windows)]
        assert_eq!(
            resolved,
            Some(PathBuf::from(r"/usr/local/bin\telemetry-worker.exe"))
        );
        #[cfg(not(windows))]
        assert_eq!(
            resolved,
            Some(PathBuf::from("/usr/local/bin/telemetry-worker"))
        );
    }

    #[test]
    fn test_resolve_ui_binary_path_missing_inputs() {
        let resolved = resolve_ui_binary_path(None, None);
        assert_eq!(resolved, None);
    }
}
