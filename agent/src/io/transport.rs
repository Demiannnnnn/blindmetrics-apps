use crate::api::client::{ApiErrorResponse, SingleResponse, APP_VERSION, APP_VERSION_HEADER};
use crate::config::WorkerConfig;
use crate::errors::AgentError;
use base64::prelude::*;
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use rusqlite::{params, Connection, Result as SqlResult};
use serde_json::json;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct QueueItem {
    pub id: i64,
    pub category: String,
    pub ephemeral_public_key: String,
    pub encrypted_payload: Vec<u8>,
    pub client_timestamp: DateTime<Utc>,
    pub subcategory: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum IngestOutcome {
    Delivered,
    DroppedForbidden(Option<String>),
    DroppedValidation(String),
    NeedsQueue(String),
}

pub struct OfflineQueue {
    conn: Connection,
}

impl OfflineQueue {
    pub fn open<P: AsRef<Path>>(path: P) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        Self::init_tables(conn)
    }

    pub fn open_in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init_tables(conn)
    }

    fn init_tables(conn: Connection) -> SqlResult<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS offline_queue (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 category TEXT NOT NULL,
                 ephemeral_public_key TEXT NOT NULL,
                 encrypted_payload BLOB NOT NULL,
                 created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                 retry_count INTEGER DEFAULT 0,
                 client_timestamp TEXT,
                 subcategory TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_offline_queue_created ON offline_queue(created_at);",
        )?;

        // Idempotent migrations for existing databases
        let _ = conn.execute(
            "ALTER TABLE offline_queue ADD COLUMN client_timestamp TEXT",
            [],
        );
        let _ = conn.execute("ALTER TABLE offline_queue ADD COLUMN subcategory TEXT", []);

        Ok(Self { conn })
    }

    pub fn enqueue(
        &mut self,
        category: &str,
        eph_pub: &str,
        payload: &[u8],
        client_timestamp: DateTime<Utc>,
        subcategory: Option<&str>,
    ) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO offline_queue (category, ephemeral_public_key, encrypted_payload, client_timestamp, subcategory) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![category, eph_pub, payload, client_timestamp.to_rfc3339(), subcategory],
        )?;
        self.enforce_retention()?;
        Ok(())
    }

    pub fn peek_batch(&self, limit: usize) -> SqlResult<Vec<QueueItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, category, ephemeral_public_key, encrypted_payload, client_timestamp, subcategory FROM offline_queue ORDER BY id ASC LIMIT ?1"
        )?;
        let items = stmt
            .query_map(params![limit as i64], |row| {
                let ts_str: Option<String> = row.get(4)?;
                let client_timestamp = ts_str
                    .and_then(|s| {
                        DateTime::parse_from_rfc3339(&s)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                    })
                    .unwrap_or_else(Utc::now);

                Ok(QueueItem {
                    id: row.get(0)?,
                    category: row.get(1)?,
                    ephemeral_public_key: row.get(2)?,
                    encrypted_payload: row.get(3)?,
                    client_timestamp,
                    subcategory: row.get(5)?,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(items)
    }

    pub fn mark_delivered(&mut self, ids: &[i64]) -> SqlResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
        let query = format!(
            "DELETE FROM offline_queue WHERE id IN ({})",
            placeholders.join(",")
        );
        let mut stmt = self.conn.prepare(&query)?;
        let params_iter = ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect::<Vec<_>>();
        stmt.execute(rusqlite::params_from_iter(params_iter))?;
        Ok(())
    }

    pub fn count(&self) -> SqlResult<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM offline_queue", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    fn enforce_retention(&mut self) -> SqlResult<()> {
        // Discard entries older than 7 days
        self.conn.execute(
            "DELETE FROM offline_queue WHERE created_at < datetime('now', '-7 days')",
            [],
        )?;
        Ok(())
    }
}

pub struct TelemetryTransport {
    client: Client,
    pub config: WorkerConfig,
}

impl TelemetryTransport {
    pub fn new(config: WorkerConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client, config }
    }

    /// Dispatches a single record to the proper API endpoint.
    /// Implements RFC error semantics:
    /// - 201: Delivered
    /// - 401: Refresh token once, then retry
    /// - 403: Drop envelope without queuing
    /// - 400 / 413: Discard invalid body
    /// - 429 / 5xx / Network error: Needs queuing in SQLite
    pub async fn send_record(
        &mut self,
        endpoint: &str,
        eph_public_key: &str,
        encrypted_payload: &[u8],
        client_timestamp: DateTime<Utc>,
        subcategory: Option<&str>,
    ) -> IngestOutcome {
        let b64_payload = BASE64_STANDARD.encode(encrypted_payload);
        let mut record_val = json!({
            "ephemeral_public_key": eph_public_key,
            "encrypted_payload": b64_payload,
            "client_timestamp": client_timestamp.to_rfc3339()
        });
        if let Some(sub) = subcategory {
            record_val["subcategory"] = json!(sub);
        }

        let req_body = json!({ "records": [record_val] });
        let url = format!(
            "{}/telemetry/{}",
            self.config.api_base_url.trim_end_matches('/'),
            endpoint
        );

        let token = self.config.access_token.as_deref().unwrap_or("");
        let mut request = self
            .client
            .post(&url)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(&req_body);
        if !token.is_empty() {
            request = request.bearer_auth(token);
        }

        let response = match request.send().await {
            Ok(resp) => resp,
            Err(err) => {
                tracing::warn!("ingest queued: network error: {:?}", err);
                return IngestOutcome::NeedsQueue(err.to_string());
            }
        };

        let status = response.status();
        if status.is_success() {
            tracing::info!("ingest ok category={} records=1", endpoint);
            return IngestOutcome::Delivered;
        }

        if status == StatusCode::UNAUTHORIZED {
            tracing::info!("Access token expired, attempting refresh...");
            if self.refresh_access_token().await.is_ok() {
                let new_token = self.config.access_token.as_deref().unwrap_or("");
                let mut retry_req = self
                    .client
                    .post(&url)
                    .header(APP_VERSION_HEADER, APP_VERSION)
                    .json(&req_body);
                if !new_token.is_empty() {
                    retry_req = retry_req.bearer_auth(new_token);
                }

                if let Ok(retry_resp) = retry_req.send().await {
                    if retry_resp.status().is_success() {
                        tracing::info!("ingest ok after token refresh category={}", endpoint);
                        return IngestOutcome::Delivered;
                    }
                    if retry_resp.status() == StatusCode::FORBIDDEN {
                        return Self::parse_forbidden(endpoint, retry_resp).await;
                    }
                }
            }
            return IngestOutcome::NeedsQueue("401 Unauthorized after refresh attempt".into());
        }

        if status == StatusCode::FORBIDDEN {
            return Self::parse_forbidden(endpoint, response).await;
        }

        if status == StatusCode::BAD_REQUEST || status == StatusCode::PAYLOAD_TOO_LARGE {
            tracing::error!("ingest dropped: {} category={}", status, endpoint);
            return IngestOutcome::DroppedValidation(status.to_string());
        }

        // 429, 5xx, or other temporary failure
        tracing::warn!("ingest queued: status={} category={}", status, endpoint);
        IngestOutcome::NeedsQueue(format!("Status {}", status))
    }

    async fn parse_forbidden(endpoint: &str, resp: reqwest::Response) -> IngestOutcome {
        let reason = if let Ok(err_obj) = resp.json::<ApiErrorResponse>().await {
            err_obj
                .error
                .details
                .and_then(|d| d.get("reason").and_then(|r| r.as_str().map(String::from)))
        } else {
            None
        };
        tracing::warn!(
            "ingest dropped: 403 reason={} category={}",
            reason.as_deref().unwrap_or("UNKNOWN"),
            endpoint
        );
        IngestOutcome::DroppedForbidden(reason)
    }

    pub async fn refresh_access_token(&mut self) -> Result<(), AgentError> {
        let refresh_token = match self.config.refresh_token.as_deref() {
            Some(t) if !t.is_empty() => t,
            _ => return Err(AgentError::Unauthorized),
        };

        let url = format!(
            "{}/auth/refresh",
            self.config.api_base_url.trim_end_matches('/')
        );
        let res = self
            .client
            .post(&url)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(&json!({ "refresh_token": refresh_token }))
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;

        if res.status().is_success() {
            let body: SingleResponse<serde_json::Value> =
                res.json().await.map_err(AgentError::Reqwest)?;

            if let Some(at) = body.data["access_token"].as_str() {
                self.config.access_token = Some(at.to_string());
            }
            if let Some(rt) = body.data["refresh_token"].as_str() {
                self.config.refresh_token = Some(rt.to_string());
            }
            self.config.save().ok();
            Ok(())
        } else {
            Err(AgentError::Unauthorized)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{bearer_token, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_offline_queue_fifo_order_and_deletion() {
        let mut queue = OfflineQueue::open_in_memory().expect("Open in-memory SQLite failed");
        let ts = Utc::now();

        queue
            .enqueue("system", "key1", b"payload1", ts, None)
            .unwrap();
        queue
            .enqueue("productivity", "key2", b"payload2", ts, Some("BASIC"))
            .unwrap();
        assert_eq!(queue.count().unwrap(), 2);

        let batch = queue.peek_batch(1).unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].category, "system");
        assert_eq!(batch[0].ephemeral_public_key, "key1");
        assert_eq!(batch[0].subcategory, None);

        queue.mark_delivered(&[batch[0].id]).unwrap();
        assert_eq!(queue.count().unwrap(), 1);

        let remaining = queue.peek_batch(10).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].category, "productivity");
        assert_eq!(remaining[0].subcategory.as_deref(), Some("BASIC"));
    }

    #[tokio::test]
    async fn test_transport_403_drops_envelope_not_queued() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/telemetry/location"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                "error": {
                    "code": "FORBIDDEN",
                    "message": "Consent not granted for category",
                    "details": { "reason": "CONSENT_NOT_GRANTED" }
                }
            })))
            .mount(&server)
            .await;

        let mut config = WorkerConfig::load_or_init();
        config.api_base_url = server.uri();
        config.access_token = Some("test_token".into());

        let mut transport = TelemetryTransport::new(config);
        let outcome = transport
            .send_record("location", "dummy_key", b"test_payload", Utc::now(), None)
            .await;

        assert_eq!(
            outcome,
            IngestOutcome::DroppedForbidden(Some("CONSENT_NOT_GRANTED".into()))
        );
    }

    #[tokio::test]
    async fn test_transport_401_refreshes_once_then_retries() {
        let server = MockServer::start().await;

        // First attempt with old token fails 401
        Mock::given(method("POST"))
            .and(path("/telemetry/system"))
            .and(bearer_token("old_token"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": { "code": "UNAUTHORIZED", "message": "Token expired" }
            })))
            .expect(1)
            .mount(&server)
            .await;

        // Refresh call succeeds and returns new tokens inside { "data": ... }
        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "access_token": "new_token",
                    "refresh_token": "new_refresh_token",
                    "token_type": "Bearer",
                    "expires_in": 900
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        // Second attempt with new token succeeds 201
        Mock::given(method("POST"))
            .and(path("/telemetry/system"))
            .and(bearer_token("new_token"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "data": {
                    "batch_id": "01912345-6789-7abc-def0-123456789abc",
                    "records_created": 1,
                    "category": "system",
                    "server_timestamp": "2026-09-28T14:00:00Z"
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let mut config = WorkerConfig::load_or_init();
        config.api_base_url = server.uri();
        config.access_token = Some("old_token".into());
        config.refresh_token = Some("initial_refresh_token".into());

        let mut transport = TelemetryTransport::new(config);
        let outcome = transport
            .send_record("system", "dummy_key", b"test_payload", Utc::now(), None)
            .await;

        assert_eq!(outcome, IngestOutcome::Delivered);
        assert_eq!(transport.config.access_token.as_deref(), Some("new_token"));
        assert_eq!(
            transport.config.refresh_token.as_deref(),
            Some("new_refresh_token")
        );
    }
}
