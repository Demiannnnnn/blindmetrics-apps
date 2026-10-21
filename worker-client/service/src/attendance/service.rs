//! Attendance service: the only code path that creates marks (ATT-01) and the owner of the local
//! journal, the mark queue, terms acceptance, the PIN and the local attendance notices.
//!
//! Network calls use the worker session with single-flight refresh (SEC-14). No telemetry state
//! is read anywhere in this file (ATT-07).

use super::journal::StoredMark;
use super::reminders::{self, AgreedTimes};
use super::{
    format_ts, parse_ts, AttendanceNotice, Clock, LocationProvider, MarkJournal, Notifier, OpError,
    PinAttempts,
};
use crate::api::client::{
    ApiClient, CredentialEvent, MarkRequest, MarkResponse, TermsAcceptanceRequest,
};
use crate::background::worker::{EngineCommand, EngineSnapshot};
use crate::credentials::CredentialStore;
use crate::errors::AgentError;
use crate::io::transport::{BackoffPolicy, OfflineQueue};
use crate::legal;
use crate::session::{with_access_token, SessionError};
use crate::state::{derive_attendance_mode, AttendanceMode, WorkerSelf};
use agent_ipc::{
    ConsentRecord, Correction, DataSource, DayState, DaySummary, LegalStatus, LocalMarkStatus,
    MarkLocation, MarkRecord, MarkType, MyScheduleView, Pagination, SecretPin, ServerMark,
    TodayResponse, TodayView,
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Utc};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard, RwLock as StdRwLock};
use telemetry_crypto_core::{
    validate_pin, verify_mark_hash, CanonicalMark, PinPolicyError, PinVerifier,
};
use tokio::sync::{mpsc, watch, Mutex, Notify};

/// After a verified reset code, `SET_PIN` works without the current PIN for this long.
pub const PIN_RESET_WINDOW_SECS: u64 = 600;
/// `/attendance/me/today` refresh period (spec §10).
pub const TODAY_REFRESH_SECS: i64 = 300;
/// Location reading budget per mark (ATT-08).
pub const LOCATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
/// Longest idle wait of the background loop.
const LOOP_TICK: std::time::Duration = std::time::Duration::from_secs(15);
const MARKS_PER_PAGE: u32 = 50;
const MAX_DAYS_RANGE: i64 = 62;
const HISTORY_YEARS: i32 = 5;

/// Everything the service needs from the outside, injectable in tests.
pub struct AttendanceDeps {
    pub creds: Arc<dyn CredentialStore>,
    pub journal: MarkJournal,
    /// `offline.db` path (telemetry queue in the same file); `None` = in-memory (tests).
    pub db_path: Option<PathBuf>,
    pub clock: Arc<dyn Clock>,
    pub notifier: Arc<dyn Notifier>,
    pub location: Arc<dyn LocationProvider>,
}

struct TodayCache {
    response: TodayResponse,
    offset_secs: Option<i64>,
}

struct DrainState {
    backoff: BackoffPolicy,
    next_retry_at: Option<DateTime<Utc>>,
}

/// Result of one pass over the mark queue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrainReport {
    pub sent: usize,
    /// Sent marks that had been stored offline (they trigger the notice).
    pub offline_sent: usize,
    pub rejected: usize,
    /// The pass stopped on a network/server error; the rest stays queued.
    pub interrupted: bool,
}

enum SendOutcome {
    Accepted(Box<MarkResponse>),
    /// `400 VALIDATION_ERROR` (reason) — the only answer that drops a mark (ATT-04).
    Rejected(String),
    Failed(SessionError),
}

/// Attendance part of the IPC snapshot (RF-AGENT-12-12).
#[derive(Clone, Debug, PartialEq)]
pub struct AttendanceSnapshot {
    pub mode: AttendanceMode,
    pub today: Option<TodayView>,
    pub pending_marks: usize,
    pub pending_offline_marks: usize,
    pub pending_corrections: u32,
    pub legal: LegalStatus,
    pub work_mode: Option<String>,
    pub location_at_mark: bool,
    /// The service can read a position when marking (RF-AGENT-15-1).
    pub location_available: bool,
}

pub struct AttendanceService {
    api: ApiClient,
    creds: Arc<dyn CredentialStore>,
    journal: StdMutex<MarkJournal>,
    db_path: Option<PathBuf>,
    clock: Arc<dyn Clock>,
    notifier: Arc<dyn Notifier>,
    location: Arc<dyn LocationProvider>,
    engine_state: watch::Receiver<EngineSnapshot>,
    engine_commands: Option<mpsc::UnboundedSender<EngineCommand>>,
    pin_attempts: StdMutex<PinAttempts>,
    reset_window_until: StdMutex<Option<DateTime<Utc>>>,
    today: StdRwLock<Option<TodayCache>>,
    online: AtomicBool,
    send_lock: Mutex<()>,
    drain: StdMutex<DrainState>,
    changes: watch::Sender<u64>,
    wake: Notify,
}

fn lock<T>(m: &StdMutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// True for errors that mean "no connection / server unavailable" (keep queued, retry later).
pub fn is_transient(err: &SessionError) -> bool {
    match err {
        SessionError::Api(AgentError::Network(_))
        | SessionError::Api(AgentError::Reqwest(_))
        | SessionError::Api(AgentError::Io(_)) => true,
        SessionError::Api(AgentError::ServerError(status))
        | SessionError::Api(AgentError::Api { status, .. }) => *status >= 500 || *status == 429,
        _ => false,
    }
}

/// Recomputes the hash of a mark returned by the server (ATT-03, `security-model.md` §9.3).
/// A mark without `worker_run`/`employer_rut` cannot be verified and counts as a mismatch.
pub fn verify_server_mark(mark: &ServerMark) -> bool {
    let (Some(worker_run), Some(employer_rut)) = (&mark.worker_run, &mark.employer_rut) else {
        return false;
    };
    let workplace_id = mark
        .workplace_id
        .clone()
        .or_else(|| mark.workplace.as_ref().map(|w| w.id.clone()));
    let canonical = CanonicalMark {
        v: 1,
        mark_id: mark.id.clone(),
        worker_id: mark.worker_id.clone(),
        worker_run: worker_run.clone(),
        employer_rut: employer_rut.clone(),
        r#type: mark.r#type,
        server_ts: mark.server_ts.clone(),
        device_ts: mark.device_ts.clone(),
        offline: mark.offline,
        source: mark.source,
        workplace_id,
        location: mark.location.clone(),
        prev_hash: mark.prev_hash.clone(),
    };
    verify_mark_hash(&canonical, &mark.hash)
}

fn policy_error(e: PinPolicyError) -> OpError {
    let (code, message) = match e {
        PinPolicyError::NonDigits => ("PIN_POLICY_NON_DIGITS", "El PIN solo puede tener números."),
        PinPolicyError::TooShort => (
            "PIN_POLICY_TOO_SHORT",
            "El PIN debe tener entre 6 y 8 dígitos.",
        ),
        PinPolicyError::TooLong => (
            "PIN_POLICY_TOO_LONG",
            "El PIN debe tener entre 6 y 8 dígitos.",
        ),
        PinPolicyError::AllDigitsEqual => (
            "PIN_POLICY_ALL_DIGITS_EQUAL",
            "No uses el mismo número repetido, como 111111.",
        ),
        PinPolicyError::SequentialDigits => (
            "PIN_POLICY_SEQUENTIAL_DIGITS",
            "No uses números repetidos ni seguidos como 123456.",
        ),
        PinPolicyError::MatchesWorkerRun => (
            "PIN_POLICY_MATCHES_WORKER_RUN",
            "No uses los números de tu RUN.",
        ),
    };
    OpError::new(code, message)
}

fn store_error(e: impl std::fmt::Display) -> OpError {
    tracing::error!("attendance local storage error: {e}");
    OpError::new(
        "STORAGE_ERROR",
        "No se pudo guardar la información en este equipo.",
    )
}

fn parse_date(raw: &str) -> Result<NaiveDate, OpError> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
        .map_err(|_| OpError::new("INVALID_INPUT", "Usa fechas con formato AAAA-MM-DD."))
}

impl AttendanceService {
    pub fn new(
        api_base_url: &str,
        deps: AttendanceDeps,
        engine_state: watch::Receiver<EngineSnapshot>,
        engine_commands: Option<mpsc::UnboundedSender<EngineCommand>>,
    ) -> Result<Arc<Self>, AgentError> {
        let (changes, _) = watch::channel(0u64);
        Ok(Arc::new(Self {
            api: ApiClient::new(api_base_url.to_string())?,
            creds: deps.creds,
            journal: StdMutex::new(deps.journal),
            db_path: deps.db_path,
            clock: deps.clock,
            notifier: deps.notifier,
            location: deps.location,
            engine_state,
            engine_commands,
            pin_attempts: StdMutex::new(PinAttempts::default()),
            reset_window_until: StdMutex::new(None),
            today: StdRwLock::new(None),
            online: AtomicBool::new(true),
            send_lock: Mutex::new(()),
            drain: StdMutex::new(DrainState {
                backoff: BackoffPolicy::default(),
                next_retry_at: None,
            }),
            changes,
            wake: Notify::new(),
        }))
    }

    // ---- shared helpers ---------------------------------------------------------------------

    /// Notified on every change that affects the snapshot.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    fn changed(&self) {
        self.changes.send_modify(|v| *v = v.wrapping_add(1));
    }

    fn worker_self(&self) -> Option<WorkerSelf> {
        self.engine_state.borrow().worker_self.clone()
    }

    fn linked(&self) -> bool {
        self.creds.get_access_token().is_some()
    }

    fn sync_engine(&self) {
        if let Some(tx) = &self.engine_commands {
            let _ = tx.send(EngineCommand::SyncNow);
        }
    }

    fn worker_tz(&self) -> Option<chrono_tz::Tz> {
        let cached = self
            .today
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .and_then(|c| c.response.timezone.clone());
        cached
            .or_else(|| self.worker_self().and_then(|m| m.timezone))
            .and_then(|tz| tz.parse::<chrono_tz::Tz>().ok())
    }

    /// Wall time in the worker's timezone (device timezone when unknown).
    fn local_naive(&self, ts: DateTime<Utc>) -> NaiveDateTime {
        match self.worker_tz() {
            Some(tz) => ts.with_timezone(&tz).naive_local(),
            None => ts.with_timezone(&chrono::Local).naive_local(),
        }
    }

    fn local_date_of(&self, raw_ts: &str) -> Option<NaiveDate> {
        parse_ts(raw_ts).map(|ts| self.local_naive(ts).date())
    }

    /// Maps a failed call to an IPC error; side effects: offline flag, engine resync.
    fn session_error(&self, err: SessionError) -> OpError {
        if is_transient(&err) {
            self.online.store(false, Ordering::SeqCst);
            return OpError::new(
                "OFFLINE",
                "Sin conexión con el servidor. Inténtalo de nuevo más tarde.",
            );
        }
        match err {
            SessionError::NotEnrolled => {
                OpError::new("NOT_ENROLLED", "Este equipo no está vinculado.")
            }
            SessionError::Revoked => {
                self.sync_engine();
                OpError::new("REVOKED", "Tu empleador desvinculó este equipo.")
            }
            SessionError::Api(AgentError::Api {
                code,
                reason,
                retry_after_seconds,
                ..
            }) => {
                let code = reason.unwrap_or(code);
                OpError::new(&code, "El servidor rechazó la solicitud.")
                    .with_retry(retry_after_seconds)
            }
            other => {
                tracing::warn!("attendance call failed: {other}");
                OpError::new("API_ERROR", "No se pudo completar la solicitud.")
            }
        }
    }

    // ---- modes and snapshot -----------------------------------------------------------------

    /// Current [`AttendanceMode`] (no telemetry input, ATT-07).
    pub fn mode(&self) -> AttendanceMode {
        let me = self.worker_self();
        derive_attendance_mode(
            self.linked(),
            me.as_ref(),
            self.creds.get_terms_accepted_version().as_deref(),
            self.creds.get_pin_verifier().is_some(),
        )
    }

    /// `(pending, pending stored offline)` of the journal.
    pub fn pending_counts(&self) -> (usize, usize) {
        lock(&self.journal).pending_counts().unwrap_or((0, 0))
    }

    /// `server_time - device clock` from the last `/attendance/me/today` (ATT-10).
    pub fn server_time_offset(&self) -> Option<i64> {
        self.today
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .and_then(|c| c.offset_secs)
    }

    pub fn snapshot(&self) -> AttendanceSnapshot {
        let me = self.worker_self();
        let mode = self.mode();
        let (pending_marks, pending_offline_marks) = self.pending_counts();
        let pending_corrections = self
            .today
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|c| c.response.pending_corrections)
            .unwrap_or(0);
        let legal = LegalStatus {
            accepted_version: me
                .as_ref()
                .and_then(|m| m.terms_accepted_version.clone())
                .or_else(|| self.creds.get_terms_accepted_version()),
            required_version: me.as_ref().and_then(|m| m.terms_required_version.clone()),
            embedded_terms_version: Some(legal::worker_terms_id().version),
            embedded_privacy_version: Some(legal::privacy_policy_id().version),
        };
        AttendanceSnapshot {
            mode,
            today: (mode != AttendanceMode::Unlinked).then(|| self.today_view()),
            pending_marks,
            pending_offline_marks,
            pending_corrections,
            legal,
            work_mode: me
                .as_ref()
                .and_then(|m| m.work_mode)
                .map(|w| w.as_str().to_string()),
            location_at_mark: me.as_ref().is_some_and(|m| m.effective_location_at_mark()),
            location_available: self.location.available(),
        }
    }

    /// First tooltip line of the tray (spec §7).
    pub fn tray_line(&self) -> String {
        let (pending, _) = self.pending_counts();
        match self.mode() {
            AttendanceMode::Unlinked => return "Equipo sin vincular".to_string(),
            AttendanceMode::Exempt => return "Asistencia: no aplica (art. 22)".to_string(),
            AttendanceMode::NeedsTerms | AttendanceMode::NeedsPin => {
                return "Asistencia: completa la configuración".to_string()
            }
            AttendanceMode::Ready { .. } => {}
        }
        if pending == 1 {
            return "1 marca pendiente de envío".to_string();
        }
        if pending > 1 {
            return format!("{pending} marcas pendientes de envío");
        }
        let cached = self
            .today
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|c| c.response.clone());
        let Some(today) = cached else {
            return "Asistencia".to_string();
        };
        let since = |t: MarkType| {
            today
                .marks
                .iter()
                .rev()
                .find(|m| m.r#type == t)
                .and_then(|m| parse_ts(m.effective_ts.as_deref().unwrap_or(&m.server_ts)))
        };
        let hhmm = |ts: DateTime<Utc>| self.local_naive(ts).format("%H:%M").to_string();
        match today.state {
            DayState::Working => match since(MarkType::WorkdayStart) {
                Some(ts) => format!("En jornada desde {}", hhmm(ts)),
                None => "En jornada".to_string(),
            },
            DayState::OnBreak => match since(MarkType::BreakStart) {
                Some(ts) => format!("En colación desde {}", hhmm(ts)),
                None => "En colación".to_string(),
            },
            DayState::Off | DayState::Unknown => "Fuera de jornada".to_string(),
        }
    }

    // ---- terms (LEG-04, LEG-05) -------------------------------------------------------------

    /// `ACCEPT_TERMS`: only the embedded texts can be accepted.
    pub async fn accept_terms(
        &self,
        version: &str,
        hash: &str,
        privacy_version: &str,
        privacy_hash: &str,
    ) -> Result<String, OpError> {
        if !self.linked() {
            return Err(OpError::new(
                "NOT_ENROLLED",
                "Este equipo no está vinculado.",
            ));
        }
        if !legal::matches_embedded(version, hash, privacy_version, privacy_hash) {
            return Err(OpError::new(
                "TERMS_MISMATCH",
                "Los términos aceptados no coinciden con los que muestra esta app.",
            ));
        }
        let terms = legal::worker_terms_id();
        let privacy = legal::privacy_policy_id();
        let api = self.api.clone();
        let res = with_access_token(&api, self.creds.as_ref(), |token| {
            let api = api.clone();
            let terms = terms.clone();
            let privacy = privacy.clone();
            async move {
                let body = TermsAcceptanceRequest {
                    document: legal::WORKER_TERMS_DOCUMENT,
                    version: &terms.version,
                    hash: &terms.hash,
                    privacy_version: &privacy.version,
                    privacy_hash: &privacy.hash,
                };
                api.accept_terms(&token, &body).await
            }
        })
        .await;
        match res {
            Ok(resp) => {
                self.creds
                    .set_terms_accepted_version(Some(&resp.terms_accepted_version))
                    .map_err(store_error)?;
                tracing::info!("worker terms accepted");
                self.sync_engine();
                self.changed();
                Ok(resp.terms_accepted_version)
            }
            Err(SessionError::Api(ref e)) if e.reason() == Some("TERMS_VERSION_MISMATCH") => {
                Err(OpError::new(
                    "TERMS_OUTDATED",
                    "Hay una versión más nueva de esta app. Actualízala para continuar.",
                ))
            }
            Err(e) => Err(self.session_error(e)),
        }
    }

    // ---- PIN (SEC-19, SEC-20) ---------------------------------------------------------------

    fn reset_window_active(&self) -> bool {
        lock(&self.reset_window_until).is_some_and(|until| self.clock.now() < until)
    }

    /// Checks a PIN with the growing delay of RF-AGENT-12-6 (never a permanent lock).
    async fn check_pin(&self, pin: &SecretPin) -> Result<(), OpError> {
        let Some(verifier) = self.creds.get_pin_verifier() else {
            return Err(OpError::new(
                "PIN_NOT_SET",
                "Crea tu PIN de asistencia antes de continuar.",
            ));
        };
        if let Err(wait) = lock(&self.pin_attempts).check(self.clock.now()) {
            return Err(OpError::new(
                "PIN_RETRY_LATER",
                format!("Espera {wait} s para reintentar."),
            )
            .with_retry(Some(wait)));
        }
        let candidate = pin.expose().clone();
        let ok = tokio::task::spawn_blocking(move || verifier.verify(&candidate))
            .await
            .unwrap_or(false);
        let mut attempts = lock(&self.pin_attempts);
        if ok {
            attempts.record_success();
            return Ok(());
        }
        let delay = attempts.record_failure(self.clock.now());
        tracing::info!(failures = attempts.failures(), "wrong attendance PIN");
        let message = if delay > 0 {
            format!("PIN incorrecto. Espera {delay} s para reintentar.")
        } else {
            "PIN incorrecto.".to_string()
        };
        Err(OpError::new("PIN_INCORRECT", message).with_retry(Some(delay)))
    }

    async fn store_new_pin(&self, pin: &SecretPin) -> Result<(), OpError> {
        let run = self.worker_self().and_then(|m| m.run);
        validate_pin(pin.expose(), run.as_deref()).map_err(policy_error)?;
        let candidate = pin.expose().clone();
        let verifier = tokio::task::spawn_blocking(move || PinVerifier::create(&candidate))
            .await
            .map_err(store_error)?
            .map_err(store_error)?;
        self.creds
            .set_pin_verifier(&verifier)
            .map_err(store_error)?;
        lock(&self.pin_attempts).record_success();
        Ok(())
    }

    /// Reports a PIN event; on failure it is kept and retried later (never blocks, RF-AGENT-12-5).
    async fn report_pin_event(&self, event: CredentialEvent) {
        let mut queue: Vec<CredentialEvent> = self
            .creds
            .get_pending_pin_event()
            .map(|raw| raw.split(',').filter_map(CredentialEvent::parse).collect())
            .unwrap_or_default();
        queue.push(event);
        self.deliver_pin_events(queue).await;
    }

    async fn deliver_pin_events(&self, mut queue: Vec<CredentialEvent>) {
        while let Some(event) = queue.first().copied() {
            let api = self.api.clone();
            let res = with_access_token(&api, self.creds.as_ref(), |token| {
                let api = api.clone();
                async move { api.post_credential_event(&token, event).await }
            })
            .await;
            match res {
                Ok(()) => {
                    queue.remove(0);
                }
                Err(e) => {
                    tracing::info!("PIN event not delivered yet, will retry: {e}");
                    break;
                }
            }
        }
        let pending = queue
            .iter()
            .map(|e| e.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let _ = self
            .creds
            .set_pending_pin_event((!pending.is_empty()).then_some(pending.as_str()));
    }

    /// Retries PIN events that could not be delivered (background loop).
    pub async fn retry_pin_events(&self) {
        let Some(raw) = self.creds.get_pending_pin_event() else {
            return;
        };
        if !self.linked() {
            return;
        }
        let queue = raw.split(',').filter_map(CredentialEvent::parse).collect();
        self.deliver_pin_events(queue).await;
    }

    /// `SET_PIN`: first PIN, or a new one right after a verified reset code.
    pub async fn set_pin(&self, pin: &SecretPin) -> Result<(), OpError> {
        if !self.linked() {
            return Err(OpError::new(
                "NOT_ENROLLED",
                "Este equipo no está vinculado.",
            ));
        }
        let existed = self.creds.get_pin_verifier().is_some();
        if existed && !self.reset_window_active() {
            return Err(OpError::new(
                "PIN_ALREADY_SET",
                "Ya tienes un PIN. Usa Cambiar PIN o Olvidé mi PIN.",
            ));
        }
        self.store_new_pin(pin).await?;
        *lock(&self.reset_window_until) = None;
        tracing::info!("attendance PIN set");
        self.changed();
        let event = if existed {
            CredentialEvent::PinChanged
        } else {
            CredentialEvent::PinSet
        };
        self.report_pin_event(event).await;
        Ok(())
    }

    /// `CHANGE_PIN { current, new }`.
    pub async fn change_pin(&self, current: &SecretPin, new: &SecretPin) -> Result<(), OpError> {
        if !self.linked() {
            return Err(OpError::new(
                "NOT_ENROLLED",
                "Este equipo no está vinculado.",
            ));
        }
        self.check_pin(current).await?;
        self.store_new_pin(new).await?;
        tracing::info!("attendance PIN changed");
        self.changed();
        self.report_pin_event(CredentialEvent::PinChanged).await;
        Ok(())
    }

    /// `REQUEST_PIN_RESET`: the server emails a code to the worker's personal email.
    pub async fn request_pin_reset(&self) -> Result<(), OpError> {
        let api = self.api.clone();
        with_access_token(&api, self.creds.as_ref(), |token| {
            let api = api.clone();
            async move { api.request_pin_reset(&token).await }
        })
        .await
        .map_err(|e| self.reset_error(e))
    }

    /// `VERIFY_PIN_RESET { code }` → opens the 10-minute `SET_PIN` window.
    pub async fn verify_pin_reset(&self, code: &str) -> Result<u64, OpError> {
        let code = code.trim();
        if code.is_empty() || code.len() > 32 || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(OpError::new(
                "INVALID_RESET_CODE",
                "El código no es válido o ya venció.",
            ));
        }
        let api = self.api.clone();
        let owned = code.to_string();
        with_access_token(&api, self.creds.as_ref(), |token| {
            let api = api.clone();
            let code = owned.clone();
            async move { api.verify_pin_reset(&token, &code).await }
        })
        .await
        .map_err(|e| self.reset_error(e))?;
        *lock(&self.reset_window_until) =
            Some(self.clock.now() + Duration::seconds(PIN_RESET_WINDOW_SECS as i64));
        lock(&self.pin_attempts).record_success();
        tracing::info!("PIN reset code verified");
        Ok(PIN_RESET_WINDOW_SECS)
    }

    fn reset_error(&self, err: SessionError) -> OpError {
        match &err {
            SessionError::Api(AgentError::Api {
                status: 429,
                retry_after_seconds,
                ..
            }) => {
                return OpError::new(
                    "RATE_LIMITED",
                    "Hiciste demasiados intentos. Espera un momento.",
                )
                .with_retry(*retry_after_seconds)
            }
            SessionError::Api(e) if e.reason() == Some("INVALID_RESET_CODE") => {
                return OpError::new("INVALID_RESET_CODE", "El código no es válido o ya venció.")
            }
            _ => {}
        }
        self.session_error(err)
    }

    // ---- marks (ATT-01..04, ATT-08, ATT-10) -------------------------------------------------

    /// `MARK { type, pin }`: the **only** way a mark is created (ATT-01). It is stored in the
    /// journal before any network call and never refused for subscription, telemetry or a later
    /// terms version (ATT-02).
    pub async fn mark(
        self: &Arc<Self>,
        r#type: MarkType,
        pin: &SecretPin,
    ) -> Result<MarkRecord, OpError> {
        match self.mode() {
            AttendanceMode::Unlinked => {
                return Err(OpError::new("NOT_ENROLLED", "Este equipo no está vinculado."))
            }
            AttendanceMode::Exempt => {
                return Err(OpError::new(
                    "ATTENDANCE_EXEMPT",
                    "Tu contrato no está sujeto a control de jornada (art. 22 del Código del Trabajo). No necesitas marcar.",
                ))
            }
            AttendanceMode::NeedsTerms => {
                return Err(OpError::new(
                    "TERMS_REQUIRED",
                    "Acepta los términos y la política de privacidad antes de tu primera marca.",
                ))
            }
            AttendanceMode::NeedsPin => {
                return Err(OpError::new(
                    "PIN_NOT_SET",
                    "Crea tu PIN de asistencia antes de marcar.",
                ))
            }
            AttendanceMode::Ready { .. } => {}
        }
        self.check_pin(pin).await?;

        let now = self.clock.now();
        let client_mark_id = uuid::Uuid::now_v7().to_string();
        let device_ts = format_ts(now);
        let worker_id = self.creds.get_worker_id();
        lock(&self.journal)
            .insert_pending(
                &client_mark_id,
                worker_id.as_deref(),
                r#type,
                &device_ts,
                now,
            )
            .map_err(store_error)?;
        tracing::info!(mark_type = ?r#type, offline = false, "mark queued");
        self.changed();

        // ATT-08: one best-effort reading, only when the schedule asks for it.
        if self
            .worker_self()
            .is_some_and(|m| m.effective_location_at_mark())
        {
            if let Some(location) = self.read_location().await {
                let _ = lock(&self.journal).set_location(&client_mark_id, &location);
            }
        }

        // Sends this mark after any older pending one (device_ts order).
        self.drain_pending().await;

        let service = self.clone();
        tokio::spawn(async move {
            let _ = service.refresh_today().await;
        });

        lock(&self.journal)
            .get(&client_mark_id)
            .map_err(store_error)?
            .map(|m| m.record)
            .ok_or_else(|| store_error("mark vanished from the journal"))
    }

    async fn read_location(&self) -> Option<MarkLocation> {
        let provider = self.location.clone();
        match tokio::time::timeout(
            LOCATION_TIMEOUT,
            tokio::task::spawn_blocking(move || provider.read_once()),
        )
        .await
        {
            Ok(Ok(location)) => location,
            _ => {
                tracing::info!("location at mark unavailable; mark sent without it (ATT-08)");
                None
            }
        }
    }

    async fn send_one(&self, row: &StoredMark) -> SendOutcome {
        let body = MarkRequest {
            client_mark_id: row.record.client_mark_id.clone(),
            r#type: row.record.r#type,
            device_ts: row.record.device_ts.clone(),
            offline: row.record.offline,
            identification: "DEVICE_PIN",
            location: row.location.clone(),
        };
        let api = self.api.clone();
        let res = with_access_token(&api, self.creds.as_ref(), |token| {
            let api = api.clone();
            let body = body.clone();
            async move { api.post_mark(&token, &body).await }
        })
        .await;
        match res {
            Ok(resp) => SendOutcome::Accepted(Box::new(resp)),
            Err(SessionError::Api(AgentError::Api {
                status: 400,
                code,
                reason,
                ..
            })) => SendOutcome::Rejected(reason.unwrap_or(code)),
            Err(e) => SendOutcome::Failed(e),
        }
    }

    fn store_accepted(&self, row: &StoredMark, resp: &MarkResponse) {
        let verified = verify_server_mark(&resp.mark);
        let status = if verified {
            LocalMarkStatus::Confirmed
        } else {
            tracing::warn!(mark_type = ?row.record.r#type, "mark receipt hash does not verify");
            LocalMarkStatus::HashMismatch
        };
        let payload = serde_json::to_string(resp).unwrap_or_default();
        if let Err(e) = lock(&self.journal).record_server_answer(
            &row.record.client_mark_id,
            status,
            &resp.mark,
            resp.duplicate_of.as_deref(),
            &payload,
        ) {
            tracing::error!("could not store the server answer of a mark: {e}");
        }
    }

    /// Sends every pending mark of the current worker in `device_ts` order (RF-AGENT-12-8).
    /// Idempotent by `client_mark_id`. Stops at the first network/server error; the remaining
    /// marks stay queued and are flagged `offline` (ATT-04).
    pub async fn drain_pending(&self) -> DrainReport {
        let _guard = self.send_lock.lock().await;
        let mut report = DrainReport::default();
        let Some(worker_id) = self.creds.get_worker_id() else {
            return report;
        };
        if !self.linked() {
            return report;
        }
        let rows = lock(&self.journal)
            .pending_for_worker(&worker_id)
            .unwrap_or_default();
        if rows.is_empty() {
            return report;
        }
        for (index, row) in rows.iter().enumerate() {
            match self.send_one(row).await {
                SendOutcome::Accepted(resp) => {
                    self.store_accepted(row, &resp);
                    self.online.store(true, Ordering::SeqCst);
                    report.sent += 1;
                    if row.record.offline {
                        report.offline_sent += 1;
                    }
                }
                SendOutcome::Rejected(reason) => {
                    tracing::warn!(mark_type = ?row.record.r#type, %reason, "mark rejected by the server");
                    let _ =
                        lock(&self.journal).record_rejection(&row.record.client_mark_id, &reason);
                    report.rejected += 1;
                }
                SendOutcome::Failed(err) => {
                    {
                        let journal = lock(&self.journal);
                        for rest in &rows[index..] {
                            let _ = journal.record_failed_attempt(&rest.record.client_mark_id);
                        }
                    }
                    if is_transient(&err) {
                        self.online.store(false, Ordering::SeqCst);
                    } else if matches!(err, SessionError::Revoked) {
                        self.sync_engine();
                    }
                    tracing::info!(
                        pending = rows.len() - index,
                        "marks kept in the offline queue: {err}"
                    );
                    report.interrupted = true;
                    break;
                }
            }
        }
        {
            let mut drain = lock(&self.drain);
            if report.interrupted {
                let delay = drain.backoff.next_backoff();
                let delay = Duration::from_std(delay).unwrap_or_else(|_| Duration::seconds(300));
                drain.next_retry_at = Some(self.clock.now() + delay);
            } else {
                drain.backoff.reset();
                drain.next_retry_at = None;
            }
        }
        if report.offline_sent > 0 {
            self.notifier.notify(&AttendanceNotice::OfflineMarksSent {
                count: report.offline_sent,
            });
        }
        self.changed();
        report
    }

    fn retry_due(&self, now: DateTime<Utc>) -> bool {
        let pending = self
            .creds
            .get_worker_id()
            .map(|w| {
                lock(&self.journal)
                    .pending_for_worker(&w)
                    .map(|r| !r.is_empty())
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        pending && lock(&self.drain).next_retry_at.map_or(true, |t| now >= t)
    }

    fn reset_backoff(&self) {
        let mut drain = lock(&self.drain);
        drain.backoff.reset();
        drain.next_retry_at = None;
    }

    // ---- reads (ATT-05, ATT-06, ATT-09) -----------------------------------------------------

    /// Refreshes the `/attendance/me/today` cache; detects new corrections (notice).
    pub async fn refresh_today(&self) -> Result<(), OpError> {
        if !self.linked() {
            return Err(OpError::new(
                "NOT_ENROLLED",
                "Este equipo no está vinculado.",
            ));
        }
        let api = self.api.clone();
        let res = with_access_token(&api, self.creds.as_ref(), |token| {
            let api = api.clone();
            async move { api.get_today(&token).await }
        })
        .await;
        match res {
            Ok(response) => {
                let offset_secs = parse_ts(&response.server_time)
                    .map(|server| (server - self.clock.now()).num_seconds());
                let pending_corrections = response.pending_corrections;
                *self.today.write().unwrap_or_else(|p| p.into_inner()) = Some(TodayCache {
                    response,
                    offset_secs,
                });
                let was_offline = !self.online.swap(true, Ordering::SeqCst);
                if was_offline {
                    // Reconnected: drain the queue now (spec §10).
                    self.wake.notify_one();
                }
                if pending_corrections > 0 {
                    self.notice_new_corrections().await;
                }
                self.changed();
                Ok(())
            }
            Err(e) => {
                let err = self.session_error(e);
                self.changed();
                Err(err)
            }
        }
    }

    async fn notice_new_corrections(&self) {
        let api = self.api.clone();
        let res = with_access_token(&api, self.creds.as_ref(), |token| {
            let api = api.clone();
            async move { api.get_my_corrections(&token, Some("PENDING")).await }
        })
        .await;
        let Ok(corrections) = res else {
            return;
        };
        let now = self.clock.now();
        let mut fresh = false;
        {
            let journal = lock(&self.journal);
            for c in corrections
                .iter()
                .filter(|c| c.status == agent_ipc::CorrectionStatus::Pending)
            {
                if journal
                    .notice_once(&format!("correction:{}", c.id), now)
                    .unwrap_or(false)
                {
                    fresh = true;
                }
            }
        }
        if fresh {
            self.notifier.notify(&AttendanceNotice::CorrectionProposed);
        }
    }

    /// `TODAY` view: server data while online, local journal otherwise (`source`).
    pub fn today_view(&self) -> TodayView {
        let now = self.clock.now();
        let today = self.local_naive(now).date();
        let worker_id = self.creds.get_worker_id();
        let rows = lock(&self.journal)
            .all(worker_id.as_deref())
            .unwrap_or_default();
        let local_marks = rows
            .into_iter()
            .filter(|m| self.local_date_of(&m.record.device_ts) == Some(today))
            .map(|m| m.record)
            .collect();
        let cache = self.today.read().unwrap_or_else(|p| p.into_inner());
        let online = self.online.load(Ordering::SeqCst);
        TodayView {
            source: if online && cache.is_some() {
                DataSource::Server
            } else {
                DataSource::Local
            },
            server: cache.as_ref().map(|c| c.response.clone()),
            local_marks,
            server_time_offset_secs: cache.as_ref().and_then(|c| c.offset_secs),
        }
    }

    fn validate_range(&self, from: &str, to: &str) -> Result<(NaiveDate, NaiveDate), OpError> {
        let from = parse_date(from)?;
        let to = parse_date(to)?;
        if from > to {
            return Err(OpError::new(
                "INVALID_INPUT",
                "La fecha inicial debe ser anterior a la final.",
            ));
        }
        let today = self.local_naive(self.clock.now()).date();
        let oldest = today
            .with_year(today.year() - HISTORY_YEARS)
            .unwrap_or(today - Duration::days(365 * HISTORY_YEARS as i64));
        if from < oldest {
            return Err(OpError::new(
                "INVALID_INPUT",
                "Puedes consultar hasta 5 años hacia atrás.",
            ));
        }
        Ok((from, to))
    }

    fn journal_in_range(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        only_unsent: bool,
    ) -> Vec<MarkRecord> {
        let worker_id = self.creds.get_worker_id();
        let rows = lock(&self.journal)
            .all(worker_id.as_deref())
            .unwrap_or_default();
        let mut out: Vec<MarkRecord> = rows
            .into_iter()
            .filter(|m| {
                self.local_date_of(&m.record.device_ts)
                    .is_some_and(|d| d >= from && d <= to)
            })
            .filter(|m| {
                !only_unsent
                    || matches!(
                        m.record.status,
                        LocalMarkStatus::Pending | LocalMarkStatus::Rejected
                    )
            })
            .map(|m| m.record)
            .collect();
        out.reverse(); // newest first, like the server
        out
    }

    /// `MARKS { from, to, page }`: server when reachable, local journal otherwise.
    pub async fn marks(
        &self,
        from: &str,
        to: &str,
        page: Option<u32>,
    ) -> Result<
        (
            DataSource,
            Vec<ServerMark>,
            Vec<MarkRecord>,
            Option<Pagination>,
        ),
        OpError,
    > {
        let (from_d, to_d) = self.validate_range(from, to)?;
        let (from_s, to_s) = (from_d.to_string(), to_d.to_string());
        let page = page.unwrap_or(1).max(1);
        let api = self.api.clone();
        let res = with_access_token(&api, self.creds.as_ref(), |token| {
            let api = api.clone();
            let (from_s, to_s) = (from_s.clone(), to_s.clone());
            async move {
                api.get_my_marks(&token, &from_s, &to_s, page, MARKS_PER_PAGE)
                    .await
            }
        })
        .await;
        match res {
            Ok(collection) => {
                self.online.store(true, Ordering::SeqCst);
                Ok((
                    DataSource::Server,
                    collection.data,
                    self.journal_in_range(from_d, to_d, true),
                    collection.pagination,
                ))
            }
            Err(e)
                if is_transient(&e)
                    || matches!(e, SessionError::NotEnrolled | SessionError::Revoked) =>
            {
                if is_transient(&e) {
                    self.online.store(false, Ordering::SeqCst);
                }
                Ok((
                    DataSource::Local,
                    Vec::new(),
                    self.journal_in_range(from_d, to_d, false),
                    None,
                ))
            }
            Err(e) => Err(self.session_error(e)),
        }
    }

    /// `DAYS { from, to }`: server summaries only (ATT-09), at most 62 days.
    pub async fn days(&self, from: &str, to: &str) -> Result<Vec<DaySummary>, OpError> {
        let (from_d, to_d) = self.validate_range(from, to)?;
        if (to_d - from_d).num_days() >= MAX_DAYS_RANGE {
            return Err(OpError::new(
                "INVALID_INPUT",
                "Consulta como máximo 62 días a la vez.",
            ));
        }
        let (from_s, to_s) = (from_d.to_string(), to_d.to_string());
        let api = self.api.clone();
        with_access_token(&api, self.creds.as_ref(), |token| {
            let api = api.clone();
            let (from_s, to_s) = (from_s.clone(), to_s.clone());
            async move { api.get_my_days(&token, &from_s, &to_s).await }
        })
        .await
        .map_err(|e| self.session_error(e))
    }

    /// `CONSENT_HISTORY` (RF-AGENT-17-1): the worker's id comes from the last `GET /workers/me`.
    pub async fn consent_history(&self) -> Result<Vec<ConsentRecord>, OpError> {
        if !self.linked() {
            return Err(OpError::new(
                "NOT_ENROLLED",
                "Este equipo no está vinculado.",
            ));
        }
        let Some(worker_id) = self.worker_self().map(|w| w.id) else {
            return Err(OpError::new(
                "OFFLINE",
                "Tu historial se consulta con conexión.",
            ));
        };
        let api = self.api.clone();
        with_access_token(&api, self.creds.as_ref(), |token| {
            let api = api.clone();
            let id = worker_id.clone();
            async move { api.get_my_consent_records(&token, &id).await }
        })
        .await
        .map_err(|e| self.session_error(e))
    }

    /// `SCHEDULE` (RF-AGENT-16-1): read-only, never cached; `OFFLINE` without connection.
    pub async fn schedule(&self) -> Result<MyScheduleView, OpError> {
        let api = self.api.clone();
        with_access_token(&api, self.creds.as_ref(), |token| {
            let api = api.clone();
            async move { api.get_my_schedule(&token).await }
        })
        .await
        .map_err(|e| self.session_error(e))
    }

    /// `CORRECTIONS`.
    pub async fn corrections(&self) -> Result<Vec<Correction>, OpError> {
        let api = self.api.clone();
        with_access_token(&api, self.creds.as_ref(), |token| {
            let api = api.clone();
            async move { api.get_my_corrections(&token, None).await }
        })
        .await
        .map_err(|e| self.session_error(e))
    }

    /// `RESPOND_CORRECTION { id, accept }` (ATT-05).
    pub async fn respond_correction(&self, id: &str, accept: bool) -> Result<Correction, OpError> {
        if uuid::Uuid::parse_str(id).is_err() {
            return Err(OpError::new(
                "INVALID_INPUT",
                "Identificador de corrección inválido.",
            ));
        }
        let api = self.api.clone();
        let owned = id.to_string();
        let res = with_access_token(&api, self.creds.as_ref(), |token| {
            let api = api.clone();
            let id = owned.clone();
            async move { api.respond_correction(&token, &id, accept).await }
        })
        .await;
        match res {
            Ok(correction) => {
                tracing::info!(accept, "correction answered");
                let _ = self.refresh_today().await;
                Ok(correction)
            }
            Err(SessionError::Api(ref e)) if e.reason() == Some("CORRECTION_NOT_PENDING") => {
                Err(OpError::new(
                    "CORRECTION_NOT_PENDING",
                    "Esta corrección ya no está pendiente.",
                ))
            }
            Err(e) => Err(self.session_error(e)),
        }
    }

    // ---- notices (RF-AGENT-12-13) -----------------------------------------------------------

    fn agreed_today(&self, date: NaiveDate) -> Option<AgreedTimes> {
        let cached = self
            .today
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .filter(|c| c.response.date == date.to_string())
            .map(|c| c.response.schedule_today.clone());
        if let Some(schedule) = cached {
            return schedule.map(|s| {
                AgreedTimes::from_strings(
                    s.start.as_deref(),
                    s.end.as_deref(),
                    s.break_start.as_deref(),
                    s.break_end.as_deref(),
                )
            });
        }
        let weekday = date.weekday().number_from_monday();
        let me = self.worker_self()?;
        let day = me
            .schedule
            .as_ref()?
            .days
            .iter()
            .find(|d| u32::from(d.weekday) == weekday)?
            .clone();
        Some(AgreedTimes::from_strings(
            Some(&day.start),
            Some(&day.end),
            day.break_start.as_deref(),
            day.break_end.as_deref(),
        ))
    }

    fn mark_types_on(&self, date: NaiveDate) -> Vec<MarkType> {
        let mut types: Vec<MarkType> = self
            .today
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .filter(|c| c.response.date == date.to_string())
            .map(|c| c.response.marks.iter().map(|m| m.r#type).collect())
            .unwrap_or_default();
        let worker_id = self.creds.get_worker_id();
        let rows = lock(&self.journal)
            .all(worker_id.as_deref())
            .unwrap_or_default();
        types.extend(
            rows.into_iter()
                .filter(|m| m.record.status != LocalMarkStatus::Rejected)
                .filter(|m| self.local_date_of(&m.record.device_ts) == Some(date))
                .map(|m| m.record.r#type),
        );
        types
    }

    fn notice_shown(&self, date: NaiveDate, mark_type: MarkType) -> bool {
        lock(&self.journal)
            .notice_seen(&format!("missing:{date}:{mark_type:?}"))
            .unwrap_or(false)
    }

    /// Missing-mark and disconnection notices, each once per day (persisted keys).
    pub async fn check_reminders(&self, now: DateTime<Utc>) {
        if !matches!(self.mode(), AttendanceMode::Ready { .. }) {
            return;
        }
        let local = self.local_naive(now);
        let (date, time) = (local.date(), local.time());

        if let Some(agreed) = self.agreed_today(date) {
            let mut due = reminders::due_missing_marks(&agreed, &self.mark_types_on(date), time);
            if due.iter().any(|(t, _)| !self.notice_shown(date, *t)) {
                // Spec §10: refresh today at the agreed time + 30 min, so a mark made from
                // another channel (portal) is seen before reminding.
                let _ = self.refresh_today().await;
                due = reminders::due_missing_marks(&agreed, &self.mark_types_on(date), time);
            }
            for (mark_type, at) in due {
                let key = format!("missing:{date}:{mark_type:?}");
                if lock(&self.journal).notice_once(&key, now).unwrap_or(false) {
                    self.notifier.notify(&AttendanceNotice::MissingMark {
                        r#type: mark_type,
                        agreed: reminders::hhmm(at),
                    });
                }
            }
        }

        let Some(me) = self.worker_self() else {
            return;
        };
        if !me.is_remote_or_hybrid() {
            return;
        }
        let Some(start) = me
            .effective_disconnection()
            .and_then(|d| reminders::parse_time(&d.start))
        else {
            return;
        };
        if reminders::disconnection_notice_due(start, time) {
            let key = format!("disconnection:{date}");
            if lock(&self.journal).notice_once(&key, now).unwrap_or(false) {
                self.notifier.notify(&AttendanceNotice::DisconnectionSoon {
                    start: reminders::hhmm(start),
                });
            }
        }
    }

    // ---- unlink / storage -------------------------------------------------------------------

    /// Before an explicit unlink: last attempt to send pending marks, then the journal is erased
    /// with the credentials (RF-AGENT-12-10).
    pub async fn prepare_unlink(&self) {
        let report = self.drain_pending().await;
        let (left, _) = self.pending_counts();
        if left > 0 {
            tracing::warn!(left, sent = report.sent, "unlink with unsent marks");
        }
        if let Err(e) = lock(&self.journal).clear() {
            tracing::error!("could not clear the mark journal: {e}");
        }
        *self.today.write().unwrap_or_else(|p| p.into_inner()) = None;
        lock(&self.pin_attempts).record_success();
        *lock(&self.reset_window_until) = None;
        self.changed();
    }

    /// The telemetry queue stored in the same `offline.db` (in memory for tests).
    pub fn open_telemetry_queue(&self) -> Option<OfflineQueue> {
        match &self.db_path {
            Some(path) => OfflineQueue::open(path).ok(),
            None => OfflineQueue::open_in_memory().ok(),
        }
    }

    // ---- background task (architecture.md §4.1 "attendance task") ---------------------------

    /// Mark queue drain with backoff and on reconnect, today refresh every 5 min, pending PIN
    /// events, local notices and journal retention. Never creates a mark (ATT-01).
    pub async fn run(self: Arc<Self>, shutdown: Arc<Notify>) {
        let mut engine_rx = self.engine_state.clone();
        let mut engine_alive = true;
        let mut had_error = engine_rx.borrow().last_error.is_some();
        let mut last_today: Option<DateTime<Utc>> = None;
        let mut last_retention: Option<DateTime<Utc>> = None;
        loop {
            let now = self.clock.now();
            if self.linked()
                && last_today.map_or(true, |t| (now - t).num_seconds() >= TODAY_REFRESH_SECS)
            {
                let _ = self.refresh_today().await;
                // Undelivered PIN events are retried at the same pace (not every tick).
                self.retry_pin_events().await;
                last_today = Some(now);
            }
            if self.retry_due(now) {
                self.drain_pending().await;
            }
            self.check_reminders(now).await;
            if last_retention.map_or(true, |t| (now - t).num_hours() >= 24) {
                let _ = lock(&self.journal).enforce_retention(now);
                last_retention = Some(now);
            }

            let until_retry = lock(&self.drain)
                .next_retry_at
                .and_then(|t| (t - self.clock.now()).to_std().ok())
                .unwrap_or(LOOP_TICK);
            let wait = until_retry.clamp(std::time::Duration::from_secs(1), LOOP_TICK);
            tokio::select! {
                _ = shutdown.notified() => break,
                _ = self.wake.notified() => self.reset_backoff(),
                changed = engine_rx.changed(), if engine_alive => {
                    if changed.is_err() {
                        engine_alive = false;
                        continue;
                    }
                    let (error_now, synced) = {
                        let snap = engine_rx.borrow();
                        (snap.last_error.is_some(), snap.worker_self.is_some())
                    };
                    if had_error && !error_now && synced {
                        // The lifecycle poll reached the server again: reconnect.
                        self.reset_backoff();
                    }
                    had_error = error_now;
                    self.changed();
                }
                _ = tokio::time::sleep(wait) => {}
            }
        }
    }
}

#[cfg(test)]
impl AttendanceService {
    /// Service over an in-memory journal, system clock, silent notifier and no location.
    pub(crate) fn for_tests(
        api_base_url: &str,
        creds: Arc<dyn CredentialStore>,
        engine_state: watch::Receiver<EngineSnapshot>,
    ) -> Arc<Self> {
        let deps = AttendanceDeps {
            creds,
            journal: MarkJournal::open_in_memory().expect("in-memory journal"),
            db_path: None,
            clock: Arc::new(super::SystemClock),
            notifier: Arc::new(super::test_support::RecordingNotifier::default()),
            location: Arc::new(super::UnavailableLocationProvider),
        };
        Self::new(api_base_url, deps, engine_state, None).expect("attendance service")
    }
}
