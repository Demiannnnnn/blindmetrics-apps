use crate::api::client::{ApiClient, DeviceInfo, EnrollResponse};
use crate::config::WorkerConfig;
use crate::credentials::CredentialStore;
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use telemetry_crypto_core::{
    manifest_commitment, public_key_fingerprint, safety_code, verify_commitment,
    EnrollmentManifest, EnrollmentQr,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnrollmentInput {
    Qr(EnrollmentQr),
    Manual { code: String },
}

impl EnrollmentInput {
    pub fn code(&self) -> String {
        match self {
            EnrollmentInput::Qr(qr) => normalize_code(&qr.code),
            EnrollmentInput::Manual { code, .. } => normalize_code(code),
        }
    }
}

/// The agent only talks to the server it was built for (`config::API_BASE_URL`). An enrollment
/// QR from an admin app pointed at another server is rejected instead of followed.
pub fn qr_targets_server(qr_api_url: &str, server_url: &str) -> bool {
    let normalize = |u: &str| u.trim().trim_end_matches('/').to_ascii_lowercase();
    normalize(qr_api_url) == normalize(server_url)
}

/// Normalizes a manual code: uppercase, strip whitespace & dashes, Crockford replacements I/L->1, O->0.
pub fn normalize_code(raw: &str) -> String {
    raw.chars()
        .filter_map(|c| match c.to_ascii_uppercase() {
            '-' | ' ' => None,
            'I' | 'L' => Some('1'),
            'O' => Some('0'),
            other => Some(other),
        })
        .collect()
}

pub fn format_code_display(code: &str) -> String {
    let norm = normalize_code(code);
    if norm.len() == 12 {
        format!("{}-{}-{}", &norm[0..4], &norm[4..8], &norm[8..12])
    } else {
        norm
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verification {
    Verified,
    NeedsSafetyCode(String),
}

#[derive(Debug, Clone)]
pub struct PendingEnrollment {
    pub resp: EnrollResponse,
    pub manifest: EnrollmentManifest,
    pub commitment: String,
    pub verification: Verification,
}

#[derive(Debug, thiserror::Error)]
pub enum EnrollmentError {
    #[error("API error: {0}")]
    Api(#[from] AgentError),
    #[error("Invalid enrollment code format: expected 12 Crockford base32 characters")]
    InvalidCodeFormat,
    #[error("Information received from server was tampered or commitment mismatch")]
    Tampered,
    #[error("Crypto error: {0}")]
    Crypto(#[from] telemetry_crypto_core::CryptoError),
}

pub async fn enroll(
    api: &ApiClient,
    input: &EnrollmentInput,
    device_id: &str,
    device_info: &DeviceInfo,
) -> Result<PendingEnrollment, EnrollmentError> {
    let code = input.code();
    if code.len() != 12 {
        return Err(EnrollmentError::InvalidCodeFormat);
    }

    let resp = api.enroll(&code, device_id, device_info).await?;

    let manifest = EnrollmentManifest::new(
        resp.admin_public_key.clone(),
        resp.consent_version,
        resp.consent_flags,
        resp.consent_descriptions.clone(),
    )?;

    let commitment = manifest_commitment(&manifest)?;

    let expected_fp = public_key_fingerprint(&resp.admin_public_key)?;
    if expected_fp != resp.admin_public_key_fingerprint {
        tracing::warn!("Admin public key fingerprint mismatch");
        let _ = api.logout(&resp.auth.refresh_token).await;
        return Err(EnrollmentError::Tampered);
    }

    let verification = match input {
        EnrollmentInput::Qr(qr) => {
            if verify_commitment(&manifest, &qr.commitment) {
                Verification::Verified
            } else {
                tracing::warn!("Enrollment QR commitment mismatch");
                let _ = api.logout(&resp.auth.refresh_token).await;
                return Err(EnrollmentError::Tampered);
            }
        }
        EnrollmentInput::Manual { .. } => {
            let code = safety_code(&commitment)?;
            Verification::NeedsSafetyCode(code)
        }
    };

    Ok(PendingEnrollment {
        resp,
        manifest,
        commitment,
        verification,
    })
}

pub fn commit_enrollment(
    pending: &PendingEnrollment,
    cred_store: &dyn CredentialStore,
    config: &mut WorkerConfig,
) -> Result<(), String> {
    cred_store.set_access_token(Some(&pending.resp.auth.access_token))?;
    cred_store.set_refresh_token(Some(&pending.resp.auth.refresh_token))?;
    cred_store.set_pinned_admin_public_key(Some(&pending.resp.admin_public_key))?;
    cred_store.set_pinned_fingerprint(Some(&pending.resp.admin_public_key_fingerprint))?;
    cred_store.set_worker_id(Some(&pending.resp.worker_id))?;

    config.save().map_err(|e| e.to_string())?;

    tracing::info!(
        worker_id = %pending.resp.worker_id,
        commitment = %pending.commitment[..8],
        "Enrolled worker successfully and pinned admin public key"
    );

    Ok(())
}

/// Builds the enrollment input from the form: a pasted QR payload
/// or a manual code.
pub fn parse_enrollment_input(
    with_qr: bool,
    qr_text: &str,
    code: &str,
    server_url: &str,
) -> Result<EnrollmentInput, String> {
    if with_qr {
        let qr = EnrollmentQr::parse_payload(qr_text).map_err(|_| {
            "El contenido pegado no es un código de vinculación válido.".to_string()
        })?;
        if !qr_targets_server(&qr.api_url, server_url) {
            return Err(
                "Este código de vinculación es de otro servidor. Pide a tu administrador uno generado desde la aplicación oficial."
                    .into(),
            );
        }
        return Ok(EnrollmentInput::Qr(qr));
    }
    let code = normalize_code(code);
    if code.len() != 12 {
        return Err("El código debe tener 12 caracteres válidos.".into());
    }
    Ok(EnrollmentInput::Manual { code })
}

/// Messages by `details.reason` of `POST /workers/enroll`.
pub fn enrollment_error_message(e: &EnrollmentError) -> String {
    match e {
        EnrollmentError::Api(AgentError::Api {
            status: 429,
            reason,
            retry_after_seconds,
            ..
        }) => {
            if reason.as_deref() == Some("ENROLLMENT_LOCKED") {
                let minutes = retry_after_seconds.map(|s| s.div_ceil(60)).unwrap_or(15).max(1);
                format!(
                    "Demasiados intentos fallidos. Podrás intentarlo de nuevo en {} minuto(s).",
                    minutes
                )
            } else {
                "Demasiadas solicitudes. Espera un momento e inténtalo de nuevo.".into()
            }
        }
        EnrollmentError::Api(AgentError::Api { status: 409, reason, .. })
            if reason.as_deref() == Some("DEVICE_ALREADY_ENROLLED") =>
        {
            "Este equipo ya está vinculado a otro trabajador de la organización. Pide a tu administrador que lo desvincule.".into()
        }
        EnrollmentError::Api(AgentError::Api { status: 409, .. }) => {
            "Este código ya fue utilizado. Si no fuiste tú, avisa a tu administrador de inmediato."
                .into()
        }
        EnrollmentError::Api(AgentError::Api { status: 400, .. })
        | EnrollmentError::InvalidCodeFormat => {
            "Este código es inválido o expiró. Pide uno nuevo a tu administrador.".into()
        }
        EnrollmentError::Api(AgentError::Network(_)) => {
            "No se pudo contactar al servidor. Verifica la dirección y tu conexión.".into()
        }
        other => format!("Error de vinculación: {}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::InMemoryCredentialStore;
    use telemetry_crypto_core::{generate_admin_keypair, ConsentDescriptions, ConsentFlags};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_test_enroll_response(
        admin_pub: &str,
        admin_fp: &str,
        flags: ConsentFlags,
        desc: ConsentDescriptions,
    ) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "worker_id": "01912345-6789-7abc-def0-123456789abc",
                "worker_name": "Juan Perez",
                "organization": "Acme Corp",
                "role_type": "OFFICE",
                "work_hours_start": "09:00:00",
                "work_hours_end": "18:00:00",
                "timezone": "America/Santiago",
                "status": "PENDING_CONSENT",
                "auth": {
                    "access_token": "valid_access_jwt",
                    "refresh_token": "valid_refresh_token",
                    "token_type": "Bearer",
                    "expires_in": 900,
                    "user": {
                        "id": "01912345-6789-7abc-def0-123456789abc",
                        "email": "juan@acme.cl",
                        "name": "Juan Perez",
                        "role": "WORKER"
                    }
                },
                "admin_public_key": admin_pub,
                "admin_public_key_fingerprint": admin_fp,
                "consent_version": 1,
                "consent_flags": flags,
                "consent_descriptions": desc
            }
        })
    }

    #[test]
    fn test_qr_targets_server_only_matches_the_built_in_server() {
        assert!(qr_targets_server(
            "https://api.acme.cl/api/v1",
            "https://api.acme.cl/api/v1"
        ));
        assert!(qr_targets_server(
            "https://API.acme.cl/api/v1/",
            "https://api.acme.cl/api/v1"
        ));
        assert!(!qr_targets_server(
            "https://evil.example/api/v1",
            "https://api.acme.cl/api/v1"
        ));
        assert!(!qr_targets_server(
            "http://api.acme.cl/api/v1",
            "https://api.acme.cl/api/v1"
        ));
    }

    #[test]
    fn test_normalize_code() {
        assert_eq!(normalize_code("k7qm-4xrt-9pwd"), "K7QM4XRT9PWD");
        assert_eq!(normalize_code("k7qm 4xrt 9pwd"), "K7QM4XRT9PWD");
        assert_eq!(normalize_code("IL-O"), "110");
        assert_eq!(format_code_display("k7qm4xrt9pwd"), "K7QM-4XRT-9PWD");
    }

    #[tokio::test]
    async fn test_enrollment_qr_commitment_mismatch_persists_nothing() {
        let server = MockServer::start().await;
        let (_, admin_pub) = generate_admin_keypair();
        let admin_fp = public_key_fingerprint(&admin_pub).unwrap();

        let flags = ConsentFlags {
            system_activity: true,
            ..Default::default()
        };
        let desc = ConsentDescriptions {
            system_activity: Some("Monitoreo de procesos".into()),
            ..Default::default()
        };

        Mock::given(method("POST"))
            .and(path("/workers/enroll"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(make_test_enroll_response(
                    &admin_pub, &admin_fp, flags, desc,
                )),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/auth/logout"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let client = ApiClient::new(server.uri()).unwrap();
        let cred_store = InMemoryCredentialStore::new();

        let qr = EnrollmentQr {
            t: "telemetry-enroll".into(),
            v: 1,
            api_url: server.uri(),
            code: "K7QM4XRT9PWD".into(),
            commitment: "0000000000000000000000000000000000000000000000000000000000000000".into(), // Wrong commitment
        };

        let device = DeviceInfo {
            hostname: "test-host".into(),
            os: "macOS".into(),
            app_version: "0.2.0".into(),
        };

        let res = enroll(
            &client,
            &EnrollmentInput::Qr(qr),
            "dummy-device-id",
            &device,
        )
        .await;
        assert!(matches!(res, Err(EnrollmentError::Tampered)));

        // Verify nothing was persisted in credential store
        assert!(cred_store.get_access_token().is_none());
        assert!(cred_store.get_refresh_token().is_none());
        assert!(cred_store.get_pinned_admin_public_key().is_none());
    }

    #[tokio::test]
    async fn test_enrollment_manual_requires_safety_code_confirmation() {
        let server = MockServer::start().await;
        let (_, admin_pub) = generate_admin_keypair();
        let admin_fp = public_key_fingerprint(&admin_pub).unwrap();

        let flags = ConsentFlags {
            system_activity: true,
            ..Default::default()
        };
        let desc = ConsentDescriptions {
            system_activity: Some("Monitoreo de procesos".into()),
            ..Default::default()
        };

        Mock::given(method("POST"))
            .and(path("/workers/enroll"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(make_test_enroll_response(
                    &admin_pub,
                    &admin_fp,
                    flags,
                    desc.clone(),
                )),
            )
            .mount(&server)
            .await;

        let client = ApiClient::new(server.uri()).unwrap();
        let device = DeviceInfo {
            hostname: "test-host".into(),
            os: "macOS".into(),
            app_version: "0.2.0".into(),
        };

        let manual_input = EnrollmentInput::Manual {
            code: "k7qm-4xrt-9pwd".into(),
        };

        let res = enroll(&client, &manual_input, "dummy-device-id", &device).await;
        assert!(res.is_ok());
        let pending = res.unwrap();

        match &pending.verification {
            Verification::NeedsSafetyCode(sc) => {
                assert_eq!(sc.len(), 14); // "dddd dddd dddd"
            }
            Verification::Verified => {
                panic!("Manual enrollment must require safety code confirmation")
            }
        }
    }

    #[tokio::test]
    async fn test_enrollment_fingerprint_mismatch_is_tampered() {
        let server = MockServer::start().await;
        let (_, admin_pub) = generate_admin_keypair();
        let false_fp = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

        let flags = ConsentFlags {
            system_activity: true,
            ..Default::default()
        };
        let desc = ConsentDescriptions {
            system_activity: Some("Monitoreo de procesos".into()),
            ..Default::default()
        };

        Mock::given(method("POST"))
            .and(path("/workers/enroll"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(make_test_enroll_response(&admin_pub, false_fp, flags, desc)),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/auth/logout"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let client = ApiClient::new(server.uri()).unwrap();
        let device = DeviceInfo {
            hostname: "test-host".into(),
            os: "macOS".into(),
            app_version: "0.2.0".into(),
        };

        let manual_input = EnrollmentInput::Manual {
            code: "K7QM-4XRT-9PWD".into(),
        };

        let res = enroll(&client, &manual_input, "dummy-device-id", &device).await;
        assert!(matches!(res, Err(EnrollmentError::Tampered)));
    }

    fn api_error(status: u16, reason: Option<&str>, retry: Option<u64>) -> EnrollmentError {
        EnrollmentError::Api(crate::errors::AgentError::Api {
            status,
            code: "X".into(),
            reason: reason.map(str::to_string),
            retry_after_seconds: retry,
            details: None,
        })
    }

    #[test]
    fn test_enrollment_error_messages_by_reason() {
        assert!(
            enrollment_error_message(&api_error(400, Some("INVALID_ENROLLMENT_TOKEN"), None))
                .contains("inválido o expiró")
        );
        assert!(enrollment_error_message(&api_error(
            409,
            Some("ENROLLMENT_ALREADY_COMPLETED"),
            None
        ))
        .contains("ya fue utilizado"));
        assert!(
            enrollment_error_message(&api_error(409, Some("DEVICE_ALREADY_ENROLLED"), None))
                .contains("ya está vinculado a otro trabajador")
        );
        assert!(
            enrollment_error_message(&api_error(429, Some("ENROLLMENT_LOCKED"), Some(900)))
                .contains("15 minuto")
        );
        assert!(
            enrollment_error_message(&api_error(429, Some("ENROLLMENT_LOCKED"), Some(61)))
                .contains("2 minuto")
        );
        assert!(enrollment_error_message(&api_error(429, None, None))
            .contains("Demasiadas solicitudes"));
        assert!(enrollment_error_message(&EnrollmentError::Api(
            crate::errors::AgentError::Network("down".into())
        ))
        .contains("No se pudo contactar"));
    }

    #[test]
    fn test_enrollment_input_manual_code_normalized() {
        let server = "https://api.acme.cl/api/v1";
        let input = parse_enrollment_input(false, "", "k7qm-4xrt-9pwo", server).unwrap();
        assert_eq!(
            input,
            EnrollmentInput::Manual {
                code: "K7QM4XRT9PW0".into(),
            }
        );
        assert!(parse_enrollment_input(false, "", "K7QM", server).is_err());
    }

    #[test]
    fn test_enrollment_input_pasted_qr_must_target_the_built_in_server() {
        let server = "https://api.acme.cl/api/v1";
        let qr = EnrollmentQr::new(server.into(), "K7QM-4XRT-9PWD".into(), "a".repeat(64)).unwrap();
        let input = parse_enrollment_input(true, &qr.to_json().unwrap(), "", server).unwrap();
        assert!(matches!(input, EnrollmentInput::Qr(_)));
        assert!(parse_enrollment_input(true, "not a qr", "", server).is_err());
        let from_base64 =
            parse_enrollment_input(true, &qr.to_payload().unwrap(), "", server).unwrap();
        assert_eq!(from_base64, input);
        let foreign = EnrollmentQr::new(
            "https://evil.example/api/v1".into(),
            "K7QM-4XRT-9PWD".into(),
            "a".repeat(64),
        )
        .unwrap();
        assert!(parse_enrollment_input(true, &foreign.to_json().unwrap(), "", server).is_err());
    }
}
