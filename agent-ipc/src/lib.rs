pub mod attendance;
pub mod client;
pub mod framing;
pub mod transport;

pub use attendance::*;
pub use client::IpcClient;
pub use framing::{read_frame, write_frame, FramingError, MAX_FRAME_SIZE};
pub use futures::{Stream as EventStream, StreamExt};
pub use transport::{
    connect_stream, default_runtime_dir, get_runtime_dir_from_env, read_ipc_token, socket_endpoint,
    token_path, verify_token, write_ipc_token, Listener, ListenerTrait, SocketEndpoint, Stream,
    StreamTrait,
};

use serde::{Deserialize, Serialize};
use telemetry_crypto_core::{ConsentDescriptions, ConsentFlags};

pub const PROTOCOL_VERSION: u16 = 1;

/// Frame sent from the client to the service over the IPC socket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(deny_unknown_fields)]
pub enum ClientFrame {
    Hello {
        protocol_version: u16,
        token: String,
    },
    Call {
        id: u64,
        request: Request,
    },
    Subscribe,
}

/// Frame sent from the service to the client over the IPC socket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(deny_unknown_fields)]
pub enum ServerFrame {
    Welcome {
        service_version: String,
        protocol_version: u16,
    },
    Reply {
        id: u64,
        response: Response,
    },
    Event(Event),
    Rejected {
        code: String,
        message: String,
    },
}

/// Hints from the service tray indicating which window screen to open.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WindowScreenHint {
    Home,
    /// Asistencia (A) — tray *Marcar asistencia…* (RF-AGENT-12-14).
    Attendance,
    Telemetry,
    Withdraw,
}

/// Verification result returned after an enrollment attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(deny_unknown_fields)]
pub enum VerificationResult {
    Verified,
    NeedsSafetyCode { safety_code: String },
}

/// Requests supported by the IPC server (RF-AGENT-10-1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(deny_unknown_fields)]
pub enum Request {
    Snapshot,
    Enroll {
        #[serde(default)]
        qr_payload: Option<String>,
        #[serde(default)]
        code: Option<String>,
    },
    ConfirmSafetyCode {
        matches: bool,
    },
    ConsentDecision {
        #[serde(default)]
        accept: Option<ConsentFlags>,
    },
    Withdraw,
    DataRequest {
        #[serde(default)]
        categories: Option<Vec<String>>,
    },
    DataDeletion {
        #[serde(default)]
        categories: Option<Vec<String>>,
        #[serde(default)]
        reason: Option<String>,
    },
    Unlink {
        confirm: String,
    },
    Diagnostics,
    OpenWindowHint {
        screen: WindowScreenHint,
    },
    /// Unsolicited stop requests are refused (`NOT_ALLOWED`): only the in-process tray may stop
    /// the service (PROC-04, RF-AGENT-10-1). A window can only *answer* a quit request that the tray
    /// initiated, with `ConfirmQuit`.
    Shutdown,
    /// Window answers the tray's *Salir* confirmation dialog. Valid only for the pending quit
    /// request identified by `nonce` (one-shot, expires after 30 s).
    ConfirmQuit {
        nonce: String,
    },
    /// Window dismissed the tray's *Salir* confirmation dialog (*Cancelar*).
    CancelQuit {
        nonce: String,
    },
    /// Embedded legal texts with version and hash (LEG-04) → `Response::LegalDocuments`.
    LegalDocuments,
    /// Accepts the embedded worker terms and privacy policy (LEG-04/05). The values must equal
    /// the embedded ones; the service sends `POST /workers/me/terms-acceptance`.
    /// Errors: `TERMS_MISMATCH` (values differ from the embedded texts), `TERMS_OUTDATED`
    /// (`409 TERMS_VERSION_MISMATCH`: a newer app version is required).
    AcceptTerms {
        version: String,
        hash: String,
        privacy_version: String,
        privacy_hash: String,
    },
    /// Creates the attendance PIN (SEC-19/20). Allowed when no PIN exists or within 10 min of a
    /// successful `VerifyPinReset`. Errors: `PIN_ALREADY_SET`, `PIN_POLICY_*` (cause of the
    /// policy violation: `NON_DIGITS`, `TOO_SHORT`, `TOO_LONG`, `ALL_DIGITS_EQUAL`,
    /// `SEQUENTIAL_DIGITS`, `MATCHES_WORKER_RUN`).
    SetPin {
        pin: SecretPin,
    },
    /// Changes the PIN after checking the current one. Errors: `PIN_INCORRECT` /
    /// `PIN_RETRY_LATER` (with `retry_after_secs`), `PIN_POLICY_*`, `PIN_NOT_SET`.
    ChangePin {
        current: SecretPin,
        new: SecretPin,
    },
    /// Asks the server to email a reset code (`POST /workers/me/pin-reset/request`).
    RequestPinReset,
    /// Verifies the emailed code (`POST /workers/me/pin-reset/verify`); on success the next
    /// `SetPin` within 10 min does not require the current PIN. Errors: `INVALID_RESET_CODE`,
    /// `RATE_LIMITED` (with `retry_after_secs`).
    VerifyPinReset {
        code: String,
    },
    /// Records a mark (ATT-01): PIN check, local journal, `POST /attendance/marks`, queued if
    /// offline. Never refused for subscription, telemetry or a later terms version (ATT-02).
    /// Errors: `TERMS_REQUIRED` (before the first acceptance, no mark created), `PIN_NOT_SET`,
    /// `PIN_INCORRECT` / `PIN_RETRY_LATER` (with `retry_after_secs`), `ATTENDANCE_EXEMPT`,
    /// `NOT_ENROLLED`.
    Mark {
        #[serde(rename = "type")]
        r#type: MarkType,
        pin: SecretPin,
    },
    /// Today's view: server answer when online, local journal otherwise (`source`).
    Today,
    /// Server day summaries `GET /attendance/me/days` (dates `YYYY-MM-DD`, ≤ 62 days).
    /// Error `OFFLINE` without connection.
    Days {
        from: String,
        to: String,
    },
    /// Marks of a range: `GET /attendance/me/marks` online, local journal offline (`source`).
    Marks {
        from: String,
        to: String,
        #[serde(default)]
        page: Option<u32>,
    },
    /// Corrections proposed by the employer (`GET /attendance/me/corrections`).
    Corrections,
    /// Accept (`true`) or oppose (`false`) a pending correction (ATT-05).
    /// Error `CORRECTION_NOT_PENDING` (`409`).
    RespondCorrection {
        id: String,
        accept: bool,
    },
}

/// Responses returned by the IPC server (RF-AGENT-10-1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(deny_unknown_fields)]
pub enum Response {
    Snapshot {
        snapshot: Box<AppSnapshot>,
    },
    Enroll {
        result: VerificationResult,
    },
    ConfirmSafetyCode,
    ConsentDecision,
    Withdraw,
    DataRequest {
        request_id: String,
        status: String,
        #[serde(default)]
        estimated_completion: Option<String>,
    },
    DataDeletion {
        request_id: String,
        status: String,
        #[serde(default)]
        message: Option<String>,
    },
    Unlink,
    Diagnostics {
        diagnostics: Box<DiagnosticsInfo>,
    },
    OpenWindowHint,
    Shutdown,
    ConfirmQuit,
    CancelQuit,
    LegalDocuments {
        documents: Vec<LegalDocument>,
    },
    AcceptTerms {
        terms_accepted_version: String,
    },
    SetPin,
    ChangePin,
    RequestPinReset,
    VerifyPinReset {
        /// Seconds during which `SetPin` does not require the current PIN.
        set_pin_window_secs: u64,
    },
    Mark {
        mark: Box<MarkRecord>,
    },
    Today {
        today: Box<TodayView>,
    },
    Days {
        days: Vec<DaySummary>,
    },
    Marks {
        source: DataSource,
        /// Server marks (newest first) when `source = SERVER`.
        #[serde(default)]
        marks: Vec<ServerMark>,
        /// Journal marks of the range: all of them when `source = LOCAL`, only the ones not yet
        /// confirmed by the server when `source = SERVER`.
        #[serde(default)]
        local: Vec<MarkRecord>,
        #[serde(default)]
        pagination: Option<Pagination>,
    },
    Corrections {
        corrections: Vec<Correction>,
    },
    RespondCorrection {
        correction: Box<Correction>,
    },
    Error {
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        retry_after_secs: Option<u64>,
    },
}

impl Response {
    pub fn is_ok(&self) -> bool {
        !matches!(self, Response::Error { .. })
    }

    pub fn is_err(&self) -> bool {
        matches!(self, Response::Error { .. })
    }
}

/// Events pushed asynchronously to subscribed clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(deny_unknown_fields)]
pub enum Event {
    Snapshot(Box<AppSnapshot>),
    /// The tray's *Salir* asks the window to show the confirmation dialog (PROC-04). `nonce`
    /// must be echoed in `Request::ConfirmQuit` within `expires_in_secs`.
    QuitRequested {
        nonce: String,
        expires_in_secs: u64,
    },
    /// The pending quit request was cancelled or expired; the dialog must close.
    QuitCancelled,
}

/// Category state representation in the observable snapshot.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CategoryState {
    Active,
    NotAccepted,
    NotRequested,
    Sleeping,
    Unavailable,
}

/// State of all six telemetry categories.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CategoryStates {
    pub system_activity: CategoryState,
    pub network_activity: CategoryState,
    pub productivity_basic: CategoryState,
    pub productivity_screenshots: CategoryState,
    pub file_activity: CategoryState,
    pub location: CategoryState,
}

/// Consent offer snapshot shown on C1 (contains no manifest hash or secrets).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConsentOfferSnapshot {
    pub organization: Option<String>,
    pub consent_version: i32,
    pub requested_flags: ConsentFlags,
    pub descriptions: ConsentDescriptions,
    pub is_update: bool,
    #[serde(default)]
    pub previous_selection: Option<ConsentFlags>,
}

/// Observable state of the worker application (RF-AGENT-10-7).
/// SEC-18 / PROC-03: Holds NO tokens, NO keys, NO PINs, and NO manifest hashes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AppSnapshot {
    pub service_version: String,
    pub run_mode: String,
    pub consent_offer: Option<ConsentOfferSnapshot>,
    pub accepted_selection: Option<ConsentFlags>,
    pub category_states: CategoryStates,
    pub work_hours_start: Option<String>,
    pub work_hours_end: Option<String>,
    pub timezone: Option<String>,
    pub worker_name: Option<String>,
    pub organization: Option<String>,
    pub queue_size: usize,
    pub last_flush: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
    pub suggested_screen: Option<WindowScreenHint>,
    pub tray_available: bool,
    pub pending_withdrawal: bool,
    pub device_identifier_short: String,
    pub key_fingerprint: Option<String>,
    pub is_enrolled: bool,
    /// Attendance mode (`architecture.md` §4.1), independent of `run_mode` (ATT-07).
    #[serde(default)]
    pub attendance_mode: AttendanceModeView,
    /// A newer worker terms version must be read; marking keeps working (ATT-02, LEG-05).
    #[serde(default)]
    pub terms_update_pending: bool,
    #[serde(default)]
    pub attendance_today: Option<TodayView>,
    /// Marks of the local journal still waiting to be sent (ATT-04).
    #[serde(default)]
    pub pending_marks: usize,
    #[serde(default)]
    pub pending_corrections: u32,
    /// Ordered by priority (spec §1); the window shows the first one.
    #[serde(default)]
    pub banners: Vec<Banner>,
    #[serde(default)]
    pub legal: LegalStatus,
    /// `ON_SITE` | `REMOTE` | `HYBRID` when known.
    #[serde(default)]
    pub work_mode: Option<String>,
    /// The worker's schedule asks for a location reading when marking (ATT-08).
    #[serde(default)]
    pub location_at_mark: bool,
}

/// Diagnostics report returned by the service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticsInfo {
    pub service_version: String,
    pub uptime_seconds: u64,
    pub api_base_url: String,
    pub server_time_offset_seconds: Option<i64>,
    pub pending_marks: usize,
    pub telemetry_queue_items: usize,
    pub telemetry_queue_bytes: u64,
    pub last_successful_send: Option<chrono::DateTime<chrono::Utc>>,
    pub device_id_short: String,
    pub key_fingerprint: Option<String>,
    pub tray_available: bool,
    pub os_permissions: String,
}

/// Typed errors for IPC communication.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum IpcError {
    #[error("Service is not running: {0}")]
    ServiceNotRunning(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Protocol mismatch: {0}")]
    ProtocolMismatch(String),
    #[error("Request timed out: {0}")]
    Timeout(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("Server rejected request: [{code}] {message}")]
    Rejected { code: String, message: String },
    #[error("IPC call error: [{code}] {message}")]
    CallError {
        code: String,
        message: String,
        retry_after_secs: Option<u64>,
    },
    #[error("Framing error: {0}")]
    Framing(String),
}

impl From<FramingError> for IpcError {
    fn from(err: FramingError) -> Self {
        match err {
            FramingError::Io(e) => IpcError::Io(e.to_string()),
            other => IpcError::Framing(other.to_string()),
        }
    }
}

impl From<std::io::Error> for IpcError {
    fn from(err: std::io::Error) -> Self {
        IpcError::Io(err.to_string())
    }
}
