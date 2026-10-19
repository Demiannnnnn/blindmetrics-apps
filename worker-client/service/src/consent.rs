//! Consent package shown on C1 and the decisions sent to the server (TASK-AGENT-09).
//!
//! Granular consent (DP-11): the admin requests categories, the worker accepts any non-empty
//! subset, can change it later, reject everything or withdraw. Every checkbox starts unchecked
//! (DP-08). Every decision carries the commitment of the **full** manifest that was displayed
//! (LEG-02), the same commitment the admin put in the enrollment QR.

use crate::api::client::{ApiClient, ConsentDecisionResponse, ConsentRequest};
use crate::credentials::CredentialStore;
use crate::enrollment::PendingEnrollment;
use crate::errors::AgentError;
use crate::session::{with_access_token, SessionError};
use crate::state::{has_any_consent, WorkerSelf};
use telemetry_crypto_core::{
    manifest_commitment, ConsentDescriptions, ConsentFlags, CryptoError, EnrollmentManifest,
};

/// What the worker decided on C1 or with "Retirar consentimiento".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsentAction {
    /// Accept exactly these categories (non-empty subset of the requested ones).
    Accept(ConsentFlags),
    /// Reject everything or withdraw: telemetry becomes `DECLINED` (contract v3, `status` unchanged).
    Decline,
}

impl ConsentAction {
    pub fn is_accept(&self) -> bool {
        matches!(self, ConsentAction::Accept(_))
    }
}

/// Categories set in both `a` and `b`.
pub fn intersect(a: &ConsentFlags, b: &ConsentFlags) -> ConsentFlags {
    ConsentFlags {
        system_activity: a.system_activity && b.system_activity,
        network_activity: a.network_activity && b.network_activity,
        productivity_basic: a.productivity_basic && b.productivity_basic,
        productivity_screenshots: a.productivity_screenshots && b.productivity_screenshots,
        file_activity: a.file_activity && b.file_activity,
        location: a.location && b.location,
    }
}

/// Number of categories set.
pub fn count_categories(flags: &ConsentFlags) -> usize {
    [
        flags.system_activity,
        flags.network_activity,
        flags.productivity_basic,
        flags.productivity_screenshots,
        flags.file_activity,
        flags.location,
    ]
    .iter()
    .filter(|f| **f)
    .count()
}

/// What the worker is asked to decide on C1.
#[derive(Clone, Debug, PartialEq)]
pub struct ConsentOffer {
    pub organization: Option<String>,
    pub consent_version: i32,
    /// Requested categories: the only ones the worker can choose.
    pub flags: ConsentFlags,
    pub descriptions: ConsentDescriptions,
    pub work_hours_start: Option<String>,
    pub work_hours_end: Option<String>,
    pub timezone: Option<String>,
    /// Commitment of the displayed manifest, sent as `manifest_hash`.
    pub manifest_hash: String,
    /// Selection accepted before on this device, if any ("Mantener mi selección anterior").
    pub previous_selection: Option<ConsentFlags>,
}

impl ConsentOffer {
    /// Offer right after enrollment: the manifest was already verified (E2).
    pub fn from_enrollment(pending: &PendingEnrollment) -> Self {
        let r = &pending.resp;
        Self {
            organization: Some(r.organization.clone()),
            consent_version: r.consent_version,
            flags: r.consent_flags,
            descriptions: r.consent_descriptions.clone(),
            work_hours_start: r.work_hours_start.clone(),
            work_hours_end: r.work_hours_end.clone(),
            timezone: r.timezone.clone(),
            manifest_hash: pending.commitment.clone(),
            previous_selection: None,
        }
    }

    /// Offer for a later version (or after restart), rebuilt from `/workers/me` and the key
    /// pinned at enrollment — never a key sent by the server (SEC-08).
    pub fn from_worker_self(
        me: &WorkerSelf,
        pinned_admin_pub: &str,
        previous_selection: Option<ConsentFlags>,
    ) -> Result<Self, CryptoError> {
        let manifest = EnrollmentManifest::new(
            pinned_admin_pub.to_string(),
            me.consent_version,
            me.requested_consent_flags,
            me.consent_descriptions.clone(),
        )?;
        Ok(Self {
            organization: None,
            consent_version: me.consent_version,
            flags: me.requested_consent_flags,
            descriptions: me.consent_descriptions.clone(),
            work_hours_start: me.work_hours_start.clone(),
            work_hours_end: me.work_hours_end.clone(),
            timezone: me.timezone.clone(),
            manifest_hash: manifest_commitment(&manifest)?,
            previous_selection: previous_selection.filter(has_any_consent),
        })
    }

    /// The previous selection restricted to what is still requested (explicit worker action,
    /// DP-08: never applied by default).
    pub fn keep_previous_selection(&self) -> ConsentFlags {
        self.previous_selection
            .map(|prev| intersect(&prev, &self.flags))
            .unwrap_or_default()
    }
}

/// `PATCH /workers/me/consent` with automatic token refresh. `Accept` sends the flags as
/// given: callers restrict them to the requested categories first.
pub async fn send_consent(
    api: &ApiClient,
    creds: &dyn CredentialStore,
    consent_version: i32,
    manifest_hash: &str,
    action: ConsentAction,
) -> Result<ConsentDecisionResponse, SessionError> {
    if creds.get_worker_id().is_none() {
        return Err(SessionError::NotEnrolled);
    }
    let consent_flags = match action {
        ConsentAction::Accept(flags) => Some(flags),
        ConsentAction::Decline => None,
    };
    with_access_token(api, creds, |token| {
        let api = api.clone();
        let manifest_hash = manifest_hash.to_string();
        async move {
            let body = ConsentRequest {
                accepted: consent_flags.is_some(),
                consent_version,
                manifest_hash: &manifest_hash,
                consent_flags,
            };
            api.patch_my_consent(&token, &body).await
        }
    })
    .await
}

/// Sends the decision for `offer` and updates the locally accepted selection only after the
/// server recorded it (RF-AGENT-09-3).
pub async fn submit_decision(
    api: &ApiClient,
    creds: &dyn CredentialStore,
    offer: &ConsentOffer,
    action: ConsentAction,
) -> Result<ConsentDecisionResponse, SessionError> {
    let action = match action {
        ConsentAction::Accept(selection) => {
            // DP-11: never send a category the admin did not request.
            let selection = intersect(&selection, &offer.flags);
            if !has_any_consent(&selection) {
                return Err(SessionError::Invalid(
                    "Selecciona al menos una categoría o usa Rechazar todo.",
                ));
            }
            ConsentAction::Accept(selection)
        }
        ConsentAction::Decline => ConsentAction::Decline,
    };
    let resp = send_consent(
        api,
        creds,
        offer.consent_version,
        &offer.manifest_hash,
        action,
    )
    .await?;
    match action {
        ConsentAction::Accept(selection) => {
            creds
                .set_accepted_manifest_hash(Some(&offer.manifest_hash))
                .map_err(SessionError::Store)?;
            creds
                .set_accepted_flags(Some(&intersect(&selection, &resp.consent_flags)))
                .map_err(SessionError::Store)?;
        }
        ConsentAction::Decline => {
            creds
                .set_accepted_manifest_hash(None)
                .map_err(SessionError::Store)?;
            creds
                .set_accepted_flags(None)
                .map_err(SessionError::Store)?;
        }
    }
    tracing::info!(
        accepted = action.is_accept(),
        categories = count_categories(&resp.consent_flags),
        consent_version = offer.consent_version,
        "consent decision recorded"
    );
    Ok(resp)
}

/// `details.current_consent_version` of a `409 CONSENT_VERSION_MISMATCH`.
pub fn current_version_from_conflict(err: &SessionError) -> Option<i32> {
    match err {
        SessionError::Api(AgentError::Api {
            status: 409,
            reason,
            details,
            ..
        }) if reason.as_deref() == Some("CONSENT_VERSION_MISMATCH") => details
            .as_ref()
            .and_then(|d| d.get("current_consent_version"))
            .and_then(|v| v.as_i64())
            .and_then(|v| i32::try_from(v).ok()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::InMemoryCredentialStore;
    use crate::state::WorkerStatus;
    use telemetry_crypto_core::{generate_admin_keypair, verify_commitment};
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn worker_self(version: i32) -> WorkerSelf {
        WorkerSelf {
            id: "w-1".into(),
            status: WorkerStatus::Active,
            telemetry_status: crate::state::TelemetryStatus::PendingDecision,
            role_type: "OFFICE".into(),
            work_hours_start: Some("09:00:00".into()),
            work_hours_end: Some("18:00:00".into()),
            timezone: Some("America/Santiago".into()),
            consent_version: version,
            accepted_consent_version: Some(1),
            consent_flags: ConsentFlags::default(),
            requested_consent_flags: ConsentFlags {
                system_activity: true,
                network_activity: true,
                file_activity: true,
                ..Default::default()
            },
            consent_descriptions: ConsentDescriptions {
                system_activity: Some("Aplicaciones en uso".into()),
                network_activity: Some("Dominios visitados".into()),
                file_activity: Some("Archivos de la carpeta de trabajo".into()),
                ..Default::default()
            },
            admin_public_key_fingerprint: "fp".into(),
            enrollment_completed_at: None,
            ingestion_blocked_reason: None,
            ..crate::state::tests::worker_v3()
        }
    }

    fn enrolled_store() -> InMemoryCredentialStore {
        let creds = InMemoryCredentialStore::new();
        creds.set_worker_id(Some("w-1")).unwrap();
        creds.set_access_token(Some("tok")).unwrap();
        creds.set_refresh_token(Some("ref")).unwrap();
        creds
    }

    fn decision_response(flags: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "status": "ACTIVE", "consent_version": 3, "accepted_consent_version": 3,
                "consent_flags": flags,
                "consent_accepted_at": "2026-09-30T10:00:00Z",
                "consent_rejected_at": null,
                "enrollment_completed_at": "2026-09-30T10:00:00Z"
            }
        })
    }

    fn offer(version: i32) -> ConsentOffer {
        ConsentOffer::from_worker_self(&worker_self(version), &generate_admin_keypair().1, None)
            .unwrap()
    }

    #[test]
    fn test_consent_offer_from_worker_self_matches_admin_manifest() {
        let (_priv, admin_pub) = generate_admin_keypair();
        let me = worker_self(2);
        let offer = ConsentOffer::from_worker_self(&me, &admin_pub, None).unwrap();
        // Same manifest the admin commits to: requested flags + descriptions + version.
        let manifest = EnrollmentManifest::new(
            admin_pub,
            2,
            me.requested_consent_flags,
            me.consent_descriptions.clone(),
        )
        .unwrap();
        assert!(verify_commitment(&manifest, &offer.manifest_hash));
        assert_eq!(offer.consent_version, 2);
        assert!(offer.flags.system_activity);
        assert!(offer.previous_selection.is_none());
    }

    #[tokio::test]
    async fn test_consent_accept_subset_sends_granular_body_and_persists_intersection() {
        let server = MockServer::start().await;
        let offer = offer(3);
        Mock::given(method("PATCH"))
            .and(path("/workers/me/consent"))
            .and(body_json(serde_json::json!({
                "accepted": true,
                "consent_version": 3,
                "manifest_hash": offer.manifest_hash,
                "consent_flags": {
                    "system_activity": true, "network_activity": false,
                    "productivity_basic": false, "productivity_screenshots": false,
                    "file_activity": true, "location": false
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(decision_response(
                serde_json::json!({
                    "system_activity": true, "network_activity": false, "productivity_basic": false,
                    "productivity_screenshots": false, "file_activity": true, "location": false
                }),
            )))
            .expect(1)
            .mount(&server)
            .await;

        let api = ApiClient::new(server.uri()).unwrap();
        let creds = enrolled_store();
        let selection = ConsentFlags {
            system_activity: true,
            file_activity: true,
            ..Default::default()
        };
        submit_decision(&api, &creds, &offer, ConsentAction::Accept(selection))
            .await
            .unwrap();
        assert_eq!(
            creds.get_accepted_manifest_hash().as_deref(),
            Some(offer.manifest_hash.as_str())
        );
        assert_eq!(creds.get_accepted_flags().unwrap(), selection);
    }

    #[tokio::test]
    async fn test_consent_selection_never_includes_unrequested_categories() {
        let server = MockServer::start().await;
        let offer = offer(3);
        // Location and screenshots were not requested: they are stripped before sending.
        Mock::given(method("PATCH"))
            .and(path("/workers/me/consent"))
            .and(body_json(serde_json::json!({
                "accepted": true,
                "consent_version": 3,
                "manifest_hash": offer.manifest_hash,
                "consent_flags": {
                    "system_activity": true, "network_activity": false,
                    "productivity_basic": false, "productivity_screenshots": false,
                    "file_activity": false, "location": false
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(decision_response(
                serde_json::json!({
                    "system_activity": true, "network_activity": false, "productivity_basic": false,
                    "productivity_screenshots": false, "file_activity": false, "location": false
                }),
            )))
            .expect(1)
            .mount(&server)
            .await;
        let api = ApiClient::new(server.uri()).unwrap();
        let creds = enrolled_store();
        let selection = ConsentFlags {
            system_activity: true,
            location: true,
            productivity_screenshots: true,
            ..Default::default()
        };
        submit_decision(&api, &creds, &offer, ConsentAction::Accept(selection))
            .await
            .unwrap();
        let stored = creds.get_accepted_flags().unwrap();
        assert!(stored.system_activity && !stored.location && !stored.productivity_screenshots);
    }

    #[tokio::test]
    async fn test_consent_accept_empty_selection_rejected_locally() {
        let api = ApiClient::new("http://127.0.0.1:9".into()).unwrap();
        let creds = enrolled_store();
        let unrequested_only = ConsentFlags {
            location: true,
            ..Default::default()
        };
        for selection in [ConsentFlags::default(), unrequested_only] {
            let err = submit_decision(&api, &creds, &offer(3), ConsentAction::Accept(selection))
                .await
                .unwrap_err();
            assert!(matches!(err, SessionError::Invalid(_)), "{err:?}");
        }
        assert!(creds.get_accepted_flags().is_none());
    }

    #[tokio::test]
    async fn test_consent_decline_sends_accepted_false_without_flags() {
        let server = MockServer::start().await;
        let offer = offer(3);
        Mock::given(method("PATCH"))
            .and(path("/workers/me/consent"))
            .and(body_json(serde_json::json!({
                "accepted": false, "consent_version": 3, "manifest_hash": offer.manifest_hash
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "status": "ACTIVE", "consent_version": 3, "accepted_consent_version": null,
                    "consent_flags": ConsentFlags::default(), "consent_accepted_at": null,
                    "consent_rejected_at": "2026-09-30T10:00:00Z"
                }
            })))
            .expect(1)
            .mount(&server)
            .await;
        let api = ApiClient::new(server.uri()).unwrap();
        let creds = enrolled_store();
        creds.set_accepted_manifest_hash(Some("old")).unwrap();
        submit_decision(&api, &creds, &offer, ConsentAction::Decline)
            .await
            .unwrap();
        assert!(creds.get_accepted_manifest_hash().is_none());
        assert!(creds.get_accepted_flags().is_none());
    }

    #[tokio::test]
    async fn test_consent_rejected_by_server_keeps_nothing_accepted() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/workers/me/consent"))
            .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                "error": { "code": "CONFLICT", "message": "stale",
                           "details": { "reason": "CONSENT_VERSION_MISMATCH", "current_consent_version": 4 } }
            })))
            .mount(&server)
            .await;

        let api = ApiClient::new(server.uri()).unwrap();
        let creds = enrolled_store();
        let selection = ConsentFlags {
            system_activity: true,
            ..Default::default()
        };
        let err = submit_decision(&api, &creds, &offer(2), ConsentAction::Accept(selection))
            .await
            .unwrap_err();
        assert_eq!(current_version_from_conflict(&err), Some(4));
        assert!(creds.get_accepted_manifest_hash().is_none());
        assert!(creds.get_accepted_flags().is_none());
    }

    #[test]
    fn test_consent_keep_previous_selection_is_explicit_and_restricted() {
        let (_p, admin_pub) = generate_admin_keypair();
        let previous = ConsentFlags {
            system_activity: true,
            location: true, // no longer requested
            ..Default::default()
        };
        let offer =
            ConsentOffer::from_worker_self(&worker_self(4), &admin_pub, Some(previous)).unwrap();
        let kept = offer.keep_previous_selection();
        assert!(kept.system_activity && !kept.location);
        // Without a previous selection nothing is pre-selected (DP-08).
        let fresh = ConsentOffer::from_worker_self(&worker_self(4), &admin_pub, None).unwrap();
        assert_eq!(fresh.keep_previous_selection(), ConsentFlags::default());
        let empty = ConsentOffer::from_worker_self(
            &worker_self(4),
            &admin_pub,
            Some(ConsentFlags::default()),
        )
        .unwrap();
        assert!(empty.previous_selection.is_none());
    }
}
