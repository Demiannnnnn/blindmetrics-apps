//! Local mark journal and offline queue: table `marks` in `offline.db` (RF-AGENT-12-9).
//!
//! Every mark is written here **before** it is sent (ATT-04), so a crash, a network failure or a
//! revocation never loses it; the worker sees pending and sent marks offline (ATT-06). The table
//! is plain (attendance is not secret, SEC-21) and is independent of the telemetry queue: the
//! DP-09 purge (`OfflineQueue::clear`) never touches it. Retention: 5 years or until unlink.
//!
//! The same database also keeps `attendance_notices`, the keys of local notices already shown
//! ("once per mark and day"), so a restart does not repeat them.

use agent_ipc::{LocalMarkStatus, MarkLocation, MarkRecord, MarkType, ServerMark};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use std::path::Path;

/// Marks older than this are purged (R38 arts. 22.1, 53 f).
pub const RETENTION_YEARS: i64 = 5;

/// Columns of `marks` (name, SQL type) for the idempotent migration.
const MARK_COLUMNS: [(&str, &str); 16] = [
    ("client_mark_id", "TEXT PRIMARY KEY"),
    ("worker_id", "TEXT"),
    ("type", "TEXT NOT NULL DEFAULT ''"),
    ("device_ts", "TEXT NOT NULL DEFAULT ''"),
    ("offline", "INTEGER NOT NULL DEFAULT 0"),
    ("status", "TEXT NOT NULL DEFAULT 'PENDING'"),
    ("server_mark_id", "TEXT"),
    ("server_ts", "TEXT"),
    ("hash", "TEXT"),
    ("prev_hash", "TEXT"),
    ("rejection_reason", "TEXT"),
    ("payload_json", "TEXT"),
    ("location_json", "TEXT"),
    ("duplicate_of", "TEXT"),
    ("attempts", "INTEGER NOT NULL DEFAULT 0"),
    ("created_at", "TEXT"),
];

/// A journal row: the IPC view plus what is needed to (re)send it.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredMark {
    pub record: MarkRecord,
    pub worker_id: Option<String>,
    pub location: Option<MarkLocation>,
    pub attempts: u32,
}

/// Owner of the `marks` and `attendance_notices` tables.
pub struct MarkJournal {
    conn: Connection,
}

fn status_str(status: LocalMarkStatus) -> &'static str {
    match status {
        LocalMarkStatus::Pending => "PENDING",
        LocalMarkStatus::Confirmed => "CONFIRMED",
        LocalMarkStatus::HashMismatch => "HASH_MISMATCH",
        LocalMarkStatus::Rejected => "REJECTED",
    }
}

fn parse_status(raw: &str) -> LocalMarkStatus {
    match raw {
        "CONFIRMED" => LocalMarkStatus::Confirmed,
        "HASH_MISMATCH" => LocalMarkStatus::HashMismatch,
        "REJECTED" => LocalMarkStatus::Rejected,
        // Unknown values are treated as not yet delivered: never silently dropped (ATT-04).
        _ => LocalMarkStatus::Pending,
    }
}

fn type_str(t: MarkType) -> &'static str {
    match t {
        MarkType::WorkdayStart => "WORKDAY_START",
        MarkType::WorkdayEnd => "WORKDAY_END",
        MarkType::BreakStart => "BREAK_START",
        MarkType::BreakEnd => "BREAK_END",
    }
}

fn parse_type(raw: &str) -> Option<MarkType> {
    match raw {
        "WORKDAY_START" => Some(MarkType::WorkdayStart),
        "WORKDAY_END" => Some(MarkType::WorkdayEnd),
        "BREAK_START" => Some(MarkType::BreakStart),
        "BREAK_END" => Some(MarkType::BreakEnd),
        _ => None,
    }
}

const SELECT_COLUMNS: &str = "client_mark_id, worker_id, type, device_ts, offline, status, \
     payload_json, location_json, duplicate_of, rejection_reason, attempts";

fn row_to_stored(row: &rusqlite::Row<'_>) -> SqlResult<Option<StoredMark>> {
    let client_mark_id: String = row.get(0)?;
    let worker_id: Option<String> = row.get(1)?;
    let type_raw: String = row.get(2)?;
    let device_ts: String = row.get(3)?;
    let offline: i64 = row.get(4)?;
    let status_raw: String = row.get(5)?;
    let payload: Option<String> = row.get(6)?;
    let location_raw: Option<String> = row.get(7)?;
    let duplicate_of: Option<String> = row.get(8)?;
    let rejection_reason: Option<String> = row.get(9)?;
    let attempts: i64 = row.get(10)?;

    let Some(r#type) = parse_type(&type_raw) else {
        return Ok(None);
    };
    let server_mark = payload
        .as_deref()
        .and_then(|p| serde_json::from_str::<serde_json::Value>(p).ok())
        .and_then(|v| serde_json::from_value::<ServerMark>(v.get("mark")?.clone()).ok());
    let location = location_raw
        .as_deref()
        .and_then(|l| serde_json::from_str::<MarkLocation>(l).ok());
    Ok(Some(StoredMark {
        record: MarkRecord {
            client_mark_id,
            r#type,
            device_ts,
            offline: offline != 0,
            status: parse_status(&status_raw),
            server_mark,
            duplicate_of,
            rejection_reason,
        },
        worker_id,
        location,
        attempts: attempts.clamp(0, i64::from(u32::MAX)) as u32,
    }))
}

impl MarkJournal {
    pub fn open<P: AsRef<Path>>(path: P) -> SqlResult<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> SqlResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    /// Creates or migrates the tables (idempotent, safe on every start).
    fn init(conn: Connection) -> SqlResult<Self> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS marks (client_mark_id TEXT PRIMARY KEY);
             CREATE TABLE IF NOT EXISTS attendance_notices (
                 key TEXT PRIMARY KEY,
                 created_at TEXT NOT NULL
             );",
        )?;
        let existing: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(marks)")?;
            let names = stmt
                .query_map([], |r| r.get::<_, String>(1))?
                .collect::<SqlResult<Vec<_>>>()?;
            names
        };
        for (name, sql_type) in MARK_COLUMNS {
            if !existing.iter().any(|c| c == name) {
                conn.execute(
                    &format!("ALTER TABLE marks ADD COLUMN {name} {sql_type}"),
                    [],
                )?;
            }
        }
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_marks_status_device_ts ON marks(status, device_ts)",
            [],
        )?;
        Ok(Self { conn })
    }

    /// Stores a new mark as `PENDING` before any network call (ATT-04).
    pub fn insert_pending(
        &self,
        client_mark_id: &str,
        worker_id: Option<&str>,
        r#type: MarkType,
        device_ts: &str,
        created_at: DateTime<Utc>,
    ) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO marks (client_mark_id, worker_id, type, device_ts, offline, status, attempts, created_at)
             VALUES (?1, ?2, ?3, ?4, 0, 'PENDING', 0, ?5)",
            params![
                client_mark_id,
                worker_id,
                type_str(r#type),
                device_ts,
                super::format_ts(created_at)
            ],
        )?;
        Ok(())
    }

    /// Stores the single location reading taken for this mark (ATT-08), re-sent on retries.
    pub fn set_location(&self, client_mark_id: &str, location: &MarkLocation) -> SqlResult<()> {
        let json = serde_json::to_string(location)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.conn.execute(
            "UPDATE marks SET location_json = ?2 WHERE client_mark_id = ?1",
            params![client_mark_id, json],
        )?;
        Ok(())
    }

    /// A send attempt failed: the mark stays `PENDING` and is flagged `offline` from now on.
    pub fn record_failed_attempt(&self, client_mark_id: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE marks SET offline = 1, attempts = attempts + 1 WHERE client_mark_id = ?1",
            params![client_mark_id],
        )?;
        Ok(())
    }

    /// The server accepted the mark: stores its answer and the local verification result.
    pub fn record_server_answer(
        &self,
        client_mark_id: &str,
        status: LocalMarkStatus,
        mark: &ServerMark,
        duplicate_of: Option<&str>,
        payload_json: &str,
    ) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE marks SET status = ?2, server_mark_id = ?3, server_ts = ?4, hash = ?5,
                 prev_hash = ?6, duplicate_of = ?7, payload_json = ?8, attempts = attempts + 1
             WHERE client_mark_id = ?1",
            params![
                client_mark_id,
                status_str(status),
                mark.id,
                mark.server_ts,
                mark.hash,
                mark.prev_hash,
                duplicate_of,
                payload_json
            ],
        )?;
        Ok(())
    }

    /// `400` validation answer: never retried, shown with its reason (ATT-04).
    pub fn record_rejection(&self, client_mark_id: &str, reason: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE marks SET status = 'REJECTED', rejection_reason = ?2, attempts = attempts + 1
             WHERE client_mark_id = ?1",
            params![client_mark_id, reason],
        )?;
        Ok(())
    }

    pub fn get(&self, client_mark_id: &str) -> SqlResult<Option<StoredMark>> {
        let sql = format!("SELECT {SELECT_COLUMNS} FROM marks WHERE client_mark_id = ?1");
        let row = self
            .conn
            .query_row(&sql, params![client_mark_id], row_to_stored)
            .optional()?;
        Ok(row.flatten())
    }

    /// Pending marks of `worker_id` in `device_ts` order (the drain order, RF-AGENT-12-8).
    /// Marks of another worker (an earlier enrollment) are never sent with this session.
    pub fn pending_for_worker(&self, worker_id: &str) -> SqlResult<Vec<StoredMark>> {
        let sql = format!(
            "SELECT {SELECT_COLUMNS} FROM marks WHERE status = 'PENDING' AND worker_id = ?1
             ORDER BY device_ts ASC, created_at ASC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![worker_id], row_to_stored)?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(rows.into_iter().flatten().collect())
    }

    /// Every mark, oldest first, optionally limited to one worker.
    pub fn all(&self, worker_id: Option<&str>) -> SqlResult<Vec<StoredMark>> {
        let rows = match worker_id {
            Some(w) => {
                let sql = format!(
                    "SELECT {SELECT_COLUMNS} FROM marks WHERE worker_id = ?1 ORDER BY device_ts ASC"
                );
                let mut stmt = self.conn.prepare(&sql)?;
                let rows = stmt
                    .query_map(params![w], row_to_stored)?
                    .collect::<SqlResult<Vec<_>>>()?;
                rows
            }
            None => {
                let sql = format!("SELECT {SELECT_COLUMNS} FROM marks ORDER BY device_ts ASC");
                let mut stmt = self.conn.prepare(&sql)?;
                let rows = stmt
                    .query_map([], row_to_stored)?
                    .collect::<SqlResult<Vec<_>>>()?;
                rows
            }
        };
        Ok(rows.into_iter().flatten().collect())
    }

    /// `(pending, pending with offline = 1)` over every worker (diagnostics, banners).
    pub fn pending_counts(&self) -> SqlResult<(usize, usize)> {
        self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(offline), 0) FROM marks WHERE status = 'PENDING'",
            [],
            |r| {
                let total: i64 = r.get(0)?;
                let offline: i64 = r.get(1)?;
                Ok((total.max(0) as usize, offline.max(0) as usize))
            },
        )
    }

    /// Explicit unlink: the journal goes with the credentials ("hasta desvincular").
    pub fn clear(&self) -> SqlResult<()> {
        self.conn.execute("DELETE FROM marks", [])?;
        self.conn.execute("DELETE FROM attendance_notices", [])?;
        Ok(())
    }

    /// Purges delivered marks older than 5 years and notices older than 60 days. Pending marks
    /// are never purged by age (ATT-04).
    pub fn enforce_retention(&self, now: DateTime<Utc>) -> SqlResult<()> {
        let cutoff = super::format_ts(now - Duration::days(365 * RETENTION_YEARS + 1));
        self.conn.execute(
            "DELETE FROM marks WHERE status <> 'PENDING' AND device_ts < ?1",
            params![cutoff],
        )?;
        let notice_cutoff = super::format_ts(now - Duration::days(60));
        self.conn.execute(
            "DELETE FROM attendance_notices WHERE created_at < ?1",
            params![notice_cutoff],
        )?;
        Ok(())
    }

    /// True when the notice key was already recorded.
    pub fn notice_seen(&self, key: &str) -> SqlResult<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM attendance_notices WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Records a notice key; `true` only the first time (so it is shown once).
    pub fn notice_once(&self, key: &str, now: DateTime<Utc>) -> SqlResult<bool> {
        let inserted = self.conn.execute(
            "INSERT OR IGNORE INTO attendance_notices (key, created_at) VALUES (?1, ?2)",
            params![key, super::format_ts(now)],
        )?;
        Ok(inserted == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 11, h, m, 0).unwrap()
    }

    fn server_mark(id: &str) -> ServerMark {
        serde_json::from_value(serde_json::json!({
            "id": id, "worker_id": "w", "type": "WORKDAY_START",
            "server_ts": "2026-10-11T12:00:05Z", "device_ts": "2026-10-11T12:00:04Z",
            "offline": false, "source": "AGENT", "hash": "h", "prev_hash": "p"
        }))
        .unwrap()
    }

    #[test]
    fn test_journal_migration_is_idempotent_and_survives_old_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("offline.db");
        {
            // A database created by an older build with fewer columns.
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE marks (client_mark_id TEXT PRIMARY KEY, type TEXT NOT NULL, device_ts TEXT NOT NULL, status TEXT NOT NULL);
                 INSERT INTO marks VALUES ('m-old', 'WORKDAY_START', '2026-10-10T09:00:00Z', 'PENDING');",
            )
            .unwrap();
        }
        let j = MarkJournal::open(&path).unwrap();
        drop(j);
        let j = MarkJournal::open(&path).unwrap();
        let old = j.get("m-old").unwrap().unwrap();
        assert_eq!(old.record.status, LocalMarkStatus::Pending);
        assert!(!old.record.offline);
    }

    #[test]
    fn test_journal_pending_order_offline_flag_and_answer() {
        let j = MarkJournal::open_in_memory().unwrap();
        j.insert_pending(
            "b",
            Some("w"),
            MarkType::BreakStart,
            "2026-10-11T13:00:00Z",
            t(13, 0),
        )
        .unwrap();
        j.insert_pending(
            "a",
            Some("w"),
            MarkType::WorkdayStart,
            "2026-10-11T09:00:00Z",
            t(9, 0),
        )
        .unwrap();
        j.insert_pending(
            "x",
            Some("other"),
            MarkType::WorkdayStart,
            "2026-10-11T08:00:00Z",
            t(8, 0),
        )
        .unwrap();
        let ids: Vec<String> = j
            .pending_for_worker("w")
            .unwrap()
            .into_iter()
            .map(|m| m.record.client_mark_id)
            .collect();
        assert_eq!(ids, vec!["a", "b"]);

        j.record_failed_attempt("a").unwrap();
        assert!(j.get("a").unwrap().unwrap().record.offline);
        assert_eq!(j.pending_counts().unwrap(), (3, 1));

        let mark = server_mark("s-1");
        j.record_server_answer("a", LocalMarkStatus::Confirmed, &mark, None, r#"{"mark":{"id":"s-1","worker_id":"w","type":"WORKDAY_START","server_ts":"2026-10-11T12:00:05Z","device_ts":"2026-10-11T12:00:04Z","offline":false,"source":"AGENT","hash":"h","prev_hash":"p"}}"#)
            .unwrap();
        let a = j.get("a").unwrap().unwrap();
        assert_eq!(a.record.status, LocalMarkStatus::Confirmed);
        assert_eq!(a.record.server_mark.unwrap().id, "s-1");

        j.record_rejection("b", "DEVICE_TS_IN_FUTURE").unwrap();
        let b = j.get("b").unwrap().unwrap();
        assert_eq!(b.record.status, LocalMarkStatus::Rejected);
        assert_eq!(
            b.record.rejection_reason.as_deref(),
            Some("DEVICE_TS_IN_FUTURE")
        );
        assert_eq!(j.pending_counts().unwrap(), (1, 0));
    }

    #[test]
    fn test_journal_retention_keeps_pending_and_recent_marks() {
        let j = MarkJournal::open_in_memory().unwrap();
        j.insert_pending(
            "old-sent",
            Some("w"),
            MarkType::WorkdayStart,
            "2020-01-01T09:00:00Z",
            t(9, 0),
        )
        .unwrap();
        j.record_rejection("old-sent", "X").unwrap();
        j.insert_pending(
            "old-pending",
            Some("w"),
            MarkType::WorkdayStart,
            "2020-01-02T09:00:00Z",
            t(9, 0),
        )
        .unwrap();
        j.insert_pending(
            "new",
            Some("w"),
            MarkType::WorkdayStart,
            "2026-10-11T09:00:00Z",
            t(9, 0),
        )
        .unwrap();
        j.enforce_retention(Utc.with_ymd_and_hms(2026, 10, 11, 0, 0, 0).unwrap())
            .unwrap();
        assert!(j.get("old-sent").unwrap().is_none());
        assert!(j.get("old-pending").unwrap().is_some());
        assert!(j.get("new").unwrap().is_some());
    }

    #[test]
    fn test_journal_notice_once_and_clear() {
        let j = MarkJournal::open_in_memory().unwrap();
        assert!(j
            .notice_once("missing:2026-10-11:WORKDAY_START", t(9, 30))
            .unwrap());
        assert!(!j
            .notice_once("missing:2026-10-11:WORKDAY_START", t(9, 31))
            .unwrap());
        j.insert_pending(
            "a",
            Some("w"),
            MarkType::WorkdayStart,
            "2026-10-11T09:00:00Z",
            t(9, 0),
        )
        .unwrap();
        j.clear().unwrap();
        assert!(j.all(None).unwrap().is_empty());
        assert!(j
            .notice_once("missing:2026-10-11:WORKDAY_START", t(9, 32))
            .unwrap());
    }
}
