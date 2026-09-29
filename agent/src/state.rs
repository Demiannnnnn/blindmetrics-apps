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

/// Operational state governing telemetry collection and UI view (DP-06, DP-07).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunMode {
    Unenrolled,
    AwaitingConsent { reason: AwaitingReason },
    Collecting(ConsentFlags),
    Paused,
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
}
