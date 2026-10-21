//! Worker management (TASK-ADMIN-03, TASK-ADMIN-08, TASK-ADMIN-09).
//!
//! Enrollment packages from the API (`POST /workers`, `POST /workers/:id/enrollment-token`)
//! are shown only after the admin app recomputes the commitment with its **local** key and
//! checks the server's commitment, safety code and `qr_payload` (SEC-07, RF-ADMIN-09-1).

use super::keys::{ensure_server_key_with_client, get_admin_public_key};
use super::CommandError;
use crate::client::{get_api_client, AdminApiClient, CollectionResponse, PaginationMeta};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use telemetry_crypto_core::enrollment::{
    manifest_commitment, safety_code, EnrollmentManifest, EnrollmentQr,
};
use telemetry_crypto_core::{ConsentDescriptions, ConsentFlags};

/// Work modality (`api-contract.md` §11). Closed set: unknown values fail to deserialize.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkMode {
    OnSite,
    Remote,
    Hybrid,
}

impl WorkMode {
    /// Case-insensitive parse used by list filters and update forms.
    pub fn parse(raw: &str) -> Result<Self, CommandError> {
        match raw.trim().to_uppercase().as_str() {
            "ON_SITE" => Ok(Self::OnSite),
            "REMOTE" => Ok(Self::Remote),
            "HYBRID" => Ok(Self::Hybrid),
            _ => Err(validation_error(
                "Modalidad debe ser ON_SITE, REMOTE o HYBRID",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OnSite => "ON_SITE",
            Self::Remote => "REMOTE",
            Self::Hybrid => "HYBRID",
        }
    }
}

/// Journey type of an agreed schedule (R38 art. 25.1 e).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JourneyType {
    Fixed,
    Shifts,
    Cycles,
    Biweekly,
    Exceptional,
    PartTime,
}

/// Who asked for a schedule version.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RequestedBy {
    Employer,
    Worker,
}

/// Telemetry state of a worker, independent from `status` (`domain.md` §5).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TelemetryStatus {
    NotRequested,
    PendingDecision,
    Accepted,
    Declined,
    Paused,
}

/// Link/terms lifecycle of a worker (contract v3 §12).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkerStatus {
    PendingEnrollment,
    PendingTerms,
    Active,
    Revoked,
}

impl WorkerStatus {
    pub fn parse(raw: &str) -> Result<Self, CommandError> {
        match raw.trim().to_uppercase().as_str() {
            "PENDING_ENROLLMENT" => Ok(Self::PendingEnrollment),
            "PENDING_TERMS" => Ok(Self::PendingTerms),
            "ACTIVE" => Ok(Self::Active),
            "REVOKED" => Ok(Self::Revoked),
            _ => Err(validation_error(format!("Unknown worker status '{raw}'"))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PendingEnrollment => "PENDING_ENROLLMENT",
            Self::PendingTerms => "PENDING_TERMS",
            Self::Active => "ACTIVE",
            Self::Revoked => "REVOKED",
        }
    }
}

fn default_work_mode() -> WorkMode {
    WorkMode::OnSite
}

fn default_telemetry_status() -> TelemetryStatus {
    TelemetryStatus::NotRequested
}

fn default_role_type() -> String {
    "OFFICE".to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExceptionalResolution {
    pub number: String,
    pub date: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScheduleDay {
    pub weekday: u8,
    pub start: String,
    pub end: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub break_start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub break_end: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DisconnectionWindow {
    pub start: String,
    pub end: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScheduleInput {
    pub journey_type: JourneyType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exceptional_resolution: Option<ExceptionalResolution>,
    #[serde(default)]
    pub days: Vec<ScheduleDay>,
    #[serde(default)]
    pub break_counts_as_work: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disconnection: Option<DisconnectionWindow>,
    #[serde(default)]
    pub location_at_mark: bool,
    pub effective_from: String,
    pub requested_by: RequestedBy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScheduleResponse {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub version: Option<i32>,
    pub journey_type: JourneyType,
    #[serde(default)]
    pub exceptional_resolution: Option<ExceptionalResolution>,
    #[serde(default)]
    pub days: Vec<ScheduleDay>,
    #[serde(default)]
    pub break_counts_as_work: bool,
    #[serde(default)]
    pub disconnection: Option<DisconnectionWindow>,
    #[serde(default)]
    pub location_at_mark: bool,
    pub effective_from: String,
    #[serde(default)]
    pub requested_by: Option<RequestedBy>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub warnings: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScheduleUpdateResponse {
    pub schedule: ScheduleResponse,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WorkerWorkplace {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub address: Option<serde_json::Value>,
}

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
    #[serde(default)]
    pub email: String,
    pub name: String,
    pub device_identifier: Option<String>,
    pub device_info: Option<WorkerDeviceInfo>,
    pub role_type: String, // "OFFICE" | "FIELD"
    pub is_active: bool,
    pub status: WorkerStatus,
    #[serde(default)]
    pub status_changed_at: Option<String>,
    /// Effective flags: the subset the worker accepted.
    pub consent_flags: ConsentFlags,
    pub requested_consent_flags: ConsentFlags,
    pub consent_descriptions: ConsentDescriptions,
    pub consent_version: i32,
    pub accepted_consent_version: Option<i32>,
    pub consent_accepted_at: Option<String>,
    pub enrollment_completed_at: Option<String>,
    #[serde(default)]
    pub enrollment_redeemed_at: Option<String>,
    pub enrollment_expires_at: Option<String>,
    #[serde(default)]
    pub enrollment_expired: bool,
    #[serde(default)]
    pub work_hours_start: Option<String>,
    #[serde(default)]
    pub work_hours_end: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    pub created_at: String,
    pub updated_at: String,

    // v3 fields (RF-ADMIN-14-1)
    #[serde(default)]
    pub run: String,
    #[serde(default)]
    pub personal_email: String,
    #[serde(default)]
    pub job_title: String,
    #[serde(default)]
    pub workplace: Option<WorkerWorkplace>,
    #[serde(default = "default_work_mode")]
    pub work_mode: WorkMode,
    #[serde(default)]
    pub art22_exempt: bool,
    #[serde(default)]
    pub schedule: Option<ScheduleResponse>,
    #[serde(default = "default_telemetry_status")]
    pub telemetry_status: TelemetryStatus,
    #[serde(default)]
    pub terms_accepted_version: Option<String>,
    #[serde(default)]
    pub warnings: Option<Vec<String>>,
    /// Ex-worker portal access deadline (RFC 3339), only for `REVOKED` workers (API TASK-27).
    #[serde(default)]
    pub portal_access_until: Option<String>,
}

/// Enrollment package issued by the API (api-contract §5).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EnrollmentPackage {
    pub code: String,
    pub expires_at: String,
    pub commitment: String,
    pub safety_code: String,
    pub qr_payload: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CreateWorkerApiResponse {
    pub worker: WorkerResponse,
    pub enrollment: EnrollmentPackage,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RegenerateTokenResponse {
    pub enrollment: EnrollmentPackage,
}

/// What the enrollment modal (W3) renders, after local verification (RF-ADMIN-09-2).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EnrollmentView {
    /// The verified canonical `qr_payload` (base64 of the QR JSON).
    pub qr_payload: String,
    pub code: String,
    pub safety_code: String,
    pub commitment: String,
    pub expires_at: String,
    /// Server the agent will contact (shown to the admin).
    pub api_url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CreatedWorker {
    pub worker: WorkerResponse,
    pub enrollment: EnrollmentView,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConsentConfigResponse {
    pub consent_version: i32,
    pub requires_worker_acceptance: bool,
    pub worker: WorkerResponse,
}

/// Consent evidence row (api-contract §5, granular consent).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConsentRecord {
    pub id: String,
    pub worker_id: String,
    #[serde(default)]
    pub admin_id: Option<String>,
    pub consent_version: i32,
    /// `ACCEPTED` | `REJECTED` | `WITHDRAWN` | `NARROWED`.
    pub decision: String,
    #[serde(default)]
    pub consent_flags: Option<ConsentFlags>,
    /// Chosen subset; `null` in rows written before granular consent (= all requested).
    #[serde(default)]
    pub accepted_consent_flags: Option<ConsentFlags>,
    #[serde(default)]
    pub consent_descriptions: Option<ConsentDescriptions>,
    #[serde(default)]
    pub manifest_hash: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    #[serde(default)]
    pub app_version: Option<String>,
    pub created_at: String,
}

/// Create-worker form (W2, contract v3 §11). Validation here is authoritative; the React
/// form mirrors it for live messages (RF-ADMIN-14-2).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CreateWorkerForm {
    pub name: String,
    #[serde(default)]
    pub run: String,
    #[serde(default)]
    pub personal_email: String,
    #[serde(default)]
    pub job_title: String,
    #[serde(default)]
    pub workplace_id: String,
    #[serde(default = "default_work_mode")]
    pub work_mode: WorkMode,
    #[serde(default)]
    pub art22_exempt: bool,
    #[serde(default)]
    pub schedule: Option<ScheduleInput>,
    #[serde(default = "default_role_type")]
    pub role_type: String, // "OFFICE" | "FIELD"
    #[serde(default)]
    pub consent_flags: ConsentFlags,
    #[serde(default)]
    pub consent_descriptions: ConsentDescriptions,
    #[serde(default)]
    pub location_cause: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub enrollment_ttl_hours: Option<u32>,
}

impl CreateWorkerForm {
    /// Validates the whole form and returns the canonical RUN (`12.345.678-5`).
    ///
    /// Enforces ATT-11 (warnings only, never here), ATT-12 (only art. 22 may skip the
    /// schedule), DP-04 (location needs FIELD + cause), DP-05 (no screenshots outside
    /// `ON_SITE`) and DP-08 (an all-false consent is valid: attendance-only worker).
    pub fn validate(&self) -> Result<String, CommandError> {
        let name = self.name.trim();
        if name.chars().count() < 2 || name.chars().count() > 255 {
            return Err(CommandError::validation(
                "El nombre debe tener entre 2 y 255 caracteres.",
            ));
        }

        let canonical_run = validate_run(&self.run)?;
        validate_personal_email(&self.personal_email)?;
        validate_job_title(&self.job_title)?;

        if self.workplace_id.trim().is_empty() {
            return Err(CommandError::validation(
                "Se requiere seleccionar un lugar de trabajo.",
            ));
        }

        let role_upper = validate_role(&self.role_type)?;

        if !self.art22_exempt {
            let sched = self.schedule.as_ref().ok_or_else(|| {
                CommandError::validation(
                    "Se debe ingresar la jornada pactada para trabajadores no exentos del art. 22.",
                )
            })?;
            validate_schedule(sched, self.work_mode)?;
        }

        if let Some(tz) = &self.timezone {
            if !tz.trim().is_empty() {
                validate_timezone(tz)?;
            }
        }

        // DP-05: screenshots not allowed for REMOTE or HYBRID
        if self.consent_flags.productivity_screenshots && self.work_mode != WorkMode::OnSite {
            return Err(CommandError::new(
                "SCREENSHOTS_NOT_ALLOWED_REMOTE",
                "Las capturas de pantalla no están permitidas para trabajadores en teletrabajo o modalidad mixta (R38 art. 55 a).",
            ));
        }

        // DP-04 / RF-ADMIN-14-2: location tracking requires FIELD role and location_cause
        if self.consent_flags.location {
            if role_upper != "FIELD" {
                return Err(CommandError::new(
                    "VALIDATION_ERROR",
                    "La ubicación solo puede solicitarse a trabajadores en terreno (FIELD).",
                ));
            }
            validate_location_cause(self.location_cause.as_deref())?;
        }

        if let Some(ttl) = self.enrollment_ttl_hours {
            if !(1..=72).contains(&ttl) {
                return Err(CommandError::validation(
                    "La vigencia del código debe estar entre 1 y 72 horas.",
                ));
            }
        }

        validate_consent_descriptions(&self.consent_flags, &self.consent_descriptions)?;

        Ok(canonical_run)
    }

    pub fn normalized_descriptions(&self) -> ConsentDescriptions {
        let mut d = self.consent_descriptions.clone();
        d.retain_enabled(&self.consent_flags);
        d
    }
}

/// RFC 3986 unreserved characters stay literal; everything else is escaped.
const QUERY_VALUE_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');
pub const VALID_ROLES: [&str; 2] = ["OFFICE", "FIELD"];
/// Top-level areas of the IANA tz database (plus `Etc`). Used for a lightweight
/// format check; the server remains the authority on the exact zone list.
const IANA_AREAS: [&str; 11] = [
    "Africa",
    "America",
    "Antarctica",
    "Arctic",
    "Asia",
    "Atlantic",
    "Australia",
    "Europe",
    "Indian",
    "Pacific",
    "Etc",
];
const MIN_SEARCH_LEN: usize = 2;
const MAX_PER_PAGE: usize = 100;

fn validation_error(message: impl Into<String>) -> CommandError {
    CommandError::validation(message)
}

/// Enrollment code lifetime accepted by the API (hours).
const MIN_TTL_HOURS: u32 = 1;
const MAX_TTL_HOURS: u32 = 72;
const DEFAULT_TTL_HOURS: u32 = 24;

fn parse_work_hour(t: &str) -> Option<chrono::NaiveTime> {
    let t = t.trim();
    chrono::NaiveTime::parse_from_str(t, "%H:%M:%S")
        .or_else(|_| chrono::NaiveTime::parse_from_str(t, "%H:%M"))
        .ok()
}

/// Accepts `HH:MM` or `HH:MM:SS` and returns the `HH:MM:SS` form the API expects.
pub fn normalize_work_hour(t: &str) -> Result<String, CommandError> {
    parse_work_hour(t)
        .map(|time| time.format("%H:%M:%S").to_string())
        .ok_or_else(|| validation_error("Work hours must be in HH:MM format"))
}

pub fn validate_work_hours(start: &str, end: &str) -> Result<(), CommandError> {
    match (parse_work_hour(start), parse_work_hour(end)) {
        (Some(s), Some(e)) if s < e => Ok(()),
        (Some(_), Some(_)) => Err(validation_error(
            "Work hours start must be earlier than end",
        )),
        _ => Err(validation_error("Work hours must be in HH:MM format")),
    }
}

pub fn validate_timezone(tz: &str) -> Result<(), CommandError> {
    let tz = tz.trim();
    if tz == "UTC" {
        return Ok(());
    }
    let mut parts = tz.split('/');
    let area_ok = parts.next().is_some_and(|a| IANA_AREAS.contains(&a));
    let rest: Vec<&str> = parts.collect();
    let rest_ok = !rest.is_empty()
        && rest.iter().all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+'))
        });
    if area_ok && rest_ok {
        Ok(())
    } else {
        Err(validation_error(format!(
            "'{}' is not a valid IANA timezone (e.g. America/Santiago)",
            tz
        )))
    }
}

/// Every enabled category needs a 10–500 character description (RF-ADMIN-08-2).
pub fn validate_consent_descriptions(
    flags: &ConsentFlags,
    descriptions: &ConsentDescriptions,
) -> Result<(), CommandError> {
    // Validate descriptions: required for active flags, 10..=500 chars (RF-ADMIN-08-2)
    let check_desc = |flag: bool, desc: Option<&str>, cat_name: &str| -> Result<(), CommandError> {
        if flag {
            // Count characters, not bytes: accented text must match the UI limit.
            let len = desc.unwrap_or("").trim().chars().count();
            if !(10..=500).contains(&len) {
                return Err(CommandError::validation(format!(
                    "Description for '{}' must be between 10 and 500 characters",
                    cat_name
                )));
            }
        }
        Ok(())
    };

    check_desc(
        flags.system_activity,
        descriptions.system_activity.as_deref(),
        "system_activity",
    )?;
    check_desc(
        flags.network_activity,
        descriptions.network_activity.as_deref(),
        "network_activity",
    )?;
    check_desc(
        flags.productivity_basic,
        descriptions.productivity_basic.as_deref(),
        "productivity_basic",
    )?;
    check_desc(
        flags.productivity_screenshots,
        descriptions.productivity_screenshots.as_deref(),
        "productivity_screenshots",
    )?;
    check_desc(
        flags.file_activity,
        descriptions.file_activity.as_deref(),
        "file_activity",
    )?;
    check_desc(flags.location, descriptions.location.as_deref(), "location")?;
    Ok(())
}

/// RUN (RF-ADMIN-14-2): mod 11 through `telemetry_crypto_core::rut`; returns the canonical form.
pub fn validate_run(raw: &str) -> Result<String, CommandError> {
    if raw.trim().is_empty() {
        return Err(CommandError::validation(
            "Se requiere el RUN del trabajador.",
        ));
    }
    telemetry_crypto_core::rut::parse(raw)
        .map(|r| r.format())
        .map_err(|e| CommandError::validation(format!("RUN inválido: {e}")))
}

/// Personal email (R38 art. 12 e): receives every mark receipt, so a plausible address is required.
pub fn validate_personal_email(raw: &str) -> Result<(), CommandError> {
    let e = raw.trim();
    let ok = e.chars().count() <= 254
        && !e.chars().any(char::is_whitespace)
        && e.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && !domain.contains('@')
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
        });
    if ok {
        Ok(())
    } else {
        Err(CommandError::validation(
            "Se requiere un correo personal válido.",
        ))
    }
}

/// Job title, 2–100 characters (not bytes).
pub fn validate_job_title(raw: &str) -> Result<(), CommandError> {
    let n = raw.trim().chars().count();
    if (2..=100).contains(&n) {
        Ok(())
    } else {
        Err(CommandError::validation(
            "El cargo debe tener entre 2 y 100 caracteres.",
        ))
    }
}

/// `location_cause`, 20–500 characters, shown verbatim to the worker (DP-04, R38 art. 53 e).
pub fn validate_location_cause(raw: Option<&str>) -> Result<(), CommandError> {
    let len = raw.unwrap_or("").trim().chars().count();
    if (20..=500).contains(&len) {
        Ok(())
    } else {
        Err(CommandError::new(
            "LOCATION_CAUSE_REQUIRED",
            "Se requiere declarar una causa de seguridad o legal (20 a 500 caracteres) para solicitar ubicación (R38 art. 53 e).",
        ))
    }
}

/// Strict `HH:MM` (00:00–23:59) to minutes since midnight.
pub fn time_to_minutes_str(t: &str) -> Option<u32> {
    let b = t.trim().as_bytes();
    if b.len() != 5 || b[2] != b':' {
        return None;
    }
    let digits = |x: &[u8]| -> Option<u32> {
        x.iter().try_fold(0u32, |acc, c| {
            c.is_ascii_digit().then(|| acc * 10 + u32::from(c - b'0'))
        })
    };
    let h = digits(&b[0..2])?;
    let m = digits(&b[3..5])?;
    (h <= 23 && m <= 59).then_some(h * 60 + m)
}

/// Length of the disconnection window in minutes; a window may cross midnight.
pub fn calculate_disconnection_minutes(start: &str, end: &str) -> Result<u32, CommandError> {
    let s = time_to_minutes_str(start)
        .ok_or_else(|| validation_error("Hora de inicio de desconexión inválida"))?;
    let e = time_to_minutes_str(end)
        .ok_or_else(|| validation_error("Hora de fin de desconexión inválida"))?;
    let mins = if e >= s { e - s } else { 24 * 60 - s + e };
    Ok(mins)
}

fn validate_date(raw: &str, what: &str) -> Result<(), CommandError> {
    chrono::NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| validation_error(format!("{what} inválida (use AAAA-MM-DD)")))
}

/// Schedule rules that do not depend on the work mode. Exceeding the daily/weekly legal
/// maximum is **not** an error here (ATT-11): the server answers with `warnings`.
pub fn validate_schedule_basic(schedule: &ScheduleInput) -> Result<(), CommandError> {
    if schedule.journey_type == JourneyType::Exceptional {
        let res = schedule.exceptional_resolution.as_ref().ok_or_else(|| {
            validation_error("Jornada excepcional requiere número y fecha de resolución de la DT")
        })?;
        if res.number.trim().is_empty() {
            return Err(validation_error(
                "El número de resolución no puede estar vacío",
            ));
        }
        validate_date(&res.date, "La fecha de resolución")?;
    }

    if schedule.days.is_empty() {
        return Err(validation_error(
            "La jornada debe tener al menos un día de trabajo.",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for day in &schedule.days {
        if !(1..=7).contains(&day.weekday) {
            return Err(validation_error(
                "Día de la semana inválido (debe ser 1 a 7)",
            ));
        }
        if !seen.insert(day.weekday) {
            return Err(validation_error(format!(
                "El día {} está repetido en la jornada",
                day.weekday
            )));
        }
        let s = time_to_minutes_str(&day.start).ok_or_else(|| {
            validation_error(format!("Hora de entrada inválida en día {}", day.weekday))
        })?;
        let e = time_to_minutes_str(&day.end).ok_or_else(|| {
            validation_error(format!("Hora de salida inválida en día {}", day.weekday))
        })?;
        if s >= e {
            return Err(validation_error(format!(
                "La hora de entrada debe ser anterior a la hora de salida en día {}",
                day.weekday
            )));
        }
        match (&day.break_start, &day.break_end) {
            (None, None) => {}
            (Some(bs), Some(be)) => {
                let bs_min = time_to_minutes_str(bs)
                    .ok_or_else(|| validation_error("Hora de inicio de colación inválida"))?;
                let be_min = time_to_minutes_str(be)
                    .ok_or_else(|| validation_error("Hora de fin de colación inválida"))?;
                if bs_min < s || be_min > e || bs_min >= be_min {
                    return Err(validation_error(format!(
                        "Franja de colación inválida dentro de la jornada en día {}",
                        day.weekday
                    )));
                }
            }
            _ => {
                return Err(validation_error(format!(
                    "La colación requiere inicio y término en día {}",
                    day.weekday
                )))
            }
        }
    }

    if let Some(disc) = &schedule.disconnection {
        let disc_mins = calculate_disconnection_minutes(&disc.start, &disc.end)?;
        if disc_mins < 12 * 60 {
            return Err(validation_error(
                "La desconexión debe ser de al menos 12 horas continuas.",
            ));
        }
    }

    validate_date(&schedule.effective_from, "La fecha de vigencia")?;
    if schedule
        .note
        .as_deref()
        .is_some_and(|n| n.chars().count() > MAX_REASON_CHARS)
    {
        return Err(validation_error(
            "La nota admite como máximo 500 caracteres",
        ));
    }

    Ok(())
}

/// Full schedule validation for a worker with the given modality. CT art. 152 quáter J:
/// `REMOTE`/`HYBRID` need a disconnection window of at least 12 continuous hours.
pub fn validate_schedule(
    schedule: &ScheduleInput,
    work_mode: WorkMode,
) -> Result<(), CommandError> {
    validate_schedule_basic(schedule)?;

    if work_mode != WorkMode::OnSite {
        let disc = schedule.disconnection.as_ref().ok_or_else(|| {
            validation_error("Se requiere franja de desconexión (≥ 12 h continuas) para teletrabajo o jornada mixta.")
        })?;
        let disc_mins = calculate_disconnection_minutes(&disc.start, &disc.end)?;
        if disc_mins < 12 * 60 {
            return Err(validation_error(
                "La desconexión debe ser de al menos 12 horas continuas.",
            ));
        }
    }

    Ok(())
}

pub fn parse_schedule_update_response(
    value: serde_json::Value,
) -> Result<ScheduleUpdateResponse, CommandError> {
    if let Ok(resp) = serde_json::from_value::<ScheduleUpdateResponse>(value.clone()) {
        return Ok(resp);
    }
    if let Ok(schedule) = serde_json::from_value::<ScheduleResponse>(value.clone()) {
        let warnings = schedule.warnings.clone().unwrap_or_default();
        return Ok(ScheduleUpdateResponse { schedule, warnings });
    }
    Err(CommandError::new(
        "DESERIALIZATION_ERROR",
        "Formato de respuesta de actualización de jornada desconocido",
    ))
}

fn validate_role(role: &str) -> Result<String, CommandError> {
    let upper = role.trim().to_uppercase();
    if VALID_ROLES.contains(&upper.as_str()) {
        Ok(upper)
    } else {
        Err(validation_error("Role must be OFFICE or FIELD"))
    }
}

/// Editable worker fields (`PUT /workers/:id`). Consent is edited separately via
/// `workers_update_consent_config`; status via `workers_set_status`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct UpdateWorkerForm {
    pub name: Option<String>,
    pub run: Option<String>,
    pub personal_email: Option<String>,
    pub role_type: Option<String>,
    pub job_title: Option<String>,
    pub workplace_id: Option<String>,
    pub work_mode: Option<WorkMode>,
    pub art22_exempt: Option<bool>,
    pub location_cause: Option<String>,
    pub timezone: Option<String>,
}

impl UpdateWorkerForm {
    /// Validates the form and builds the request body with only the provided fields.
    pub fn to_body(&self) -> Result<serde_json::Value, CommandError> {
        let mut body = serde_json::Map::new();

        if let Some(name) = &self.name {
            let n = name.trim();
            if n.chars().count() < 2 || n.chars().count() > 255 {
                return Err(validation_error(
                    "El nombre debe tener entre 2 y 255 caracteres",
                ));
            }
            body.insert("name".into(), n.into());
        }
        if let Some(run) = &self.run {
            body.insert("run".into(), validate_run(run)?.into());
        }
        if let Some(email) = &self.personal_email {
            validate_personal_email(email)?;
            body.insert("personal_email".into(), email.trim().into());
        }
        if let Some(role) = &self.role_type {
            body.insert("role_type".into(), validate_role(role)?.into());
        }
        if let Some(title) = &self.job_title {
            validate_job_title(title)?;
            body.insert("job_title".into(), title.trim().into());
        }
        if let Some(w) = &self.workplace_id {
            if w.trim().is_empty() {
                return Err(validation_error("workplace_id no puede estar vacío"));
            }
            body.insert("workplace_id".into(), w.trim().into());
        }
        if let Some(m) = self.work_mode {
            body.insert("work_mode".into(), m.as_str().into());
        }
        if let Some(exempt) = self.art22_exempt {
            body.insert("art22_exempt".into(), serde_json::Value::Bool(exempt));
        }
        if let Some(cause) = &self.location_cause {
            let c = cause.trim();
            if !c.is_empty() {
                validate_location_cause(Some(c))?;
            }
            body.insert("location_cause".into(), c.into());
        }
        if let Some(tz) = &self.timezone {
            validate_timezone(tz)?;
            body.insert("timezone".into(), tz.trim().into());
        }

        if body.is_empty() {
            return Err(validation_error("No fields to update"));
        }
        Ok(serde_json::Value::Object(body))
    }
}

pub fn ensure_matches_request(
    form: &CreateWorkerForm,
    created: &WorkerResponse,
) -> Result<(), CommandError> {
    if created.requested_consent_flags != form.consent_flags {
        return Err(CommandError::new(
            "ENROLLMENT_TAMPER_CHECK_FAILED",
            "Server returned different requested consent flags",
        ));
    }

    let normalized_expected = form.normalized_descriptions();
    if created.consent_descriptions != normalized_expected {
        return Err(CommandError::new(
            "ENROLLMENT_TAMPER_CHECK_FAILED",
            "Server returned different consent descriptions",
        ));
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
    pub is_active: Option<bool>,
    pub workplace_id: Option<String>,
    pub work_mode: Option<String>,
}

impl WorkerListQuery {
    /// Builds `/workers?…` with validated, percent-encoded parameters (RF-ADMIN-03-1).
    pub fn to_endpoint(&self) -> Result<String, CommandError> {
        let mut params = Vec::new();
        if let Some(p) = self.page {
            params.push(format!("page={}", p.max(1)));
        }
        if let Some(pp) = self.per_page {
            params.push(format!("per_page={}", pp.clamp(1, MAX_PER_PAGE)));
        }
        if let Some(s) = self.search.as_deref().map(str::trim) {
            // Fuzzy search only from 2 characters to avoid hammering the server.
            if s.chars().count() >= MIN_SEARCH_LEN {
                params.push(format!(
                    "search={}",
                    utf8_percent_encode(s, QUERY_VALUE_ENCODE_SET)
                ));
            }
        }
        if let Some(r) = &self.role_type {
            params.push(format!("role_type={}", validate_role(r)?));
        }
        if let Some(st) = &self.status {
            params.push(format!("status={}", WorkerStatus::parse(st)?.as_str()));
        }
        if let Some(active) = self.is_active {
            params.push(format!("is_active={}", active));
        }
        // `workplace_id` / `work_mode` are not query parameters of the API (`WorkerQueryParams`
        // ignores them): they are applied by `workers_list_with_client` (RF-ADMIN-23-1).

        Ok(if params.is_empty() {
            "/workers".to_string()
        } else {
            format!("/workers?{}", params.join("&"))
        })
    }
}

fn integrity_error(field: &str) -> CommandError {
    tracing::error!(field, "enrollment integrity check failed");
    CommandError::new(
        "ENROLLMENT_INTEGRITY_ERROR",
        "El paquete de vinculación del servidor no coincide con tu llave local. No se muestra el QR: revisa la conexión con el servidor.",
    )
}

/// SEC-10: `https://` (plus `http://` when `allow_http`, debug builds), a host, no
/// credentials and no fragment.
pub fn validate_qr_api_url(url: &str, allow_http: bool) -> Result<(), CommandError> {
    let rest = if let Some(rest) = url.strip_prefix("https://") {
        rest
    } else if let Some(rest) = url.strip_prefix("http://").filter(|_| allow_http) {
        rest
    } else {
        return Err(integrity_error("api_url"));
    };
    let authority = rest.split(['/', '?']).next().unwrap_or_default();
    if authority.is_empty()
        || authority.contains('@')
        || url.contains('#')
        || url.chars().any(char::is_whitespace)
    {
        return Err(integrity_error("api_url"));
    }
    Ok(())
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}

/// RF-ADMIN-09-1: recomputes the commitment with the **local** key and checks the server's
/// commitment, safety code and `qr_payload`. Any difference ⇒ no QR.
pub fn verify_enrollment_package(
    local_admin_pub_b64: &str,
    worker: &WorkerResponse,
    enrollment: &EnrollmentPackage,
    allow_http: bool,
) -> Result<EnrollmentView, CommandError> {
    let manifest = EnrollmentManifest::new(
        local_admin_pub_b64.to_owned(),
        worker.consent_version,
        worker.requested_consent_flags,
        worker.consent_descriptions.clone(),
    )
    .map_err(|_| integrity_error("manifest"))?;
    let local_commitment =
        manifest_commitment(&manifest).map_err(|_| integrity_error("manifest"))?;
    if !constant_time_eq(
        &local_commitment,
        &enrollment.commitment.to_ascii_lowercase(),
    ) {
        return Err(integrity_error("commitment"));
    }
    let local_safety_code =
        safety_code(&local_commitment).map_err(|_| integrity_error("safety_code"))?;
    if local_safety_code != enrollment.safety_code {
        return Err(integrity_error("safety_code"));
    }
    let qr = EnrollmentQr::parse_payload(&enrollment.qr_payload)
        .map_err(|_| integrity_error("qr_payload"))?;
    if qr.code != format_qr_code(&enrollment.code)
        || !constant_time_eq(&qr.commitment.to_ascii_lowercase(), &local_commitment)
    {
        return Err(integrity_error("qr_payload"));
    }
    validate_qr_api_url(&qr.api_url, allow_http)?;
    Ok(EnrollmentView {
        qr_payload: enrollment.qr_payload.clone(),
        code: enrollment.code.clone(),
        safety_code: local_safety_code,
        commitment: local_commitment,
        expires_at: enrollment.expires_at.clone(),
        api_url: qr.api_url,
    })
}

fn validate_ttl(ttl_hours: Option<u32>) -> Result<u32, CommandError> {
    let ttl = ttl_hours.unwrap_or(DEFAULT_TTL_HOURS);
    if !(MIN_TTL_HOURS..=MAX_TTL_HOURS).contains(&ttl) {
        return Err(validation_error(
            "La vigencia del código debe estar entre 1 y 72 horas",
        ));
    }
    Ok(ttl)
}

fn validate_worker_id(id: &str) -> Result<(), CommandError> {
    let valid = !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if valid {
        Ok(())
    } else {
        Err(validation_error("Identificador de trabajador inválido"))
    }
}

/// Filters `GET /workers` does not support, applied by the app (RF-ADMIN-23-1).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct LocalWorkerFilters {
    pub workplace_id: Option<String>,
    pub work_mode: Option<WorkMode>,
}

impl LocalWorkerFilters {
    pub fn is_empty(&self) -> bool {
        self.workplace_id.is_none() && self.work_mode.is_none()
    }

    pub fn matches(&self, worker: &WorkerResponse) -> bool {
        let workplace_ok = self.workplace_id.as_deref().map_or(true, |id| {
            worker
                .workplace
                .as_ref()
                .is_some_and(|w| w.id.eq_ignore_ascii_case(id))
        });
        let mode_ok = self.work_mode.map_or(true, |m| worker.work_mode == m);
        workplace_ok && mode_ok
    }
}

impl WorkerListQuery {
    /// Validated local filters: `workplace_id` must be a UUID, `work_mode` a known mode.
    pub fn local_filters(&self) -> Result<LocalWorkerFilters, CommandError> {
        let workplace_id = match self.workplace_id.as_deref().map(str::trim) {
            Some(id) if !id.is_empty() => Some(
                uuid::Uuid::parse_str(id)
                    .map_err(|_| validation_error("Lugar de trabajo inválido"))?
                    .to_string(),
            ),
            _ => None,
        };
        let work_mode = match self.work_mode.as_deref().map(str::trim) {
            Some(m) if !m.is_empty() => Some(WorkMode::parse(m)?),
            _ => None,
        };
        Ok(LocalWorkerFilters {
            workplace_id,
            work_mode,
        })
    }
}

/// Pages read at most when filtering locally (100 workers each).
const MAX_LOCAL_FILTER_PAGES: usize = 20;

/// Filters every page of `GET /workers` by `filters` and returns the requested page of the result
/// with a pagination block that describes the filtered list (RF-ADMIN-23-1).
async fn workers_list_filtered_locally(
    client: &AdminApiClient,
    query: &WorkerListQuery,
    filters: &LocalWorkerFilters,
) -> Result<CollectionResponse<WorkerResponse>, CommandError> {
    let mut matched = Vec::new();
    let mut server_page = 1;
    loop {
        let page_query = WorkerListQuery {
            page: Some(server_page),
            per_page: Some(MAX_PER_PAGE),
            ..query.clone()
        };
        let resp: CollectionResponse<WorkerResponse> =
            client.get_collection(&page_query.to_endpoint()?).await?;
        let received = resp.data.len();
        matched.extend(resp.data.into_iter().filter(|w| filters.matches(w)));
        let total_pages = resp.pagination.map_or(1, |p| p.total_pages);
        if server_page >= total_pages || received == 0 {
            break;
        }
        if server_page >= MAX_LOCAL_FILTER_PAGES {
            return Err(validation_error(
                "Hay demasiados trabajadores para filtrar por lugar o modalidad. Usa la búsqueda por nombre o los otros filtros.",
            ));
        }
        server_page += 1;
    }
    Ok(paginate_locally(
        matched,
        query.page.unwrap_or(1),
        query.per_page.unwrap_or(20),
    ))
}

/// Slices `items` like the server would (1-based `page`, `per_page` clamped to 1..=100).
pub fn paginate_locally<T>(items: Vec<T>, page: usize, per_page: usize) -> CollectionResponse<T> {
    let per_page = per_page.clamp(1, MAX_PER_PAGE);
    let total = items.len();
    let total_pages = total.div_ceil(per_page).max(1);
    let page = page.clamp(1, total_pages);
    let data = items
        .into_iter()
        .skip((page - 1) * per_page)
        .take(per_page)
        .collect();
    CollectionResponse {
        data,
        pagination: Some(PaginationMeta {
            total,
            page,
            per_page,
            total_pages,
        }),
    }
}

pub async fn workers_list_with_client(
    client: &AdminApiClient,
    query: Option<WorkerListQuery>,
) -> Result<CollectionResponse<WorkerResponse>, CommandError> {
    let query = query.unwrap_or_default();
    let filters = query.local_filters()?;
    if filters.is_empty() {
        return Ok(client.get_collection(&query.to_endpoint()?).await?);
    }
    workers_list_filtered_locally(client, &query, &filters).await
}

#[tauri::command]
pub async fn workers_list(
    query: Option<WorkerListQuery>,
) -> Result<CollectionResponse<WorkerResponse>, CommandError> {
    workers_list_with_client(get_api_client(), query).await
}

pub async fn workers_get_with_client(
    client: &AdminApiClient,
    id: &str,
) -> Result<WorkerResponse, CommandError> {
    validate_worker_id(id)?;
    Ok(client.get_json(&format!("/workers/{id}")).await?)
}

#[tauri::command]
pub async fn workers_get(id: String) -> Result<WorkerResponse, CommandError> {
    workers_get_with_client(get_api_client(), &id).await
}

#[tauri::command]
pub async fn workers_create(form: CreateWorkerForm) -> Result<CreatedWorker, CommandError> {
    workers_create_with_client(get_api_client(), form).await
}

pub async fn workers_create_with_client(
    client: &AdminApiClient,
    form: CreateWorkerForm,
) -> Result<CreatedWorker, CommandError> {
    let canonical_run = form.validate()?;

    // RF-ADMIN-08-1: the server must hold our local key (registered now if
    // missing) before it accepts workers; a different key blocks creation (K2).
    let admin_pub = ensure_server_key_with_client(client).await?;

    // Contract v3 §11: `work_hours_*` are derived by the server from the schedule, so the
    // app never sends (nor invents) them. An art. 22 exempt worker carries no schedule.
    let mut body = serde_json::json!({
        "name": form.name.trim(),
        "email": form.personal_email.trim(),
        "personal_email": form.personal_email.trim(),
        "run": canonical_run,
        "job_title": form.job_title.trim(),
        "workplace_id": form.workplace_id.trim(),
        "work_mode": form.work_mode,
        "art22_exempt": form.art22_exempt,
        "role_type": validate_role(&form.role_type)?,
        "consent_flags": form.consent_flags,
        "consent_descriptions": form.normalized_descriptions(),
        "enrollment_ttl_hours": validate_ttl(form.enrollment_ttl_hours)?,
    });

    if let Some(obj) = body.as_object_mut() {
        if let (false, Some(sched)) = (form.art22_exempt, &form.schedule) {
            obj.insert(
                "schedule".to_string(),
                serde_json::to_value(sched)
                    .map_err(|e| CommandError::new("SERIALIZATION_ERROR", e.to_string()))?,
            );
        }
        if form.consent_flags.location {
            obj.insert(
                "location_cause".to_string(),
                serde_json::Value::String(
                    form.location_cause
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .to_string(),
                ),
            );
        }
        if let Some(tz) = &form.timezone {
            if !tz.trim().is_empty() {
                obj.insert(
                    "timezone".to_string(),
                    serde_json::Value::String(tz.trim().to_string()),
                );
            }
        }
    }

    let created: CreateWorkerApiResponse = client.post_json("/workers", &body).await?;

    // Tamper check (RF-3.1): compare server response with request
    ensure_matches_request(&form, &created.worker)?;
    let view = verify_enrollment_package(
        &admin_pub,
        &created.worker,
        &created.enrollment,
        cfg!(debug_assertions),
    )?;

    tracing::info!(
        worker_id = %created.worker.id,
        commitment = %&view.commitment[..8],
        "Worker created and enrollment QR issued successfully"
    );

    let mut warnings = created.warnings;
    if let Some(w) = &created.worker.warnings {
        for warning in w {
            if !warnings.contains(warning) {
                warnings.push(warning.clone());
            }
        }
    }
    if let Some(sched) = &created.worker.schedule {
        if let Some(w) = &sched.warnings {
            for warning in w {
                if !warnings.contains(warning) {
                    warnings.push(warning.clone());
                }
            }
        }
    }

    Ok(CreatedWorker {
        worker: created.worker,
        enrollment: view,
        warnings,
    })
}

const MAX_REASON_CHARS: usize = 500;

#[tauri::command]
pub async fn workers_set_status(
    id: String,
    status: String,
    reason: Option<String>,
) -> Result<WorkerResponse, CommandError> {
    workers_set_status_with_client(get_api_client(), &id, &status, reason).await
}

pub async fn workers_set_status_with_client(
    client: &AdminApiClient,
    id: &str,
    status: &str,
    reason: Option<String>,
) -> Result<WorkerResponse, CommandError> {
    validate_worker_id(id)?;
    let status = status.trim().to_uppercase();
    if status != "REVOKED" {
        return Err(validation_error(
            "workers_set_status acepta solo REVOKED (para pausar o reanudar telemetría use workers_set_telemetry_status)",
        ));
    }
    let reason = reason
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty());
    if reason
        .as_ref()
        .is_some_and(|r| r.chars().count() > MAX_REASON_CHARS)
    {
        return Err(validation_error(
            "El motivo admite como máximo 500 caracteres",
        ));
    }
    let body = serde_json::json!({ "status": "REVOKED", "reason": reason });
    let worker: WorkerResponse = client
        .patch_json(&format!("/workers/{id}/status"), &body)
        .await?;
    tracing::info!(worker_id = %id, to = "REVOKED", "Worker revoked");
    Ok(worker)
}

#[tauri::command]
pub async fn workers_set_telemetry_status(
    id: String,
    status: String,
    reason: Option<String>,
) -> Result<WorkerResponse, CommandError> {
    workers_set_telemetry_status_with_client(get_api_client(), &id, &status, reason).await
}

pub async fn workers_set_telemetry_status_with_client(
    client: &AdminApiClient,
    id: &str,
    status: &str,
    reason: Option<String>,
) -> Result<WorkerResponse, CommandError> {
    validate_worker_id(id)?;
    let status = status.trim().to_uppercase();
    if status != "PAUSED" && status != "ACTIVE" {
        return Err(validation_error(format!(
            "telemetry_status must be PAUSED or ACTIVE (got '{}')",
            status
        )));
    }
    let reason = reason
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty());
    if reason
        .as_ref()
        .is_some_and(|r| r.chars().count() > MAX_REASON_CHARS)
    {
        return Err(validation_error(
            "El motivo admite como máximo 500 caracteres",
        ));
    }
    let body = serde_json::json!({ "status": status, "reason": reason });
    let worker: WorkerResponse = client
        .patch_json(&format!("/workers/{id}/telemetry-status"), &body)
        .await?;
    tracing::info!(worker_id = %id, telemetry_status = %status, "Worker telemetry status changed");
    Ok(worker)
}

#[tauri::command]
pub async fn workers_update_schedule(
    id: String,
    schedule: ScheduleInput,
) -> Result<ScheduleUpdateResponse, CommandError> {
    workers_update_schedule_with_client(get_api_client(), &id, schedule).await
}

pub async fn workers_update_schedule_with_client(
    client: &AdminApiClient,
    id: &str,
    schedule: ScheduleInput,
) -> Result<ScheduleUpdateResponse, CommandError> {
    validate_worker_id(id)?;
    // The disconnection rule depends on the worker's modality (CT art. 152 quáter J), and an
    // art. 22 exempt worker has no agreed schedule (ATT-12).
    let worker = workers_get_with_client(client, id).await?;
    if worker.art22_exempt {
        return Err(validation_error(
            "El trabajador está excluido de la limitación de jornada (art. 22): no tiene jornada pactada.",
        ));
    }
    validate_schedule(&schedule, worker.work_mode)?;

    let raw: serde_json::Value = client
        .put_json(&format!("/workers/{id}/schedule"), &schedule)
        .await?;
    let resp = parse_schedule_update_response(raw)?;
    tracing::info!(worker_id = %id, "Worker schedule updated");
    Ok(resp)
}

/// RF-ADMIN-09-3: new code, then verification against the worker's current manifest.
/// `409 ENROLLMENT_ALREADY_COMPLETED` (`details.hint = REVOKE_FIRST`) reaches the UI as is.
pub async fn workers_regenerate_token_with_client(
    client: &AdminApiClient,
    id: &str,
    ttl_hours: Option<u32>,
) -> Result<EnrollmentView, CommandError> {
    validate_worker_id(id)?;
    let ttl = validate_ttl(ttl_hours)?;
    let admin_pub = get_admin_public_key()?;
    let resp: RegenerateTokenResponse = client
        .post_json(
            &format!("/workers/{id}/enrollment-token"),
            &serde_json::json!({ "enrollment_ttl_hours": ttl }),
        )
        .await?;
    let worker = workers_get_with_client(client, id).await?;
    let view = verify_enrollment_package(
        &admin_pub,
        &worker,
        &resp.enrollment,
        cfg!(debug_assertions),
    )?;
    tracing::info!(worker_id = %id, "enrollment code regenerated");
    Ok(view)
}

#[tauri::command]
pub async fn workers_regenerate_token(
    id: String,
    ttl_hours: Option<u32>,
) -> Result<EnrollmentView, CommandError> {
    workers_regenerate_token_with_client(get_api_client(), &id, ttl_hours).await
}

#[tauri::command]
pub async fn workers_update_consent_config(
    id: String,
    consent_flags: ConsentFlags,
    consent_descriptions: ConsentDescriptions,
) -> Result<ConsentConfigResponse, CommandError> {
    workers_update_consent_config_with_client(
        get_api_client(),
        &id,
        consent_flags,
        consent_descriptions,
    )
    .await
}

/// `PUT /workers/:id/consent-config`. A new version must be re-accepted by the
/// worker before collection resumes (`requires_worker_acceptance`).
pub async fn workers_update_consent_config_with_client(
    client: &AdminApiClient,
    id: &str,
    consent_flags: ConsentFlags,
    consent_descriptions: ConsentDescriptions,
) -> Result<ConsentConfigResponse, CommandError> {
    validate_worker_id(id)?;
    validate_consent_descriptions(&consent_flags, &consent_descriptions)?;
    let mut normalized_desc = consent_descriptions;
    normalized_desc.retain_enabled(&consent_flags);

    let body = serde_json::json!({
        "consent_flags": consent_flags,
        "consent_descriptions": normalized_desc
    });

    let resp: ConsentConfigResponse = client
        .put_json(&format!("/workers/{id}/consent-config"), &body)
        .await?;
    tracing::info!(
        worker_id = %id,
        consent_version = resp.consent_version,
        requires_acceptance = resp.requires_worker_acceptance,
        "Consent configuration updated"
    );
    Ok(resp)
}

pub async fn workers_get_consent_records_with_client(
    client: &AdminApiClient,
    id: &str,
) -> Result<Vec<ConsentRecord>, CommandError> {
    validate_worker_id(id)?;
    let records: CollectionResponse<ConsentRecord> = client
        .get_collection(&format!("/workers/{id}/consent-records?per_page=100"))
        .await?;
    Ok(records.data)
}

#[tauri::command]
pub async fn workers_get_consent_records(id: String) -> Result<Vec<ConsentRecord>, CommandError> {
    workers_get_consent_records_with_client(get_api_client(), &id).await
}

#[tauri::command]
pub async fn workers_update(
    id: String,
    form: UpdateWorkerForm,
) -> Result<WorkerResponse, CommandError> {
    workers_update_with_client(get_api_client(), &id, form).await
}

pub async fn workers_update_with_client(
    client: &AdminApiClient,
    id: &str,
    form: UpdateWorkerForm,
) -> Result<WorkerResponse, CommandError> {
    validate_worker_id(id)?;
    let body = form.to_body()?;
    let worker: WorkerResponse = client.put_json(&format!("/workers/{id}"), &body).await?;
    tracing::info!(worker_id = %id, "Worker profile updated");
    Ok(worker)
}

#[tauri::command]
pub async fn workers_delete(id: String) -> Result<(), CommandError> {
    workers_delete_with_client(get_api_client(), &id).await
}

/// Soft delete (RF-ADMIN-03-4): the server revokes the worker's credentials immediately.
pub async fn workers_delete_with_client(
    client: &AdminApiClient,
    id: &str,
) -> Result<(), CommandError> {
    validate_worker_id(id)?;
    client.delete(&format!("/workers/{id}")).await?;
    tracing::info!(worker_id = %id, "Worker soft-deleted");
    Ok(())
}

#[tauri::command]
pub async fn workers_grant_portal_access(
    id: String,
    email: String,
) -> Result<WorkerResponse, CommandError> {
    workers_grant_portal_access_with_client(get_api_client(), &id, &email).await
}

/// Enables portal access for an ex-worker (R38 art. 22.1, API TASK-27): read-only, 90 days.
/// The API only accepts `REVOKED` workers (`400 VALIDATION_ERROR` otherwise) and answers `409`
/// when another worker of this admin already uses the email.
pub async fn workers_grant_portal_access_with_client(
    client: &AdminApiClient,
    id: &str,
    email: &str,
) -> Result<WorkerResponse, CommandError> {
    validate_worker_id(id)?;
    let email = email.trim().to_lowercase();
    validate_personal_email(&email)
        .map_err(|_| validation_error("Ingresa un correo válido para el acceso al portal."))?;
    let body = serde_json::json!({ "email": email });
    let worker: WorkerResponse = client
        .post_json(&format!("/workers/{id}/portal-access"), &body)
        .await?;
    // The email itself is never logged.
    tracing::info!(worker_id = %id, until = ?worker.portal_access_until, "Ex-worker portal access granted");
    Ok(worker)
}

#[tauri::command]
pub async fn workers_revoke_portal_access(id: String) -> Result<(), CommandError> {
    workers_revoke_portal_access_with_client(get_api_client(), &id).await
}

/// Removes an ex-worker's portal access before it expires (API TASK-27, `204`).
pub async fn workers_revoke_portal_access_with_client(
    client: &AdminApiClient,
    id: &str,
) -> Result<(), CommandError> {
    validate_worker_id(id)?;
    client
        .delete(&format!("/workers/{id}/portal-access"))
        .await?;
    tracing::info!(worker_id = %id, "Ex-worker portal access removed");
    Ok(())
}

/// `XXXX-XXXX-XXXX` form of a 12-symbol code (the API may return it with or without hyphens).
fn format_qr_code(raw: &str) -> String {
    let clean: String = raw.chars().filter(|c| *c != '-' && *c != ' ').collect();
    if clean.len() == 12 {
        format!("{}-{}-{}", &clean[0..4], &clean[4..8], &clean[8..12])
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::test_support::admin_me_json;
    use crate::client::AdminApiClient;
    use crate::commands::keys::generate_and_save_master_keys;
    use telemetry_crypto_core::{generate_admin_keypair, verify_commitment};
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const API_URL: &str = "https://api.example.cl/api/v1";

    fn make_valid_schedule() -> ScheduleInput {
        ScheduleInput {
            journey_type: JourneyType::Fixed,
            exceptional_resolution: None,
            days: vec![ScheduleDay {
                weekday: 1,
                start: "08:30".into(),
                end: "17:30".into(),
                break_start: Some("13:00".into()),
                break_end: Some("14:00".into()),
            }],
            break_counts_as_work: false,
            disconnection: None,
            location_at_mark: false,
            effective_from: "2026-10-14".into(),
            requested_by: RequestedBy::Employer,
            note: None,
        }
    }

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
            run: "12.345.678-5".into(),
            personal_email: "pedro@acme.cl".into(),
            name: "Pedro Morales".into(),
            job_title: "Técnico".into(),
            workplace_id: "wp-1".into(),
            work_mode: WorkMode::OnSite,
            art22_exempt: false,
            schedule: Some(make_valid_schedule()),
            role_type: "FIELD".into(),
            consent_flags: flags,
            consent_descriptions: desc,
            location_cause: None,
            timezone: Some("America/Santiago".into()),
            enrollment_ttl_hours: Some(24),
        }
    }

    fn created_worker_json(form: &CreateWorkerForm, id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "admin_id": "admin-1",
            "email": form.personal_email,
            "personal_email": form.personal_email,
            "run": "12.345.678-5",
            "name": form.name,
            "job_title": form.job_title,
            "workplace": {
                "id": form.workplace_id,
                "name": "Casa matriz",
                "address": null
            },
            "work_mode": form.work_mode,
            "art22_exempt": form.art22_exempt,
            "schedule": {
                "id": "sched-1",
                "journey_type": "FIXED",
                "days": form.schedule.as_ref().map(|s| &s.days).cloned().unwrap_or_default(),
                "break_counts_as_work": false,
                "disconnection": null,
                "location_at_mark": false,
                "effective_from": "2026-10-14",
                "requested_by": "EMPLOYER",
                "note": null,
                "created_at": "2026-10-01T10:00:00Z",
                "warnings": null
            },
            "telemetry_status": "PENDING_DECISION",
            "terms_accepted_version": null,
            "warnings": null,
            "device_identifier": null,
            "device_info": null,
            "role_type": form.role_type,
            "is_active": true,
            "status": "PENDING_ENROLLMENT",
            "status_changed_at": null,
            "consent_flags": ConsentFlags::default(),
            "requested_consent_flags": form.consent_flags,
            "consent_descriptions": form.normalized_descriptions(),
            "consent_version": 1,
            "accepted_consent_version": null,
            "consent_accepted_at": null,
            "enrollment_completed_at": null,
            "enrollment_redeemed_at": null,
            "enrollment_expires_at": "2026-10-01T10:00:00Z",
            "enrollment_expired": false,
            "work_hours_start": "08:30:00",
            "work_hours_end": "17:30:00",
            "timezone": "America/Santiago",
            "created_at": "2026-09-30T10:00:00Z",
            "updated_at": "2026-09-30T10:00:00Z"
        })
    }

    /// The package exactly as `telemetry-api` builds it for `admin_pub` (server-side key).
    fn server_package(
        admin_pub: &str,
        worker: &serde_json::Value,
        code: &str,
    ) -> serde_json::Value {
        let worker: WorkerResponse = serde_json::from_value(worker.clone()).unwrap();
        let manifest = EnrollmentManifest::new(
            admin_pub.to_string(),
            worker.consent_version,
            worker.requested_consent_flags,
            worker.consent_descriptions,
        )
        .unwrap();
        let commitment = manifest_commitment(&manifest).unwrap();
        let qr = EnrollmentQr::new(API_URL.into(), code.into(), commitment.clone()).unwrap();
        serde_json::json!({
            "code": code,
            "expires_at": "2026-10-01T10:00:00Z",
            "commitment": commitment,
            "safety_code": safety_code(&commitment).unwrap(),
            "qr_payload": qr.to_payload().unwrap(),
        })
    }

    /// Server without a registered key (the local one gets registered first).
    async fn mount_admin_without_key(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(admin_me_json("Acme", None)))
            .mount(server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/admins/me/public-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "public_key": "registered", "public_key_fingerprint": "registered_fp" }
            })))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/admins/me/devices"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": [] })),
            )
            .mount(server)
            .await;
    }

    async fn mount_create(
        server: &MockServer,
        worker: serde_json::Value,
        package: serde_json::Value,
    ) {
        Mock::given(method("POST"))
            .and(path("/workers"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": { "worker": worker, "enrollment": package }
            })))
            .mount(server)
            .await;
    }

    fn client_for(server: &MockServer) -> AdminApiClient {
        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("tok".into()));
        client
    }

    #[test]
    fn test_workers_create_office_with_location_rejected_client_side() {
        let mut form = make_valid_form();
        form.role_type = "OFFICE".into();
        form.consent_flags.location = true;
        form.consent_descriptions.location = Some("Ubicacion GPS del trabajador".into());

        let err = form.validate().unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert!(err.message.contains("FIELD") || err.message.contains("OFFICE"));
    }

    #[tokio::test]
    async fn test_workers_create_verifies_server_enrollment_package() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        let local_pub = generate_and_save_master_keys().unwrap();
        let form = make_valid_form();
        let worker = created_worker_json(&form, "01912345-6789-7abc-def0-123456789aaa");
        mount_admin_without_key(&server).await;
        mount_create(
            &server,
            worker.clone(),
            server_package(&local_pub, &worker, "A68K-2PZS-PC4H"),
        )
        .await;

        let created = workers_create_with_client(&client_for(&server), form)
            .await
            .expect("workers_create failed");

        assert_eq!(created.enrollment.code, "A68K-2PZS-PC4H");
        assert_eq!(created.enrollment.safety_code.len(), 14); // "dddd dddd dddd"
        assert_eq!(created.enrollment.api_url, API_URL);
        let manifest = EnrollmentManifest::new(
            local_pub,
            created.worker.consent_version,
            created.worker.requested_consent_flags,
            created.worker.consent_descriptions,
        )
        .unwrap();
        let qr = EnrollmentQr::parse_payload(&created.enrollment.qr_payload).unwrap();
        assert!(verify_commitment(&manifest, &qr.commitment));
        assert_eq!(qr.commitment, created.enrollment.commitment);
    }

    #[tokio::test]
    async fn test_workers_create_rejects_foreign_commitment() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        let _local_pub = generate_and_save_master_keys().unwrap();
        // A compromised server computes a consistent package over ITS OWN key.
        let (_attacker_priv, attacker_pub) = generate_admin_keypair();
        let form = make_valid_form();
        let worker = created_worker_json(&form, "01912345-6789-7abc-def0-123456789bbb");
        mount_admin_without_key(&server).await;
        mount_create(
            &server,
            worker.clone(),
            server_package(&attacker_pub, &worker, "A68K-2PZS-PC4H"),
        )
        .await;

        let err = workers_create_with_client(&client_for(&server), form)
            .await
            .unwrap_err();
        assert_eq!(err.code, "ENROLLMENT_INTEGRITY_ERROR");
    }

    #[tokio::test]
    async fn test_workers_create_rejects_qr_payload_with_other_code() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        let local_pub = generate_and_save_master_keys().unwrap();
        let form = make_valid_form();
        let worker = created_worker_json(&form, "01912345-6789-7abc-def0-123456789ccc");
        let mut package = server_package(&local_pub, &worker, "A68K-2PZS-PC4H");
        package["code"] = serde_json::json!("B91M-4KPR-XZ2W");
        mount_admin_without_key(&server).await;
        mount_create(&server, worker, package).await;

        let err = workers_create_with_client(&client_for(&server), form)
            .await
            .unwrap_err();
        assert_eq!(err.code, "ENROLLMENT_INTEGRITY_ERROR");
    }

    #[tokio::test]
    async fn test_workers_create_tamper_check_rejects_modified_descriptions() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        let local_pub = generate_and_save_master_keys().unwrap();
        let form = make_valid_form();
        let mut worker = created_worker_json(&form, "worker-tampered-1");
        worker["consent_descriptions"]["system_activity"] =
            serde_json::json!("Tampered description by attacker");
        mount_admin_without_key(&server).await;
        mount_create(
            &server,
            worker.clone(),
            server_package(&local_pub, &worker, "A68K-2PZS-PC4H"),
        )
        .await;

        let err = workers_create_with_client(&client_for(&server), form)
            .await
            .unwrap_err();
        assert_eq!(err.code, "ENROLLMENT_TAMPER_CHECK_FAILED");
    }

    #[tokio::test]
    async fn test_server_key_mismatch_blocks_worker_creation() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        let _ = generate_and_save_master_keys().unwrap();
        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(admin_me_json("Acme", Some(&"f".repeat(64)))),
            )
            .mount(&server)
            .await;

        let err = workers_create_with_client(&client_for(&server), make_valid_form())
            .await
            .unwrap_err();
        assert_eq!(err.code, "PUBLIC_KEY_MISMATCH");
    }

    #[tokio::test]
    async fn test_workers_create_organization_missing_maps_reason() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        let _ = generate_and_save_master_keys().unwrap();
        mount_admin_without_key(&server).await;
        Mock::given(method("POST"))
            .and(path("/workers"))
            .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                "error": { "code": "CONFLICT", "message": "Organization name required",
                           "details": { "reason": "ORGANIZATION_NAME_MISSING" } }
            })))
            .mount(&server)
            .await;
        let err = workers_create_with_client(&client_for(&server), make_valid_form())
            .await
            .unwrap_err();
        assert_eq!(err.code, "CONFLICT");
        assert!(err.is("ORGANIZATION_NAME_MISSING"));
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
        let local_pub = generate_and_save_master_keys().unwrap();
        let form = make_valid_form();
        let secret_code = "SECR-99ET-9942";
        let worker = created_worker_json(&form, "01912345-6789-7abc-def0-123456789ddd");
        mount_admin_without_key(&server).await;
        mount_create(
            &server,
            worker.clone(),
            server_package(&local_pub, &worker, secret_code),
        )
        .await;

        let res = workers_create_with_client(&client_for(&server), form)
            .await
            .unwrap();
        assert_eq!(res.enrollment.code, secret_code);

        let logs = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert!(logs.contains("Worker created and enrollment QR issued successfully"));
        assert!(
            !logs.contains(secret_code),
            "Enrollment code leaked into logs!"
        );
        assert!(
            !logs.contains("SECR99ET9942"),
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
        let worker = created_worker_json(&form, "01912345-6789-7abc-def0-123456789eee");
        mount_admin_without_key(&server).await;
        mount_create(
            &server,
            worker.clone(),
            server_package(&admin_pub, &worker, "C74J-5MN8-9PQR"),
        )
        .await;

        let created = workers_create_with_client(&client_for(&server), form)
            .await
            .unwrap();

        // 1. Agent parses the canonical base64 QR payload
        let agent_parsed_qr = EnrollmentQr::parse_payload(&created.enrollment.qr_payload).unwrap();
        assert_eq!(agent_parsed_qr.code, "C74J-5MN8-9PQR");
        assert_eq!(agent_parsed_qr.api_url, API_URL);

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

    #[tokio::test]
    async fn test_workers_regenerate_parses_enrollment_shape_and_verifies() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        let local_pub = generate_and_save_master_keys().unwrap();
        let worker_id = "01912345-6789-7abc-def0-123456789fff";
        let worker = created_worker_json(&make_valid_form(), worker_id);
        Mock::given(method("POST"))
            .and(path(format!("/workers/{worker_id}/enrollment-token")))
            .and(wiremock::matchers::body_json(
                serde_json::json!({ "enrollment_ttl_hours": 48 }),
            ))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": { "enrollment": server_package(&local_pub, &worker, "B91M-4KPR-XZ2W") }
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/workers/{worker_id}")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": worker })),
            )
            .mount(&server)
            .await;
        let client = client_for(&server);

        let view = workers_regenerate_token_with_client(&client, worker_id, Some(48))
            .await
            .unwrap();
        assert_eq!(view.code, "B91M-4KPR-XZ2W");

        assert_eq!(
            workers_regenerate_token_with_client(&client, worker_id, Some(73))
                .await
                .unwrap_err()
                .code,
            "VALIDATION_ERROR"
        );
    }

    #[tokio::test]
    async fn test_workers_regenerate_revoke_first_maps_hint() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        generate_and_save_master_keys().unwrap();
        Mock::given(method("POST"))
            .and(path("/workers/w-1/enrollment-token"))
            .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                "error": {
                    "code": "CONFLICT", "message": "Worker already completed enrollment",
                    "details": { "reason": "ENROLLMENT_ALREADY_COMPLETED", "hint": "REVOKE_FIRST", "status": "ACTIVE" }
                }
            })))
            .mount(&server)
            .await;
        let err = workers_regenerate_token_with_client(&client_for(&server), "w-1", None)
            .await
            .unwrap_err();
        assert!(err.is("ENROLLMENT_ALREADY_COMPLETED"));
        let details = err.details.unwrap();
        assert_eq!(details["hint"], "REVOKE_FIRST");
        assert_eq!(details["status"], "ACTIVE");
    }

    #[test]
    fn test_validate_qr_api_url_rules() {
        assert!(validate_qr_api_url("https://api.example.cl/api/v1", false).is_ok());
        assert!(validate_qr_api_url("http://localhost:8080/api/v1", true).is_ok());
        assert!(validate_qr_api_url("http://localhost:8080/api/v1", false).is_err());
        assert!(validate_qr_api_url("https://user:pw@api.example.cl/api/v1", false).is_err());
        assert!(validate_qr_api_url("https://api.example.cl/api/v1#x", false).is_err());
        assert!(validate_qr_api_url("ftp://api.example.cl", true).is_err());
        assert!(validate_qr_api_url("https://", false).is_err());
    }

    #[tokio::test]
    async fn test_workers_consent_records_parse_granular_evidence() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/workers/w-1/consent-records"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    { "id": "r2", "worker_id": "w-1", "admin_id": "admin-1", "consent_version": 2,
                      "decision": "ACCEPTED",
                      "consent_flags": ConsentFlags { system_activity: true, network_activity: true, ..Default::default() },
                      "accepted_consent_flags": ConsentFlags { system_activity: true, ..Default::default() },
                      "consent_descriptions": ConsentDescriptions::default(), "manifest_hash": "ab".repeat(32),
                      "ip_address": "203.0.113.7", "user_agent": "telemetry-agent/0.3.0",
                      "app_version": "0.3.0", "created_at": "2026-09-30T10:00:00Z" },
                    { "id": "r1", "worker_id": "w-1", "consent_version": 1, "decision": "NARROWED",
                      "accepted_consent_flags": null, "manifest_hash": null,
                      "ip_address": null, "user_agent": null, "created_at": "2026-09-29T10:00:00Z" }
                ],
                "pagination": { "total": 2, "page": 1, "per_page": 100, "total_pages": 1 }
            })))
            .mount(&server)
            .await;
        let records = workers_get_consent_records_with_client(&client_for(&server), "w-1")
            .await
            .unwrap();
        assert_eq!(records.len(), 2);
        let accepted = records[0].accepted_consent_flags.unwrap();
        assert!(accepted.system_activity && !accepted.network_activity);
        assert_eq!(records[0].app_version.as_deref(), Some("0.3.0"));
        assert!(records[1].manifest_hash.is_none());
    }

    #[test]
    fn test_workers_work_hours_malformed_rejected() {
        for bad in ["25:99", "9", "09-00", "", "24:00"] {
            let err = validate_work_hours(bad, "18:00").unwrap_err();
            assert_eq!(err.code, "VALIDATION_ERROR", "accepted {bad:?}");
        }
        assert!(validate_work_hours("18:00", "09:00").is_err());
        assert!(validate_work_hours("09:00", "09:00").is_err());
        assert!(validate_work_hours("09:00", "18:00:00").is_ok());
    }

    #[test]
    fn test_workers_work_hour_normalized_to_seconds() {
        assert_eq!(normalize_work_hour("08:30").unwrap(), "08:30:00");
        assert_eq!(normalize_work_hour(" 17:45:00 ").unwrap(), "17:45:00");
        assert!(normalize_work_hour("25:99").is_err());
    }

    #[test]
    fn test_workers_timezone_iana_format_validated() {
        for ok in [
            "America/Santiago",
            "America/Argentina/Buenos_Aires",
            "Etc/GMT-3",
            "UTC",
        ] {
            assert!(validate_timezone(ok).is_ok(), "rejected {ok}");
        }
        for bad in [
            "",
            "Santiago",
            "Mars/Olympus",
            "America/",
            "America/San tiago",
            "GMT+3",
        ] {
            assert!(validate_timezone(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn test_workers_create_description_limit_counts_chars_not_bytes() {
        let mut form = make_valid_form();
        // 500 two-byte characters = 1000 bytes, still within the 500-char limit.
        form.consent_descriptions.system_activity = Some("é".repeat(500));
        assert!(form.validate().is_ok());
        form.consent_descriptions.system_activity = Some("é".repeat(501));
        assert!(form.validate().is_err());
    }

    #[test]
    fn test_workers_create_ttl_out_of_range_rejected() {
        let mut form = make_valid_form();
        form.enrollment_ttl_hours = Some(0);
        assert!(form.validate().is_err());
        form.enrollment_ttl_hours = Some(73);
        assert!(form.validate().is_err());
        form.enrollment_ttl_hours = Some(72);
        assert!(form.validate().is_ok());
    }

    #[test]
    fn test_workers_list_query_builds_encoded_filters() {
        let q = WorkerListQuery {
            page: Some(2),
            per_page: Some(500),
            search: Some("ana & josé".into()),
            role_type: Some("field".into()),
            status: Some("active".into()),
            is_active: Some(true),
            workplace_id: Some("01927d3b-65e1-7000-8000-0000000000b1".into()),
            work_mode: Some("remote".into()),
        };
        // `workplace_id` / `work_mode` are not API parameters: applied locally (RF-ADMIN-23-1).
        assert_eq!(
            q.to_endpoint().unwrap(),
            "/workers?page=2&per_page=100&search=ana%20%26%20jos%C3%A9&role_type=FIELD&status=ACTIVE&is_active=true"
        );
        let filters = q.local_filters().unwrap();
        assert_eq!(
            filters.workplace_id.as_deref(),
            Some("01927d3b-65e1-7000-8000-0000000000b1")
        );
        assert_eq!(filters.work_mode, Some(WorkMode::Remote));
    }

    #[test]
    fn test_workers_list_local_filters_validated() {
        let bad_wp = WorkerListQuery {
            workplace_id: Some("wp 1&x=1".into()),
            ..Default::default()
        };
        assert_eq!(bad_wp.local_filters().unwrap_err().code, "VALIDATION_ERROR");
        let bad_mode = WorkerListQuery {
            work_mode: Some("ANYWHERE".into()),
            ..Default::default()
        };
        assert_eq!(
            bad_mode.local_filters().unwrap_err().code,
            "VALIDATION_ERROR"
        );
        let blank = WorkerListQuery {
            workplace_id: Some("  ".into()),
            work_mode: Some(String::new()),
            ..Default::default()
        };
        assert!(blank.local_filters().unwrap().is_empty());
    }

    #[test]
    fn test_workers_paginate_locally_slices_and_clamps() {
        let items: Vec<u32> = (1..=45).collect();
        let page = paginate_locally(items.clone(), 3, 20);
        assert_eq!(page.data, (41..=45).collect::<Vec<_>>());
        assert_eq!(
            page.pagination,
            Some(PaginationMeta {
                total: 45,
                page: 3,
                per_page: 20,
                total_pages: 3
            })
        );
        // Page beyond the end ⇒ last page; per_page clamped to 1..=100.
        let page = paginate_locally(items, 9, 500);
        assert_eq!(page.data.len(), 45);
        assert_eq!(
            page.pagination.map(|p| (p.page, p.total_pages)),
            Some((1, 1))
        );
        let empty = paginate_locally(Vec::<u32>::new(), 1, 20);
        assert!(empty.data.is_empty());
        assert_eq!(
            empty.pagination.map(|p| (p.total, p.total_pages)),
            Some((0, 1))
        );
    }

    #[test]
    fn test_workers_list_query_short_search_and_empty_omitted() {
        let q = WorkerListQuery {
            search: Some("a".into()),
            ..Default::default()
        };
        assert_eq!(q.to_endpoint().unwrap(), "/workers");
    }

    #[test]
    fn test_workers_list_query_invalid_status_rejected() {
        let q = WorkerListQuery {
            status: Some("INACTIVE".into()),
            ..Default::default()
        };
        assert_eq!(q.to_endpoint().unwrap_err().code, "VALIDATION_ERROR");
    }

    #[test]
    fn test_workers_update_body_contains_only_provided_fields() {
        let form = UpdateWorkerForm {
            name: Some("  Ana Díaz ".into()),
            job_title: Some("  Diseñadora ".into()),
            work_mode: Some(WorkMode::Remote),
            ..Default::default()
        };
        assert_eq!(
            form.to_body().unwrap(),
            serde_json::json!({
                "name": "Ana Díaz",
                "job_title": "Diseñadora",
                "work_mode": "REMOTE"
            })
        );
    }

    #[test]
    fn test_workers_update_invalid_forms_rejected() {
        assert!(UpdateWorkerForm::default().to_body().is_err());
        let bad_run = UpdateWorkerForm {
            run: Some("12.345.678-0".into()),
            ..Default::default()
        };
        assert!(bad_run.to_body().is_err());
        let bad_email = UpdateWorkerForm {
            personal_email: Some("bad-email".into()),
            ..Default::default()
        };
        assert!(bad_email.to_body().is_err());
        let bad_role = UpdateWorkerForm {
            role_type: Some("MANAGER".into()),
            ..Default::default()
        };
        assert!(bad_role.to_body().is_err());
        let blank_name = UpdateWorkerForm {
            name: Some("   ".into()),
            ..Default::default()
        };
        assert!(blank_name.to_body().is_err());
    }

    fn worker_json(name: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "w-1", "admin_id": "admin-1", "email": "ana@acme.cl", "name": name,
            "personal_email": "ana@acme.cl", "run": "12.345.678-5", "job_title": "Analista",
            "workplace": { "id": "wp-1", "name": "Casa matriz", "address": null },
            "work_mode": "ON_SITE", "art22_exempt": false, "schedule": null,
            "telemetry_status": "NOT_REQUESTED", "terms_accepted_version": null,
            "warnings": null,
            "device_identifier": null, "device_info": null,
            "role_type": "OFFICE", "is_active": true, "status": "ACTIVE",
            "status_changed_at": null,
            "consent_flags": ConsentFlags::default(),
            "requested_consent_flags": ConsentFlags::default(),
            "consent_descriptions": ConsentDescriptions::default(),
            "consent_version": 1, "accepted_consent_version": 1,
            "consent_accepted_at": null, "enrollment_completed_at": null,
            "enrollment_expires_at": null,
            "work_hours_start": "08:00:00", "work_hours_end": "17:00:00",
            "timezone": "America/Santiago",
            "created_at": "2026-09-28T00:00:00Z", "updated_at": "2026-09-29T00:00:00Z"
        })
    }

    #[tokio::test]
    async fn test_workers_update_sends_put_and_returns_worker() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/workers/w-1"))
            .and(wiremock::matchers::body_json(
                serde_json::json!({ "name": "Ana Díaz" }),
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "data": worker_json("Ana Díaz") })),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("tok".into()));
        let form = UpdateWorkerForm {
            name: Some("Ana Díaz".into()),
            ..Default::default()
        };
        let updated = workers_update_with_client(&client, "w-1", form)
            .await
            .unwrap();
        assert_eq!(updated.name, "Ana Díaz");
    }

    #[tokio::test]
    async fn test_workers_update_surfaces_server_reason() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/workers/w-1"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": {
                    "code": "VALIDATION_ERROR",
                    "message": "location requires FIELD",
                    "details": { "reason": "LOCATION_REQUIRES_FIELD" }
                }
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("tok".into()));
        let form = UpdateWorkerForm {
            role_type: Some("OFFICE".into()),
            ..Default::default()
        };
        let err = workers_update_with_client(&client, "w-1", form)
            .await
            .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert!(err.is("LOCATION_REQUIRES_FIELD"));
        assert_eq!(err.message, "location requires FIELD");
    }

    #[tokio::test]
    async fn test_workers_delete_sends_soft_delete() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/workers/w-1"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("tok".into()));
        workers_delete_with_client(&client, "w-1").await.unwrap();
    }

    #[tokio::test]
    async fn test_workers_set_status_rejects_paused_locally() {
        let client = AdminApiClient::new("http://127.0.0.1:9".into());
        let err = workers_set_status_with_client(&client, "w-1", "PAUSED", None)
            .await
            .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert!(err.message.contains("REVOKED"));

        let err_active = workers_set_status_with_client(&client, "w-1", "ACTIVE", None)
            .await
            .unwrap_err();
        assert_eq!(err_active.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn test_workers_set_status_sends_revoked_body() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/workers/w-1/status"))
            .and(wiremock::matchers::body_json(
                serde_json::json!({ "status": "REVOKED", "reason": "Término de contrato" }),
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "data": worker_json("Ana") })),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("tok".into()));
        workers_set_status_with_client(
            &client,
            "w-1",
            "revoked",
            Some("Término de contrato".into()),
        )
        .await
        .unwrap();
    }

    fn revoked_worker_json(until: Option<&str>) -> serde_json::Value {
        let mut w = worker_json("Ana");
        w["status"] = serde_json::json!("REVOKED");
        w["is_active"] = serde_json::json!(false);
        w["portal_access_until"] = serde_json::json!(until);
        w
    }

    #[test]
    fn test_worker_response_portal_access_until_is_optional() {
        let without: WorkerResponse = serde_json::from_value(worker_json("Ana")).unwrap();
        assert_eq!(without.portal_access_until, None);
        let with: WorkerResponse =
            serde_json::from_value(revoked_worker_json(Some("2027-01-19T15:00:00Z"))).unwrap();
        assert_eq!(
            with.portal_access_until.as_deref(),
            Some("2027-01-19T15:00:00Z")
        );
    }

    #[tokio::test]
    async fn test_workers_grant_portal_access_posts_normalized_email() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/workers/w-1/portal-access"))
            .and(wiremock::matchers::header("authorization", "Bearer tok"))
            .and(wiremock::matchers::body_json(
                serde_json::json!({ "email": "ex.ana@correo.cl" }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "data": revoked_worker_json(Some("2027-01-19T15:00:00Z")) }),
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("tok".into()));
        let worker = workers_grant_portal_access_with_client(&client, "w-1", "  Ex.Ana@Correo.CL ")
            .await
            .unwrap();
        assert_eq!(worker.status, WorkerStatus::Revoked);
        assert_eq!(
            worker.portal_access_until.as_deref(),
            Some("2027-01-19T15:00:00Z")
        );
    }

    #[tokio::test]
    async fn test_workers_grant_portal_access_rejects_bad_input_locally() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("tok".into()));

        for bad in [
            "",
            "sin-arroba",
            "a@b",
            "con espacio@correo.cl",
            "a@@correo.cl",
        ] {
            let err = workers_grant_portal_access_with_client(&client, "w-1", bad)
                .await
                .unwrap_err();
            assert_eq!(err.code, "VALIDATION_ERROR", "{bad}");
        }
        let err = workers_grant_portal_access_with_client(&client, "../admins", "a@correo.cl")
            .await
            .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
        let err = workers_revoke_portal_access_with_client(&client, "w 1")
            .await
            .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn test_workers_grant_portal_access_maps_api_errors() {
        let cases = [
            (400, "VALIDATION_ERROR", "Worker is not REVOKED"),
            (404, "NOT_FOUND", "Worker not found"),
            (409, "CONFLICT", "Email already in use"),
        ];
        for (status, code, message) in cases {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/workers/w-1/portal-access"))
                .respond_with(
                    ResponseTemplate::new(status).set_body_json(serde_json::json!({
                        "error": { "code": code, "message": message }
                    })),
                )
                .expect(1)
                .mount(&server)
                .await;
            let client = AdminApiClient::new(server.uri());
            client.set_access_token_for_test(Some("tok".into()));
            let err = workers_grant_portal_access_with_client(&client, "w-1", "ex@correo.cl")
                .await
                .unwrap_err();
            assert_eq!(err.code, code);
            assert_eq!(err.message, message);
        }
    }

    #[tokio::test]
    async fn test_workers_revoke_portal_access_sends_delete() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/workers/w-1/portal-access"))
            .and(wiremock::matchers::header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("tok".into()));
        workers_revoke_portal_access_with_client(&client, "w-1")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_workers_revoke_portal_access_maps_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/workers/w-9/portal-access"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "error": { "code": "NOT_FOUND", "message": "Worker not found" }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("tok".into()));
        let err = workers_revoke_portal_access_with_client(&client, "w-9")
            .await
            .unwrap_err();
        assert_eq!(err.code, "NOT_FOUND");
    }

    #[tokio::test]
    async fn test_workers_set_telemetry_status_patch_json() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/workers/w-1/telemetry-status"))
            .and(wiremock::matchers::body_json(
                serde_json::json!({ "status": "PAUSED", "reason": "Licencia médica" }),
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "data": worker_json("Ana") })),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("tok".into()));
        let updated = workers_set_telemetry_status_with_client(
            &client,
            "w-1",
            "PAUSED",
            Some("Licencia médica".into()),
        )
        .await
        .unwrap();
        assert_eq!(updated.name, "Ana");
    }

    #[tokio::test]
    async fn test_workers_update_schedule_put_json() {
        let server = MockServer::start().await;
        let schedule_input = make_valid_schedule();
        Mock::given(method("GET"))
            .and(path("/workers/w-1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "data": worker_json("Ana") })),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("PUT"))
            .and(path("/workers/w-1/schedule"))
            .and(wiremock::matchers::body_json(
                serde_json::to_value(&schedule_input).unwrap(),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "schedule": {
                        "id": "sched-2",
                        "journey_type": "FIXED",
                        "days": schedule_input.days,
                        "break_counts_as_work": false,
                        "disconnection": null,
                        "location_at_mark": false,
                        "effective_from": "2026-10-14",
                        "requested_by": "EMPLOYER",
                        "note": null,
                        "created_at": "2026-10-02T10:00:00Z",
                        "warnings": ["WEEKLY_MAX_EXCEEDED"]
                    },
                    "warnings": ["WEEKLY_MAX_EXCEEDED"]
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("tok".into()));
        let res = workers_update_schedule_with_client(&client, "w-1", schedule_input)
            .await
            .unwrap();
        assert_eq!(res.warnings, vec!["WEEKLY_MAX_EXCEEDED".to_string()]);
    }

    #[tokio::test]
    async fn test_workers_create_v3_exact_body() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        let local_pub = generate_and_save_master_keys().unwrap();
        mount_admin_without_key(&server).await;

        let form = make_valid_form();
        let worker_val = created_worker_json(&form, "worker-v3-1");

        let expected_body = serde_json::json!({
            "name": "Pedro Morales",
            "email": "pedro@acme.cl",
            "personal_email": "pedro@acme.cl",
            "run": "12.345.678-5",
            "job_title": "Técnico",
            "workplace_id": "wp-1",
            "work_mode": "ON_SITE",
            "art22_exempt": false,
            "role_type": "FIELD",
            "consent_flags": form.consent_flags,
            "consent_descriptions": form.normalized_descriptions(),
            "enrollment_ttl_hours": 24,
            "timezone": "America/Santiago",
            "schedule": serde_json::to_value(form.schedule.as_ref().unwrap()).unwrap(),
        });

        Mock::given(method("POST"))
            .and(path("/workers"))
            .and(wiremock::matchers::body_json(expected_body))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {
                    "worker": worker_val.clone(),
                    "enrollment": server_package(&local_pub, &worker_val, "A68K-2PZS-PC4H"),
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let created = workers_create_with_client(&client_for(&server), form)
            .await
            .expect("workers_create failed");
        assert_eq!(created.worker.run, "12.345.678-5");
        assert_eq!(created.worker.job_title, "Técnico");
    }

    #[test]
    fn test_workers_create_v3_screenshots_remote_rejected_client_side() {
        let mut form = make_valid_form();
        form.work_mode = WorkMode::Remote;
        form.consent_flags.productivity_screenshots = true;
        form.consent_descriptions.productivity_screenshots = Some("Capturas periódicas".into());
        if let Some(s) = form.schedule.as_mut() {
            s.disconnection = Some(DisconnectionWindow {
                start: "20:00".into(),
                end: "08:00".into(),
            });
        }
        let err = form.validate().unwrap_err();
        assert_eq!(err.code, "SCREENSHOTS_NOT_ALLOWED_REMOTE");

        form.work_mode = WorkMode::Hybrid;
        let err_hybrid = form.validate().unwrap_err();
        assert_eq!(err_hybrid.code, "SCREENSHOTS_NOT_ALLOWED_REMOTE");
    }

    #[test]
    fn test_workers_create_v3_location_cause_required_client_side() {
        let mut form = make_valid_form();
        form.role_type = "FIELD".into();
        form.consent_flags.location = true;
        form.consent_descriptions.location = Some("Ubicación en terreno".into());
        form.location_cause = None;

        let err = form.validate().unwrap_err();
        assert_eq!(err.code, "LOCATION_CAUSE_REQUIRED");

        form.location_cause = Some("Corta".into());
        let err_short = form.validate().unwrap_err();
        assert_eq!(err_short.code, "LOCATION_CAUSE_REQUIRED");

        form.location_cause =
            Some("Causa de seguridad vial en rutas interurbanas de reparto".into());
        assert!(form.validate().is_ok());
    }

    #[tokio::test]
    async fn test_workers_create_v3_employer_profile_missing_surfaces_api_error() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        let _ = generate_and_save_master_keys().unwrap();
        mount_admin_without_key(&server).await;
        Mock::given(method("POST"))
            .and(path("/workers"))
            .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                "error": {
                    "code": "CONFLICT",
                    "message": "Employer profile required",
                    "details": { "reason": "EMPLOYER_PROFILE_MISSING" }
                }
            })))
            .mount(&server)
            .await;

        let err = workers_create_with_client(&client_for(&server), make_valid_form())
            .await
            .unwrap_err();
        assert_eq!(err.code, "CONFLICT");
        assert!(err.is("EMPLOYER_PROFILE_MISSING"));
    }

    #[tokio::test]
    async fn test_workers_create_v3_schedule_warnings_collected() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        let local_pub = generate_and_save_master_keys().unwrap();
        mount_admin_without_key(&server).await;

        let form = make_valid_form();
        let worker_val = created_worker_json(&form, "worker-warn-1");

        Mock::given(method("POST"))
            .and(path("/workers"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {
                    "worker": worker_val.clone(),
                    "enrollment": server_package(&local_pub, &worker_val, "A68K-2PZS-PC4H"),
                    "warnings": ["DAILY_MAX_EXCEEDED", "WEEKLY_MAX_EXCEEDED"]
                }
            })))
            .mount(&server)
            .await;

        let created = workers_create_with_client(&client_for(&server), form)
            .await
            .expect("workers_create failed");
        assert_eq!(
            created.warnings,
            vec![
                "DAILY_MAX_EXCEEDED".to_string(),
                "WEEKLY_MAX_EXCEEDED".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn test_workers_consent_config_missing_description_rejected() {
        let client = AdminApiClient::new("http://127.0.0.1:9".into());
        let flags = ConsentFlags {
            file_activity: true,
            ..Default::default()
        };
        let err = workers_update_consent_config_with_client(
            &client,
            "w-1",
            flags,
            ConsentDescriptions::default(),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn test_workers_consent_config_drops_disabled_descriptions() {
        let flags = ConsentFlags {
            system_activity: true,
            ..Default::default()
        };
        let desc = ConsentDescriptions {
            system_activity: Some("Aplicaciones en uso en horario".into()),
            network_activity: Some("Texto de una categoría desactivada".into()),
            ..Default::default()
        };
        let mut expected_desc = desc.clone();
        expected_desc.retain_enabled(&flags);

        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/workers/w-1/consent-config"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "consent_flags": flags,
                "consent_descriptions": expected_desc,
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "consent_version": 2,
                    "requires_worker_acceptance": true,
                    "worker": worker_json("Ana")
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("tok".into()));
        let resp = workers_update_consent_config_with_client(&client, "w-1", flags, desc)
            .await
            .unwrap();
        assert_eq!(resp.consent_version, 2);
        assert!(resp.requires_worker_acceptance);
    }

    fn worker_at(id: &str, workplace: Option<&str>, mode: &str) -> serde_json::Value {
        let mut w = worker_json("Ana");
        w["id"] = serde_json::json!(id);
        w["workplace"] = match workplace {
            Some(wp) => serde_json::json!({ "id": wp, "name": "Lugar", "address": null }),
            None => serde_json::Value::Null,
        };
        w["work_mode"] = serde_json::json!(mode);
        w
    }

    #[tokio::test]
    async fn test_workers_list_filters_workplace_and_mode_across_pages() {
        const WP_A: &str = "01927d3b-65e1-7000-8000-0000000000b1";
        const WP_B: &str = "01927d3b-65e1-7000-8000-0000000000b2";
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/workers"))
            .and(query_param("page", "1"))
            .and(query_param("per_page", "100"))
            .and(query_param("status", "ACTIVE"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [worker_at("w1", Some(WP_A), "REMOTE"), worker_at("w2", Some(WP_B), "REMOTE"),
                         worker_at("w3", None, "REMOTE")],
                "pagination": { "total": 5, "page": 1, "per_page": 100, "total_pages": 2 }
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/workers"))
            .and(query_param("page", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [worker_at("w4", Some(WP_A), "ON_SITE"), worker_at("w5", Some(WP_A), "REMOTE")],
                "pagination": { "total": 5, "page": 2, "per_page": 100, "total_pages": 2 }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let query = WorkerListQuery {
            page: Some(1),
            per_page: Some(1),
            status: Some("ACTIVE".into()),
            workplace_id: Some(WP_A.to_uppercase()),
            work_mode: Some("REMOTE".into()),
            ..Default::default()
        };
        let resp = workers_list_with_client(&client_for(&server), Some(query))
            .await
            .unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].id, "w1");
        assert_eq!(
            resp.pagination,
            Some(PaginationMeta {
                total: 2,
                page: 1,
                per_page: 1,
                total_pages: 2
            })
        );
        for req in server.received_requests().await.unwrap_or_default() {
            let q = req.url.query().unwrap_or_default().to_string();
            assert!(
                !q.contains("workplace_id") && !q.contains("work_mode"),
                "{q}"
            );
        }
    }

    #[tokio::test]
    async fn test_workers_list_local_filter_stops_at_page_cap() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/workers"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [worker_at("w1", None, "ON_SITE")],
                "pagination": { "total": 5000, "page": 1, "per_page": 100, "total_pages": 50 }
            })))
            .mount(&server)
            .await;
        let query = WorkerListQuery {
            work_mode: Some("REMOTE".into()),
            ..Default::default()
        };
        let err = workers_list_with_client(&client_for(&server), Some(query))
            .await
            .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(
            server.received_requests().await.unwrap_or_default().len(),
            MAX_LOCAL_FILTER_PAGES
        );
    }
}
