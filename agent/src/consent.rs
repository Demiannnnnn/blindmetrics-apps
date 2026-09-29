//! Consent package shown on C1 and the decisions sent to the server.
//!
//! The worker accepts or rejects the whole package (DP-08). Every decision
//! carries the commitment of the manifest that was displayed (LEG-02), which
//! is the same commitment the admin put in the enrollment QR.

use crate::api::client::{ApiClient, ConsentDecisionResponse};
use crate::credentials::CredentialStore;
use crate::enrollment::PendingEnrollment;
use crate::session::{with_access_token, SessionError};
use crate::state::WorkerSelf;
use telemetry_crypto_core::{
    manifest_commitment, ConsentDescriptions, ConsentFlags, CryptoError, EnrollmentManifest,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Accept,
    Reject,
    Withdraw,
}

impl Decision {
    pub fn as_str(self) -> &'static str {
        match self {
            Decision::Accept => "ACCEPT",
            Decision::Reject => "REJECT",
            Decision::Withdraw => "WITHDRAW",
        }
    }
}

/// What the worker is asked to accept on C1.
#[derive(Clone, Debug, PartialEq)]
pub struct ConsentOffer {
    pub organization: Option<String>,
    pub consent_version: i32,
    /// Requested categories (what collection would start with on ACCEPT).
    pub flags: ConsentFlags,
    pub descriptions: ConsentDescriptions,
    pub work_hours_start: Option<String>,
    pub work_hours_end: Option<String>,
    pub timezone: Option<String>,
    /// Commitment of the displayed manifest, sent as `manifest_hash`.
    pub manifest_hash: String,
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
        }
    }

    /// Offer for a later version (or after restart), rebuilt from `/workers/me`
    /// and the key pinned at enrollment — never a key sent by the server (SEC-08).
    pub fn from_worker_self(me: &WorkerSelf, pinned_admin_pub: &str) -> Result<Self, CryptoError> {
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
        })
    }
}

/// Sends ACCEPT or REJECT for `offer` and updates the locally accepted
/// package only after the server recorded the decision.
pub async fn submit_offer_decision(
    api: &ApiClient,
    creds: &dyn CredentialStore,
    offer: &ConsentOffer,
    decision: Decision,
) -> Result<ConsentDecisionResponse, SessionError> {
    let resp = send_decision(
        api,
        creds,
        decision,
        offer.consent_version,
        &offer.manifest_hash,
    )
    .await?;
    match decision {
        Decision::Accept => {
            creds
                .set_accepted_manifest_hash(Some(&offer.manifest_hash))
                .map_err(SessionError::Store)?;
            creds
                .set_accepted_flags(Some(&offer.flags))
                .map_err(SessionError::Store)?;
        }
        Decision::Reject | Decision::Withdraw => {
            creds
                .set_accepted_manifest_hash(None)
                .map_err(SessionError::Store)?;
            creds
                .set_accepted_flags(None)
                .map_err(SessionError::Store)?;
        }
    }
    tracing::info!(
        decision = decision.as_str(),
        consent_version = offer.consent_version,
        "Consent decision recorded"
    );
    Ok(resp)
}

/// `PATCH /workers/:id/consent` with automatic token refresh.
pub async fn send_decision(
    api: &ApiClient,
    creds: &dyn CredentialStore,
    decision: Decision,
    consent_version: i32,
    manifest_hash: &str,
) -> Result<ConsentDecisionResponse, SessionError> {
    let worker_id = creds.get_worker_id().ok_or(SessionError::NotEnrolled)?;
    with_access_token(api, creds, |token| {
        let api = api.clone();
        let worker_id = worker_id.clone();
        let manifest_hash = manifest_hash.to_string();
        async move {
            api.patch_consent(
                &worker_id,
                &token,
                decision.as_str(),
                consent_version,
                &manifest_hash,
            )
            .await
        }
    })
    .await
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
            status: WorkerStatus::PendingConsent,
            role_type: "OFFICE".into(),
            work_hours_start: Some("09:00:00".into()),
            work_hours_end: Some("18:00:00".into()),
            timezone: Some("America/Santiago".into()),
            consent_version: version,
            accepted_consent_version: Some(1),
            consent_flags: ConsentFlags::default(),
            requested_consent_flags: ConsentFlags {
                system_activity: true,
                ..Default::default()
            },
            consent_descriptions: ConsentDescriptions {
                system_activity: Some("Aplicaciones en uso".into()),
                ..Default::default()
            },
            admin_public_key_fingerprint: "fp".into(),
        }
    }

    fn enrolled_store() -> InMemoryCredentialStore {
        let creds = InMemoryCredentialStore::new();
        creds.set_worker_id(Some("w-1")).unwrap();
        creds.set_access_token(Some("tok")).unwrap();
        creds.set_refresh_token(Some("ref")).unwrap();
        creds
    }

    #[test]
    fn test_consent_offer_from_worker_self_matches_admin_manifest() {
        let (_priv, admin_pub) = generate_admin_keypair();
        let me = worker_self(2);
        let offer = ConsentOffer::from_worker_self(&me, &admin_pub).unwrap();
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
    }

    #[tokio::test]
    async fn test_consent_accept_sends_real_version_and_hash_then_persists() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/workers/w-1/consent"))
            .and(body_json(serde_json::json!({
                "decision": "ACCEPT", "consent_version": 3, "manifest_hash": "h".repeat(64)
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "status": "ACTIVE", "consent_version": 3, "accepted_consent_version": 3,
                    "consent_flags": { "system_activity": true, "network_activity": false,
                        "productivity_basic": false, "productivity_screenshots": false,
                        "file_activity": false, "location": false },
                    "consent_accepted_at": "2026-09-29T10:00:00Z"
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let api = ApiClient::new(server.uri()).unwrap();
        let creds = enrolled_store();
        let offer = ConsentOffer {
            organization: None,
            consent_version: 3,
            flags: ConsentFlags {
                system_activity: true,
                ..Default::default()
            },
            descriptions: ConsentDescriptions::default(),
            work_hours_start: None,
            work_hours_end: None,
            timezone: None,
            manifest_hash: "h".repeat(64),
        };
        submit_offer_decision(&api, &creds, &offer, Decision::Accept)
            .await
            .unwrap();
        assert_eq!(
            creds.get_accepted_manifest_hash().as_deref(),
            Some("h".repeat(64).as_str())
        );
        assert!(creds.get_accepted_flags().unwrap().system_activity);
    }

    #[tokio::test]
    async fn test_consent_rejected_by_server_keeps_nothing_accepted() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/workers/w-1/consent"))
            .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                "error": { "code": "CONFLICT", "message": "stale",
                           "details": { "reason": "CONSENT_VERSION_MISMATCH" } }
            })))
            .mount(&server)
            .await;

        let api = ApiClient::new(server.uri()).unwrap();
        let creds = enrolled_store();
        let offer =
            ConsentOffer::from_worker_self(&worker_self(2), &generate_admin_keypair().1).unwrap();
        let err = submit_offer_decision(&api, &creds, &offer, Decision::Accept)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            SessionError::Api(crate::errors::AgentError::Api { status: 409, .. })
        ));
        assert!(creds.get_accepted_manifest_hash().is_none());
        assert!(creds.get_accepted_flags().is_none());
    }
}
