use crate::config::WorkerConfig;
use crate::core::buffer::MetricsBuffer;
use crate::core::collectors::keystroke_counter::KeystrokeCollector;
use crate::core::collectors::SystemCollector;
use crate::core::metrics::MetricPayload;
use crate::io::transport::{IngestOutcome, OfflineQueue, TelemetryTransport};
use crate::notifications::NotificationDispatcher;
use crate::pipeline::build_envelopes;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

pub struct BackgroundEngine {
    config: WorkerConfig,
    buffer: Arc<MetricsBuffer>,
}

impl BackgroundEngine {
    pub fn new(config: WorkerConfig, buffer: Arc<MetricsBuffer>) -> Self {
        Self { config, buffer }
    }

    /// Spawns the collection, periodic notification, and dispatch loops.
    pub async fn run(self) {
        tracing::info!("Telemetry Worker Background Engine started");

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

        // 2. Collector thread (samples system & productivity every 30s)
        let collector_buffer = buffer.clone();
        tokio::spawn(async move {
            let mut sys_collector = SystemCollector::new();
            loop {
                sleep(Duration::from_secs(30)).await;

                // Productivity metric snapshot
                let prod = KeystrokeCollector::snapshot_and_reset();
                collector_buffer.push(MetricPayload::ProductivityBasic(prod));

                // System metric sample
                let sys = sys_collector.collect();
                collector_buffer.push(MetricPayload::System(sys));
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
                    // Create uninitialized dummy if in-memory fails
                    OfflineQueue::open_in_memory()
                        .unwrap_or_else(|_| panic!("Fatal: cannot allocate SQLite database"))
                })
            }
        };

        let mut transport = TelemetryTransport::new(self.config.clone());

        loop {
            sleep(flush_interval).await;

            // Extract snapshot via std::mem::take
            let batch = self.buffer.drain_for_flush();
            if batch.is_empty() {
                continue;
            }

            let worker_id = self
                .config
                .worker_id
                .as_deref()
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
                .unwrap_or_else(uuid::Uuid::nil);

            if let Some(admin_key) = &self.config.admin_public_key {
                let records = build_envelopes(batch, worker_id, admin_key);
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
                            // Drop permanently from queue on explicit non-recoverable error
                            delivered_ids.push(item.id);
                        }
                        IngestOutcome::NeedsQueue(_) => {
                            // Stop on network failure to maintain strict FIFO delivery order
                            break;
                        }
                    }
                }
                let _ = queue.mark_delivered(&delivered_ids);
            }
        }
    }
}
