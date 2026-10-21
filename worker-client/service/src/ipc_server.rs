use crate::api::client::{ApiClient, DeviceInfo};
use crate::attendance::AttendanceService;
use crate::background::worker::{handle_revocation, EngineCommand, EngineHandle};
use crate::config::WorkerConfig;
use crate::consent::{submit_decision, ConsentAction, ConsentOffer};
use crate::core::buffer::MetricsBuffer;
use crate::credentials::CredentialStore;
use crate::enrollment::{
    commit_enrollment, enroll, enrollment_error_message, parse_enrollment_input, EnrollmentError,
    PendingEnrollment, Verification,
};
use crate::session::with_access_token;
use crate::state::{AttendanceMode, AwaitingReason, RunMode, WorkerSelf};
use agent_ipc::framing::{read_frame, write_frame};
use agent_ipc::transport::{
    bind_listener, socket_endpoint, token_path, verify_token, write_ipc_token, SocketEndpoint,
};
use agent_ipc::{
    AppSnapshot, AttendanceModeView, Banner, CategoryState, CategoryStates, ClientFrame,
    ConsentOfferSnapshot, DiagnosticsInfo, Event, ListenerTrait as _, Request, Response,
    ServerFrame, Stream, VerificationResult, WindowScreenHint, PROTOCOL_VERSION,
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
    /// Attendance module (TASK-AGENT-12): marks, journal, terms, PIN.
    pub attendance: Arc<AttendanceService>,
    pending_quit: Arc<Mutex<Option<PendingQuit>>>,
}

/// Banners of the window by priority (spec §1): service problem > pending marks > correction to
/// review > new terms version > telemetry notice.
pub fn compute_banners(
    service_problem: bool,
    pending_marks: usize,
    pending_corrections: u32,
    terms_update_pending: bool,
    run_mode: &RunMode,
) -> Vec<Banner> {
    let mut banners = Vec::new();
    if service_problem {
        banners.push(Banner::ServiceProblem);
    }
    if pending_marks > 0 {
        banners.push(Banner::PendingMarksOffline);
    }
    if pending_corrections > 0 {
        banners.push(Banner::CorrectionToReview);
    }
    if terms_update_pending {
        banners.push(Banner::NewTermsVersion);
    }
    let telemetry_notice = matches!(
        run_mode,
        RunMode::AwaitingConsent {
            reason: AwaitingReason::NoConsent | AwaitingReason::NewVersionPending
        } | RunMode::Halted(_)
            | RunMode::EmployerSuspended { .. }
    );
    if telemetry_notice {
        banners.push(Banner::TelemetryNotice);
    }
    banners
}

fn mode_view(mode: AttendanceMode) -> (AttendanceModeView, bool) {
    match mode {
        AttendanceMode::Unlinked => (AttendanceModeView::Unlinked, false),
        AttendanceMode::NeedsTerms => (AttendanceModeView::NeedsTerms, false),
        AttendanceMode::NeedsPin => (AttendanceModeView::NeedsPin, false),
        AttendanceMode::Ready {
            terms_update_pending,
        } => (AttendanceModeView::Ready, terms_update_pending),
        AttendanceMode::Exempt => (AttendanceModeView::Exempt, false),
    }
}

impl IpcServerContext {
    pub async fn build_snapshot(&self) -> AppSnapshot {
        let engine_state = self.engine.state.borrow().clone();
        let run_mode_str = engine_state.run_mode.as_str().to_string();

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
        let queue_size = self
            .attendance
            .open_telemetry_queue()
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

        let attendance = self.attendance.snapshot();
        let (attendance_mode, terms_update_pending) = mode_view(attendance.mode);
        let banners = compute_banners(
            engine_state.last_error.is_some(),
            attendance.pending_marks,
            attendance.pending_corrections,
            terms_update_pending,
            &engine_state.run_mode,
        );

        AppSnapshot {
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            run_mode: run_mode_str,
            consent_offer,
            accepted_selection,
            category_states,
            work_hours_start,
            work_hours_end,
            timezone,
            worker_name: ws.and_then(|w| w.name.clone()),
            organization: ws.and_then(|w| w.organization.clone()),
            queue_size,
            last_flush: None,
            last_error: engine_state.last_error,
            suggested_screen,
            tray_available,
            pending_withdrawal,
            device_identifier_short,
            key_fingerprint,
            is_enrolled,
            attendance_mode,
            terms_update_pending,
            attendance_today: attendance.today,
            pending_marks: attendance.pending_marks,
            pending_corrections: attendance.pending_corrections,
            banners,
            legal: attendance.legal,
            work_mode: attendance.work_mode,
            location_at_mark: attendance.location_at_mark,
            location_available: attendance.location_available,
            personal_email: ws.and_then(|w| w.personal_email.clone()),
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

/// Everything the IPC server is built from.
pub struct IpcServerDeps {
    pub config: WorkerConfig,
    pub cred_store: Arc<dyn CredentialStore>,
    pub buffer: Arc<MetricsBuffer>,
    pub engine: EngineHandle,
    pub runtime_dir: PathBuf,
    pub tray_available: Arc<AtomicBool>,
    pub shutdown_notify: Arc<Notify>,
    pub attendance: Arc<AttendanceService>,
}

pub struct IpcServer {
    pub context: Arc<IpcServerContext>,
    pub runtime_dir: PathBuf,
    pub token: String,
}

impl IpcServer {
    pub fn new(deps: IpcServerDeps) -> std::io::Result<Self> {
        let IpcServerDeps {
            config,
            cred_store,
            buffer,
            engine,
            runtime_dir,
            tray_available,
            shutdown_notify,
            attendance,
        } = deps;
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
            attendance,
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

        // Attendance changes (marks, queue, today view) also refresh the window.
        let ctx = self.context.clone();
        tokio::spawn(async move {
            let mut changes = ctx.attendance.subscribe();
            loop {
                if changes.changed().await.is_err() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
                changes.borrow_and_update();
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
                        let code = match &err {
                            EnrollmentError::Tampered => "ENROLLMENT_TAMPERED",
                            _ => "ENROLLMENT_ERROR",
                        };
                        Response::Error {
                            code: code.into(),
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
                // LEG-05: new telemetry decisions wait until the current terms were read.
                // Rejecting stays possible (protective for the worker).
                let terms_pending = matches!(
                    self.context.attendance.mode(),
                    AttendanceMode::NeedsTerms
                        | AttendanceMode::Ready {
                            terms_update_pending: true
                        }
                );
                if accept.is_some() && terms_pending {
                    return Response::Error {
                        code: "TERMS_REQUIRED".into(),
                        message: "Lee los nuevos términos antes de decidir.".into(),
                        retry_after_secs: None,
                    };
                }
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

                // RF-AGENT-12-10: last attempt to send pending marks, then the journal goes
                // with the credentials (the UI warned with `pending_marks`).
                self.context.attendance.prepare_unlink().await;
                match self.context.attendance.open_telemetry_queue() {
                    Some(mut queue) => handle_revocation(
                        self.context.cred_store.as_ref(),
                        &self.context.buffer,
                        &mut queue,
                    ),
                    None => {
                        let _ = self.context.cred_store.wipe();
                        let _ = self.context.buffer.drain_for_flush();
                    }
                }
                let _ = self.context.engine.commands.send(EngineCommand::SyncNow);
                self.context.broadcast_snapshot().await;
                Response::Unlink
            }
            Request::Diagnostics => {
                let uptime_seconds = self.context.start_time.elapsed().as_secs();
                let queue = self.context.attendance.open_telemetry_queue();
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
                    server_time_offset_seconds: self.context.attendance.server_time_offset(),
                    pending_marks: self.context.attendance.pending_counts().0,
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
            Request::LegalDocuments => Response::LegalDocuments {
                documents: crate::legal::documents(),
            },
            Request::AcceptTerms {
                version,
                hash,
                privacy_version,
                privacy_hash,
            } => match self
                .context
                .attendance
                .accept_terms(&version, &hash, &privacy_version, &privacy_hash)
                .await
            {
                Ok(terms_accepted_version) => Response::AcceptTerms {
                    terms_accepted_version,
                },
                Err(e) => e.into(),
            },
            Request::SetPin { pin } => match self.context.attendance.set_pin(&pin).await {
                Ok(()) => Response::SetPin,
                Err(e) => e.into(),
            },
            Request::ChangePin { current, new } => {
                match self.context.attendance.change_pin(&current, &new).await {
                    Ok(()) => Response::ChangePin,
                    Err(e) => e.into(),
                }
            }
            Request::RequestPinReset => match self.context.attendance.request_pin_reset().await {
                Ok(()) => Response::RequestPinReset,
                Err(e) => e.into(),
            },
            Request::VerifyPinReset { code } => {
                match self.context.attendance.verify_pin_reset(&code).await {
                    Ok(set_pin_window_secs) => Response::VerifyPinReset {
                        set_pin_window_secs,
                    },
                    Err(e) => e.into(),
                }
            }
            Request::Mark { r#type, pin } => {
                match self.context.attendance.mark(r#type, &pin).await {
                    Ok(mark) => Response::Mark {
                        mark: Box::new(mark),
                    },
                    Err(e) => e.into(),
                }
            }
            Request::Today => {
                let _ = self.context.attendance.refresh_today().await;
                Response::Today {
                    today: Box::new(self.context.attendance.today_view()),
                }
            }
            Request::Days { from, to } => match self.context.attendance.days(&from, &to).await {
                Ok(days) => Response::Days { days },
                Err(e) => e.into(),
            },
            Request::ConsentHistory => match self.context.attendance.consent_history().await {
                Ok(records) => Response::ConsentHistory { records },
                Err(e) => e.into(),
            },
            Request::Schedule => match self.context.attendance.schedule().await {
                Ok(schedule) => Response::Schedule {
                    schedule: Box::new(schedule),
                },
                Err(e) => e.into(),
            },
            Request::Marks { from, to, page } => {
                match self.context.attendance.marks(&from, &to, page).await {
                    Ok((source, marks, local, pagination)) => Response::Marks {
                        source,
                        marks,
                        local,
                        pagination,
                    },
                    Err(e) => e.into(),
                }
            }
            Request::Corrections => match self.context.attendance.corrections().await {
                Ok(corrections) => Response::Corrections { corrections },
                Err(e) => e.into(),
            },
            Request::RespondCorrection { id, accept } => {
                match self.context.attendance.respond_correction(&id, accept).await {
                    Ok(correction) => Response::RespondCorrection {
                        correction: Box::new(correction),
                    },
                    Err(e) => e.into(),
                }
            }
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
    use crate::io::transport::OfflineQueue;
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
        let attendance = AttendanceService::for_tests(
            &config.api_base_url,
            cred_store.clone(),
            engine_handle.state.clone(),
        );

        let server = Arc::new(
            IpcServer::new(IpcServerDeps {
                config,
                cred_store: cred_store.clone(),
                buffer,
                engine: engine_handle,
                runtime_dir: runtime_dir.clone(),
                tray_available: tray_available.clone(),
                shutdown_notify: shutdown_notify.clone(),
                attendance,
            })
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
        let attendance = AttendanceService::for_tests(
            &config.api_base_url,
            cred_store.clone(),
            engine_handle.state.clone(),
        );
        let server = Arc::new(
            IpcServer::new(IpcServerDeps {
                config,
                cred_store,
                buffer,
                engine: engine_handle,
                runtime_dir: runtime_dir.clone(),
                tray_available: Arc::new(AtomicBool::new(true)),
                shutdown_notify: Arc::new(Notify::new()),
                attendance,
            })
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
        let engine_handle = engine.handle(rt_handle);
        let attendance = AttendanceService::for_tests(
            &config.api_base_url,
            cred_store.clone(),
            engine_handle.state.clone(),
        );

        let server = Arc::new(
            IpcServer::new(IpcServerDeps {
                config,
                cred_store,
                buffer,
                engine: engine_handle,
                runtime_dir: runtime_dir.clone(),
                tray_available: Arc::new(AtomicBool::new(true)),
                shutdown_notify: shutdown_notify.clone(),
                attendance,
            })
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
        let engine_handle = engine.handle(rt_handle);
        let attendance = AttendanceService::for_tests(
            &config.api_base_url,
            cred_store.clone(),
            engine_handle.state.clone(),
        );

        let server = Arc::new(
            IpcServer::new(IpcServerDeps {
                config,
                cred_store,
                buffer,
                engine: engine_handle,
                runtime_dir: runtime_dir.clone(),
                tray_available: Arc::new(AtomicBool::new(true)),
                shutdown_notify: shutdown_notify.clone(),
                attendance,
            })
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

    #[tokio::test]
    async fn test_ipc_attendance_ops_when_unlinked() {
        let ts = start_test_server().await;
        let client = IpcClient::connect(&ts.runtime_dir).await.expect("connect");

        match client.call(Request::LegalDocuments).await.expect("call") {
            Response::LegalDocuments { documents } => {
                assert_eq!(documents.len(), 2);
                assert_eq!(
                    documents[0].document,
                    agent_ipc::LegalDocumentKind::WorkerTerms
                );
            }
            other => panic!("unexpected {other:?}"),
        }
        let res = client
            .call(Request::Mark {
                r#type: agent_ipc::MarkType::WorkdayStart,
                pin: agent_ipc::SecretPin::new("975318"),
            })
            .await
            .expect("call");
        assert!(matches!(res, Response::Error { ref code, .. } if code == "NOT_ENROLLED"));
        let res = client
            .call(Request::SetPin {
                pin: agent_ipc::SecretPin::new("975318"),
            })
            .await
            .expect("call");
        assert!(matches!(res, Response::Error { ref code, .. } if code == "NOT_ENROLLED"));
        let res = client
            .call(Request::Days {
                from: "ayer".into(),
                to: "hoy".into(),
            })
            .await
            .expect("call");
        assert!(matches!(res, Response::Error { ref code, .. } if code == "INVALID_INPUT"));

        match client.call(Request::Snapshot).await.expect("call") {
            Response::Snapshot { snapshot } => {
                assert_eq!(snapshot.attendance_mode, AttendanceModeView::Unlinked);
                assert_eq!(snapshot.pending_marks, 0);
                assert!(snapshot.attendance_today.is_none());
                assert!(snapshot.banners.is_empty());
                assert_eq!(
                    snapshot.legal.embedded_terms_version.as_deref(),
                    Some("2026-10-11")
                );
            }
            other => panic!("unexpected {other:?}"),
        }
        match client.call(Request::Diagnostics).await.expect("call") {
            Response::Diagnostics { diagnostics } => assert_eq!(diagnostics.pending_marks, 0),
            other => panic!("unexpected {other:?}"),
        }

        ts.server.context.shutdown_notify.notify_waiters();
        let _ = ts.server_task.await;
    }

    #[test]
    fn test_ipc_banners_follow_spec_priority() {
        use crate::state::HaltReason;
        let all = compute_banners(
            true,
            2,
            1,
            true,
            &RunMode::Halted(HaltReason::KeyFingerprintMismatch),
        );
        assert_eq!(
            all,
            vec![
                Banner::ServiceProblem,
                Banner::PendingMarksOffline,
                Banner::CorrectionToReview,
                Banner::NewTermsVersion,
                Banner::TelemetryNotice
            ]
        );
        let mut sorted = all.clone();
        sorted.sort();
        assert_eq!(sorted, all, "enum order is the priority order");
        assert!(compute_banners(false, 0, 0, false, &RunMode::NotRequested).is_empty());
        assert!(compute_banners(
            false,
            0,
            0,
            false,
            &RunMode::AwaitingConsent {
                reason: AwaitingReason::TermsPending
            }
        )
        .is_empty());
    }

    #[tokio::test]
    async fn test_ipc_enroll_tampered_qr_returns_enrollment_tampered_code() {
        let server = wiremock::MockServer::start().await;
        let (_, admin_pub) = telemetry_crypto_core::generate_admin_keypair();
        let admin_fp = telemetry_crypto_core::public_key_fingerprint(&admin_pub).unwrap();

        let flags = telemetry_crypto_core::ConsentFlags {
            system_activity: true,
            ..Default::default()
        };
        let desc = telemetry_crypto_core::ConsentDescriptions {
            system_activity: Some("Monitoreo de procesos".into()),
            ..Default::default()
        };

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/workers/enroll"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "worker_id": "01912345-6789-7abc-def0-123456789abc",
                        "worker_name": "Juan Perez",
                        "organization": "Acme Corp",
                        "role_type": "OFFICE",
                        "work_hours_start": "09:00:00",
                        "work_hours_end": "18:00:00",
                        "timezone": "America/Santiago",
                        "status": "PENDING_TERMS",
                        "auth": {
                            "access_token": "valid_access_jwt",
                            "refresh_token": "valid_refresh_token",
                            "token_type": "Bearer",
                            "expires_in": 900,
                            "user": {
                                "id": "01912345-6789-7abc-def0-123456789abc",
                                "email": "juan@acme.cl",
                                "name": "Juan Perez",
                                "role": "WORKER"
                            }
                        },
                        "admin_public_key": admin_pub,
                        "admin_public_key_fingerprint": admin_fp,
                        "consent_version": 1,
                        "consent_flags": flags,
                        "consent_descriptions": desc
                    }
                })),
            )
            .mount(&server)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/auth/logout"))
            .respond_with(wiremock::ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let dir = tempdir().expect("tempdir");
        let runtime_dir = dir.path().to_path_buf();
        let mut config = WorkerConfig::load_or_init_at(runtime_dir.join("config.json"));
        config.api_base_url = server.uri();
        let cred_store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
        let buffer = Arc::new(MetricsBuffer::new(10));
        let engine = BackgroundEngine::new(config.clone(), buffer.clone(), cred_store.clone())
            .expect("engine");
        let engine_handle = engine.handle(tokio::runtime::Handle::current());
        let attendance = AttendanceService::for_tests(
            &config.api_base_url,
            cred_store.clone(),
            engine_handle.state.clone(),
        );
        let ipc_server = Arc::new(
            IpcServer::new(IpcServerDeps {
                config,
                cred_store: cred_store.clone(),
                buffer,
                engine: engine_handle,
                runtime_dir: runtime_dir.clone(),
                tray_available: Arc::new(AtomicBool::new(true)),
                shutdown_notify: Arc::new(Notify::new()),
                attendance,
            })
            .expect("server new"),
        );
        let server_clone = ipc_server.clone();
        let server_task = tokio::spawn(async move {
            let _ = server_clone.run().await;
        });

        let mut client = None;
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if let Ok(c) = IpcClient::connect(&runtime_dir).await {
                client = Some(c);
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let client = client.expect("connect to IPC server");

        let tampered_qr = telemetry_crypto_core::EnrollmentQr {
            t: "telemetry-enroll".into(),
            v: 1,
            api_url: server.uri(),
            code: "K7QM-4XRT-9PWD".into(),
            commitment: "0000000000000000000000000000000000000000000000000000000000000000".into(),
        };

        let res = client
            .call(Request::Enroll {
                qr_payload: Some(tampered_qr.to_payload().unwrap()),
                code: None,
            })
            .await
            .expect("call");

        match res {
            Response::Error { code, .. } => {
                assert_eq!(code, "ENROLLMENT_TAMPERED");
            }
            other => panic!("Expected Error with ENROLLMENT_TAMPERED, got {:?}", other),
        }

        // SEC-07: Verify credentials were not persisted
        assert!(cred_store.get_access_token().is_none());
        assert!(cred_store.get_refresh_token().is_none());
        assert!(cred_store.get_pinned_admin_public_key().is_none());

        ipc_server.context.shutdown_notify.notify_waiters();
        let _ = server_task.await;
    }
}
