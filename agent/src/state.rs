//! Worker lifecycle (contract v3) and the two derived modes of the worker app:
//! [`AttendanceMode`] (attendance) and [`RunMode`] (telemetry), `architecture.md` §4.1.
//!
//! The two modes are independent (ATT-07): nothing in [`RunMode`] changes whether the worker can
//! mark, and nothing in attendance changes what telemetry collects.

use serde::{Deserialize, Serialize};
use telemetry_crypto_core::{ConsentDescriptions, ConsentFlags};

/// Link/terms status of the worker (`api-contract.md` §12). Unknown values never stop marking
/// (ATT-02) but stop telemetry (RF-AGENT-12-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkerStatus {
    PendingEnrollment,
    PendingTerms,
    Active,
    Revoked,
    #[serde(other)]
    Unknown,
}

/// Telemetry state of the worker (`api-contract.md` §12, `domain.md` §5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TelemetryStatus {
    NotRequested,
    PendingDecision,
    Accepted,
    Declined,
    Paused,
    /// Missing or unknown value: no telemetry (conservative), attendance unaffected.
    #[default]
    #[serde(other)]
    Unknown,
}

/// Work mode of the worker (`domain.md` §7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkMode {
    OnSite,
    Remote,
    Hybrid,
    #[serde(other)]
    Unknown,
}

impl WorkMode {
    /// Wire name, for the snapshot.
    pub fn as_str(self) -> &'static str {
        match self {
            WorkMode::OnSite => "ON_SITE",
            WorkMode::Remote => "REMOTE",
            WorkMode::Hybrid => "HYBRID",
            WorkMode::Unknown => "UNKNOWN",
        }
    }
}

/// Disconnection window of a remote/hybrid worker (`HH:MM`, may cross midnight).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisconnectionWindow {
    pub start: String,
    pub end: String,
}

/// One weekday of the agreed schedule (1 = Monday … 7 = Sunday).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleDay {
    pub weekday: u8,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub break_start: Option<String>,
    #[serde(default)]
    pub break_end: Option<String>,
}

/// Summary of the agreed schedule in force, as `GET /workers/me` returns it (v3).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleSummary {
    #[serde(default)]
    pub journey_type: Option<String>,
    #[serde(default)]
    pub days: Vec<ScheduleDay>,
    #[serde(default)]
    pub break_counts_as_work: bool,
    #[serde(default)]
    pub disconnection: Option<DisconnectionWindow>,
    #[serde(default)]
    pub location_at_mark: Option<bool>,
    #[serde(default)]
    pub effective_from: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AwaitingReason {
    NoConsent,
    NewVersionPending,
    /// The worker terms were never accepted on this account (LEG-05): no telemetry yet.
    TermsPending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HaltReason {
    KeyFingerprintMismatch,
    /// `status` or `telemetry_status` not understood by this version (RF-AGENT-12-1).
    UnknownServerState,
    /// No tray icon could be created: no invisible monitoring (PROC-02).
    TrayUnavailable,
}

/// Why the employer cannot receive telemetry (BILL-03, TASK-AGENT-09).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BillingBlock {
    SubscriptionSuspended,
    SubscriptionCancelled,
}

impl BillingBlock {
    /// Maps `ingestion_blocked_reason` or an ingest error code; unknown values are treated as
    /// a suspension (the conservative choice: stop collecting, keep the queue).
    pub fn from_reason(reason: &str) -> Self {
        if reason == "SUBSCRIPTION_CANCELLED" {
            BillingBlock::SubscriptionCancelled
        } else {
            BillingBlock::SubscriptionSuspended
        }
    }
}

/// Telemetry run mode (DP-06, DP-07). Affects telemetry only, never attendance (ATT-07).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunMode {
    Unenrolled,
    /// The employer requests no telemetry (`NOT_REQUESTED`).
    NotRequested,
    AwaitingConsent {
        reason: AwaitingReason,
    },
    /// The worker rejected or withdrew telemetry (`DECLINED`).
    Declined,
    Collecting(ConsentFlags),
    Paused,
    /// The employer's subscription is suspended/cancelled: nothing is collected or sent,
    /// the queue is kept, the worker's rights are unaffected (BILL-03).
    EmployerSuspended {
        reason: BillingBlock,
    },
    Halted(HaltReason),
}

impl RunMode {
    /// Wire name used in the IPC snapshot.
    pub fn as_str(&self) -> &'static str {
        match self {
            RunMode::Unenrolled => "UNENROLLED",
            RunMode::NotRequested => "NOT_REQUESTED",
            RunMode::AwaitingConsent { .. } => "AWAITING_CONSENT",
            RunMode::Declined => "DECLINED",
            RunMode::Collecting(_) => "COLLECTING",
            RunMode::Paused => "PAUSED",
            RunMode::EmployerSuspended { .. } => "EMPLOYER_SUSPENDED",
            RunMode::Halted(_) => "HALTED",
        }
    }
}

/// Mirrors `GET /workers/me` (contract v3, `api-contract.md` §6, §10, §11, §12).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerSelf {
    pub id: String,
    pub status: WorkerStatus,
    #[serde(default)]
    pub telemetry_status: TelemetryStatus,
    pub role_type: String, // "OFFICE" | "FIELD"
    #[serde(default)]
    pub work_hours_start: Option<String>,
    #[serde(default)]
    pub work_hours_end: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    pub consent_version: i32,
    pub accepted_consent_version: Option<i32>,
    pub consent_flags: ConsentFlags,
    pub requested_consent_flags: ConsentFlags,
    pub consent_descriptions: ConsentDescriptions,
    pub admin_public_key_fingerprint: String,
    #[serde(default)]
    pub enrollment_completed_at: Option<String>,
    /// `SUBSCRIPTION_SUSPENDED` / `SUBSCRIPTION_CANCELLED` while the employer cannot receive
    /// telemetry (API TASK-17).
    #[serde(default)]
    pub ingestion_blocked_reason: Option<String>,
    #[serde(default)]
    pub terms_accepted_version: Option<String>,
    #[serde(default)]
    pub terms_required_version: Option<String>,
    #[serde(default)]
    pub work_mode: Option<WorkMode>,
    #[serde(default)]
    pub art22_exempt: bool,
    #[serde(default)]
    pub schedule: Option<ScheduleSummary>,
    #[serde(default)]
    pub location_at_mark: bool,
    /// Safety/legal cause declared by the employer for telemetry location (DP-04 v3).
    #[serde(default)]
    pub location_cause: Option<String>,
    #[serde(default)]
    pub disconnection: Option<DisconnectionWindow>,
    /// Worker RUN, only used locally for the PIN policy (SEC-20). Never logged.
    #[serde(default)]
    pub run: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub organization: Option<String>,
}

impl WorkerSelf {
    /// `location_at_mark` from the top level or the schedule in force (ATT-08).
    pub fn effective_location_at_mark(&self) -> bool {
        self.location_at_mark
            || self
                .schedule
                .as_ref()
                .and_then(|s| s.location_at_mark)
                .unwrap_or(false)
    }

    /// Disconnection window from the top level or the schedule in force (DP-12).
    pub fn effective_disconnection(&self) -> Option<&DisconnectionWindow> {
        self.disconnection
            .as_ref()
            .or_else(|| self.schedule.as_ref()?.disconnection.as_ref())
    }

    /// True for `REMOTE`/`HYBRID` workers (and unknown modes, conservatively).
    pub fn is_remote_or_hybrid(&self) -> bool {
        !matches!(self.work_mode, Some(WorkMode::OnSite))
    }
}

pub fn has_any_consent(flags: &ConsentFlags) -> bool {
    flags.system_activity
        || flags.network_activity
        || flags.productivity_basic
        || flags.productivity_screenshots
        || flags.file_activity
        || flags.location
}

/// Pure derivation of the telemetry [`RunMode`] (`architecture.md` §4.1, DP-06, SEC-08,
/// PROC-02, BILL-03, LEG-05). `Collecting` requires `telemetry_status == ACCEPTED`, a local
/// accepted manifest, the current consent version accepted and a visible tray.
pub fn derive_run_mode(
    me: &WorkerSelf,
    pinned_fp: &str,
    accepted_hash: Option<&str>,
    tray_available: bool,
) -> RunMode {
    if matches!(
        me.status,
        WorkerStatus::Revoked | WorkerStatus::PendingEnrollment
    ) {
        return RunMode::Unenrolled;
    }
    if me.admin_public_key_fingerprint != pinned_fp {
        return RunMode::Halted(HaltReason::KeyFingerprintMismatch);
    }
    if me.status == WorkerStatus::Unknown || me.telemetry_status == TelemetryStatus::Unknown {
        return RunMode::Halted(HaltReason::UnknownServerState);
    }
    if me.telemetry_status == TelemetryStatus::NotRequested {
        return RunMode::NotRequested;
    }
    // LEG-05: no telemetry before the worker terms were accepted.
    if me.status == WorkerStatus::PendingTerms || me.terms_accepted_version.is_none() {
        return RunMode::AwaitingConsent {
            reason: AwaitingReason::TermsPending,
        };
    }
    let outdated = me
        .accepted_consent_version
        .map_or(true, |v| v < me.consent_version);
    match me.telemetry_status {
        TelemetryStatus::Declined => RunMode::Declined,
        TelemetryStatus::Paused => RunMode::Paused,
        TelemetryStatus::PendingDecision => RunMode::AwaitingConsent {
            reason: if me.accepted_consent_version.is_some() && outdated {
                AwaitingReason::NewVersionPending
            } else {
                AwaitingReason::NoConsent
            },
        },
        TelemetryStatus::Accepted => {
            if !has_any_consent(&me.consent_flags) || accepted_hash.is_none() {
                return RunMode::AwaitingConsent {
                    reason: AwaitingReason::NoConsent,
                };
            }
            if outdated {
                return RunMode::AwaitingConsent {
                    reason: AwaitingReason::NewVersionPending,
                };
            }
            if !tray_available {
                return RunMode::Halted(HaltReason::TrayUnavailable);
            }
            // BILL-03: an employer without an active subscription receives nothing.
            if let Some(reason) = me.ingestion_blocked_reason.as_deref() {
                return RunMode::EmployerSuspended {
                    reason: BillingBlock::from_reason(reason),
                };
            }
            RunMode::Collecting(me.consent_flags)
        }
        TelemetryStatus::NotRequested | TelemetryStatus::Unknown => {
            RunMode::Halted(HaltReason::UnknownServerState)
        }
    }
}

/// Attendance mode of the worker app (`architecture.md` §4.1). Independent of [`RunMode`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttendanceMode {
    Unlinked,
    /// Linked, worker terms never accepted (first acceptance).
    NeedsTerms,
    /// Terms accepted, no PIN verifier in the keystore.
    NeedsPin,
    /// Can mark. A newer terms version only sets the banner flag (ATT-02).
    Ready {
        terms_update_pending: bool,
    },
    /// Art. 22 exempt worker (ATT-12).
    Exempt,
}

/// Pure derivation of [`AttendanceMode`].
///
/// `local_terms_version` is the version accepted on this device (keystore), used while
/// `/workers/me` is unknown (offline start). Nothing about telemetry is an input (ATT-07).
pub fn derive_attendance_mode(
    linked: bool,
    me: Option<&WorkerSelf>,
    local_terms_version: Option<&str>,
    has_pin: bool,
) -> AttendanceMode {
    if !linked {
        return AttendanceMode::Unlinked;
    }
    if me.is_some_and(|m| m.art22_exempt) {
        return AttendanceMode::Exempt;
    }
    let accepted = me
        .and_then(|m| m.terms_accepted_version.as_deref())
        .or(local_terms_version);
    let Some(accepted) = accepted else {
        return AttendanceMode::NeedsTerms;
    };
    if !has_pin {
        return AttendanceMode::NeedsPin;
    }
    // Versions are ISO dates (`AAAA-MM-DD`): lexicographic order is chronological.
    let terms_update_pending = me
        .and_then(|m| m.terms_required_version.as_deref())
        .is_some_and(|required| required > accepted);
    AttendanceMode::Ready {
        terms_update_pending,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const PINNED_FP: &str =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// An `ACTIVE` worker with telemetry accepted, terms accepted, on site.
    pub(crate) fn worker_v3() -> WorkerSelf {
        let flags = ConsentFlags {
            system_activity: true,
            network_activity: true,
            ..Default::default()
        };
        WorkerSelf {
            id: "worker-1".into(),
            status: WorkerStatus::Active,
            telemetry_status: TelemetryStatus::Accepted,
            role_type: "OFFICE".into(),
            work_hours_start: None,
            work_hours_end: None,
            timezone: None,
            consent_version: 1,
            accepted_consent_version: Some(1),
            consent_flags: flags,
            requested_consent_flags: flags,
            consent_descriptions: ConsentDescriptions::default(),
            admin_public_key_fingerprint: PINNED_FP.into(),
            enrollment_completed_at: None,
            ingestion_blocked_reason: None,
            terms_accepted_version: Some("2026-10-11".into()),
            terms_required_version: Some("2026-10-11".into()),
            work_mode: Some(WorkMode::OnSite),
            art22_exempt: false,
            schedule: None,
            location_at_mark: false,
            location_cause: None,
            disconnection: None,
            run: None,
            name: None,
            organization: None,
        }
    }

    #[test]
    fn test_state_run_mode_derivation_table() {
        let hash = Some("accepted-hash");
        let collecting = RunMode::Collecting(worker_v3().consent_flags);
        let awaiting = |reason| RunMode::AwaitingConsent { reason };

        type Case = (
            &'static str,
            fn(&mut WorkerSelf),
            Option<&'static str>,
            bool,
            RunMode,
        );
        let cases: Vec<Case> = vec![
            ("accepted collects", |_| {}, hash, true, collecting.clone()),
            (
                "no local manifest",
                |_| {},
                None,
                true,
                awaiting(AwaitingReason::NoConsent),
            ),
            (
                "empty flags",
                |m| m.consent_flags = ConsentFlags::default(),
                hash,
                true,
                awaiting(AwaitingReason::NoConsent),
            ),
            (
                "tray missing",
                |_| {},
                hash,
                false,
                RunMode::Halted(HaltReason::TrayUnavailable),
            ),
            (
                "paused",
                |m| m.telemetry_status = TelemetryStatus::Paused,
                hash,
                true,
                RunMode::Paused,
            ),
            (
                "declined",
                |m| m.telemetry_status = TelemetryStatus::Declined,
                hash,
                true,
                RunMode::Declined,
            ),
            (
                "not requested",
                |m| m.telemetry_status = TelemetryStatus::NotRequested,
                hash,
                true,
                RunMode::NotRequested,
            ),
            (
                "pending decision first time",
                |m| {
                    m.telemetry_status = TelemetryStatus::PendingDecision;
                    m.accepted_consent_version = None;
                },
                hash,
                true,
                awaiting(AwaitingReason::NoConsent),
            ),
            (
                "pending decision new version",
                |m| {
                    m.telemetry_status = TelemetryStatus::PendingDecision;
                    m.consent_version = 2;
                },
                hash,
                true,
                awaiting(AwaitingReason::NewVersionPending),
            ),
            (
                "accepted but newer version",
                |m| m.consent_version = 2,
                hash,
                true,
                awaiting(AwaitingReason::NewVersionPending),
            ),
            (
                "pending terms",
                |m| m.status = WorkerStatus::PendingTerms,
                hash,
                true,
                awaiting(AwaitingReason::TermsPending),
            ),
            (
                "terms never accepted",
                |m| m.terms_accepted_version = None,
                hash,
                true,
                awaiting(AwaitingReason::TermsPending),
            ),
            (
                "revoked",
                |m| m.status = WorkerStatus::Revoked,
                hash,
                true,
                RunMode::Unenrolled,
            ),
            (
                "pending enrollment",
                |m| m.status = WorkerStatus::PendingEnrollment,
                hash,
                true,
                RunMode::Unenrolled,
            ),
            (
                "key mismatch",
                |m| m.admin_public_key_fingerprint = "other".into(),
                hash,
                true,
                RunMode::Halted(HaltReason::KeyFingerprintMismatch),
            ),
            (
                "unknown status",
                |m| m.status = WorkerStatus::Unknown,
                hash,
                true,
                RunMode::Halted(HaltReason::UnknownServerState),
            ),
            (
                "unknown telemetry",
                |m| m.telemetry_status = TelemetryStatus::Unknown,
                hash,
                true,
                RunMode::Halted(HaltReason::UnknownServerState),
            ),
            (
                "suspended",
                |m| m.ingestion_blocked_reason = Some("SUBSCRIPTION_SUSPENDED".into()),
                hash,
                true,
                RunMode::EmployerSuspended {
                    reason: BillingBlock::SubscriptionSuspended,
                },
            ),
            (
                "cancelled",
                |m| m.ingestion_blocked_reason = Some("SUBSCRIPTION_CANCELLED".into()),
                hash,
                true,
                RunMode::EmployerSuspended {
                    reason: BillingBlock::SubscriptionCancelled,
                },
            ),
            (
                "billing does not override pause",
                |m| {
                    m.ingestion_blocked_reason = Some("X".into());
                    m.telemetry_status = TelemetryStatus::Paused;
                },
                hash,
                true,
                RunMode::Paused,
            ),
        ];
        for (name, mutate, accepted, tray, expected) in cases {
            let mut me = worker_v3();
            mutate(&mut me);
            assert_eq!(
                derive_run_mode(&me, PINNED_FP, accepted, tray),
                expected,
                "case: {name}"
            );
        }
        assert_eq!(
            BillingBlock::from_reason("WHATEVER"),
            BillingBlock::SubscriptionSuspended
        );
    }

    #[test]
    fn test_state_attendance_mode_derivation_table() {
        let me = worker_v3();
        let ready = AttendanceMode::Ready {
            terms_update_pending: false,
        };
        assert_eq!(
            derive_attendance_mode(false, Some(&me), None, true),
            AttendanceMode::Unlinked
        );
        assert_eq!(derive_attendance_mode(true, Some(&me), None, true), ready);
        assert_eq!(
            derive_attendance_mode(true, Some(&me), None, false),
            AttendanceMode::NeedsPin
        );

        let mut no_terms = me.clone();
        no_terms.terms_accepted_version = None;
        assert_eq!(
            derive_attendance_mode(true, Some(&no_terms), None, true),
            AttendanceMode::NeedsTerms
        );
        // Offline start: the version accepted on this device is enough.
        assert_eq!(
            derive_attendance_mode(true, None, None, true),
            AttendanceMode::NeedsTerms
        );
        assert_eq!(
            derive_attendance_mode(true, None, Some("2026-10-11"), true),
            ready
        );

        let mut newer = me.clone();
        newer.terms_required_version = Some("2027-01-01".into());
        assert_eq!(
            derive_attendance_mode(true, Some(&newer), None, true),
            AttendanceMode::Ready {
                terms_update_pending: true
            }
        );

        let mut exempt = me.clone();
        exempt.art22_exempt = true;
        assert_eq!(
            derive_attendance_mode(true, Some(&exempt), None, true),
            AttendanceMode::Exempt
        );
    }

    #[test]
    fn test_state_attendance_mode_ignores_every_run_mode_input() {
        // ATT-07: telemetry state (paused, declined, halted key, suspended subscription, unknown
        // telemetry) never changes the attendance mode.
        let base = derive_attendance_mode(true, Some(&worker_v3()), None, true);
        let mutations: Vec<fn(&mut WorkerSelf)> = vec![
            |m| m.telemetry_status = TelemetryStatus::Paused,
            |m| m.telemetry_status = TelemetryStatus::Declined,
            |m| m.telemetry_status = TelemetryStatus::Unknown,
            |m| m.admin_public_key_fingerprint = "other".into(),
            |m| m.ingestion_blocked_reason = Some("SUBSCRIPTION_SUSPENDED".into()),
            |m| m.status = WorkerStatus::Unknown,
        ];
        for mutate in mutations {
            let mut me = worker_v3();
            mutate(&mut me);
            assert_eq!(derive_attendance_mode(true, Some(&me), None, true), base);
        }
    }

    #[test]
    fn test_state_worker_self_v3_parses_and_unknown_values_degrade() {
        let mut v = serde_json::to_value(worker_v3()).unwrap();
        v["status"] = "SOMETHING_NEW".into();
        v["telemetry_status"] = "ALSO_NEW".into();
        v["work_mode"] = "REMOTE".into();
        v["schedule"] = serde_json::json!({
            "journey_type": "FIXED",
            "days": [{ "weekday": 1, "start": "09:00", "end": "18:00",
                       "break_start": "13:00", "break_end": "14:00" }],
            "break_counts_as_work": false,
            "disconnection": { "start": "20:00", "end": "08:00" },
            "location_at_mark": true
        });
        let me: WorkerSelf = serde_json::from_value(v).unwrap();
        assert_eq!(me.status, WorkerStatus::Unknown);
        assert_eq!(me.telemetry_status, TelemetryStatus::Unknown);
        assert!(me.effective_location_at_mark());
        assert_eq!(me.effective_disconnection().unwrap().start, "20:00");
        assert!(me.is_remote_or_hybrid());
    }
}
