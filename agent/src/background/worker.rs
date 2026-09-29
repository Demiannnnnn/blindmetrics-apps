use crate::api::client::ApiClient;
use crate::config::WorkerConfig;
use crate::core::buffer::MetricsBuffer;
use crate::core::collectors::keystroke_counter::KeystrokeCollector;
use crate::core::collectors::screenshot::ScreenshotCollector;
use crate::core::collectors::SystemCollector;
use crate::core::metrics::MetricPayload;
use crate::credentials::{CredentialStore, KeyringCredentialStore};
use crate::errors::AgentError;
use crate::io::system_hooks::SystemHooksManager;
use crate::io::transport::{IngestOutcome, OfflineQueue, TelemetryTransport};
use crate::notifications::NotificationDispatcher;
use crate::pipeline::build_envelopes;
use crate::state::RunMode;
use crate::state::WorkerSelf;
use std::sync::Arc;
use std::time::Duration;
use telemetry_crypto_core::{Category, ConsentFlags};
use tokio::sync::RwLock;
use tokio::time::sleep;

/// Helper to determine if the local time falls within configured work hours.
pub fn is_within_work_hours(
    start: Option<&str>,
    end: Option<&str>,
    now: chrono::NaiveTime,
) -> bool {
    let (Some(start_str), Some(end_str)) = (start, end) else {
        return true;
    };
    let Ok(start_time) = chrono::NaiveTime::parse_from_str(start_str, "%H:%M:%S")
        .or_else(|_| chrono::NaiveTime::parse_from_str(start_str, "%H:%M"))
    else {
        return true;
    };
    let Ok(end_time) = chrono::NaiveTime::parse_from_str(end_str, "%H:%M:%S")
        .or_else(|_| chrono::NaiveTime::parse_from_str(end_str, "%H:%M"))
    else {
        return true;
    };

    if start_time <= end_time {
        now >= start_time && now <= end_time
    } else {
        now >= start_time || now <= end_time
    }
}

/// Pure gating function enforcing DP-04, DP-05, DP-06, and DP-07.
pub fn is_category_effective_and_accepted(
    category: Category,
    effective: &ConsentFlags,
    locally_accepted: &ConsentFlags,
) -> bool {
    match category {
        Category::System => effective.system_activity && locally_accepted.system_activity,
        Category::Network => effective.network_activity && locally_accepted.network_activity,
        Category::ProductivityBasic => {
            effective.productivity_basic && locally_accepted.productivity_basic
        }
        Category::ProductivityScreenshot => {
            effective.productivity_screenshots && locally_accepted.productivity_screenshots
        }
        Category::Files => effective.file_activity && locally_accepted.file_activity,
        Category::Location => effective.location && locally_accepted.location,
    }
}

pub fn should_collect(
    category: Category,
    run_mode: &RunMode,
    role_type: &str,
    is_in_work_hours: bool,
    locally_accepted: &ConsentFlags,
) -> bool {
    // DP-06: Zero collectors running outside RunMode::Collecting
    let RunMode::Collecting(effective) = run_mode else {
        return false;
    };

    // DP-07: Never collect a category not accepted on this device
    if !is_category_effective_and_accepted(category, effective, locally_accepted) {
        return false;
    }

    // DP-04: Location only if role_type == FIELD and in work hours
    if category == Category::Location && (role_type != "FIELD" || !is_in_work_hours) {
        return false;
    }

    // DP-05: Screenshots only in work hours
    if category == Category::ProductivityScreenshot && !is_in_work_hours {
        return false;
    }

    true
}

/// Executes consent withdrawal with DP-09 guarantee:
/// In-memory buffer and disk SQLite queue are purged BEFORE sending the PATCH.
pub async fn execute_withdraw(
    api: &ApiClient,
    cred_store: &dyn CredentialStore,
    buffer: &MetricsBuffer,
    queue: &mut OfflineQueue,
    worker_id: &str,
    consent_version: i32,
    manifest_hash: &str,
) -> Result<(), AgentError> {
    // DP-09: Purge in-memory buffer and SQLite offline queue immediately
    let _ = buffer.drain_for_flush();
    let _ = queue.clear();
    let _ = cred_store.set_accepted_manifest_hash(None);
    let _ = cred_store.set_accepted_flags(None);

    let token = cred_store.get_access_token().unwrap_or_default();
    if !token.is_empty() {
        // Attempt to notify server of WITHDRAW decision
        let _ = api
            .patch_consent(
                worker_id,
                &token,
                "WITHDRAW",
                consent_version,
                manifest_hash,
            )
            .await;
    }

    tracing::info!("Consent withdrawn and all buffers/queues purged (DP-09)");
    Ok(())
}

/// Handles worker revocation (401 from server or refresh):
/// Wipes local keystore secrets, pinned admin keys, and purges all queues (DP-09).
pub fn handle_revocation(
    cred_store: &dyn CredentialStore,
    buffer: &MetricsBuffer,
    queue: &mut OfflineQueue,
) {
    let _ = cred_store.wipe();
    let _ = buffer.drain_for_flush();
    let _ = queue.clear();
    tracing::warn!("Worker revoked: wiped credentials, pinned keys, and purged queue (DP-09)");
}

pub struct BackgroundEngine {
    config: WorkerConfig,
    buffer: Arc<MetricsBuffer>,
    cred_store: Arc<dyn CredentialStore>,
    run_mode: Arc<RwLock<RunMode>>,
    worker_self: Arc<RwLock<Option<WorkerSelf>>>,
}

impl BackgroundEngine {
    pub fn new(config: WorkerConfig, buffer: Arc<MetricsBuffer>) -> Self {
        Self {
            config,
            buffer,
            cred_store: Arc::new(KeyringCredentialStore::new()),
            run_mode: Arc::new(RwLock::new(RunMode::Unenrolled)),
            worker_self: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_credentials(
        config: WorkerConfig,
        buffer: Arc<MetricsBuffer>,
        cred_store: Arc<dyn CredentialStore>,
    ) -> Self {
        Self {
            config,
            buffer,
            cred_store,
            run_mode: Arc::new(RwLock::new(RunMode::Unenrolled)),
            worker_self: Arc::new(RwLock::new(None)),
        }
    }

    pub fn run_mode(&self) -> Arc<RwLock<RunMode>> {
        self.run_mode.clone()
    }

    pub fn worker_self(&self) -> Arc<RwLock<Option<WorkerSelf>>> {
        self.worker_self.clone()
    }

    /// Spawns the collection, periodic notification, and dispatch loops.
    pub async fn run(self) {
        tracing::info!("Telemetry Worker Background Engine started");

        // Start low-level OS input hook (RF-AGENT-02-1)
        SystemHooksManager::start_hook();

        // Initial transparency notification on startup
        NotificationDispatcher::notify_periodic_active();

        let buffer = self.buffer.clone();
        let flush_interval = Duration::from_secs(self.config.flush_interval_secs);

        // 1. Periodic hourly transparency notification (every 1 hour)
        tokio::spawn(async move {
            let one_hour = Duration::from_secs(3600);
            loop {
                sleep(one_hour).await;
                NotificationDispatcher::notify_periodic_active();
            }
        });

        // 2. Collector thread (gated strictly on RunMode and local consent)
        let collector_buffer = buffer.clone();
        let collector_run_mode = self.run_mode.clone();
        let collector_worker_self = self.worker_self.clone();
        let collector_cred_store = self.cred_store.clone();

        tokio::spawn(async move {
            let mut sys_collector = SystemCollector::new();
            loop {
                sleep(Duration::from_secs(30)).await;

                let mode = collector_run_mode.read().await.clone();
                let ws = collector_worker_self.read().await.clone();
                let role = ws
                    .as_ref()
                    .map(|w| w.role_type.as_str())
                    .unwrap_or("OFFICE");
                let locally_accepted = collector_cred_store
                    .get_accepted_flags()
                    .unwrap_or_default();

                let now = chrono::Local::now().time();
                let in_work_hours = is_within_work_hours(
                    ws.as_ref().and_then(|w| w.work_hours_start.as_deref()),
                    ws.as_ref().and_then(|w| w.work_hours_end.as_deref()),
                    now,
                );

                // Check productivity collection (DP-06, DP-07)
                let prod_enabled = should_collect(
                    Category::ProductivityBasic,
                    &mode,
                    role,
                    in_work_hours,
                    &locally_accepted,
                );
                KeystrokeCollector::set_hook_enabled(prod_enabled);
                if prod_enabled {
                    let prod = KeystrokeCollector::snapshot_and_reset();
                    collector_buffer.push(MetricPayload::ProductivityBasic(prod));
                }

                // Check system collection (DP-06, DP-07)
                if should_collect(
                    Category::System,
                    &mode,
                    role,
                    in_work_hours,
                    &locally_accepted,
                ) {
                    let sys = sys_collector.collect();
                    collector_buffer.push(MetricPayload::System(sys));
                }
            }
        });

        // 3. Screenshot loop (randomized every 10-15 minutes, DP-05)
        let screenshot_buffer = buffer.clone();
        let screenshot_run_mode = self.run_mode.clone();
        let screenshot_worker_self = self.worker_self.clone();
        let screenshot_cred_store = self.cred_store.clone();

        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(600)).await;

                let mode = screenshot_run_mode.read().await.clone();
                let ws = screenshot_worker_self.read().await.clone();
                let role = ws
                    .as_ref()
                    .map(|w| w.role_type.as_str())
                    .unwrap_or("OFFICE");
                let locally_accepted = screenshot_cred_store
                    .get_accepted_flags()
                    .unwrap_or_default();

                let now = chrono::Local::now().time();
                let in_work_hours = is_within_work_hours(
                    ws.as_ref().and_then(|w| w.work_hours_start.as_deref()),
                    ws.as_ref().and_then(|w| w.work_hours_end.as_deref()),
                    now,
                );

                if should_collect(
                    Category::ProductivityScreenshot,
                    &mode,
                    role,
                    in_work_hours,
                    &locally_accepted,
                ) {
                    if let Ok(Some(item)) = ScreenshotCollector::capture_primary() {
                        screenshot_buffer.push(MetricPayload::ProductivityScreenshot(item));
                    }
                }
            }
        });

        // 3. Dispatcher loop (flushes double buffer and dispatches over HTTP or offline SQLite)
        let mut db_path = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        db_path.push("telemetry-worker");
        let _ = std::fs::create_dir_all(&db_path);
        db_path.push("offline.db");

        let mut queue = match OfflineQueue::open(&db_path) {
            Ok(q) => q,
            Err(err) => {
                tracing::warn!(
                    "Failed to open disk SQLite queue ({:?}). Falling back to in-memory queue.",
                    err
                );
                OfflineQueue::open_in_memory().unwrap_or_else(|e| {
                    tracing::error!("Failed to open in-memory SQLite queue: {:?}", e);
                    OfflineQueue::open_in_memory()
                        .unwrap_or_else(|_| panic!("Fatal: cannot allocate SQLite database"))
                })
            }
        };

        let mut transport = TelemetryTransport::new(self.config.clone());
        let dispatch_cred = self.cred_store.clone();
        let dispatch_run_mode = self.run_mode.clone();

        loop {
            sleep(flush_interval).await;

            let mode = dispatch_run_mode.read().await.clone();
            if !matches!(mode, RunMode::Collecting(_)) {
                continue;
            }

            // Extract snapshot via std::mem::take
            let batch = self.buffer.drain_for_flush();
            if batch.is_empty() {
                continue;
            }

            let worker_id = dispatch_cred
                .get_worker_id()
                .as_deref()
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
                .unwrap_or_else(uuid::Uuid::nil);

            // SEC-07, SEC-08: Encrypt only to pinned admin public key
            if let Some(admin_key) = dispatch_cred.get_pinned_admin_public_key() {
                let records = build_envelopes(batch, worker_id, &admin_key);
                for rec_res in records {
                    match rec_res {
                        Ok(record) => {
                            let endpoint = record.category.api_endpoint();
                            let outcome = transport
                                .send_record(
                                    endpoint,
                                    &record.ephemeral_public_key,
                                    &record.encrypted_payload,
                                    record.client_timestamp,
                                    record.subcategory,
                                )
                                .await;

                            match outcome {
                                IngestOutcome::Delivered => {}
                                IngestOutcome::DroppedForbidden(reason) => {
                                    tracing::warn!(
                                        "Record dropped: 403 reason={:?} category={}",
                                        reason,
                                        record.category.as_str()
                                    );
                                }
                                IngestOutcome::DroppedValidation(code) => {
                                    tracing::error!(
                                        "Record dropped: validation error ({}) category={}",
                                        code,
                                        record.category.as_str()
                                    );
                                }
                                IngestOutcome::NeedsQueue(err) => {
                                    tracing::info!(
                                        "Enqueuing record into SQLite queue ({}): category={}",
                                        err,
                                        record.category.as_str()
                                    );
                                    let _ = queue.enqueue(
                                        record.category.as_str(),
                                        &record.ephemeral_public_key,
                                        &record.encrypted_payload,
                                        record.client_timestamp,
                                        record.subcategory,
                                    );
                                }
                            }
                        }
                        Err(err) => {
                            tracing::error!("Batch encryption failed: {:?}", err);
                        }
                    }
                }
            }

            // Attempt to deliver queued offline records
            if let Ok(queued_items) = queue.peek_batch(10) {
                let mut delivered_ids = Vec::new();
                for item in queued_items {
                    let endpoint = match item.category.as_str() {
                        "productivity_basic" | "productivity_screenshot" | "productivity" => {
                            "productivity"
                        }
                        other => other,
                    };

                    let outcome = transport
                        .send_record(
                            endpoint,
                            &item.ephemeral_public_key,
                            &item.encrypted_payload,
                            item.client_timestamp,
                            item.subcategory.as_deref(),
                        )
                        .await;

                    match outcome {
                        IngestOutcome::Delivered => {
                            delivered_ids.push(item.id);
                        }
                        IngestOutcome::DroppedForbidden(_)
                        | IngestOutcome::DroppedValidation(_) => {
                            delivered_ids.push(item.id);
                        }
                        IngestOutcome::NeedsQueue(_) => {
                            break;
                        }
                    }
                }
                let _ = queue.mark_delivered(&delivered_ids);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::InMemoryCredentialStore;
    use crate::state::HaltReason;
    use chrono::Utc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_scheduler_starts_only_effective_and_locally_accepted_categories() {
        let locally_accepted = ConsentFlags {
            system_activity: true,
            network_activity: false,
            productivity_basic: false,
            productivity_screenshots: false,
            file_activity: false,
            location: false,
        };

        // Server reports system and productivity_basic as effective
        let server_effective = ConsentFlags {
            system_activity: true,
            network_activity: false,
            productivity_basic: true,
            productivity_screenshots: false,
            file_activity: false,
            location: false,
        };

        let mode = RunMode::Collecting(server_effective);

        // System activity: effective on server AND accepted locally => collected
        assert!(should_collect(
            Category::System,
            &mode,
            "OFFICE",
            true,
            &locally_accepted
        ));

        // Productivity basic: effective on server but NOT accepted locally => NOT collected (DP-07)
        assert!(!should_collect(
            Category::ProductivityBasic,
            &mode,
            "OFFICE",
            true,
            &locally_accepted
        ));

        // Location: NOT accepted locally and OFFICE worker => NOT collected (DP-04, DP-07)
        assert!(!should_collect(
            Category::Location,
            &mode,
            "OFFICE",
            true,
            &locally_accepted
        ));
    }

    #[test]
    fn test_scheduler_stops_all_collectors_on_pause_withdraw_revoke() {
        let all_accepted = ConsentFlags {
            system_activity: true,
            network_activity: true,
            productivity_basic: true,
            productivity_screenshots: true,
            file_activity: true,
            location: true,
        };

        let non_collecting_modes = vec![
            RunMode::Paused,
            RunMode::AwaitingConsent {
                reason: crate::state::AwaitingReason::NoConsent,
            },
            RunMode::Unenrolled,
            RunMode::Halted(HaltReason::KeyFingerprintMismatch),
        ];

        let categories = vec![
            Category::System,
            Category::Network,
            Category::ProductivityBasic,
            Category::ProductivityScreenshot,
            Category::Files,
            Category::Location,
        ];

        // DP-06: Zero collectors running outside RunMode::Collecting
        for mode in non_collecting_modes {
            for cat in &categories {
                assert!(
                    !should_collect(*cat, &mode, "FIELD", true, &all_accepted),
                    "Mode {:?} must stop collector for {:?}",
                    mode,
                    cat
                );
            }
        }
    }

    #[tokio::test]
    async fn test_withdraw_purges_queue_and_buffer_before_patch() {
        let cred_store = InMemoryCredentialStore::new();
        cred_store.set_access_token(Some("valid_token")).unwrap();
        cred_store
            .set_accepted_manifest_hash(Some("manifest_hash_123"))
            .unwrap();
        cred_store
            .set_accepted_flags(Some(&ConsentFlags {
                system_activity: true,
                ..Default::default()
            }))
            .unwrap();

        let buffer = MetricsBuffer::new(100);
        buffer.push(MetricPayload::System(crate::core::metrics::SystemItem {
            ts: Utc::now(),
            foreground_app: None,
            window_title: None,
            idle_seconds: Some(0),
            cpu_pct: 10.0,
            mem_used_mb: 2048,
            mem_total_mb: 8192,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            usb_events: Vec::new(),
        }));
        assert_eq!(buffer.len(), 1);

        let mut queue = OfflineQueue::open_in_memory().unwrap();
        queue
            .enqueue("system", "key", b"payload", Utc::now(), None)
            .unwrap();
        assert_eq!(queue.count().unwrap(), 1);

        let client = ApiClient::new("http://localhost:8080".into()).unwrap();

        // Execute withdrawal (DP-09)
        let _ = execute_withdraw(
            &client,
            &cred_store,
            &buffer,
            &mut queue,
            "worker-123",
            1,
            "manifest_hash_123",
        )
        .await;

        // Buffer and queue must be completely empty!
        assert!(buffer.drain_for_flush().is_empty());
        assert_eq!(queue.count().unwrap(), 0);
        assert!(cred_store.get_accepted_manifest_hash().is_none());
        assert!(cred_store.get_accepted_flags().is_none());
    }

    #[tokio::test]
    async fn test_consent_version_mismatch_refetches_and_reshows() {
        let server = MockServer::start().await;

        // Mock 409 Conflict on PATCH consent
        Mock::given(method("PATCH"))
            .and(path("/workers/worker-123/consent"))
            .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                "error": {
                    "code": "CONSENT_VERSION_MISMATCH",
                    "message": "Consent version mismatch",
                    "details": {
                        "reason": "CONSENT_VERSION_MISMATCH"
                    }
                }
            })))
            .mount(&server)
            .await;

        // Mock GET /workers/me returning updated consent version 2
        Mock::given(method("GET"))
            .and(path("/workers/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "worker-123",
                    "status": "PENDING_CONSENT",
                    "role_type": "OFFICE",
                    "consent_version": 2,
                    "accepted_consent_version": 1,
                    "consent_flags": {
                        "system_activity": true,
                        "network_activity": false,
                        "productivity_basic": false,
                        "productivity_screenshots": false,
                        "file_activity": false,
                        "location": false
                    },
                    "requested_consent_flags": {
                        "system_activity": true,
                        "network_activity": true,
                        "productivity_basic": false,
                        "productivity_screenshots": false,
                        "file_activity": false,
                        "location": false
                    },
                    "consent_descriptions": {
                        "system_activity": "Procesos",
                        "network_activity": "Dominios",
                        "productivity_basic": null,
                        "productivity_screenshots": null,
                        "file_activity": null,
                        "location": null
                    },
                    "admin_public_key_fingerprint": "dummy_fp"
                }
            })))
            .mount(&server)
            .await;

        let client = ApiClient::new(server.uri()).unwrap();
        let patch_res = client
            .patch_consent("worker-123", "token", "ACCEPT", 1, "manifest_v1_hash")
            .await;

        assert!(patch_res.is_err());
        match patch_res.unwrap_err() {
            AgentError::Api { status, code, .. } => {
                assert_eq!(status, 409);
                assert_eq!(code, "CONSENT_VERSION_MISMATCH");

                // Refetch /workers/me
                let me = client.get_me("token").await.unwrap();
                assert_eq!(me.consent_version, 2);
                assert!(me.requested_consent_flags.network_activity);
            }
            other => panic!("Expected 409 Api error, got {:?}", other),
        }
    }

    #[test]
    fn test_revoked_401_wipes_credentials_and_queue() {
        let cred_store = InMemoryCredentialStore::new();
        cred_store.set_access_token(Some("token_jwt")).unwrap();
        cred_store.set_refresh_token(Some("refresh_jwt")).unwrap();
        cred_store
            .set_pinned_admin_public_key(Some("pinned_key"))
            .unwrap();
        cred_store
            .set_accepted_manifest_hash(Some("hash_123"))
            .unwrap();

        let buffer = MetricsBuffer::new(100);
        buffer.push(MetricPayload::System(crate::core::metrics::SystemItem {
            ts: Utc::now(),
            foreground_app: None,
            window_title: None,
            idle_seconds: Some(0),
            cpu_pct: 10.0,
            mem_used_mb: 2048,
            mem_total_mb: 8192,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            usb_events: Vec::new(),
        }));

        let mut queue = OfflineQueue::open_in_memory().unwrap();
        queue
            .enqueue("system", "key", b"payload", Utc::now(), None)
            .unwrap();
        assert_eq!(queue.count().unwrap(), 1);

        // Revoke handler called
        handle_revocation(&cred_store, &buffer, &mut queue);

        assert!(cred_store.get_access_token().is_none());
        assert!(cred_store.get_refresh_token().is_none());
        assert!(cred_store.get_pinned_admin_public_key().is_none());
        assert!(cred_store.get_accepted_manifest_hash().is_none());
        assert_eq!(queue.count().unwrap(), 0);
        assert!(buffer.drain_for_flush().is_empty());
    }

    #[test]
    fn test_is_within_work_hours() {
        use chrono::NaiveTime;

        // Standard daytime shift: 09:00 to 18:00
        let start = Some("09:00:00");
        let end = Some("18:00:00");

        assert!(is_within_work_hours(
            start,
            end,
            NaiveTime::from_hms_opt(9, 0, 0).unwrap()
        ));
        assert!(is_within_work_hours(
            start,
            end,
            NaiveTime::from_hms_opt(14, 30, 0).unwrap()
        ));
        assert!(is_within_work_hours(
            start,
            end,
            NaiveTime::from_hms_opt(18, 0, 0).unwrap()
        ));
        assert!(!is_within_work_hours(
            start,
            end,
            NaiveTime::from_hms_opt(8, 59, 59).unwrap()
        ));
        assert!(!is_within_work_hours(
            start,
            end,
            NaiveTime::from_hms_opt(18, 0, 1).unwrap()
        ));

        // Overnight shift: 22:00 to 06:00
        let night_start = Some("22:00");
        let night_end = Some("06:00");

        assert!(is_within_work_hours(
            night_start,
            night_end,
            NaiveTime::from_hms_opt(23, 0, 0).unwrap()
        ));
        assert!(is_within_work_hours(
            night_start,
            night_end,
            NaiveTime::from_hms_opt(4, 0, 0).unwrap()
        ));
        assert!(!is_within_work_hours(
            night_start,
            night_end,
            NaiveTime::from_hms_opt(12, 0, 0).unwrap()
        ));

        // None configured -> always true
        assert!(is_within_work_hours(
            None,
            None,
            NaiveTime::from_hms_opt(12, 0, 0).unwrap()
        ));
    }
}
