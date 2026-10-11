use agent_ipc::client::IpcClient;
use agent_ipc::framing::{read_frame, write_frame};
use agent_ipc::transport::{bind_listener, token_path, write_ipc_token, SocketEndpoint};
use agent_ipc::{
    AppSnapshot, CategoryState, CategoryStates, ClientFrame, Event, IpcError, Request, Response,
    ServerFrame, PROTOCOL_VERSION,
};
use interprocess::local_socket::traits::tokio::Listener as _;

#[cfg(unix)]
#[tokio::test]
async fn test_ipc_handshake_success_and_call() {
    let temp = tempfile::tempdir().unwrap();
    let runtime_dir = temp.path();
    let t_path = token_path(runtime_dir);
    let token = write_ipc_token(&t_path).unwrap();

    let endpoint = SocketEndpoint::Path(runtime_dir.join("telemetry-worker").join("agent.sock"));
    let listener = bind_listener(&endpoint).await.unwrap();

    let server_task = tokio::spawn(async move {
        let mut stream = listener.accept().await.unwrap();
        // Read Hello
        let frame: Option<ClientFrame> = read_frame(&mut stream).await.unwrap();
        match frame {
            Some(ClientFrame::Hello {
                protocol_version,
                token: cand_token,
            }) => {
                assert_eq!(protocol_version, PROTOCOL_VERSION);
                assert_eq!(cand_token, token);
                // Send Welcome
                let welcome = ServerFrame::Welcome {
                    service_version: "0.1.0".into(),
                    protocol_version: PROTOCOL_VERSION,
                };
                write_frame(&mut stream, &welcome).await.unwrap();
            }
            _ => panic!("Expected Hello"),
        }

        // Read Call
        let frame: Option<ClientFrame> = read_frame(&mut stream).await.unwrap();
        match frame {
            Some(ClientFrame::Call { id, request }) => {
                assert_eq!(request, Request::Snapshot);
                let snapshot = AppSnapshot {
                    service_version: "0.1.0".into(),
                    run_mode: "UNENROLLED".into(),
                    consent_offer: None,
                    accepted_selection: None,
                    category_states: CategoryStates {
                        system_activity: CategoryState::NotRequested,
                        network_activity: CategoryState::NotRequested,
                        productivity_basic: CategoryState::NotRequested,
                        productivity_screenshots: CategoryState::NotRequested,
                        file_activity: CategoryState::NotRequested,
                        location: CategoryState::NotRequested,
                    },
                    work_hours_start: None,
                    work_hours_end: None,
                    timezone: None,
                    worker_name: None,
                    organization: None,
                    queue_size: 0,
                    last_flush: None,
                    last_error: None,
                    suggested_screen: None,
                    tray_available: true,
                    pending_withdrawal: false,
                    device_identifier_short: "12345678".into(),
                    key_fingerprint: None,
                    is_enrolled: false,
                };
                let reply = ServerFrame::Reply {
                    id,
                    response: Response::Snapshot {
                        snapshot: Box::new(snapshot),
                    },
                };
                write_frame(&mut stream, &reply).await.unwrap();
            }
            _ => panic!("Expected Call"),
        }
    });

    let client = IpcClient::connect(runtime_dir).await.unwrap();
    let res = client.call(Request::Snapshot).await.unwrap();
    match res {
        Response::Snapshot { snapshot } => {
            assert_eq!(snapshot.service_version, "0.1.0");
            assert_eq!(snapshot.run_mode, "UNENROLLED");
        }
        _ => panic!("Unexpected response: {:?}", res),
    }

    server_task.await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn test_ipc_handshake_unauthorized() {
    let temp = tempfile::tempdir().unwrap();
    let runtime_dir = temp.path();
    let t_path = token_path(runtime_dir);
    let _token = write_ipc_token(&t_path).unwrap();

    let endpoint = SocketEndpoint::Path(runtime_dir.join("telemetry-worker").join("agent.sock"));
    let listener = bind_listener(&endpoint).await.unwrap();

    let server_task = tokio::spawn(async move {
        let mut stream = listener.accept().await.unwrap();
        let frame: Option<ClientFrame> = read_frame(&mut stream).await.unwrap();
        match frame {
            Some(ClientFrame::Hello { .. }) => {
                let rejected = ServerFrame::Rejected {
                    code: "UNAUTHORIZED".into(),
                    message: "Invalid token".into(),
                };
                write_frame(&mut stream, &rejected).await.unwrap();
            }
            _ => panic!("Expected Hello"),
        }
    });

    let res = IpcClient::connect_with_endpoint(&endpoint, "wrong_token").await;
    assert!(matches!(res, Err(IpcError::Unauthorized(_))));

    server_task.await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn test_ipc_handshake_protocol_mismatch() {
    let temp = tempfile::tempdir().unwrap();
    let runtime_dir = temp.path();
    let t_path = token_path(runtime_dir);
    let token = write_ipc_token(&t_path).unwrap();

    let endpoint = SocketEndpoint::Path(runtime_dir.join("telemetry-worker").join("agent.sock"));
    let listener = bind_listener(&endpoint).await.unwrap();

    let server_task = tokio::spawn(async move {
        let mut stream = listener.accept().await.unwrap();
        let frame: Option<ClientFrame> = read_frame(&mut stream).await.unwrap();
        match frame {
            Some(ClientFrame::Hello { .. }) => {
                let welcome = ServerFrame::Welcome {
                    service_version: "0.1.0".into(),
                    protocol_version: 99, // Mismatched version
                };
                write_frame(&mut stream, &welcome).await.unwrap();
            }
            _ => panic!("Expected Hello"),
        }
    });

    let res = IpcClient::connect_with_endpoint(&endpoint, &token).await;
    assert!(matches!(res, Err(IpcError::ProtocolMismatch(_))));

    server_task.await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn test_ipc_subscribe_events() {
    use futures::StreamExt;

    let temp = tempfile::tempdir().unwrap();
    let runtime_dir = temp.path();
    let t_path = token_path(runtime_dir);
    let _token = write_ipc_token(&t_path).unwrap();

    let endpoint = SocketEndpoint::Path(runtime_dir.join("telemetry-worker").join("agent.sock"));
    let listener = bind_listener(&endpoint).await.unwrap();

    let server_task = tokio::spawn(async move {
        let mut stream = listener.accept().await.unwrap();
        let frame: Option<ClientFrame> = read_frame(&mut stream).await.unwrap();
        assert!(matches!(frame, Some(ClientFrame::Hello { .. })));

        let welcome = ServerFrame::Welcome {
            service_version: "0.1.0".into(),
            protocol_version: PROTOCOL_VERSION,
        };
        write_frame(&mut stream, &welcome).await.unwrap();

        let sub_frame: Option<ClientFrame> = read_frame(&mut stream).await.unwrap();
        assert!(matches!(sub_frame, Some(ClientFrame::Subscribe)));

        // Send an Event
        let snapshot = AppSnapshot {
            service_version: "0.1.0".into(),
            run_mode: "PAUSED".into(),
            consent_offer: None,
            accepted_selection: None,
            category_states: CategoryStates {
                system_activity: CategoryState::Sleeping,
                network_activity: CategoryState::Sleeping,
                productivity_basic: CategoryState::Sleeping,
                productivity_screenshots: CategoryState::Sleeping,
                file_activity: CategoryState::Sleeping,
                location: CategoryState::Sleeping,
            },
            work_hours_start: None,
            work_hours_end: None,
            timezone: None,
            worker_name: None,
            organization: None,
            queue_size: 0,
            last_flush: None,
            last_error: None,
            suggested_screen: None,
            tray_available: true,
            pending_withdrawal: false,
            device_identifier_short: "12345678".into(),
            key_fingerprint: None,
            is_enrolled: true,
        };
        let ev = ServerFrame::Event(Event::Snapshot(Box::new(snapshot)));
        write_frame(&mut stream, &ev).await.unwrap();
    });

    let mut stream = IpcClient::subscribe(runtime_dir).await.unwrap();
    let event = stream.next().await.unwrap();
    match event {
        Event::Snapshot(snapshot) => {
            assert_eq!(snapshot.run_mode, "PAUSED");
            assert!(snapshot.is_enrolled);
        }
    }

    server_task.await.unwrap();
}

#[tokio::test]
async fn test_snapshot_no_secrets() {
    let snapshot = AppSnapshot {
        service_version: "0.1.0".into(),
        run_mode: "PAUSED".into(),
        consent_offer: None,
        accepted_selection: None,
        category_states: CategoryStates {
            system_activity: CategoryState::Sleeping,
            network_activity: CategoryState::Sleeping,
            productivity_basic: CategoryState::Sleeping,
            productivity_screenshots: CategoryState::Sleeping,
            file_activity: CategoryState::Sleeping,
            location: CategoryState::Sleeping,
        },
        work_hours_start: None,
        work_hours_end: None,
        timezone: None,
        worker_name: Some("Juan Perez".into()),
        organization: Some("Acme".into()),
        queue_size: 0,
        last_flush: None,
        last_error: None,
        suggested_screen: None,
        tray_available: true,
        pending_withdrawal: false,
        device_identifier_short: "12345678".into(),
        key_fingerprint: Some("0123456789abcdef".into()),
        is_enrolled: true,
    };

    let json = serde_json::to_string(&snapshot).unwrap();
    let lower = json.to_lowercase();
    assert!(
        !lower.contains("\"token\"")
            && !lower.contains("access_token")
            && !lower.contains("refresh_token"),
        "Snapshot must not contain token"
    );
    assert!(
        !lower.contains("\"secret\""),
        "Snapshot must not contain secret"
    );
    assert!(
        !lower.contains("private_key") && !lower.contains("private"),
        "Snapshot must not contain private key"
    );
    assert!(
        !lower.contains("\"pin\"") && !lower.contains("pin_verifier"),
        "Snapshot must not contain PIN"
    );
    assert!(
        !lower.contains("manifest_hash"),
        "Snapshot must not contain manifest hash"
    );
}
