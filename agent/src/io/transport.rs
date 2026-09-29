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

pub const MAX_PAYLOAD_SIZE_BYTES: usize = 5 * 1024 * 1024; // 5 MB per payload
pub const MAX_QUEUE_CAPACITY_BYTES: i64 = 100 * 1024 * 1024; // 100 MB max queue capacity
pub const MAX_RETRIES_BEFORE_QUARANTINE: i64 = 10; // Max 10 attempts

#[derive(Clone, Debug)]
pub struct BackoffPolicy {
    pub base: Duration,
    pub max: Duration,
    pub multiplier: f64,
    pub jitter_pct: f64,
    pub consecutive_failures: u32,
}

impl Default for BackoffPolicy {
    fn default() -> Self {
        Self {
            base: Duration::from_secs(2),
            max: Duration::from_secs(300),
            multiplier: 2.0,
            jitter_pct: 0.20,
            consecutive_failures: 0,
        }
    }
}

impl BackoffPolicy {
    pub fn new(base: Duration, max: Duration) -> Self {
        Self {
            base,
            max,
            multiplier: 2.0,
            jitter_pct: 0.20,
            consecutive_failures: 0,
        }
    }

    pub fn next_backoff(&mut self) -> Duration {
        let failures = self.consecutive_failures;
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);

        let base_secs = self.base.as_secs_f64();
        let max_secs = self.max.as_secs_f64();

        // Exponential backoff: base * 2^failures capped at max (RF-AGENT-05-4)
        let exp_secs = (base_secs * self.multiplier.powi(failures as i32)).min(max_secs);

        // Jitter factor in [-jitter_pct, +jitter_pct] (RF-AGENT-05-4)
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(42);
        let rand_factor = (((nanos % 4001) as f64 / 10000.0) - self.jitter_pct)
            .clamp(-self.jitter_pct, self.jitter_pct);
        let jittered_secs = (exp_secs * (1.0 + rand_factor)).max(0.1);

        Duration::from_secs_f64(jittered_secs)
    }

    pub fn reset(&mut self) {
        self.consecutive_failures = 0;
    }

    pub fn failures(&self) -> u32 {
        self.consecutive_failures
    }
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
        if payload.len() > MAX_PAYLOAD_SIZE_BYTES {
            return Err(rusqlite::Error::ToSqlConversionFailure(
                format!(
                    "Payload exceeds maximum allowed size of 5 MB ({})",
                    payload.len()
                )
                .into(),
            ));
        }

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

    pub fn increment_retries(&mut self, ids: &[i64]) -> SqlResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
        let query = format!(
            "UPDATE offline_queue SET retry_count = retry_count + 1 WHERE id IN ({})",
            placeholders.join(",")
        );
        let mut stmt = self.conn.prepare(&query)?;
        let params_iter = ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect::<Vec<_>>();
        stmt.execute(rusqlite::params_from_iter(params_iter))?;

        // Quarantine packets that reached 10 attempts (RF-AGENT-05-1 & Section 5)
        self.conn.execute(
            "DELETE FROM offline_queue WHERE retry_count >= ?1",
            params![MAX_RETRIES_BEFORE_QUARANTINE],
        )?;
        Ok(())
    }

    pub fn count(&self) -> SqlResult<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM offline_queue", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    /// Purges all queued envelopes upon consent withdrawal or revocation (DP-09).
    pub fn clear(&mut self) -> SqlResult<()> {
        self.conn.execute("DELETE FROM offline_queue", [])?;
        Ok(())
    }

    pub fn enforce_retention(&mut self) -> SqlResult<()> {
        // Discard entries older than 7 days (RF-AGENT-05-1)
        self.conn.execute(
            "DELETE FROM offline_queue WHERE created_at < datetime('now', '-7 days')",
            [],
        )?;

        // Quarantine records exceeding max retries
        self.conn.execute(
            "DELETE FROM offline_queue WHERE retry_count >= ?1",
            params![MAX_RETRIES_BEFORE_QUARANTINE],
        )?;

        // Cap to 100 MB or 50,000 entries (FIFO eviction)
        let total_bytes: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(length(encrypted_payload)), 0) FROM offline_queue",
            [],
            |r| r.get(0),
        )?;

        if total_bytes > MAX_QUEUE_CAPACITY_BYTES {
            self.conn.execute(
                "DELETE FROM offline_queue WHERE id IN (SELECT id FROM offline_queue ORDER BY id ASC LIMIT (SELECT MAX(1, COUNT(*) / 10) FROM offline_queue))",
                [],
            )?;
        }

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
            if res.status() == StatusCode::UNAUTHORIZED {
                // Refresh token invalid or revoked: wipe local session (RF-AGENT-05-3 & Sec 4)
                self.config.access_token = None;
                self.config.refresh_token = None;
                self.config.save().ok();
            }
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

    #[test]
    fn test_sqlite_queue_fifo_three_items() {
        let mut queue = OfflineQueue::open_in_memory().expect("Open in-memory SQLite failed");
        let ts = Utc::now();

        queue
            .enqueue("system", "key_A", b"payload_A", ts, None)
            .unwrap();
        queue
            .enqueue("network", "key_B", b"payload_B", ts, None)
            .unwrap();
        queue
            .enqueue("files", "key_C", b"payload_C", ts, None)
            .unwrap();

        assert_eq!(queue.count().unwrap(), 3);

        let batch = queue.peek_batch(2).unwrap();
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].category, "system");
        assert_eq!(batch[0].ephemeral_public_key, "key_A");
        assert_eq!(batch[1].category, "network");
        assert_eq!(batch[1].ephemeral_public_key, "key_B");
    }

    #[test]
    fn test_sqlite_delivered_deletion() {
        let mut queue = OfflineQueue::open_in_memory().unwrap();
        let ts = Utc::now();

        queue.enqueue("system", "key1", b"p1", ts, None).unwrap();
        queue.enqueue("system", "key2", b"p2", ts, None).unwrap();
        assert_eq!(queue.count().unwrap(), 2);

        let items = queue.peek_batch(10).unwrap();
        let id_1 = items[0].id;
        let id_2 = items[1].id;

        queue.mark_delivered(&[id_1]).unwrap();
        assert_eq!(queue.count().unwrap(), 1);

        let remaining = queue.peek_batch(10).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, id_2);

        queue.mark_delivered(&[id_2]).unwrap();
        assert_eq!(queue.count().unwrap(), 0);
    }

    #[test]
    fn test_retention_policy_purges_old_records() {
        let mut queue = OfflineQueue::open_in_memory().unwrap();

        // Insert record artificially created 8 days ago
        queue.conn.execute(
            "INSERT INTO offline_queue (category, ephemeral_public_key, encrypted_payload, created_at)
             VALUES ('system', 'old_key', X'010203', datetime('now', '-8 days'))",
            [],
        ).unwrap();

        // Insert record artificially created 2 days ago
        queue.conn.execute(
            "INSERT INTO offline_queue (category, ephemeral_public_key, encrypted_payload, created_at)
             VALUES ('system', 'recent_key', X'040506', datetime('now', '-2 days'))",
            [],
        ).unwrap();

        assert_eq!(queue.count().unwrap(), 2);

        queue.enforce_retention().unwrap();

        assert_eq!(queue.count().unwrap(), 1);
        let items = queue.peek_batch(10).unwrap();
        assert_eq!(items[0].ephemeral_public_key, "recent_key");
    }

    #[test]
    fn test_payload_size_limit_and_quarantine() {
        let mut queue = OfflineQueue::open_in_memory().unwrap();
        let ts = Utc::now();

        // 1. Attempt payload exceeding 5 MB limit
        let huge_payload = vec![0u8; MAX_PAYLOAD_SIZE_BYTES + 10];
        let res = queue.enqueue("system", "key", &huge_payload, ts, None);
        assert!(res.is_err(), "Payload > 5MB must be rejected");

        // 2. Enqueue valid payload and verify 10 retries quarantine
        let valid_payload = vec![1u8, 2, 3];
        queue
            .enqueue("system", "key_retries", &valid_payload, ts, None)
            .unwrap();
        let item = queue.peek_batch(1).unwrap();
        let id = item[0].id;

        // Increment 9 times
        for _ in 0..9 {
            queue.increment_retries(&[id]).unwrap();
        }
        assert_eq!(
            queue.count().unwrap(),
            1,
            "Should still be present at 9 retries"
        );

        // 10th retry -> triggers quarantine / deletion
        queue.increment_retries(&[id]).unwrap();
        assert_eq!(
            queue.count().unwrap(),
            0,
            "Record reaching 10 retries must be quarantined/purged"
        );
    }

    #[test]
    fn test_backoff_exponential_with_jitter() {
        let mut backoff = BackoffPolicy::new(Duration::from_secs(2), Duration::from_secs(300));
        assert_eq!(backoff.failures(), 0);

        // Failure 0: base 2s * 2^0 = 2s, +/- 20% jitter -> [1.6s, 2.4s]
        let b0 = backoff.next_backoff();
        assert!(b0 >= Duration::from_millis(1500) && b0 <= Duration::from_millis(2500));
        assert_eq!(backoff.failures(), 1);

        // Failure 1: base 2s * 2^1 = 4s, +/- 20% jitter -> [3.2s, 4.8s]
        let b1 = backoff.next_backoff();
        assert!(b1 >= Duration::from_millis(3000) && b1 <= Duration::from_millis(5000));
        assert_eq!(backoff.failures(), 2);

        // Failure 2: base 2s * 2^2 = 8s, +/- 20% jitter -> [6.4s, 9.6s]
        let b2 = backoff.next_backoff();
        assert!(b2 >= Duration::from_millis(6000) && b2 <= Duration::from_millis(10000));

        // Advance to high failure count to test cap
        for _ in 0..10 {
            backoff.next_backoff();
        }
        let capped = backoff.next_backoff();
        // Capped at 300s +/- 20% jitter -> [240s, 360s]
        assert!(capped >= Duration::from_secs(230) && capped <= Duration::from_secs(370));

        backoff.reset();
        assert_eq!(backoff.failures(), 0);
        let b_reset = backoff.next_backoff();
        assert!(b_reset >= Duration::from_millis(1500) && b_reset <= Duration::from_millis(2500));
    }
}
