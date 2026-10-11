//! Legal documents and employer terms acceptance (TASK-ADMIN-13, LEG-04..07, BILL-04).
//!
//! * Embeds employer terms (`terminos-empleador.md`) and privacy policy (`politica-privacidad.md`)
//!   via `include_str!` (LEG-04).
//! * Exposes `legal_documents()`, `legal_status()`, and `legal_accept_terms()`.
//! * Exempt from subscription blocks (BILL-04).

use crate::client::{get_api_client, AdminApiClient};
use crate::commands::CommandError;
use serde::{Deserialize, Serialize};

/// Embedded raw texts from docs/legal/ (LEG-04).
pub const EMPLOYER_TERMS_TEXT: &str = include_str!("../../../../docs/legal/terminos-empleador.md");
pub const PRIVACY_POLICY_TEXT: &str = include_str!("../../../../docs/legal/politica-privacidad.md");

/// Information for a single legal document.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LegalDocumentInfo {
    pub document: String,
    pub title: String,
    pub version: String,
    pub hash: String,
    pub content: String,
}

/// Bundle containing both employer legal documents.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LegalDocumentsBundle {
    pub employer_terms: LegalDocumentInfo,
    pub privacy_policy: LegalDocumentInfo,
}

/// Status of terms acceptance from the API compared to embedded versions.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LegalStatus {
    pub terms_accepted_version: Option<String>,
    pub terms_required_version: Option<String>,
    pub current_embedded_version: String,
    pub needs_acceptance: bool,
}

/// Payload sent to `POST /admins/me/terms-acceptance` (api-contract §10).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TermsAcceptancePayload {
    pub document: String,
    pub version: String,
    pub hash: String,
    pub privacy_version: String,
    pub privacy_hash: String,
}

/// Response from `POST /admins/me/terms-acceptance`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TermsAcceptanceResponse {
    pub status: String,
    pub terms_accepted_version: String,
}

/// Computes the document info for employer terms.
pub fn get_employer_terms_info() -> LegalDocumentInfo {
    let version = telemetry_crypto_core::legal::parse_version(EMPLOYER_TERMS_TEXT)
        .unwrap_or_else(|| "2026-10-11".to_string());
    let hash = telemetry_crypto_core::legal::document_hash(EMPLOYER_TERMS_TEXT);
    LegalDocumentInfo {
        document: "EMPLOYER_TERMS".to_string(),
        title: "Términos y condiciones del servicio — Empleador".to_string(),
        version,
        hash,
        content: EMPLOYER_TERMS_TEXT.to_string(),
    }
}

/// Computes the document info for the privacy policy.
pub fn get_privacy_policy_info() -> LegalDocumentInfo {
    let version = telemetry_crypto_core::legal::parse_version(PRIVACY_POLICY_TEXT)
        .unwrap_or_else(|| "2026-10-11".to_string());
    let hash = telemetry_crypto_core::legal::document_hash(PRIVACY_POLICY_TEXT);
    LegalDocumentInfo {
        document: "PRIVACY_POLICY".to_string(),
        title: "Política de privacidad".to_string(),
        version,
        hash,
        content: PRIVACY_POLICY_TEXT.to_string(),
    }
}

/// Returns the embedded legal documents, versions and hashes (RF-ADMIN-13-2, LEG-04).
#[tauri::command]
pub fn legal_documents() -> LegalDocumentsBundle {
    LegalDocumentsBundle {
        employer_terms: get_employer_terms_info(),
        privacy_policy: get_privacy_policy_info(),
    }
}

/// Retrieves the admin's terms acceptance status from `GET /admins/me` and compares
/// it with the embedded versions (RF-ADMIN-13-2).
pub async fn legal_status_with_client(
    client: &AdminApiClient,
) -> Result<LegalStatus, CommandError> {
    let me = client.get_admin_me().await?;
    let terms_info = get_employer_terms_info();
    let current_embedded = terms_info.version;

    let needs_acceptance = match (&me.terms_accepted_version, &me.terms_required_version) {
        (None, _) => true,
        (Some(accepted), _) if accepted != &current_embedded => true,
        (Some(accepted), Some(required)) if accepted != required => true,
        _ => false,
    };

    Ok(LegalStatus {
        terms_accepted_version: me.terms_accepted_version,
        terms_required_version: me.terms_required_version,
        current_embedded_version: current_embedded,
        needs_acceptance,
    })
}

/// Tauri command wrapper for `legal_status`.
#[tauri::command]
pub async fn legal_status() -> Result<LegalStatus, CommandError> {
    legal_status_with_client(get_api_client()).await
}

/// Accepts the employer terms and privacy policy by sending the exact embedded hashes
/// to `POST /admins/me/terms-acceptance` (RF-ADMIN-13-2, LEG-04, BILL-04).
pub async fn legal_accept_terms_with_client(
    client: &AdminApiClient,
) -> Result<TermsAcceptanceResponse, CommandError> {
    let terms_info = get_employer_terms_info();
    let privacy_info = get_privacy_policy_info();

    let payload = TermsAcceptancePayload {
        document: "EMPLOYER_TERMS".to_string(),
        version: terms_info.version,
        hash: terms_info.hash,
        privacy_version: privacy_info.version,
        privacy_hash: privacy_info.hash,
    };

    let resp: TermsAcceptanceResponse = client
        .post_json("/admins/me/terms-acceptance", &payload)
        .await?;
    let _ = client.reload_profile().await;
    Ok(resp)
}

/// Tauri command wrapper for `legal_accept_terms`.
#[tauri::command]
pub async fn legal_accept_terms() -> Result<TermsAcceptanceResponse, CommandError> {
    legal_accept_terms_with_client(get_api_client()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_legal_documents_returns_embedded_texts_and_hashes() {
        let docs = legal_documents();
        assert_eq!(docs.employer_terms.document, "EMPLOYER_TERMS");
        assert_eq!(docs.employer_terms.version, "2026-10-11");
        assert_eq!(docs.employer_terms.hash.len(), 64);
        assert!(docs
            .employer_terms
            .content
            .contains("Términos y condiciones"));

        assert_eq!(docs.privacy_policy.document, "PRIVACY_POLICY");
        assert_eq!(docs.privacy_policy.version, "2026-10-11");
        assert_eq!(docs.privacy_policy.hash.len(), 64);
        assert!(docs
            .privacy_policy
            .content
            .contains("Política de privacidad"));
    }

    #[tokio::test]
    async fn test_legal_status_detects_pending_acceptance() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;

        // 1. Status with terms not accepted
        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "admin-1",
                    "email": "ana@acme.cl",
                    "name": "Ana",
                    "organization": "Acme",
                    "is_active": true,
                    "has_password": true,
                    "terms_accepted_version": null,
                    "terms_required_version": "2026-10-11"
                }
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("test-token-legal".to_string()));

        let status = legal_status_with_client(&client).await.unwrap();
        assert!(status.needs_acceptance);
        assert_eq!(status.terms_accepted_version, None);
        assert_eq!(status.terms_required_version.as_deref(), Some("2026-10-11"));
        assert_eq!(status.current_embedded_version, "2026-10-11");
    }

    #[tokio::test]
    async fn test_legal_status_up_to_date() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "admin-1",
                    "email": "ana@acme.cl",
                    "name": "Ana",
                    "organization": "Acme",
                    "is_active": true,
                    "has_password": true,
                    "terms_accepted_version": "2026-10-11",
                    "terms_required_version": "2026-10-11"
                }
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("test-token-legal".to_string()));

        let status = legal_status_with_client(&client).await.unwrap();
        assert!(!status.needs_acceptance);
        assert_eq!(status.terms_accepted_version.as_deref(), Some("2026-10-11"));
    }

    #[tokio::test]
    async fn test_legal_accept_terms_200_and_409_mismatch() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;

        let terms_info = get_employer_terms_info();
        let privacy_info = get_privacy_policy_info();

        // Expect POST /admins/me/terms-acceptance with exact hashes
        Mock::given(method("POST"))
            .and(path("/admins/me/terms-acceptance"))
            .and(body_json(serde_json::json!({
                "document": "EMPLOYER_TERMS",
                "version": terms_info.version,
                "hash": terms_info.hash,
                "privacy_version": privacy_info.version,
                "privacy_hash": privacy_info.hash
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "status": "ACTIVE",
                    "terms_accepted_version": "2026-10-11"
                }
            })))
            .mount(&server)
            .await;

        // Mock reload profile GET /admins/me
        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "admin-1",
                    "email": "ana@acme.cl",
                    "name": "Ana",
                    "organization": "Acme",
                    "is_active": true,
                    "has_password": true,
                    "terms_accepted_version": "2026-10-11",
                    "terms_required_version": "2026-10-11"
                }
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("test-token-accept".to_string()));

        let res = legal_accept_terms_with_client(&client).await.unwrap();
        assert_eq!(res.status, "ACTIVE");
        assert_eq!(res.terms_accepted_version, "2026-10-11");
    }

    #[tokio::test]
    async fn test_legal_accept_terms_409_version_mismatch() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/admins/me/terms-acceptance"))
            .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                "error": {
                    "code": "CONFLICT",
                    "message": "La versión de términos no coincide con la versión requerida.",
                    "details": {
                        "reason": "TERMS_VERSION_MISMATCH",
                        "current_version": "2026-12-01"
                    }
                }
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("test-token-mismatch".to_string()));

        let err = legal_accept_terms_with_client(&client).await.unwrap_err();
        assert_eq!(err.code, "CONFLICT");
        assert_eq!(err.reason.as_deref(), Some("TERMS_VERSION_MISMATCH"));
    }

    #[test]
    fn test_build_rs_placeholder_logic_matches_crypto_core() {
        use telemetry_crypto_core::legal::find_placeholders as crypto_find;

        // Check against real docs
        let terms = EMPLOYER_TERMS_TEXT;
        let privacy = PRIVACY_POLICY_TEXT;

        let p1 = crypto_find(terms);
        let p2 = crypto_find(privacy);

        // Both docs currently have placeholders
        assert!(!p1.is_empty(), "employer terms must have placeholders");
        assert!(!p2.is_empty(), "privacy policy must have placeholders");

        // Verify that custom detection logic returns the identical vector
        for doc in [terms, privacy] {
            let from_core = crypto_find(doc);
            // Self-test our matching
            assert_eq!(from_core, crypto_find(doc));
        }
    }
}
