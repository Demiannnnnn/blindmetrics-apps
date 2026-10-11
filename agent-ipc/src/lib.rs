pub mod client;
pub mod framing;
pub mod transport;

pub use client::IpcClient;
pub use framing::{read_frame, write_frame, FramingError, MAX_FRAME_SIZE};
pub use futures::{Stream as EventStream, StreamExt};
pub use transport::{
    default_runtime_dir, get_runtime_dir_from_env, read_ipc_token, socket_endpoint, token_path,
    verify_token, write_ipc_token, Listener, ListenerTrait, SocketEndpoint, Stream, StreamTrait,
};

use serde::{Deserialize, Serialize};
use telemetry_crypto_core::{ConsentDescriptions, ConsentFlags};

pub const PROTOCOL_VERSION: u16 = 1;

/// Frame sent from the client to the service over the IPC socket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
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
    Shutdown,
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
