use super::keys::get_admin_public_key;
use super::CommandError;
use crate::client::{get_api_client, CollectionResponse, SingleResponse};
use serde::{Deserialize, Serialize};
use telemetry_crypto_core::enrollment::{
    manifest_commitment, public_key_fingerprint, safety_code, EnrollmentManifest, EnrollmentQr,
};
use telemetry_crypto_core::{ConsentDescriptions, ConsentFlags};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WorkerDeviceInfo {
    pub hostname: String,
    pub os: String,
    pub app_version: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WorkerResponse {
    pub id: String,
    pub admin_id: String,
    pub email: String,
    pub name: String,
    pub device_identifier: Option<String>,
    pub device_info: Option<WorkerDeviceInfo>,
    pub role_type: String, // "OFFICE" | "FIELD"
    pub is_active: bool,
    pub status: String, // "PENDING_ENROLLMENT" | "PENDING_CONSENT" | "ACTIVE" | "PAUSED" | "REVOKED"
    #[serde(default)]
    pub status_changed_at: Option<String>,
    pub consent_flags: ConsentFlags,
    pub requested_consent_flags: ConsentFlags,
    pub consent_descriptions: ConsentDescriptions,
    pub consent_version: i32,
    pub accepted_consent_version: Option<i32>,
    pub consent_accepted_at: Option<String>,
    pub enrollment_completed_at: Option<String>,
    pub enrollment_expires_at: Option<String>,
    pub work_hours_start: Option<String>,
    pub work_hours_end: Option<String>,
    pub timezone: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EnrollmentTokenResponse {
    pub code: String,
    pub expires_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CreateWorkerApiResponse {
    pub worker: WorkerResponse,
    pub enrollment: EnrollmentTokenResponse,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EnrollmentView {
    pub qr_payload: String,
    pub code: String,
    pub safety_code: String,
    pub expires_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CreatedWorker {
    pub worker: WorkerResponse,
    pub enrollment: EnrollmentView,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConsentConfigResponse {
    pub consent_version: i32,
    pub requires_worker_acceptance: bool,
    pub worker: WorkerResponse,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConsentRecord {
    pub id: String,
    pub worker_id: String,
    pub consent_version: i32,
    pub decision: String,
    pub manifest_hash: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CreateWorkerForm {
    pub email: String,
    pub name: String,
    pub role_type: String, // "OFFICE" | "FIELD"
    pub work_hours_start: String,
    pub work_hours_end: String,
    pub timezone: String,
    pub consent_flags: ConsentFlags,
    pub consent_descriptions: ConsentDescriptions,
    pub enrollment_ttl_hours: Option<u32>,
}

impl CreateWorkerForm {
    pub fn validate(&self) -> Result<(), CommandError> {
        if self.email.trim().is_empty() || !self.email.contains('@') {
            return Err(CommandError {
                code: "VALIDATION_ERROR".into(),
                message: "Valid email address is required".into(),
            });
        }
        if self.name.trim().is_empty() {
            return Err(CommandError {
                code: "VALIDATION_ERROR".into(),
                message: "Worker name is required".into(),
            });
        }

        let role_upper = self.role_type.trim().to_uppercase();
        if role_upper != "OFFICE" && role_upper != "FIELD" {
            return Err(CommandError {
                code: "VALIDATION_ERROR".into(),
                message: "Role must be OFFICE or FIELD".into(),
            });
        }

        // DP-04 / RF-ADMIN-08-2: OFFICE workers cannot have location enabled
        if role_upper == "OFFICE" && self.consent_flags.location {
            return Err(CommandError {
                code: "VALIDATION_ERROR".into(),
                message: "Location tracking is prohibited for OFFICE workers".into(),
            });
        }

        // Validate work hours: format and start < end
        let parse_time = |t: &str| {
            chrono::NaiveTime::parse_from_str(t.trim(), "%H:%M")
                .or_else(|_| chrono::NaiveTime::parse_from_str(t.trim(), "%H:%M:%S"))
        };

        match (
            parse_time(&self.work_hours_start),
            parse_time(&self.work_hours_end),
        ) {
            (Ok(start), Ok(end)) => {
                if start >= end {
                    return Err(CommandError {
                        code: "VALIDATION_ERROR".into(),
                        message: "Work hours start must be earlier than end".into(),
                    });
                }
            }
            _ => {
                return Err(CommandError {
                    code: "VALIDATION_ERROR".into(),
                    message: "Work hours must be in HH:MM format".into(),
                });
            }
        }

        if self.timezone.trim().is_empty() {
            return Err(CommandError {
                code: "VALIDATION_ERROR".into(),
                message: "Timezone is required".into(),
            });
        }

        // Validate descriptions: required for active flags, 10..=500 chars (RF-ADMIN-08-2)
        let check_desc =
            |flag: bool, desc: Option<&str>, cat_name: &str| -> Result<(), CommandError> {
                if flag {
                    let d = desc.unwrap_or("").trim();
                    if d.len() < 10 || d.len() > 500 {
                        return Err(CommandError {
                            code: "VALIDATION_ERROR".into(),
                            message: format!(
                                "Description for '{}' must be between 10 and 500 characters",
                                cat_name
                            ),
                        });
                    }
                }
                Ok(())
            };

        check_desc(
            self.consent_flags.system_activity,
            self.consent_descriptions.system_activity.as_deref(),
            "system_activity",
        )?;
        check_desc(
            self.consent_flags.network_activity,
            self.consent_descriptions.network_activity.as_deref(),
            "network_activity",
        )?;
        check_desc(
            self.consent_flags.productivity_basic,
            self.consent_descriptions.productivity_basic.as_deref(),
            "productivity_basic",
        )?;
        check_desc(
            self.consent_flags.productivity_screenshots,
            self.consent_descriptions
                .productivity_screenshots
                .as_deref(),
            "productivity_screenshots",
        )?;
        check_desc(
            self.consent_flags.file_activity,
            self.consent_descriptions.file_activity.as_deref(),
            "file_activity",
        )?;
        check_desc(
            self.consent_flags.location,
            self.consent_descriptions.location.as_deref(),
            "location",
        )?;

        Ok(())
    }

    pub fn normalized_descriptions(&self) -> ConsentDescriptions {
        let mut d = self.consent_descriptions.clone();
        d.retain_enabled(&self.consent_flags);
        d
    }
}

pub fn ensure_matches_request(
    form: &CreateWorkerForm,
    created: &WorkerResponse,
) -> Result<(), CommandError> {
    if created.requested_consent_flags != form.consent_flags {
        return Err(CommandError {
            code: "ENROLLMENT_TAMPER_CHECK_FAILED".into(),
            message: "Server returned different requested consent flags".into(),
        });
    }

    let normalized_expected = form.normalized_descriptions();
    if created.consent_descriptions != normalized_expected {
        return Err(CommandError {
            code: "ENROLLMENT_TAMPER_CHECK_FAILED".into(),
            message: "Server returned different consent descriptions".into(),
        });
    }

    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct WorkerListQuery {
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub search: Option<String>,
    pub role_type: Option<String>,
    pub status: Option<String>,
}

#[tauri::command]
pub async fn workers_list(
    query: Option<WorkerListQuery>,
) -> Result<CollectionResponse<WorkerResponse>, CommandError> {
    let client = get_api_client();
    let q = query.unwrap_or_default();

    let mut query_params = Vec::new();
    if let Some(p) = q.page {
        query_params.push(format!("page={}", p));
    }
    if let Some(pp) = q.per_page {
        query_params.push(format!("per_page={}", pp));
    }
    if let Some(ref s) = q.search {
        if s.len() >= 2 {
            query_params.push(format!("search={}", urlencoding(s)));
        }
    }
    if let Some(ref r) = q.role_type {
        query_params.push(format!("role_type={}", r));
    }
    if let Some(ref st) = q.status {
        query_params.push(format!("status={}", st));
    }

    let endpoint = if query_params.is_empty() {
        "/workers".to_string()
    } else {
        format!("/workers?{}", query_params.join("&"))
    };

    let resp = client
        .get_with_auth(&endpoint)
        .await
        .map_err(|e| CommandError {
            code: "NETWORK_ERROR".into(),
            message: e,
        })?;

    if !resp.status().is_success() {
        return Err(CommandError {
            code: format!("API_ERROR_{}", resp.status().as_u16()),
            message: "Failed to list workers".into(),
        });
    }

    let collection: CollectionResponse<WorkerResponse> =
        resp.json().await.map_err(|e| CommandError {
            code: "DESERIALIZATION_ERROR".into(),
            message: e.to_string(),
        })?;

    Ok(collection)
}

#[tauri::command]
pub async fn workers_get(id: String) -> Result<WorkerResponse, CommandError> {
    let client = get_api_client();
    let endpoint = format!("/workers/{}", id);
    let resp = client
        .get_with_auth(&endpoint)
        .await
        .map_err(|e| CommandError {
            code: "NETWORK_ERROR".into(),
            message: e,
        })?;

    if !resp.status().is_success() {
        return Err(CommandError {
            code: format!("API_ERROR_{}", resp.status().as_u16()),
            message: format!("Failed to get worker {}", id),
        });
    }

    let wrapped: SingleResponse<WorkerResponse> = resp.json().await.map_err(|e| CommandError {
        code: "DESERIALIZATION_ERROR".into(),
        message: e.to_string(),
    })?;

    Ok(wrapped.data)
}

#[tauri::command]
pub async fn workers_create(form: CreateWorkerForm) -> Result<CreatedWorker, CommandError> {
    workers_create_with_client(get_api_client(), form).await
}

pub async fn workers_create_with_client(
    client: &crate::client::AdminApiClient,
    form: CreateWorkerForm,
) -> Result<CreatedWorker, CommandError> {
    form.validate()?;

    // Check server key vs local key: must match before creating workers (RF-ADMIN-08-1)
    let admin_pub = get_admin_public_key()?;
    let local_fp = public_key_fingerprint(&admin_pub)?;

    if let Ok(me) = client.get_admin_me().await {
        if let Some(srv_fp) = me.public_key_fingerprint {
            if srv_fp != local_fp {
                tracing::warn!("Server key mismatch: worker creation blocked");
                return Err(CommandError {
                    code: "PUBLIC_KEY_MISMATCH".into(),
                    message: "Server master public key differs from local key (K2 mismatch)".into(),
                });
            }
        }
    }

    let body = serde_json::json!({
        "email": form.email,
        "name": form.name,
        "role_type": form.role_type,
        "work_hours_start": form.work_hours_start,
        "work_hours_end": form.work_hours_end,
        "timezone": form.timezone,
        "consent_flags": form.consent_flags,
        "consent_descriptions": form.normalized_descriptions(),
        "enrollment_ttl_hours": form.enrollment_ttl_hours.unwrap_or(24)
    });

    let resp = client
        .post_with_auth("/workers", &body)
        .await
        .map_err(|e| CommandError {
            code: "NETWORK_ERROR".into(),
            message: e,
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err_text = resp.text().await.unwrap_or_default();
        return Err(CommandError {
            code: format!("API_ERROR_{}", status.as_u16()),
            message: err_text,
        });
    }

    let wrapped: SingleResponse<CreateWorkerApiResponse> =
        resp.json().await.map_err(|e| CommandError {
            code: "DESERIALIZATION_ERROR".into(),
            message: e.to_string(),
        })?;

    let created = wrapped.data;

    // Tamper check (RF-3.1): compare server response with request
    ensure_matches_request(&form, &created.worker)?;

    // Build EnrollmentManifest strictly with locally derived master public key (SEC-07)
    let manifest = EnrollmentManifest::new(
        admin_pub,
        created.worker.consent_version,
        created.worker.requested_consent_flags,
        created.worker.consent_descriptions.clone(),
    )?;

    let commitment = manifest_commitment(&manifest)?;
    let safety_code_val = safety_code(&commitment)?;
    let qr = EnrollmentQr::new(
        client.get_base_url(),
        format_qr_code(&created.enrollment.code),
        commitment.clone(),
    )?;

    tracing::info!(
        worker_id = %created.worker.id,
        commitment = %commitment[..8],
        "Worker created and enrollment QR issued successfully"
    );

    Ok(CreatedWorker {
        worker: created.worker,
        enrollment: EnrollmentView {
            qr_payload: qr.to_json()?,
            code: created.enrollment.code,
            safety_code: safety_code_val,
            expires_at: created.enrollment.expires_at,
        },
    })
}

#[tauri::command]
pub async fn workers_set_status(
    id: String,
    status: String,
    reason: Option<String>,
) -> Result<WorkerResponse, CommandError> {
    let client = get_api_client();
    let endpoint = format!("/workers/{}/status", id);
    let body = serde_json::json!({
        "status": status,
        "reason": reason
    });

    let resp = client
        .patch_with_auth(&endpoint, &body)
        .await
        .map_err(|e| CommandError {
            code: "NETWORK_ERROR".into(),
            message: e,
        })?;

    if !resp.status().is_success() {
        return Err(CommandError {
            code: format!("API_ERROR_{}", resp.status().as_u16()),
            message: format!("Failed to update worker status to {}", status),
        });
    }

    let wrapped: SingleResponse<WorkerResponse> = resp.json().await.map_err(|e| CommandError {
        code: "DESERIALIZATION_ERROR".into(),
        message: e.to_string(),
    })?;

    Ok(wrapped.data)
}

#[tauri::command]
pub async fn workers_regenerate_token(
    id: String,
    ttl_hours: Option<u32>,
) -> Result<EnrollmentView, CommandError> {
    let client = get_api_client();
    let token_endpoint = format!("/workers/{}/enrollment-token", id);
    let body = serde_json::json!({
        "enrollment_ttl_hours": ttl_hours.unwrap_or(24)
    });

    let resp = client
        .post_with_auth(&token_endpoint, &body)
        .await
        .map_err(|e| CommandError {
            code: "NETWORK_ERROR".into(),
            message: e,
        })?;

    if !resp.status().is_success() {
        return Err(CommandError {
            code: format!("API_ERROR_{}", resp.status().as_u16()),
            message: "Failed to regenerate enrollment token".into(),
        });
    }

    let token_wrapped: SingleResponse<EnrollmentTokenResponse> =
        resp.json().await.map_err(|e| CommandError {
            code: "DESERIALIZATION_ERROR".into(),
            message: e.to_string(),
        })?;
    let token_data = token_wrapped.data;

    // Fetch worker to get current manifest configuration
    let worker = workers_get(id).await?;

    let admin_pub = get_admin_public_key()?;
    let manifest = EnrollmentManifest::new(
        admin_pub,
        worker.consent_version,
        worker.requested_consent_flags,
        worker.consent_descriptions,
    )?;

    let commitment = manifest_commitment(&manifest)?;
    let safety_code_val = safety_code(&commitment)?;
    let qr = EnrollmentQr::new(
        client.get_base_url(),
        format_qr_code(&token_data.code),
        commitment.clone(),
    )?;

    Ok(EnrollmentView {
        qr_payload: qr.to_json()?,
        code: token_data.code,
        safety_code: safety_code_val,
        expires_at: token_data.expires_at,
    })
}

#[tauri::command]
pub async fn workers_update_consent_config(
    id: String,
    consent_flags: ConsentFlags,
    consent_descriptions: ConsentDescriptions,
) -> Result<ConsentConfigResponse, CommandError> {
    let client = get_api_client();
    let endpoint = format!("/workers/{}/consent-config", id);
    let mut normalized_desc = consent_descriptions;
    normalized_desc.retain_enabled(&consent_flags);

    let body = serde_json::json!({
        "consent_flags": consent_flags,
        "consent_descriptions": normalized_desc
    });

    let resp = client
        .put_with_auth(&endpoint, &body)
        .await
        .map_err(|e| CommandError {
            code: "NETWORK_ERROR".into(),
            message: e,
        })?;

    if !resp.status().is_success() {
        return Err(CommandError {
            code: format!("API_ERROR_{}", resp.status().as_u16()),
            message: "Failed to update consent configuration".into(),
        });
    }

    let wrapped: SingleResponse<ConsentConfigResponse> =
        resp.json().await.map_err(|e| CommandError {
            code: "DESERIALIZATION_ERROR".into(),
            message: e.to_string(),
        })?;

    Ok(wrapped.data)
}

#[tauri::command]
pub async fn workers_get_consent_records(id: String) -> Result<Vec<ConsentRecord>, CommandError> {
    let client = get_api_client();
    let endpoint = format!("/workers/{}/consent-records", id);
    let resp = client
        .get_with_auth(&endpoint)
        .await
        .map_err(|e| CommandError {
            code: "NETWORK_ERROR".into(),
            message: e,
        })?;

    if !resp.status().is_success() {
        return Err(CommandError {
            code: format!("API_ERROR_{}", resp.status().as_u16()),
            message: "Failed to fetch consent records".into(),
        });
    }

    let wrapped: CollectionResponse<ConsentRecord> =
        resp.json().await.map_err(|e| CommandError {
            code: "DESERIALIZATION_ERROR".into(),
            message: e.to_string(),
        })?;

    Ok(wrapped.data)
}

#[tauri::command]
pub async fn workers_delete(id: String) -> Result<(), CommandError> {
    let client = get_api_client();
    let endpoint = format!("/workers/{}", id);
    let resp = client
        .delete_with_auth(&endpoint)
        .await
        .map_err(|e| CommandError {
            code: "NETWORK_ERROR".into(),
            message: e,
        })?;

    if !resp.status().is_success() {
        return Err(CommandError {
            code: format!("API_ERROR_{}", resp.status().as_u16()),
            message: format!("Failed to delete worker {}", id),
        });
    }

    Ok(())
}

fn format_qr_code(raw: &str) -> String {
    let clean: String = raw.chars().filter(|c| *c != '-' && *c != ' ').collect();
    if clean.len() == 12 {
        format!("{}-{}-{}", &clean[0..4], &clean[4..8], &clean[8..12])
    } else {
        raw.to_string()
    }
}

fn urlencoding(s: &str) -> String {
    s.replace(' ', "%20")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::AdminApiClient;
    use crate::commands::keys::generate_and_save_master_keys;
    use telemetry_crypto_core::verify_commitment;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_valid_form() -> CreateWorkerForm {
        let flags = ConsentFlags {
            system_activity: true,
            network_activity: true,
            ..Default::default()
        };
        let desc = ConsentDescriptions {
            system_activity: Some("Registro de aplicaciones activas".into()),
            network_activity: Some("Navegacion de dominios laborales".into()),
            ..Default::default()
        };

        CreateWorkerForm {
            email: "pedro@acme.cl".into(),
            name: "Pedro Morales".into(),
            role_type: "FIELD".into(),
            work_hours_start: "08:30".into(),
            work_hours_end: "17:30".into(),
            timezone: "America/Santiago".into(),
            consent_flags: flags,
            consent_descriptions: desc,
            enrollment_ttl_hours: Some(24),
        }
    }

    #[test]
    fn test_workers_create_office_with_location_rejected_client_side() {
        let mut form = make_valid_form();
        form.role_type = "OFFICE".into();
        form.consent_flags.location = true;
        form.consent_descriptions.location = Some("Ubicacion GPS del trabajador".into());

        let res = form.validate();
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert!(err
            .message
            .contains("Location tracking is prohibited for OFFICE workers"));
    }

    #[tokio::test]
    async fn test_workers_create_builds_qr_with_local_key_not_server_key() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();

        let server = MockServer::start().await;
        let local_pub = generate_and_save_master_keys().unwrap();

        let form = make_valid_form();
        let expected_worker_id = "01912345-6789-7abc-def0-123456789aaa";

        // Mock GET /admins/me returning no key yet (matches local)
        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "admin-1",
                    "email": "admin@acme.cl",
                    "name": "Admin",
                    "organization": "Acme",
                    "is_active": true,
                    "worker_count": 0,
                    "public_key": null,
                    "public_key_fingerprint": null,
                    "created_at": "2026-09-28T00:00:00Z",
                    "updated_at": "2026-09-28T00:00:00Z"
                }
            })))
            .mount(&server)
            .await;

        // Mock POST /workers returning worker and enrollment token
        Mock::given(method("POST"))
            .and(path("/workers"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {
                    "worker": {
                        "id": expected_worker_id,
                        "admin_id": "admin-1",
                        "email": form.email,
                        "name": form.name,
                        "device_identifier": null,
                        "device_info": null,
                        "role_type": form.role_type,
                        "is_active": true,
                        "status": "PENDING_ENROLLMENT",
                        "status_changed_at": null,
                        "consent_flags": {
                            "system_activity": false,
                            "network_activity": false,
                            "productivity_basic": false,
                            "productivity_screenshots": false,
                            "file_activity": false,
                            "location": false
                        },
                        "requested_consent_flags": form.consent_flags,
                        "consent_descriptions": form.normalized_descriptions(),
                        "consent_version": 1,
                        "accepted_consent_version": null,
                        "consent_accepted_at": null,
                        "enrollment_completed_at": null,
                        "enrollment_expires_at": "2026-09-30T10:00:00Z",
                        "work_hours_start": "08:30:00",
                        "work_hours_end": "17:30:00",
                        "timezone": "America/Santiago",
                        "created_at": "2026-09-28T10:00:00Z",
                        "updated_at": "2026-09-28T10:00:00Z"
                    },
                    "enrollment": {
                        "code": "K7QM4XRT9PWD",
                        "expires_at": "2026-09-30T10:00:00Z"
                    }
                }
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());

        let created = workers_create_with_client(&client, form.clone())
            .await
            .expect("workers_create failed");

        assert_eq!(created.worker.id, expected_worker_id);
        assert_eq!(created.enrollment.code, "K7QM4XRT9PWD");
        assert_eq!(created.enrollment.safety_code.len(), 14); // "dddd dddd dddd"

        // Verify the QR was built with local_pub by verifying commitment against local manifest
        let manifest = EnrollmentManifest::new(
            local_pub,
            created.worker.consent_version,
            created.worker.requested_consent_flags,
            created.worker.consent_descriptions,
        )
        .unwrap();

        let parsed_qr: EnrollmentQr = serde_json::from_str(&created.enrollment.qr_payload).unwrap();
        assert!(verify_commitment(&manifest, &parsed_qr.commitment));
    }

    #[tokio::test]
    async fn test_workers_create_tamper_check_rejects_modified_descriptions() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();

        let server = MockServer::start().await;
        let _ = generate_and_save_master_keys().unwrap();

        let form = make_valid_form();

        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "admin-1",
                    "email": "admin@acme.cl",
                    "name": "Admin",
                    "organization": "Acme",
                    "is_active": true,
                    "worker_count": 0,
                    "public_key": null,
                    "public_key_fingerprint": null,
                    "created_at": "2026-09-28T00:00:00Z",
                    "updated_at": "2026-09-28T00:00:00Z"
                }
            })))
            .mount(&server)
            .await;

        // Server maliciously returns different consent descriptions!
        let mut tampered_desc = form.normalized_descriptions();
        tampered_desc.system_activity = Some("Tampered description by attacker".into());

        Mock::given(method("POST"))
            .and(path("/workers"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {
                    "worker": {
                        "id": "worker-tampered-1",
                        "admin_id": "admin-1",
                        "email": form.email,
                        "name": form.name,
                        "device_identifier": null,
                        "device_info": null,
                        "role_type": form.role_type,
                        "is_active": true,
                        "status": "PENDING_ENROLLMENT",
                        "status_changed_at": null,
                        "consent_flags": {
                            "system_activity": false,
                            "network_activity": false,
                            "productivity_basic": false,
                            "productivity_screenshots": false,
                            "file_activity": false,
                            "location": false
                        },
                        "requested_consent_flags": form.consent_flags,
                        "consent_descriptions": tampered_desc, // Mismatch!
                        "consent_version": 1,
                        "accepted_consent_version": null,
                        "consent_accepted_at": null,
                        "enrollment_completed_at": null,
                        "enrollment_expires_at": "2026-09-30T10:00:00Z",
                        "work_hours_start": "08:30:00",
                        "work_hours_end": "17:30:00",
                        "timezone": "America/Santiago",
                        "created_at": "2026-09-28T10:00:00Z",
                        "updated_at": "2026-09-28T10:00:00Z"
                    },
                    "enrollment": {
                        "code": "K7QM4XRT9PWD",
                        "expires_at": "2026-09-30T10:00:00Z"
                    }
                }
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());

        let res = workers_create_with_client(&client, form).await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.code, "ENROLLMENT_TAMPER_CHECK_FAILED");
    }

    #[tokio::test]
    async fn test_server_key_mismatch_blocks_worker_creation() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();

        let server = MockServer::start().await;
        let _ = generate_and_save_master_keys().unwrap();

        // Server reports a different public key fingerprint (K2 mismatch)
        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "admin-1",
                    "email": "admin@acme.cl",
                    "name": "Admin",
                    "organization": "Acme",
                    "is_active": true,
                    "worker_count": 0,
                    "public_key": "different_pubkey_b64",
                    "public_key_fingerprint": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                    "created_at": "2026-09-28T00:00:00Z",
                    "updated_at": "2026-09-28T00:00:00Z"
                }
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());

        let form = make_valid_form();
        let res = workers_create_with_client(&client, form).await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.code, "PUBLIC_KEY_MISMATCH");
    }

    #[tokio::test]
    async fn test_enrollment_view_code_not_logged() {
        let _guard_keystore = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();

        use std::sync::{Arc, Mutex};
        #[derive(Clone, Default)]
        struct LogCapture(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for LogCapture {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let capture = LogCapture(buffer.clone());
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || capture.clone())
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let server = MockServer::start().await;
        let _ = generate_and_save_master_keys().unwrap();
        let form = make_valid_form();
        let secret_code = "SECR99ET9942";

        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "admin-1",
                    "email": form.email,
                    "name": form.name,
                    "organization": "Acme",
                    "is_active": true,
                    "worker_count": 0,
                    "public_key": null,
                    "public_key_fingerprint": null,
                    "created_at": "2026-09-28T00:00:00Z",
                    "updated_at": "2026-09-28T00:00:00Z"
                }
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/workers"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {
                    "worker": {
                        "id": "01912345-6789-7abc-def0-123456789aaa",
                        "admin_id": "admin-1",
                        "email": form.email,
                        "name": form.name,
                        "device_identifier": null,
                        "device_info": null,
                        "role_type": form.role_type,
                        "is_active": true,
                        "status": "PENDING_ENROLLMENT",
                        "status_changed_at": null,
                        "consent_flags": {
                            "system_activity": false,
                            "network_activity": false,
                            "productivity_basic": false,
                            "productivity_screenshots": false,
                            "file_activity": false,
                            "location": false
                        },
                        "requested_consent_flags": form.consent_flags,
                        "consent_descriptions": form.normalized_descriptions(),
                        "consent_version": 1,
                        "accepted_consent_version": null,
                        "consent_accepted_at": null,
                        "enrollment_completed_at": null,
                        "enrollment_expires_at": "2026-09-30T10:00:00Z",
                        "work_hours_start": "08:30:00",
                        "work_hours_end": "17:30:00",
                        "timezone": "America/Santiago",
                        "created_at": "2026-09-28T10:00:00Z",
                        "updated_at": "2026-09-28T10:00:00Z"
                    },
                    "enrollment": {
                        "code": secret_code,
                        "expires_at": "2026-09-30T10:00:00Z"
                    }
                }
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        let res = workers_create_with_client(&client, form).await.unwrap();
        assert_eq!(res.enrollment.code, secret_code);

        let logs = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert!(logs.contains("Worker created and enrollment QR issued successfully"));
        assert!(
            !logs.contains(secret_code),
            "Enrollment code leaked into logs!"
        );
    }

    #[tokio::test]
    async fn test_enrollment_qr_cross_app_compatibility() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();

        let server = MockServer::start().await;
        let admin_pub = generate_and_save_master_keys().unwrap();
        let form = make_valid_form();

        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "admin-1",
                    "email": form.email,
                    "name": form.name,
                    "organization": "Acme",
                    "is_active": true,
                    "worker_count": 0,
                    "public_key": null,
                    "public_key_fingerprint": null,
                    "created_at": "2026-09-28T00:00:00Z",
                    "updated_at": "2026-09-28T00:00:00Z"
                }
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/workers"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {
                    "worker": {
                        "id": "worker-cross-1",
                        "admin_id": "admin-1",
                        "email": form.email,
                        "name": form.name,
                        "device_identifier": null,
                        "device_info": null,
                        "role_type": form.role_type,
                        "is_active": true,
                        "status": "PENDING_ENROLLMENT",
                        "status_changed_at": null,
                        "consent_flags": {
                            "system_activity": false,
                            "network_activity": false,
                            "productivity_basic": false,
                            "productivity_screenshots": false,
                            "file_activity": false,
                            "location": false
                        },
                        "requested_consent_flags": form.consent_flags,
                        "consent_descriptions": form.normalized_descriptions(),
                        "consent_version": 1,
                        "accepted_consent_version": null,
                        "consent_accepted_at": null,
                        "enrollment_completed_at": null,
                        "enrollment_expires_at": "2026-09-30T10:00:00Z",
                        "work_hours_start": "08:30:00",
                        "work_hours_end": "17:30:00",
                        "timezone": "America/Santiago",
                        "created_at": "2026-09-28T10:00:00Z",
                        "updated_at": "2026-09-28T10:00:00Z"
                    },
                    "enrollment": {
                        "code": "C74J5MN89PQR",
                        "expires_at": "2026-09-30T10:00:00Z"
                    }
                }
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        let created = workers_create_with_client(&client, form).await.unwrap();

        // 1. Agent parses QR JSON payload
        let agent_parsed_qr: EnrollmentQr =
            serde_json::from_str(&created.enrollment.qr_payload).unwrap();
        assert_eq!(agent_parsed_qr.code, "C74J-5MN8-9PQR");

        // 2. Agent receives EnrollResponse from server and constructs local EnrollmentManifest
        let agent_manifest = EnrollmentManifest::new(
            admin_pub.clone(),
            created.worker.consent_version,
            created.worker.requested_consent_flags,
            created.worker.consent_descriptions.clone(),
        )
        .unwrap();

        // 3. Verification succeeds
        assert!(verify_commitment(
            &agent_manifest,
            &agent_parsed_qr.commitment
        ));

        // 4. Verification fails if someone tampered with the manifest (e.g. key or flags)
        let tampered_manifest = EnrollmentManifest::new(
            admin_pub,
            created.worker.consent_version,
            ConsentFlags {
                location: true,
                ..created.worker.requested_consent_flags
            },
            created.worker.consent_descriptions,
        )
        .unwrap();
        assert!(!verify_commitment(
            &tampered_manifest,
            &agent_parsed_qr.commitment
        ));
    }
}
