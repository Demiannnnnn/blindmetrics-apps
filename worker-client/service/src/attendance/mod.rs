//! Attendance module of the service (TASK-AGENT-12): marks with PIN, local journal and offline
//! queue, hash verification, terms, PIN management, corrections and local notices.
//!
//! Rules enforced here: ATT-01 (a mark only comes from the `MARK` op with a valid PIN), ATT-02
//! (marking is never blocked by subscription, telemetry or a later terms version), ATT-03/04
//! (server time, hash check, offline queue idempotent by `client_mark_id`), ATT-07 (no telemetry
//! input), ATT-08 (single best-effort location reading), SEC-19/20 (PIN).

pub mod journal;
pub mod pin_guard;
pub mod reminders;
pub mod service;

pub use journal::MarkJournal;
pub use pin_guard::PinAttempts;
pub use service::AttendanceService;

use agent_ipc::{MarkLocation, MarkType};
use chrono::{DateTime, Utc};

/// Source of "now" for the attendance logic, injectable in tests (no `Utc::now()` in pure logic).
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// Wall clock of the device.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// One-shot location reading for a mark (ATT-08). Blocking; the service runs it on a blocking
/// thread with a 5 s timeout and sends the mark without location on `None`.
pub trait LocationProvider: Send + Sync {
    fn read_once(&self) -> Option<MarkLocation>;
    /// Whether this build can obtain a position at all. Shown to the worker so the window never
    /// claims a location is recorded when it cannot be (RF-AGENT-15-1, PLAT-02).
    fn available(&self) -> bool;
}

/// No OS geolocation backend exists in this build: the reading is unavailable and marks are sent
/// without location (PLAT-02: never a placeholder position).
pub struct UnavailableLocationProvider;

impl LocationProvider for UnavailableLocationProvider {
    fn read_once(&self) -> Option<MarkLocation> {
        None
    }

    fn available(&self) -> bool {
        false
    }
}

/// Local notices of the attendance module (spec §8). Never contain RUN, email, hash or location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttendanceNotice {
    /// 30 min after an agreed mark that was not made (once per mark and day).
    MissingMark { r#type: MarkType, agreed: String },
    /// Marks stored offline were delivered.
    OfflineMarksSent { count: usize },
    /// The employer proposed a correction (ATT-05).
    CorrectionProposed,
    /// 30 min before a remote/hybrid worker's disconnection window.
    DisconnectionSoon { start: String },
}

impl AttendanceNotice {
    pub fn summary(&self) -> &'static str {
        match self {
            AttendanceNotice::MissingMark { .. } => "Marca pendiente",
            AttendanceNotice::OfflineMarksSent { .. } => "Marcas enviadas",
            AttendanceNotice::CorrectionProposed => "Corrección propuesta",
            AttendanceNotice::DisconnectionSoon { .. } => "Desconexión",
        }
    }

    /// Exact copy of `docs/worker-app-spec.md` §8.
    pub fn body(&self) -> String {
        match self {
            AttendanceNotice::MissingMark { r#type, agreed } => {
                let what = match r#type {
                    MarkType::WorkdayStart => "tu entrada",
                    MarkType::WorkdayEnd => "tu salida",
                    MarkType::BreakStart => "el inicio de tu colación",
                    MarkType::BreakEnd => "el término de tu colación",
                };
                format!("No registraste {what} de las {agreed}. Ábrela para marcar.")
            }
            AttendanceNotice::OfflineMarksSent { count } => {
                format!("Se enviaron {count} marcas guardadas sin conexión.")
            }
            AttendanceNotice::CorrectionProposed => {
                "Tu empleador propuso una corrección. Tienes 48 horas para revisarla.".to_string()
            }
            AttendanceNotice::DisconnectionSoon { start } => {
                format!("En 30 minutos comienza tu desconexión ({start}).")
            }
        }
    }
}

/// Delivery of [`AttendanceNotice`]s, injectable in tests.
pub trait Notifier: Send + Sync {
    fn notify(&self, notice: &AttendanceNotice);
}

/// Desktop notifications through the OS notification service.
pub struct DesktopNotifier;

impl Notifier for DesktopNotifier {
    fn notify(&self, notice: &AttendanceNotice) {
        crate::notifications::NotificationDispatcher::notify_attendance(
            notice.summary(),
            &notice.body(),
        );
    }
}

/// Error of an attendance operation, mapped 1:1 to `Response::Error` of the IPC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpError {
    pub code: String,
    pub message: String,
    pub retry_after_secs: Option<u64>,
}

impl OpError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            retry_after_secs: None,
        }
    }

    pub fn with_retry(mut self, secs: Option<u64>) -> Self {
        self.retry_after_secs = secs.filter(|s| *s > 0);
        self
    }
}

impl From<OpError> for agent_ipc::Response {
    fn from(e: OpError) -> Self {
        agent_ipc::Response::Error {
            code: e.code,
            message: e.message,
            retry_after_secs: e.retry_after_secs,
        }
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ` (second precision, the canonical mark format).
pub fn format_ts(ts: DateTime<Utc>) -> String {
    ts.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Parses an RFC 3339 timestamp from the server or the journal.
pub fn parse_ts(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// Test doubles shared by the attendance, engine and IPC tests.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::sync::Mutex;

    /// Settable clock.
    pub struct FixedClock(pub Mutex<DateTime<Utc>>);

    impl FixedClock {
        pub fn at(ts: DateTime<Utc>) -> std::sync::Arc<Self> {
            std::sync::Arc::new(Self(Mutex::new(ts)))
        }

        pub fn set(&self, ts: DateTime<Utc>) {
            *self.0.lock().unwrap() = ts;
        }

        pub fn advance_secs(&self, secs: i64) {
            let mut t = self.0.lock().unwrap();
            *t += chrono::Duration::seconds(secs);
        }
    }

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            *self.0.lock().unwrap()
        }
    }

    /// Records every notice instead of showing it.
    #[derive(Default)]
    pub struct RecordingNotifier(pub Mutex<Vec<AttendanceNotice>>);

    impl RecordingNotifier {
        pub fn taken(&self) -> Vec<AttendanceNotice> {
            std::mem::take(&mut *self.0.lock().unwrap())
        }
    }

    impl Notifier for RecordingNotifier {
        fn notify(&self, notice: &AttendanceNotice) {
            self.0.lock().unwrap().push(notice.clone());
        }
    }

    /// Location provider returning a fixed reading.
    pub struct FixedLocation(pub Option<MarkLocation>);

    impl LocationProvider for FixedLocation {
        fn read_once(&self) -> Option<MarkLocation> {
            self.0.clone()
        }

        fn available(&self) -> bool {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attendance_notice_texts_match_spec() {
        let n = AttendanceNotice::MissingMark {
            r#type: MarkType::WorkdayStart,
            agreed: "09:00".into(),
        };
        assert_eq!(
            n.body(),
            "No registraste tu entrada de las 09:00. Ábrela para marcar."
        );
        assert_eq!(
            AttendanceNotice::OfflineMarksSent { count: 2 }.body(),
            "Se enviaron 2 marcas guardadas sin conexión."
        );
        assert_eq!(
            AttendanceNotice::CorrectionProposed.body(),
            "Tu empleador propuso una corrección. Tienes 48 horas para revisarla."
        );
        assert_eq!(
            AttendanceNotice::DisconnectionSoon {
                start: "20:00".into()
            }
            .body(),
            "En 30 minutos comienza tu desconexión (20:00)."
        );
    }

    #[test]
    fn test_attendance_ts_format_is_second_precision_utc() {
        let ts = parse_ts("2026-10-11T12:00:04.987+00:00").unwrap();
        assert_eq!(format_ts(ts), "2026-10-11T12:00:04Z");
    }
}
#[cfg(test)]
mod service_tests;
