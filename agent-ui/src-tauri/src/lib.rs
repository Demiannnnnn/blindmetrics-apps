pub mod client;
pub mod commands;
pub mod error;

pub use error::CommandError;

use agent_ipc::{default_runtime_dir, Event, EventStream, IpcClient, StreamExt};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;
use tauri::Emitter;

/// Tauri event names the WebView listens to (only `src/api/tauri-bridge.ts` uses them).
pub const EVENT_SNAPSHOT: &str = "snapshot";
pub const EVENT_QUIT_REQUESTED: &str = "quit-requested";
pub const EVENT_QUIT_CANCELLED: &str = "quit-cancelled";
pub const EVENT_SERVICE_DISCONNECTED: &str = "service-disconnected";

/// Re-emits every service event as a Tauri event until the stream ends (RF-AGENT-11-3).
/// Emits `service-disconnected` when the connection closes so the window can show screen X.
pub async fn forward_events<S, F>(mut stream: S, mut emit: F)
where
    S: EventStream<Item = Event> + Unpin,
    F: FnMut(&'static str, Value),
{
    while let Some(event) = stream.next().await {
        match event {
            Event::Snapshot(snapshot) => {
                emit(EVENT_SNAPSHOT, json!(*snapshot));
            }
            Event::QuitRequested {
                nonce,
                expires_in_secs,
            } => {
                emit(
                    EVENT_QUIT_REQUESTED,
                    json!({ "nonce": nonce, "expires_in_secs": expires_in_secs }),
                );
            }
            Event::QuitCancelled => emit(EVENT_QUIT_CANCELLED, Value::Null),
        }
    }
    emit(EVENT_SERVICE_DISCONNECTED, Value::Null);
}

/// Starts background subscriber that streams service events to the WebView (RF-AGENT-11-3).
pub fn start_snapshot_subscription(app_handle: tauri::AppHandle, runtime_dir: PathBuf) {
    tauri::async_runtime::spawn(async move {
        loop {
            match IpcClient::subscribe(&runtime_dir).await {
                Ok(stream) => {
                    tracing::info!("Subscribed to service events");
                    let handle = app_handle.clone();
                    forward_events(Box::pin(stream), move |name, payload| {
                        if let Err(e) = handle.emit(name, payload) {
                            tracing::warn!("Event {} not delivered: {}", name, e);
                        }
                    })
                    .await;
                    tracing::warn!("Service event stream closed, reconnecting...");
                }
                Err(_) => {
                    // The service might not be running yet; wait and retry.
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

/// Builds and runs the Tauri window. Returns an error instead of panicking (release aborts on panic).
pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            start_snapshot_subscription(handle, default_runtime_dir());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::service_status,
            commands::ensure_service,
            commands::fetch_snapshot,
            commands::enroll_worker,
            commands::verify_safety_code,
            commands::submit_consent,
            commands::withdraw_consent,
            commands::request_data,
            commands::request_deletion,
            commands::unlink_device,
            commands::get_diagnostics,
            commands::set_window_screen_hint,
            commands::confirm_quit,
            commands::cancel_quit,
        ])
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_ipc::framing::{read_frame, write_frame};
    use agent_ipc::transport::{bind_listener, socket_endpoint, token_path, write_ipc_token};
    use agent_ipc::{
        AppSnapshot, CategoryState, CategoryStates, ClientFrame, ListenerTrait, ServerFrame,
        PROTOCOL_VERSION,
    };
    use std::path::Path;
    use tempfile::tempdir;

    fn dummy_snapshot() -> AppSnapshot {
        AppSnapshot {
            service_version: "0.1.0-test".into(),
            run_mode: "COLLECTING".into(),
            consent_offer: None,
            accepted_selection: None,
            category_states: CategoryStates {
                system_activity: CategoryState::Active,
                network_activity: CategoryState::NotRequested,
                productivity_basic: CategoryState::Active,
                productivity_screenshots: CategoryState::Unavailable,
                file_activity: CategoryState::NotAccepted,
                location: CategoryState::NotRequested,
            },
            work_hours_start: None,
            work_hours_end: None,
            timezone: None,
            worker_name: Some("Test Worker".into()),
            organization: Some("Acme SpA".into()),
            queue_size: 0,
            last_flush: None,
            last_error: None,
            suggested_screen: None,
            tray_available: true,
            pending_withdrawal: false,
            device_identifier_short: "abcd1234".into(),
            key_fingerprint: None,
            is_enrolled: true,
        }
    }

    /// Fake service: after `delay` it listens on the runtime dir socket, answers the handshake and,
    /// for a `Subscribe`, pushes `events` and closes the connection.
    fn spawn_fake_server(runtime_dir: &Path, delay: Duration, events: Vec<Event>) {
        let runtime_dir = runtime_dir.to_path_buf();
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            let token = write_ipc_token(&token_path(&runtime_dir)).expect("write token");
            let listener = bind_listener(&socket_endpoint(&runtime_dir))
                .await
                .expect("bind listener");
            loop {
                let Ok(mut stream) = listener.accept().await else {
                    return;
                };
                let token = token.clone();
                let events = events.clone();
                tokio::spawn(async move {
                    let Ok(Some(ClientFrame::Hello {
                        protocol_version,
                        token: client_token,
                    })) = read_frame::<_, ClientFrame>(&mut stream).await
                    else {
                        return;
                    };
                    assert_eq!(protocol_version, PROTOCOL_VERSION);
                    assert_eq!(client_token, token);
                    let welcome = ServerFrame::Welcome {
                        service_version: "0.1.0-test".into(),
                        protocol_version: PROTOCOL_VERSION,
                    };
                    if write_frame(&mut stream, &welcome).await.is_err() {
                        return;
                    }
                    if let Ok(Some(ClientFrame::Subscribe)) =
                        read_frame::<_, ClientFrame>(&mut stream).await
                    {
                        for ev in events {
                            if write_frame(&mut stream, &ServerFrame::Event(ev))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                });
            }
        });
    }

    #[test]
    fn test_proc_03_no_reqwest_or_keyring_dependency() {
        // PROC-03: agent-ui-core holds NO tokens, keys, PIN verifier or API URL logic.
        // It talks only to the service over the local IPC socket. It NEVER depends on reqwest or keyring.
        let cargo_toml = include_str!("../Cargo.toml");
        assert!(
            !cargo_toml.contains("reqwest"),
            "agent-ui-core must NEVER depend on reqwest (PROC-03)"
        );
        assert!(
            !cargo_toml.contains("keyring"),
            "agent-ui-core must NEVER depend on keyring (PROC-03)"
        );
    }

    #[test]
    fn test_find_service_binary_prefers_override() {
        let temp = tempfile::NamedTempFile::new().expect("temp file");
        let path_str = temp.path().to_str().expect("path str");
        let found = client::find_service_binary(Some(path_str), None);
        assert_eq!(found, Some(temp.path().to_path_buf()));
    }

    #[tokio::test]
    async fn test_forward_events_reemits_snapshots_and_quit_requests() {
        let dir = tempdir().expect("tempdir");
        let runtime_dir = dir.path().to_path_buf();
        spawn_fake_server(
            &runtime_dir,
            Duration::ZERO,
            vec![
                Event::Snapshot(Box::new(dummy_snapshot())),
                Event::QuitRequested {
                    nonce: "n1".into(),
                    expires_in_secs: 30,
                },
                Event::QuitCancelled,
            ],
        );
        tokio::time::sleep(Duration::from_millis(100)).await;

        let stream = IpcClient::subscribe(&runtime_dir)
            .await
            .expect("subscribe to fake server");
        let mut emitted: Vec<(&'static str, Value)> = Vec::new();
        forward_events(Box::pin(stream), |name, payload| {
            emitted.push((name, payload))
        })
        .await;

        let names: Vec<&str> = emitted.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            vec![
                EVENT_SNAPSHOT,
                EVENT_QUIT_REQUESTED,
                EVENT_QUIT_CANCELLED,
                EVENT_SERVICE_DISCONNECTED
            ]
        );
        assert_eq!(emitted[0].1["worker_name"], "Test Worker");
        assert_eq!(emitted[0].1["service_version"], "0.1.0-test");
        assert_eq!(emitted[1].1["nonce"], "n1");
        assert_eq!(emitted[1].1["expires_in_secs"], 30);
    }

    #[tokio::test]
    async fn test_ensure_service_reports_missing_binary() {
        let dir = tempdir().expect("tempdir");
        let err = client::ensure_service_with(
            dir.path(),
            None,
            Some(Path::new("/nonexistent-dir/telemetry-worker")),
            Duration::from_millis(300),
        )
        .await
        .expect_err("no service binary");
        assert_eq!(err.code, "SERVICE_NOT_FOUND");
    }

    #[cfg(unix)]
    fn write_script(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, body).expect("write script");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_ensure_service_launches_binary_and_retries_until_socket_is_up() {
        // PROC-04: if the service does not answer, launch it and retry the connection (<= 5 s).
        let dir = tempdir().expect("tempdir");
        let runtime_dir = dir.path().join("runtime");
        let script = dir.path().join("fake-service.sh");
        let marker = dir.path().join("started");
        write_script(
            &script,
            &format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
        );
        // The "service" starts listening 400 ms after being launched.
        spawn_fake_server(&runtime_dir, Duration::from_millis(400), Vec::new());

        client::ensure_service_with(&runtime_dir, script.to_str(), None, Duration::from_secs(5))
            .await
            .expect("service becomes reachable");
        assert!(marker.exists(), "the service binary was launched");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_ensure_service_times_out_when_service_never_listens() {
        let dir = tempdir().expect("tempdir");
        let script = dir.path().join("fake-service.sh");
        write_script(&script, "#!/bin/sh\nexit 0\n");
        let err = client::ensure_service_with(
            &dir.path().join("runtime"),
            script.to_str(),
            None,
            Duration::from_millis(400),
        )
        .await
        .expect_err("service never listens");
        assert_eq!(err.code, "SERVICE_TIMEOUT");
    }
}
