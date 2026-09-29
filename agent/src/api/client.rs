use crate::errors::AgentError;
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_VERSION_HEADER: &str = "X-App-Version";

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

#[derive(Clone, Debug)]
pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    pub fn new(base_url: String) -> Result<Self, AgentError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
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

    async fn handle_error_response<T>(
        status: StatusCode,
        resp: reqwest::Response,
    ) -> Result<T, AgentError> {
        if status == StatusCode::UNAUTHORIZED {
            return Err(AgentError::Unauthorized);
        }

        let status_code = status.as_u16();
        if let Ok(err_resp) = resp.json::<ApiErrorResponse>().await {
            let reason = err_resp
                .error
                .details
                .as_ref()
                .and_then(|d| d.get("reason").and_then(|r| r.as_str().map(String::from)));
            Err(AgentError::Api {
                status: status_code,
                code: err_resp.error.code,
                reason,
            })
        } else {
            Err(AgentError::ServerError(status_code))
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
            } => {
                assert_eq!(status, 403);
                assert_eq!(code, "FORBIDDEN");
                assert_eq!(reason.as_deref(), Some("CONSENT_NOT_GRANTED"));
            }
            other => panic!("Unexpected error: {:?}", other),
        }
    }
}
