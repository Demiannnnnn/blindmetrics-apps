//! Employer legal profile, workplaces, attendance settings, and incidents (TASK-ADMIN-13).
//!
//! Enforces:
//! * RUT validation via `telemetry_crypto_core::rut` (mod 11, canonical format `76.123.456-7`).
//! * Chilean regions: closed set of the 16 official regions.
//! * Strict length bounds (PLAT-02: no fake data).
//! * Exempt from subscription blocks for read (`employer_get`) (BILL-04).

use crate::client::{get_api_client, AdminApiClient};
use crate::commands::CommandError;
use serde::{Deserialize, Serialize};

/// The 16 official administrative regions of Chile.
pub const OFFICIAL_REGIONS: [&str; 16] = [
    "Región de Arica y Parinacota",
    "Región de Tarapacá",
    "Región de Antofagasta",
    "Región de Atacama",
    "Región de Coquimbo",
    "Región de Valparaíso",
    "Región Metropolitana de Santiago",
    "Región del Libertador General Bernardo O'Higgins",
    "Región del Maule",
    "Región de Ñuble",
    "Región del Biobío",
    "Región de La Araucanía",
    "Región de Los Ríos",
    "Región de Los Lagos",
    "Región de Aysén del General Carlos Ibáñez del Campo",
    "Región de Magallanes y de la Antártica Chilena",
];

/// Validates a region string against the 16 official regions of Chile.
/// Normalizes aliases and missing prefixes to the official name.
pub fn validate_region(region: &str) -> Result<String, CommandError> {
    let trimmed = region.trim();
    if trimmed.is_empty() {
        return Err(CommandError::validation(
            "Debes seleccionar una región oficial de Chile.",
        ));
    }
    // Exact match (case-insensitive)
    if let Some(&official) = OFFICIAL_REGIONS
        .iter()
        .find(|&&r| r.eq_ignore_ascii_case(trimmed))
    {
        return Ok(official.to_string());
    }
    // Common aliases for Santiago Metropolitan Region
    if trimmed.eq_ignore_ascii_case("Región Metropolitana")
        || trimmed.eq_ignore_ascii_case("Metropolitana")
        || trimmed.eq_ignore_ascii_case("RM")
    {
        return Ok("Región Metropolitana de Santiago".to_string());
    }
    // Match without "Región de" or "Región del"
    for &official in &OFFICIAL_REGIONS {
        let stripped = official
            .strip_prefix("Región del ")
            .or_else(|| official.strip_prefix("Región de "))
            .unwrap_or(official);
        if stripped.eq_ignore_ascii_case(trimmed) {
            return Ok(official.to_string());
        }
    }

    Err(CommandError::validation(format!(
        "La región '{trimmed}' no es válida. Debe ser una de las 16 regiones oficiales de Chile."
    )))
}

/// Address structure used for employer headquarters and workplaces (api-contract §11).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Address {
    pub street: String,
    pub number: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub office: Option<String>,
    pub commune: String,
    pub city: String,
    pub region: String,
}

/// Validates street, number, commune, city and region lengths and values.
pub fn validate_address(addr: &Address) -> Result<Address, CommandError> {
    let street = addr.street.trim();
    if street.is_empty() || street.chars().count() > 200 {
        return Err(CommandError::validation(
            "La calle debe tener entre 1 y 200 caracteres.",
        ));
    }
    let number = addr.number.trim();
    if number.is_empty() || number.chars().count() > 50 {
        return Err(CommandError::validation(
            "El número de calle debe tener entre 1 y 50 caracteres.",
        ));
    }
    let floor = addr
        .floor
        .as_ref()
        .map(|f| f.trim())
        .filter(|f| !f.is_empty())
        .map(|f| {
            if f.chars().count() > 50 {
                Err(CommandError::validation(
                    "El piso no puede superar 50 caracteres.",
                ))
            } else {
                Ok(f.to_string())
            }
        })
        .transpose()?;
    let office = addr
        .office
        .as_ref()
        .map(|o| o.trim())
        .filter(|o| !o.is_empty())
        .map(|o| {
            if o.chars().count() > 50 {
                Err(CommandError::validation(
                    "La oficina/depto no puede superar 50 caracteres.",
                ))
            } else {
                Ok(o.to_string())
            }
        })
        .transpose()?;
    let commune = addr.commune.trim();
    if commune.is_empty() || commune.chars().count() > 100 {
        return Err(CommandError::validation(
            "La comuna debe tener entre 1 y 100 caracteres.",
        ));
    }
    let city = addr.city.trim();
    if city.is_empty() || city.chars().count() > 100 {
        return Err(CommandError::validation(
            "La ciudad debe tener entre 1 y 100 caracteres.",
        ));
    }
    let valid_region = validate_region(&addr.region)?;
    Ok(Address {
        street: street.to_string(),
        number: number.to_string(),
        floor,
        office,
        commune: commune.to_string(),
        city: city.to_string(),
        region: valid_region,
    })
}

/// Kind of legal entity or natural person.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EmployerKind {
    LegalEntity,
    NaturalPerson,
}

/// Employer legal profile (api-contract §11).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EmployerProfile {
    pub kind: EmployerKind,
    pub legal_name: String,
    pub rut: String,
    pub address: Address,
}

/// Validates employer legal profile form fields in Rust.
pub fn validate_employer_profile(form: &EmployerProfile) -> Result<EmployerProfile, CommandError> {
    let legal_name = form.legal_name.trim();
    if legal_name.chars().count() < 2 || legal_name.chars().count() > 255 {
        return Err(CommandError::validation(
            "La razón social o nombre debe tener entre 2 y 255 caracteres.",
        ));
    }
    let parsed_rut = telemetry_crypto_core::rut::parse(&form.rut)
        .map_err(|e| CommandError::validation(format!("RUT inválido: {e}")))?;
    let canonical_rut = parsed_rut.format();
    let validated_address = validate_address(&form.address)?;

    Ok(EmployerProfile {
        kind: form.kind.clone(),
        legal_name: legal_name.to_string(),
        rut: canonical_rut,
        address: validated_address,
    })
}

/// Workplace establishment (api-contract §11, R38 art. 13).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Workplace {
    pub id: String,
    pub name: String,
    pub address: Address,
    #[serde(default)]
    pub assigned_workers_count: Option<usize>,
}

/// Form payload for creating/updating a workplace.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WorkplaceForm {
    pub name: String,
    pub address: Address,
}

/// Validates workplace form fields.
pub fn validate_workplace_form(form: &WorkplaceForm) -> Result<WorkplaceForm, CommandError> {
    let name = form.name.trim();
    if name.chars().count() < 2 || name.chars().count() > 100 {
        return Err(CommandError::validation(
            "El nombre del lugar de trabajo debe tener entre 2 y 100 caracteres.",
        ));
    }
    let validated_address = validate_address(&form.address)?;
    Ok(WorkplaceForm {
        name: name.to_string(),
        address: validated_address,
    })
}

fn default_true() -> bool {
    true
}

/// Attendance settings (api-contract §9.4, R38 art. 45.1).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AttendanceSettings {
    #[serde(default = "default_true")]
    pub missing_mark_alerts: bool,
}

/// Technical incident record (api-contract §9.4, R38 art. 27.6).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TechnicalIncident {
    pub id: String,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Form to report a new technical incident.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CreateIncidentForm {
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    pub description: String,
}

/// Validates technical incident inputs.
pub fn validate_incident_form(
    form: &CreateIncidentForm,
) -> Result<CreateIncidentForm, CommandError> {
    let desc = form.description.trim();
    if desc.chars().count() < 5 || desc.chars().count() > 1000 {
        return Err(CommandError::validation(
            "La descripción del incidente debe tener entre 5 y 1000 caracteres.",
        ));
    }
    let start_dt = chrono::DateTime::parse_from_rfc3339(form.started_at.trim()).map_err(|_| {
        CommandError::validation(
            "La fecha/hora de inicio debe estar en formato ISO 8601 / RFC 3339.",
        )
    })?;

    let ended_at = if let Some(end) = &form.ended_at {
        let end_trimmed = end.trim();
        if end_trimmed.is_empty() {
            None
        } else {
            let end_dt = chrono::DateTime::parse_from_rfc3339(end_trimmed).map_err(|_| {
                CommandError::validation(
                    "La fecha/hora de término debe estar en formato ISO 8601 / RFC 3339.",
                )
            })?;
            if end_dt < start_dt {
                return Err(CommandError::validation(
                    "La fecha de término no puede ser anterior a la de inicio.",
                ));
            }
            Some(end_trimmed.to_string())
        }
    } else {
        None
    };

    Ok(CreateIncidentForm {
        started_at: form.started_at.trim().to_string(),
        ended_at,
        description: desc.to_string(),
    })
}

// ============================================================================
// Command Implementations
// ============================================================================

/// `GET /admins/me/employer` (RF-ADMIN-13-4, BILL-04).
/// Returns `Ok(None)` if no employer profile exists yet (`404` or `EMPLOYER_PROFILE_MISSING`).
pub async fn employer_get_with_client(
    client: &AdminApiClient,
) -> Result<Option<EmployerProfile>, CommandError> {
    match client
        .get_json::<EmployerProfile>("/admins/me/employer")
        .await
    {
        Ok(profile) => Ok(Some(profile)),
        Err(err)
            if err.status == 404
                || err.code == "RESOURCE_NOT_FOUND"
                || err.has_reason("EMPLOYER_PROFILE_MISSING") =>
        {
            Ok(None)
        }
        Err(err) => Err(CommandError::from(err)),
    }
}

#[tauri::command]
pub async fn employer_get() -> Result<Option<EmployerProfile>, CommandError> {
    employer_get_with_client(get_api_client()).await
}

/// `PUT /admins/me/employer` (RF-ADMIN-13-4).
pub async fn employer_update_with_client(
    client: &AdminApiClient,
    form: EmployerProfile,
) -> Result<EmployerProfile, CommandError> {
    let validated = validate_employer_profile(&form)?;
    let saved: EmployerProfile = client.put_json("/admins/me/employer", &validated).await?;
    Ok(saved)
}

#[tauri::command]
pub async fn employer_update(form: EmployerProfile) -> Result<EmployerProfile, CommandError> {
    employer_update_with_client(get_api_client(), form).await
}

/// `GET /admins/me/workplaces` (RF-ADMIN-13-4).
pub async fn workplaces_list_with_client(
    client: &AdminApiClient,
) -> Result<Vec<Workplace>, CommandError> {
    let res = client
        .get_collection::<Workplace>("/admins/me/workplaces")
        .await?;
    Ok(res.data)
}

#[tauri::command]
pub async fn workplaces_list() -> Result<Vec<Workplace>, CommandError> {
    workplaces_list_with_client(get_api_client()).await
}

/// `POST /admins/me/workplaces` (RF-ADMIN-13-4).
pub async fn workplaces_create_with_client(
    client: &AdminApiClient,
    form: WorkplaceForm,
) -> Result<Workplace, CommandError> {
    let validated = validate_workplace_form(&form)?;
    let created: Workplace = client
        .post_json("/admins/me/workplaces", &validated)
        .await?;
    Ok(created)
}

#[tauri::command]
pub async fn workplaces_create(form: WorkplaceForm) -> Result<Workplace, CommandError> {
    workplaces_create_with_client(get_api_client(), form).await
}

/// `PUT /admins/me/workplaces/:id` (RF-ADMIN-13-4).
pub async fn workplaces_update_with_client(
    client: &AdminApiClient,
    id: String,
    form: WorkplaceForm,
) -> Result<Workplace, CommandError> {
    let id_trimmed = id.trim();
    if id_trimmed.is_empty() {
        return Err(CommandError::validation("El ID de lugar es requerido."));
    }
    let validated = validate_workplace_form(&form)?;
    let path = format!("/admins/me/workplaces/{id_trimmed}");
    let updated: Workplace = client.put_json(&path, &validated).await?;
    Ok(updated)
}

#[tauri::command]
pub async fn workplaces_update(id: String, form: WorkplaceForm) -> Result<Workplace, CommandError> {
    workplaces_update_with_client(get_api_client(), id, form).await
}

/// `DELETE /admins/me/workplaces/:id` (RF-ADMIN-13-4).
pub async fn workplaces_delete_with_client(
    client: &AdminApiClient,
    id: String,
) -> Result<(), CommandError> {
    let id_trimmed = id.trim();
    if id_trimmed.is_empty() {
        return Err(CommandError::validation("El ID de lugar es requerido."));
    }
    let path = format!("/admins/me/workplaces/{id_trimmed}");
    client.delete(&path).await?;
    Ok(())
}

#[tauri::command]
pub async fn workplaces_delete(id: String) -> Result<(), CommandError> {
    workplaces_delete_with_client(get_api_client(), id).await
}

/// `GET /attendance/settings` (RF-ADMIN-13-4).
pub async fn attendance_settings_get_with_client(
    client: &AdminApiClient,
) -> Result<AttendanceSettings, CommandError> {
    match client
        .get_json::<AttendanceSettings>("/attendance/settings")
        .await
    {
        Ok(settings) => Ok(settings),
        Err(err) if err.status == 404 => Ok(AttendanceSettings {
            missing_mark_alerts: true,
        }),
        Err(err) => Err(CommandError::from(err)),
    }
}

#[tauri::command]
pub async fn attendance_settings_get() -> Result<AttendanceSettings, CommandError> {
    attendance_settings_get_with_client(get_api_client()).await
}

/// `PUT /attendance/settings` (RF-ADMIN-13-4).
pub async fn attendance_settings_update_with_client(
    client: &AdminApiClient,
    form: AttendanceSettings,
) -> Result<AttendanceSettings, CommandError> {
    let updated: AttendanceSettings = client.put_json("/attendance/settings", &form).await?;
    Ok(updated)
}

#[tauri::command]
pub async fn attendance_settings_update(
    form: AttendanceSettings,
) -> Result<AttendanceSettings, CommandError> {
    attendance_settings_update_with_client(get_api_client(), form).await
}

/// `GET /attendance/incidents` (RF-ADMIN-13-4).
pub async fn incidents_list_with_client(
    client: &AdminApiClient,
) -> Result<Vec<TechnicalIncident>, CommandError> {
    let res = client
        .get_collection::<TechnicalIncident>("/attendance/incidents")
        .await?;
    Ok(res.data)
}

#[tauri::command]
pub async fn incidents_list() -> Result<Vec<TechnicalIncident>, CommandError> {
    incidents_list_with_client(get_api_client()).await
}

/// `POST /attendance/incidents` (RF-ADMIN-13-4).
pub async fn incidents_create_with_client(
    client: &AdminApiClient,
    form: CreateIncidentForm,
) -> Result<TechnicalIncident, CommandError> {
    let validated = validate_incident_form(&form)?;
    let created: TechnicalIncident = client
        .post_json("/attendance/incidents", &validated)
        .await?;
    Ok(created)
}

#[tauri::command]
pub async fn incidents_create(form: CreateIncidentForm) -> Result<TechnicalIncident, CommandError> {
    incidents_create_with_client(get_api_client(), form).await
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_valid_address() -> Address {
        Address {
            street: "Av. Apoquindo".to_string(),
            number: "1234".to_string(),
            floor: Some("5".to_string()),
            office: Some("501".to_string()),
            commune: "Las Condes".to_string(),
            city: "Santiago".to_string(),
            region: "Región Metropolitana de Santiago".to_string(),
        }
    }

    #[test]
    fn test_region_validation_official_and_aliases() {
        assert_eq!(
            validate_region("Región de Valparaíso").unwrap(),
            "Región de Valparaíso"
        );
        assert_eq!(
            validate_region("Valparaíso").unwrap(),
            "Región de Valparaíso"
        );
        assert_eq!(
            validate_region("Región Metropolitana").unwrap(),
            "Región Metropolitana de Santiago"
        );
        assert_eq!(
            validate_region("Metropolitana").unwrap(),
            "Región Metropolitana de Santiago"
        );
        assert_eq!(
            validate_region("Arica y Parinacota").unwrap(),
            "Región de Arica y Parinacota"
        );
        assert_eq!(
            validate_region("Región del Biobío").unwrap(),
            "Región del Biobío"
        );

        // Invalid regions rejected
        assert!(validate_region("Provincia de Mendoza").is_err());
        assert!(validate_region("").is_err());
        assert!(validate_region("Región 99").is_err());
    }

    #[test]
    fn test_address_validation_bounds() {
        let mut addr = sample_valid_address();
        assert!(validate_address(&addr).is_ok());

        // Empty street rejected
        addr.street = "".to_string();
        assert!(validate_address(&addr).is_err());

        // Street too long (>200 chars)
        addr.street = "A".repeat(201);
        assert!(validate_address(&addr).is_err());
    }

    #[tokio::test]
    async fn test_employer_update_invalid_rut_rejected_locally() {
        let server = MockServer::start().await;
        let client = AdminApiClient::new(server.uri());

        let invalid_profile = EmployerProfile {
            kind: EmployerKind::LegalEntity,
            legal_name: "Empresa Ficticia SpA".to_string(),
            rut: "76.123.456-9".to_string(), // Invalid DV (expected 0)
            address: sample_valid_address(),
        };

        // Call update: must fail in validation BEFORE sending any network request
        let err = employer_update_with_client(&client, invalid_profile)
            .await
            .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert!(err.message.contains("RUT inválido"));

        // Verify MockServer received 0 requests
        assert_eq!(server.received_requests().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_employer_get_and_put_valid() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;

        let sample_profile = EmployerProfile {
            kind: EmployerKind::LegalEntity,
            legal_name: "Tecnología y Servicios SpA".to_string(),
            rut: "76.123.456-0".to_string(),
            address: sample_valid_address(),
        };

        Mock::given(method("GET"))
            .and(path("/admins/me/employer"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": sample_profile
            })))
            .mount(&server)
            .await;

        Mock::given(method("PUT"))
            .and(path("/admins/me/employer"))
            .and(body_json(serde_json::json!(sample_profile)))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": sample_profile
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("test-token-employer".to_string()));

        let fetched = employer_get_with_client(&client).await.unwrap();
        assert_eq!(fetched, Some(sample_profile.clone()));

        let updated = employer_update_with_client(&client, sample_profile.clone())
            .await
            .unwrap();
        assert_eq!(updated, sample_profile);
    }

    #[tokio::test]
    async fn test_employer_get_404_returns_none() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/admins/me/employer"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "error": {
                    "code": "RESOURCE_NOT_FOUND",
                    "message": "Employer profile not found",
                    "details": { "reason": "EMPLOYER_PROFILE_MISSING" }
                }
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("test-token-employer".to_string()));

        let fetched = employer_get_with_client(&client).await.unwrap();
        assert_eq!(fetched, None);
    }

    #[tokio::test]
    async fn test_workplaces_crud() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;

        let workplace = Workplace {
            id: "wp-123".to_string(),
            name: "Casa Matriz".to_string(),
            address: sample_valid_address(),
            assigned_workers_count: Some(10),
        };

        // List
        Mock::given(method("GET"))
            .and(path("/admins/me/workplaces"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [workplace],
                "pagination": { "total": 1, "page": 1, "per_page": 20, "total_pages": 1 }
            })))
            .mount(&server)
            .await;

        // Create
        Mock::given(method("POST"))
            .and(path("/admins/me/workplaces"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": workplace
            })))
            .mount(&server)
            .await;

        // Update
        Mock::given(method("PUT"))
            .and(path("/admins/me/workplaces/wp-123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": workplace
            })))
            .mount(&server)
            .await;

        // Delete
        Mock::given(method("DELETE"))
            .and(path("/admins/me/workplaces/wp-123"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("test-token-wp".to_string()));

        let list = workplaces_list_with_client(&client).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Casa Matriz");

        let form = WorkplaceForm {
            name: "Casa Matriz".to_string(),
            address: sample_valid_address(),
        };
        let created = workplaces_create_with_client(&client, form.clone())
            .await
            .unwrap();
        assert_eq!(created.id, "wp-123");

        let updated = workplaces_update_with_client(&client, "wp-123".to_string(), form)
            .await
            .unwrap();
        assert_eq!(updated.id, "wp-123");

        let deleted = workplaces_delete_with_client(&client, "wp-123".to_string()).await;
        assert!(deleted.is_ok());
    }

    #[tokio::test]
    async fn test_attendance_settings_and_incidents() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;

        // Settings PUT
        Mock::given(method("PUT"))
            .and(path("/attendance/settings"))
            .and(body_json(
                serde_json::json!({ "missing_mark_alerts": false }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "missing_mark_alerts": false }
            })))
            .mount(&server)
            .await;

        // Incident POST
        let incident = TechnicalIncident {
            id: "inc-1".to_string(),
            started_at: "2026-10-11T10:00:00Z".to_string(),
            ended_at: Some("2026-10-11T12:00:00Z".to_string()),
            description: "Corte de energía en datacenter".to_string(),
            created_at: Some("2026-10-11T12:05:00Z".to_string()),
        };

        Mock::given(method("POST"))
            .and(path("/attendance/incidents"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": incident
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("test-token-settings".to_string()));

        let updated_settings = attendance_settings_update_with_client(
            &client,
            AttendanceSettings {
                missing_mark_alerts: false,
            },
        )
        .await
        .unwrap();
        assert!(!updated_settings.missing_mark_alerts);

        let inc_form = CreateIncidentForm {
            started_at: "2026-10-11T10:00:00Z".to_string(),
            ended_at: Some("2026-10-11T12:00:00Z".to_string()),
            description: "Corte de energía en datacenter".to_string(),
        };
        let created_inc = incidents_create_with_client(&client, inc_form)
            .await
            .unwrap();
        assert_eq!(created_inc.id, "inc-1");
    }
}
