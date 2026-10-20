use crate::errors::AgentError;
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use telemetry_crypto_core::{ConsentDescriptions, ConsentFlags};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_VERSION_HEADER: &str = "X-App-Version";
/// Largest `Retry-After` honoured (24 h).
const MAX_RETRY_AFTER_SECONDS: u64 = 86_400;

/// `User-Agent` stored by the API as consent evidence (TASK-INT-02).
pub fn user_agent() -> String {
    format!("telemetry-agent/{APP_VERSION}")
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SingleResponse<T> {
    pub data: T,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ApiErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ApiErrorResponse {
    pub error: ApiErrorDetail,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct IngestBody<'a> {
    pub records: Vec<IngestRecord<'a>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct IngestRecord<'a> {
    pub ephemeral_public_key: &'a str,
    pub encrypted_payload: &'a str, // base64
    pub client_timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subcategory: Option<&'static str>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct IngestResponse {
    pub batch_id: String,
    pub records_created: usize,
    pub category: String,
    pub server_timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UserDto {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub user: UserDto,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct HealthResponse {
    pub status: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub hostname: String,
    pub os: String,
    pub app_version: String,
}

impl DeviceInfo {
    pub fn collect() -> Self {
        let hostname = sysinfo::System::host_name().unwrap_or_else(|| "unknown-host".to_string());
        let os = format!(
            "{} {}",
            sysinfo::System::name().unwrap_or_else(|| std::env::consts::OS.to_string()),
            sysinfo::System::os_version().unwrap_or_default()
        )
        .trim()
        .to_string();
        Self {
            hostname,
            os,
            app_version: APP_VERSION.to_string(),
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct EnrollRequest<'a> {
    pub token: &'a str,
    pub device_identifier: &'a str,
    pub device_info: &'a DeviceInfo,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EnrollResponse {
    pub worker_id: String,
    pub worker_name: String,
    pub organization: String,
    pub role_type: String,
    #[serde(default)]
    pub work_hours_start: Option<String>,
    #[serde(default)]
    pub work_hours_end: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    pub status: crate::state::WorkerStatus,
    pub auth: AuthResponse,
    pub admin_public_key: String,
    pub admin_public_key_fingerprint: String,
    pub consent_version: i32,
    pub consent_flags: ConsentFlags,
    pub consent_descriptions: ConsentDescriptions,
}

/// `PATCH /workers/me/consent` (granular consent, TASK-AGENT-09).
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct ConsentRequest<'a> {
    pub accepted: bool,
    pub consent_version: i32,
    /// Commitment of the full manifest displayed (LEG-02).
    pub manifest_hash: &'a str,
    /// Chosen subset; only with `accepted = true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consent_flags: Option<ConsentFlags>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ConsentDecisionResponse {
    pub status: crate::state::WorkerStatus,
    pub consent_version: i32,
    pub accepted_consent_version: Option<i32>,
    /// Effective flags = the accepted subset.
    pub consent_flags: ConsentFlags,
    #[serde(default)]
    pub consent_accepted_at: Option<String>,
    #[serde(default)]
    pub consent_rejected_at: Option<String>,
    #[serde(default)]
    pub enrollment_completed_at: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DataRequestResponse {
    pub request_id: String,
    pub status: String,
    #[serde(default)]
    pub estimated_completion: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DataDeletionResponse {
    pub request_id: String,
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    pub fn new(base_url: String) -> Result<Self, AgentError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent(user_agent())
            .build()
            .map_err(AgentError::Reqwest)?;
        Ok(Self { client, base_url })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Dispatches telemetry records to `/telemetry/{endpoint}`.
    pub async fn ingest(
        &self,
        endpoint: &str,
        records: &[IngestRecord<'_>],
        token: Option<&str>,
    ) -> Result<IngestResponse, AgentError> {
        let url = format!(
            "{}/telemetry/{}",
            self.base_url.trim_end_matches('/'),
            endpoint
        );
        let body = IngestBody {
            records: records.to_vec(),
        };

        let mut req = self
            .client
            .post(&url)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(&body);

        if let Some(t) = token {
            if !t.is_empty() {
                req = req.bearer_auth(t);
            }
        }

        let resp = req
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;
        let status = resp.status();

        if status.is_success() {
            let wrapped: SingleResponse<IngestResponse> =
                resp.json().await.map_err(AgentError::Reqwest)?;
            Ok(wrapped.data)
        } else {
            Self::handle_error_response(status, resp).await
        }
    }

    /// Calls `POST /auth/refresh` to rotate credentials.
    pub async fn refresh(&self, refresh_token: &str) -> Result<AuthResponse, AgentError> {
        let url = format!("{}/auth/refresh", self.base_url.trim_end_matches('/'));
        let body = serde_json::json!({ "refresh_token": refresh_token });

        let resp = self
            .client
            .post(&url)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;

        let status = resp.status();
        if status.is_success() {
            let wrapped: SingleResponse<AuthResponse> =
                resp.json().await.map_err(AgentError::Reqwest)?;
            Ok(wrapped.data)
        } else if status == StatusCode::UNAUTHORIZED {
            Err(AgentError::Unauthorized)
        } else {
            Self::handle_error_response(status, resp).await
        }
    }

    /// Calls `GET /health` to verify server connectivity.
    pub async fn health(&self) -> Result<HealthResponse, AgentError> {
        let url = format!("{}/health", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .get(&url)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;

        let status = resp.status();
        if status.is_success() {
            let res: HealthResponse = resp.json().await.map_err(AgentError::Reqwest)?;
            Ok(res)
        } else {
            Err(AgentError::ServerError(status.as_u16()))
        }
    }

    /// Calls `POST /workers/enroll` with 15s timeout to redeem a one-time token.
    pub async fn enroll(
        &self,
        token: &str,
        device_identifier: &str,
        device_info: &DeviceInfo,
    ) -> Result<EnrollResponse, AgentError> {
        let url = format!("{}/workers/enroll", self.base_url.trim_end_matches('/'));
        let body = EnrollRequest {
            token,
            device_identifier,
            device_info,
        };

        let resp = self
            .client
            .post(&url)
            .timeout(Duration::from_secs(15))
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;

        let status = resp.status();
        if status.is_success() {
            let wrapped: SingleResponse<EnrollResponse> =
                resp.json().await.map_err(AgentError::Reqwest)?;
            Ok(wrapped.data)
        } else {
            Self::handle_error_response(status, resp).await
        }
    }

    /// Calls `GET /workers/me` with Bearer auth to fetch current worker lifecycle state.
    pub async fn get_me(&self, access_token: &str) -> Result<crate::state::WorkerSelf, AgentError> {
        let url = format!("{}/workers/me", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .get(&url)
            .bearer_auth(access_token)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;

        let status = resp.status();
        if status.is_success() {
            let wrapped: SingleResponse<crate::state::WorkerSelf> =
                resp.json().await.map_err(AgentError::Reqwest)?;
            Ok(wrapped.data)
        } else {
            Self::handle_error_response(status, resp).await
        }
    }

    /// `PATCH /workers/me/consent`: accept a subset, reject or withdraw (TASK-AGENT-09).
    /// Replaces the deprecated `PATCH /workers/:id/consent`.
    pub async fn patch_my_consent(
        &self,
        access_token: &str,
        body: &ConsentRequest<'_>,
    ) -> Result<ConsentDecisionResponse, AgentError> {
        let url = format!("{}/workers/me/consent", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .patch(&url)
            .bearer_auth(access_token)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(body)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;

        let status = resp.status();
        if status.is_success() {
            let wrapped: SingleResponse<ConsentDecisionResponse> =
                resp.json().await.map_err(AgentError::Reqwest)?;
            Ok(wrapped.data)
        } else {
            Self::handle_error_response(status, resp).await
        }
    }

    /// Calls `POST /auth/logout` to revoke refresh token.
    pub async fn logout(&self, refresh_token: &str) -> Result<(), AgentError> {
        let url = format!("{}/auth/logout", self.base_url.trim_end_matches('/'));
        let body = serde_json::json!({ "refresh_token": refresh_token });

        let resp = self
            .client
            .post(&url)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;

        let status = resp.status();
        if status.is_success() || status == StatusCode::NO_CONTENT {
            Ok(())
        } else {
            Self::handle_error_response(status, resp).await
        }
    }

    /// Calls `POST /workers/me/data-request` to request an export of worker data (ARCO / GDPR).
    pub async fn post_data_request(
        &self,
        access_token: &str,
        categories: &[&str],
    ) -> Result<DataRequestResponse, AgentError> {
        let url = format!(
            "{}/workers/me/data-request",
            self.base_url.trim_end_matches('/')
        );
        let body = serde_json::json!({
            "type": "EXPORT",
            "categories": categories,
        });

        let resp = self
            .client
            .post(&url)
            .bearer_auth(access_token)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;

        let status = resp.status();
        if status.is_success() || status == StatusCode::ACCEPTED {
            let val: serde_json::Value = resp.json().await.map_err(AgentError::Reqwest)?;
            let item: DataRequestResponse = if let Some(data) = val.get("data") {
                serde_json::from_value(data.clone())
                    .map_err(|e| AgentError::Network(e.to_string()))?
            } else {
                serde_json::from_value(val).map_err(|e| AgentError::Network(e.to_string()))?
            };
            Ok(item)
        } else {
            Self::handle_error_response(status, resp).await
        }
    }

    /// Calls `POST /workers/me/data-deletion` to request formal deletion of data (ARCO / GDPR).
    pub async fn post_data_deletion(
        &self,
        access_token: &str,
        reason: Option<&str>,
    ) -> Result<DataDeletionResponse, AgentError> {
        let url = format!(
            "{}/workers/me/data-deletion",
            self.base_url.trim_end_matches('/')
        );
        let mut body = serde_json::json!({
            "categories": ["all"],
        });
        if let Some(r) = reason {
            body["reason"] = serde_json::json!(r);
        }

        let resp = self
            .client
            .post(&url)
            .bearer_auth(access_token)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;

        let status = resp.status();
        if status.is_success() || status == StatusCode::ACCEPTED {
            let val: serde_json::Value = resp.json().await.map_err(AgentError::Reqwest)?;
            let item: DataDeletionResponse = if let Some(data) = val.get("data") {
                serde_json::from_value(data.clone())
                    .map_err(|e| AgentError::Network(e.to_string()))?
            } else {
                serde_json::from_value(val).map_err(|e| AgentError::Network(e.to_string()))?
            };
            Ok(item)
        } else {
            Self::handle_error_response(status, resp).await
        }
    }

    async fn handle_error_response<T>(
        status: StatusCode,
        resp: reqwest::Response,
    ) -> Result<T, AgentError> {
        if status == StatusCode::UNAUTHORIZED {
            return Err(AgentError::Unauthorized);
        }
        let retry_header = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .filter(|s| *s <= MAX_RETRY_AFTER_SECONDS);

        let status_code = status.as_u16();
        match resp.json::<ApiErrorResponse>().await {
            Ok(err_resp) => {
                let details = err_resp.error.details.filter(|d| d.is_object());
                let reason = details
                    .as_ref()
                    .and_then(|d| d.get("reason").and_then(|r| r.as_str().map(String::from)));
                let retry_detail = details
                    .as_ref()
                    .and_then(|d| d.get("retry_after_seconds"))
                    .and_then(|r| r.as_u64())
                    .filter(|s| *s <= MAX_RETRY_AFTER_SECONDS);
                Err(AgentError::Api {
                    status: status_code,
                    code: err_resp.error.code,
                    reason,
                    retry_after_seconds: retry_header.or(retry_detail),
                    details,
                })
            }
            Err(_) => Err(AgentError::ServerError(status_code)),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Contract v3: terms, PIN events and attendance (api-contract.md §9, §10, §12)
// ---------------------------------------------------------------------------------------------

/// `POST /workers/me/terms-acceptance` body (LEG-04).
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct TermsAcceptanceRequest<'a> {
    pub document: &'a str,
    pub version: &'a str,
    pub hash: &'a str,
    pub privacy_version: &'a str,
    pub privacy_hash: &'a str,
}

/// `200 { status, terms_accepted_version }`.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TermsAcceptanceResponse {
    pub status: crate::state::WorkerStatus,
    pub terms_accepted_version: String,
}

/// PIN events reported to the server for the R38 art. 7 f email (never the PIN, SEC-19).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CredentialEvent {
    PinSet,
    PinChanged,
}

impl CredentialEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            CredentialEvent::PinSet => "PIN_SET",
            CredentialEvent::PinChanged => "PIN_CHANGED",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "PIN_SET" => Some(CredentialEvent::PinSet),
            "PIN_CHANGED" => Some(CredentialEvent::PinChanged),
            _ => None,
        }
    }
}

/// `POST /attendance/marks` body (`api-contract.md` §9.2).
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct MarkRequest {
    pub client_mark_id: String,
    #[serde(rename = "type")]
    pub r#type: agent_ipc::MarkType,
    pub device_ts: String,
    pub offline: bool,
    pub identification: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<agent_ipc::MarkLocation>,
}

/// `201 { mark }` · `200 { mark, duplicate_of }`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MarkResponse {
    pub mark: agent_ipc::ServerMark,
    #[serde(default)]
    pub duplicate_of: Option<String>,
}

/// Collection envelope `{ data: [...], pagination }`.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct CollectionResponse<T> {
    pub data: Vec<T>,
    #[serde(default)]
    pub pagination: Option<agent_ipc::Pagination>,
}

/// `GET /attendance/verify/:hash` response (R38 art. 8, api-contract §9.4).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PublicVerifyResponse {
    pub valid: bool,
    #[serde(default)]
    pub r#type: Option<agent_ipc::MarkType>,
    #[serde(default)]
    pub effective_date: Option<String>,
    #[serde(default)]
    pub effective_time: Option<String>,
    #[serde(default)]
    pub employer_legal_name: Option<String>,
    #[serde(default)]
    pub workplace_name: Option<String>,
    #[serde(default)]
    pub offline: Option<bool>,
    #[serde(default)]
    pub corrected: Option<bool>,
}

impl ApiClient {
    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    /// Sends an authenticated request and unwraps `{ "data": T }`.
    async fn send_data<T: serde::de::DeserializeOwned>(
        req: reqwest::RequestBuilder,
    ) -> Result<T, AgentError> {
        let resp = req
            .header(APP_VERSION_HEADER, APP_VERSION)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;
        let status = resp.status();
        if status.is_success() {
            let wrapped: SingleResponse<T> = resp.json().await.map_err(AgentError::Reqwest)?;
            Ok(wrapped.data)
        } else {
            Self::handle_error_response(status, resp).await
        }
    }

    /// Sends an authenticated request whose success body is ignored (`202`/`204`).
    async fn send_no_content(req: reqwest::RequestBuilder) -> Result<(), AgentError> {
        let resp = req
            .header(APP_VERSION_HEADER, APP_VERSION)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            Self::handle_error_response(status, resp).await
        }
    }

    /// `POST /workers/me/terms-acceptance` (LEG-04). `409 TERMS_VERSION_MISMATCH` when the
    /// embedded text is not the version the server requires.
    pub async fn accept_terms(
        &self,
        access_token: &str,
        body: &TermsAcceptanceRequest<'_>,
    ) -> Result<TermsAcceptanceResponse, AgentError> {
        let req = self
            .client
            .post(self.url("workers/me/terms-acceptance"))
            .bearer_auth(access_token)
            .json(body);
        Self::send_data(req).await
    }

    /// `POST /workers/me/credential-events { type }` → `204` (SEC-19: the PIN never travels).
    pub async fn post_credential_event(
        &self,
        access_token: &str,
        event: CredentialEvent,
    ) -> Result<(), AgentError> {
        let req = self
            .client
            .post(self.url("workers/me/credential-events"))
            .bearer_auth(access_token)
            .json(&serde_json::json!({ "type": event }));
        Self::send_no_content(req).await
    }

    /// `POST /workers/me/pin-reset/request` → `202` (code to the personal email).
    pub async fn request_pin_reset(&self, access_token: &str) -> Result<(), AgentError> {
        let req = self
            .client
            .post(self.url("workers/me/pin-reset/request"))
            .bearer_auth(access_token)
            .json(&serde_json::json!({}));
        Self::send_no_content(req).await
    }

    /// `POST /workers/me/pin-reset/verify { code }` → `204` · `400 INVALID_RESET_CODE` · `429`.
    pub async fn verify_pin_reset(&self, access_token: &str, code: &str) -> Result<(), AgentError> {
        let req = self
            .client
            .post(self.url("workers/me/pin-reset/verify"))
            .bearer_auth(access_token)
            .json(&serde_json::json!({ "code": code }));
        Self::send_no_content(req).await
    }

    /// `POST /attendance/marks` (idempotent by `client_mark_id`, ATT-04).
    pub async fn post_mark(
        &self,
        access_token: &str,
        body: &MarkRequest,
    ) -> Result<MarkResponse, AgentError> {
        let req = self
            .client
            .post(self.url("attendance/marks"))
            .bearer_auth(access_token)
            .json(body);
        Self::send_data(req).await
    }

    /// `GET /attendance/me/today`.
    pub async fn get_today(
        &self,
        access_token: &str,
    ) -> Result<agent_ipc::TodayResponse, AgentError> {
        let req = self
            .client
            .get(self.url("attendance/me/today"))
            .bearer_auth(access_token);
        Self::send_data(req).await
    }

    /// `GET /attendance/me/marks?from&to&page&per_page` (newest first).
    pub async fn get_my_marks(
        &self,
        access_token: &str,
        from: &str,
        to: &str,
        page: u32,
        per_page: u32,
    ) -> Result<CollectionResponse<agent_ipc::ServerMark>, AgentError> {
        let resp = self
            .client
            .get(self.url("attendance/me/marks"))
            .bearer_auth(access_token)
            .query(&[
                ("from", from.to_string()),
                ("to", to.to_string()),
                ("page", page.to_string()),
                ("per_page", per_page.to_string()),
            ])
            .header(APP_VERSION_HEADER, APP_VERSION)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;
        let status = resp.status();
        if status.is_success() {
            resp.json().await.map_err(AgentError::Reqwest)
        } else {
            Self::handle_error_response(status, resp).await
        }
    }

    /// `GET /attendance/me/days?from&to` (≤ 62 days, server-computed, ATT-09).
    pub async fn get_my_days(
        &self,
        access_token: &str,
        from: &str,
        to: &str,
    ) -> Result<Vec<agent_ipc::DaySummary>, AgentError> {
        let req = self
            .client
            .get(self.url("attendance/me/days"))
            .bearer_auth(access_token)
            .query(&[("from", from), ("to", to)]);
        Self::send_data(req).await
    }

    /// `GET /attendance/me/corrections[?status]`.
    pub async fn get_my_corrections(
        &self,
        access_token: &str,
        status: Option<&str>,
    ) -> Result<Vec<agent_ipc::Correction>, AgentError> {
        let mut req = self
            .client
            .get(self.url("attendance/me/corrections"))
            .bearer_auth(access_token)
            .header(APP_VERSION_HEADER, APP_VERSION);
        if let Some(status) = status {
            req = req.query(&[("status", status)]);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;
        let code = resp.status();
        if code.is_success() {
            let page: CollectionResponse<agent_ipc::Correction> =
                resp.json().await.map_err(AgentError::Reqwest)?;
            Ok(page.data)
        } else {
            Self::handle_error_response(code, resp).await
        }
    }

    /// `POST /attendance/me/corrections/:id/response { accept }` · `409 CORRECTION_NOT_PENDING`.
    pub async fn respond_correction(
        &self,
        access_token: &str,
        id: &str,
        accept: bool,
    ) -> Result<agent_ipc::Correction, AgentError> {
        let req = self
            .client
            .post(self.url(&format!("attendance/me/corrections/{id}/response")))
            .bearer_auth(access_token)
            .json(&serde_json::json!({ "accept": accept }));
        Self::send_data(req).await
    }

    /// `GET /attendance/verify/:hash` (public receipt verification, api-contract §9.4).
    pub async fn verify_mark_hash(&self, hash: &str) -> Result<PublicVerifyResponse, AgentError> {
        let url = self.url(&format!("attendance/verify/{hash}"));
        let resp = self
            .client
            .get(&url)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;
        let status = resp.status();
        if status.is_success() || status == StatusCode::NOT_FOUND {
            let res: PublicVerifyResponse = resp.json().await.map_err(AgentError::Reqwest)?;
            Ok(res)
        } else {
            Self::handle_error_response(status, resp).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_api_ingest_body_matches_contract() {
        let ts = Utc.with_ymd_and_hms(2026, 9, 28, 14, 0, 0).unwrap();
        let record = IngestRecord {
            ephemeral_public_key: "b64_dummy_eph_key_44_chars_long_placeholder=",
            encrypted_payload: "b64_dummy_encrypted_payload",
            client_timestamp: ts,
            subcategory: Some("BASIC"),
        };
        let body = IngestBody {
            records: vec![record],
        };

        let json = serde_json::to_string(&body).expect("Serialization failed");
        let val: serde_json::Value = serde_json::from_str(&json).expect("Deserialization failed");

        assert_eq!(val["records"].as_array().unwrap().len(), 1);
        let rec = &val["records"][0];
        assert_eq!(
            rec["ephemeral_public_key"],
            "b64_dummy_eph_key_44_chars_long_placeholder="
        );
        assert_eq!(rec["encrypted_payload"], "b64_dummy_encrypted_payload");
        assert_eq!(rec["client_timestamp"], "2026-09-28T14:00:00Z");
        assert_eq!(rec["subcategory"], "BASIC");
    }

    #[tokio::test]
    async fn test_api_unwraps_data_envelope_and_parses_error_reason() {
        let server = MockServer::start().await;

        // Mock success with SingleResponse envelope
        Mock::given(method("POST"))
            .and(path("/telemetry/system"))
            .and(header(APP_VERSION_HEADER, APP_VERSION))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {
                    "batch_id": "01912345-6789-7abc-def0-123456789abc",
                    "records_created": 1,
                    "category": "system",
                    "server_timestamp": "2026-09-28T14:00:01Z"
                }
            })))
            .mount(&server)
            .await;

        let client = ApiClient::new(server.uri()).expect("Failed to create client");
        let ts = Utc::now();
        let records = vec![IngestRecord {
            ephemeral_public_key: "dummy_key",
            encrypted_payload: "dummy_payload",
            client_timestamp: ts,
            subcategory: None,
        }];

        let res = client.ingest("system", &records, Some("valid_token")).await;
        assert!(res.is_ok());
        let res = res.unwrap();
        assert_eq!(res.category, "system");
        assert_eq!(res.records_created, 1);

        // Mock 403 Forbidden with domain error reason
        Mock::given(method("POST"))
            .and(path("/telemetry/productivity"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": {
                    "code": "FORBIDDEN",
                    "message": "Consent not granted for category",
                    "details": {
                        "reason": "CONSENT_NOT_GRANTED"
                    }
                }
            })))
            .mount(&server)
            .await;

        let err_res = client
            .ingest("productivity", &records, Some("valid_token"))
            .await;
        assert!(err_res.is_err());
        match err_res.unwrap_err() {
            AgentError::Api {
                status,
                code,
                reason,
                ..
            } => {
                assert_eq!(status, 403);
                assert_eq!(code, "FORBIDDEN");
                assert_eq!(reason.as_deref(), Some("CONSENT_NOT_GRANTED"));
            }
            other => panic!("Unexpected error: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_api_client_data_request_and_deletion() {
        let server = MockServer::start().await;

        // Mock 202 Accepted on POST /workers/me/data-request
        Mock::given(method("POST"))
            .and(path("/workers/me/data-request"))
            .respond_with(ResponseTemplate::new(202).set_body_json(serde_json::json!({
                "data": {
                    "request_id": "REQ-123456",
                    "status": "PENDING",
                    "estimated_completion": "2026-09-29T10:00:00Z"
                }
            })))
            .mount(&server)
            .await;

        // Mock 202 Accepted on POST /workers/me/data-deletion
        Mock::given(method("POST"))
            .and(path("/workers/me/data-deletion"))
            .respond_with(ResponseTemplate::new(202).set_body_json(serde_json::json!({
                "data": {
                    "request_id": "DEL-789012",
                    "status": "QUEUED",
                    "message": "Solicitud de eliminación registrada"
                }
            })))
            .mount(&server)
            .await;

        let client = ApiClient::new(server.uri()).expect("Failed to create client");

        let req_res = client
            .post_data_request("worker_token", &["system", "network"])
            .await;
        assert!(req_res.is_ok());
        let req_data = req_res.unwrap();
        assert_eq!(req_data.request_id, "REQ-123456");
        assert_eq!(req_data.status, "PENDING");

        let del_res = client
            .post_data_deletion("worker_token", Some("Derecho al olvido"))
            .await;
        assert!(del_res.is_ok());
        let del_data = del_res.unwrap();
        assert_eq!(del_data.request_id, "DEL-789012");
        assert_eq!(del_data.status, "QUEUED");
    }

    #[tokio::test]
    async fn test_verify_mark_hash_wiremock() -> Result<(), Box<dyn std::error::Error>> {
        let server = MockServer::start().await;
        let client = ApiClient::new(server.uri())?;

        Mock::given(method("GET"))
            .and(path(
                "/attendance/verify/e3bb5518ebb00d41e6e0d071a5c220228ba832f1510d6f190a127e448f68c52c",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "valid": true,
                "type": "WORKDAY_START",
                "effective_date": "20/10/26",
                "effective_time": "01:08:04",
                "employer_legal_name": "Acme SpA",
                "workplace_name": "Casa Matriz",
                "offline": false,
                "corrected": false
            })))
            .mount(&server)
            .await;

        let res = client
            .verify_mark_hash("e3bb5518ebb00d41e6e0d071a5c220228ba832f1510d6f190a127e448f68c52c")
            .await?;
        assert!(res.valid);
        assert_eq!(res.r#type, Some(agent_ipc::MarkType::WorkdayStart));
        assert_eq!(res.employer_legal_name.as_deref(), Some("Acme SpA"));
        Ok(())
    }
}
