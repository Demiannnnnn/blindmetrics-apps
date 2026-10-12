use crate::api::client::{ApiClient, DeviceInfo};
use crate::background::worker::{handle_revocation, EngineCommand, EngineHandle};
use crate::config::WorkerConfig;
use crate::consent::{submit_decision, ConsentAction, ConsentOffer};
use crate::core::buffer::MetricsBuffer;
use crate::credentials::CredentialStore;
use crate::enrollment::{
    commit_enrollment, enroll, enrollment_error_message, parse_enrollment_input, EnrollmentError,
    PendingEnrollment, Verification,
};
use crate::io::transport::OfflineQueue;
use crate::session::with_access_token;
use crate::state::{RunMode, WorkerSelf};
use agent_ipc::framing::{read_frame, write_frame};
use agent_ipc::transport::{
    bind_listener, socket_endpoint, token_path, verify_token, write_ipc_token, SocketEndpoint,
};
use agent_ipc::{
    AppSnapshot, CategoryState, CategoryStates, ClientFrame, ConsentOfferSnapshot, DiagnosticsInfo,
    Event, ListenerTrait as _, Request, Response, ServerFrame, Stream, VerificationResult,
    WindowScreenHint, PROTOCOL_VERSION,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, Notify, RwLock};

/// How long a tray-initiated *Salir* confirmation stays valid (PROC-04).
pub const QUIT_CONFIRM_TTL: Duration = Duration::from_secs(30);

/// A quit request initiated by the in-process tray and awaiting the window's confirmation.
/// One-shot: consumed by the first valid `ConfirmQuit`, or dropped on cancel/expiry.
struct PendingQuit {
    nonce: String,
    expires_at: Instant,
}

pub struct IpcServerContext {
    pub config: WorkerConfig,
    pub cred_store: Arc<dyn CredentialStore>,
    pub buffer: Arc<MetricsBuffer>,
    pub engine: EngineHandle,
    pub api_base_url: String,
    pub start_time: Instant,
    pub tray_available: Arc<AtomicBool>,
    pub suggested_screen: Arc<RwLock<Option<WindowScreenHint>>>,
    pub pending_enrollment: Arc<Mutex<Option<PendingEnrollment>>>,
    pub subscribers: Arc<Mutex<Vec<mpsc::Sender<Event>>>>,
    pub shutdown_notify: Arc<Notify>,
    pending_quit: Arc<Mutex<Option<PendingQuit>>>,
}

impl IpcServerContext {
    pub async fn build_snapshot(&self) -> AppSnapshot {
        let engine_state = self.engine.state.borrow().clone();
        let run_mode_str = match &engine_state.run_mode {
            RunMode::Unenrolled => "UNENROLLED",
            RunMode::AwaitingConsent { .. } => "AWAITING_CONSENT",
            RunMode::Collecting(_) => "COLLECTING",
            RunMode::Paused => "PAUSED",
            RunMode::EmployerSuspended { .. } => "EMPLOYER_SUSPENDED",
            RunMode::Halted(_) => "HALTED",
        }
        .to_string();

        let pending = self.pending_enrollment.lock().await;
        let consent_offer = if let Some(p) = pending.as_ref() {
            Some(ConsentOfferSnapshot {
                organization: Some(p.resp.organization.clone()),
                consent_version: p.resp.consent_version,
                requested_flags: p.resp.consent_flags,
                descriptions: p.resp.consent_descriptions.clone(),
                is_update: false,
                previous_selection: None,
            })
        } else if let Some(ws) = engine_state.worker_self.as_ref() {
            let prev = self.cred_store.get_accepted_flags();
            let is_update = ws
                .accepted_consent_version
                .map(|v| v < ws.consent_version)
                .unwrap_or(false);
            Some(ConsentOfferSnapshot {
                organization: None,
                consent_version: ws.consent_version,
                requested_flags: ws.requested_consent_flags,
                descriptions: ws.consent_descriptions.clone(),
                is_update,
                previous_selection: prev,
            })
        } else {
            None
        };

        let accepted_selection = self.cred_store.get_accepted_flags();

        let category_states = compute_category_states(
            engine_state.worker_self.as_ref(),
            accepted_selection.as_ref(),
            &engine_state.run_mode,
        );

        let ws = engine_state.worker_self.as_ref();
        let work_hours_start = ws.and_then(|w| w.work_hours_start.clone());
        let work_hours_end = ws.and_then(|w| w.work_hours_end.clone());
        let timezone = ws.and_then(|w| w.timezone.clone());

        // Queue size from offline database or 0
        let queue_size = OfflineQueue::open_default()
            .map(|q| q.count().unwrap_or(0))
            .unwrap_or(0);

        let suggested_screen = *self.suggested_screen.read().await;
        let tray_available = self.tray_available.load(Ordering::Relaxed);
        let pending_withdrawal = self.cred_store.get_pending_withdrawal();

        let device_identifier_short = if self.config.device_identifier.len() >= 8 {
            self.config.device_identifier[..8].to_string()
        } else {
            self.config.device_identifier.clone()
        };

        let key_fingerprint = self.cred_store.get_pinned_fingerprint();
        let is_enrolled = self.cred_store.get_access_token().is_some();

        AppSnapshot {
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            run_mode: run_mode_str,
            consent_offer,
            accepted_selection,
            category_states,
            work_hours_start,
            work_hours_end,
            timezone,
            worker_name: None,
            organization: None,
            queue_size,
            last_flush: None,
            last_error: engine_state.last_error,
            suggested_screen,
            tray_available,
            pending_withdrawal,
            device_identifier_short,
            key_fingerprint,
            is_enrolled,
        }
    }

    pub async fn broadcast_snapshot(&self) {
        let snapshot = self.build_snapshot().await;
        let event = Event::Snapshot(Box::new(snapshot));

        let mut subs = self.subscribers.lock().await;
        let mut active = Vec::new();
        for tx in subs.drain(..) {
            if tx.send(event.clone()).await.is_ok() {
                active.push(tx);
            }
        }
        *subs = active;
    }

    pub async fn set_suggested_screen(&self, hint: Option<WindowScreenHint>) {
        *self.suggested_screen.write().await = hint;
        self.broadcast_snapshot().await;
    }

    pub async fn has_subscribers(&self) -> bool {
        !self.subscribers.lock().await.is_empty()
    }

    async fn broadcast_event(&self, event: Event) {
        let mut subs = self.subscribers.lock().await;
        let mut active = Vec::new();
        for tx in subs.drain(..) {
            if tx.send(event.clone()).await.is_ok() {
                active.push(tx);
            }
        }
        *subs = active;
    }

    /// Called ONLY by the in-process tray when the user picks *Salir* (PROC-04). Asks the
    /// subscribed window to show the confirmation dialog by pushing `Event::QuitRequested` with a
    /// fresh one-shot nonce valid for [`QUIT_CONFIRM_TTL`]. Returns `false` (nothing pending) when
    /// no window is subscribed, so the tray can fall back to its own confirmation.
    pub async fn request_quit(self: &Arc<Self>) -> bool {
        self.request_quit_with_ttl(QUIT_CONFIRM_TTL).await
    }

    async fn request_quit_with_ttl(self: &Arc<Self>, ttl: Duration) -> bool {
        if !self.has_subscribers().await {
            return false;
        }
        let nonce = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        *self.pending_quit.lock().await = Some(PendingQuit {
            nonce: nonce.clone(),
            expires_at: Instant::now() + ttl,
        });
        self.broadcast_event(Event::QuitRequested {
            nonce: nonce.clone(),
            expires_in_secs: ttl.as_secs(),
        })
        .await;

        // Expire the request so a stale dialog can never stop the service later.
        let ctx = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(ttl).await;
            let mut pending = ctx.pending_quit.lock().await;
            if pending.as_ref().is_some_and(|p| p.nonce == nonce) {
                *pending = None;
                drop(pending);
                ctx.broadcast_event(Event::QuitCancelled).await;
            }
        });
        true
    }

    /// Validates the window's answer to a tray-initiated quit request. Only a pending, unexpired
    /// request with a matching nonce stops the service; the request is consumed (one-shot).
    async fn confirm_quit(&self, nonce: &str) -> Response {
        let mut pending = self.pending_quit.lock().await;
        let Some(current) = pending.as_ref() else {
            return not_allowed(
                "No hay una solicitud de salida pendiente iniciada desde la bandeja",
            );
        };
        if Instant::now() >= current.expires_at {
            *pending = None;
            return not_allowed(
                "La solicitud de salida expiró; vuelve a elegir Salir en la bandeja",
            );
        }
        if !verify_token(&current.nonce, nonce) {
            return not_allowed("Código de confirmación de salida inválido");
        }
        *pending = None;
        drop(pending);
        tracing::info!("Quit confirmed by window for a tray-initiated request");
        self.shutdown_notify.notify_waiters();
        Response::ConfirmQuit
    }

    async fn cancel_quit(&self, nonce: &str) -> Response {
        let mut pending = self.pending_quit.lock().await;
        if pending
            .as_ref()
            .is_some_and(|p| verify_token(&p.nonce, nonce))
        {
            *pending = None;
            drop(pending);
            self.broadcast_event(Event::QuitCancelled).await;
        }
        Response::CancelQuit
    }

    /// Remaining validity of the pending quit request, if any (for late-subscribing windows).
    async fn pending_quit_event(&self) -> Option<Event> {
        let pending = self.pending_quit.lock().await;
        let p = pending.as_ref()?;
        let remaining = p.expires_at.checked_duration_since(Instant::now())?;
        Some(Event::QuitRequested {
            nonce: p.nonce.clone(),
            expires_in_secs: remaining.as_secs().max(1),
        })
    }
}

fn not_allowed(message: &str) -> Response {
    Response::Error {
        code: "NOT_ALLOWED".into(),
        message: message.into(),
        retry_after_secs: None,
    }
}

fn compute_category_states(
    worker_self: Option<&WorkerSelf>,
    accepted: Option<&telemetry_crypto_core::ConsentFlags>,
    run_mode: &RunMode,
) -> CategoryStates {
    let requested = worker_self
        .map(|w| w.requested_consent_flags)
        .unwrap_or_default();
    let accepted = accepted.copied().unwrap_or_default();

    let is_collecting = matches!(run_mode, RunMode::Collecting(_));

    let cat_state = |req: bool, acc: bool| {
        if !req {
            CategoryState::NotRequested
        } else if !acc {
            CategoryState::NotAccepted
        } else if is_collecting {
            CategoryState::Active
        } else {
            CategoryState::Sleeping
        }
    };

    CategoryStates {
        system_activity: cat_state(requested.system_activity, accepted.system_activity),
        network_activity: cat_state(requested.network_activity, accepted.network_activity),
        productivity_basic: cat_state(requested.productivity_basic, accepted.productivity_basic),
        productivity_screenshots: cat_state(
            requested.productivity_screenshots,
            accepted.productivity_screenshots,
        ),
        file_activity: cat_state(requested.file_activity, accepted.file_activity),
        location: cat_state(requested.location, accepted.location),
    }
}

pub struct IpcServer {
    pub context: Arc<IpcServerContext>,
    pub runtime_dir: PathBuf,
    pub token: String,
}

impl IpcServer {
    pub fn new(
        config: WorkerConfig,
        cred_store: Arc<dyn CredentialStore>,
        buffer: Arc<MetricsBuffer>,
        engine: EngineHandle,
        runtime_dir: PathBuf,
        tray_available: Arc<AtomicBool>,
        shutdown_notify: Arc<Notify>,
    ) -> std::io::Result<Self> {
        let t_path = token_path(&runtime_dir);
        let token = write_ipc_token(&t_path)?;

        let context = Arc::new(IpcServerContext {
            api_base_url: config.api_base_url.clone(),
            config,
            cred_store,
            buffer,
            engine,
            start_time: Instant::now(),
            tray_available,
            suggested_screen: Arc::new(RwLock::new(None)),
            pending_enrollment: Arc::new(Mutex::new(None)),
            subscribers: Arc::new(Mutex::new(Vec::new())),
            shutdown_notify,
            pending_quit: Arc::new(Mutex::new(None)),
        });

        Ok(Self {
            context,
            runtime_dir,
            token,
        })
    }

    pub async fn run(self: Arc<Self>) -> std::io::Result<()> {
        let endpoint = socket_endpoint(&self.runtime_dir);
        let listener = bind_listener(&endpoint).await?;
        tracing::info!("IPC server listening at {:?}", endpoint);

        // Spawn debounced snapshot watcher
        let ctx = self.context.clone();
        tokio::spawn(async move {
            let mut state_rx = ctx.engine.state.clone();
            loop {
                if state_rx.changed().await.is_err() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
                ctx.broadcast_snapshot().await;
            }
        });

        // Connection concurrency limiter (RF-AGENT-10-2: max 16 concurrent connections)
        let conn_limiter = Arc::new(tokio::sync::Semaphore::new(16));

        loop {
            tokio::select! {
                _ = self.context.shutdown_notify.notified() => {
                    tracing::info!("IPC server received shutdown notification");
                    break;
                }
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok(mut stream) => {
                            match conn_limiter.clone().try_acquire_owned() {
                                Ok(permit) => {
                                    let server = self.clone();
                                    tokio::spawn(async move {
                                        let _permit = permit;
                                        if let Err(e) = server.handle_connection(stream).await {
                                            tracing::debug!("IPC client connection ended: {}", e);
                                        }
                                    });
                                }
                                Err(_) => {
                                    // RF-AGENT-10-2: Extra connections get Rejected { code: "BUSY" } and are closed
                                    tokio::spawn(async move {
                                        let rejected = ServerFrame::Rejected {
                                            code: "BUSY".into(),
                                            message: "Demasiadas conexiones simultáneas (límite 16)".into(),
                                        };
                                        let _ = write_frame(&mut stream, &rejected).await;
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("IPC accept error: {}", e);
                        }
                    }
                }
            }
        }

        // Clean up socket file and token file on shutdown (RF-AGENT-10-12)
        let _ = std::fs::remove_file(token_path(&self.runtime_dir));
        if let SocketEndpoint::Path(p) = socket_endpoint(&self.runtime_dir) {
            let _ = std::fs::remove_file(p);
        }
        tracing::info!("IPC server resources cleaned up");

        Ok(())
    }

    async fn handle_connection(
        self: Arc<Self>,
        mut stream: Stream,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 1. Handshake: First frame must be Hello within 5 seconds (RF-AGENT-10-2)
        let handshake_res = tokio::time::timeout(
            Duration::from_secs(5),
            read_frame::<_, ClientFrame>(&mut stream),
        )
        .await;

        let frame = match handshake_res {
            Ok(Ok(frame)) => frame,
            Ok(Err(e)) => {
                // RF-AGENT-10-2: On an oversized/invalid frame during handshake, send BAD_FRAME before closing
                let rejected = ServerFrame::Rejected {
                    code: "BAD_FRAME".into(),
                    message: e.to_string(),
                };
                let _ = write_frame(&mut stream, &rejected).await;
                return Ok(());
            }
            Err(_timeout) => {
                // Handshake timed out: close connection
                tracing::debug!("IPC handshake timed out after 5s");
                return Ok(());
            }
        };

        let Some(ClientFrame::Hello {
            protocol_version,
            token,
        }) = frame
        else {
            let rejected = ServerFrame::Rejected {
                code: "HANDSHAKE_REQUIRED".into(),
                message: "First frame must be Hello".into(),
            };
            write_frame(&mut stream, &rejected).await?;
            return Ok(());
        };

        if protocol_version != PROTOCOL_VERSION {
            let rejected = ServerFrame::Rejected {
                code: "PROTOCOL_MISMATCH".into(),
                message: format!(
                    "Client protocol version {} is incompatible with service version {}",
                    protocol_version, PROTOCOL_VERSION
                ),
            };
            write_frame(&mut stream, &rejected).await?;
            return Ok(());
        }

        if !verify_token(&self.token, &token) {
            let rejected = ServerFrame::Rejected {
                code: "UNAUTHORIZED".into(),
                message: "Invalid authentication token".into(),
            };
            write_frame(&mut stream, &rejected).await?;
            return Ok(());
        }

        // Welcome
        let welcome = ServerFrame::Welcome {
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: PROTOCOL_VERSION,
        };
        write_frame(&mut stream, &welcome).await?;

        // 2. Process requests or subscribe
        loop {
            tokio::select! {
                _ = self.context.shutdown_notify.notified() => {
                    break;
                }
                frame_res = read_frame::<_, ClientFrame>(&mut stream) => {
                    match frame_res {
                        Ok(Some(ClientFrame::Call { id, request })) => {
                            let response = self.dispatch_request(request).await;
                            let reply = ServerFrame::Reply { id, response };
                            write_frame(&mut stream, &reply).await?;
                        }
                        Ok(Some(ClientFrame::Subscribe)) => {
                            // Register subscription
                            let (tx, mut rx) = mpsc::channel::<Event>(32);
                            {
                                let mut subs = self.context.subscribers.lock().await;
                                subs.push(tx);
                            }
                            // Send initial snapshot immediately
                            let initial = self.context.build_snapshot().await;
                            write_frame(&mut stream, &ServerFrame::Event(Event::Snapshot(Box::new(initial)))).await?;

                            // A window opened while a tray quit request is pending still sees it.
                            if let Some(ev) = self.context.pending_quit_event().await {
                                write_frame(&mut stream, &ServerFrame::Event(ev)).await?;
                            }

                            // Forward events until client disconnects
                            while let Some(ev) = rx.recv().await {
                                if write_frame(&mut stream, &ServerFrame::Event(ev)).await.is_err() {
                                    break;
                                }
                            }
                            break;
                        }
                        Ok(Some(ClientFrame::Hello { .. })) => {
                            let rejected = ServerFrame::Rejected {
                                code: "ALREADY_AUTHENTICATED".into(),
                                message: "Session already authenticated".into(),
                            };
                            write_frame(&mut stream, &rejected).await?;
                        }
                        Ok(None) => break, // EOF
                        Err(e) => {
                            // RF-AGENT-10-2: On an oversized/invalid frame, send BAD_FRAME before closing
                            let rejected = ServerFrame::Rejected {
                                code: "BAD_FRAME".into(),
                                message: e.to_string(),
                            };
                            let _ = write_frame(&mut stream, &rejected).await;
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn dispatch_request(&self, request: Request) -> Response {
        match request {
            Request::Snapshot => {
                let snapshot = self.context.build_snapshot().await;
                Response::Snapshot {
                    snapshot: Box::new(snapshot),
                }
            }
            Request::Enroll { qr_payload, code } => {
                let with_qr = qr_payload.is_some();
                let qr_text = qr_payload.unwrap_or_default();
                let code_text = code.unwrap_or_default();

                let input = match parse_enrollment_input(
                    with_qr,
                    &qr_text,
                    &code_text,
                    &self.context.api_base_url,
                ) {
                    Ok(input) => input,
                    Err(msg) => {
                        return Response::Error {
                            code: "INVALID_INPUT".into(),
                            message: msg,
                            retry_after_secs: None,
                        };
                    }
                };

                let api = match ApiClient::new(self.context.api_base_url.clone()) {
                    Ok(api) => api,
                    Err(e) => {
                        return Response::Error {
                            code: "API_ERROR".into(),
                            message: format!("Cliente HTTP no disponible: {}", e),
                            retry_after_secs: None,
                        };
                    }
                };

                let device_info = DeviceInfo::collect();
                let enroll_res = enroll(
                    &api,
                    &input,
                    &self.context.config.device_identifier,
                    &device_info,
                )
                .await;

                match enroll_res {
                    Ok(pending) => match &pending.verification {
                        Verification::NeedsSafetyCode(safety_code) => {
                            let sc = safety_code.clone();
                            *self.context.pending_enrollment.lock().await = Some(pending);
                            self.context.broadcast_snapshot().await;
                            Response::Enroll {
                                result: VerificationResult::NeedsSafetyCode { safety_code: sc },
                            }
                        }
                        Verification::Verified => {
                            let mut config = self.context.config.clone();
                            if let Err(e) = commit_enrollment(
                                &pending,
                                self.context.cred_store.as_ref(),
                                &mut config,
                            ) {
                                return Response::Error {
                                    code: "COMMIT_ERROR".into(),
                                    message: format!("No se pudo guardar la vinculación: {}", e),
                                    retry_after_secs: None,
                                };
                            }
                            *self.context.pending_enrollment.lock().await = None;
                            let _ = self.context.engine.commands.send(EngineCommand::Enrolled {
                                api_base_url: self.context.api_base_url.clone(),
                            });
                            self.context.broadcast_snapshot().await;
                            Response::Enroll {
                                result: VerificationResult::Verified,
                            }
                        }
                    },
                    Err(err) => {
                        let retry = match &err {
                            EnrollmentError::Api(crate::errors::AgentError::Api {
                                retry_after_seconds,
                                ..
                            }) => *retry_after_seconds,
                            _ => None,
                        };
                        Response::Error {
                            code: "ENROLLMENT_ERROR".into(),
                            message: enrollment_error_message(&err),
                            retry_after_secs: retry,
                        }
                    }
                }
            }
            Request::ConfirmSafetyCode { matches } => {
                let pending = self.context.pending_enrollment.lock().await.take();
                let Some(pending) = pending else {
                    return Response::Error {
                        code: "NO_PENDING_ENROLLMENT".into(),
                        message: "No hay ninguna vinculación pendiente de verificación".into(),
                        retry_after_secs: None,
                    };
                };

                if matches {
                    let mut config = self.context.config.clone();
                    if let Err(e) =
                        commit_enrollment(&pending, self.context.cred_store.as_ref(), &mut config)
                    {
                        return Response::Error {
                            code: "COMMIT_ERROR".into(),
                            message: format!("No se pudo guardar la vinculación: {}", e),
                            retry_after_secs: None,
                        };
                    }
                    let _ = self.context.engine.commands.send(EngineCommand::Enrolled {
                        api_base_url: self.context.api_base_url.clone(),
                    });
                    self.context.broadcast_snapshot().await;
                    Response::ConfirmSafetyCode
                } else {
                    let _ = self.context.cred_store.wipe();
                    self.context.broadcast_snapshot().await;
                    Response::Error {
                        code: "VERIFICATION_FAILED".into(),
                        message: "Código de seguridad no coincide. Se canceló la vinculación."
                            .into(),
                        retry_after_secs: None,
                    }
                }
            }
            Request::ConsentDecision { accept } => {
                let api = match ApiClient::new(self.context.api_base_url.clone()) {
                    Ok(a) => a,
                    Err(e) => {
                        return Response::Error {
                            code: "API_ERROR".into(),
                            message: e.to_string(),
                            retry_after_secs: None,
                        };
                    }
                };

                // Build offer from /workers/me state and pinned key
                let engine_state = self.context.engine.state.borrow().clone();
                let Some(ws) = engine_state.worker_self else {
                    return Response::Error {
                        code: "NOT_SYNCED".into(),
                        message: "El estado del servidor no está disponible aún".into(),
                        retry_after_secs: None,
                    };
                };

                let pinned = self
                    .context
                    .cred_store
                    .get_pinned_admin_public_key()
                    .unwrap_or_default();
                let prev = self.context.cred_store.get_accepted_flags();

                let offer = match ConsentOffer::from_worker_self(&ws, &pinned, prev) {
                    Ok(o) => o,
                    Err(e) => {
                        return Response::Error {
                            code: "OFFER_ERROR".into(),
                            message: format!(
                                "Error al preparar la oferta de consentimiento: {}",
                                e
                            ),
                            retry_after_secs: None,
                        };
                    }
                };

                let action = match accept {
                    Some(flags) => ConsentAction::Accept(flags),
                    None => ConsentAction::Decline,
                };

                match submit_decision(&api, self.context.cred_store.as_ref(), &offer, action).await
                {
                    Ok(_) => {
                        let _ = self.context.engine.commands.send(EngineCommand::SyncNow);
                        self.context.broadcast_snapshot().await;
                        Response::ConsentDecision
                    }
                    Err(e) => Response::Error {
                        code: "DECISION_ERROR".into(),
                        message: format!("No se pudo registrar la decisión: {}", e),
                        retry_after_secs: None,
                    },
                }
            }
            Request::Withdraw => {
                let _ = self.context.engine.commands.send(EngineCommand::Withdraw);
                self.context.broadcast_snapshot().await;
                Response::Withdraw
            }
            Request::DataRequest { categories } => {
                let api = match ApiClient::new(self.context.api_base_url.clone()) {
                    Ok(a) => a,
                    Err(e) => {
                        return Response::Error {
                            code: "API_ERROR".into(),
                            message: e.to_string(),
                            retry_after_secs: None,
                        };
                    }
                };
                let creds = self.context.cred_store.clone();
                let default_categories = ["system", "network", "productivity", "files", "location"];
                let cat_slice: Vec<&str> = match &categories {
                    Some(cats) => cats.iter().map(|s| s.as_str()).collect(),
                    None => default_categories.to_vec(),
                };

                let res = with_access_token(&api, creds.as_ref(), |token| {
                    let api = api.clone();
                    let cats = cat_slice.clone();
                    async move { api.post_data_request(&token, &cats).await }
                })
                .await;

                match res {
                    Ok(resp) => Response::DataRequest {
                        request_id: resp.request_id,
                        status: resp.status,
                        estimated_completion: resp.estimated_completion,
                    },
                    Err(e) => Response::Error {
                        code: "ARCO_ERROR".into(),
                        message: format!("Error en solicitud ARCO: {}", e),
                        retry_after_secs: None,
                    },
                }
            }
            Request::DataDeletion { reason, .. } => {
                let api = match ApiClient::new(self.context.api_base_url.clone()) {
                    Ok(a) => a,
                    Err(e) => {
                        return Response::Error {
                            code: "API_ERROR".into(),
                            message: e.to_string(),
                            retry_after_secs: None,
                        };
                    }
                };
                let creds = self.context.cred_store.clone();
                let res = with_access_token(&api, creds.as_ref(), |token| {
                    let api = api.clone();
                    let r = reason.clone();
                    async move { api.post_data_deletion(&token, r.as_deref()).await }
                })
                .await;

                match res {
                    Ok(resp) => Response::DataDeletion {
                        request_id: resp.request_id,
                        status: resp.status,
                        message: resp.message,
                    },
                    Err(e) => Response::Error {
                        code: "ARCO_ERROR".into(),
                        message: format!("Error en supresión ARCO: {}", e),
                        retry_after_secs: None,
                    },
                }
            }
            Request::Unlink { confirm } => {
                if confirm != "DESVINCULAR" {
                    return Response::Error {
                        code: "CONFIRMATION_REQUIRED".into(),
                        message: "Debes escribir DESVINCULAR para confirmar".into(),
                        retry_after_secs: None,
                    };
                }

                let mut queue = OfflineQueue::open_default()
                    .unwrap_or_else(|_| OfflineQueue::open_in_memory().unwrap());
                handle_revocation(
                    self.context.cred_store.as_ref(),
                    &self.context.buffer,
                    &mut queue,
                );
                let _ = self.context.engine.commands.send(EngineCommand::SyncNow);
                self.context.broadcast_snapshot().await;
                Response::Unlink
            }
            Request::Diagnostics => {
                let uptime_seconds = self.context.start_time.elapsed().as_secs();
                let queue = OfflineQueue::open_default();
                let telemetry_queue_items =
                    queue.as_ref().map(|q| q.count().unwrap_or(0)).unwrap_or(0);
                let telemetry_queue_bytes = queue
                    .as_ref()
                    .map(|q| q.size_bytes().unwrap_or(0))
                    .unwrap_or(0);

                let device_id_short = if self.context.config.device_identifier.len() >= 8 {
                    self.context.config.device_identifier[..8].to_string()
                } else {
                    self.context.config.device_identifier.clone()
                };

                let key_fingerprint = self.context.cred_store.get_pinned_fingerprint();
                let tray_available = self.context.tray_available.load(Ordering::Relaxed);

                let diag = DiagnosticsInfo {
                    service_version: env!("CARGO_PKG_VERSION").to_string(),
                    uptime_seconds,
                    api_base_url: self.context.api_base_url.clone(),
                    server_time_offset_seconds: None,
                    pending_marks: 0,
                    telemetry_queue_items,
                    telemetry_queue_bytes,
                    last_successful_send: None,
                    device_id_short,
                    key_fingerprint,
                    tray_available,
                    os_permissions: "OK".into(),
                };
                Response::Diagnostics {
                    diagnostics: Box::new(diag),
                }
            }
            Request::OpenWindowHint { screen } => {
                *self.context.suggested_screen.write().await = Some(screen);
                self.context.broadcast_snapshot().await;
                Response::OpenWindowHint
            }
            Request::ConfirmQuit { nonce } => self.context.confirm_quit(&nonce).await,
            Request::CancelQuit { nonce } => self.context.cancel_quit(&nonce).await,
            Request::Shutdown => Response::Error {
                code: "NOT_ALLOWED".into(),
                message:
                    "SHUTDOWN no está permitido a través de IPC; solo la bandeja del sistema puede detener el servicio"
                        .into(),
                retry_after_secs: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::worker::execute_withdraw;
    use crate::background::BackgroundEngine;
    use crate::credentials::InMemoryCredentialStore;
    use agent_ipc::{connect_stream, IpcClient, StreamExt};
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_ipc_server_lifecycle_and_calls() {
        let dir = tempdir().expect("tempdir");
        let runtime_dir = dir.path().to_path_buf();

        let config = WorkerConfig::load_or_init_at(runtime_dir.join("config.json"));
        let cred_store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
        let buffer = Arc::new(MetricsBuffer::new(100));

        let engine = BackgroundEngine::new(config.clone(), buffer.clone(), cred_store.clone())
            .expect("engine");
        let rt_handle = tokio::runtime::Handle::current();
        let engine_handle = engine.handle(rt_handle);
        let tray_available = Arc::new(AtomicBool::new(true));
        let shutdown_notify = Arc::new(Notify::new());

        let server = Arc::new(
            IpcServer::new(
                config,
                cred_store.clone(),
                buffer,
                engine_handle,
                runtime_dir.clone(),
                tray_available.clone(),
                shutdown_notify.clone(),
            )
            .expect("server new"),
        );

        let server_clone = server.clone();
        let server_task = tokio::spawn(async move {
            let _ = server_clone.run().await;
        });

        // Wait for server to start listening
        tokio::time::sleep(Duration::from_millis(150)).await;

        // 1. Connect client
        let client = IpcClient::connect(&runtime_dir)
            .await
            .expect("client connect");

        // 2. Request snapshot
        let res = client.call(Request::Snapshot).await.expect("call snapshot");
        let snapshot = match res {
            Response::Snapshot { snapshot } => snapshot,
            other => panic!("Expected Snapshot, got {:?}", other),
        };
        assert_eq!(snapshot.run_mode, "UNENROLLED");
        assert!(snapshot.tray_available);
        assert!(!snapshot.is_enrolled);

        // 3. Request diagnostics
        let res = client
            .call(Request::Diagnostics)
            .await
            .expect("call diagnostics");
        match res {
            Response::Diagnostics { diagnostics } => {
                assert!(diagnostics.tray_available);
                assert_eq!(diagnostics.os_permissions, "OK");
            }
            other => panic!("Expected Diagnostics, got {:?}", other),
        }

        // 4. Request window hint
        let res = client
            .call(Request::OpenWindowHint {
                screen: WindowScreenHint::Telemetry,
            })
            .await
            .expect("call hint");
        assert_eq!(res, Response::OpenWindowHint);

        let res = client.call(Request::Snapshot).await.expect("snapshot 2");
        if let Response::Snapshot { snapshot } = res {
            assert_eq!(snapshot.suggested_screen, Some(WindowScreenHint::Telemetry));
        }

        // 5. Unlink requires confirmation
        let res = client
            .call(Request::Unlink {
                confirm: "NO".into(),
            })
            .await
            .expect("unlink fail");
        assert!(matches!(res, Response::Error { ref code, .. } if code == "CONFIRMATION_REQUIRED"));

        let res = client
            .call(Request::Unlink {
                confirm: "DESVINCULAR".into(),
            })
            .await
            .expect("unlink ok");
        assert_eq!(res, Response::Unlink);

        // 6. Withdraw request
        let res = client.call(Request::Withdraw).await.expect("withdraw");
        assert_eq!(res, Response::Withdraw);

        // 7. Subscribe to events
        let mut sub = IpcClient::subscribe(&runtime_dir).await.expect("subscribe");
        let first_ev = sub.next().await.expect("first event");
        assert!(matches!(first_ev, Event::Snapshot(_)));

        // 8. Shutdown over IPC is refused with NOT_ALLOWED and service continues running (RF-AGENT-10-1)
        let res = client.call(Request::Shutdown).await.expect("shutdown call");
        assert!(
            matches!(res, Response::Error { ref code, .. } if code == "NOT_ALLOWED"),
            "Expected NOT_ALLOWED error on IPC shutdown, got {:?}",
            res
        );

        // Assert service keeps running after refused shutdown
        let post_shutdown_snap = client
            .call(Request::Snapshot)
            .await
            .expect("snapshot after refused shutdown");
        assert!(matches!(post_shutdown_snap, Response::Snapshot { .. }));

        // In-process notify triggers actual shutdown
        shutdown_notify.notify_waiters();

        let _ = tokio::time::timeout(Duration::from_secs(2), server_task).await;

        // Token file and socket should be removed
        assert!(!token_path(&runtime_dir).exists());
        if let SocketEndpoint::Path(p) = socket_endpoint(&runtime_dir) {
            assert!(!p.exists());
        }
    }

    struct TestServer {
        server: Arc<IpcServer>,
        runtime_dir: PathBuf,
        server_task: tokio::task::JoinHandle<()>,
        _dir: tempfile::TempDir,
    }

    async fn start_test_server() -> TestServer {
        let dir = tempdir().expect("tempdir");
        let runtime_dir = dir.path().to_path_buf();
        let config = WorkerConfig::load_or_init_at(runtime_dir.join("config.json"));
        let cred_store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
        let buffer = Arc::new(MetricsBuffer::new(10));
        let engine = BackgroundEngine::new(config.clone(), buffer.clone(), cred_store.clone())
            .expect("engine");
        let engine_handle = engine.handle(tokio::runtime::Handle::current());
        let server = Arc::new(
            IpcServer::new(
                config,
                cred_store,
                buffer,
                engine_handle,
                runtime_dir.clone(),
                Arc::new(AtomicBool::new(true)),
                Arc::new(Notify::new()),
            )
            .expect("server new"),
        );
        let server_clone = server.clone();
        let server_task = tokio::spawn(async move {
            let _ = server_clone.run().await;
        });
        tokio::time::sleep(Duration::from_millis(150)).await;
        TestServer {
            server,
            runtime_dir,
            server_task,
            _dir: dir,
        }
    }

    async fn next_non_snapshot<S: agent_ipc::EventStream<Item = Event> + Unpin>(
        sub: &mut S,
    ) -> Event {
        loop {
            let ev = tokio::time::timeout(Duration::from_secs(2), sub.next())
                .await
                .expect("event in time")
                .expect("event");
            if !matches!(ev, Event::Snapshot(_)) {
                return ev;
            }
        }
    }

    #[tokio::test]
    async fn test_quit_unsolicited_confirm_is_not_allowed() {
        // PROC-04: no IPC client can stop the service unless the tray started a quit request.
        let ts = start_test_server().await;
        let client = IpcClient::connect(&ts.runtime_dir).await.expect("connect");

        let res = client
            .call(Request::ConfirmQuit {
                nonce: "guess".into(),
            })
            .await
            .expect("call");
        assert!(matches!(res, Response::Error { ref code, .. } if code == "NOT_ALLOWED"));

        // Still running.
        assert!(matches!(
            client.call(Request::Snapshot).await.expect("snapshot"),
            Response::Snapshot { .. }
        ));
        assert!(!ts.server_task.is_finished());

        // No window subscribed: the tray gets `false` and nothing becomes pending.
        assert!(!ts.server.context.request_quit().await);
        let res = client
            .call(Request::ConfirmQuit {
                nonce: "guess".into(),
            })
            .await
            .expect("call");
        assert!(matches!(res, Response::Error { ref code, .. } if code == "NOT_ALLOWED"));

        ts.server.context.shutdown_notify.notify_waiters();
        let _ = ts.server_task.await;
    }

    #[tokio::test]
    async fn test_quit_tray_request_confirmed_once_by_window() {
        let ts = start_test_server().await;
        let client = IpcClient::connect(&ts.runtime_dir).await.expect("connect");
        let mut sub = IpcClient::subscribe(&ts.runtime_dir)
            .await
            .expect("subscribe");
        let _ = sub.next().await.expect("initial snapshot");

        assert!(ts.server.context.request_quit().await);
        let nonce = match next_non_snapshot(&mut sub).await {
            Event::QuitRequested {
                nonce,
                expires_in_secs,
            } => {
                assert_eq!(expires_in_secs, 30);
                nonce
            }
            other => panic!("Expected QuitRequested, got {:?}", other),
        };

        // Wrong nonce: refused, request stays pending.
        let res = client
            .call(Request::ConfirmQuit {
                nonce: "wrong".into(),
            })
            .await
            .expect("call");
        assert!(matches!(res, Response::Error { ref code, .. } if code == "NOT_ALLOWED"));
        assert!(!ts.server_task.is_finished());

        // Plain SHUTDOWN stays refused even while a quit request is pending.
        let res = client.call(Request::Shutdown).await.expect("call");
        assert!(matches!(res, Response::Error { ref code, .. } if code == "NOT_ALLOWED"));

        // Correct nonce: the service stops.
        let res = client
            .call(Request::ConfirmQuit {
                nonce: nonce.clone(),
            })
            .await
            .expect("call");
        assert_eq!(res, Response::ConfirmQuit);
        let _ = tokio::time::timeout(Duration::from_secs(2), ts.server_task).await;

        // One-shot: the nonce cannot be replayed.
        assert!(matches!(
            ts.server.context.confirm_quit(&nonce).await,
            Response::Error { ref code, .. } if code == "NOT_ALLOWED"
        ));
    }

    #[tokio::test]
    async fn test_quit_request_expires() {
        let ts = start_test_server().await;
        let client = IpcClient::connect(&ts.runtime_dir).await.expect("connect");
        let mut sub = IpcClient::subscribe(&ts.runtime_dir)
            .await
            .expect("subscribe");
        let _ = sub.next().await.expect("initial snapshot");

        assert!(
            ts.server
                .context
                .request_quit_with_ttl(Duration::from_millis(200))
                .await
        );
        let nonce = match next_non_snapshot(&mut sub).await {
            Event::QuitRequested { nonce, .. } => nonce,
            other => panic!("Expected QuitRequested, got {:?}", other),
        };
        assert_eq!(next_non_snapshot(&mut sub).await, Event::QuitCancelled);

        let res = client
            .call(Request::ConfirmQuit { nonce })
            .await
            .expect("call");
        assert!(matches!(res, Response::Error { ref code, .. } if code == "NOT_ALLOWED"));
        assert!(!ts.server_task.is_finished());

        ts.server.context.shutdown_notify.notify_waiters();
        let _ = ts.server_task.await;
    }

    #[tokio::test]
    async fn test_quit_request_cancelled_by_window() {
        let ts = start_test_server().await;
        let client = IpcClient::connect(&ts.runtime_dir).await.expect("connect");
        let mut sub = IpcClient::subscribe(&ts.runtime_dir)
            .await
            .expect("subscribe");
        let _ = sub.next().await.expect("initial snapshot");

        assert!(ts.server.context.request_quit().await);
        let nonce = match next_non_snapshot(&mut sub).await {
            Event::QuitRequested { nonce, .. } => nonce,
            other => panic!("Expected QuitRequested, got {:?}", other),
        };
        let res = client
            .call(Request::CancelQuit {
                nonce: nonce.clone(),
            })
            .await
            .expect("call");
        assert_eq!(res, Response::CancelQuit);
        assert_eq!(next_non_snapshot(&mut sub).await, Event::QuitCancelled);

        let res = client
            .call(Request::ConfirmQuit { nonce })
            .await
            .expect("call");
        assert!(matches!(res, Response::Error { ref code, .. } if code == "NOT_ALLOWED"));

        ts.server.context.shutdown_notify.notify_waiters();
        let _ = ts.server_task.await;
    }

    #[tokio::test]
    async fn test_ipc_handshake_timeout() {
        tokio::time::pause();

        let dir = tempdir().expect("tempdir");
        let runtime_dir = dir.path().to_path_buf();

        let config = WorkerConfig::load_or_init_at(runtime_dir.join("config.json"));
        let cred_store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
        let buffer = Arc::new(MetricsBuffer::new(10));
        let engine = BackgroundEngine::new(config.clone(), buffer.clone(), cred_store.clone())
            .expect("engine");
        let rt_handle = tokio::runtime::Handle::current();
        let shutdown_notify = Arc::new(Notify::new());

        let server = Arc::new(
            IpcServer::new(
                config,
                cred_store,
                buffer,
                engine.handle(rt_handle),
                runtime_dir.clone(),
                Arc::new(AtomicBool::new(true)),
                shutdown_notify.clone(),
            )
            .expect("server new"),
        );

        let server_clone = server.clone();
        let server_task = tokio::spawn(async move {
            let _ = server_clone.run().await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let endpoint = socket_endpoint(&runtime_dir);
        let mut stream = connect_stream(&endpoint).await.expect("connect stream");

        // Advance virtual clock past 5 seconds without sending Hello
        tokio::time::advance(Duration::from_secs(6)).await;

        // Server should close connection on timeout
        let frame: Result<Option<ServerFrame>, _> = read_frame(&mut stream).await;
        assert!(
            frame.is_err() || frame.unwrap().is_none(),
            "Connection should be closed after 5s handshake timeout"
        );

        shutdown_notify.notify_waiters();
        let _ = server_task.await;
    }

    #[tokio::test]
    async fn test_ipc_bad_frame_rejected() {
        let dir = tempdir().expect("tempdir");
        let runtime_dir = dir.path().to_path_buf();

        let config = WorkerConfig::load_or_init_at(runtime_dir.join("config.json"));
        let cred_store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
        let buffer = Arc::new(MetricsBuffer::new(10));
        let engine = BackgroundEngine::new(config.clone(), buffer.clone(), cred_store.clone())
            .expect("engine");
        let rt_handle = tokio::runtime::Handle::current();
        let shutdown_notify = Arc::new(Notify::new());

        let server = Arc::new(
            IpcServer::new(
                config,
                cred_store,
                buffer,
                engine.handle(rt_handle),
                runtime_dir.clone(),
                Arc::new(AtomicBool::new(true)),
                shutdown_notify.clone(),
            )
            .expect("server new"),
        );

        let server_clone = server.clone();
        let server_task = tokio::spawn(async move {
            let _ = server_clone.run().await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let endpoint = socket_endpoint(&runtime_dir);
        let mut stream = connect_stream(&endpoint).await.expect("connect stream");

        // Send a frame with invalid JSON (unknown fields under deny_unknown_fields)
        use tokio::io::AsyncWriteExt;
        let invalid_json = b"{\"type\": \"HELLO\", \"unknown_bad_field\": 123}";
        let len = (invalid_json.len() as u32).to_be_bytes();
        stream.write_all(&len).await.unwrap();
        stream.write_all(invalid_json).await.unwrap();
        stream.flush().await.unwrap();

        // Server should respond with BAD_FRAME and close (RF-AGENT-10-2)
        let reply: Option<ServerFrame> = read_frame(&mut stream).await.expect("read frame");
        assert!(
            matches!(reply, Some(ServerFrame::Rejected { ref code, .. }) if code == "BAD_FRAME"),
            "Expected BAD_FRAME response, got {:?}",
            reply
        );

        shutdown_notify.notify_waiters();
        let _ = server_task.await;
    }

    #[tokio::test]
    async fn test_offline_withdrawal_sets_pending_and_clears_queue() {
        let creds = InMemoryCredentialStore::new();
        creds.set_access_token(Some("token_jwt")).unwrap();
        creds.set_accepted_manifest_hash(Some("hash_123")).unwrap();
        let buffer = MetricsBuffer::new(10);
        let mut queue = OfflineQueue::open_in_memory().unwrap();
        let client = ApiClient::new("http://127.0.0.1:9".into()).unwrap();

        // Server is offline: withdrawal fails to reach server, but purges local state and sets pending_withdrawal = true (RF-AGENT-10-11)
        let res = execute_withdraw(&client, &creds, &buffer, &mut queue, 1, "hash_123").await;
        assert!(res.is_err());
        assert!(creds.get_pending_withdrawal());
        assert!(creds.get_accepted_manifest_hash().is_none());
        assert_eq!(queue.count().unwrap(), 0);
    }
}
