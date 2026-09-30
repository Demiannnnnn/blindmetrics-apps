//! Subscription check on every start and billing portal (TASK-ADMIN-12 = TASK-APP-01).
//!
//! The desktop app never processes payments (BILL-01): it reads `GET /subscriptions/me` and
//! opens the portal URL **provided by the server** after validating it (SEC-17).

use super::CommandError;
use crate::client::{get_api_client, AdminApiClient};
use serde::{Deserialize, Serialize};

const MAX_PORTAL_URL_LEN: usize = 2048;

/// `GET /subscriptions/me` (api-contract §4).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SubscriptionStatus {
    pub status: String,
    pub effective_status: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub grace_period_ends_at: Option<String>,
    #[serde(default)]
    pub trial_days_left: Option<i64>,
    #[serde(default)]
    pub status_changed_at: Option<String>,
    #[serde(default)]
    pub portal_url: Option<String>,
    #[serde(default)]
    pub support_email: Option<String>,
    #[serde(default)]
    pub billing_configured: bool,
}

impl SubscriptionStatus {
    /// `SUSPENDED` or `CANCELLED`: the console must be locked (BILL-02).
    pub fn is_blocked(&self) -> bool {
        matches!(self.effective_status.as_str(), "SUSPENDED" | "CANCELLED")
    }
}

const KNOWN_STATUSES: [&str; 4] = ["TRIAL", "ACTIVE", "SUSPENDED", "CANCELLED"];

pub async fn subscription_status_with_client(
    client: &AdminApiClient,
) -> Result<SubscriptionStatus, CommandError> {
    let status: SubscriptionStatus = client.get_json("/subscriptions/me").await?;
    if !KNOWN_STATUSES.contains(&status.effective_status.as_str()) {
        return Err(CommandError::new(
            "SUBSCRIPTION_STATUS_UNKNOWN",
            "No se pudo interpretar el estado de la suscripción.",
        ));
    }
    client.record_portal(status.portal_url.clone(), status.support_email.clone());
    if status.effective_status == "TRIAL" {
        client.set_trial_days_left(status.trial_days_left);
    } else {
        client.set_trial_days_left(None);
    }
    tracing::info!(
        effective_status = %status.effective_status,
        trial_days_left = ?status.trial_days_left,
        "subscription checked"
    );
    Ok(status)
}

/// RF-ADMIN-12-1: called by the UI after every login, unlock and app start.
#[tauri::command]
pub async fn subscription_status() -> Result<SubscriptionStatus, CommandError> {
    subscription_status_with_client(get_api_client()).await
}

/// SEC-17: `https://` with a host, no credentials, ≤ 2048 chars; `http://localhost` or
/// `http://127.0.0.1` only when `allow_local_http` (debug builds).
pub fn validate_portal_url(url: &str, allow_local_http: bool) -> Result<(), CommandError> {
    let invalid = || {
        CommandError::new(
            "PORTAL_URL_INVALID",
            "La dirección del portal no es válida.",
        )
    };
    let url = url.trim();
    if url.is_empty() || url.len() > MAX_PORTAL_URL_LEN || url.chars().any(char::is_whitespace) {
        return Err(invalid());
    }
    let rest = if let Some(rest) = url.strip_prefix("https://") {
        rest
    } else if let Some(rest) = url.strip_prefix("http://").filter(|_| allow_local_http) {
        let host = rest.split(['/', ':', '?', '#']).next().unwrap_or_default();
        if host != "localhost" && host != "127.0.0.1" {
            return Err(invalid());
        }
        rest
    } else {
        return Err(invalid());
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') || authority.starts_with(':') {
        return Err(invalid());
    }
    Ok(())
}

/// Opens the last portal URL received from the server in the system browser (RF-ADMIN-12-3).
#[tauri::command]
pub fn open_billing_portal() -> Result<(), CommandError> {
    let Some(url) = get_api_client().portal().portal_url else {
        return Err(CommandError::new(
            "PORTAL_URL_UNAVAILABLE",
            "El servidor no informó la dirección del portal; contacta a soporte.",
        ));
    };
    if let Err(e) = validate_portal_url(&url, cfg!(debug_assertions)) {
        tracing::warn!("billing portal url rejected");
        return Err(e);
    }
    open::that_detached(&url).map_err(|e| {
        tracing::warn!("system browser unavailable: {}", e);
        CommandError::new(
            "BROWSER_UNAVAILABLE",
            "No se pudo abrir el navegador. Copia la dirección y ábrela manualmente.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_subscription_status_parses_contract() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/subscriptions/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "status": "TRIAL", "effective_status": "SUSPENDED", "reason": "TRIAL_EXPIRED",
                    "expires_at": "2026-09-20T00:00:00Z", "grace_period_ends_at": null,
                    "trial_days_left": 0, "status_changed_at": "2026-09-01T00:00:00Z",
                    "portal_url": "https://app.tudominio.cl/dashboard/facturacion",
                    "support_email": "contacto@tudominio.cl", "billing_configured": true
                }
            })))
            .mount(&server)
            .await;
        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("a".into()));
        let status = subscription_status_with_client(&client).await.unwrap();
        assert!(status.is_blocked());
        assert_eq!(status.reason.as_deref(), Some("TRIAL_EXPIRED"));
        assert_eq!(
            client.portal().portal_url.as_deref(),
            Some("https://app.tudominio.cl/dashboard/facturacion")
        );
        assert_eq!(client.trial_days_left(), None);
    }

    #[tokio::test]
    async fn test_subscription_status_trial_records_days_and_rejects_unknown() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/subscriptions/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "status": "TRIAL", "effective_status": "TRIAL", "trial_days_left": 5,
                          "portal_url": "https://app.tudominio.cl/f", "support_email": "s@t.cl",
                          "billing_configured": false }
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/subscriptions/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "status": "WEIRD", "effective_status": "WEIRD" }
            })))
            .mount(&server)
            .await;
        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("a".into()));
        let status = subscription_status_with_client(&client).await.unwrap();
        assert!(!status.is_blocked());
        assert_eq!(client.trial_days_left(), Some(5));
        let err = subscription_status_with_client(&client).await.unwrap_err();
        assert_eq!(err.code, "SUBSCRIPTION_STATUS_UNKNOWN");
    }

    #[test]
    fn test_validate_portal_url_rules() {
        assert!(
            validate_portal_url("https://app.tudominio.cl/dashboard/facturacion", false).is_ok()
        );
        assert!(validate_portal_url("https://app.tudominio.cl", false).is_ok());
        assert!(validate_portal_url("http://app.tudominio.cl/f", false).is_err());
        assert!(validate_portal_url("http://localhost:3000/dashboard/facturacion", false).is_err());
        assert!(validate_portal_url("http://localhost:3000/dashboard/facturacion", true).is_ok());
        assert!(validate_portal_url("http://127.0.0.1:3000/f", true).is_ok());
        assert!(validate_portal_url("http://evil.example/f", true).is_err());
        assert!(validate_portal_url("https://user:pass@app.tudominio.cl/f", false).is_err());
        assert!(validate_portal_url("javascript:alert(1)", true).is_err());
        assert!(validate_portal_url("file:///etc/passwd", true).is_err());
        assert!(validate_portal_url("https://", false).is_err());
        assert!(validate_portal_url("https://app.tudominio.cl/a b", false).is_err());
        let long = format!("https://app.tudominio.cl/{}", "a".repeat(2100));
        assert!(validate_portal_url(&long, false).is_err());
    }
}
