pub mod client;
pub mod commands;
pub mod error;
pub mod qr;

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
            commands::legal_documents,
            commands::accept_terms,
            commands::set_pin,
            commands::change_pin,
            commands::request_pin_reset,
            commands::verify_pin_reset,
            commands::mark_attendance,
            commands::attendance_today,
            commands::attendance_days,
            commands::attendance_marks,
            commands::list_corrections,
            commands::respond_correction,
            commands::decode_enrollment_qr_image,
        ])
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_ipc::framing::{read_frame, write_frame};
    use agent_ipc::transport::{bind_listener, socket_endpoint, token_path, write_ipc_token};
    use agent_ipc::{
        AppSnapshot, CategoryState, CategoryStates, ClientFrame, Correction, CorrectionStatus,
        DataSource, LegalDocument, LegalDocumentKind, ListenerTrait, LocalMarkStatus, MarkRecord,
        MarkType, Pagination, Request, Response, SecretPin, ServerFrame, ServerMark, TodayView,
        PROTOCOL_VERSION,
    };
    use std::path::Path;
    use std::sync::{Arc, Mutex};
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
            attendance_mode: Default::default(),
            terms_update_pending: false,
            attendance_today: None,
            pending_marks: 0,
            pending_corrections: 0,
            banners: Vec::new(),
            legal: Default::default(),
            work_mode: None,
            location_at_mark: false,
            location_available: false,
            personal_email: None,
        }
    }

    /// Fake service: after `delay` it listens on the runtime dir socket, answers the handshake and,
    /// for a `Subscribe`, pushes `events` and closes the connection.
    fn spawn_fake_server(runtime_dir: &Path, delay: Duration, events: Vec<Event>) {
        spawn_fake_server_triggered(runtime_dir, None, delay, events);
    }

    /// Fake service that optionally waits for a trigger file before waiting `delay_after_trigger`
    /// and listening on the socket (RF-INT-06-2).
    fn spawn_fake_server_triggered(
        runtime_dir: &Path,
        trigger_marker: Option<&Path>,
        delay_after_trigger: Duration,
        events: Vec<Event>,
    ) {
        let runtime_dir = runtime_dir.to_path_buf();
        let trigger = trigger_marker.map(|p| p.to_path_buf());
        tokio::spawn(async move {
            if let Some(marker) = trigger {
                let start = std::time::Instant::now();
                while !marker.exists() && start.elapsed() < Duration::from_secs(5) {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                if !marker.exists() {
                    return;
                }
            }
            if !delay_after_trigger.is_zero() {
                tokio::time::sleep(delay_after_trigger).await;
            }
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

    type Handler = Arc<dyn Fn(&Request) -> Response + Send + Sync>;

    /// Fake service answering `Call` frames with `handler` and recording every request.
    fn spawn_fake_call_server(runtime_dir: &Path, handler: Handler) -> Arc<Mutex<Vec<Request>>> {
        let seen: Arc<Mutex<Vec<Request>>> = Arc::new(Mutex::new(Vec::new()));
        let runtime_dir = runtime_dir.to_path_buf();
        let recorded = seen.clone();
        let token = write_ipc_token(&token_path(&runtime_dir)).expect("write token");
        tokio::spawn(async move {
            let listener = bind_listener(&socket_endpoint(&runtime_dir))
                .await
                .expect("bind listener");
            loop {
                let Ok(mut stream) = listener.accept().await else {
                    return;
                };
                let token = token.clone();
                let handler = handler.clone();
                let recorded = recorded.clone();
                tokio::spawn(async move {
                    let Ok(Some(ClientFrame::Hello { token: t, .. })) =
                        read_frame::<_, ClientFrame>(&mut stream).await
                    else {
                        return;
                    };
                    assert_eq!(t, token);
                    let welcome = ServerFrame::Welcome {
                        service_version: "0.1.0-test".into(),
                        protocol_version: PROTOCOL_VERSION,
                    };
                    if write_frame(&mut stream, &welcome).await.is_err() {
                        return;
                    }
                    while let Ok(Some(ClientFrame::Call { id, request })) =
                        read_frame::<_, ClientFrame>(&mut stream).await
                    {
                        let response = handler(&request);
                        recorded.lock().expect("lock").push(request);
                        let reply = ServerFrame::Reply { id, response };
                        if write_frame(&mut stream, &reply).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });
        seen
    }

    fn server_mark() -> ServerMark {
        serde_json::from_value(json!({
            "id": "01927d3b-65e1-7000-8000-000000000001",
            "client_mark_id": "01927d3b-65e1-7000-8000-0000000000c1",
            "worker_id": "01927d3b-65e1-7000-8000-0000000000aa",
            "type": "WORKDAY_START",
            "server_ts": "2026-10-11T12:00:05Z", "device_ts": "2026-10-11T12:00:04Z",
            "offline": false, "source": "AGENT", "identification": "DEVICE_PIN",
            "location": null, "hash": "a".repeat(64), "prev_hash": "b".repeat(64),
            "effective_ts": "2026-10-11T12:00:05Z"
        }))
        .expect("server mark")
    }

    fn correction() -> Correction {
        serde_json::from_value(json!({
            "id": "01927d3b-65e1-7000-8000-0000000000d1",
            "kind": "MODIFY", "original_mark": null, "type": "WORKDAY_START",
            "proposed_ts": "2026-10-11T12:00:00Z", "reason": "Ajuste de hora",
            "status": "OPPOSED", "respond_before": "2026-10-13T12:00:00Z"
        }))
        .expect("correction")
    }

    /// Answers every attendance op like the service would (shapes of `agent-ipc`).
    fn attendance_handler(request: &Request) -> Response {
        match request {
            Request::LegalDocuments => Response::LegalDocuments {
                documents: vec![LegalDocument {
                    document: LegalDocumentKind::WorkerTerms,
                    version: "2026-10-11".into(),
                    hash: "c".repeat(64),
                    text: "# Términos".into(),
                }],
            },
            Request::AcceptTerms { version, .. } => Response::AcceptTerms {
                terms_accepted_version: version.clone(),
            },
            Request::SetPin { .. } => Response::SetPin,
            Request::ChangePin { .. } => Response::ChangePin,
            Request::RequestPinReset => Response::RequestPinReset,
            Request::VerifyPinReset { .. } => Response::VerifyPinReset {
                set_pin_window_secs: 600,
            },
            Request::Mark { pin, .. } if pin.expose().as_str() == "000000" => Response::Error {
                code: "PIN_INCORRECT".into(),
                message: "PIN incorrecto. Espera 2 s para reintentar.".into(),
                retry_after_secs: Some(2),
            },
            Request::Mark { r#type, .. } => Response::Mark {
                mark: Box::new(MarkRecord {
                    client_mark_id: "01927d3b-65e1-7000-8000-0000000000c1".into(),
                    r#type: *r#type,
                    device_ts: "2026-10-11T12:00:04Z".into(),
                    offline: false,
                    status: LocalMarkStatus::Confirmed,
                    server_mark: Some(server_mark()),
                    duplicate_of: None,
                    rejection_reason: None,
                }),
            },
            Request::Today => Response::Today {
                today: Box::new(TodayView {
                    source: DataSource::Local,
                    server: None,
                    local_marks: Vec::new(),
                    server_time_offset_secs: Some(3),
                }),
            },
            Request::Days { .. } => Response::Error {
                code: "OFFLINE".into(),
                message: "Sin conexión".into(),
                retry_after_secs: None,
            },
            Request::Marks { .. } => Response::Marks {
                source: DataSource::Server,
                marks: vec![server_mark()],
                local: Vec::new(),
                pagination: Some(Pagination {
                    total: 1,
                    page: 1,
                    per_page: 50,
                    total_pages: 1,
                }),
            },
            Request::Corrections => Response::Corrections {
                corrections: vec![correction()],
            },
            Request::RespondCorrection { .. } => Response::RespondCorrection {
                correction: Box::new(correction()),
            },
            _ => Response::Error {
                code: "UNEXPECTED".into(),
                message: "not handled".into(),
                retry_after_secs: None,
            },
        }
    }

    #[tokio::test]
    async fn test_attendance_ops_are_forwarded_to_the_service() {
        let dir = tempdir().expect("tempdir");
        let rt = dir.path().to_path_buf();
        let seen = spawn_fake_call_server(&rt, Arc::new(attendance_handler));
        tokio::time::sleep(Duration::from_millis(100)).await;

        let docs = client::legal_documents(&rt).await.expect("legal");
        assert_eq!(docs[0].version, "2026-10-11");
        let accepted = client::accept_terms(
            &rt,
            "2026-10-11".into(),
            "c".repeat(64),
            "2026-10-11".into(),
            "d".repeat(64),
        )
        .await
        .expect("accept");
        assert_eq!(accepted, "2026-10-11");
        client::set_pin(&rt, SecretPin::new("975318"))
            .await
            .expect("set pin");
        client::change_pin(&rt, SecretPin::new("975318"), SecretPin::new("482916"))
            .await
            .expect("change pin");
        client::request_pin_reset(&rt).await.expect("reset");
        assert_eq!(
            client::verify_pin_reset(&rt, "123456".into())
                .await
                .expect("verify"),
            600
        );
        let mark = client::mark(&rt, MarkType::BreakStart, SecretPin::new("975318"))
            .await
            .expect("mark");
        assert_eq!(mark.r#type, MarkType::BreakStart);
        assert_eq!(mark.status, LocalMarkStatus::Confirmed);
        let today = client::today(&rt).await.expect("today");
        assert_eq!(today.server_time_offset_secs, Some(3));
        let page = client::marks(&rt, "2026-10-01".into(), "2026-10-12".into(), Some(2))
            .await
            .expect("marks");
        assert_eq!(page.source, DataSource::Server);
        assert_eq!(page.marks.len(), 1);
        assert_eq!(
            client::corrections(&rt).await.expect("corrections").len(),
            1
        );
        let answered =
            client::respond_correction(&rt, "01927d3b-65e1-7000-8000-0000000000d1".into(), false)
                .await
                .expect("respond");
        assert_eq!(answered.status, CorrectionStatus::Opposed);

        let seen = seen.lock().expect("lock").clone();
        assert_eq!(
            seen,
            vec![
                Request::LegalDocuments,
                Request::AcceptTerms {
                    version: "2026-10-11".into(),
                    hash: "c".repeat(64),
                    privacy_version: "2026-10-11".into(),
                    privacy_hash: "d".repeat(64),
                },
                Request::SetPin {
                    pin: SecretPin::new("975318")
                },
                Request::ChangePin {
                    current: SecretPin::new("975318"),
                    new: SecretPin::new("482916")
                },
                Request::RequestPinReset,
                Request::VerifyPinReset {
                    code: "123456".into()
                },
                Request::Mark {
                    r#type: MarkType::BreakStart,
                    pin: SecretPin::new("975318")
                },
                Request::Today,
                Request::Marks {
                    from: "2026-10-01".into(),
                    to: "2026-10-12".into(),
                    page: Some(2)
                },
                Request::Corrections,
                Request::RespondCorrection {
                    id: "01927d3b-65e1-7000-8000-0000000000d1".into(),
                    accept: false
                },
            ]
        );
    }

    #[tokio::test]
    async fn test_service_errors_keep_code_and_retry_delay() {
        let dir = tempdir().expect("tempdir");
        let rt = dir.path().to_path_buf();
        spawn_fake_call_server(&rt, Arc::new(attendance_handler));
        tokio::time::sleep(Duration::from_millis(100)).await;

        let err = client::mark(&rt, MarkType::WorkdayStart, SecretPin::new("000000"))
            .await
            .expect_err("wrong pin");
        assert_eq!(err.code, "PIN_INCORRECT");
        assert_eq!(err.retry_after_secs, Some(2));
        // The WebView receives the delay to run the countdown (RF-AGENT-13-4).
        let wire = serde_json::to_value(&err).expect("json");
        assert_eq!(wire["retry_after_secs"], 2);

        let err = client::days(&rt, "2026-10-01".into(), "2026-10-12".into())
            .await
            .expect_err("offline");
        assert_eq!(err.code, "OFFLINE");
        assert_eq!(err.retry_after_secs, None);
        let wire = serde_json::to_value(&err).expect("json");
        assert!(wire.get("retry_after_secs").is_none());
    }

    #[test]
    fn test_command_pin_argument_deserializes_into_secret_pin() {
        // Tauri deserializes command arguments with serde: the PIN lands in a zeroizing type
        // and never prints in logs (SEC-19).
        let pin: SecretPin = serde_json::from_value(json!("975318")).expect("pin");
        assert_eq!(pin.expose().as_str(), "975318");
        assert!(!format!("{pin:?}").contains("975318"));
        let mark_type: MarkType = serde_json::from_value(json!("BREAK_END")).expect("type");
        assert_eq!(mark_type, MarkType::BreakEnd);
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
        // The fake server starts listening 200 ms after the script touches the marker (RF-INT-06-2),
        // guaranteeing that the binary was launched and ensuring retry until socket is up without test races.
        spawn_fake_server_triggered(
            &runtime_dir,
            Some(&marker),
            Duration::from_millis(200),
            Vec::new(),
        );

        client::ensure_service_with(&runtime_dir, script.to_str(), None, Duration::from_secs(5))
            .await
            .expect("service becomes reachable");

        // Bounded check for the marker to eliminate race conditions (RF-INT-06-2)
        let mut marker_found = false;
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            if marker.exists() {
                marker_found = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(marker_found, "the service binary was launched");
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

    #[tokio::test]
    async fn test_qr_decoded_payload_feeds_ipc_enroll_flow() {
        use telemetry_crypto_core::EnrollmentQr;

        let qr = EnrollmentQr::new(
            "https://api.telemetry.local/api/v1".into(),
            "A68K-2PZS-PC4H".into(),
            "a".repeat(64),
        )
        .expect("enrollment qr");
        let pasted_payload = qr.to_payload().expect("payload");

        let code = qrcode::QrCode::new(pasted_payload.as_bytes()).expect("qr code");
        let img: image::RgbImage = code
            .render::<image::Rgb<u8>>()
            .min_dimensions(300, 300)
            .build();
        let mut png_bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .expect("png write");

        let decoded_payload =
            qr::decode_qr_from_image_bytes(&png_bytes).expect("decode from image");
        assert_eq!(decoded_payload, pasted_payload);

        let dir = tempdir().expect("tempdir");
        let rt = dir.path().to_path_buf();
        let seen = spawn_fake_call_server(
            &rt,
            Arc::new(|req| match req {
                Request::Enroll { .. } => Response::Enroll {
                    result: agent_ipc::VerificationResult::Verified,
                },
                _ => Response::Error {
                    code: "UNHANDLED".into(),
                    message: "not handled".into(),
                    retry_after_secs: None,
                },
            }),
        );
        let mut connected = false;
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if IpcClient::connect(&rt).await.is_ok() {
                connected = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(connected, "fake call server failed to start");

        let res_decoded = client::enroll(&rt, Some(decoded_payload), None)
            .await
            .expect("enroll with decoded");
        assert_eq!(res_decoded, agent_ipc::VerificationResult::Verified);

        let res_pasted = client::enroll(&rt, Some(pasted_payload.clone()), None)
            .await
            .expect("enroll with pasted");
        assert_eq!(res_pasted, agent_ipc::VerificationResult::Verified);

        let requests = seen.lock().expect("lock").clone();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0],
            Request::Enroll {
                qr_payload: Some(pasted_payload.clone()),
                code: None,
            }
        );
        assert_eq!(requests[0], requests[1]);
    }
}
