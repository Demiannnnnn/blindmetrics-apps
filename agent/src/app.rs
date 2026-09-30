use crate::api::client::{ApiClient, DataDeletionResponse, DataRequestResponse, DeviceInfo};
use crate::background::worker::{EngineCommand, EngineHandle, EngineSnapshot};
use crate::config::WorkerConfig;
use crate::consent::{count_categories, submit_decision, ConsentAction, ConsentOffer};
use crate::core::buffer::MetricsBuffer;
use crate::core::collectors::keystroke_counter::KeystrokeCollector;
use crate::credentials::CredentialStore;
use crate::enrollment::{
    commit_enrollment, enroll, normalize_code, validate_server_url, EnrollmentError,
    EnrollmentInput, PendingEnrollment, Verification,
};
use crate::session::{with_access_token, SessionError};
use crate::state::{AwaitingReason, BillingBlock, RunMode, WorkerSelf};
use crate::ui::tray::{SystemTrayManager, TrayAction};
use eframe::egui::{self, Color32, RichText};
use std::future::Future;
use std::sync::{mpsc, Arc};
use telemetry_crypto_core::{ConsentFlags, EnrollmentQr};

/// Results of async work spawned on the engine runtime, drained every frame.
enum UiEvent {
    Enrolled(Box<Result<PendingEnrollment, EnrollmentError>>),
    /// `true` when categories were accepted, `false` for reject/withdraw.
    ConsentDecided(bool, Result<(), SessionError>),
    DataRequest(Result<DataRequestResponse, SessionError>),
    DataDeletion(Result<DataDeletionResponse, SessionError>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ActiveScreen {
    E1Enrollment,
    E2Verification {
        safety_code: String,
    },
    E3TamperingAlert,
    C1Consent,
    IInactive,
    MMain {
        active_tab: MainTab,
    },
    WWithdrawConfirm,
    RUnlinkedNotice,
    HHalted {
        server_fp: String,
        pinned_fp: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainTab {
    WhatsCollected,
    MySchedule,
    MyRights,
    Diagnostics,
}

pub struct WorkerDesktopApp {
    pub config: WorkerConfig,
    pub buffer: Arc<MetricsBuffer>,
    pub cred_store: Arc<dyn CredentialStore>,
    engine: Option<EngineHandle>,
    events_tx: mpsc::Sender<UiEvent>,
    events_rx: mpsc::Receiver<UiEvent>,
    /// A request is in flight: action buttons are disabled.
    busy: bool,
    /// Package shown on C1 (from the enroll response or /workers/me).
    consent_offer: Option<ConsentOffer>,
    /// Categories ticked on C1; always starts empty (DP-08).
    consent_selection: ConsentFlags,
    /// Re-open C1 once the engine publishes the refreshed `/workers/me` (409 on a decision).
    reopen_consent_after_sync: bool,
    /// `429 ENROLLMENT_LOCKED`: the Link button stays disabled until then.
    enroll_locked_until: Option<std::time::Instant>,
    pub screen: ActiveScreen,
    pub enrollment_code: String,
    pub enrollment_server_url: String,
    /// E1 mode: pasted QR payload (verified automatically) or manual code.
    enroll_with_qr: bool,
    enrollment_qr_text: String,
    pub pending_enrollment: Option<PendingEnrollment>,
    pub worker_self: Option<WorkerSelf>,
    pub run_mode: RunMode,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
    pub tray: Option<SystemTrayManager>,
    pub window_visible: bool,
    pub last_arco_request_id: Option<String>,
    pub last_arco_status: Option<String>,
    pub last_arco_message: Option<String>,
    pub show_deletion_confirm: bool,
}

impl WorkerDesktopApp {
    pub fn new(
        config: WorkerConfig,
        buffer: Arc<MetricsBuffer>,
        cred_store: Arc<dyn CredentialStore>,
        engine: Option<EngineHandle>,
    ) -> Self {
        let has_pinned_key = cred_store.get_pinned_admin_public_key().is_some();
        let has_token = cred_store.get_access_token().is_some();
        let has_accepted = cred_store.get_accepted_manifest_hash().is_some();

        let initial_screen = if !has_pinned_key || !has_token {
            ActiveScreen::E1Enrollment
        } else if !has_accepted {
            ActiveScreen::IInactive
        } else {
            ActiveScreen::MMain {
                active_tab: MainTab::WhatsCollected,
            }
        };
        let (events_tx, events_rx) = mpsc::channel();
        let error_message = engine.is_none().then(|| {
            "El motor en segundo plano no está disponible; reinicie la aplicación.".to_string()
        });

        Self {
            enrollment_server_url: config.api_base_url.clone(),
            config,
            buffer,
            cred_store,
            engine,
            events_tx,
            events_rx,
            busy: false,
            consent_offer: None,
            consent_selection: ConsentFlags::default(),
            reopen_consent_after_sync: false,
            enroll_locked_until: None,
            screen: initial_screen,
            enrollment_code: String::new(),
            enroll_with_qr: true,
            enrollment_qr_text: String::new(),
            pending_enrollment: None,
            worker_self: None,
            run_mode: RunMode::Unenrolled,
            status_message: None,
            error_message,
            tray: Some(SystemTrayManager::new()),
            window_visible: true,
            last_arco_request_id: None,
            last_arco_status: None,
            last_arco_message: None,
            show_deletion_confirm: false,
        }
    }

    fn api(&self) -> Result<ApiClient, String> {
        ApiClient::new(self.config.api_base_url.clone())
            .map_err(|e| format!("Cliente HTTP no disponible: {}", e))
    }

    fn send_engine(&self, cmd: EngineCommand) {
        if let Some(engine) = &self.engine {
            let _ = engine.commands.send(cmd);
        }
    }

    /// Runs `fut` on the engine runtime; the result arrives as a `UiEvent`.
    /// The egui thread never blocks on the network.
    fn spawn<F>(&mut self, ctx: &egui::Context, fut: F)
    where
        F: Future<Output = UiEvent> + Send + 'static,
    {
        let Some(engine) = &self.engine else {
            self.error_message = Some(
                "El motor en segundo plano no está disponible; reinicie la aplicación.".into(),
            );
            return;
        };
        self.busy = true;
        let tx = self.events_tx.clone();
        let ctx = ctx.clone();
        engine.runtime.spawn(async move {
            let _ = tx.send(fut.await);
            ctx.request_repaint();
        });
    }

    /// Mirrors the engine state and moves between screens on lifecycle changes.
    fn sync_from_engine(&mut self) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        if !engine.state.has_changed().unwrap_or(false) {
            return;
        }
        let snap: EngineSnapshot = engine.state.borrow_and_update().clone();
        let previous = std::mem::replace(&mut self.run_mode, snap.run_mode.clone());
        self.worker_self = snap.worker_self.clone();
        if let Some(err) = snap.last_error {
            self.error_message = Some(err);
        }
        if self.reopen_consent_after_sync && self.worker_self.is_some() && !snap.revoked {
            self.reopen_consent_after_sync = false;
            self.open_consent_from_server();
        }

        if snap.revoked {
            self.consent_offer = None;
            self.pending_enrollment = None;
            self.screen = ActiveScreen::RUnlinkedNotice;
            return;
        }
        if previous == self.run_mode {
            return;
        }

        match &self.run_mode {
            RunMode::Halted(_) => {
                self.screen = ActiveScreen::HHalted {
                    server_fp: snap
                        .worker_self
                        .as_ref()
                        .map(|w| w.admin_public_key_fingerprint.clone())
                        .unwrap_or_default(),
                    pinned_fp: self.cred_store.get_pinned_fingerprint().unwrap_or_default(),
                };
            }
            RunMode::AwaitingConsent {
                reason: AwaitingReason::NewVersionPending,
            } => {
                if matches!(
                    self.screen,
                    ActiveScreen::MMain { .. } | ActiveScreen::IInactive
                ) {
                    self.open_consent_from_server();
                }
            }
            RunMode::AwaitingConsent { .. } => {
                if matches!(self.screen, ActiveScreen::MMain { .. }) {
                    self.screen = ActiveScreen::IInactive;
                }
            }
            RunMode::Collecting(_) | RunMode::Paused | RunMode::EmployerSuspended { .. } => {
                if matches!(self.screen, ActiveScreen::IInactive) {
                    self.screen = ActiveScreen::MMain {
                        active_tab: MainTab::WhatsCollected,
                    };
                }
            }
            RunMode::Unenrolled => {
                if self.cred_store.get_access_token().is_none()
                    && !matches!(
                        self.screen,
                        ActiveScreen::E1Enrollment
                            | ActiveScreen::E2Verification { .. }
                            | ActiveScreen::E3TamperingAlert
                            | ActiveScreen::C1Consent
                    )
                {
                    self.screen = ActiveScreen::E1Enrollment;
                }
            }
        }
    }

    /// Shows C1 for the package currently requested by the server, rebuilt
    /// with the pinned key so the manifest hash matches the admin's (SEC-08).
    fn open_consent_from_server(&mut self) {
        let offer = match (
            &self.worker_self,
            self.cred_store.get_pinned_admin_public_key(),
        ) {
            (Some(me), Some(pinned)) => {
                ConsentOffer::from_worker_self(me, &pinned, self.cred_store.get_accepted_flags())
                    .ok()
            }
            _ => None,
        };
        match offer {
            Some(offer) => {
                self.consent_offer = Some(offer);
                // DP-08: every checkbox starts unchecked.
                self.consent_selection = ConsentFlags::default();
                self.screen = ActiveScreen::C1Consent;
            }
            None => {
                self.error_message = Some(
                    "No se pudo obtener la solicitud vigente del servidor; se reintentará automáticamente."
                        .into(),
                );
                self.send_engine(EngineCommand::SyncNow);
            }
        }
    }

    fn commit_pending(&mut self, pending: &PendingEnrollment) {
        let url = self.enrollment_server_url.trim().to_string();
        match commit_enrollment(pending, &url, self.cred_store.as_ref(), &mut self.config) {
            Ok(()) => {
                self.consent_offer = Some(ConsentOffer::from_enrollment(pending));
                self.consent_selection = ConsentFlags::default();
                self.send_engine(EngineCommand::Enrolled {
                    api_base_url: self.config.api_base_url.clone(),
                });
                self.screen = ActiveScreen::C1Consent;
            }
            Err(e) => {
                let _ = self.cred_store.wipe();
                self.error_message = Some(format!("No se pudo guardar la vinculación: {}", e));
                self.screen = ActiveScreen::E1Enrollment;
            }
        }
    }

    fn handle_event(&mut self, event: UiEvent) {
        self.busy = false;
        match event {
            UiEvent::Enrolled(res) => match *res {
                Ok(pending) => {
                    self.pending_enrollment = Some(pending.clone());
                    match &pending.verification {
                        Verification::Verified => self.commit_pending(&pending),
                        Verification::NeedsSafetyCode(sc) => {
                            self.screen = ActiveScreen::E2Verification {
                                safety_code: sc.clone(),
                            };
                        }
                    }
                }
                Err(EnrollmentError::Tampered) => {
                    self.screen = ActiveScreen::E3TamperingAlert;
                }
                Err(e) => {
                    if let EnrollmentError::Api(crate::errors::AgentError::Api {
                        status: 429,
                        retry_after_seconds: Some(wait),
                        ..
                    }) = &e
                    {
                        self.enroll_locked_until =
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(*wait));
                    }
                    self.error_message = Some(enrollment_error_message(&e));
                }
            },
            UiEvent::ConsentDecided(accepted, Ok(())) => {
                self.error_message = None;
                self.pending_enrollment = None;
                self.consent_offer = None;
                self.consent_selection = ConsentFlags::default();
                self.screen = if accepted {
                    ActiveScreen::MMain {
                        active_tab: MainTab::WhatsCollected,
                    }
                } else {
                    ActiveScreen::IInactive
                };
                self.send_engine(EngineCommand::SyncNow);
            }
            UiEvent::ConsentDecided(_, Err(SessionError::Revoked)) => {
                self.send_engine(EngineCommand::SyncNow);
            }
            UiEvent::ConsentDecided(_, Err(SessionError::Invalid(msg))) => {
                self.error_message = Some(msg.to_string());
            }
            UiEvent::ConsentDecided(_, Err(e)) => {
                let changed = matches!(
                    &e,
                    SessionError::Api(err) if matches!(
                        err.reason(),
                        Some("CONSENT_VERSION_MISMATCH") | Some("CONSENT_FLAG_NOT_REQUESTED")
                    )
                );
                if changed {
                    // RF-AGENT-09-5: re-read /workers/me and show the new request.
                    self.error_message =
                        Some("La solicitud de tu empleador cambió; revísala nuevamente.".into());
                    self.reopen_consent_after_sync = true;
                } else {
                    self.error_message = Some(format!(
                        "El servidor no registró tu decisión ({}). Inténtalo nuevamente.",
                        e
                    ));
                }
                self.consent_offer = None;
                self.screen = ActiveScreen::IInactive;
                self.send_engine(EngineCommand::SyncNow);
            }
            UiEvent::DataRequest(Ok(resp)) => {
                self.last_arco_request_id = Some(resp.request_id.clone());
                self.last_arco_status = Some(resp.status.clone());
                self.last_arco_message = resp
                    .estimated_completion
                    .map(|e| format!("Estimado: {}", e));
                self.status_message = Some(format!(
                    "Solicitud de exportación creada exitosamente (ID: {}).",
                    resp.request_id
                ));
                self.error_message = None;
            }
            UiEvent::DataRequest(Err(e)) => {
                self.error_message = Some(format!("Error en solicitud de datos: {}", e));
            }
            UiEvent::DataDeletion(Ok(resp)) => {
                self.last_arco_request_id = Some(resp.request_id.clone());
                self.last_arco_status = Some(resp.status.clone());
                self.last_arco_message = resp.message;
                self.status_message = Some(format!(
                    "Solicitud de eliminación registrada exitosamente (ID: {}).",
                    resp.request_id
                ));
                self.error_message = None;
            }
            UiEvent::DataDeletion(Err(e)) => {
                self.error_message = Some(format!("Error al solicitar eliminación: {}", e));
            }
        }
    }
}

/// Builds the enrollment input from the E1 form (RF-AGENT-08-1): a pasted QR
/// payload (verified against its commitment) or a manual code + server URL.
fn parse_enrollment_input(
    with_qr: bool,
    qr_text: &str,
    code: &str,
    server_url: &str,
) -> Result<EnrollmentInput, String> {
    let allow_http = cfg!(debug_assertions);
    if with_qr {
        // RF-AGENT-09-8: base64 `qr_payload` of the API or raw JSON of older admin apps.
        let qr = EnrollmentQr::parse_payload(qr_text).map_err(|_| {
            "El contenido pegado no es un código de vinculación válido.".to_string()
        })?;
        validate_server_url(&qr.api_url, allow_http)?;
        return Ok(EnrollmentInput::Qr(qr));
    }
    let code = normalize_code(code);
    if code.len() != 12 {
        return Err("El código debe tener 12 caracteres válidos.".into());
    }
    if server_url.trim().is_empty() {
        return Err("Ingrese la dirección del servidor.".into());
    }
    validate_server_url(server_url, allow_http)?;
    Ok(EnrollmentInput::Manual {
        api_url: server_url.to_string(),
        code,
    })
}

/// RF-AGENT-09-9: messages by `details.reason` of `POST /workers/enroll`.
fn enrollment_error_message(e: &EnrollmentError) -> String {
    use crate::errors::AgentError;
    match e {
        EnrollmentError::Api(AgentError::Api {
            status: 429,
            reason,
            retry_after_seconds,
            ..
        }) => {
            if reason.as_deref() == Some("ENROLLMENT_LOCKED") {
                let minutes = retry_after_seconds.map(|s| s.div_ceil(60)).unwrap_or(15).max(1);
                format!(
                    "Demasiados intentos fallidos. Podrás intentarlo de nuevo en {} minuto(s).",
                    minutes
                )
            } else {
                "Demasiadas solicitudes. Espera un momento e inténtalo de nuevo.".into()
            }
        }
        EnrollmentError::Api(AgentError::Api { status: 409, reason, .. })
            if reason.as_deref() == Some("DEVICE_ALREADY_ENROLLED") =>
        {
            "Este equipo ya está vinculado a otro trabajador de la organización. Pide a tu administrador que lo desvincule.".into()
        }
        EnrollmentError::Api(AgentError::Api { status: 409, .. }) => {
            "Este código ya fue utilizado. Si no fuiste tú, avisa a tu administrador de inmediato."
                .into()
        }
        EnrollmentError::Api(AgentError::Api { status: 400, .. })
        | EnrollmentError::InvalidCodeFormat => {
            "Este código es inválido o expiró. Pide uno nuevo a tu administrador.".into()
        }
        EnrollmentError::Api(AgentError::Network(_)) => {
            "No se pudo contactar al servidor. Verifica la dirección y tu conexión.".into()
        }
        other => format!("Error de vinculación: {}", other),
    }
}

impl eframe::App for WorkerDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        while let Ok(event) = self.events_rx.try_recv() {
            self.handle_event(event);
        }
        self.sync_from_engine();

        // 1. Intercept window close request and hide to tray (RF-AGENT-07-2, docs §3)
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.window_visible = false;
        }

        // 2. Poll and handle system tray menu actions
        if let Some(ref mut tray) = self.tray {
            if let Some(action) = tray.poll_action() {
                match action {
                    TrayAction::Open => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        self.window_visible = true;
                    }
                    TrayAction::WhatsCollected => {
                        self.screen = ActiveScreen::MMain {
                            active_tab: MainTab::WhatsCollected,
                        };
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        self.window_visible = true;
                    }
                    TrayAction::WithdrawConsent => {
                        self.screen = ActiveScreen::WWithdrawConfirm;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        self.window_visible = true;
                    }
                    TrayAction::Quit => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }

            // Sync tray icon color with RunMode status light (docs §3)
            let (r, g, b) = match &self.run_mode {
                RunMode::Collecting(_) => (46, 204, 113),
                RunMode::Paused | RunMode::EmployerSuspended { .. } => (241, 196, 15),
                RunMode::AwaitingConsent { .. } | RunMode::Unenrolled => (180, 180, 180),
                RunMode::Halted(_) => (231, 76, 60),
            };
            tray.set_icon_color(r, g, b);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.render_header(ui);
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                if let Some(ref err) = self.error_message {
                    ui.label(
                        RichText::new(format!("⚠️ {}", err))
                            .color(Color32::from_rgb(231, 76, 60))
                            .strong(),
                    );
                    ui.add_space(6.0);
                }
                if let Some(ref msg) = self.status_message {
                    ui.label(
                        RichText::new(format!("ℹ️ {}", msg)).color(Color32::from_rgb(46, 204, 113)),
                    );
                    ui.add_space(6.0);
                }

                match &self.screen.clone() {
                    ActiveScreen::E1Enrollment => self.render_screen_e1(ui),
                    ActiveScreen::E2Verification { safety_code } => {
                        self.render_screen_e2(ui, safety_code)
                    }
                    ActiveScreen::E3TamperingAlert => self.render_screen_e3(ui),
                    ActiveScreen::C1Consent => self.render_screen_c1(ui),
                    ActiveScreen::IInactive => self.render_screen_i(ui),
                    ActiveScreen::MMain { active_tab } => self.render_screen_m(ui, *active_tab),
                    ActiveScreen::WWithdrawConfirm => self.render_screen_w(ui),
                    ActiveScreen::RUnlinkedNotice => self.render_screen_r(ui),
                    ActiveScreen::HHalted {
                        server_fp,
                        pinned_fp,
                    } => self.render_screen_h(ui, server_fp, pinned_fp),
                }
            });
        });
    }
}

impl WorkerDesktopApp {
    fn render_header(&mut self, ui: &mut egui::Ui) {
        let (light, light_text, color) = match &self.run_mode {
            RunMode::Collecting(_) => (
                "🟢",
                "Recolección Activa y Protegida",
                Color32::from_rgb(46, 204, 113),
            ),
            RunMode::Paused => (
                "🟡",
                "Pausado por el Empleador",
                Color32::from_rgb(241, 196, 15),
            ),
            RunMode::EmployerSuspended { reason } => (
                "🟡",
                match reason {
                    BillingBlock::SubscriptionSuspended | BillingBlock::SubscriptionCancelled => {
                        "Recolección detenida: la cuenta de tu empleador no está activa"
                    }
                },
                Color32::from_rgb(241, 196, 15),
            ),
            RunMode::AwaitingConsent { .. } => {
                ("⚪", "Esperando Consentimiento", Color32::LIGHT_GRAY)
            }
            RunMode::Unenrolled => ("⚪", "No Vinculado", Color32::GRAY),
            RunMode::Halted(_) => (
                "🔴",
                "Detenido por Seguridad",
                Color32::from_rgb(231, 76, 60),
            ),
        };

        ui.horizontal(|ui| {
            ui.heading(RichText::new("Telemetry Worker").strong().size(18.0));
            ui.label(RichText::new("v0.1.0").color(Color32::GRAY).small());
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new(light).size(16.0));
            ui.label(RichText::new(light_text).color(color).strong());
        });

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Dispositivo:").color(Color32::LIGHT_GRAY));
            let dev_preview = if self.config.device_identifier.len() >= 8 {
                &self.config.device_identifier[..8]
            } else {
                &self.config.device_identifier
            };
            ui.monospace(dev_preview);
            if let Some(ref ws) = self.worker_self {
                ui.label(RichText::new(format!("Rol: {}", ws.role_type)).strong());
            }
        });
    }

    fn render_screen_e1(&mut self, ui: &mut egui::Ui) {
        ui.heading("Vincular este equipo a su empleador");
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.enroll_with_qr, true, "Código QR");
            ui.selectable_value(&mut self.enroll_with_qr, false, "Ingresar código");
        });
        ui.add_space(10.0);

        if self.enroll_with_qr {
            ui.label("Pegue el contenido del código QR que le entregó su administrador:");
            ui.add(
                egui::TextEdit::multiline(&mut self.enrollment_qr_text)
                    .desired_rows(4)
                    .hint_text("eyJ0IjoidGVsZW1ldHJ5LWVucm9sbCIs…"),
            );
            ui.label(
                RichText::new(
                    "El QR incluye la dirección del servidor y se verifica automáticamente.",
                )
                .small()
                .color(Color32::LIGHT_GRAY),
            );
        } else {
            ui.label("Dirección del servidor:");
            ui.text_edit_singleline(&mut self.enrollment_server_url);
            ui.add_space(6.0);
            ui.label("Código de 12 caracteres (ej. K7QM-4XRT-9PWD):");
            ui.text_edit_singleline(&mut self.enrollment_code);
        }

        ui.add_space(10.0);
        // RF-AGENT-09-9: `429 ENROLLMENT_LOCKED` blocks the button for `Retry-After`.
        let locked_for = self
            .enroll_locked_until
            .and_then(|until| until.checked_duration_since(std::time::Instant::now()));
        if locked_for.is_none() {
            self.enroll_locked_until = None;
        }
        if let Some(wait) = locked_for {
            ui.label(
                RichText::new(format!(
                    "Podrás intentarlo de nuevo en {} minuto(s).",
                    wait.as_secs().div_ceil(60).max(1)
                ))
                .color(Color32::from_rgb(241, 196, 15)),
            );
        }
        let link = egui::Button::new(RichText::new("🔗 Vincular Equipo").strong());
        if ui
            .add_enabled(!self.busy && locked_for.is_none(), link)
            .clicked()
        {
            let input = match self.enrollment_input() {
                Ok(input) => input,
                Err(msg) => {
                    self.error_message = Some(msg);
                    return;
                }
            };
            // The QR carries the server address; manual codes use the typed one.
            self.enrollment_server_url =
                input.api_url(&self.config.api_base_url).trim().to_string();
            let api = match ApiClient::new(self.enrollment_server_url.clone()) {
                Ok(api) => api,
                Err(e) => {
                    self.error_message = Some(format!("Dirección de servidor inválida: {}", e));
                    return;
                }
            };
            self.error_message = None;
            let device_id = self.config.device_identifier.clone();
            self.spawn(&ui.ctx().clone(), async move {
                let dev_info = DeviceInfo::collect();
                UiEvent::Enrolled(Box::new(enroll(&api, &input, &device_id, &dev_info).await))
            });
        }
        if self.busy {
            ui.spinner();
        }
    }

    fn enrollment_input(&self) -> Result<EnrollmentInput, String> {
        parse_enrollment_input(
            self.enroll_with_qr,
            &self.enrollment_qr_text,
            &self.enrollment_code,
            &self.enrollment_server_url,
        )
    }

    fn render_screen_e2(&mut self, ui: &mut egui::Ui, safety_code: &str) {
        ui.heading("Verificación de Seguridad");
        ui.add_space(8.0);
        ui.label("Pida a su administrador que lea el código en su pantalla. ¿Coinciden?");

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(safety_code).size(26.0).monospace().strong());
        });
        ui.add_space(16.0);

        ui.horizontal(|ui| {
            if ui
                .button(
                    RichText::new("✓ Coinciden")
                        .strong()
                        .color(Color32::from_rgb(46, 204, 113)),
                )
                .clicked()
            {
                if let Some(pending) = self.pending_enrollment.clone() {
                    self.commit_pending(&pending);
                }
            }

            if ui
                .button(
                    RichText::new("✗ No coinciden")
                        .strong()
                        .color(Color32::from_rgb(231, 76, 60)),
                )
                .clicked()
            {
                // Credentials from the redeem are discarded (E3).
                if let (Some(pending), Ok(api)) = (self.pending_enrollment.take(), self.api()) {
                    let refresh = pending.resp.auth.refresh_token.clone();
                    if let Some(engine) = &self.engine {
                        engine.runtime.spawn(async move {
                            let _ = api.logout(&refresh).await;
                        });
                    }
                }
                self.screen = ActiveScreen::E3TamperingAlert;
            }
        });
    }

    fn render_screen_e3(&mut self, ui: &mut egui::Ui) {
        ui.heading(
            RichText::new("⚠️ Alerta de Manipulación").color(Color32::from_rgb(231, 76, 60)),
        );
        ui.add_space(8.0);
        ui.label("La información recibida del servidor no coincide con la generada por su administrador. Por su seguridad, este equipo NO fue vinculado.");

        ui.add_space(12.0);
        if ui.button("Volver al inicio").clicked() {
            self.screen = ActiveScreen::E1Enrollment;
        }
    }

    fn render_screen_c1(&mut self, ui: &mut egui::Ui) {
        let Some(offer) = self.consent_offer.clone() else {
            ui.label("No hay una solicitud de consentimiento cargada.");
            if ui.button("Volver").clicked() {
                self.screen = ActiveScreen::IInactive;
            }
            return;
        };
        let org = offer
            .organization
            .clone()
            .unwrap_or_else(|| "Tu empleador".into());

        ui.heading(format!("{} — Telemetría del equipo", org));
        ui.add_space(6.0);
        ui.label("Tu empleador solicita poder recolectar estos datos. Elige cuáles aceptas:");
        if offer.previous_selection.is_some() {
            ui.label(
                RichText::new(format!(
                    "Tu empleador actualizó la solicitud (versión {}). Revisa las categorías.",
                    offer.consent_version
                ))
                .color(Color32::from_rgb(241, 196, 15)),
            );
        }
        ui.add_space(8.0);

        // RF-AGENT-09-1: one checkbox per REQUESTED category, all unchecked by default (DP-08);
        // the admin's description is shown verbatim (LEG-01).
        let previous = offer.previous_selection.unwrap_or_default();
        let desc = &offer.descriptions;
        let rows: [(&str, bool, &mut bool, Option<&str>, bool); 6] = [
            (
                "Aplicaciones en uso",
                offer.flags.system_activity,
                &mut self.consent_selection.system_activity,
                desc.system_activity.as_deref(),
                previous.system_activity,
            ),
            (
                "Navegación web (solo dominios)",
                offer.flags.network_activity,
                &mut self.consent_selection.network_activity,
                desc.network_activity.as_deref(),
                previous.network_activity,
            ),
            (
                "Actividad de teclado y mouse (conteo)",
                offer.flags.productivity_basic,
                &mut self.consent_selection.productivity_basic,
                desc.productivity_basic.as_deref(),
                previous.productivity_basic,
            ),
            (
                "Capturas de pantalla",
                offer.flags.productivity_screenshots,
                &mut self.consent_selection.productivity_screenshots,
                desc.productivity_screenshots.as_deref(),
                previous.productivity_screenshots,
            ),
            (
                "Metadatos de archivos",
                offer.flags.file_activity,
                &mut self.consent_selection.file_activity,
                desc.file_activity.as_deref(),
                previous.file_activity,
            ),
            (
                "Ubicación geográfica",
                offer.flags.location,
                &mut self.consent_selection.location,
                desc.location.as_deref(),
                previous.location,
            ),
        ];
        for (title, requested, selected, description, accepted_before) in rows {
            if requested {
                let label = if offer.previous_selection.is_some() && !accepted_before {
                    format!("{title} (no aceptada antes)")
                } else {
                    title.to_string()
                };
                ui.checkbox(selected, RichText::new(label).strong());
                if let Some(d) = description {
                    ui.label(
                        RichText::new(format!("   \"{}\"", d))
                            .italics()
                            .color(Color32::LIGHT_GRAY),
                    );
                }
            } else {
                *selected = false;
                ui.label(
                    RichText::new(format!("— {} — no solicitado por tu empleador", title))
                        .color(Color32::GRAY),
                );
            }
        }

        ui.add_space(8.0);
        if let (Some(s), Some(e), Some(tz)) = (
            &offer.work_hours_start,
            &offer.work_hours_end,
            &offer.timezone,
        ) {
            ui.label(format!("Horario laboral: {} - {} ({})", s, e, tz));
        }

        ui.add_space(6.0);
        ui.label(
            RichText::new("Los datos se cifran en este equipo; el servidor no puede leerlos.")
                .small()
                .color(Color32::LIGHT_GRAY),
        );
        ui.label(
            RichText::new("Puedes aceptar solo algunas categorías y cambiar tu elección o retirarla cuando quieras desde esta app. También puedes pedir una copia o la eliminación de tus datos.")
                .small()
                .color(Color32::LIGHT_GRAY),
        );

        ui.add_space(10.0);
        if offer.previous_selection.is_some()
            && ui
                .add_enabled(
                    !self.busy,
                    egui::Button::new("Mantener mi selección anterior"),
                )
                .clicked()
        {
            // RF-AGENT-09-2: explicit worker action, never a default.
            self.consent_selection = offer.keep_previous_selection();
        }

        let selected = count_categories(&crate::consent::intersect(
            &self.consent_selection,
            &offer.flags,
        ));
        ui.add_space(6.0);
        let mut action = None;
        ui.horizontal(|ui| {
            let reject = egui::Button::new(
                RichText::new("RECHAZAR TODO")
                    .strong()
                    .color(Color32::from_rgb(231, 76, 60)),
            );
            if ui.add_enabled(!self.busy, reject).clicked() {
                action = Some(ConsentAction::Decline);
            }
            let accept = egui::Button::new(
                RichText::new(format!("ACEPTAR SELECCIÓN ({selected})"))
                    .strong()
                    .color(Color32::from_rgb(46, 204, 113)),
            );
            if ui.add_enabled(!self.busy && selected > 0, accept).clicked() {
                action = Some(ConsentAction::Accept(self.consent_selection));
            }
            if self.busy {
                ui.spinner();
            }
        });

        if let Some(action) = action {
            // The decision (with the displayed manifest's hash, LEG-02) must be
            // recorded by the server before anything is collected.
            let api = match self.api() {
                Ok(api) => api,
                Err(e) => {
                    self.error_message = Some(e);
                    return;
                }
            };
            let creds = self.cred_store.clone();
            self.spawn(&ui.ctx().clone(), async move {
                let res = submit_decision(&api, creds.as_ref(), &offer, action)
                    .await
                    .map(|_| ());
                UiEvent::ConsentDecided(action.is_accept(), res)
            });
        }
    }

    /// One category of "Qué se recolecta" (RF-AGENT-09-6).
    fn render_category_state(
        ui: &mut egui::Ui,
        title: &str,
        requested: bool,
        accepted: bool,
        description: Option<&str>,
    ) {
        let (text, color) = match (requested, accepted) {
            (true, true) => (
                format!("✅ {} — Activa", title),
                Color32::from_rgb(46, 204, 113),
            ),
            (true, false) => (
                format!("◻ {} — No aceptada por ti", title),
                Color32::LIGHT_GRAY,
            ),
            (false, _) => (format!("— {} — No solicitada", title), Color32::GRAY),
        };
        ui.label(RichText::new(text).color(color).strong());
        if requested {
            if let Some(d) = description {
                ui.label(
                    RichText::new(format!("   \"{}\"", d))
                        .italics()
                        .color(Color32::LIGHT_GRAY),
                );
            }
        }
    }

    fn render_screen_i(&mut self, ui: &mut egui::Ui) {
        ui.heading("Monitoreo Apagado");
        ui.add_space(8.0);
        ui.label(
            "El monitoreo está desactivado. No se está recolectando ningún dato de este equipo.",
        );

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui.button("Revisar solicitud nuevamente").clicked() {
                self.open_consent_from_server();
            }

            if ui
                .button(
                    RichText::new("Desvincular este equipo").color(Color32::from_rgb(231, 76, 60)),
                )
                .clicked()
            {
                let _ = self.buffer.drain_for_flush();
                let _ = self.cred_store.wipe();
                self.send_engine(EngineCommand::SyncNow);
                self.screen = ActiveScreen::E1Enrollment;
            }
        });
    }

    fn render_screen_m(&mut self, ui: &mut egui::Ui, active_tab: MainTab) {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(active_tab == MainTab::WhatsCollected, "Qué se recolecta")
                .clicked()
            {
                self.screen = ActiveScreen::MMain {
                    active_tab: MainTab::WhatsCollected,
                };
            }
            if ui
                .selectable_label(active_tab == MainTab::MySchedule, "Mi horario")
                .clicked()
            {
                self.screen = ActiveScreen::MMain {
                    active_tab: MainTab::MySchedule,
                };
            }
            if ui
                .selectable_label(active_tab == MainTab::MyRights, "Mis derechos")
                .clicked()
            {
                self.screen = ActiveScreen::MMain {
                    active_tab: MainTab::MyRights,
                };
            }
            if ui
                .selectable_label(active_tab == MainTab::Diagnostics, "Diagnóstico")
                .clicked()
            {
                self.screen = ActiveScreen::MMain {
                    active_tab: MainTab::Diagnostics,
                };
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        match active_tab {
            MainTab::WhatsCollected => self.render_tab_whats_collected(ui),
            MainTab::MySchedule => self.render_tab_schedule(ui),
            MainTab::MyRights => self.render_tab_rights(ui),
            MainTab::Diagnostics => self.render_tab_diagnostics(ui),
        }
    }

    fn render_tab_whats_collected(&mut self, ui: &mut egui::Ui) {
        ui.heading("Categorías de Telemetría");
        ui.add_space(6.0);

        if let RunMode::EmployerSuspended { .. } = self.run_mode {
            ui.label(
                RichText::new("La cuenta de tu empleador no está activa: no se recolecta ni se envía nada. Tus derechos y tu consentimiento no cambian.")
                    .color(Color32::from_rgb(241, 196, 15)),
            );
            ui.add_space(6.0);
        }

        // DP-07: what runs is effective (server) ∩ accepted on this device.
        let accepted = self.cred_store.get_accepted_flags().unwrap_or_default();
        let (requested, descriptions) = match &self.worker_self {
            Some(ws) => (ws.requested_consent_flags, ws.consent_descriptions.clone()),
            None => (accepted, Default::default()),
        };
        let effective = self
            .worker_self
            .as_ref()
            .map(|ws| crate::consent::intersect(&ws.consent_flags, &accepted))
            .unwrap_or(accepted);

        Self::render_category_state(
            ui,
            "Aplicaciones en uso",
            requested.system_activity,
            effective.system_activity,
            descriptions.system_activity.as_deref(),
        );
        Self::render_category_state(
            ui,
            "Navegación web (solo dominios)",
            requested.network_activity,
            effective.network_activity,
            descriptions.network_activity.as_deref(),
        );
        Self::render_category_state(
            ui,
            "Actividad de teclado y mouse (conteo)",
            requested.productivity_basic,
            effective.productivity_basic,
            descriptions.productivity_basic.as_deref(),
        );
        Self::render_category_state(
            ui,
            "Capturas de pantalla",
            requested.productivity_screenshots,
            effective.productivity_screenshots,
            descriptions.productivity_screenshots.as_deref(),
        );
        Self::render_category_state(
            ui,
            "Metadatos de archivos",
            requested.file_activity,
            effective.file_activity,
            descriptions.file_activity.as_deref(),
        );
        Self::render_category_state(
            ui,
            "Ubicación geográfica",
            requested.location,
            effective.location,
            descriptions.location.as_deref(),
        );

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Cambiar mi selección").clicked() {
                self.open_consent_from_server();
            }
            if ui
                .button(
                    RichText::new("Retirar consentimiento").color(Color32::from_rgb(231, 76, 60)),
                )
                .clicked()
            {
                self.screen = ActiveScreen::WWithdrawConfirm;
            }
        });
    }

    fn render_tab_schedule(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mi Horario Laboral");
        ui.add_space(6.0);
        match &self.worker_self {
            Some(ws) => match (&ws.work_hours_start, &ws.work_hours_end) {
                (Some(s), Some(e)) => {
                    ui.label(format!("Horario: {} a {}", s, e));
                    ui.label(format!(
                        "Zona horaria: {}",
                        ws.timezone.as_deref().unwrap_or("no definida")
                    ));
                }
                _ => {
                    ui.label("Su empleador no definió un horario laboral.");
                }
            },
            None => {
                ui.label("Horario no disponible: aún no hay conexión con el servidor.");
            }
        }
        ui.add_space(6.0);
        ui.label(RichText::new("Ubicación y capturas de pantalla solo se recolectan dentro de esta ventana horaria.").small().color(Color32::LIGHT_GRAY));
    }

    fn render_tab_rights(&mut self, ui: &mut egui::Ui) {
        ui.heading("Derechos del Titular de Datos (ARCO / GDPR)");
        ui.add_space(6.0);
        ui.label(RichText::new("Sus datos están cifrados con la clave de su empleador; los datos exportados se entregan cifrados y solo pueden ser leídos con dicha clave.").italics().color(Color32::LIGHT_GRAY));

        ui.add_space(10.0);

        if let Some(ref rid) = self.last_arco_request_id {
            ui.group(|ui| {
                ui.label(RichText::new("Última solicitud registrada:").strong());
                ui.label(format!("ID de solicitud: {}", rid));
                if let Some(ref status) = self.last_arco_status {
                    ui.label(format!("Estado: {}", status));
                }
                if let Some(ref msg) = self.last_arco_message {
                    ui.label(format!("Detalle: {}", msg));
                }
            });
            ui.add_space(8.0);
        }

        ui.horizontal(|ui| {
            let export = egui::Button::new("📥 Solicitar copia de mis datos");
            if ui.add_enabled(!self.busy, export).clicked() {
                match self.api() {
                    Ok(api) => {
                        let creds = self.cred_store.clone();
                        self.spawn(&ui.ctx().clone(), async move {
                            let categories =
                                ["system", "network", "productivity", "files", "location"];
                            let res = with_access_token(&api, creds.as_ref(), |token| {
                                let api = api.clone();
                                async move { api.post_data_request(&token, &categories).await }
                            })
                            .await;
                            UiEvent::DataRequest(res)
                        });
                    }
                    Err(e) => self.error_message = Some(e),
                }
            }

            if !self.show_deletion_confirm
                && ui
                    .button(
                        RichText::new("🗑️ Solicitar eliminación de mis datos")
                            .color(Color32::from_rgb(231, 76, 60)),
                    )
                    .clicked()
            {
                self.show_deletion_confirm = true;
            }
        });

        if self.show_deletion_confirm {
            ui.add_space(8.0);
            ui.group(|ui| {
                ui.label(RichText::new("¿Confirmar solicitud formal de supresión de datos?").strong().color(Color32::from_rgb(231, 76, 60)));
                ui.label("Se enviará una solicitud de purga a la organización conforme al derecho al olvido laboral.");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Confirmar Supresión").strong().color(Color32::from_rgb(231, 76, 60))).clicked() {
                        self.show_deletion_confirm = false;
                        match self.api() {
                            Ok(api) => {
                                let creds = self.cred_store.clone();
                                self.spawn(&ui.ctx().clone(), async move {
                                    let res = with_access_token(&api, creds.as_ref(), |token| {
                                        let api = api.clone();
                                        async move {
                                            api.post_data_deletion(&token, Some("Derecho al olvido laboral"))
                                                .await
                                        }
                                    })
                                    .await;
                                    UiEvent::DataDeletion(res)
                                });
                            }
                            Err(e) => self.error_message = Some(e),
                        }
                    }
                    if ui.button("Cancelar").clicked() {
                        self.show_deletion_confirm = false;
                    }
                });
            });
        }
    }

    fn render_tab_diagnostics(&mut self, ui: &mut egui::Ui) {
        ui.heading("Diagnóstico del Agente");
        ui.add_space(6.0);

        let (kpm, cpm) = KeystrokeCollector::get_current_counts();
        ui.label(format!("Pulsaciones de teclado (minuto): {}", kpm));
        ui.label(format!("Clics de mouse (minuto): {}", cpm));

        let fp = self
            .cred_store
            .get_pinned_fingerprint()
            .unwrap_or_else(|| "No fijada".into());
        let fp_preview = if fp.len() >= 16 { &fp[..16] } else { &fp };
        ui.label(format!("Huella digital de la clave admin: {}", fp_preview));

        ui.label(format!(
            "Versión del cliente: {}",
            crate::api::client::APP_VERSION
        ));
    }

    fn render_screen_w(&mut self, ui: &mut egui::Ui) {
        ui.heading(RichText::new("¿Retirar consentimiento?").color(Color32::from_rgb(231, 76, 60)));
        ui.add_space(8.0);
        ui.label("Si retira su consentimiento, toda la recolección se detendrá inmediatamente y los datos no enviados en este equipo serán eliminados.");

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui.button("Cancelar").clicked() {
                self.screen = ActiveScreen::MMain {
                    active_tab: MainTab::WhatsCollected,
                };
            }

            if ui
                .button(
                    RichText::new("Confirmar Retiro")
                        .strong()
                        .color(Color32::from_rgb(231, 76, 60)),
                )
                .clicked()
            {
                // DP-09 / DP-10: stop and purge immediately; the engine purges the
                // on-disk queue and sends `accepted: false` with the current version.
                let _ = self.buffer.drain_for_flush();
                self.send_engine(EngineCommand::Withdraw);
                self.run_mode = RunMode::AwaitingConsent {
                    reason: AwaitingReason::NoConsent,
                };
                self.screen = ActiveScreen::IInactive;
            }
        });
    }

    fn render_screen_r(&mut self, ui: &mut egui::Ui) {
        ui.heading("Equipo Desvinculado");
        ui.add_space(8.0);
        ui.label("Este equipo fue desvinculado por su empleador. Las credenciales y claves locales fueron eliminadas.");

        ui.add_space(12.0);
        if ui.button("Vincular nuevamente").clicked() {
            self.screen = ActiveScreen::E1Enrollment;
        }
    }

    fn render_screen_h(&mut self, ui: &mut egui::Ui, server_fp: &str, pinned_fp: &str) {
        ui.heading(
            RichText::new("Fallo de Seguridad: Clave Alterada")
                .color(Color32::from_rgb(231, 76, 60)),
        );
        ui.add_space(8.0);
        ui.label("La clave del empleador reportada por el servidor no coincide con la fijada durante la vinculación. El envío de telemetría ha sido suspendido.");

        ui.add_space(6.0);
        ui.label(format!("Huella fijada: {}", pinned_fp));
        ui.label(format!("Huella recibida: {}", server_fp));

        ui.add_space(12.0);
        if ui.button("Desvincular y Limpiar").clicked() {
            let _ = self.buffer.drain_for_flush();
            let _ = self.cred_store.wipe();
            self.send_engine(EngineCommand::SyncNow);
            self.screen = ActiveScreen::E1Enrollment;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api_error(status: u16, reason: Option<&str>, retry: Option<u64>) -> EnrollmentError {
        EnrollmentError::Api(crate::errors::AgentError::Api {
            status,
            code: "X".into(),
            reason: reason.map(str::to_string),
            retry_after_seconds: retry,
            details: None,
        })
    }

    #[test]
    fn test_enrollment_error_messages_by_reason() {
        assert!(
            enrollment_error_message(&api_error(400, Some("INVALID_ENROLLMENT_TOKEN"), None))
                .contains("inválido o expiró")
        );
        assert!(enrollment_error_message(&api_error(
            409,
            Some("ENROLLMENT_ALREADY_COMPLETED"),
            None
        ))
        .contains("ya fue utilizado"));
        assert!(
            enrollment_error_message(&api_error(409, Some("DEVICE_ALREADY_ENROLLED"), None))
                .contains("ya está vinculado a otro trabajador")
        );
        assert!(
            enrollment_error_message(&api_error(429, Some("ENROLLMENT_LOCKED"), Some(900)))
                .contains("15 minuto")
        );
        assert!(
            enrollment_error_message(&api_error(429, Some("ENROLLMENT_LOCKED"), Some(61)))
                .contains("2 minuto")
        );
        assert!(enrollment_error_message(&api_error(429, None, None))
            .contains("Demasiadas solicitudes"));
        assert!(enrollment_error_message(&EnrollmentError::Api(
            crate::errors::AgentError::Network("down".into())
        ))
        .contains("No se pudo contactar"));
    }

    #[test]
    fn test_enrollment_input_manual_code_normalized() {
        let input =
            parse_enrollment_input(false, "", "k7qm-4xrt-9pwo", "https://api.acme.cl/api/v1")
                .unwrap();
        assert_eq!(
            input,
            EnrollmentInput::Manual {
                api_url: "https://api.acme.cl/api/v1".into(),
                code: "K7QM4XRT9PW0".into(),
            }
        );
        assert!(parse_enrollment_input(false, "", "K7QM", "https://x").is_err());
        assert!(parse_enrollment_input(false, "", "K7QM-4XRT-9PWD", " ").is_err());
    }

    #[test]
    fn test_enrollment_input_pasted_qr_carries_server_url() {
        let qr = EnrollmentQr::new(
            "https://api.acme.cl/api/v1".into(),
            "K7QM-4XRT-9PWD".into(),
            "a".repeat(64),
        )
        .unwrap();
        let input = parse_enrollment_input(true, &qr.to_json().unwrap(), "", "").unwrap();
        assert_eq!(input.api_url("fallback"), "https://api.acme.cl/api/v1");
        assert!(matches!(input, EnrollmentInput::Qr(_)));
        assert!(parse_enrollment_input(true, "not a qr", "", "").is_err());
        // RF-AGENT-09-8: the API's canonical base64 payload works the same way.
        let from_base64 = parse_enrollment_input(true, &qr.to_payload().unwrap(), "", "").unwrap();
        assert_eq!(from_base64, input);
        // SEC-10: a QR pointing to plain http is rejected outside debug builds.
        let http_qr = EnrollmentQr::new(
            "http://api.acme.cl/api/v1".into(),
            "K7QM-4XRT-9PWD".into(),
            "a".repeat(64),
        )
        .unwrap();
        assert_eq!(
            parse_enrollment_input(true, &http_qr.to_json().unwrap(), "", "").is_ok(),
            cfg!(debug_assertions)
        );
    }
}
