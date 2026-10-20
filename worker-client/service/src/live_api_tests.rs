//! Live API contract tests for agent (TASK-INT-05, RF-INT-05-3).
//!
//! These tests run against a real running `telemetry-api` v3 instance.
//! Opt-in via `--features live-api`. Requires `TELEMETRY_LIVE_API_URL`.

use crate::api::client::{
    ApiClient, CredentialEvent, DeviceInfo, MarkRequest, TermsAcceptanceRequest,
};
use crate::legal::{privacy_policy_id, worker_terms_id, WORKER_TERMS_DOCUMENT};
use crate::state::WorkerStatus;
use agent_ipc::{DayState, MarkSource, MarkType};
use rand::Rng;
use reqwest::Client;
use telemetry_crypto_core::attendance::{verify_mark_hash, CanonicalMark};
use telemetry_crypto_core::enrollment::{
    manifest_commitment, safety_code, ConsentDescriptions, ConsentFlags, EnrollmentManifest,
    EnrollmentQr,
};
use telemetry_crypto_core::rut::{compute_dv, format_rut};
use telemetry_crypto_core::{derive_public_key, generate_admin_keypair};

fn live_api_url() -> String {
    let url = std::env::var("TELEMETRY_LIVE_API_URL")
        .expect("TELEMETRY_LIVE_API_URL is required when running with feature live-api");
    let trimmed = url.trim().trim_end_matches('/').to_string();
    assert!(
        !trimmed.is_empty(),
        "TELEMETRY_LIVE_API_URL must not be empty"
    );
    trimmed
}

fn generate_unique_rut(prefix: u32) -> String {
    let body = prefix * 1_000_000 + rand::thread_rng().gen_range(100_000..999_999);
    let dv = compute_dv(body);
    format_rut(body, dv)
}

#[tokio::test]
async fn test_worker_live_api_contract_suite() {
    let api_url = live_api_url();
    let http = Client::new();

    let test_suffix = uuid::Uuid::new_v4().simple().to_string();
    let admin_email = format!("admin_worker_{test_suffix}@acme.test");
    let password = "LivePassword123!#";
    let org_name = format!("Acme SpA {test_suffix}");
    let employer_rut = generate_unique_rut(76);
    let worker_run = generate_unique_rut(16);
    let worker_email = format!("worker_{test_suffix}@acme.test");

    // 1. Prepare admin and worker by raw HTTP (contract v3 shapes)
    let reg_res: serde_json::Value = http
        .post(format!("{api_url}/auth/register"))
        .json(&serde_json::json!({
            "name": "Admin Prep",
            "email": admin_email,
            "password": password,
            "organization": org_name,
        }))
        .send()
        .await
        .expect("register admin failed")
        .json()
        .await
        .expect("register json");

    let admin_token = reg_res["data"]["access_token"]
        .as_str()
        .expect("admin access token")
        .to_string();

    // Legal current versions
    let legal_cur: serde_json::Value = http
        .get(format!("{api_url}/legal/current"))
        .send()
        .await
        .expect("legal current")
        .json()
        .await
        .expect("legal json");
    let emp_terms = &legal_cur["data"]["employer_terms"];
    let priv_doc = &legal_cur["data"]["privacy"];

    // Accept employer terms
    let _ = http
        .post(format!("{api_url}/admins/me/terms-acceptance"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({
            "document": "EMPLOYER_TERMS",
            "version": emp_terms["version"],
            "hash": emp_terms["hash"],
            "privacy_version": priv_doc["version"],
            "privacy_hash": priv_doc["hash"]
        }))
        .send()
        .await
        .expect("accept employer terms");

    // Employer profile
    let _ = http
        .put(format!("{api_url}/admins/me/employer"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({
            "kind": "LEGAL_ENTITY",
            "legal_name": org_name,
            "rut": employer_rut,
            "address": {
                "street": "Av. Apoquindo",
                "number": "1234",
                "floor": "5",
                "office": "501",
                "commune": "Las Condes",
                "city": "Santiago",
                "region": "Región Metropolitana de Santiago"
            }
        }))
        .send()
        .await
        .expect("set employer profile");

    // Workplace
    let wp_res: serde_json::Value = http
        .post(format!("{api_url}/admins/me/workplaces"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({
            "name": "Casa Matriz",
            "address": {
                "street": "Av. Apoquindo",
                "number": "1234",
                "floor": "5",
                "office": "501",
                "commune": "Las Condes",
                "city": "Santiago",
                "region": "Región Metropolitana de Santiago"
            }
        }))
        .send()
        .await
        .expect("create workplace")
        .json()
        .await
        .expect("workplace json");
    let workplace_id = wp_res["data"]["id"]
        .as_str()
        .expect("workplace id")
        .to_string();

    // Register admin public key
    let (admin_priv_b64, _) = generate_admin_keypair();
    let admin_pub_b64 = derive_public_key(&admin_priv_b64).expect("derive pubkey");
    let _ = http
        .put(format!("{api_url}/admins/me/public-key"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({ "public_key": admin_pub_b64 }))
        .send()
        .await
        .expect("register public key");

    // Create worker v3
    let create_res: serde_json::Value = http
        .post(format!("{api_url}/workers"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({
            "name": "Juan Trabajador",
            "email": worker_email,
            "personal_email": worker_email,
            "run": worker_run,
            "job_title": "Desarrollador",
            "workplace_id": workplace_id,
            "work_mode": "ON_SITE",
            "art22_exempt": false,
            "role_type": "OFFICE",
            "consent_flags": {
                "system_activity": false,
                "network_activity": false,
                "productivity_basic": false,
                "productivity_screenshots": false,
                "file_activity": false,
                "location": false
            },
            "consent_descriptions": {
                "system_activity": null,
                "network_activity": null,
                "productivity_basic": null,
                "productivity_screenshots": null,
                "file_activity": null,
                "location": null
            },
            "enrollment_ttl_hours": 24,
            "schedule": {
                "journey_type": "FIXED",
                "days": [
                    {"weekday": 1, "start": "09:00", "end": "18:00", "break_start": "13:00", "break_end": "14:00"}
                ],
                "break_counts_as_work": false,
                "location_at_mark": false,
                "effective_from": "2026-10-14",
                "requested_by": "EMPLOYER"
            }
        }))
        .send()
        .await
        .expect("create worker request")
        .json()
        .await
        .expect("create worker json");

    let enrollment = &create_res["data"]["enrollment"];
    let enroll_code = enrollment["code"].as_str().expect("code").to_string();
    let server_commitment = enrollment["commitment"]
        .as_str()
        .expect("commitment")
        .to_string();
    let server_safety_code = enrollment["safety_code"]
        .as_str()
        .expect("safety code")
        .to_string();
    let server_qr_payload = enrollment["qr_payload"]
        .as_str()
        .expect("qr_payload")
        .to_string();

    // 2. Worker client instantiation
    let client = ApiClient::new(api_url.clone()).expect("create ApiClient");

    // 3. Complete Enrollment v2 (package, safety_code, commitment)
    let device_id = format!("agent_device_{test_suffix}_{}", "a".repeat(32));
    let device_info = DeviceInfo::collect();

    let enroll_response = client
        .enroll(&enroll_code, &device_id, &device_info)
        .await
        .expect("client.enroll must succeed");

    assert_eq!(enroll_response.status, WorkerStatus::PendingTerms);
    assert_eq!(enroll_response.admin_public_key, admin_pub_b64);

    // Recompute manifest commitment and safety code using telemetry-crypto-core
    let manifest = EnrollmentManifest {
        v: 1,
        admin_public_key: enroll_response.admin_public_key.clone(),
        consent_version: enroll_response.consent_version,
        consent_flags: ConsentFlags {
            system_activity: enroll_response.consent_flags.system_activity,
            network_activity: enroll_response.consent_flags.network_activity,
            productivity_basic: enroll_response.consent_flags.productivity_basic,
            productivity_screenshots: enroll_response.consent_flags.productivity_screenshots,
            file_activity: enroll_response.consent_flags.file_activity,
            location: enroll_response.consent_flags.location,
        },
        consent_descriptions: ConsentDescriptions {
            system_activity: enroll_response.consent_descriptions.system_activity.clone(),
            network_activity: enroll_response
                .consent_descriptions
                .network_activity
                .clone(),
            productivity_basic: enroll_response
                .consent_descriptions
                .productivity_basic
                .clone(),
            productivity_screenshots: enroll_response
                .consent_descriptions
                .productivity_screenshots
                .clone(),
            file_activity: enroll_response.consent_descriptions.file_activity.clone(),
            location: enroll_response.consent_descriptions.location.clone(),
        },
    };
    let local_commitment = manifest_commitment(&manifest).expect("commitment calculation");
    let local_safety_code = safety_code(&local_commitment).expect("safety code calculation");

    assert_eq!(
        local_commitment, server_commitment,
        "Manifest commitment must match server's commitment"
    );
    assert_eq!(
        local_safety_code, server_safety_code,
        "Safety code must match server's safety code"
    );

    // Verify QR payload matches
    let parsed_qr = EnrollmentQr::parse_payload(&server_qr_payload).expect("parse QR payload");
    assert_eq!(parsed_qr.code, enroll_code);
    assert_eq!(parsed_qr.commitment, local_commitment);

    let worker_access_token = enroll_response.auth.access_token;

    // 4. GET /workers/me (includes personal_email, v3.2)
    let worker_self = client
        .get_me(&worker_access_token)
        .await
        .expect("get_me must succeed");
    assert_eq!(worker_self.status, WorkerStatus::PendingTerms);
    assert_eq!(
        worker_self.personal_email.as_deref(),
        Some(worker_email.as_str()),
        "GET /workers/me must include personal_email"
    );
    assert_eq!(worker_self.run.as_deref(), Some(worker_run.as_str()));

    // 5. Worker terms (text, version, hash)
    let terms_id = worker_terms_id();
    let privacy_id = privacy_policy_id();
    let terms_req = TermsAcceptanceRequest {
        document: WORKER_TERMS_DOCUMENT,
        version: &terms_id.version,
        hash: &terms_id.hash,
        privacy_version: &privacy_id.version,
        privacy_hash: &privacy_id.hash,
    };
    let terms_resp = client
        .accept_terms(&worker_access_token, &terms_req)
        .await
        .expect("accept_terms must succeed");
    assert_eq!(terms_resp.status, WorkerStatus::Active);

    let worker_active = client
        .get_me(&worker_access_token)
        .await
        .expect("get_me active");
    assert_eq!(worker_active.status, WorkerStatus::Active);

    // 6. PIN credential events
    client
        .post_credential_event(&worker_access_token, CredentialEvent::PinSet)
        .await
        .expect("post PIN_SET credential event");
    client
        .post_credential_event(&worker_access_token, CredentialEvent::PinChanged)
        .await
        .expect("post PIN_CHANGED credential event");

    // 7. Online mark with client_mark_id (and idempotent retry => duplicate_of)
    let client_mark_id = uuid::Uuid::now_v7().to_string();
    let mark_ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let online_req = MarkRequest {
        client_mark_id: client_mark_id.clone(),
        r#type: MarkType::WorkdayStart,
        device_ts: mark_ts.clone(),
        offline: false,
        identification: "DEVICE_PIN",
        location: None,
    };
    let mark_resp = client
        .post_mark(&worker_access_token, &online_req)
        .await
        .expect("post_mark online must succeed");
    assert!(mark_resp.duplicate_of.is_none());
    let mark1 = mark_resp.mark;
    assert_eq!(mark1.r#type, MarkType::WorkdayStart);
    assert!(!mark1.offline);
    assert_eq!(mark1.source, MarkSource::Agent);

    // Retry idempotently with same client_mark_id
    let retry_resp = client
        .post_mark(&worker_access_token, &online_req)
        .await
        .expect("post_mark retry must succeed");
    assert_eq!(
        retry_resp.duplicate_of.as_deref(),
        Some(mark1.id.as_str()),
        "Idempotent retry must return duplicate_of with original mark id"
    );

    // 8. Recompute hash of the response with telemetry-crypto-core (no HASH_MISMATCH)
    let canonical1 = CanonicalMark {
        v: 1,
        mark_id: mark1.id.clone(),
        worker_id: mark1.worker_id.clone(),
        worker_run: mark1
            .worker_run
            .clone()
            .expect("worker_run must be present"),
        employer_rut: mark1
            .employer_rut
            .clone()
            .expect("employer_rut must be present"),
        r#type: mark1.r#type,
        server_ts: mark1.server_ts.clone(),
        device_ts: mark1.device_ts.clone(),
        offline: mark1.offline,
        source: mark1.source,
        workplace_id: mark1.workplace.as_ref().map(|w| w.id.clone()),
        location: mark1.location.clone(),
        prev_hash: mark1.prev_hash.clone(),
    };
    assert!(
        verify_mark_hash(&canonical1, &mark1.hash),
        "Online mark hash must verify without HASH_MISMATCH"
    );

    // 9. GET /attendance/me/today with expected state
    let today = client
        .get_today(&worker_access_token)
        .await
        .expect("get_today must succeed");
    assert_eq!(today.state, DayState::Working);
    assert!(!today.marks.is_empty());
    assert_eq!(today.marks[0].id, mark1.id);

    // 10. Public receipt query GET /attendance/verify/:hash
    let verify_pub = client
        .verify_mark_hash(&mark1.hash)
        .await
        .expect("verify_mark_hash must succeed");
    assert!(verify_pub.valid);
    assert_eq!(verify_pub.r#type, Some(MarkType::WorkdayStart));
    assert_eq!(verify_pub.offline, Some(false));
    assert!(verify_pub.effective_date.is_some());
    assert!(verify_pub.effective_time.is_some());
    assert_eq!(
        verify_pub.employer_legal_name.as_deref(),
        Some(org_name.as_str())
    );

    // 11. Offline mark (offline: true, past device_ts) with its effective_ts
    let past_time = (chrono::Utc::now() - chrono::Duration::hours(2))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let offline_client_mark_id = uuid::Uuid::now_v7().to_string();
    let offline_req = MarkRequest {
        client_mark_id: offline_client_mark_id,
        r#type: MarkType::BreakStart,
        device_ts: past_time.clone(),
        offline: true,
        identification: "DEVICE_PIN",
        location: None,
    };
    let offline_resp = client
        .post_mark(&worker_access_token, &offline_req)
        .await
        .expect("post offline mark must succeed");
    let mark2 = offline_resp.mark;
    assert!(mark2.offline);
    assert_eq!(
        mark2.effective_ts.as_deref(),
        Some(mark2.device_ts.as_str()),
        "For offline marks, effective_ts must equal device_ts"
    );
    assert_eq!(
        mark2.prev_hash, mark1.hash,
        "Mark hash chain must connect mark2.prev_hash to mark1.hash"
    );

    // Recompute offline mark hash
    let canonical2 = CanonicalMark {
        v: 1,
        mark_id: mark2.id.clone(),
        worker_id: mark2.worker_id.clone(),
        worker_run: mark2.worker_run.clone().expect("worker_run"),
        employer_rut: mark2.employer_rut.clone().expect("employer_rut"),
        r#type: mark2.r#type,
        server_ts: mark2.server_ts.clone(),
        device_ts: mark2.device_ts.clone(),
        offline: mark2.offline,
        source: mark2.source,
        workplace_id: mark2.workplace.as_ref().map(|w| w.id.clone()),
        location: mark2.location.clone(),
        prev_hash: mark2.prev_hash.clone(),
    };
    assert!(
        verify_mark_hash(&canonical2, &mark2.hash),
        "Offline mark hash must verify without HASH_MISMATCH"
    );
}
