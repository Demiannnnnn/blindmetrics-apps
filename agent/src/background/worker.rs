use crate::api::client::ApiClient;
use crate::config::WorkerConfig;
use crate::core::buffer::MetricsBuffer;
use crate::core::collectors::files::FileCollector;
use crate::core::collectors::keystroke_counter::KeystrokeCollector;
use crate::core::collectors::location::LocationCollector;
use crate::core::collectors::network::NetworkCollector;
use crate::core::collectors::screenshot::ScreenshotCollector;
use crate::core::collectors::SystemCollector;
use crate::core::metrics::MetricPayload;
use crate::credentials::CredentialStore;
use crate::errors::AgentError;
use crate::io::system_hooks::SystemHooksManager;

use crate::consent::{current_version_from_conflict, send_consent, ConsentAction};
use crate::io::transport::{BackoffPolicy, IngestOutcome, OfflineQueue, TelemetryTransport};
use crate::notifications::NotificationDispatcher;
use crate::pipeline::build_envelopes;
use crate::session::{with_access_token, SessionError};
use crate::state::{
    derive_run_mode, AwaitingReason, BillingBlock, RunMode, WorkerSelf, WorkerStatus,
};
use std::sync::Arc;
use std::time::Duration;
use telemetry_crypto_core::{Category, ConsentFlags};
use tokio::sync::{mpsc, watch, RwLock};
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

/// Executes consent withdrawal with DP-09 guarantee: the in-memory buffer and
/// the on-disk SQLite queue are purged BEFORE the decision is sent
/// (`PATCH /workers/me/consent { accepted: false }`, TASK-AGENT-09).
///
/// Withdrawal must always be possible (DP-10): if the admin published a newer version in the
/// meantime (`409 CONSENT_VERSION_MISMATCH`), it is retried once with the current version.
pub async fn execute_withdraw(
    api: &ApiClient,
    cred_store: &dyn CredentialStore,
    buffer: &MetricsBuffer,
    queue: &mut OfflineQueue,
    consent_version: i32,
    manifest_hash: &str,
) -> Result<(), SessionError> {
    let _ = buffer.drain_for_flush();
    let _ = queue.clear();
    let _ = cred_store.set_accepted_manifest_hash(None);
    let _ = cred_store.set_accepted_flags(None);
    tracing::info!("Consent withdrawn locally: buffer and queue purged (DP-09)");

    match send_consent(
        api,
        cred_store,
        consent_version,
        manifest_hash,
        ConsentAction::Decline,
    )
    .await
    {
        Ok(_) => Ok(()),
        Err(err) => match current_version_from_conflict(&err) {
            Some(current) if current != consent_version => {
                tracing::info!(
                    current,
                    "withdrawal retried with the current consent version"
                );
                send_consent(
                    api,
                    cred_store,
                    current,
                    manifest_hash,
                    ConsentAction::Decline,
                )
                .await
                .map(|_| ())
            }
            _ => Err(err),
        },
    }
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

/// Derives the run mode from `/workers/me`, also holding collection while a
/// newer consent version than the one accepted on this device is pending.
pub fn lifecycle_run_mode(
    me: &WorkerSelf,
    pinned_fp: &str,
    accepted_hash: Option<&str>,
) -> RunMode {
    let mode = derive_run_mode(me, pinned_fp, accepted_hash);
    let outdated = me
        .accepted_consent_version
        .map_or(true, |v| v < me.consent_version);
    if matches!(mode, RunMode::Collecting(_)) && outdated {
        return RunMode::AwaitingConsent {
            reason: AwaitingReason::NewVersionPending,
        };
    }
    mode
}

/// How often `/workers/me` is polled (worker-app-spec: every 5 minutes).
pub const LIFECYCLE_POLL: Duration = Duration::from_secs(300);

/// Requests from the UI thread. The egui thread never blocks on the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineCommand {
    /// Re-read `/workers/me` now (after a consent decision).
    SyncNow,
    /// Enrollment committed against `api_base_url`.
    Enrolled { api_base_url: String },
    /// DP-09: purge buffer and on-disk queue, then send WITHDRAW.
    Withdraw,
}

/// State published to the UI after every change.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineSnapshot {
    pub run_mode: RunMode,
    pub worker_self: Option<WorkerSelf>,
    /// The server revoked this device and local state was wiped.
    pub revoked: bool,
    pub last_error: Option<String>,
}

impl Default for EngineSnapshot {
    fn default() -> Self {
        Self {
            run_mode: RunMode::Unenrolled,
            worker_self: None,
            revoked: false,
            last_error: None,
        }
    }
}

/// UI-side handle: spawn async work on the engine runtime and talk to the engine.
#[derive(Clone)]
pub struct EngineHandle {
    pub runtime: tokio::runtime::Handle,
    pub commands: mpsc::UnboundedSender<EngineCommand>,
    pub state: watch::Receiver<EngineSnapshot>,
}

/// Queue-backed delivery state owned by the dispatcher loop.
struct Dispatcher {
    transport: TelemetryTransport,
    backoff: BackoffPolicy,
    next_retry_at: Option<std::time::Instant>,
}

pub struct BackgroundEngine {
    config: WorkerConfig,
    buffer: Arc<MetricsBuffer>,
    cred_store: Arc<dyn CredentialStore>,
    api: ApiClient,
    run_mode: Arc<RwLock<RunMode>>,
    worker_self: Arc<RwLock<Option<WorkerSelf>>>,
    commands: mpsc::UnboundedReceiver<EngineCommand>,
    commands_tx: mpsc::UnboundedSender<EngineCommand>,
    state: watch::Sender<EngineSnapshot>,
}

impl BackgroundEngine {
    pub fn new(
        config: WorkerConfig,
        buffer: Arc<MetricsBuffer>,
        cred_store: Arc<dyn CredentialStore>,
    ) -> Result<Self, AgentError> {
        let api = ApiClient::new(config.api_base_url.clone())?;
        let (commands_tx, commands) = mpsc::unbounded_channel();
        let (state, _) = watch::channel(EngineSnapshot::default());
        Ok(Self {
            config,
            buffer,
            cred_store,
            api,
            run_mode: Arc::new(RwLock::new(RunMode::Unenrolled)),
            worker_self: Arc::new(RwLock::new(None)),
            commands,
            commands_tx,
            state,
        })
    }

    /// Handle for the UI; `runtime` must be the runtime that runs [`Self::run`].
    pub fn handle(&self, runtime: tokio::runtime::Handle) -> EngineHandle {
        EngineHandle {
            runtime,
            commands: self.commands_tx.clone(),
            state: self.state.subscribe(),
        }
    }

    pub fn run_mode(&self) -> Arc<RwLock<RunMode>> {
        self.run_mode.clone()
    }

    pub fn worker_self(&self) -> Arc<RwLock<Option<WorkerSelf>>> {
        self.worker_self.clone()
    }

    /// Records the new state for collectors and the UI, notifying transitions (AGENT-07).
    async fn publish(
        &self,
        run_mode: RunMode,
        worker_self: Option<WorkerSelf>,
        revoked: bool,
        last_error: Option<String>,
    ) {
        let previous = self.run_mode.read().await.clone();
        if previous != run_mode {
            tracing::info!(from = ?previous, to = ?run_mode, "Run mode changed");
            match (&previous, &run_mode) {
                (_, RunMode::Paused) => NotificationDispatcher::notify_admin_paused(),
                (RunMode::Paused, RunMode::Collecting(_)) => {
                    NotificationDispatcher::notify_admin_resumed()
                }
                (RunMode::EmployerSuspended { .. }, RunMode::Collecting(_)) => {
                    NotificationDispatcher::notify_employer_reactivated()
                }
                (_, RunMode::EmployerSuspended { .. }) => {
                    NotificationDispatcher::notify_employer_suspended()
                }
                (_, RunMode::Collecting(flags)) => {
                    NotificationDispatcher::notify_collection_started(count_enabled(flags))
                }
                (
                    _,
                    RunMode::AwaitingConsent {
                        reason: AwaitingReason::NewVersionPending,
                    },
                ) => NotificationDispatcher::notify_consent_request_changed(),
                (_, RunMode::Halted(_)) => NotificationDispatcher::notify_halted(),
                _ => {}
            }
        }
        if revoked {
            NotificationDispatcher::notify_unlinked();
        }
        *self.run_mode.write().await = run_mode.clone();
        *self.worker_self.write().await = worker_self.clone();
        self.state.send_replace(EngineSnapshot {
            run_mode,
            worker_self,
            revoked,
            last_error,
        });
    }

    async fn publish_error(&self, error: String) {
        let mode = self.run_mode.read().await.clone();
        let ws = self.worker_self.read().await.clone();
        self.publish(mode, ws, false, Some(error)).await;
    }

    /// Polls `GET /workers/me` and derives the run mode (DP-06, SEC-08).
    async fn sync_lifecycle(&self, queue: &mut OfflineQueue) {
        if self.cred_store.get_access_token().is_none() {
            self.publish(RunMode::Unenrolled, None, false, None).await;
            return;
        }
        let api = self.api.clone();
        let me = with_access_token(&api, self.cred_store.as_ref(), |token| {
            let api = api.clone();
            async move { api.get_me(&token).await }
        })
        .await;

        match me {
            Ok(me) if me.status == WorkerStatus::Revoked => self.revoke(queue).await,
            Ok(me) => {
                let pinned = self.cred_store.get_pinned_fingerprint().unwrap_or_default();
                let accepted = self.cred_store.get_accepted_manifest_hash();
                let mode = lifecycle_run_mode(&me, &pinned, accepted.as_deref());
                self.publish(mode, Some(me), false, None).await;
            }
            Err(SessionError::Revoked) => self.revoke(queue).await,
            Err(SessionError::NotEnrolled) => {
                self.publish(RunMode::Unenrolled, None, false, None).await
            }
            // Offline or server error: keep the current mode; data keeps queuing.
            Err(e) => {
                tracing::warn!("Lifecycle sync failed: {}", e);
                self.publish_error(format!("No se pudo contactar al servidor: {}", e))
                    .await;
            }
        }
    }

    async fn revoke(&self, queue: &mut OfflineQueue) {
        handle_revocation(self.cred_store.as_ref(), &self.buffer, queue);
        self.publish(RunMode::Unenrolled, None, true, None).await;
    }

    async fn withdraw(&self, queue: &mut OfflineQueue) {
        let accepted_hash = self.cred_store.get_accepted_manifest_hash();
        // The API requires the CURRENT version for any decision (TASK-AGENT-09).
        let version = self
            .worker_self
            .read()
            .await
            .as_ref()
            .map(|w| w.consent_version);
        // Stop collectors immediately, before any network round-trip.
        let ws = self.worker_self.read().await.clone();
        self.publish(
            RunMode::AwaitingConsent {
                reason: AwaitingReason::NoConsent,
            },
            ws,
            false,
            None,
        )
        .await;

        let result = match (accepted_hash, version) {
            (Some(hash), Some(version)) => {
                execute_withdraw(
                    &self.api,
                    self.cred_store.as_ref(),
                    &self.buffer,
                    queue,
                    version,
                    &hash,
                )
                .await
            }
            // Nothing accepted server-side is known: purge locally only.
            _ => {
                let _ = self.buffer.drain_for_flush();
                let _ = queue.clear();
                let _ = self.cred_store.set_accepted_manifest_hash(None);
                let _ = self.cred_store.set_accepted_flags(None);
                Ok(())
            }
        };
        match result {
            Err(SessionError::Revoked) => self.revoke(queue).await,
            Err(e) => {
                tracing::warn!("WITHDRAW could not be sent: {}", e);
                self.publish_error(format!(
                    "Se detuvo la recolección y se borraron los datos locales, pero el servidor no confirmó el retiro: {}",
                    e
                ))
                .await;
            }
            Ok(()) => self.sync_lifecycle(queue).await,
        }
    }

    /// BILL-03: an ingest answered `402`/`403 SUBSCRIPTION_*`. Collection stops at once (the
    /// queue is kept) and `/workers/me` confirms the block.
    async fn employer_blocked(&self, reason: BillingBlock, queue: &mut OfflineQueue) {
        if matches!(*self.run_mode.read().await, RunMode::Collecting(_)) {
            let ws = self.worker_self.read().await.clone();
            self.publish(RunMode::EmployerSuspended { reason }, ws, false, None)
                .await;
        }
        self.sync_lifecycle(queue).await;
    }

    /// Spawns the collection, periodic notification, and dispatch loops.
    pub async fn run(mut self) {
        tracing::info!("Telemetry Worker Background Engine started");

        // Start low-level OS input hook (RF-AGENT-02-1)
        SystemHooksManager::start_hook();

        let buffer = self.buffer.clone();
        let flush_interval = Duration::from_secs(self.config.flush_interval_secs.max(1));

        // 1. Periodic hourly transparency notification, only while collecting.
        let notify_run_mode = self.run_mode.clone();
        tokio::spawn(async move {
            let one_hour = Duration::from_secs(3600);
            loop {
                sleep(one_hour).await;
                if matches!(*notify_run_mode.read().await, RunMode::Collecting(_)) {
                    NotificationDispatcher::notify_periodic_active();
                }
            }
        });

        // 2. Collector thread (gated strictly on RunMode and local consent)
        let collector_buffer = buffer.clone();
        let collector_run_mode = self.run_mode.clone();
        let collector_worker_self = self.worker_self.clone();
        let collector_cred_store = self.cred_store.clone();
        let corporate_dir = self.config.corporate_watched_dir();

        tokio::spawn(async move {
            let mut sys_collector = SystemCollector::new();
            let net_collector = NetworkCollector::new();
            let mut file_collector = FileCollector::new(corporate_dir);
            let _ = file_collector.start_watching();

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

                // Check network collection (DP-02, DP-06, DP-07)
                if should_collect(
                    Category::Network,
                    &mode,
                    role,
                    in_work_hours,
                    &locally_accepted,
                ) {
                    for item in net_collector.drain() {
                        collector_buffer.push(MetricPayload::Network(item));
                    }
                }

                // Check file metadata collection (DP-03, DP-06, DP-07)
                if should_collect(
                    Category::Files,
                    &mode,
                    role,
                    in_work_hours,
                    &locally_accepted,
                ) {
                    for item in file_collector.drain() {
                        collector_buffer.push(MetricPayload::File(item));
                    }
                }

                // Check location collection (DP-04, DP-06, DP-07)
                if should_collect(
                    Category::Location,
                    &mode,
                    role,
                    in_work_hours,
                    &locally_accepted,
                ) {
                    let start_t = ws
                        .as_ref()
                        .and_then(|w| w.work_hours_start.as_deref())
                        .and_then(|s| {
                            chrono::NaiveTime::parse_from_str(s, "%H:%M:%S")
                                .or_else(|_| chrono::NaiveTime::parse_from_str(s, "%H:%M"))
                                .ok()
                        });
                    let end_t = ws
                        .as_ref()
                        .and_then(|w| w.work_hours_end.as_deref())
                        .and_then(|s| {
                            chrono::NaiveTime::parse_from_str(s, "%H:%M:%S")
                                .or_else(|_| chrono::NaiveTime::parse_from_str(s, "%H:%M"))
                                .ok()
                        });
                    let loc_collector =
                        LocationCollector::new(role, start_t, end_t, locally_accepted.location);
                    if let Some(loc) = loc_collector.collect() {
                        collector_buffer.push(MetricPayload::Location(loc));
                    }
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

        // 4. Dispatcher + lifecycle loop: owns the on-disk queue, so withdrawal
        // and revocation purge the real queue (DP-09).
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
                match OfflineQueue::open_in_memory() {
                    Ok(q) => q,
                    Err(e) => {
                        tracing::error!("Cannot open any SQLite queue, engine stopped: {:?}", e);
                        self.publish_error("No se pudo abrir la cola local de envío.".into())
                            .await;
                        return;
                    }
                }
            }
        };

        let transport =
            match TelemetryTransport::new(&self.config.api_base_url, self.cred_store.clone()) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Cannot build HTTP transport, engine stopped: {:?}", e);
                    self.publish_error("No se pudo inicializar el cliente HTTP.".into())
                        .await;
                    return;
                }
            };
        let mut dispatcher = Dispatcher {
            transport,
            backoff: BackoffPolicy::default(),
            next_retry_at: None,
        };

        let mut flush_tick = tokio::time::interval(flush_interval);
        let mut lifecycle_tick = tokio::time::interval(LIFECYCLE_POLL);

        loop {
            tokio::select! {
                _ = lifecycle_tick.tick() => self.sync_lifecycle(&mut queue).await,
                Some(cmd) = self.commands.recv() => match cmd {
                    EngineCommand::SyncNow => self.sync_lifecycle(&mut queue).await,
                    EngineCommand::Enrolled { api_base_url } => {
                        match ApiClient::new(api_base_url.clone()) {
                            Ok(api) => self.api = api,
                            Err(e) => tracing::error!("Invalid API client for {}: {:?}", api_base_url, e),
                        }
                        if let Err(e) = dispatcher.transport.set_base_url(&api_base_url) {
                            tracing::error!("Invalid transport for {}: {:?}", api_base_url, e);
                        }
                        self.config.api_base_url = api_base_url;
                        self.sync_lifecycle(&mut queue).await;
                    }
                    EngineCommand::Withdraw => self.withdraw(&mut queue).await,
                },
                _ = flush_tick.tick() => {
                    let mode = self.run_mode.read().await.clone();
                    if !matches!(mode, RunMode::Collecting(_)) {
                        continue;
                    }
                    match self.flush(&mut dispatcher, &mut queue).await {
                        FlushResult::Revoked => self.revoke(&mut queue).await,
                        FlushResult::EmployerBlocked(reason) => {
                            self.employer_blocked(reason, &mut queue).await
                        }
                        FlushResult::Done => {}
                    }
                }
            }
        }
    }

    async fn flush(&self, d: &mut Dispatcher, queue: &mut OfflineQueue) -> FlushResult {
        let transport = &mut d.transport;
        let backoff = &mut d.backoff;
        let next_retry_at = &mut d.next_retry_at;
        let dispatch_cred = &self.cred_store;

        let mut employer_block: Option<BillingBlock> = None;
        let mut is_in_backoff = if let Some(retry_time) = *next_retry_at {
            if std::time::Instant::now() < retry_time {
                true
            } else {
                *next_retry_at = None;
                false
            }
        } else {
            false
        };

        // Extract snapshot via std::mem::take
        let batch = self.buffer.drain_for_flush();
        let worker_id = dispatch_cred
            .get_worker_id()
            .as_deref()
            .and_then(|id| uuid::Uuid::parse_str(id).ok())
            .unwrap_or_else(uuid::Uuid::nil);

        // SEC-07, SEC-08: Encrypt only to pinned admin public key
        if !batch.is_empty() {
            if let Some(admin_key) = dispatch_cred.get_pinned_admin_public_key() {
                let records = build_envelopes(batch, worker_id, &admin_key);
                for rec_res in records {
                    match rec_res {
                        Ok(record) => {
                            if is_in_backoff {
                                let _ = queue.enqueue(
                                    record.category.as_str(),
                                    &record.ephemeral_public_key,
                                    &record.encrypted_payload,
                                    record.client_timestamp,
                                    record.subcategory,
                                );
                                continue;
                            }

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
                                IngestOutcome::Delivered => {
                                    backoff.reset();
                                    *next_retry_at = None;
                                }
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
                                IngestOutcome::Revoked => return FlushResult::Revoked,
                                IngestOutcome::EmployerBlocked(reason) => {
                                    // BILL-03: consented data is kept; nothing else is sent.
                                    let _ = queue.enqueue(
                                        record.category.as_str(),
                                        &record.ephemeral_public_key,
                                        &record.encrypted_payload,
                                        record.client_timestamp,
                                        record.subcategory,
                                    );
                                    employer_block = Some(BillingBlock::from_reason(&reason));
                                    is_in_backoff = true;
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
                                    let delay = backoff.next_backoff();
                                    *next_retry_at = Some(std::time::Instant::now() + delay);
                                    is_in_backoff = true;
                                }
                            }
                        }
                        Err(err) => {
                            tracing::error!("Batch encryption failed: {:?}", err);
                        }
                    }
                }
            }
        }

        if let Some(reason) = employer_block {
            return FlushResult::EmployerBlocked(reason);
        }

        // Attempt to deliver queued offline records if not in backoff
        if !is_in_backoff {
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
                            backoff.reset();
                            *next_retry_at = None;
                        }
                        IngestOutcome::DroppedForbidden(_)
                        | IngestOutcome::DroppedValidation(_) => {
                            delivered_ids.push(item.id);
                        }
                        IngestOutcome::Revoked => return FlushResult::Revoked,
                        IngestOutcome::EmployerBlocked(reason) => {
                            // Kept as is: no retry counted, delivered after reactivation.
                            let _ = queue.mark_delivered(&delivered_ids);
                            return FlushResult::EmployerBlocked(BillingBlock::from_reason(
                                &reason,
                            ));
                        }
                        IngestOutcome::NeedsQueue(err) => {
                            let _ = queue.increment_retries(&[item.id]);
                            let delay = backoff.next_backoff();
                            *next_retry_at = Some(std::time::Instant::now() + delay);
                            tracing::warn!(
                                "Failed delivering queued item {}: backing off for {:?} ({})",
                                item.id,
                                delay,
                                err
                            );
                            break;
                        }
                    }
                }
                let _ = queue.mark_delivered(&delivered_ids);
            }
        }
        FlushResult::Done
    }
}

#[derive(Debug, PartialEq, Eq)]
enum FlushResult {
    Done,
    Revoked,
    /// `402`/`403 SUBSCRIPTION_*` from the ingestion (BILL-03).
    EmployerBlocked(BillingBlock),
}

fn count_enabled(flags: &ConsentFlags) -> usize {
    [
        flags.system_activity,
        flags.network_activity,
        flags.productivity_basic,
        flags.productivity_screenshots,
        flags.file_activity,
        flags.location,
    ]
    .iter()
    .filter(|f| **f)
    .count()
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

        cred_store.set_worker_id(Some("worker-123")).unwrap();
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/workers/me/consent"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "accepted": false,
                "consent_version": 1,
                "manifest_hash": "manifest_hash_123"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "status": "PENDING_CONSENT", "consent_version": 1,
                    "accepted_consent_version": null,
                    "consent_flags": ConsentFlags::default()
                }
            })))
            .expect(1)
            .mount(&server)
            .await;
        let client = ApiClient::new(server.uri()).unwrap();

        // Execute withdrawal (DP-09)
        execute_withdraw(
            &client,
            &cred_store,
            &buffer,
            &mut queue,
            1,
            "manifest_hash_123",
        )
        .await
        .unwrap();

        // Buffer and queue must be completely empty!
        assert!(buffer.drain_for_flush().is_empty());
        assert_eq!(queue.count().unwrap(), 0);
        assert!(cred_store.get_accepted_manifest_hash().is_none());
        assert!(cred_store.get_accepted_flags().is_none());
    }

    #[tokio::test]
    async fn test_withdraw_retries_once_with_current_version() {
        let server = MockServer::start().await;
        // The admin published v2 after the agent last synced: the first attempt is stale.
        Mock::given(method("PATCH"))
            .and(path("/workers/me/consent"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "accepted": false, "consent_version": 1, "manifest_hash": "h"
            })))
            .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                "error": {
                    "code": "CONFLICT", "message": "Consent version 1 is not the current version",
                    "details": { "reason": "CONSENT_VERSION_MISMATCH", "current_consent_version": 2 }
                }
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/workers/me/consent"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "accepted": false, "consent_version": 2, "manifest_hash": "h"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "status": "PENDING_CONSENT", "consent_version": 2,
                          "accepted_consent_version": null, "consent_flags": ConsentFlags::default() }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let creds = InMemoryCredentialStore::new();
        creds.set_access_token(Some("t")).unwrap();
        creds.set_worker_id(Some("w")).unwrap();
        let client = ApiClient::new(server.uri()).unwrap();
        let buffer = MetricsBuffer::new(10);
        let mut queue = OfflineQueue::open_in_memory().unwrap();
        execute_withdraw(&client, &creds, &buffer, &mut queue, 1, "h")
            .await
            .unwrap();
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

    fn me_json(status: &str, version: i32, accepted: Option<i32>, fp: &str) -> serde_json::Value {
        let flags = serde_json::json!({
            "system_activity": true, "network_activity": false, "productivity_basic": false,
            "productivity_screenshots": false, "file_activity": false, "location": false
        });
        serde_json::json!({ "data": {
            "id": "worker-1", "status": status, "role_type": "OFFICE",
            "work_hours_start": "09:00:00", "work_hours_end": "18:00:00",
            "timezone": "America/Santiago",
            "consent_version": version, "accepted_consent_version": accepted,
            "consent_flags": flags, "requested_consent_flags": flags,
            "consent_descriptions": {
                "system_activity": "Aplicaciones en uso", "network_activity": null,
                "productivity_basic": null, "productivity_screenshots": null,
                "file_activity": null, "location": null
            },
            "admin_public_key_fingerprint": fp
        }})
    }

    fn test_engine(
        server_uri: &str,
        creds: Arc<InMemoryCredentialStore>,
    ) -> (BackgroundEngine, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let mut config = WorkerConfig::load_or_init_at(dir.path().join("config.json"));
        config.api_base_url = server_uri.to_string();
        let engine =
            BackgroundEngine::new(config, Arc::new(MetricsBuffer::new(100)), creds).unwrap();
        (engine, dir)
    }

    fn enrolled_creds() -> Arc<InMemoryCredentialStore> {
        let creds = Arc::new(InMemoryCredentialStore::new());
        creds.set_access_token(Some("access")).unwrap();
        creds.set_refresh_token(Some("refresh")).unwrap();
        creds.set_worker_id(Some("worker-1")).unwrap();
        creds.set_pinned_fingerprint(Some("pinned_fp")).unwrap();
        creds
            .set_pinned_admin_public_key(Some("pinned_key"))
            .unwrap();
        creds
    }

    #[test]
    fn test_lifecycle_new_version_holds_collection() {
        let flags = ConsentFlags {
            system_activity: true,
            ..Default::default()
        };
        let me = WorkerSelf {
            id: "w".into(),
            status: WorkerStatus::Active,
            role_type: "OFFICE".into(),
            work_hours_start: None,
            work_hours_end: None,
            timezone: None,
            consent_version: 2,
            accepted_consent_version: Some(1),
            consent_flags: flags,
            requested_consent_flags: flags,
            consent_descriptions: Default::default(),
            admin_public_key_fingerprint: "fp".into(),
            enrollment_completed_at: None,
            ingestion_blocked_reason: None,
        };
        assert_eq!(
            lifecycle_run_mode(&me, "fp", Some("hash")),
            RunMode::AwaitingConsent {
                reason: AwaitingReason::NewVersionPending
            }
        );
        let current = WorkerSelf {
            accepted_consent_version: Some(2),
            ..me
        };
        assert_eq!(
            lifecycle_run_mode(&current, "fp", Some("hash")),
            RunMode::Collecting(flags)
        );
    }

    #[tokio::test]
    async fn test_engine_sync_active_worker_starts_collecting_and_publishes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/workers/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(me_json(
                "ACTIVE",
                1,
                Some(1),
                "pinned_fp",
            )))
            .mount(&server)
            .await;

        let creds = enrolled_creds();
        creds.set_accepted_manifest_hash(Some("accepted")).unwrap();
        let (engine, _dir) = test_engine(&server.uri(), creds);
        let mut state = engine.state.subscribe();
        let mut queue = OfflineQueue::open_in_memory().unwrap();

        engine.sync_lifecycle(&mut queue).await;

        assert!(matches!(
            *engine.run_mode.read().await,
            RunMode::Collecting(_)
        ));
        assert!(state.has_changed().unwrap());
        let snap = state.borrow_and_update().clone();
        assert!(matches!(snap.run_mode, RunMode::Collecting(_)));
        assert_eq!(snap.worker_self.unwrap().id, "worker-1");
    }

    #[tokio::test]
    async fn test_engine_sync_key_mismatch_halts() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/workers/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(me_json(
                "ACTIVE",
                1,
                Some(1),
                "other_fp",
            )))
            .mount(&server)
            .await;
        let creds = enrolled_creds();
        creds.set_accepted_manifest_hash(Some("accepted")).unwrap();
        let (engine, _dir) = test_engine(&server.uri(), creds);
        let mut queue = OfflineQueue::open_in_memory().unwrap();
        engine.sync_lifecycle(&mut queue).await;
        assert_eq!(
            *engine.run_mode.read().await,
            RunMode::Halted(HaltReason::KeyFingerprintMismatch)
        );
    }

    #[tokio::test]
    async fn test_engine_sync_revoked_wipes_and_reports() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/workers/me"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let creds = enrolled_creds();
        let (engine, _dir) = test_engine(&server.uri(), creds.clone());
        let mut queue = OfflineQueue::open_in_memory().unwrap();
        queue
            .enqueue("system", "key", b"payload", Utc::now(), None)
            .unwrap();

        engine.sync_lifecycle(&mut queue).await;

        assert!(creds.get_access_token().is_none());
        assert!(creds.get_pinned_admin_public_key().is_none());
        assert_eq!(queue.count().unwrap(), 0);
        assert!(engine.state.borrow().revoked);
        assert_eq!(*engine.run_mode.read().await, RunMode::Unenrolled);
    }

    #[tokio::test]
    async fn test_engine_withdraw_purges_real_queue_and_sends_current_version() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/workers/me/consent"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "accepted": false, "consent_version": 3, "manifest_hash": "accepted_hash"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "status": "PENDING_CONSENT", "consent_version": 3,
                          "accepted_consent_version": null,
                          "consent_flags": ConsentFlags::default() }
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/workers/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(me_json(
                "PENDING_CONSENT",
                3,
                None,
                "pinned_fp",
            )))
            .mount(&server)
            .await;

        let creds = enrolled_creds();
        creds
            .set_accepted_manifest_hash(Some("accepted_hash"))
            .unwrap();
        let (engine, _dir) = test_engine(&server.uri(), creds.clone());
        let me: WorkerSelf =
            serde_json::from_value(me_json("ACTIVE", 3, Some(3), "pinned_fp")["data"].clone())
                .unwrap();
        *engine.worker_self.write().await = Some(me);
        let mut queue = OfflineQueue::open_in_memory().unwrap();
        queue
            .enqueue("system", "key", b"payload", Utc::now(), None)
            .unwrap();

        engine.withdraw(&mut queue).await;

        assert_eq!(queue.count().unwrap(), 0);
        assert!(creds.get_accepted_manifest_hash().is_none());
        assert!(matches!(
            *engine.run_mode.read().await,
            RunMode::AwaitingConsent { .. }
        ));
    }

    #[tokio::test]
    async fn test_engine_billing_block_stops_collectors_and_keeps_queue() {
        let server = MockServer::start().await;
        let mut blocked = me_json("ACTIVE", 1, Some(1), "pinned_fp");
        blocked["data"]["ingestion_blocked_reason"] = serde_json::json!("SUBSCRIPTION_SUSPENDED");
        Mock::given(method("GET"))
            .and(path("/workers/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(blocked))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/workers/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(me_json(
                "ACTIVE",
                1,
                Some(1),
                "pinned_fp",
            )))
            .mount(&server)
            .await;

        let creds = enrolled_creds();
        creds.set_accepted_manifest_hash(Some("accepted")).unwrap();
        creds
            .set_accepted_flags(Some(&ConsentFlags {
                system_activity: true,
                ..Default::default()
            }))
            .unwrap();
        let (engine, _dir) = test_engine(&server.uri(), creds.clone());
        let mut queue = OfflineQueue::open_in_memory().unwrap();
        queue
            .enqueue("system", "key", b"payload", Utc::now(), None)
            .unwrap();

        engine.sync_lifecycle(&mut queue).await;
        let mode = engine.run_mode.read().await.clone();
        assert_eq!(
            mode,
            RunMode::EmployerSuspended {
                reason: BillingBlock::SubscriptionSuspended
            }
        );
        // DP-06: no collector runs outside Collecting.
        for category in [Category::System, Category::Network, Category::Files] {
            assert!(!should_collect(
                category,
                &mode,
                "OFFICE",
                true,
                &creds.get_accepted_flags().unwrap()
            ));
        }
        // Consented ciphertext is kept, credentials untouched (BILL-03).
        assert_eq!(queue.count().unwrap(), 1);
        assert!(creds.get_access_token().is_some());

        // Reactivation: the next poll resumes collection.
        engine.sync_lifecycle(&mut queue).await;
        assert!(matches!(
            *engine.run_mode.read().await,
            RunMode::Collecting(_)
        ));
        assert_eq!(queue.count().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_flush_402_keeps_record_and_requests_sync() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/telemetry/system"))
            .respond_with(ResponseTemplate::new(402).set_body_json(serde_json::json!({
                "error": { "code": "SUBSCRIPTION_SUSPENDED", "message": "suspended",
                           "details": { "reason": "SUBSCRIPTION_EXPIRED" } }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let creds = enrolled_creds();
        let (engine, _dir) = test_engine(&server.uri(), creds.clone());
        let (_priv, admin_pub) = telemetry_crypto_core::generate_admin_keypair();
        creds.set_pinned_admin_public_key(Some(&admin_pub)).unwrap();
        creds
            .set_worker_id(Some("01912345-6789-7abc-def0-123456789abc"))
            .unwrap();
        engine
            .buffer
            .push(MetricPayload::System(crate::core::metrics::SystemItem {
                ts: Utc::now(),
                foreground_app: None,
                window_title: None,
                idle_seconds: Some(0),
                cpu_pct: 1.0,
                mem_used_mb: 1,
                mem_total_mb: 2,
                net_rx_bytes: 0,
                net_tx_bytes: 0,
                usb_events: Vec::new(),
            }));
        let mut dispatcher = Dispatcher {
            transport: TelemetryTransport::new(&server.uri(), creds.clone()).unwrap(),
            backoff: BackoffPolicy::default(),
            next_retry_at: None,
        };
        let mut queue = OfflineQueue::open_in_memory().unwrap();
        let result = engine.flush(&mut dispatcher, &mut queue).await;
        assert_eq!(
            result,
            FlushResult::EmployerBlocked(BillingBlock::SubscriptionSuspended)
        );
        assert_eq!(
            queue.count().unwrap(),
            1,
            "record kept for after the reactivation"
        );
    }
}
