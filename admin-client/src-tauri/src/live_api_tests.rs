//! Live API contract tests for admin-client-core (TASK-INT-05, RF-INT-05-2).
//!
//! These tests run against a real running `telemetry-api` v3 instance.
//! Opt-in via `--features live-api`. Requires `TELEMETRY_LIVE_API_URL`.

use crate::client::AdminApiClient;
use crate::commands::attendance::*;
use crate::commands::employer::*;
use crate::commands::keys::*;
use crate::commands::legal::*;
use crate::commands::reports::*;
use crate::commands::workers::*;
use crate::keystore::{init_test_vault, load_admin_refresh_token, KEYSTORE_TEST_MUTEX};
use rand::Rng;
use reqwest::StatusCode;
use telemetry_crypto_core::attendance::MarkType;
use telemetry_crypto_core::rut::{compute_dv, format_rut};

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
async fn test_admin_live_api_contract_suite() {
    let _guard = KEYSTORE_TEST_MUTEX.lock().await;
    init_test_vault();

    let api_url = live_api_url();
    let client = AdminApiClient::new(api_url.clone());
    let http = client.http().clone();

    let test_suffix = uuid::Uuid::new_v4().simple().to_string();
    let admin_email = format!("admin_{test_suffix}@acme.test");
    let password = "LivePassword123!#";
    let org_name = format!("Acme SpA {test_suffix}");
    let employer_rut = generate_unique_rut(76);
    let worker_run = generate_unique_rut(15);
    let worker_email = format!("worker_{test_suffix}@acme.test");

    // 1. Register + login with password (with device)
    let reg_profile = client
        .register("Admin Real", &admin_email, password, &org_name)
        .await
        .expect("Admin registration must succeed");
    assert_eq!(reg_profile.email, admin_email);
    assert_eq!(reg_profile.organization, org_name);

    let login_profile = client
        .login(&admin_email, password)
        .await
        .expect("Admin login must succeed");
    assert_eq!(login_profile.id, reg_profile.id);

    // 2. Single-use refresh
    let stored_refresh = load_admin_refresh_token().expect("refresh token in keystore");
    let refresh_profile = client
        .unlock_with_refresh()
        .await
        .expect("unlock with refresh must succeed");
    assert_eq!(refresh_profile.id, login_profile.id);

    // Verify refresh token is single-use: attempting reuse triggers 401 TOKEN_REUSE_DETECTED
    let reuse_resp = http
        .post(client.url("/auth/refresh"))
        .json(&serde_json::json!({ "refresh_token": stored_refresh.as_str() }))
        .send()
        .await
        .expect("reuse request failed");
    assert_eq!(reuse_resp.status(), StatusCode::UNAUTHORIZED);
    let reuse_body: serde_json::Value = reuse_resp.json().await.expect("json");
    assert_eq!(
        reuse_body["error"]["details"]["reason"], "TOKEN_REUSE_DETECTED",
        "Reused refresh token must trigger TOKEN_REUSE_DETECTED"
    );

    // Re-login to obtain a fresh session for the rest of the test
    let _ = client
        .login(&admin_email, password)
        .await
        .expect("re-login must succeed");

    // 3. Employer terms acceptance (must be accepted before accessing console resources, LEG-05)
    let terms_resp = legal_accept_terms_with_client(&client)
        .await
        .expect("employer terms acceptance must succeed");
    assert_eq!(terms_resp.status, "ACTIVE");

    // 4. Employer profile and workplace
    let emp_profile = EmployerProfile {
        kind: EmployerKind::LegalEntity,
        legal_name: org_name.clone(),
        rut: employer_rut.clone(),
        address: Address {
            street: "Av. Apoquindo".to_string(),
            number: "1234".to_string(),
            floor: Some("5".to_string()),
            office: Some("501".to_string()),
            commune: "Las Condes".to_string(),
            city: "Santiago".to_string(),
            region: "Región Metropolitana de Santiago".to_string(),
        },
    };
    let saved_emp = employer_update_with_client(&client, emp_profile)
        .await
        .expect("employer update must succeed");
    assert_eq!(saved_emp.rut, employer_rut);

    let fetched_emp = employer_get_with_client(&client)
        .await
        .expect("employer get must succeed")
        .expect("employer profile must exist");
    assert_eq!(fetched_emp.rut, employer_rut);

    let wp_form = WorkplaceForm {
        name: "Casa Matriz".to_string(),
        address: Address {
            street: "Av. Apoquindo".to_string(),
            number: "1234".to_string(),
            floor: Some("5".to_string()),
            office: Some("501".to_string()),
            commune: "Las Condes".to_string(),
            city: "Santiago".to_string(),
            region: "Región Metropolitana de Santiago".to_string(),
        },
    };
    let wp = workplaces_create_with_client(&client, wp_form)
        .await
        .expect("workplace create must succeed");
    let wp_list = workplaces_list_with_client(&client)
        .await
        .expect("workplaces list must succeed");
    assert!(wp_list.iter().any(|w| w.id == wp.id));

    // 5. Register admin public key (with telemetry-crypto-core)
    let master_pub = generate_and_save_master_keys().expect("generate master keys");
    let registered_pub = keys_register_on_server_with_client(&client)
        .await
        .expect("register public key on server");
    assert_eq!(registered_pub, master_pub);

    let k_status = keys_status_with_client(&client)
        .await
        .expect("keys status must succeed");
    assert!(k_status.matches_server);
    assert!(k_status.is_registered_on_server);

    // 6. Create a worker v3 (legal data + agreed schedule)
    let schedule = ScheduleInput {
        journey_type: JourneyType::Fixed,
        exceptional_resolution: None,
        days: vec![ScheduleDay {
            weekday: 1,
            start: "09:00".to_string(),
            end: "18:00".to_string(),
            break_start: Some("13:00".to_string()),
            break_end: Some("14:00".to_string()),
        }],
        break_counts_as_work: false,
        disconnection: None,
        location_at_mark: false,
        effective_from: "2026-10-14".to_string(),
        requested_by: RequestedBy::Employer,
        note: None,
    };
    let worker_form = CreateWorkerForm {
        name: "Trabajador Real".to_string(),
        run: worker_run.clone(),
        personal_email: worker_email.clone(),
        job_title: "Operador".to_string(),
        workplace_id: wp.id.clone(),
        work_mode: WorkMode::OnSite,
        art22_exempt: false,
        schedule: Some(schedule),
        role_type: "OFFICE".to_string(),
        consent_flags: Default::default(),
        consent_descriptions: Default::default(),
        location_cause: None,
        timezone: Some("America/Santiago".to_string()),
        enrollment_ttl_hours: Some(24),
    };
    let created = workers_create_with_client(&client, worker_form)
        .await
        .expect("worker create must succeed");
    assert_eq!(created.worker.run, worker_run);
    assert_eq!(created.worker.personal_email, worker_email);

    // 7. Emit enrollment code
    assert!(!created.enrollment.code.is_empty());
    let initial_code = created.enrollment.code.clone();

    let regen = workers_regenerate_token_with_client(&client, &created.worker.id, Some(48))
        .await
        .expect("regenerate enrollment token must succeed");
    assert!(!regen.code.is_empty());
    let active_code = regen.code;
    assert_ne!(active_code, initial_code);

    // 8. List workers
    let list = workers_list_with_client(
        &client,
        Some(WorkerListQuery {
            page: Some(1),
            per_page: Some(10),
            search: None,
            role_type: None,
            is_active: None,
            status: None,
            workplace_id: None,
            work_mode: None,
        }),
    )
    .await
    .expect("workers list must succeed");
    assert!(list.data.iter().any(|w| w.id == created.worker.id));

    // 9. Prepare attendance mark via worker flow (HTTP calls against live API)
    let dev_id = format!("device_{test_suffix}_{}", "d".repeat(32));
    let enroll_resp: serde_json::Value = http
        .post(client.url("/workers/enroll"))
        .json(&serde_json::json!({
            "token": active_code,
            "device_identifier": dev_id,
            "device_info": { "hostname": "TEST-HOST", "os": "macOS", "app_version": "0.1.0" }
        }))
        .send()
        .await
        .expect("enroll request failed")
        .json()
        .await
        .expect("enroll json failed");

    let worker_access_token = enroll_resp["data"]["auth"]["access_token"]
        .as_str()
        .expect("worker access token")
        .to_string();

    // Accept worker terms
    let legal_cur: serde_json::Value = http
        .get(client.url("/legal/current"))
        .send()
        .await
        .expect("legal current")
        .json()
        .await
        .expect("legal json");
    let w_terms = &legal_cur["data"]["worker_terms"];
    let priv_doc = &legal_cur["data"]["privacy"];

    let _terms_ok = http
        .post(client.url("/workers/me/terms-acceptance"))
        .bearer_auth(&worker_access_token)
        .json(&serde_json::json!({
            "document": "WORKER_TERMS",
            "version": w_terms["version"],
            "hash": w_terms["hash"],
            "privacy_version": priv_doc["version"],
            "privacy_hash": priv_doc["hash"]
        }))
        .send()
        .await
        .expect("accept terms failed");

    // Post an online mark
    let mark_now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mark_resp: serde_json::Value = http
        .post(client.url("/attendance/marks"))
        .bearer_auth(&worker_access_token)
        .json(&serde_json::json!({
            "client_mark_id": uuid::Uuid::now_v7().to_string(),
            "type": "WORKDAY_START",
            "device_ts": mark_now,
            "offline": false,
            "identification": "DEVICE_PIN"
        }))
        .send()
        .await
        .expect("mark request failed")
        .json()
        .await
        .expect("mark json failed");

    let mark_id = mark_resp["data"]["mark"]["id"]
        .as_str()
        .expect("mark id")
        .to_string();
    assert!(!mark_id.is_empty());

    // 10. Attendance board, days and marks from admin client + hash verification
    let board = attendance_board_with_client(&client)
        .await
        .expect("attendance board must succeed");
    assert!(
        board.iter().any(|b| b.worker_id == created.worker.id),
        "Board must contain the created worker"
    );

    let days = attendance_days_with_client(&client, &created.worker.id, "2026-10-18", "2026-10-22")
        .await
        .expect("attendance days must succeed");
    assert!(!days.is_empty(), "Days must return summaries");

    let marks = attendance_marks_with_client(
        &client,
        AttendanceMarksQuery {
            worker_id: Some(created.worker.id.clone()),
            from: None,
            to: None,
            page: Some(1),
            per_page: Some(10),
        },
    )
    .await
    .expect("attendance marks must succeed");
    assert!(!marks.data.is_empty(), "Marks list must not be empty");

    // Verification of hash of that mark with the same function the app uses:
    let target_mark = marks
        .data
        .iter()
        .find(|m| m.id == mark_id)
        .expect("target mark must be found");
    assert_eq!(
        target_mark.hash_verified,
        Some(true),
        "Mark hash must verify successfully with verify_mark"
    );

    // 11. Propose correction and list it
    let proposed = attendance_propose_correction_with_client(
        &client,
        ProposeCorrectionInput {
            worker_ids: vec![created.worker.id.clone()],
            kind: CorrectionKind::Add,
            original_mark_id: None,
            r#type: MarkType::WorkdayStart,
            proposed_ts: "2026-10-19T09:00:00Z".to_string(),
            reason: "Marca omitida por corte de energia".to_string(),
            confirm_no_harm: true,
            original_mark_ts: None,
            timezone: Some("America/Santiago".to_string()),
        },
    )
    .await
    .expect("propose correction must succeed");
    assert_eq!(proposed.len(), 1);
    let correction_id = proposed[0].id.clone();

    let corr_list = attendance_corrections_with_client(
        &client,
        AttendanceCorrectionsQuery {
            worker_id: Some(created.worker.id.clone()),
            status: None,
            page: Some(1),
            per_page: Some(10),
        },
    )
    .await
    .expect("list corrections must succeed");
    assert!(
        corr_list.data.iter().any(|c| c.id == correction_id),
        "Proposed correction must appear in corrections list"
    );

    // 12. Report R38 (JSON) and export (XLSX, PDF)
    let filters = ReportFilters {
        worker_ids: vec![created.worker.id.clone()],
        period: Some(ReportPeriod::LastWeek),
        from: None,
        to: None,
        search: None,
        journey_type: None,
        schedule_id: None,
        workplace_id: None,
        job_title: None,
        hash: None,
    };
    let report_json = attendance_report_with_client(&client, ReportKind::Attendance, &filters)
        .await
        .expect("attendance report JSON must succeed");
    assert_eq!(report_json.report, ReportKind::Attendance);
    assert!(!report_json.sections.is_empty());

    let temp_export_dir = tempfile::tempdir().expect("tempdir for export");
    let xlsx_path = attendance_export_report_with_client(
        &client,
        ReportKind::Attendance,
        &filters,
        ExportFormat::Xlsx,
        temp_export_dir.path(),
    )
    .await
    .expect("export XLSX must succeed");
    let xlsx_bytes = std::fs::read(&xlsx_path).expect("read exported xlsx file");
    assert!(
        xlsx_bytes.starts_with(b"PK"),
        "XLSX export must start with PK signature"
    );

    let pdf_path = attendance_export_report_with_client(
        &client,
        ReportKind::Attendance,
        &filters,
        ExportFormat::Pdf,
        temp_export_dir.path(),
    )
    .await
    .expect("export PDF must succeed");
    let pdf_bytes = std::fs::read(&pdf_path).expect("read exported pdf file");
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "PDF export must start with %PDF signature"
    );

    // 13. Typed error mapping: duplicate RUN -> 409 RUN_ALREADY_EXISTS
    let schedule2 = ScheduleInput {
        journey_type: JourneyType::Fixed,
        exceptional_resolution: None,
        days: vec![ScheduleDay {
            weekday: 1,
            start: "09:00".to_string(),
            end: "18:00".to_string(),
            break_start: Some("13:00".to_string()),
            break_end: Some("14:00".to_string()),
        }],
        break_counts_as_work: false,
        disconnection: None,
        location_at_mark: false,
        effective_from: "2026-10-14".to_string(),
        requested_by: RequestedBy::Employer,
        note: None,
    };
    let dup_worker = CreateWorkerForm {
        name: "Duplicado".to_string(),
        run: worker_run.clone(),
        personal_email: format!("other_{test_suffix}@acme.test"),
        job_title: "Operador".to_string(),
        workplace_id: wp.id.clone(),
        work_mode: WorkMode::OnSite,
        art22_exempt: false,
        schedule: Some(schedule2),
        role_type: "OFFICE".to_string(),
        consent_flags: Default::default(),
        consent_descriptions: Default::default(),
        location_cause: None,
        timezone: Some("America/Santiago".to_string()),
        enrollment_ttl_hours: Some(24),
    };
    let err = workers_create_with_client(&client, dup_worker)
        .await
        .unwrap_err();
    assert_eq!(err.code, "CONFLICT");
    assert_eq!(err.reason.as_deref(), Some("RUN_ALREADY_EXISTS"));
    assert!(
        err.is("RUN_ALREADY_EXISTS"),
        "Error must map to RUN_ALREADY_EXISTS"
    );
}
