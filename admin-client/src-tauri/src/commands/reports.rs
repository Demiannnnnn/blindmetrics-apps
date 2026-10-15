//! Legal attendance reports (R38 arts. 25, 27, 28) — TASK-ADMIN-16.
//!
//! The **server** computes every report and generates the XLSX/PDF/DOCX exports
//! (`api-contract.md` §9.4). This module only:
//! * validates the art. 25 filters before calling the API (report enum, period XOR range,
//!   range ≤ 5 years or ≤ 24 months for Sundays/holidays, UUIDs, 64-hex hash);
//! * returns the report JSON untouched (ATT-09, R38 art. 28 a: no reordering, rounding or
//!   recomputation — cells keep the server's formatted strings and `altered` flags);
//! * downloads the server file to the Downloads folder without overwriting (` (2)` suffix),
//!   `0600` on Unix, at most [`MAX_EXPORT_BYTES`].
//!
//! Every call is a `GET /attendance/*`, reachable while the subscription is blocked (BILL-04).

use crate::client::{get_api_client, AdminApiClient};
use crate::commands::workers::JourneyType;
use crate::commands::CommandError;
use chrono::{Months, NaiveDate};
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use reqwest::Method;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use telemetry_crypto_core::attendance::is_canonical_uuid;

/// Largest export accepted from the server (RF-ADMIN-16-1).
pub const MAX_EXPORT_BYTES: usize = 50 * 1024 * 1024;
/// Range limit of every report (R38 art. 25: "rango hasta 5 años").
pub const MAX_RANGE_MONTHS: u32 = 60;
/// Range limit of the Sundays/holidays report (R38 art. 27.3).
pub const MAX_RANGE_MONTHS_SUNDAYS: u32 = 24;
/// More selected workers than this must use "all workers" (empty selection) instead.
pub const MAX_SELECTED_WORKERS: usize = 200;
/// Longest free-text filter (`search`, `job_title`).
pub const MAX_TEXT_FILTER_CHARS: usize = 100;
/// Server-side generation of long ranges may exceed the default 15 s request timeout.
const REPORT_TIMEOUT: Duration = Duration::from_secs(60);
const EXPORT_TIMEOUT: Duration = Duration::from_secs(180);
/// Longest file name written to disk (characters, extension included).
const MAX_FILE_NAME_CHARS: usize = 150;

const QUERY_VALUE_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

// ============================================================================
// Enums
// ============================================================================

/// The five reports the server computes (R38 art. 27). The sixth, *Reporte diario*, is an email
/// the server sends every day and has no endpoint.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ReportKind {
    Attendance,
    DailyHours,
    SundaysHolidays,
    ShiftChanges,
    Incidents,
}

impl ReportKind {
    /// Path segment of `/attendance/reports/{report}`.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Attendance => "attendance",
            Self::DailyHours => "daily-hours",
            Self::SundaysHolidays => "sundays-holidays",
            Self::ShiftChanges => "shift-changes",
            Self::Incidents => "incidents",
        }
    }

    /// Maximum range in months for this report (R38 arts. 25, 27.3).
    pub fn max_range_months(self) -> u32 {
        match self {
            Self::SundaysHolidays => MAX_RANGE_MONTHS_SUNDAYS,
            _ => MAX_RANGE_MONTHS,
        }
    }
}

/// Predefined periods of R38 art. 25.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportPeriod {
    LastWeek,
    LastFortnight,
    LastMonth,
}

impl ReportPeriod {
    fn as_str(self) -> &'static str {
        match self {
            Self::LastWeek => "last_week",
            Self::LastFortnight => "last_fortnight",
            Self::LastMonth => "last_month",
        }
    }
}

/// Export formats of R38 art. 28 (Excel, PDF, Word).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Xlsx,
    Pdf,
    Docx,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Xlsx => "xlsx",
            Self::Pdf => "pdf",
            Self::Docx => "docx",
        }
    }

    /// True when `bytes` start like a file of this format (PDF header or ZIP container for
    /// OOXML), so an error page is never saved as a report.
    pub fn matches_content(self, bytes: &[u8]) -> bool {
        match self {
            Self::Pdf => bytes.starts_with(b"%PDF-"),
            Self::Xlsx | Self::Docx => bytes.starts_with(b"PK\x03\x04"),
        }
    }
}

fn journey_type_str(value: JourneyType) -> &'static str {
    match value {
        JourneyType::Fixed => "FIXED",
        JourneyType::Shifts => "SHIFTS",
        JourneyType::Cycles => "CYCLES",
        JourneyType::Biweekly => "BIWEEKLY",
        JourneyType::Exceptional => "EXCEPTIONAL",
        JourneyType::PartTime => "PART_TIME",
    }
}

// ============================================================================
// Filters (R38 art. 25)
// ============================================================================

/// Filters of R38 art. 25, combinable in any order (RF-ADMIN-16-3). Exactly one of `period` or
/// `from`/`to` (dates `YYYY-MM-DD`) is required.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct ReportFilters {
    #[serde(default)]
    pub worker_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<ReportPeriod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journey_type: Option<JourneyType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workplace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn parse_filter_date(value: &str, label: &str) -> Result<NaiveDate, CommandError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        CommandError::validation(format!(
            "La fecha {label} debe tener el formato AAAA-MM-DD."
        ))
    })
}

fn checked_uuid(value: &str, label: &str) -> Result<String, CommandError> {
    let lower = value.to_ascii_lowercase();
    if is_canonical_uuid(&lower) {
        Ok(lower)
    } else {
        Err(CommandError::validation(format!(
            "El identificador de {label} no es válido."
        )))
    }
}

fn checked_text(value: &str, label: &str) -> Result<String, CommandError> {
    if value.chars().count() > MAX_TEXT_FILTER_CHARS {
        return Err(CommandError::validation(format!(
            "El filtro {label} admite hasta {MAX_TEXT_FILTER_CHARS} caracteres."
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(CommandError::validation(format!(
            "El filtro {label} contiene caracteres no permitidos."
        )));
    }
    Ok(value.to_string())
}

/// Validates `filters` for `report` and returns the query parameters in a stable order
/// (RF-ADMIN-16-1). Never calls the network.
pub fn validate_report_filters(
    report: ReportKind,
    filters: &ReportFilters,
) -> Result<Vec<(&'static str, String)>, CommandError> {
    let mut params: Vec<(&'static str, String)> = Vec::new();

    // Workers: deduplicated, order kept (the server sorts sections alphabetically).
    let mut ids: Vec<String> = Vec::new();
    for raw in &filters.worker_ids {
        let id = checked_uuid(raw.trim(), "trabajador")?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.len() > MAX_SELECTED_WORKERS {
        return Err(CommandError::validation(format!(
            "Selecciona hasta {MAX_SELECTED_WORKERS} trabajadores o deja la selección vacía para incluir a todos."
        )));
    }
    if !ids.is_empty() {
        params.push(("worker_ids", ids.join(",")));
    }
    if let Some(search) = non_empty(&filters.search) {
        params.push(("search", checked_text(search, "de búsqueda")?));
    }

    // Period XOR range.
    let from = non_empty(&filters.from);
    let to = non_empty(&filters.to);
    match (filters.period, from, to) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
            return Err(CommandError::validation(
                "Elige un período predeterminado o un rango de fechas, no ambos.",
            ));
        }
        (Some(period), None, None) => params.push(("period", period.as_str().to_string())),
        (None, Some(from), Some(to)) => {
            let from_date = parse_filter_date(from, "de inicio")?;
            let to_date = parse_filter_date(to, "de término")?;
            if from_date > to_date {
                return Err(CommandError::validation(
                    "La fecha de inicio no puede ser posterior a la de término.",
                ));
            }
            let months = report.max_range_months();
            let earliest = to_date
                .checked_sub_months(Months::new(months))
                .unwrap_or(NaiveDate::MIN);
            if from_date < earliest {
                let limit = if months == MAX_RANGE_MONTHS_SUNDAYS {
                    "24 meses".to_string()
                } else {
                    format!("{} años", months / 12)
                };
                return Err(CommandError::validation(format!(
                    "El rango de este reporte no puede superar {limit} (R38 art. 25)."
                )));
            }
            params.push(("from", from_date.format("%Y-%m-%d").to_string()));
            params.push(("to", to_date.format("%Y-%m-%d").to_string()));
        }
        (None, _, _) => {
            return Err(CommandError::validation(
                "Elige un período predeterminado o completa las dos fechas del rango.",
            ));
        }
    }

    if let Some(journey) = filters.journey_type {
        params.push(("journey_type", journey_type_str(journey).to_string()));
    }
    if let Some(id) = non_empty(&filters.schedule_id) {
        params.push(("schedule_id", checked_uuid(id, "la jornada")?));
    }
    if let Some(id) = non_empty(&filters.workplace_id) {
        params.push(("workplace_id", checked_uuid(id, "lugar de trabajo")?));
    }
    if let Some(title) = non_empty(&filters.job_title) {
        params.push(("job_title", checked_text(title, "de cargo")?));
    }
    if let Some(hash) = non_empty(&filters.hash) {
        let hash = hash.to_ascii_lowercase();
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CommandError::validation(
                "El hash debe tener 64 caracteres hexadecimales.",
            ));
        }
        params.push(("hash", hash));
    }
    Ok(params)
}

fn encode_query(params: &[(&'static str, String)]) -> String {
    params
        .iter()
        .map(|(key, value)| {
            // Commas separate `worker_ids`; every other byte outside the unreserved set is encoded.
            let encoded = value
                .split(',')
                .map(|part| utf8_percent_encode(part, QUERY_VALUE_ENCODE_SET).to_string())
                .collect::<Vec<_>>()
                .join(",");
            format!("{key}={encoded}")
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// `/attendance/reports/{report}?…` with validated filters.
pub fn report_endpoint(
    report: ReportKind,
    filters: &ReportFilters,
) -> Result<String, CommandError> {
    let params = validate_report_filters(report, filters)?;
    Ok(format!(
        "/attendance/reports/{}?{}",
        report.slug(),
        encode_query(&params)
    ))
}

/// `/attendance/reports/{report}/export?format=…&…` with validated filters.
pub fn export_endpoint(
    report: ReportKind,
    filters: &ReportFilters,
    format: ExportFormat,
) -> Result<String, CommandError> {
    let mut params = vec![("format", format.extension().to_string())];
    params.extend(validate_report_filters(report, filters)?);
    Ok(format!(
        "/attendance/reports/{}/export?{}",
        report.slug(),
        encode_query(&params)
    ))
}

// ============================================================================
// Report JSON (api-contract §9.4) — passed through untouched
// ============================================================================

/// Employer header of every report.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ReportEmployer {
    pub legal_name: String,
    pub rut: String,
}

/// Workplace of a section header: the server may send the name or `{ name, address }`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum ReportWorkplace {
    Name(String),
    Detail {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        address: Option<String>,
    },
}

/// Worker header of a section (R38 art. 27: nombre, RUN, lugar, cargo).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ReportWorker {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workplace: Option<ReportWorkplace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_title: Option<String>,
}

/// One cell, already formatted by the server (`dd/mm/aa`, `hh:mm:ss`, `+`/`-`). Altered values
/// arrive as `{ "v": "08:02:00", "altered": true }` (ATT-05).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ReportCell {
    Flagged {
        v: String,
        #[serde(default)]
        altered: bool,
    },
    Text(String),
    Number(serde_json::Number),
    Empty,
}

/// One table of the report (one per worker; the incidents report may have no worker).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ReportSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker: Option<ReportWorker>,
    #[serde(default)]
    pub header_notes: Vec<String>,
    pub columns: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<ReportCell>>,
    #[serde(default)]
    pub totals: Vec<Vec<ReportCell>>,
}

/// One entry of the abbreviations legend (R38 art. 28), in the server's order.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Abbreviation {
    pub code: String,
    pub meaning: String,
}

/// Reads the `abbreviations` object keeping the server's key order (a map type would sort it).
fn deserialize_abbreviations<'de, D>(deserializer: D) -> Result<Vec<Abbreviation>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OrderedMap;

    impl<'de> Visitor<'de> for OrderedMap {
        type Value = Vec<Abbreviation>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an object of abbreviation → meaning")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut out = Vec::new();
            while let Some((code, meaning)) = map.next_entry::<String, String>()? {
                out.push(Abbreviation { code, meaning });
            }
            Ok(out)
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }
    }

    deserializer.deserialize_any(OrderedMap)
}

/// `GET /attendance/reports/{report}` response (api-contract §9.4).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LegalReport {
    pub report: ReportKind,
    pub generated_at: String,
    pub employer: ReportEmployer,
    #[serde(default)]
    pub sections: Vec<ReportSection>,
    #[serde(default, deserialize_with = "deserialize_abbreviations")]
    pub abbreviations: Vec<Abbreviation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_message: Option<String>,
}

/// `GET /attendance/reports/{report}` (RF-ADMIN-16-1, BILL-04).
pub async fn attendance_report_with_client(
    client: &AdminApiClient,
    report: ReportKind,
    filters: &ReportFilters,
) -> Result<LegalReport, CommandError> {
    let endpoint = report_endpoint(report, filters)?;
    let resp = client
        .send_with_auth_timeout(Method::GET, &endpoint, None, Some(REPORT_TIMEOUT))
        .await?;
    let wrapped: crate::client::SingleResponse<LegalReport> = resp
        .json()
        .await
        .map_err(crate::client::ApiError::deserialization)?;
    Ok(wrapped.data)
}

// ============================================================================
// Export download (RF-ADMIN-16-1, RF-ADMIN-16-5)
// ============================================================================

/// File name from a `Content-Disposition` header (`filename*=UTF-8''…` preferred over
/// `filename=`), before sanitizing.
pub fn content_disposition_file_name(header: &str) -> Option<String> {
    let mut plain: Option<String> = None;
    let mut extended: Option<String> = None;
    for part in header.split(';').skip(1) {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if key == "filename*" {
            if let Some((charset, rest)) = value.split_once('\'') {
                if let Some((_, encoded)) = rest.split_once('\'') {
                    if charset.eq_ignore_ascii_case("utf-8") {
                        if let Ok(decoded) = percent_decode_str(encoded).decode_utf8() {
                            extended = Some(decoded.into_owned());
                        }
                    }
                }
            }
        } else if key == "filename" {
            plain = Some(value.trim_matches('"').to_string());
        }
    }
    extended.or(plain).filter(|s| !s.trim().is_empty())
}

const WINDOWS_RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Safe file name for the Downloads folder: no directories, control or reserved characters,
/// no leading/trailing dots or spaces, bounded length, and always the extension of `format`.
pub fn sanitize_export_file_name(
    raw: Option<&str>,
    report: ReportKind,
    format: ExportFormat,
    today: NaiveDate,
) -> String {
    let ext = format.extension();
    let fallback = format!("reporte-{}_{}", report.slug(), today.format("%Y-%m-%d"));
    let base = raw
        .map(|r| r.rsplit(['/', '\\']).next().unwrap_or(r))
        .unwrap_or("");
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
        .collect();
    let cleaned = cleaned.trim_matches(|c: char| c == '.' || c.is_whitespace());

    let suffix = format!(".{ext}");
    let stem = if cleaned.to_ascii_lowercase().ends_with(&suffix) {
        &cleaned[..cleaned.len() - suffix.len()]
    } else {
        cleaned
    };
    let stem = stem.trim_matches(|c: char| c == '.' || c.is_whitespace());
    let mut stem = if stem.is_empty() {
        fallback
    } else {
        stem.to_string()
    };
    let upper = stem.split('.').next().unwrap_or("").to_ascii_uppercase();
    if WINDOWS_RESERVED.contains(&upper.as_str()) {
        stem = format!("reporte-{stem}");
    }
    let max_stem = MAX_FILE_NAME_CHARS - suffix.chars().count();
    if stem.chars().count() > max_stem {
        stem = stem.chars().take(max_stem).collect();
        stem = stem.trim_end_matches(['.', ' ']).to_string();
    }
    format!("{stem}{suffix}")
}

/// Writes `bytes` as a new file in `dir` without overwriting: `name.ext`, `name (2).ext`, …
/// (`create_new`), permissions `0600` on Unix. A partially written file is removed.
pub fn write_new_file(dir: &Path, file_name: &str, bytes: &[u8]) -> Result<PathBuf, CommandError> {
    use std::io::Write;
    std::fs::create_dir_all(dir).map_err(|e| {
        CommandError::new(
            "FILE_WRITE_ERROR",
            format!("No se pudo crear la carpeta de descargas: {e}"),
        )
    })?;
    let (stem, ext) = match file_name.rsplit_once('.') {
        Some((stem, ext)) => (stem, format!(".{ext}")),
        None => (file_name, String::new()),
    };
    for attempt in 1..=99 {
        let candidate = if attempt == 1 {
            dir.join(file_name)
        } else {
            dir.join(format!("{stem} ({attempt}){ext}"))
        };
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&candidate) {
            Ok(mut file) => {
                if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
                    drop(file);
                    if let Err(rm) = std::fs::remove_file(&candidate) {
                        tracing::warn!("partial report file not removed: {}", rm);
                    }
                    return Err(CommandError::new(
                        "FILE_WRITE_ERROR",
                        format!("No se pudo escribir el reporte: {e}"),
                    ));
                }
                return Ok(candidate);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(CommandError::new(
                    "FILE_WRITE_ERROR",
                    format!("No se pudo crear el archivo del reporte: {e}"),
                ))
            }
        }
    }
    Err(CommandError::new(
        "FILE_WRITE_ERROR",
        "Hay demasiados archivos con el mismo nombre en la carpeta de descargas.",
    ))
}

fn export_too_large() -> CommandError {
    CommandError::new(
        "EXPORT_TOO_LARGE",
        "El archivo del reporte supera los 50 MiB. Acota el período o los trabajadores.",
    )
}

/// Downloads the server export of `report` into `dir` and returns the saved path
/// (RF-ADMIN-16-1). Nothing is written when the server answers an error (e.g. `402`).
pub async fn attendance_export_report_with_client(
    client: &AdminApiClient,
    report: ReportKind,
    filters: &ReportFilters,
    format: ExportFormat,
    dir: &Path,
) -> Result<PathBuf, CommandError> {
    let endpoint = export_endpoint(report, filters, format)?;
    let mut resp = client
        .send_with_auth_timeout(Method::GET, &endpoint, None, Some(EXPORT_TIMEOUT))
        .await?;

    if resp
        .content_length()
        .is_some_and(|len| len > MAX_EXPORT_BYTES as u64)
    {
        return Err(export_too_large());
    }
    let disposition = resp
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(content_disposition_file_name);

    let mut bytes: Vec<u8> = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(crate::client::ApiError::network)?
    {
        if bytes.len() + chunk.len() > MAX_EXPORT_BYTES {
            return Err(export_too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    if !format.matches_content(&bytes) {
        tracing::error!(
            format = format.extension(),
            "report export is not a file of the requested format"
        );
        return Err(CommandError::new(
            "INVALID_EXPORT_FILE",
            "El servidor no entregó un archivo válido del formato pedido.",
        ));
    }

    let today = chrono::Local::now().date_naive();
    let file_name = sanitize_export_file_name(disposition.as_deref(), report, format, today);
    let dir = dir.to_path_buf();
    let path = tokio::task::spawn_blocking(move || write_new_file(&dir, &file_name, &bytes))
        .await
        .map_err(|e| CommandError::new("FILE_WRITE_ERROR", format!("{e}")))??;
    tracing::info!(
        report = report.slug(),
        format = format.extension(),
        "report export saved"
    );
    Ok(path)
}

/// Downloads folder (fallback Documents, then home) — same as the key backup export.
fn downloads_dir() -> PathBuf {
    crate::commands::vault::backup_directory()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Result of *Imprimir*: the saved PDF and whether the system viewer was launched.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PrintedReport {
    pub path: String,
    pub opened: bool,
}

// ============================================================================
// Tauri commands
// ============================================================================

/// RF-ADMIN-16-1: report JSON exactly as the server computes it.
#[tauri::command]
pub async fn attendance_report(
    report: ReportKind,
    filters: ReportFilters,
) -> Result<LegalReport, CommandError> {
    attendance_report_with_client(get_api_client(), report, &filters).await
}

/// RF-ADMIN-16-1: saves the server export to Downloads and returns its path.
#[tauri::command]
pub async fn attendance_export_report(
    report: ReportKind,
    filters: ReportFilters,
    format: ExportFormat,
) -> Result<String, CommandError> {
    let path = attendance_export_report_with_client(
        get_api_client(),
        report,
        &filters,
        format,
        &downloads_dir(),
    )
    .await?;
    Ok(path_string(&path))
}

/// RF-ADMIN-16-5 *Imprimir*: downloads the PDF and opens it with the system viewer. The path
/// comes from Rust, never from the WebView.
#[tauri::command]
pub async fn attendance_print_report(
    report: ReportKind,
    filters: ReportFilters,
) -> Result<PrintedReport, CommandError> {
    let path = attendance_export_report_with_client(
        get_api_client(),
        report,
        &filters,
        ExportFormat::Pdf,
        &downloads_dir(),
    )
    .await?;
    let opened = match open::that_detached(&path) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("PDF viewer could not be started: {}", e);
            false
        }
    };
    Ok(PrintedReport {
        path: path_string(&path),
        opened,
    })
}

/// RF-ADMIN-16-1: opens the Downloads folder with the system file manager.
#[tauri::command]
pub async fn open_downloads_folder() -> Result<(), CommandError> {
    let dir = downloads_dir();
    open::that_detached(&dir).map_err(|e| {
        tracing::warn!("downloads folder could not be opened: {}", e);
        CommandError::new(
            "FOLDER_UNAVAILABLE",
            "No se pudo abrir la carpeta de descargas.",
        )
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const W1: &str = "01927d3b-65e1-7000-8000-0000000000aa";
    const W2: &str = "01927d3b-65e1-7000-8000-0000000000ab";
    const WP: &str = "01927d3b-65e1-7000-8000-0000000000b1";
    const HASH: &str = "57c4cad20df41e012451c2841647daef7691fd333b1aaef5afb7fdad7666784e";

    fn make_test_client(server: &MockServer) -> AdminApiClient {
        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("test-admin-token".to_string()));
        client
    }

    fn last_week() -> ReportFilters {
        ReportFilters {
            period: Some(ReportPeriod::LastWeek),
            ..Default::default()
        }
    }

    fn range(from: &str, to: &str) -> ReportFilters {
        ReportFilters {
            from: Some(from.into()),
            to: Some(to.into()),
            ..Default::default()
        }
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    // ------------------------------------------------------------------ filters

    #[test]
    fn test_reports_kind_and_format_serialization() {
        assert_eq!(
            serde_json::to_value(ReportKind::SundaysHolidays).unwrap(),
            "sundays-holidays"
        );
        let kind: ReportKind = serde_json::from_value(serde_json::json!("daily-hours")).unwrap();
        assert_eq!(kind, ReportKind::DailyHours);
        assert!(serde_json::from_value::<ReportKind>(serde_json::json!("daily")).is_err());
        assert_eq!(serde_json::to_value(ExportFormat::Docx).unwrap(), "docx");
        assert!(serde_json::from_value::<ExportFormat>(serde_json::json!("csv")).is_err());
        let period: ReportPeriod =
            serde_json::from_value(serde_json::json!("last_fortnight")).unwrap();
        assert_eq!(period, ReportPeriod::LastFortnight);
    }

    #[test]
    fn test_reports_filters_all_combined_in_stable_order() {
        let filters = ReportFilters {
            worker_ids: vec![W2.into(), W1.to_uppercase(), W2.into()],
            search: Some("  Pérez ".into()),
            period: None,
            from: Some("2026-09-01".into()),
            to: Some("2026-09-30".into()),
            journey_type: Some(JourneyType::PartTime),
            schedule_id: None,
            workplace_id: Some(WP.into()),
            job_title: Some("Vendedor".into()),
            hash: Some(HASH.to_uppercase()),
        };
        let endpoint = report_endpoint(ReportKind::DailyHours, &filters).unwrap();
        assert_eq!(
            endpoint,
            format!(
                "/attendance/reports/daily-hours?worker_ids={W2},{W1}&search=P%C3%A9rez&from=2026-09-01&to=2026-09-30&journey_type=PART_TIME&workplace_id={WP}&job_title=Vendedor&hash={HASH}"
            )
        );
    }

    #[test]
    fn test_reports_filters_period_xor_range() {
        let err =
            validate_report_filters(ReportKind::Attendance, &ReportFilters::default()).unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert!(err.message.contains("período"));

        let both = ReportFilters {
            period: Some(ReportPeriod::LastMonth),
            from: Some("2026-09-01".into()),
            ..Default::default()
        };
        assert!(validate_report_filters(ReportKind::Attendance, &both)
            .unwrap_err()
            .message
            .contains("no ambos"));

        let half = ReportFilters {
            from: Some("2026-09-01".into()),
            ..Default::default()
        };
        assert!(validate_report_filters(ReportKind::Attendance, &half).is_err());

        let params = validate_report_filters(ReportKind::Incidents, &last_week()).unwrap();
        assert_eq!(params, vec![("period", "last_week".to_string())]);
    }

    #[test]
    fn test_reports_filters_range_limits_per_report() {
        // Exactly 5 years: accepted; one more day: rejected.
        assert!(validate_report_filters(
            ReportKind::Attendance,
            &range("2021-10-15", "2026-10-15")
        )
        .is_ok());
        let err =
            validate_report_filters(ReportKind::Attendance, &range("2021-10-14", "2026-10-15"))
                .unwrap_err();
        assert!(err.message.contains("5 años"));

        // Sundays/holidays: 24 months.
        assert!(validate_report_filters(
            ReportKind::SundaysHolidays,
            &range("2024-10-15", "2026-10-15")
        )
        .is_ok());
        let err = validate_report_filters(
            ReportKind::SundaysHolidays,
            &range("2024-10-14", "2026-10-15"),
        )
        .unwrap_err();
        assert!(err.message.contains("24 meses"));

        // Inverted and malformed dates.
        assert!(validate_report_filters(
            ReportKind::Attendance,
            &range("2026-10-15", "2026-10-01")
        )
        .is_err());
        assert!(validate_report_filters(
            ReportKind::Attendance,
            &range("15/10/2026", "2026-10-20")
        )
        .is_err());
    }

    #[test]
    fn test_reports_filters_reject_bad_ids_hash_and_text() {
        let bad_worker = ReportFilters {
            worker_ids: vec!["not-a-uuid".into()],
            ..last_week()
        };
        assert!(validate_report_filters(ReportKind::Attendance, &bad_worker).is_err());

        let bad_workplace = ReportFilters {
            workplace_id: Some("../etc".into()),
            ..last_week()
        };
        assert!(validate_report_filters(ReportKind::Attendance, &bad_workplace).is_err());

        let bad_schedule = ReportFilters {
            schedule_id: Some("123".into()),
            ..last_week()
        };
        assert!(validate_report_filters(ReportKind::ShiftChanges, &bad_schedule).is_err());

        for hash in ["abc", &"g".repeat(64), &format!("{HASH}0")] {
            let filters = ReportFilters {
                hash: Some(hash.to_string()),
                ..last_week()
            };
            let err = validate_report_filters(ReportKind::Attendance, &filters).unwrap_err();
            assert!(err.message.contains("64"));
        }

        let long = ReportFilters {
            job_title: Some("x".repeat(MAX_TEXT_FILTER_CHARS + 1)),
            ..last_week()
        };
        assert!(validate_report_filters(ReportKind::Attendance, &long).is_err());

        let control = ReportFilters {
            search: Some("Juan\u{0}".into()),
            ..last_week()
        };
        assert!(validate_report_filters(ReportKind::Attendance, &control).is_err());

        let too_many = ReportFilters {
            worker_ids: (0..=MAX_SELECTED_WORKERS)
                .map(|i| format!("01927d3b-65e1-7000-8000-{:012x}", i))
                .collect(),
            ..last_week()
        };
        assert!(validate_report_filters(ReportKind::Attendance, &too_many).is_err());

        // Blank optional filters are ignored.
        let blank = ReportFilters {
            search: Some("   ".into()),
            job_title: Some(String::new()),
            hash: Some(" ".into()),
            ..last_week()
        };
        assert_eq!(
            validate_report_filters(ReportKind::Attendance, &blank).unwrap(),
            vec![("period", "last_week".to_string())]
        );
    }

    #[test]
    fn test_reports_export_endpoint_puts_format_first() {
        let endpoint =
            export_endpoint(ReportKind::Incidents, &last_week(), ExportFormat::Xlsx).unwrap();
        assert_eq!(
            endpoint,
            "/attendance/reports/incidents/export?format=xlsx&period=last_week"
        );
    }

    // ------------------------------------------------------------------ report JSON

    /// Raw JSON text: `serde_json::json!` would sort the `abbreviations` keys.
    fn daily_hours_fixture() -> String {
        r#"{
            "data": {
                "report": "daily-hours",
                "generated_at": "2026-10-12T10:00:00Z",
                "employer": { "legal_name": "Comercial Los Andes SpA", "rut": "76.123.456-0" },
                "sections": [
                    {
                        "worker": { "name": "Juan Pérez", "run": "12.345.678-5", "workplace": "Casa matriz", "job_title": "Vendedor" },
                        "header_notes": ["Jornada ordinaria fija"],
                        "columns": ["Fecha", "Jornada ordinaria pactada", "Marcaciones jornada", "Colación", "Marcaciones colación", "Tiempo faltante", "Tiempo extra", "Otras marcaciones", "Observaciones"],
                        "rows": [
                            ["06/10/26", "09:00:00 - 18:00:00", { "v": "08:02:00", "altered": true }, "13:00:00 - 14:00:00", "13:01:10 - 14:00:05", "-00:00:00", "+00:57:55", "", "Marca corregida"],
                            ["07/10/26", "09:00:00 - 18:00:00", "09:12:30 - 18:00:00", "13:00:00 - 14:00:00", "No aplica", "-00:12:30", "+00:00:00", null, ""]
                        ],
                        "totals": [["Total semanal", "", "", "", "", "-00:12:30", "+00:57:55", "", "+00:45:25"]]
                    }
                ],
                "abbreviations": { "VAC": "Vacaciones", "A.I.": "Ausencia injustificada", "L.M.": "Licencia médica" }
            }
        }"#
        .to_string()
    }

    #[tokio::test]
    async fn test_reports_daily_hours_json_passes_through_untouched() {
        let server = MockServer::start().await;
        let client = make_test_client(&server);
        Mock::given(method("GET"))
            .and(path("/attendance/reports/daily-hours"))
            .and(query_param("period", "last_week"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(daily_hours_fixture(), "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let report = attendance_report_with_client(&client, ReportKind::DailyHours, &last_week())
            .await
            .unwrap();
        assert_eq!(report.report, ReportKind::DailyHours);
        assert_eq!(report.employer.rut, "76.123.456-0");
        let section = &report.sections[0];
        assert_eq!(section.columns.len(), 9);
        assert_eq!(
            section.rows[0][2],
            ReportCell::Flagged {
                v: "08:02:00".into(),
                altered: true
            }
        );
        assert_eq!(section.rows[1][5], ReportCell::Text("-00:12:30".into()));
        assert_eq!(section.rows[1][7], ReportCell::Empty);
        assert_eq!(section.totals[0][8], ReportCell::Text("+00:45:25".into()));
        // Abbreviations keep the server order (not alphabetical).
        let codes: Vec<&str> = report
            .abbreviations
            .iter()
            .map(|a| a.code.as_str())
            .collect();
        assert_eq!(codes, vec!["VAC", "A.I.", "L.M."]);

        // What reaches the WebView keeps the same cell values and shape.
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["sections"][0]["rows"][0][2]["v"], "08:02:00");
        assert_eq!(json["sections"][0]["rows"][0][2]["altered"], true);
        assert_eq!(json["sections"][0]["rows"][1][2], "09:12:30 - 18:00:00");
        assert!(json["sections"][0]["rows"][1][7].is_null());
        assert_eq!(json["sections"][0]["worker"]["workplace"], "Casa matriz");
        assert!(json.get("empty_message").is_none());
    }

    #[tokio::test]
    async fn test_reports_each_report_fixture_including_empty() {
        let server = MockServer::start().await;
        let client = make_test_client(&server);
        let employer =
            serde_json::json!({ "legal_name": "Comercial Los Andes SpA", "rut": "76.123.456-0" });
        let fixtures = [
            (
                ReportKind::Attendance,
                serde_json::json!({
                    "report": "attendance", "generated_at": "2026-10-12T10:00:00Z", "employer": employer,
                    "sections": [{
                        "worker": { "name": "Ana Soto", "run": "9.876.543-3", "workplace": { "name": "Sucursal Centro", "address": "Huérfanos 1234, Santiago" } },
                        "columns": ["Fecha", "Asistencia", "Ausencia", "Observaciones"],
                        "rows": [["05/10/26", "Sí", "", ""], ["06/10/26", "No", "Justificada", "L.M."]]
                    }],
                    "abbreviations": { "L.M.": "Licencia médica" }
                }),
            ),
            (
                ReportKind::SundaysHolidays,
                serde_json::json!({
                    "report": "sundays-holidays", "generated_at": "2026-10-12T10:00:00Z", "employer": employer,
                    "sections": [{
                        "worker": { "name": "Ana Soto", "run": "9.876.543-3" },
                        "header_notes": ["La jornada de este trabajador no incluye domingos o festivos"],
                        "columns": ["Fecha", "Asistencia", "Ausencia", "Observaciones"],
                        "rows": [],
                        "totals": [["Total octubre 2026", "0", "0", ""], ["Total", "0", "0", ""]]
                    }],
                    "abbreviations": {}
                }),
            ),
            (
                ReportKind::ShiftChanges,
                serde_json::json!({
                    "report": "shift-changes", "generated_at": "2026-10-12T10:00:00Z", "employer": employer,
                    "sections": [{
                        "worker": { "name": "Ana Soto", "run": "9.876.543-3", "job_title": "Cajera" },
                        "header_notes": ["Sin cambios o modificaciones en el periodo consultado"],
                        "columns": ["Fecha asignación turno", "Turno", "Extensión", "Fecha asignación nuevo turno", "Inicio de turno", "Nuevo turno", "Extensión", "Solicitado por", "Observaciones"],
                        "rows": []
                    }]
                }),
            ),
            (
                ReportKind::Incidents,
                serde_json::json!({
                    "report": "incidents", "generated_at": "2026-10-12T10:00:00Z", "employer": employer,
                    "sections": [{
                        "columns": ["Inicio", "Término", "Descripción"],
                        "rows": [["01/10/26 10:00:00", "01/10/26 10:45:00", "Caída del servidor de correo"]]
                    }],
                    "abbreviations": null
                }),
            ),
        ];
        for (kind, body) in &fixtures {
            Mock::given(method("GET"))
                .and(path(format!("/attendance/reports/{}", kind.slug())))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": body })),
                )
                .mount(&server)
                .await;
        }
        for (kind, _) in &fixtures {
            let report = attendance_report_with_client(&client, *kind, &last_week())
                .await
                .unwrap();
            assert_eq!(report.report, *kind);
            assert_eq!(report.sections.len(), 1);
        }
        let attendance =
            attendance_report_with_client(&client, ReportKind::Attendance, &last_week())
                .await
                .unwrap();
        assert_eq!(
            attendance.sections[0]
                .worker
                .as_ref()
                .and_then(|w| w.workplace.clone()),
            Some(ReportWorkplace::Detail {
                name: "Sucursal Centro".into(),
                address: Some("Huérfanos 1234, Santiago".into())
            })
        );
        let incidents = attendance_report_with_client(&client, ReportKind::Incidents, &last_week())
            .await
            .unwrap();
        assert!(incidents.sections[0].worker.is_none());
        assert!(incidents.abbreviations.is_empty());

        // Empty selection: message kept verbatim.
        let empty_server = MockServer::start().await;
        let empty_client = make_test_client(&empty_server);
        Mock::given(method("GET"))
            .and(path("/attendance/reports/attendance"))
            .and(query_param("search", "Nadie"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "report": "attendance", "generated_at": "2026-10-12T10:00:00Z", "employer": employer,
                    "sections": [], "abbreviations": {},
                    "empty_message": "No hay trabajadores que coincidan con la selección"
                }
            })))
            .mount(&empty_server)
            .await;
        let filters = ReportFilters {
            search: Some("Nadie".into()),
            ..last_week()
        };
        let empty = attendance_report_with_client(&empty_client, ReportKind::Attendance, &filters)
            .await
            .unwrap();
        assert!(empty.sections.is_empty());
        assert_eq!(
            empty.empty_message.as_deref(),
            Some("No hay trabajadores que coincidan con la selección")
        );
    }

    #[tokio::test]
    async fn test_reports_invalid_filters_never_reach_the_network() {
        let server = MockServer::start().await;
        let client = make_test_client(&server);
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        let err = attendance_report_with_client(
            &client,
            ReportKind::SundaysHolidays,
            &range("2023-01-01", "2026-10-01"),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
        let dir = tempfile::tempdir().unwrap();
        let err = attendance_export_report_with_client(
            &client,
            ReportKind::Attendance,
            &ReportFilters::default(),
            ExportFormat::Pdf,
            dir.path(),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    // ------------------------------------------------------------------ export

    fn pdf_bytes() -> Vec<u8> {
        b"%PDF-1.7\n% reporte\n".to_vec()
    }

    #[test]
    fn test_reports_content_disposition_parsing() {
        assert_eq!(
            content_disposition_file_name(
                "attachment; filename=\"reporte-asistencia_76.123.456-0_2026-10-01_2026-10-31.xlsx\""
            )
            .as_deref(),
            Some("reporte-asistencia_76.123.456-0_2026-10-01_2026-10-31.xlsx")
        );
        assert_eq!(
            content_disposition_file_name(
                "attachment; filename=\"fallback.pdf\"; filename*=UTF-8''reporte-jornada_%C3%B1.pdf"
            )
            .as_deref(),
            Some("reporte-jornada_ñ.pdf")
        );
        assert_eq!(
            content_disposition_file_name("attachment; filename=plain.docx").as_deref(),
            Some("plain.docx")
        );
        assert_eq!(content_disposition_file_name("attachment"), None);
        assert_eq!(
            content_disposition_file_name("attachment; filename=\"\""),
            None
        );
    }

    #[test]
    fn test_reports_file_name_sanitized() {
        let today = date(2026, 10, 15);
        let pdf = ExportFormat::Pdf;
        let kind = ReportKind::DailyHours;
        assert_eq!(
            sanitize_export_file_name(Some("../../etc/passwd"), kind, pdf, today),
            "passwd.pdf"
        );
        assert_eq!(
            sanitize_export_file_name(Some("..\\..\\Windows\\evil.exe"), kind, pdf, today),
            "evil.exe.pdf"
        );
        assert_eq!(
            sanitize_export_file_name(Some(".hidden.pdf"), kind, pdf, today),
            "hidden.pdf"
        );
        assert_eq!(
            sanitize_export_file_name(Some("a<b>c:d\"e|f?g*h\u{7}.PDF"), kind, pdf, today),
            "abcdefgh.pdf"
        );
        assert_eq!(
            sanitize_export_file_name(Some("CON.pdf"), kind, pdf, today),
            "reporte-CON.pdf"
        );
        assert_eq!(
            sanitize_export_file_name(None, kind, ExportFormat::Xlsx, today),
            "reporte-daily-hours_2026-10-15.xlsx"
        );
        assert_eq!(
            sanitize_export_file_name(Some(" ... "), kind, ExportFormat::Docx, today),
            "reporte-daily-hours_2026-10-15.docx"
        );
        let long = sanitize_export_file_name(Some(&"x".repeat(400)), kind, pdf, today);
        assert_eq!(long.chars().count(), MAX_FILE_NAME_CHARS);
        assert!(long.ends_with(".pdf"));
    }

    #[test]
    fn test_reports_format_magic_bytes() {
        assert!(ExportFormat::Pdf.matches_content(&pdf_bytes()));
        assert!(ExportFormat::Xlsx.matches_content(b"PK\x03\x04rest"));
        assert!(ExportFormat::Docx.matches_content(b"PK\x03\x04rest"));
        assert!(!ExportFormat::Pdf.matches_content(b"PK\x03\x04"));
        assert!(!ExportFormat::Xlsx.matches_content(b"<html>error</html>"));
    }

    #[tokio::test]
    async fn test_reports_export_saves_with_server_name_and_never_overwrites() {
        let server = MockServer::start().await;
        let client = make_test_client(&server);
        let name = "reporte-asistencia_76.123.456-0_2026-10-01_2026-10-31.pdf";
        Mock::given(method("GET"))
            .and(path("/attendance/reports/attendance/export"))
            .and(query_param("format", "pdf"))
            .and(query_param("from", "2026-10-01"))
            .and(query_param("to", "2026-10-31"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Type", "application/pdf")
                    .insert_header(
                        "Content-Disposition",
                        format!("attachment; filename=\"{name}\"").as_str(),
                    )
                    .set_body_bytes(pdf_bytes()),
            )
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let filters = range("2026-10-01", "2026-10-31");
        let first = attendance_export_report_with_client(
            &client,
            ReportKind::Attendance,
            &filters,
            ExportFormat::Pdf,
            dir.path(),
        )
        .await
        .unwrap();
        assert_eq!(first, dir.path().join(name));
        assert_eq!(std::fs::read(&first).unwrap(), pdf_bytes());

        let second = attendance_export_report_with_client(
            &client,
            ReportKind::Attendance,
            &filters,
            ExportFormat::Pdf,
            dir.path(),
        )
        .await
        .unwrap();
        assert_eq!(
            second,
            dir.path()
                .join("reporte-asistencia_76.123.456-0_2026-10-01_2026-10-31 (2).pdf")
        );
        // The first file is intact.
        assert_eq!(std::fs::read(&first).unwrap(), pdf_bytes());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&second).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[tokio::test]
    async fn test_reports_export_size_limit() {
        let server = MockServer::start().await;
        let client = make_test_client(&server);
        let mut big = b"PK\x03\x04".to_vec();
        big.resize(MAX_EXPORT_BYTES + 1, 0);
        Mock::given(method("GET"))
            .and(path("/attendance/reports/daily-hours/export"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(big))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let err = attendance_export_report_with_client(
            &client,
            ReportKind::DailyHours,
            &last_week(),
            ExportFormat::Xlsx,
            dir.path(),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "EXPORT_TOO_LARGE");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn test_reports_export_rejects_body_of_another_format() {
        let server = MockServer::start().await;
        let client = make_test_client(&server);
        Mock::given(method("GET"))
            .and(path("/attendance/reports/incidents/export"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>oops</html>"))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let err = attendance_export_report_with_client(
            &client,
            ReportKind::Incidents,
            &last_week(),
            ExportFormat::Docx,
            dir.path(),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "INVALID_EXPORT_FILE");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn test_reports_export_402_writes_nothing_and_exempt_200_saves() {
        let blocked = MockServer::start().await;
        let blocked_client = make_test_client(&blocked);
        Mock::given(method("GET"))
            .and(path("/attendance/reports/attendance/export"))
            .respond_with(ResponseTemplate::new(402).set_body_json(serde_json::json!({
                "error": {
                    "code": "SUBSCRIPTION_SUSPENDED",
                    "message": "Subscription suspended",
                    "details": {
                        "reason": "TRIAL_EXPIRED",
                        "subscription_status": "SUSPENDED",
                        "portal_url": "https://portal.example.cl/billing",
                        "support_email": "soporte@example.cl"
                    }
                }
            })))
            .mount(&blocked)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let err = attendance_export_report_with_client(
            &blocked_client,
            ReportKind::Attendance,
            &last_week(),
            ExportFormat::Pdf,
            dir.path(),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "SUBSCRIPTION_SUSPENDED");
        assert_eq!(err.reason.as_deref(), Some("TRIAL_EXPIRED"));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);

        // Exempt (BILL-04): a suspended admin's export is answered 200 and saved; without a
        // Content-Disposition name the fallback name is used.
        let exempt = MockServer::start().await;
        let exempt_client = make_test_client(&exempt);
        Mock::given(method("GET"))
            .and(path("/attendance/reports/attendance/export"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(pdf_bytes()))
            .mount(&exempt)
            .await;
        let saved = attendance_export_report_with_client(
            &exempt_client,
            ReportKind::Attendance,
            &last_week(),
            ExportFormat::Pdf,
            dir.path(),
        )
        .await
        .unwrap();
        let name = saved.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("reporte-attendance_") && name.ends_with(".pdf"));
    }

    #[test]
    fn test_reports_write_new_file_suffixes() {
        let dir = tempfile::tempdir().unwrap();
        let a = write_new_file(dir.path(), "r.xlsx", b"1").unwrap();
        let b = write_new_file(dir.path(), "r.xlsx", b"2").unwrap();
        let c = write_new_file(dir.path(), "r.xlsx", b"3").unwrap();
        assert_eq!(a, dir.path().join("r.xlsx"));
        assert_eq!(b, dir.path().join("r (2).xlsx"));
        assert_eq!(c, dir.path().join("r (3).xlsx"));
        assert_eq!(std::fs::read(&a).unwrap(), b"1");
    }
}
