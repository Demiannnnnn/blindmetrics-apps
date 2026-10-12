//! Attendance console commands: live board, days/marks, hash verification, and corrections (TASK-ADMIN-15).
//!
//! Enforces:
//! * ATT-03: Local hash verification using `telemetry_crypto_core::attendance::mark_hash`.
//! * ATT-05: Corrections only through the R38 art. 39-41 procedure (no deletion, reason 5-500, no harm to worker, from next business day).
//! * ATT-09: No client-side recalculation of missing/extra time. Days range validated to <= 62 days in Rust.
//! * BILL-04: All attendance GET endpoints are allowed while subscription is blocked.

use crate::client::{get_api_client, AdminApiClient, CollectionResponse};
use crate::commands::CommandError;
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use telemetry_crypto_core::attendance::{
    is_canonical_uuid, verify_mark_hash, CanonicalMark, MarkLocation, MarkSource, MarkType,
};

// ============================================================================
// DTOs & Domain Types
// ============================================================================

/// State of a worker on the live attendance board.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttendanceState {
    Working,
    OnBreak,
    Off,
    Unmarked,
    #[serde(other)]
    Unknown,
}

/// Workplace summary for the live board or marks.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BoardWorkplace {
    pub id: String,
    pub name: String,
}

/// Workplace info on a mark.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MarkWorkplace {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

/// Canonical attendance mark returned by the server (api-contract §9.1).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AttendanceMark {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_mark_id: Option<String>,
    pub worker_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub employer_rut: Option<String>,
    #[serde(rename = "type")]
    pub r#type: MarkType,
    pub server_ts: String,
    pub device_ts: String,
    pub offline: bool,
    pub source: MarkSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identification: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workplace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workplace: Option<MarkWorkplace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<MarkLocation>,
    pub hash: String,
    pub prev_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrected_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    /// Locally computed verification of `hash` using `telemetry_crypto_core::attendance::mark_hash`.
    /// `Some(true)` if valid, `Some(false)` if mismatch or verification failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash_verified: Option<bool>,
}

/// Verifies `mark.hash` locally using `telemetry_crypto_core::attendance::mark_hash` (ATT-03).
pub fn verify_mark(mark: &mut AttendanceMark) {
    let effective_mark_id = mark
        .mark_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(&mark.id);

    let Some(run) = &mark.worker_run else {
        mark.hash_verified = Some(false);
        return;
    };
    let Some(rut) = &mark.employer_rut else {
        mark.hash_verified = Some(false);
        return;
    };

    let workplace_id = mark
        .workplace_id
        .clone()
        .or_else(|| mark.workplace.as_ref().map(|w| w.id.clone()));

    let canonical = CanonicalMark {
        v: 1,
        mark_id: effective_mark_id.to_string(),
        worker_id: mark.worker_id.clone(),
        worker_run: run.clone(),
        employer_rut: rut.clone(),
        r#type: mark.r#type,
        server_ts: mark.server_ts.clone(),
        device_ts: mark.device_ts.clone(),
        offline: mark.offline,
        source: mark.source,
        workplace_id,
        location: mark.location.clone(),
        prev_hash: mark.prev_hash.clone(),
    };

    mark.hash_verified = Some(verify_mark_hash(&canonical, &mark.hash));
}

/// Row of the live attendance board (api-contract §9.4, AS1).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BoardWorker {
    pub worker_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workplace: Option<BoardWorkplace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_mode: Option<String>,
    pub state: AttendanceState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_mark: Option<AttendanceMark>,
    #[serde(default)]
    pub today_marks: Vec<AttendanceMark>,
    #[serde(default)]
    pub alerts: Vec<String>,
}

/// Agreed schedule for a day.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AgreedDaySchedule {
    pub start: String,
    pub end: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#break: Option<String>,
    #[serde(default)]
    pub break_counts_as_work: bool,
}

/// Justification of an absence.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AbsenceType {
    Justified,
    Unjustified,
}

/// Server-computed summary of a day's attendance (api-contract §9.5, AS2).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DaySummary {
    pub date: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agreed: Option<AgreedDaySchedule>,
    #[serde(default)]
    pub marks: Vec<AttendanceMark>,
    pub attended: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absence: Option<AbsenceType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absence_note: Option<String>,
    pub missing: String,
    pub extra: String,
    #[serde(default)]
    pub anomalies: Vec<String>,
    #[serde(default)]
    pub altered: bool,
}

/// Query parameters for fetching marks.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct AttendanceMarksQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_page: Option<usize>,
}

/// Kind of attendance correction (R38 art. 39).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CorrectionKind {
    Modify,
    Add,
    Fix,
}

/// Status of an attendance correction (R38 art. 40).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CorrectionStatus {
    Pending,
    Accepted,
    Opposed,
    Consolidated,
    Withdrawn,
}

/// Record of an attendance correction proposal (api-contract §9.4).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Correction {
    pub id: String,
    pub worker_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_name: Option<String>,
    pub kind: CorrectionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_mark: Option<AttendanceMark>,
    #[serde(rename = "type")]
    pub r#type: MarkType,
    pub proposed_ts: String,
    pub reason: String,
    pub status: CorrectionStatus,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notified_at: Option<String>,
    pub respond_before: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responded_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resulting_mark_id: Option<String>,
}

/// Input payload to propose an attendance correction (RF-ADMIN-15-5).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProposeCorrectionInput {
    pub worker_ids: Vec<String>,
    pub kind: CorrectionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_mark_id: Option<String>,
    #[serde(rename = "type")]
    pub r#type: MarkType,
    pub proposed_ts: String,
    pub reason: String,
    /// Must be true to submit (RF-ADMIN-15-5).
    pub confirm_no_harm: bool,
    /// Original mark timestamp for local validation of harm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_mark_ts: Option<String>,
    /// Worker's time zone (e.g. "America/Santiago"). Defaults to "America/Santiago".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

/// Query parameters for fetching corrections.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct AttendanceCorrectionsQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<CorrectionStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_page: Option<usize>,
}

// ============================================================================
// Validations
// ============================================================================

/// Validates date range for `attendance_days` (RF-ADMIN-15-2).
/// Must be valid dates and range <= 62 days.
pub fn validate_days_range(
    from_str: &str,
    to_str: &str,
) -> Result<(NaiveDate, NaiveDate), CommandError> {
    let parse_date = |s: &str, name: &str| -> Result<NaiveDate, CommandError> {
        let trimmed = s.trim();
        if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
            return Ok(d);
        }
        if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
            return Ok(dt.date_naive());
        }
        Err(CommandError::validation(format!(
            "El parámetro '{}' debe tener formato YYYY-MM-DD o RFC 3339.",
            name
        )))
    };

    let from = parse_date(from_str, "from")?;
    let to = parse_date(to_str, "to")?;

    if from > to {
        return Err(CommandError::validation(
            "La fecha de inicio ('from') no puede ser posterior a la de término ('to').",
        ));
    }

    let days = (to - from).num_days();
    if days > 62 {
        return Err(CommandError::validation(
            "El rango de fechas no puede superar los 62 días (R38 art. 25).",
        ));
    }

    Ok((from, to))
}

/// Validates correction form fields in Rust with injected `now` parameter (RF-ADMIN-15-5).
pub fn validate_correction_input_at(
    form: &ProposeCorrectionInput,
    now: DateTime<Utc>,
) -> Result<(), CommandError> {
    if !form.confirm_no_harm {
        return Err(CommandError::validation(
            "Debes confirmar que esta corrección no perjudica al trabajador.",
        ));
    }

    if form.worker_ids.is_empty() {
        return Err(CommandError::validation(
            "Debes seleccionar al menos un trabajador para la corrección.",
        ));
    }

    for wid in &form.worker_ids {
        if !is_canonical_uuid(wid) {
            return Err(CommandError::validation(format!(
                "El identificador del trabajador '{}' no es un UUID canónico.",
                wid
            )));
        }
    }

    let reason = form.reason.trim();
    if reason.chars().count() < 5 || reason.chars().count() > 500 {
        return Err(CommandError::validation(
            "El motivo de la corrección debe tener entre 5 y 500 caracteres.",
        ));
    }

    // MODIFY requires original_mark_id
    if form.kind == CorrectionKind::Modify {
        let orig_id = form.original_mark_id.as_deref().unwrap_or("").trim();
        if orig_id.is_empty() || !is_canonical_uuid(orig_id) {
            return Err(CommandError {
                code: "VALIDATION_ERROR".to_string(),
                message: "Debes seleccionar la marca original que deseas modificar.".to_string(),
                reason: Some("ORIGINAL_MARK_REQUIRED".to_string()),
                details: None,
            });
        }
    }

    // Worker's time zone (defaults to America/Santiago)
    let tz: Tz = form
        .timezone
        .as_deref()
        .unwrap_or("America/Santiago")
        .parse()
        .unwrap_or(chrono_tz::America::Santiago);

    // Proposed timestamp must be valid RFC 3339
    let proposed_dt = DateTime::parse_from_rfc3339(form.proposed_ts.trim()).map_err(|_| {
        CommandError::validation(
            "La fecha/hora propuesta debe estar en formato RFC 3339 con precisión de segundos.",
        )
    })?;

    // Convert proposed_dt and now to worker's time zone before comparing dates
    let proposed_in_tz = proposed_dt.with_timezone(&tz);
    let now_in_tz = now.with_timezone(&tz);

    // R38 art. 41: Corrections are allowed only from the next business day (not same day or future)
    let proposed_date = proposed_in_tz.date_naive();
    let today_date = now_in_tz.date_naive();
    if proposed_date >= today_date {
        return Err(CommandError {
            code: "VALIDATION_ERROR".to_string(),
            message: "Solo puedes proponer correcciones desde el día hábil siguiente a la jornada corregida.".to_string(),
            reason: Some("CORRECTION_TOO_EARLY".to_string()),
            details: None,
        });
    }

    // Check that correction does not harm the worker (CORRECTION_HARMS_WORKER):
    // The same zone must be used for the WORKDAY_START/END ordering check.
    // For WORKDAY_START: proposed start can only be earlier (proposed_ts <= original_ts).
    // For WORKDAY_END: proposed end can only be later (proposed_ts >= original_ts).
    if form.kind == CorrectionKind::Modify {
        if let Some(orig_ts_str) = &form.original_mark_ts {
            if let Ok(orig_dt) = DateTime::parse_from_rfc3339(orig_ts_str.trim()) {
                let orig_in_tz = orig_dt.with_timezone(&tz);
                match form.r#type {
                    MarkType::WorkdayStart if proposed_in_tz > orig_in_tz => {
                        return Err(CommandError {
                            code: "VALIDATION_ERROR".to_string(),
                            message: "La corrección de inicio de jornada solo puede adelantar la entrada; postergarla perjudica al trabajador.".to_string(),
                            reason: Some("CORRECTION_HARMS_WORKER".to_string()),
                            details: None,
                        });
                    }
                    MarkType::WorkdayEnd if proposed_in_tz < orig_in_tz => {
                        return Err(CommandError {
                            code: "VALIDATION_ERROR".to_string(),
                            message: "La corrección de término de jornada solo puede postergar la salida; adelantarla perjudica al trabajador.".to_string(),
                            reason: Some("CORRECTION_HARMS_WORKER".to_string()),
                            details: None,
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

/// Validates correction form fields using current UTC time (RF-ADMIN-15-5).
pub fn validate_correction_input(form: &ProposeCorrectionInput) -> Result<(), CommandError> {
    validate_correction_input_at(form, Utc::now())
}

// ============================================================================
// Core Functions (Tested with Wiremock)
// ============================================================================

/// `GET /attendance/board` (RF-ADMIN-15-2, RF-ADMIN-15-3, AS1, BILL-04).
pub async fn attendance_board_with_client(
    client: &AdminApiClient,
) -> Result<Vec<BoardWorker>, CommandError> {
    let mut workers = client
        .get_json::<Vec<BoardWorker>>("/attendance/board")
        .await?;
    for worker in &mut workers {
        if let Some(lm) = &mut worker.last_mark {
            verify_mark(lm);
        }
        for mark in &mut worker.today_marks {
            verify_mark(mark);
        }
    }
    Ok(workers)
}

/// `GET /attendance/days?worker_id=...&from=...&to=...` (RF-ADMIN-15-2, RF-ADMIN-15-4, AS2, BILL-04).
pub async fn attendance_days_with_client(
    client: &AdminApiClient,
    worker_id: &str,
    from: &str,
    to: &str,
) -> Result<Vec<DaySummary>, CommandError> {
    let wid = worker_id.trim();
    if !is_canonical_uuid(wid) {
        return Err(CommandError::validation(
            "El ID del trabajador debe ser un UUID canónico.",
        ));
    }
    let (from_date, to_date) = validate_days_range(from, to)?;
    let endpoint = format!(
        "/attendance/days?worker_id={}&from={}&to={}",
        wid,
        from_date.format("%Y-%m-%d"),
        to_date.format("%Y-%m-%d")
    );

    let mut days = client.get_json::<Vec<DaySummary>>(&endpoint).await?;
    for day in &mut days {
        for mark in &mut day.marks {
            verify_mark(mark);
        }
    }
    Ok(days)
}

/// `GET /attendance/marks?...` (RF-ADMIN-15-2, BILL-04).
pub async fn attendance_marks_with_client(
    client: &AdminApiClient,
    query: AttendanceMarksQuery,
) -> Result<CollectionResponse<AttendanceMark>, CommandError> {
    let mut params = Vec::new();
    if let Some(wid) = &query.worker_id {
        let trimmed = wid.trim();
        if !trimmed.is_empty() {
            if !is_canonical_uuid(trimmed) {
                return Err(CommandError::validation(
                    "worker_id debe ser un UUID canónico.",
                ));
            }
            params.push(format!("worker_id={}", trimmed));
        }
    }
    if let Some(from) = &query.from {
        params.push(format!("from={}", from.trim()));
    }
    if let Some(to) = &query.to {
        params.push(format!("to={}", to.trim()));
    }
    if let Some(page) = query.page {
        params.push(format!("page={}", page));
    }
    if let Some(per_page) = query.per_page {
        params.push(format!("per_page={}", per_page));
    }

    let endpoint = if params.is_empty() {
        "/attendance/marks".to_string()
    } else {
        format!("/attendance/marks?{}", params.join("&"))
    };

    let mut resp = client.get_collection::<AttendanceMark>(&endpoint).await?;
    for mark in &mut resp.data {
        verify_mark(mark);
    }
    Ok(resp)
}

/// `POST /attendance/corrections` (RF-ADMIN-15-2, RF-ADMIN-15-5).
pub async fn attendance_propose_correction_with_client(
    client: &AdminApiClient,
    form: ProposeCorrectionInput,
) -> Result<Vec<Correction>, CommandError> {
    validate_correction_input(&form)?;

    let body = serde_json::json!({
        "worker_ids": form.worker_ids,
        "kind": form.kind,
        "original_mark_id": form.original_mark_id,
        "type": form.r#type,
        "proposed_ts": form.proposed_ts.trim(),
        "reason": form.reason.trim(),
    });

    client
        .post_json::<_, Vec<Correction>>("/attendance/corrections", &body)
        .await
        .map_err(Into::into)
}

/// `GET /attendance/corrections?...` (RF-ADMIN-15-2, RF-ADMIN-15-6, BILL-04).
pub async fn attendance_corrections_with_client(
    client: &AdminApiClient,
    query: AttendanceCorrectionsQuery,
) -> Result<CollectionResponse<Correction>, CommandError> {
    let mut params = Vec::new();
    if let Some(wid) = &query.worker_id {
        let trimmed = wid.trim();
        if !trimmed.is_empty() {
            if !is_canonical_uuid(trimmed) {
                return Err(CommandError::validation(
                    "worker_id debe ser un UUID canónico.",
                ));
            }
            params.push(format!("worker_id={}", trimmed));
        }
    }
    if let Some(status) = query.status {
        let status_str = match status {
            CorrectionStatus::Pending => "PENDING",
            CorrectionStatus::Accepted => "ACCEPTED",
            CorrectionStatus::Opposed => "OPPOSED",
            CorrectionStatus::Consolidated => "CONSOLIDATED",
            CorrectionStatus::Withdrawn => "WITHDRAWN",
        };
        params.push(format!("status={}", status_str));
    }
    if let Some(page) = query.page {
        params.push(format!("page={}", page));
    }
    if let Some(per_page) = query.per_page {
        params.push(format!("per_page={}", per_page));
    }

    let endpoint = if params.is_empty() {
        "/attendance/corrections".to_string()
    } else {
        format!("/attendance/corrections?{}", params.join("&"))
    };

    client
        .get_collection::<Correction>(&endpoint)
        .await
        .map_err(Into::into)
}

/// `DELETE /attendance/corrections/:id` (RF-ADMIN-15-2, RF-ADMIN-15-6).
pub async fn attendance_withdraw_correction_with_client(
    client: &AdminApiClient,
    id: &str,
) -> Result<(), CommandError> {
    let trimmed = id.trim();
    if !is_canonical_uuid(trimmed) {
        return Err(CommandError::validation(
            "El ID de la corrección debe ser un UUID canónico.",
        ));
    }
    client
        .delete(&format!("/attendance/corrections/{}", trimmed))
        .await
        .map_err(Into::into)
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn attendance_board() -> Result<Vec<BoardWorker>, CommandError> {
    attendance_board_with_client(get_api_client()).await
}

#[tauri::command]
pub async fn attendance_days(
    worker_id: String,
    from: String,
    to: String,
) -> Result<Vec<DaySummary>, CommandError> {
    attendance_days_with_client(get_api_client(), &worker_id, &from, &to).await
}

#[tauri::command]
pub async fn attendance_marks(
    query: AttendanceMarksQuery,
) -> Result<CollectionResponse<AttendanceMark>, CommandError> {
    attendance_marks_with_client(get_api_client(), query).await
}

#[tauri::command]
pub async fn attendance_propose_correction(
    form: ProposeCorrectionInput,
) -> Result<Vec<Correction>, CommandError> {
    attendance_propose_correction_with_client(get_api_client(), form).await
}

#[tauri::command]
pub async fn attendance_corrections(
    query: AttendanceCorrectionsQuery,
) -> Result<CollectionResponse<Correction>, CommandError> {
    attendance_corrections_with_client(get_api_client(), query).await
}

#[tauri::command]
pub async fn attendance_withdraw_correction(id: String) -> Result<(), CommandError> {
    attendance_withdraw_correction_with_client(get_api_client(), &id).await
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_test_client(server: &MockServer) -> AdminApiClient {
        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("test-admin-token".to_string()));
        client
    }

    #[test]
    fn test_validate_days_range_accepts_valid_range() {
        let res = validate_days_range("2026-10-01", "2026-10-15");
        assert!(res.is_ok());
        let (f, t) = res.unwrap();
        assert_eq!(f, NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());
        assert_eq!(t, NaiveDate::from_ymd_opt(2026, 10, 15).unwrap());
    }

    #[test]
    fn test_validate_days_range_rejects_inverted_dates() {
        let res = validate_days_range("2026-10-20", "2026-10-10");
        assert!(res.is_err());
        assert!(res.unwrap_err().message.contains("no puede ser posterior"));
    }

    #[test]
    fn test_validate_days_range_rejects_exceeding_62_days() {
        let res = validate_days_range("2026-08-01", "2026-10-15");
        assert!(res.is_err());
        assert!(res.unwrap_err().message.contains("62 días"));
    }

    #[test]
    fn test_validate_correction_input_confirm_no_harm_required() {
        let input = ProposeCorrectionInput {
            worker_ids: vec!["01927d3b-65e1-7000-8000-0000000000aa".to_string()],
            kind: CorrectionKind::Add,
            original_mark_id: None,
            r#type: MarkType::WorkdayStart,
            proposed_ts: "2026-10-01T08:00:00Z".to_string(),
            reason: "Olvidó marcar entrada".to_string(),
            confirm_no_harm: false,
            original_mark_ts: None,
            timezone: None,
        };
        let err = validate_correction_input(&input).unwrap_err();
        assert!(err.message.contains("no perjudica al trabajador"));
    }

    #[test]
    fn test_validate_correction_input_modify_requires_original_mark() {
        let input = ProposeCorrectionInput {
            worker_ids: vec!["01927d3b-65e1-7000-8000-0000000000aa".to_string()],
            kind: CorrectionKind::Modify,
            original_mark_id: None,
            r#type: MarkType::WorkdayStart,
            proposed_ts: "2026-10-01T08:00:00Z".to_string(),
            reason: "Elimina atraso".to_string(),
            confirm_no_harm: true,
            original_mark_ts: None,
            timezone: None,
        };
        let err = validate_correction_input(&input).unwrap_err();
        assert_eq!(err.reason.as_deref(), Some("ORIGINAL_MARK_REQUIRED"));
    }

    #[test]
    fn test_validate_correction_input_rejects_too_early() {
        // Today or future date in Chile time is too early (must be from next business day)
        let chile_today = Utc::now()
            .with_timezone(&chrono_tz::America::Santiago)
            .date_naive();
        let today_ts = format!("{}T08:00:00-03:00", chile_today);
        let input = ProposeCorrectionInput {
            worker_ids: vec!["01927d3b-65e1-7000-8000-0000000000aa".to_string()],
            kind: CorrectionKind::Add,
            original_mark_id: None,
            r#type: MarkType::WorkdayStart,
            proposed_ts: today_ts,
            reason: "Olvidó marcar entrada".to_string(),
            confirm_no_harm: true,
            original_mark_ts: None,
            timezone: Some("America/Santiago".to_string()),
        };
        let err = validate_correction_input(&input).unwrap_err();
        assert_eq!(err.reason.as_deref(), Some("CORRECTION_TOO_EARLY"));
    }

    #[test]
    fn test_validate_correction_input_timezone_chile_late_night() {
        // At 22:30 Chile time on 2026-10-11, UTC time is 2026-10-12 01:30:00Z (next day in UTC).
        // In Chile it is still 2026-10-11, so a same-day correction for 2026-10-11 must be REJECTED.
        let fixed_now_late_night: DateTime<Utc> =
            DateTime::parse_from_rfc3339("2026-10-12T01:30:00Z")
                .unwrap()
                .with_timezone(&Utc);

        let input_same_day = ProposeCorrectionInput {
            worker_ids: vec!["01927d3b-65e1-7000-8000-0000000000aa".to_string()],
            kind: CorrectionKind::Add,
            original_mark_id: None,
            r#type: MarkType::WorkdayStart,
            proposed_ts: "2026-10-11T22:30:00-03:00".to_string(),
            reason: "Marca olvidada por corte de energía".to_string(),
            confirm_no_harm: true,
            original_mark_ts: None,
            timezone: Some("America/Santiago".to_string()),
        };

        // Same day: must be rejected with CORRECTION_TOO_EARLY
        let err = validate_correction_input_at(&input_same_day, fixed_now_late_night).unwrap_err();
        assert_eq!(err.reason.as_deref(), Some("CORRECTION_TOO_EARLY"));

        // Next day in Chile (e.g. 2026-10-12 09:00 Chile time = 12:00:00Z UTC):
        // The correction for 2026-10-11 must be ACCEPTED.
        let fixed_now_next_day: DateTime<Utc> =
            DateTime::parse_from_rfc3339("2026-10-12T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc);

        let ok_res = validate_correction_input_at(&input_same_day, fixed_now_next_day);
        assert!(ok_res.is_ok());
    }

    #[test]
    fn test_validate_correction_input_timezone_harm_ordering() {
        let fixed_now: DateTime<Utc> = DateTime::parse_from_rfc3339("2026-10-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        // Modifying workday start later (harmful)
        let harmful_start = ProposeCorrectionInput {
            worker_ids: vec!["01927d3b-65e1-7000-8000-0000000000aa".to_string()],
            kind: CorrectionKind::Modify,
            original_mark_id: Some("01927d3b-65e1-7000-8000-000000000001".to_string()),
            r#type: MarkType::WorkdayStart,
            proposed_ts: "2026-10-11T09:15:00-03:00".to_string(),
            reason: "Intento de atraso".to_string(),
            confirm_no_harm: true,
            original_mark_ts: Some("2026-10-11T09:00:00-03:00".to_string()),
            timezone: Some("America/Santiago".to_string()),
        };
        let err = validate_correction_input_at(&harmful_start, fixed_now).unwrap_err();
        assert_eq!(err.reason.as_deref(), Some("CORRECTION_HARMS_WORKER"));

        // Modifying workday start earlier (valid)
        let valid_start = ProposeCorrectionInput {
            worker_ids: vec!["01927d3b-65e1-7000-8000-0000000000aa".to_string()],
            kind: CorrectionKind::Modify,
            original_mark_id: Some("01927d3b-65e1-7000-8000-000000000001".to_string()),
            r#type: MarkType::WorkdayStart,
            proposed_ts: "2026-10-11T08:45:00-03:00".to_string(),
            reason: "Elimina atraso justificado".to_string(),
            confirm_no_harm: true,
            original_mark_ts: Some("2026-10-11T09:00:00-03:00".to_string()),
            timezone: Some("America/Santiago".to_string()),
        };
        assert!(validate_correction_input_at(&valid_start, fixed_now).is_ok());

        // Modifying workday end earlier (harmful)
        let harmful_end = ProposeCorrectionInput {
            worker_ids: vec!["01927d3b-65e1-7000-8000-0000000000aa".to_string()],
            kind: CorrectionKind::Modify,
            original_mark_id: Some("01927d3b-65e1-7000-8000-000000000002".to_string()),
            r#type: MarkType::WorkdayEnd,
            proposed_ts: "2026-10-11T17:30:00-03:00".to_string(),
            reason: "Salida antes".to_string(),
            confirm_no_harm: true,
            original_mark_ts: Some("2026-10-11T18:00:00-03:00".to_string()),
            timezone: Some("America/Santiago".to_string()),
        };
        let err_end = validate_correction_input_at(&harmful_end, fixed_now).unwrap_err();
        assert_eq!(err_end.reason.as_deref(), Some("CORRECTION_HARMS_WORKER"));

        // Modifying workday end later (valid)
        let valid_end = ProposeCorrectionInput {
            worker_ids: vec!["01927d3b-65e1-7000-8000-0000000000aa".to_string()],
            kind: CorrectionKind::Modify,
            original_mark_id: Some("01927d3b-65e1-7000-8000-000000000002".to_string()),
            r#type: MarkType::WorkdayEnd,
            proposed_ts: "2026-10-11T19:00:00-03:00".to_string(),
            reason: "Reconocimiento de sobretiempo".to_string(),
            confirm_no_harm: true,
            original_mark_ts: Some("2026-10-11T18:00:00-03:00".to_string()),
            timezone: Some("America/Santiago".to_string()),
        };
        assert!(validate_correction_input_at(&valid_end, fixed_now).is_ok());
    }

    #[test]
    fn test_validate_correction_input_rejects_harm_to_worker() {
        // Delaying workday start harms the worker!
        let input = ProposeCorrectionInput {
            worker_ids: vec!["01927d3b-65e1-7000-8000-0000000000aa".to_string()],
            kind: CorrectionKind::Modify,
            original_mark_id: Some("01927d3b-65e1-7000-8000-000000000001".to_string()),
            r#type: MarkType::WorkdayStart,
            proposed_ts: "2026-10-01T09:30:00Z".to_string(),
            reason: "Intenta registrar llegada más tarde".to_string(),
            confirm_no_harm: true,
            original_mark_ts: Some("2026-10-01T09:00:00Z".to_string()),
            timezone: None,
        };
        let err = validate_correction_input(&input).unwrap_err();
        assert_eq!(err.reason.as_deref(), Some("CORRECTION_HARMS_WORKER"));
    }

    #[test]
    fn test_verify_mark_hash_with_crypto_core_vector() {
        // Vector 1 from CRYPTO-04
        let mut mark = AttendanceMark {
            id: "01927d3b-65e1-7000-8000-000000000001".to_string(),
            mark_id: Some("01927d3b-65e1-7000-8000-000000000001".to_string()),
            client_mark_id: None,
            worker_id: "01927d3b-65e1-7000-8000-0000000000aa".to_string(),
            worker_run: Some("12.345.678-5".to_string()),
            employer_rut: Some("76.123.456-0".to_string()),
            r#type: MarkType::WorkdayStart,
            server_ts: "2026-10-11T08:59:58Z".to_string(),
            device_ts: "2026-10-11T08:59:57Z".to_string(),
            offline: false,
            source: MarkSource::Agent,
            identification: Some("DEVICE_PIN".to_string()),
            workplace_id: Some("01927d3b-65e1-7000-8000-0000000000b1".to_string()),
            workplace: None,
            location: None,
            prev_hash: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            hash: "57c4cad20df41e012451c2841647daef7691fd333b1aaef5afb7fdad7666784e".to_string(),
            corrected_by: None,
            superseded_by: None,
            hash_verified: None,
        };

        verify_mark(&mut mark);
        assert_eq!(mark.hash_verified, Some(true));

        // Tamper with hash
        mark.hash = "0000000000000000000000000000000000000000000000000000000000000000".to_string();
        verify_mark(&mut mark);
        assert_eq!(mark.hash_verified, Some(false));
    }

    #[tokio::test]
    async fn test_attendance_board_wiremock_success() {
        let server = MockServer::start().await;
        let client = make_test_client(&server);

        Mock::given(method("GET"))
            .and(path("/attendance/board"))
            .and(header("Authorization", "Bearer test-admin-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "worker_id": "01927d3b-65e1-7000-8000-0000000000aa",
                        "name": "Juan Pérez",
                        "run": "12.345.678-5",
                        "workplace": { "id": "01927d3b-65e1-7000-8000-0000000000b1", "name": "Casa matriz" },
                        "work_mode": "ON_SITE",
                        "state": "WORKING",
                        "last_mark": {
                            "id": "01927d3b-65e1-7000-8000-000000000001",
                            "worker_id": "01927d3b-65e1-7000-8000-0000000000aa",
                            "worker_run": "12.345.678-5",
                            "employer_rut": "76.123.456-0",
                            "type": "WORKDAY_START",
                            "server_ts": "2026-10-11T08:59:58Z",
                            "device_ts": "2026-10-11T08:59:57Z",
                            "offline": false,
                            "source": "AGENT",
                            "workplace_id": "01927d3b-65e1-7000-8000-0000000000b1",
                            "prev_hash": "0000000000000000000000000000000000000000000000000000000000000000",
                            "hash": "57c4cad20df41e012451c2841647daef7691fd333b1aaef5afb7fdad7666784e"
                        },
                        "today_marks": [],
                        "alerts": []
                    }
                ]
            })))
            .mount(&server)
            .await;

        let res = attendance_board_with_client(&client).await;
        assert!(res.is_ok());
        let board = res.unwrap();
        assert_eq!(board.len(), 1);
        assert_eq!(board[0].name, "Juan Pérez");
        assert_eq!(board[0].state, AttendanceState::Working);
        assert_eq!(
            board[0].last_mark.as_ref().and_then(|m| m.hash_verified),
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_attendance_days_wiremock_and_range_validation() {
        let server = MockServer::start().await;
        let client = make_test_client(&server);

        Mock::given(method("GET"))
            .and(path("/attendance/days"))
            .and(query_param("worker_id", "01927d3b-65e1-7000-8000-0000000000aa"))
            .and(query_param("from", "2026-10-01"))
            .and(query_param("to", "2026-10-10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "date": "2026-10-09",
                        "agreed": { "start": "09:00:00", "end": "18:00:00", "break": "13:00:00-14:00:00", "break_counts_as_work": false },
                        "marks": [],
                        "attended": true,
                        "absence": null,
                        "missing": "-00:10:00",
                        "extra": "+00:00:00",
                        "anomalies": [],
                        "altered": false
                    }
                ]
            })))
            .mount(&server)
            .await;

        let res = attendance_days_with_client(
            &client,
            "01927d3b-65e1-7000-8000-0000000000aa",
            "2026-10-01",
            "2026-10-10",
        )
        .await;
        assert!(res.is_ok());
        let days = res.unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].date, "2026-10-09");
        assert_eq!(days[0].missing, "-00:10:00");

        // Reject range > 62 days locally without network call
        let bad_res = attendance_days_with_client(
            &client,
            "01927d3b-65e1-7000-8000-0000000000aa",
            "2026-07-01",
            "2026-10-10",
        )
        .await;
        assert!(bad_res.is_err());
        assert!(bad_res.unwrap_err().message.contains("62 días"));
    }

    #[tokio::test]
    async fn test_attendance_propose_and_withdraw_correction_wiremock() {
        let server = MockServer::start().await;
        let client = make_test_client(&server);

        Mock::given(method("POST"))
            .and(path("/attendance/corrections"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "01927d3b-65e1-7000-8000-0000000000c1",
                        "worker_id": "01927d3b-65e1-7000-8000-0000000000aa",
                        "kind": "ADD",
                        "type": "WORKDAY_START",
                        "proposed_ts": "2026-10-05T08:30:00Z",
                        "reason": "Marca olvidada por corte de luz",
                        "status": "PENDING",
                        "created_at": "2026-10-11T12:00:00Z",
                        "respond_before": "2026-10-13T12:00:00Z"
                    }
                ]
            })))
            .mount(&server)
            .await;

        let input = ProposeCorrectionInput {
            worker_ids: vec!["01927d3b-65e1-7000-8000-0000000000aa".to_string()],
            kind: CorrectionKind::Add,
            original_mark_id: None,
            r#type: MarkType::WorkdayStart,
            proposed_ts: "2026-10-05T08:30:00Z".to_string(),
            reason: "Marca olvidada por corte de luz".to_string(),
            confirm_no_harm: true,
            original_mark_ts: None,
            timezone: None,
        };

        let proposed = attendance_propose_correction_with_client(&client, input).await;
        assert!(proposed.is_ok());
        let list = proposed.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].status, CorrectionStatus::Pending);

        Mock::given(method("DELETE"))
            .and(path(
                "/attendance/corrections/01927d3b-65e1-7000-8000-0000000000c1",
            ))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let withdraw_res = attendance_withdraw_correction_with_client(
            &client,
            "01927d3b-65e1-7000-8000-0000000000c1",
        )
        .await;
        assert!(withdraw_res.is_ok());
    }
}
