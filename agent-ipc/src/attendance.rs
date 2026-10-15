//! Attendance, terms and PIN types shared by the service and the window (TASK-AGENT-12).
//!
//! Server-shaped types (`ServerMark`, `TodayResponse`, `DaySummary`, `Correction`) mirror
//! `api-contract.md` §9 (contract v3/v3.1). They are lenient (unknown fields ignored) because they
//! come from the API and are only forwarded from the service to the window; every request type
//! sent by the window stays strict (`deny_unknown_fields`, `security-model.md` §9.4).

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroizing;

pub use telemetry_crypto_core::{MarkLocation, MarkSource, MarkType};

/// Attendance PIN crossing the IPC boundary (SEC-19).
///
/// Wraps a [`Zeroizing<String>`] so the digits are wiped from memory when dropped. `Debug` never
/// prints the value, so a request that is logged with `{:?}` cannot leak the PIN.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretPin(Zeroizing<String>);

impl SecretPin {
    /// Wraps a PIN typed by the worker.
    pub fn new(pin: impl Into<String>) -> Self {
        Self(Zeroizing::new(pin.into()))
    }

    /// The PIN, still zeroized on drop (SEC-19). Never log or persist it.
    pub fn expose(&self) -> &Zeroizing<String> {
        &self.0
    }
}

impl std::fmt::Debug for SecretPin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretPin(<redacted>)")
    }
}

impl Serialize for SecretPin {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for SecretPin {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Ok(Self(Zeroizing::new(raw)))
    }
}

/// Workplace printed on a mark (`api-contract.md` §9.1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarkWorkplace {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
}

/// A mark as stored and returned by the server (`api-contract.md` §9.1, v3.1).
///
/// Timestamps are kept as the exact strings the server sent: they are part of the hash
/// (`security-model.md` §9.3) and must not be re-formatted before verification.
/// `worker_run` and `employer_rut` are read when the server includes them (they are part of the
/// canonical form); without them the receipt cannot be verified locally.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerMark {
    pub id: String,
    #[serde(default)]
    pub client_mark_id: Option<String>,
    pub worker_id: String,
    #[serde(default)]
    pub worker_run: Option<String>,
    #[serde(default)]
    pub employer_rut: Option<String>,
    #[serde(rename = "type")]
    pub r#type: MarkType,
    pub server_ts: String,
    pub device_ts: String,
    pub offline: bool,
    pub source: MarkSource,
    #[serde(default)]
    pub identification: Option<String>,
    #[serde(default)]
    pub workplace: Option<MarkWorkplace>,
    #[serde(default)]
    pub workplace_id: Option<String>,
    #[serde(default)]
    pub location: Option<MarkLocation>,
    pub hash: String,
    pub prev_hash: String,
    #[serde(default)]
    pub corrected_by: Option<String>,
    #[serde(default)]
    pub superseded_by: Option<String>,
    /// [v3.1] The time that counts (device time for offline/correction marks). Not hashed.
    #[serde(default)]
    pub effective_ts: Option<String>,
}

/// Agreed schedule for today (`GET /attendance/me/today`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduleToday {
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub end: Option<String>,
    #[serde(default)]
    pub break_start: Option<String>,
    #[serde(default)]
    pub break_end: Option<String>,
}

/// Attendance state of the day as computed by the server.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DayState {
    Off,
    Working,
    OnBreak,
    #[serde(other)]
    Unknown,
}

/// `GET /attendance/me/today` (`api-contract.md` §9.2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TodayResponse {
    pub date: String,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub schedule_today: Option<ScheduleToday>,
    pub state: DayState,
    #[serde(default)]
    pub next_mark: Option<MarkType>,
    #[serde(default)]
    pub marks: Vec<ServerMark>,
    #[serde(default)]
    pub pending_corrections: u32,
    pub server_time: String,
}

/// Agreed schedule of a day in a summary (`api-contract.md` §9.5).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgreedDay {
    pub start: String,
    pub end: String,
    #[serde(default, rename = "break")]
    pub break_window: Option<String>,
    #[serde(default)]
    pub break_counts_as_work: bool,
}

/// Server-computed day summary (ATT-09: values are shown verbatim, never recomputed).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaySummary {
    pub date: String,
    #[serde(default)]
    pub agreed: Option<AgreedDay>,
    #[serde(default)]
    pub marks: Vec<ServerMark>,
    #[serde(default)]
    pub attended: bool,
    #[serde(default)]
    pub absence: Option<String>,
    #[serde(default)]
    pub absence_note: Option<String>,
    #[serde(default)]
    pub missing: Option<String>,
    #[serde(default)]
    pub extra: Option<String>,
    #[serde(default)]
    pub anomalies: Vec<String>,
    #[serde(default)]
    pub altered: bool,
}

/// Correction kind proposed by the employer (R38 art. 39).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CorrectionKind {
    Modify,
    Add,
    Fix,
    #[serde(other)]
    Unknown,
}

/// Correction state (ATT-05).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CorrectionStatus {
    Pending,
    Accepted,
    Opposed,
    Consolidated,
    Withdrawn,
    #[serde(other)]
    Unknown,
}

/// Correction proposed by the employer (`api-contract.md` §9.4).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Correction {
    pub id: String,
    #[serde(default)]
    pub worker_id: Option<String>,
    pub kind: CorrectionKind,
    #[serde(default)]
    pub original_mark: Option<ServerMark>,
    #[serde(rename = "type")]
    pub r#type: MarkType,
    pub proposed_ts: String,
    pub reason: String,
    pub status: CorrectionStatus,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub notified_at: Option<String>,
    #[serde(default)]
    pub respond_before: Option<String>,
    #[serde(default)]
    pub responded_at: Option<String>,
    #[serde(default)]
    pub resulting_mark_id: Option<String>,
}

/// Pagination block of a collection response.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pagination {
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
}

/// State of a mark in the device's local journal (`offline.db` table `marks`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocalMarkStatus {
    /// Stored locally, not yet accepted by the server (ATT-04).
    Pending,
    /// Accepted by the server and its hash verified locally (ATT-03).
    Confirmed,
    /// Accepted by the server but the recomputed hash differs: kept as evidence.
    HashMismatch,
    /// `400` validation answer from the server; shown with its reason (ATT-04).
    Rejected,
}

/// A mark of the local journal, with the server data once sent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarkRecord {
    pub client_mark_id: String,
    #[serde(rename = "type")]
    pub r#type: MarkType,
    /// Device clock when the worker confirmed the mark (RFC 3339 UTC, seconds).
    pub device_ts: String,
    /// True once the first send failed (ATT-04).
    pub offline: bool,
    pub status: LocalMarkStatus,
    #[serde(default)]
    pub server_mark: Option<ServerMark>,
    #[serde(default)]
    pub duplicate_of: Option<String>,
    #[serde(default)]
    pub rejection_reason: Option<String>,
}

/// Where a reading came from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DataSource {
    Server,
    /// Device journal (offline; spec §4 "Works offline from the local journal").
    Local,
}

/// Today's attendance view (`TODAY`, snapshot `attendance_today`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TodayView {
    pub source: DataSource,
    /// Server answer of `/attendance/me/today` (last one cached when `source = LOCAL`).
    #[serde(default)]
    pub server: Option<TodayResponse>,
    /// Today's marks of the local journal with their local state.
    #[serde(default)]
    pub local_marks: Vec<MarkRecord>,
    /// `server_time - device clock` in seconds when known (ATT-10).
    #[serde(default)]
    pub server_time_offset_secs: Option<i64>,
}

/// Attendance mode of the worker app (`architecture.md` §4.1).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttendanceModeView {
    #[default]
    Unlinked,
    NeedsTerms,
    NeedsPin,
    Ready,
    Exempt,
}

/// Banners of the window, ordered by priority (spec §1): the first one is shown.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Banner {
    ServiceProblem,
    PendingMarksOffline,
    CorrectionToReview,
    NewTermsVersion,
    TelemetryNotice,
}

/// Terms acceptance state (LEG-05).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LegalStatus {
    #[serde(default)]
    pub accepted_version: Option<String>,
    #[serde(default)]
    pub required_version: Option<String>,
    #[serde(default)]
    pub embedded_terms_version: Option<String>,
    #[serde(default)]
    pub embedded_privacy_version: Option<String>,
}

/// Which embedded legal document.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LegalDocumentKind {
    WorkerTerms,
    Privacy,
}

/// Embedded legal document (LEG-04): full text, version and hash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LegalDocument {
    pub document: LegalDocumentKind,
    pub version: String,
    /// `telemetry_crypto_core::legal::document_hash(text)`.
    pub hash: String,
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_pin_debug_is_redacted_and_serde_roundtrips() {
        let pin = SecretPin::new("482916");
        assert!(!format!("{pin:?}").contains("482916"));
        let json = serde_json::to_string(&pin).unwrap();
        assert_eq!(json, "\"482916\"");
        let back: SecretPin = serde_json::from_str(&json).unwrap();
        assert_eq!(back.expose().as_str(), "482916");
    }

    #[test]
    fn test_mark_request_wire_shape_is_strict() {
        let req: crate::Request = serde_json::from_value(serde_json::json!({
            "op": "MARK", "type": "BREAK_END", "pin": "482916"
        }))
        .unwrap();
        match &req {
            crate::Request::Mark { r#type, pin } => {
                assert_eq!(*r#type, MarkType::BreakEnd);
                assert_eq!(pin.expose().as_str(), "482916");
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(!format!("{req:?}").contains("482916"));
        assert!(serde_json::from_value::<crate::Request>(serde_json::json!({
            "op": "MARK", "type": "BREAK_END", "pin": "482916", "extra": 1
        }))
        .is_err());
        let change: crate::Request = serde_json::from_value(serde_json::json!({
            "op": "CHANGE_PIN", "current": "482916", "new": "975318"
        }))
        .unwrap();
        assert!(matches!(change, crate::Request::ChangePin { .. }));
        let marks: crate::Request = serde_json::from_value(serde_json::json!({
            "op": "MARKS", "from": "2026-10-01", "to": "2026-10-12"
        }))
        .unwrap();
        assert_eq!(
            marks,
            crate::Request::Marks {
                from: "2026-10-01".into(),
                to: "2026-10-12".into(),
                page: None
            }
        );
    }

    #[test]
    fn test_server_mark_parses_contract_shape() {
        let mark: ServerMark = serde_json::from_value(serde_json::json!({
            "id": "01927d3b-65e1-7000-8000-000000000001",
            "client_mark_id": "01927d3b-65e1-7000-8000-0000000000c1",
            "worker_id": "01927d3b-65e1-7000-8000-0000000000aa",
            "type": "WORKDAY_START",
            "server_ts": "2026-10-11T12:00:05Z", "device_ts": "2026-10-11T12:00:04Z",
            "offline": false, "source": "AGENT", "identification": "DEVICE_PIN",
            "workplace": { "id": "01927d3b-65e1-7000-8000-0000000000b1", "name": "Casa matriz",
                           "address": "Av. Apoquindo 1234" },
            "location": null, "hash": "a", "prev_hash": "b",
            "corrected_by": null, "superseded_by": null,
            "effective_ts": "2026-10-11T12:00:05Z"
        }))
        .unwrap();
        assert_eq!(mark.r#type, MarkType::WorkdayStart);
        assert_eq!(mark.workplace.unwrap().name.as_deref(), Some("Casa matriz"));
    }
}
