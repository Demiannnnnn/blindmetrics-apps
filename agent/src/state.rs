use serde::{Deserialize, Serialize};
use telemetry_crypto_core::{ConsentDescriptions, ConsentFlags};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkerStatus {
    PendingEnrollment,
    PendingConsent,
    Active,
    Paused,
    Revoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AwaitingReason {
    NoConsent,
    NewVersionPending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HaltReason {
    KeyFingerprintMismatch,
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

/// Operational state governing telemetry collection and UI view (DP-06, DP-07).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunMode {
    Unenrolled,
    AwaitingConsent {
        reason: AwaitingReason,
    },
    Collecting(ConsentFlags),
    Paused,
    /// The employer's subscription is suspended/cancelled: nothing is collected or sent,
    /// the queue is kept, the worker's rights are unaffected (BILL-03).
    EmployerSuspended {
        reason: BillingBlock,
    },
    Halted(HaltReason),
}

/// Mirrors the /workers/me server model (api-contract.md §5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerSelf {
    pub id: String,
    pub status: WorkerStatus,
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
}

pub fn has_any_consent(flags: &ConsentFlags) -> bool {
    flags.system_activity
        || flags.network_activity
        || flags.productivity_basic
        || flags.productivity_screenshots
        || flags.file_activity
        || flags.location
}

/// Pure state machine function deriving RunMode from server state, pinned fingerprint, and local accepted commitment hash.
pub fn derive_run_mode(me: &WorkerSelf, pinned_fp: &str, accepted_hash: Option<&str>) -> RunMode {
    if me.admin_public_key_fingerprint != pinned_fp {
        return RunMode::Halted(HaltReason::KeyFingerprintMismatch);
    }

    match me.status {
        WorkerStatus::PendingConsent => RunMode::AwaitingConsent {
            reason: AwaitingReason::NoConsent,
        },
        WorkerStatus::Paused => RunMode::Paused,
        WorkerStatus::Active => {
            if has_any_consent(&me.consent_flags) && accepted_hash.is_some() {
                // BILL-03: an employer without an active subscription receives nothing.
                if let Some(reason) = me.ingestion_blocked_reason.as_deref() {
                    return RunMode::EmployerSuspended {
                        reason: BillingBlock::from_reason(reason),
                    };
                }
                RunMode::Collecting(me.consent_flags)
            } else {
                RunMode::AwaitingConsent {
                    reason: AwaitingReason::NoConsent,
                }
            }
        }
        WorkerStatus::Revoked | WorkerStatus::PendingEnrollment => RunMode::Unenrolled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_mode_derivation_table() {
        let pinned_fp = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let accepted_hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

        let active_flags = ConsentFlags {
            system_activity: true,
            network_activity: true,
            ..Default::default()
        };

        let mut me = WorkerSelf {
            id: "worker-1".into(),
            status: WorkerStatus::Active,
            role_type: "OFFICE".into(),
            work_hours_start: None,
            work_hours_end: None,
            timezone: None,
            consent_version: 1,
            accepted_consent_version: Some(1),
            consent_flags: active_flags,
            requested_consent_flags: active_flags,
            consent_descriptions: ConsentDescriptions::default(),
            admin_public_key_fingerprint: pinned_fp.into(),
            enrollment_completed_at: None,
            ingestion_blocked_reason: None,
        };

        // 1. ACTIVE + consent flags + accepted hash => Collecting
        assert_eq!(
            derive_run_mode(&me, pinned_fp, Some(accepted_hash)),
            RunMode::Collecting(active_flags)
        );

        // 2. ACTIVE + no accepted hash => AwaitingConsent
        assert_eq!(
            derive_run_mode(&me, pinned_fp, None),
            RunMode::AwaitingConsent {
                reason: AwaitingReason::NoConsent
            }
        );

        // 3. ACTIVE + all false flags => AwaitingConsent
        me.consent_flags = ConsentFlags::default();
        assert_eq!(
            derive_run_mode(&me, pinned_fp, Some(accepted_hash)),
            RunMode::AwaitingConsent {
                reason: AwaitingReason::NoConsent
            }
        );
        me.consent_flags = active_flags;

        // 4. PAUSED => Paused
        me.status = WorkerStatus::Paused;
        assert_eq!(
            derive_run_mode(&me, pinned_fp, Some(accepted_hash)),
            RunMode::Paused
        );

        // 5. PENDING_CONSENT => AwaitingConsent
        me.status = WorkerStatus::PendingConsent;
        assert_eq!(
            derive_run_mode(&me, pinned_fp, Some(accepted_hash)),
            RunMode::AwaitingConsent {
                reason: AwaitingReason::NoConsent
            }
        );

        // 6. REVOKED => Unenrolled
        me.status = WorkerStatus::Revoked;
        assert_eq!(
            derive_run_mode(&me, pinned_fp, Some(accepted_hash)),
            RunMode::Unenrolled
        );

        // 7. PENDING_ENROLLMENT => Unenrolled
        me.status = WorkerStatus::PendingEnrollment;
        assert_eq!(
            derive_run_mode(&me, pinned_fp, Some(accepted_hash)),
            RunMode::Unenrolled
        );

        // 8. Fingerprint mismatch => Halted
        me.status = WorkerStatus::Active;
        me.admin_public_key_fingerprint = "different_fingerprint".into();
        assert_eq!(
            derive_run_mode(&me, pinned_fp, Some(accepted_hash)),
            RunMode::Halted(HaltReason::KeyFingerprintMismatch)
        );
    }

    #[test]
    fn test_run_mode_employer_suspended_only_when_collecting() {
        let pinned_fp = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let flags = ConsentFlags {
            system_activity: true,
            ..Default::default()
        };
        let mut me = WorkerSelf {
            id: "worker-1".into(),
            status: WorkerStatus::Active,
            role_type: "OFFICE".into(),
            work_hours_start: None,
            work_hours_end: None,
            timezone: None,
            consent_version: 1,
            accepted_consent_version: Some(1),
            consent_flags: flags,
            requested_consent_flags: flags,
            consent_descriptions: ConsentDescriptions::default(),
            admin_public_key_fingerprint: pinned_fp.into(),
            enrollment_completed_at: None,
            ingestion_blocked_reason: Some("SUBSCRIPTION_SUSPENDED".into()),
        };
        assert_eq!(
            derive_run_mode(&me, pinned_fp, Some("h")),
            RunMode::EmployerSuspended {
                reason: BillingBlock::SubscriptionSuspended
            }
        );
        me.ingestion_blocked_reason = Some("SUBSCRIPTION_CANCELLED".into());
        assert_eq!(
            derive_run_mode(&me, pinned_fp, Some("h")),
            RunMode::EmployerSuspended {
                reason: BillingBlock::SubscriptionCancelled
            }
        );
        // Status and key integrity take precedence over billing.
        me.status = WorkerStatus::Paused;
        assert_eq!(derive_run_mode(&me, pinned_fp, Some("h")), RunMode::Paused);
        me.status = WorkerStatus::PendingConsent;
        assert!(matches!(
            derive_run_mode(&me, pinned_fp, Some("h")),
            RunMode::AwaitingConsent { .. }
        ));
        me.status = WorkerStatus::Active;
        assert_eq!(
            derive_run_mode(&me, "other", Some("h")),
            RunMode::Halted(HaltReason::KeyFingerprintMismatch)
        );
        // Unknown reasons are handled conservatively.
        assert_eq!(
            BillingBlock::from_reason("WHATEVER"),
            BillingBlock::SubscriptionSuspended
        );
    }
}
