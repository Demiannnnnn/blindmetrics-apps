//! Integration tests of the attendance service against `wiremock` servers reproducing the exact
//! JSON of `api-contract.md` §9, §10, §12 (the API does not implement contract v3 yet).

use super::service::{verify_server_mark, AttendanceDeps, AttendanceService};
use super::test_support::{FixedClock, FixedLocation, RecordingNotifier};
use super::{AttendanceNotice, MarkJournal};
use crate::background::worker::EngineSnapshot;
use crate::credentials::{CredentialStore, InMemoryCredentialStore};
use crate::state::tests::worker_v3;
use crate::state::{
    AwaitingReason, BillingBlock, HaltReason, RunMode, ScheduleDay, ScheduleSummary, WorkMode,
    WorkerSelf,
};
use agent_ipc::{DataSource, LocalMarkStatus, MarkLocation, MarkType, SecretPin};
use chrono::{DateTime, TimeZone, Utc};
use std::sync::Arc;
use telemetry_crypto_core::{
    mark_hash, CanonicalMark, ConsentFlags, MarkSource, PinVerifier, GENESIS_PREV_HASH,
};
use tokio::sync::watch;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};
use zeroize::Zeroizing;

const WORKER_ID: &str = "01927d3b-65e1-7000-8000-0000000000aa";
const WORKPLACE_ID: &str = "01927d3b-65e1-7000-8000-0000000000b1";
const SERVER_MARK_ID: &str = "01927d3b-65e1-7000-8000-000000000001";
const PIN: &str = "975318";

struct Harness {
    server: MockServer,
    service: Arc<AttendanceService>,
    creds: Arc<InMemoryCredentialStore>,
    clock: Arc<FixedClock>,
    notifier: Arc<RecordingNotifier>,
    engine: watch::Sender<EngineSnapshot>,
}

/// 09:00:04 in Santiago (UTC-3) on Monday 2026-10-12.
fn monday_9am() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 12, 12, 0, 4).unwrap()
}

fn worker() -> WorkerSelf {
    let mut me = worker_v3();
    me.id = WORKER_ID.into();
    me.timezone = Some("America/Santiago".into());
    me.run = Some("12.345.678-5".into());
    me
}

fn enrolled_creds(with_pin: bool) -> Arc<InMemoryCredentialStore> {
    let creds = Arc::new(InMemoryCredentialStore::new());
    creds.set_access_token(Some("access")).unwrap();
    creds.set_refresh_token(Some("refresh")).unwrap();
    creds.set_worker_id(Some(WORKER_ID)).unwrap();
    creds
        .set_terms_accepted_version(Some("2026-10-11"))
        .unwrap();
    if with_pin {
        let verifier = PinVerifier::create(&Zeroizing::new(PIN.to_string())).unwrap();
        creds.set_pin_verifier(&verifier).unwrap();
    }
    creds
}

async fn harness_with(
    me: Option<WorkerSelf>,
    creds: Arc<InMemoryCredentialStore>,
    location: Option<MarkLocation>,
) -> Harness {
    let server = MockServer::start().await;
    let clock = FixedClock::at(monday_9am());
    let notifier = Arc::new(RecordingNotifier::default());
    let (engine, engine_rx) = watch::channel(EngineSnapshot {
        run_mode: RunMode::Collecting(ConsentFlags::default()),
        worker_self: me,
        revoked: false,
        last_error: None,
    });
    let deps = AttendanceDeps {
        creds: creds.clone(),
        journal: MarkJournal::open_in_memory().unwrap(),
        db_path: None,
        clock: clock.clone(),
        notifier: notifier.clone(),
        location: Arc::new(FixedLocation(location)),
    };
    let service = AttendanceService::new(&server.uri(), deps, engine_rx, None).unwrap();
    Harness {
        server,
        service,
        creds,
        clock,
        notifier,
        engine,
    }
}

async fn harness() -> Harness {
    harness_with(Some(worker()), enrolled_creds(true), None).await
}

fn pin(p: &str) -> SecretPin {
    SecretPin::new(p)
}

/// Server `Mark` for a request body, with a real chained hash (or a tampered one).
fn server_mark_for(body: &serde_json::Value, tamper: bool, with_ruts: bool) -> serde_json::Value {
    let mark_type: MarkType = serde_json::from_value(body["type"].clone()).unwrap();
    let device_ts = body["device_ts"].as_str().unwrap().to_string();
    let offline = body["offline"].as_bool().unwrap();
    let canonical = CanonicalMark {
        v: 1,
        mark_id: SERVER_MARK_ID.into(),
        worker_id: WORKER_ID.into(),
        worker_run: "12.345.678-5".into(),
        employer_rut: "76.123.456-0".into(),
        r#type: mark_type,
        server_ts: "2026-10-12T12:00:05Z".into(),
        device_ts: device_ts.clone(),
        offline,
        source: MarkSource::Agent,
        workplace_id: Some(WORKPLACE_ID.into()),
        location: None,
        prev_hash: GENESIS_PREV_HASH.into(),
    };
    let hash = if tamper {
        "f".repeat(64)
    } else {
        mark_hash(&canonical).unwrap()
    };
    let mut mark = serde_json::json!({
        "id": SERVER_MARK_ID, "client_mark_id": body["client_mark_id"], "worker_id": WORKER_ID,
        "type": body["type"], "server_ts": "2026-10-12T12:00:05Z", "device_ts": device_ts,
        "offline": offline, "source": "AGENT", "identification": "DEVICE_PIN",
        "workplace": { "id": WORKPLACE_ID, "name": "Casa matriz",
                       "address": "Av. Apoquindo 1234, piso 5, of. 501, Las Condes, Santiago, Región Metropolitana" },
        "location": null, "hash": hash, "prev_hash": GENESIS_PREV_HASH,
        "corrected_by": null, "superseded_by": null,
        "effective_ts": if offline { device_ts.clone() } else { "2026-10-12T12:00:05Z".to_string() }
    });
    if with_ruts {
        mark["worker_run"] = "12.345.678-5".into();
        mark["employer_rut"] = "76.123.456-0".into();
    }
    mark
}

fn mark_responder(
    status: u16,
    tamper: bool,
) -> impl Fn(&Request) -> ResponseTemplate + Send + Sync + 'static {
    move |req: &Request| {
        let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
        let mut data = serde_json::json!({ "mark": server_mark_for(&body, tamper, true) });
        if status == 200 {
            data["duplicate_of"] = SERVER_MARK_ID.into();
        }
        ResponseTemplate::new(status).set_body_json(serde_json::json!({ "data": data }))
    }
}

async fn mount_today(server: &MockServer, pending_corrections: u32) {
    Mock::given(method("GET"))
        .and(path("/attendance/me/today"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": {
                "date": "2026-10-12", "timezone": "America/Santiago",
                "schedule_today": { "start": "09:00", "end": "18:00",
                                    "break_start": "13:00", "break_end": "14:00" },
                "state": "OFF", "next_mark": "WORKDAY_START", "marks": [],
                "pending_corrections": pending_corrections, "server_time": "2026-10-12T12:00:10Z"
            }})),
        )
        .mount(server)
        .await;
}

async fn requests_to(server: &MockServer, method_name: &str, p: &str) -> Vec<serde_json::Value> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.method.as_str() == method_name && r.url.path() == p)
        .map(|r| serde_json::from_slice(&r.body).unwrap_or(serde_json::Value::Null))
        .collect()
}

// ---- marks ----------------------------------------------------------------------------------

#[tokio::test]
async fn test_attendance_mark_online_201_hash_verified_confirmed() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .and(header("authorization", "Bearer access"))
        .and(body_partial_json(serde_json::json!({
            "type": "WORKDAY_START", "device_ts": "2026-10-12T12:00:04Z",
            "offline": false, "identification": "DEVICE_PIN"
        })))
        .respond_with(mark_responder(201, false))
        .expect(1)
        .mount(&h.server)
        .await;
    mount_today(&h.server, 0).await;

    let rec = h
        .service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap();
    assert_eq!(rec.status, LocalMarkStatus::Confirmed);
    assert!(!rec.offline);
    let server_mark = rec.server_mark.unwrap();
    assert_eq!(server_mark.id, SERVER_MARK_ID);
    assert!(verify_server_mark(&server_mark));
    // UUIDv7 client id, sent once, no location requested by the schedule.
    let id = uuid::Uuid::parse_str(&rec.client_mark_id).unwrap();
    assert_eq!(id.get_version_num(), 7);
    let bodies = requests_to(&h.server, "POST", "/attendance/marks").await;
    assert_eq!(bodies[0]["client_mark_id"], rec.client_mark_id);
    assert!(bodies[0].get("location").is_none());
    assert_eq!(h.service.pending_counts(), (0, 0));
}

#[tokio::test]
async fn test_attendance_mark_hash_mismatch_is_kept_as_evidence() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(mark_responder(201, true))
        .mount(&h.server)
        .await;
    let rec = h
        .service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap();
    assert_eq!(rec.status, LocalMarkStatus::HashMismatch);
    assert!(rec.server_mark.is_some());
}

#[tokio::test]
async fn test_attendance_mark_without_canonical_ruts_cannot_be_verified() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(|req: &Request| {
            let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
            ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": { "mark": server_mark_for(&body, false, false) }
            }))
        })
        .mount(&h.server)
        .await;
    let rec = h
        .service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap();
    assert_eq!(rec.status, LocalMarkStatus::HashMismatch);
}

#[tokio::test]
async fn test_attendance_mark_offline_then_reconnect_confirms_with_offline_flag() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&h.server)
        .await;

    let rec = h
        .service
        .mark(MarkType::BreakStart, &pin(PIN))
        .await
        .unwrap();
    // ATT-02/04: the mark is accepted locally and queued; the first failure sets `offline`.
    assert_eq!(rec.status, LocalMarkStatus::Pending);
    assert!(rec.offline);
    assert_eq!(h.service.pending_counts(), (1, 1));

    // Reconnection: the server answers 201; the retry carries `offline: true` and the same id.
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .and(body_partial_json(serde_json::json!({ "offline": true })))
        .respond_with(mark_responder(201, false))
        .expect(1)
        .mount(&h.server)
        .await;
    h.clock.advance_secs(30);
    let report = h.service.drain_pending().await;
    assert_eq!(report.sent, 1);
    assert_eq!(report.offline_sent, 1);
    assert_eq!(h.service.pending_counts(), (0, 0));
    assert_eq!(
        h.notifier.taken(),
        vec![AttendanceNotice::OfflineMarksSent { count: 1 }]
    );
    let bodies = requests_to(&h.server, "POST", "/attendance/marks").await;
    assert_eq!(bodies.len(), 2);
    assert_eq!(bodies[0]["client_mark_id"], bodies[1]["client_mark_id"]);
    assert_eq!(bodies[0]["device_ts"], bodies[1]["device_ts"]);
    assert_eq!(bodies[0]["offline"], false);
    assert_eq!(bodies[1]["offline"], true);

    // Nothing left: another drain sends nothing.
    assert_eq!(h.service.drain_pending().await.sent, 0);
}

#[tokio::test]
async fn test_attendance_duplicate_200_is_idempotent() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(mark_responder(200, false))
        .mount(&h.server)
        .await;
    let rec = h
        .service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap();
    assert_eq!(rec.status, LocalMarkStatus::Confirmed);
    assert_eq!(rec.duplicate_of.as_deref(), Some(SERVER_MARK_ID));
    assert_eq!(h.service.pending_counts(), (0, 0));
}

#[tokio::test]
async fn test_attendance_queue_drains_in_device_ts_order() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(ResponseTemplate::new(502))
        .up_to_n_times(2)
        .mount(&h.server)
        .await;
    h.service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap();
    h.clock.advance_secs(3600);
    // The second mark is tried after the first pending one (which fails again).
    let second = h
        .service
        .mark(MarkType::BreakStart, &pin(PIN))
        .await
        .unwrap();
    assert!(second.offline, "queued behind an unsent mark ⇒ offline");
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(mark_responder(201, false))
        .mount(&h.server)
        .await;
    let report = h.service.drain_pending().await;
    assert_eq!(report.sent, 2);
    let bodies = requests_to(&h.server, "POST", "/attendance/marks").await;
    let sent: Vec<&str> = bodies[2..]
        .iter()
        .map(|b| b["type"].as_str().unwrap())
        .collect();
    assert_eq!(sent, vec!["WORKDAY_START", "BREAK_START"]);
}

#[tokio::test]
async fn test_attendance_mark_400_rejected_with_reason_and_never_retried() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": { "code": "VALIDATION_ERROR", "message": "device_ts in the future",
                       "details": { "reason": "DEVICE_TS_IN_FUTURE" } }
        })))
        .expect(1)
        .mount(&h.server)
        .await;
    let rec = h
        .service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap();
    assert_eq!(rec.status, LocalMarkStatus::Rejected);
    assert_eq!(rec.rejection_reason.as_deref(), Some("DEVICE_TS_IN_FUTURE"));
    assert_eq!(h.service.drain_pending().await.sent, 0);
}

#[tokio::test]
async fn test_attendance_mark_401_refreshes_once_and_retries() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .and(header("authorization", "Bearer access"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&h.server)
        .await;
    Mock::given(method("POST"))
        .and(path("/auth/refresh"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": {
                "access_token": "access-2", "refresh_token": "refresh-2", "token_type": "Bearer",
                "expires_in": 900,
                "user": { "id": WORKER_ID, "email": "j@acme.cl", "name": "J", "role": "WORKER" }
            }})),
        )
        .expect(1)
        .mount(&h.server)
        .await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .and(header("authorization", "Bearer access-2"))
        .respond_with(mark_responder(201, false))
        .expect(1)
        .mount(&h.server)
        .await;
    let rec = h
        .service
        .mark(MarkType::WorkdayEnd, &pin(PIN))
        .await
        .unwrap();
    assert_eq!(rec.status, LocalMarkStatus::Confirmed);
    assert_eq!(h.creds.get_refresh_token().as_deref(), Some("refresh-2"));
}

#[tokio::test]
async fn test_attendance_location_read_once_only_when_schedule_asks() {
    let loc = MarkLocation {
        lat: -33.41,
        lon: -70.57,
        accuracy_m: 25.0,
    };
    let mut me = worker();
    me.location_at_mark = true;
    let h = harness_with(Some(me), enrolled_creds(true), Some(loc.clone())).await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(mark_responder(201, false))
        .mount(&h.server)
        .await;
    h.service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap();
    let bodies = requests_to(&h.server, "POST", "/attendance/marks").await;
    assert_eq!(
        bodies[0]["location"],
        serde_json::json!({ "lat": -33.41, "lon": -70.57, "accuracy_m": 25.0 })
    );

    // Without `location_at_mark` the provider is never used (ATT-08).
    let h = harness_with(Some(worker()), enrolled_creds(true), Some(loc)).await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(mark_responder(201, false))
        .mount(&h.server)
        .await;
    h.service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap();
    let bodies = requests_to(&h.server, "POST", "/attendance/marks").await;
    assert!(bodies[0].get("location").is_none());
}

#[tokio::test]
async fn test_attendance_wrong_pin_growing_delay_never_locks() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(mark_responder(201, false))
        .mount(&h.server)
        .await;

    let e1 = h
        .service
        .mark(MarkType::WorkdayStart, &pin("111222"))
        .await
        .unwrap_err();
    assert_eq!(
        (e1.code.as_str(), e1.retry_after_secs),
        ("PIN_INCORRECT", None)
    );
    let e2 = h
        .service
        .mark(MarkType::WorkdayStart, &pin("111222"))
        .await
        .unwrap_err();
    assert_eq!(
        (e2.code.as_str(), e2.retry_after_secs),
        ("PIN_INCORRECT", Some(2))
    );
    // During the delay even the right PIN waits (never verified, never a mark).
    let e3 = h
        .service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap_err();
    assert_eq!(
        (e3.code.as_str(), e3.retry_after_secs),
        ("PIN_RETRY_LATER", Some(2))
    );
    h.clock.advance_secs(2);
    let e4 = h
        .service
        .mark(MarkType::WorkdayStart, &pin("111222"))
        .await
        .unwrap_err();
    assert_eq!(e4.retry_after_secs, Some(4));
    // Many failures: the delay caps at 60 s and the worker can always try again.
    for _ in 0..10 {
        h.clock.advance_secs(61);
        let _ = h.service.mark(MarkType::WorkdayStart, &pin("111222")).await;
    }
    let capped = h
        .service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap_err();
    assert_eq!(capped.retry_after_secs, Some(60));
    h.clock.advance_secs(60);
    let ok = h
        .service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap();
    assert_eq!(ok.status, LocalMarkStatus::Confirmed);
    // Reset after success: a wrong PIN starts again with no delay.
    let again = h
        .service
        .mark(MarkType::WorkdayEnd, &pin("111222"))
        .await
        .unwrap_err();
    assert_eq!(again.retry_after_secs, None);
    // Only the correct attempt reached the server.
    assert_eq!(
        requests_to(&h.server, "POST", "/attendance/marks")
            .await
            .len(),
        1
    );
}

#[tokio::test]
async fn test_attendance_mark_before_terms_is_refused_without_creating_it() {
    let creds = enrolled_creds(true);
    creds.set_terms_accepted_version(None).unwrap();
    let mut me = worker();
    me.terms_accepted_version = None;
    me.status = crate::state::WorkerStatus::PendingTerms;
    let h = harness_with(Some(me), creds, None).await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(mark_responder(201, false))
        .expect(0)
        .mount(&h.server)
        .await;
    let err = h
        .service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap_err();
    assert_eq!(err.code, "TERMS_REQUIRED");
    assert_eq!(h.service.pending_counts(), (0, 0));
    assert!(h.service.today_view().local_marks.is_empty());
}

#[tokio::test]
async fn test_attendance_mark_with_newer_terms_pending_still_works() {
    let mut me = worker();
    me.terms_required_version = Some("2027-01-15".into());
    let h = harness_with(Some(me), enrolled_creds(true), None).await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(mark_responder(201, false))
        .expect(1)
        .mount(&h.server)
        .await;
    assert_eq!(
        h.service.mode(),
        crate::state::AttendanceMode::Ready {
            terms_update_pending: true
        }
    );
    let rec = h
        .service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap();
    assert_eq!(rec.status, LocalMarkStatus::Confirmed);
}

#[tokio::test]
async fn test_attendance_mark_never_depends_on_run_mode() {
    // ATT-07 / ATT-02: every telemetry state, incl. Halted and a suspended subscription.
    let modes = vec![
        RunMode::Unenrolled,
        RunMode::NotRequested,
        RunMode::AwaitingConsent {
            reason: AwaitingReason::NoConsent,
        },
        RunMode::AwaitingConsent {
            reason: AwaitingReason::NewVersionPending,
        },
        RunMode::AwaitingConsent {
            reason: AwaitingReason::TermsPending,
        },
        RunMode::Declined,
        RunMode::Collecting(ConsentFlags::default()),
        RunMode::Paused,
        RunMode::EmployerSuspended {
            reason: BillingBlock::SubscriptionSuspended,
        },
        RunMode::EmployerSuspended {
            reason: BillingBlock::SubscriptionCancelled,
        },
        RunMode::Halted(HaltReason::KeyFingerprintMismatch),
        RunMode::Halted(HaltReason::UnknownServerState),
        RunMode::Halted(HaltReason::TrayUnavailable),
    ];
    for mode in modes {
        let h = harness().await;
        let mut me = worker();
        me.ingestion_blocked_reason = Some("SUBSCRIPTION_SUSPENDED".into());
        h.engine.send_replace(EngineSnapshot {
            run_mode: mode.clone(),
            worker_self: Some(me),
            revoked: false,
            last_error: None,
        });
        Mock::given(method("POST"))
            .and(path("/attendance/marks"))
            .respond_with(mark_responder(201, false))
            .expect(1)
            .mount(&h.server)
            .await;
        let rec = h
            .service
            .mark(MarkType::WorkdayStart, &pin(PIN))
            .await
            .unwrap_or_else(|e| panic!("{mode:?}: {e:?}"));
        assert_eq!(rec.status, LocalMarkStatus::Confirmed, "{mode:?}");
    }
}

#[tokio::test]
async fn test_attendance_exempt_and_unlinked_and_no_pin() {
    let mut me = worker();
    me.art22_exempt = true;
    let h = harness_with(Some(me), enrolled_creds(true), None).await;
    let err = h
        .service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap_err();
    assert_eq!(err.code, "ATTENDANCE_EXEMPT");

    let h = harness_with(Some(worker()), enrolled_creds(false), None).await;
    let err = h
        .service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap_err();
    assert_eq!(err.code, "PIN_NOT_SET");

    let h = harness_with(None, Arc::new(InMemoryCredentialStore::new()), None).await;
    let err = h
        .service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap_err();
    assert_eq!(err.code, "NOT_ENROLLED");
}

#[tokio::test]
async fn test_attendance_offline_reads_come_from_local_journal() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&h.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/attendance/me/today"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&h.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/attendance/me/marks"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&h.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/attendance/me/days"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&h.server)
        .await;
    h.service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap();
    assert!(h.service.refresh_today().await.is_err());
    let today = h.service.today_view();
    assert_eq!(today.source, DataSource::Local);
    assert_eq!(today.local_marks.len(), 1);
    assert_eq!(today.local_marks[0].status, LocalMarkStatus::Pending);

    let (source, server, local, _) = h
        .service
        .marks("2026-10-12", "2026-10-12", None)
        .await
        .unwrap();
    assert_eq!(source, DataSource::Local);
    assert!(server.is_empty());
    assert_eq!(local.len(), 1);

    let err = h
        .service
        .days("2026-10-01", "2026-10-12")
        .await
        .unwrap_err();
    assert_eq!(err.code, "OFFLINE");
}

#[tokio::test]
async fn test_attendance_online_reads_today_marks_and_days() {
    let h = harness().await;
    mount_today(&h.server, 0).await;
    Mock::given(method("GET"))
        .and(path("/attendance/me/marks"))
        .and(wiremock::matchers::query_param("from", "2026-10-01"))
        .and(wiremock::matchers::query_param("to", "2026-10-12"))
        .and(wiremock::matchers::query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [server_mark_for(&serde_json::json!({
                "client_mark_id": null, "type": "WORKDAY_START",
                "device_ts": "2026-10-12T12:00:04Z", "offline": false
            }), false, true)],
            "pagination": { "total": 1, "page": 1, "per_page": 50, "total_pages": 1 }
        })))
        .mount(&h.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/attendance/me/days"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": [{
                "date": "2026-10-09",
                "agreed": { "start": "09:00:00", "end": "18:00:00", "break": "13:00:00-14:00:00",
                            "break_counts_as_work": false },
                "marks": [], "attended": true, "absence": null, "absence_note": null,
                "missing": "-00:12:30", "extra": "+00:40:00", "anomalies": ["BREAK_WITHOUT_END"],
                "altered": false
            }]})),
        )
        .mount(&h.server)
        .await;

    h.service.refresh_today().await.unwrap();
    let today = h.service.today_view();
    assert_eq!(today.source, DataSource::Server);
    assert_eq!(today.server_time_offset_secs, Some(6));
    assert_eq!(
        today.server.unwrap().next_mark,
        Some(MarkType::WorkdayStart)
    );

    let (source, marks, _, pagination) = h
        .service
        .marks("2026-10-01", "2026-10-12", None)
        .await
        .unwrap();
    assert_eq!(source, DataSource::Server);
    assert_eq!(marks.len(), 1);
    assert_eq!(pagination.unwrap().total, 1);

    let days = h.service.days("2026-10-01", "2026-10-12").await.unwrap();
    // ATT-09: server values verbatim.
    assert_eq!(days[0].missing.as_deref(), Some("-00:12:30"));
    assert_eq!(days[0].extra.as_deref(), Some("+00:40:00"));

    let err = h
        .service
        .days("2026-01-01", "2026-10-12")
        .await
        .unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
    let err = h
        .service
        .marks("2019-01-01", "2026-10-12", None)
        .await
        .unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
}

// ---- terms ----------------------------------------------------------------------------------

#[tokio::test]
async fn test_attendance_accept_terms_sends_embedded_document_and_stores_version() {
    let creds = enrolled_creds(false);
    creds.set_terms_accepted_version(None).unwrap();
    let h = harness_with(None, creds, None).await;
    let terms = crate::legal::worker_terms_id();
    let privacy = crate::legal::privacy_policy_id();
    Mock::given(method("POST"))
        .and(path("/workers/me/terms-acceptance"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "document": "WORKER_TERMS", "version": terms.version, "hash": terms.hash,
            "privacy_version": privacy.version, "privacy_hash": privacy.hash
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "status": "ACTIVE", "terms_accepted_version": terms.version }
        })))
        .expect(1)
        .mount(&h.server)
        .await;
    assert_eq!(h.service.mode(), crate::state::AttendanceMode::NeedsTerms);

    // Values that are not the embedded texts are refused locally.
    let err = h
        .service
        .accept_terms("2020-01-01", &terms.hash, &privacy.version, &privacy.hash)
        .await
        .unwrap_err();
    assert_eq!(err.code, "TERMS_MISMATCH");

    let v = h
        .service
        .accept_terms(&terms.version, &terms.hash, &privacy.version, &privacy.hash)
        .await
        .unwrap();
    assert_eq!(v, terms.version);
    assert_eq!(h.creds.get_terms_accepted_version(), Some(terms.version));
    assert_eq!(h.service.mode(), crate::state::AttendanceMode::NeedsPin);
}

#[tokio::test]
async fn test_attendance_terms_409_version_mismatch_is_terms_outdated() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/workers/me/terms-acceptance"))
        .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
            "error": { "code": "CONFLICT", "message": "Outdated terms",
                       "details": { "reason": "TERMS_VERSION_MISMATCH", "current_version": "2027-01-15" } }
        })))
        .mount(&h.server)
        .await;
    let t = crate::legal::worker_terms_id();
    let p = crate::legal::privacy_policy_id();
    let err = h
        .service
        .accept_terms(&t.version, &t.hash, &p.version, &p.hash)
        .await
        .unwrap_err();
    assert_eq!(err.code, "TERMS_OUTDATED");
}

// ---- PIN ------------------------------------------------------------------------------------

#[tokio::test]
async fn test_attendance_pin_policy_set_change_and_events() {
    let h = harness_with(Some(worker()), enrolled_creds(false), None).await;
    Mock::given(method("POST"))
        .and(path("/workers/me/credential-events"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({ "type": "PIN_SET" }),
        ))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&h.server)
        .await;
    Mock::given(method("POST"))
        .and(path("/workers/me/credential-events"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({ "type": "PIN_CHANGED" }),
        ))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&h.server)
        .await;

    for (bad, code) in [
        ("123456", "PIN_POLICY_SEQUENTIAL_DIGITS"),
        ("777777", "PIN_POLICY_ALL_DIGITS_EQUAL"),
        ("12345", "PIN_POLICY_TOO_SHORT"),
        ("12a456", "PIN_POLICY_NON_DIGITS"),
        ("12345678", "PIN_POLICY_SEQUENTIAL_DIGITS"),
        ("345678", "PIN_POLICY_SEQUENTIAL_DIGITS"),
    ] {
        assert_eq!(
            h.service.set_pin(&pin(bad)).await.unwrap_err().code,
            code,
            "{bad}"
        );
    }
    assert!(h.creds.get_pin_verifier().is_none());

    h.service.set_pin(&pin(PIN)).await.unwrap();
    assert!(h.creds.get_pin_verifier().unwrap().verify_str(PIN));
    assert_eq!(
        h.service.set_pin(&pin("482916")).await.unwrap_err().code,
        "PIN_ALREADY_SET"
    );

    let wrong = h
        .service
        .change_pin(&pin("111222"), &pin("482916"))
        .await
        .unwrap_err();
    assert_eq!(wrong.code, "PIN_INCORRECT");
    h.service
        .change_pin(&pin(PIN), &pin("482916"))
        .await
        .unwrap();
    assert!(h.creds.get_pin_verifier().unwrap().verify_str("482916"));
    assert!(h.creds.get_pending_pin_event().is_none());
}

#[tokio::test]
async fn test_attendance_pin_event_failure_is_retried_later() {
    let h = harness_with(Some(worker()), enrolled_creds(false), None).await;
    Mock::given(method("POST"))
        .and(path("/workers/me/credential-events"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&h.server)
        .await;
    // The PIN is created even when the event cannot be delivered (never blocks).
    h.service.set_pin(&pin(PIN)).await.unwrap();
    assert_eq!(h.creds.get_pending_pin_event().as_deref(), Some("PIN_SET"));

    Mock::given(method("POST"))
        .and(path("/workers/me/credential-events"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({ "type": "PIN_SET" }),
        ))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&h.server)
        .await;
    h.service.retry_pin_events().await;
    assert!(h.creds.get_pending_pin_event().is_none());
}

#[tokio::test]
async fn test_attendance_pin_reset_with_code() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/workers/me/pin-reset/request"))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&h.server)
        .await;
    Mock::given(method("POST"))
        .and(path("/workers/me/pin-reset/verify"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({ "code": "000000" }),
        ))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": { "code": "VALIDATION_ERROR", "message": "bad code",
                       "details": { "reason": "INVALID_RESET_CODE" } }
        })))
        .mount(&h.server)
        .await;
    Mock::given(method("POST"))
        .and(path("/workers/me/pin-reset/verify"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({ "code": "123123" }),
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;
    Mock::given(method("POST"))
        .and(path("/workers/me/credential-events"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.service.request_pin_reset().await.unwrap();
    assert_eq!(
        h.service.verify_pin_reset("000000").await.unwrap_err().code,
        "INVALID_RESET_CODE"
    );
    assert_eq!(
        h.service.set_pin(&pin("482916")).await.unwrap_err().code,
        "PIN_ALREADY_SET"
    );

    assert_eq!(h.service.verify_pin_reset(" 123123 ").await.unwrap(), 600);
    h.service.set_pin(&pin("482916")).await.unwrap();
    assert!(h.creds.get_pin_verifier().unwrap().verify_str("482916"));
    let events = requests_to(&h.server, "POST", "/workers/me/credential-events").await;
    assert_eq!(events, vec![serde_json::json!({ "type": "PIN_CHANGED" })]);
    // The window is single-use.
    assert_eq!(
        h.service.set_pin(&pin("635241")).await.unwrap_err().code,
        "PIN_ALREADY_SET"
    );

    // And it expires after 10 minutes.
    h.service.verify_pin_reset("123123").await.unwrap();
    h.clock.advance_secs(601);
    assert_eq!(
        h.service.set_pin(&pin("635241")).await.unwrap_err().code,
        "PIN_ALREADY_SET"
    );
}

#[tokio::test]
async fn test_attendance_pin_reset_rate_limited() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/workers/me/pin-reset/verify"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "120")
                .set_body_json(serde_json::json!({
                    "error": { "code": "RATE_LIMITED", "message": "too many", "details": null }
                })),
        )
        .mount(&h.server)
        .await;
    let err = h.service.verify_pin_reset("123123").await.unwrap_err();
    assert_eq!(
        (err.code.as_str(), err.retry_after_secs),
        ("RATE_LIMITED", Some(120))
    );
}

// ---- corrections ----------------------------------------------------------------------------

fn correction_json(id: &str, status: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id, "worker_id": WORKER_ID, "kind": "MODIFY",
        "original_mark": server_mark_for(&serde_json::json!({
            "client_mark_id": null, "type": "WORKDAY_START",
            "device_ts": "2026-10-09T12:12:30Z", "offline": false
        }), false, true),
        "type": "WORKDAY_START", "proposed_ts": "2026-10-09T12:00:00Z",
        "reason": "Corte de luz en la oficina", "status": status,
        "created_at": "2026-10-10T13:00:00Z", "notified_at": "2026-10-10T13:00:01Z",
        "respond_before": "2026-10-12T13:00:01Z", "responded_at": null, "resulting_mark_id": null
    })
}

#[tokio::test]
async fn test_attendance_corrections_list_accept_oppose_and_409() {
    let h = harness().await;
    let c1 = "0192aaaa-0000-7000-8000-000000000001";
    let c2 = "0192aaaa-0000-7000-8000-000000000002";
    let c3 = "0192aaaa-0000-7000-8000-000000000003";
    Mock::given(method("GET"))
        .and(path("/attendance/me/corrections"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [correction_json(c1, "PENDING"), correction_json(c2, "PENDING")],
            "pagination": { "total": 2, "page": 1, "per_page": 20, "total_pages": 1 }
        })))
        .mount(&h.server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/attendance/me/corrections/{c1}/response")))
        .and(wiremock::matchers::body_json(
            serde_json::json!({ "accept": true }),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "data": correction_json(c1, "ACCEPTED") })),
        )
        .expect(1)
        .mount(&h.server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/attendance/me/corrections/{c2}/response")))
        .and(wiremock::matchers::body_json(
            serde_json::json!({ "accept": false }),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "data": correction_json(c2, "OPPOSED") })),
        )
        .expect(1)
        .mount(&h.server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/attendance/me/corrections/{c3}/response")))
        .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
            "error": { "code": "CONFLICT", "message": "not pending",
                       "details": { "reason": "CORRECTION_NOT_PENDING" } }
        })))
        .mount(&h.server)
        .await;
    mount_today(&h.server, 0).await;

    let list = h.service.corrections().await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].reason, "Corte de luz en la oficina");
    let accepted = h.service.respond_correction(c1, true).await.unwrap();
    assert_eq!(accepted.status, agent_ipc::CorrectionStatus::Accepted);
    let opposed = h.service.respond_correction(c2, false).await.unwrap();
    assert_eq!(opposed.status, agent_ipc::CorrectionStatus::Opposed);
    let err = h.service.respond_correction(c3, true).await.unwrap_err();
    assert_eq!(err.code, "CORRECTION_NOT_PENDING");
    let err = h
        .service
        .respond_correction("../../admins", true)
        .await
        .unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
}

#[tokio::test]
async fn test_attendance_new_pending_correction_notifies_once() {
    let h = harness().await;
    mount_today(&h.server, 1).await;
    Mock::given(method("GET"))
        .and(path("/attendance/me/corrections"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [correction_json("0192aaaa-0000-7000-8000-000000000001", "PENDING")],
            "pagination": { "total": 1, "page": 1, "per_page": 20, "total_pages": 1 }
        })))
        .mount(&h.server)
        .await;
    h.service.refresh_today().await.unwrap();
    h.service.refresh_today().await.unwrap();
    assert_eq!(
        h.notifier.taken(),
        vec![AttendanceNotice::CorrectionProposed]
    );
    assert_eq!(h.service.snapshot().pending_corrections, 1);
}

// ---- local notices (injectable clock) -------------------------------------------------------

fn worker_with_schedule() -> WorkerSelf {
    let mut me = worker();
    me.schedule = Some(ScheduleSummary {
        journey_type: Some("FIXED".into()),
        days: vec![ScheduleDay {
            weekday: 1,
            start: "09:00".into(),
            end: "18:00".into(),
            break_start: Some("13:00".into()),
            break_end: Some("14:00".into()),
        }],
        ..Default::default()
    });
    me
}

#[tokio::test]
async fn test_attendance_missing_start_notification_once_per_day() {
    let h = harness_with(Some(worker_with_schedule()), enrolled_creds(true), None).await;
    // 09:29 Santiago: too early.
    h.service
        .check_reminders(Utc.with_ymd_and_hms(2026, 10, 12, 12, 29, 0).unwrap());
    assert!(h.notifier.taken().is_empty());
    // 09:31: due, shown once.
    let t = Utc.with_ymd_and_hms(2026, 10, 12, 12, 31, 0).unwrap();
    h.service.check_reminders(t);
    h.service.check_reminders(t + chrono::Duration::minutes(5));
    assert_eq!(
        h.notifier.taken(),
        vec![AttendanceNotice::MissingMark {
            r#type: MarkType::WorkdayStart,
            agreed: "09:00".into()
        }]
    );
    // Tuesday has no agreed schedule: nothing.
    h.service
        .check_reminders(Utc.with_ymd_and_hms(2026, 10, 13, 12, 31, 0).unwrap());
    assert!(h.notifier.taken().is_empty());
}

#[tokio::test]
async fn test_attendance_no_missing_notice_when_marked_or_exempt() {
    let h = harness_with(Some(worker_with_schedule()), enrolled_creds(true), None).await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&h.server)
        .await;
    // Marked offline at 09:10: still counts as marked.
    h.clock
        .set(Utc.with_ymd_and_hms(2026, 10, 12, 12, 10, 0).unwrap());
    h.service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap();
    h.service
        .check_reminders(Utc.with_ymd_and_hms(2026, 10, 12, 12, 31, 0).unwrap());
    assert!(h
        .notifier
        .taken()
        .iter()
        .all(|n| !matches!(n, AttendanceNotice::MissingMark { .. })));

    let mut exempt = worker_with_schedule();
    exempt.art22_exempt = true;
    let h = harness_with(Some(exempt), enrolled_creds(true), None).await;
    h.service
        .check_reminders(Utc.with_ymd_and_hms(2026, 10, 12, 12, 31, 0).unwrap());
    assert!(h.notifier.taken().is_empty());
}

#[tokio::test]
async fn test_attendance_disconnection_notice_for_remote_workers_only() {
    let mut me = worker_with_schedule();
    me.work_mode = Some(WorkMode::Remote);
    me.disconnection = Some(crate::state::DisconnectionWindow {
        start: "20:00".into(),
        end: "08:00".into(),
    });
    let h = harness_with(Some(me.clone()), enrolled_creds(true), None).await;
    // 19:35 Santiago = 22:35Z.
    let t = Utc.with_ymd_and_hms(2026, 10, 12, 22, 35, 0).unwrap();
    h.service.check_reminders(t);
    h.service.check_reminders(t + chrono::Duration::minutes(10));
    assert_eq!(
        h.notifier.taken(),
        vec![AttendanceNotice::DisconnectionSoon {
            start: "20:00".into()
        }]
    );

    me.work_mode = Some(WorkMode::OnSite);
    let h = harness_with(Some(me), enrolled_creds(true), None).await;
    h.service.check_reminders(t);
    assert!(h.notifier.taken().is_empty());
}

// ---- ATT-01: no background path creates a mark ----------------------------------------------

#[tokio::test]
async fn test_attendance_background_tasks_never_post_a_mark() {
    let h = harness_with(Some(worker_with_schedule()), enrolled_creds(true), None).await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(mark_responder(201, false))
        .expect(0)
        .mount(&h.server)
        .await;
    mount_today(&h.server, 0).await;
    Mock::given(method("GET"))
        .and(path("/attendance/me/corrections"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": [] })))
        .mount(&h.server)
        .await;

    // Every background entry point, at times around the agreed schedule.
    for minutes in [0, 31, 60 * 5, 60 * 9 + 31] {
        h.clock
            .set(monday_9am() + chrono::Duration::minutes(minutes));
        h.service.check_reminders(h.clock_now());
        let _ = h.service.refresh_today().await;
        h.service.drain_pending().await;
        h.service.retry_pin_events().await;
        let _ = h.service.snapshot();
        let _ = h.service.tray_line();
    }
    // The background loop itself, with engine state changes (boot, sync, reconnect).
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let task = tokio::spawn(h.service.clone().run(shutdown.clone()));
    for error in [Some("offline".to_string()), None] {
        h.engine.send_replace(EngineSnapshot {
            run_mode: RunMode::Collecting(ConsentFlags::default()),
            worker_self: Some(worker_with_schedule()),
            revoked: false,
            last_error: error,
        });
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    shutdown.notify_waiters();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), task).await;
    assert!(requests_to(&h.server, "POST", "/attendance/marks")
        .await
        .is_empty());
}

impl Harness {
    fn clock_now(&self) -> DateTime<Utc> {
        use super::Clock;
        self.clock.now()
    }
}

// ---- SEC-19: the PIN never reaches the logs ------------------------------------------------

#[derive(Clone, Default)]
struct LogSink(Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for LogSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogSink {
    type Writer = LogSink;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn test_attendance_pin_never_logged() {
    let sink = LogSink::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(sink.clone())
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let h = harness_with(Some(worker()), enrolled_creds(false), None).await;
    Mock::given(method("POST"))
        .and(path("/workers/me/credential-events"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(mark_responder(201, false))
        .mount(&h.server)
        .await;
    let new_pin = "864213";
    h.service.set_pin(&pin(PIN)).await.unwrap();
    let _ = h.service.mark(MarkType::WorkdayStart, &pin("111222")).await;
    h.clock.advance_secs(120);
    h.service
        .mark(MarkType::WorkdayStart, &pin(PIN))
        .await
        .unwrap();
    h.service
        .change_pin(&pin(PIN), &pin(new_pin))
        .await
        .unwrap();
    tracing::debug!(request = ?agent_ipc::Request::Mark {
        r#type: MarkType::WorkdayEnd,
        pin: pin(PIN)
    }, "request debug output");

    let logs = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
    // Capture works (callsites already cached by parallel tests may be skipped, so only the
    // test's own event is required).
    assert!(
        logs.contains("request debug output"),
        "logs captured: {logs}"
    );
    assert!(logs.contains("SecretPin(<redacted>)"));
    for secret in [PIN, "111222", new_pin] {
        assert!(!logs.contains(secret), "PIN {secret} leaked into logs");
    }
}

// ---- revocation and unlink --------------------------------------------------------------------

#[tokio::test]
async fn test_attendance_prepare_unlink_sends_pending_then_clears_journal() {
    let h = harness().await;
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&h.server)
        .await;
    h.service
        .mark(MarkType::WorkdayEnd, &pin(PIN))
        .await
        .unwrap();
    assert_eq!(h.service.pending_counts().0, 1);
    Mock::given(method("POST"))
        .and(path("/attendance/marks"))
        .respond_with(mark_responder(201, false))
        .expect(1)
        .mount(&h.server)
        .await;
    h.service.prepare_unlink().await;
    assert_eq!(h.service.pending_counts(), (0, 0));
    assert!(h.service.today_view().local_marks.is_empty());
}
