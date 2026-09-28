use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use crate::config::WorkerConfig;
use crate::core::buffer::MetricsBuffer;
use crate::core::collectors::keystroke_counter::KeystrokeCollector;
use crate::core::collectors::SystemCollector;
use crate::core::metrics::MetricPayload;
use crate::io::encryption::EncryptionPipeline;
use crate::io::transport::{OfflineQueue, TelemetryTransport};

pub struct BackgroundEngine {
    config: WorkerConfig,
    buffer: Arc<MetricsBuffer>,
}

impl BackgroundEngine {
    pub fn new(config: WorkerConfig, buffer: Arc<MetricsBuffer>) -> Self {
        Self { config, buffer }
    }

    /// Spawns the collection and dispatch loop.
    pub async fn run(self) {
        tracing::info!("Telemetry Worker Background Engine started");

        let buffer = self.buffer.clone();
        let flush_interval = Duration::from_secs(self.config.flush_interval_secs);

        // 1. Collector thread (samples system & productivity every 30s)
        let collector_buffer = buffer.clone();
        tokio::spawn(async move {
            let mut sys_collector = SystemCollector::new();
            loop {
                sleep(Duration::from_secs(30)).await;

                // Productivity metric snapshot
                let prod = KeystrokeCollector::snapshot_and_reset();
                collector_buffer.push(MetricPayload::Productivity(prod));

                // System metric sample
                let sys = sys_collector.collect();
                collector_buffer.push(MetricPayload::System(sys));
            }
        });

        // 2. Dispatcher loop (flushes double buffer and dispatches over HTTP or offline SQLite)
        let mut db_path = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        db_path.push("telemetry-worker");
        std::fs::create_dir_all(&db_path).ok();
        db_path.push("offline.db");

        let mut queue = OfflineQueue::open(&db_path).expect("Failed to open offline SQLite database");
        let mut transport = TelemetryTransport::new(self.config.clone());

        loop {
            sleep(flush_interval).await;

            // Extract snapshot via std::mem::take
            let batch = self.buffer.drain_for_flush();
            if batch.is_empty() {
                continue;
            }

            if let Some(admin_key) = &self.config.admin_public_key {
                let pipeline = EncryptionPipeline::new(admin_key.clone());
                match pipeline.encrypt_batch(&batch) {
                    Ok(envelope) => {
                        let category = batch[0].category();
                        // Try sending directly, if network fails enqueue in SQLite
                        if let Err(err) = transport.send_envelope(category, &envelope.ephemeral_public_key, &envelope.encrypted_payload).await {
                            tracing::warn!("Direct dispatch failed: {:?}. Enqueuing in SQLite queue.", err);
                            queue.enqueue(category, &envelope.ephemeral_public_key, &envelope.encrypted_payload).ok();
                        }
                    }
                    Err(err) => {
                        tracing::error!("Encryption failed: {:?}", err);
                    }
                }
            }

            // Attempt to deliver queued offline records
            if let Ok(queued_items) = queue.peek_batch(10) {
                let mut delivered_ids = Vec::new();
                for item in queued_items {
                    if transport.send_envelope(&item.category, &item.ephemeral_public_key, &item.encrypted_payload).await.is_ok() {
                        delivered_ids.push(item.id);
                    } else {
                        break; // Stop on first network failure to respect order
                    }
                }
                queue.mark_delivered(&delivered_ids).ok();
            }
        }
    }
}
