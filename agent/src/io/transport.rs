use base64::prelude::*;
use reqwest::{Client, StatusCode};
use rusqlite::{params, Connection, Result as SqlResult};
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use crate::config::WorkerConfig;
use crate::errors::AgentError;

pub struct QueueItem {
    pub id: i64,
    pub category: String,
    pub ephemeral_public_key: String,
    pub encrypted_payload: Vec<u8>,
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
                 retry_count INTEGER DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS idx_offline_queue_created ON offline_queue(created_at);"
        )?;
        Ok(Self { conn })
    }

    pub fn enqueue(&mut self, category: &str, eph_pub: &str, payload: &[u8]) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO offline_queue (category, ephemeral_public_key, encrypted_payload) VALUES (?1, ?2, ?3)",
            params![category, eph_pub, payload],
        )?;
        self.enforce_retention()?;
        Ok(())
    }

    pub fn peek_batch(&self, limit: usize) -> SqlResult<Vec<QueueItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, category, ephemeral_public_key, encrypted_payload FROM offline_queue ORDER BY id ASC LIMIT ?1"
        )?;
        let items = stmt.query_map(params![limit as i64], |row| {
            Ok(QueueItem {
                id: row.get(0)?,
                category: row.get(1)?,
                ephemeral_public_key: row.get(2)?,
                encrypted_payload: row.get(3)?,
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(items)
    }

    pub fn mark_delivered(&mut self, ids: &[i64]) -> SqlResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let id_list: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
        let query = format!("DELETE FROM offline_queue WHERE id IN ({})", id_list.join(","));
        self.conn.execute(&query, [])?;
        Ok(())
    }

    pub fn count(&self) -> SqlResult<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM offline_queue",
            [],
            |r| r.get(0),
        )?;
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
    config: WorkerConfig,
}

impl TelemetryTransport {
    pub fn new(config: WorkerConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");
        Self { client, config }
    }

    /// Dispatches an encrypted batch to the API. Handles 401 transparent token renewal.
    pub async fn send_envelope(
        &mut self,
        category: &str,
        eph_public_key: &str,
        encrypted_payload: &[u8],
    ) -> Result<(), AgentError> {
        let url = format!("{}/telemetry/{}", self.config.api_base_url, category);
        let b64_payload = BASE64_STANDARD.encode(encrypted_payload);

        let req_body = json!({
            "ephemeral_public_key": eph_public_key,
            "encrypted_payload": b64_payload
        });

        let token = self.config.access_token.as_deref().unwrap_or("");
        let response = self.client.post(&url)
            .bearer_auth(token)
            .json(&req_body)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;

        if response.status() == StatusCode::UNAUTHORIZED {
            self.refresh_access_token().await?;
            let new_token = self.config.access_token.as_deref().unwrap_or("");
            let retry = self.client.post(&url)
                .bearer_auth(new_token)
                .json(&req_body)
                .send()
                .await
                .map_err(|e| AgentError::Network(e.to_string()))?;

            if !retry.status().is_success() {
                return Err(AgentError::ServerError(retry.status().as_u16()));
            }
        } else if !response.status().is_success() {
            return Err(AgentError::ServerError(response.status().as_u16()));
        }

        Ok(())
    }

    async fn refresh_access_token(&mut self) -> Result<(), AgentError> {
        let refresh_token = self.config.refresh_token.as_deref().ok_or(AgentError::Unauthorized)?;
        let url = format!("{}/auth/refresh", self.config.api_base_url);

        let res = self.client.post(&url)
            .json(&json!({ "refresh_token": refresh_token }))
            .send().await
            .map_err(|e| AgentError::Network(e.to_string()))?;

        if res.status().is_success() {
            let body: serde_json::Value = res.json().await?;
            if let Some(at) = body["access_token"].as_str() {
                self.config.access_token = Some(at.to_string());
            }
            if let Some(rt) = body["refresh_token"].as_str() {
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

    #[test]
    fn test_offline_queue_fifo_order_and_deletion() {
        let mut queue = OfflineQueue::open_in_memory().expect("Open in-memory SQLite failed");

        queue.enqueue("system", "key1", b"payload1").unwrap();
        queue.enqueue("network", "key2", b"payload2").unwrap();
        assert_eq!(queue.count().unwrap(), 2);

        let batch = queue.peek_batch(1).unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].category, "system");
        assert_eq!(batch[0].ephemeral_public_key, "key1");

        queue.mark_delivered(&[batch[0].id]).unwrap();
        assert_eq!(queue.count().unwrap(), 1);

        let remaining = queue.peek_batch(10).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].category, "network");
    }
}
