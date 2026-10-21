use crate::error::CommandError;
use agent_ipc::{
    AppSnapshot, Correction, DataSource, DaySummary, DiagnosticsInfo, IpcClient, LegalDocument,
    MarkRecord, MarkType, MyScheduleView, Pagination, Request, Response, SecretPin, ServerMark,
    TodayView, VerificationResult, WindowScreenHint,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use telemetry_crypto_core::ConsentFlags;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatusDto {
    pub running: bool,
    pub service_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRequestDto {
    pub request_id: String,
    pub status: String,
    pub estimated_completion: Option<String>,
}

/// Marks of a range (`MARKS` op): server marks online, local journal offline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarksPageDto {
    pub source: DataSource,
    pub marks: Vec<ServerMark>,
    pub local: Vec<MarkRecord>,
    pub pagination: Option<Pagination>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDeletionDto {
    pub request_id: String,
    pub status: String,
    pub message: Option<String>,
}

/// Pure helper to find the service binary path (RF-AGENT-11-2 / PROC-04).
pub fn find_service_binary(
    override_bin: Option<&str>,
    current_exe: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(dev_override) = override_bin {
        let trimmed = dev_override.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    #[cfg(windows)]
    let bin_name = "telemetry-agent.exe";
    #[cfg(not(windows))]
    let bin_name = "telemetry-agent";

    // 1. Sibling next to current executable
    if let Some(exe) = current_exe {
        if let Some(parent) = exe.parent() {
            let sibling = parent.join(bin_name);
            if sibling.is_file() {
                return Some(sibling);
            }
        }
    }

    // 2. Development path: target/debug/telemetry-agent (RF-AGENT-11-2)
    if let Some(exe) = current_exe {
        let mut cur = exe.parent();
        while let Some(dir) = cur {
            let candidate = dir.join("target").join("debug").join(bin_name);
            if candidate.is_file() {
                return Some(candidate);
            }
            // If already in target/debug
            if dir.file_name().and_then(|n| n.to_str()) == Some("debug") {
                let candidate = dir.join(bin_name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            cur = dir.parent();
        }
    }

    // 3. Fallback to current working directory target/debug (only when current_exe is not specified)
    if current_exe.is_none() {
        let cwd_target = Path::new("target").join("debug").join(bin_name);
        if cwd_target.is_file() {
            return Some(cwd_target);
        }

        let parent_cwd_target = Path::new("..").join("target").join("debug").join(bin_name);
        if parent_cwd_target.is_file() {
            return Some(parent_cwd_target);
        }
    }

    None
}

/// Checks the service status.
pub async fn check_service_status(runtime_dir: &Path) -> ServiceStatusDto {
    match IpcClient::connect(runtime_dir).await {
        Ok(client) => match client.call(Request::Snapshot).await {
            Ok(Response::Snapshot { snapshot }) => ServiceStatusDto {
                running: true,
                service_version: Some(snapshot.service_version),
            },
            _ => ServiceStatusDto {
                running: true,
                service_version: None,
            },
        },
        Err(_) => ServiceStatusDto {
            running: false,
            service_version: None,
        },
    }
}

/// Ensures the service is running, launching it if necessary and waiting up to 5s (PROC-04).
pub async fn ensure_service_running(runtime_dir: &Path) -> Result<(), CommandError> {
    // `TELEMETRY_AGENT_BIN` is a development override for the service binary location.
    let override_var = std::env::var("TELEMETRY_AGENT_BIN").ok();
    let current_exe = std::env::current_exe().ok();
    ensure_service_with(
        runtime_dir,
        override_var.as_deref(),
        current_exe.as_deref(),
        SERVICE_START_TIMEOUT,
    )
    .await
}

/// How long the window waits for the service socket after launching it (PROC-04).
pub const SERVICE_START_TIMEOUT: Duration = Duration::from_secs(5);

/// Same as [`ensure_service_running`] with explicit inputs (testable without touching the
/// process environment).
pub async fn ensure_service_with(
    runtime_dir: &Path,
    override_bin: Option<&str>,
    current_exe: Option<&Path>,
    timeout: Duration,
) -> Result<(), CommandError> {
    if IpcClient::connect(runtime_dir).await.is_ok() {
        return Ok(());
    }

    let bin_path = find_service_binary(override_bin, current_exe).ok_or_else(|| {
        CommandError::new(
            "SERVICE_NOT_FOUND",
            "No se encontró el ejecutable del servicio telemetry-agent",
        )
    })?;

    tracing::info!("Starting telemetry-agent service at {:?}", bin_path);
    // The service must outlive this window (PROC-01): detach it from our stdio.
    std::process::Command::new(&bin_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            CommandError::new(
                "SPAWN_FAILED",
                format!("No se pudo ejecutar {:?}: {}", bin_path, e),
            )
        })?;

    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if IpcClient::connect(runtime_dir).await.is_ok() {
            tracing::info!("Connected to service after {:?}", start.elapsed());
            return Ok(());
        }
    }

    Err(CommandError::new(
        "SERVICE_TIMEOUT",
        "El servicio no respondió en 5 segundos tras iniciarse",
    ))
}

pub async fn send_request(runtime_dir: &Path, req: Request) -> Result<Response, CommandError> {
    let client = IpcClient::connect(runtime_dir).await?;
    let resp = client.call(req).await?;
    match resp {
        Response::Error {
            code,
            message,
            retry_after_secs,
        } => Err(CommandError::new(code, message).with_retry(retry_after_secs)),
        other => Ok(other),
    }
}

pub async fn get_snapshot(runtime_dir: &Path) -> Result<AppSnapshot, CommandError> {
    match send_request(runtime_dir, Request::Snapshot).await? {
        Response::Snapshot { snapshot } => Ok(*snapshot),
        _ => Err(CommandError::new(
            "INVALID_RESPONSE",
            "Respuesta inesperada para Snapshot",
        )),
    }
}

pub async fn enroll(
    runtime_dir: &Path,
    qr_payload: Option<String>,
    code: Option<String>,
) -> Result<VerificationResult, CommandError> {
    match send_request(runtime_dir, Request::Enroll { qr_payload, code }).await? {
        Response::Enroll { result } => Ok(result),
        _ => Err(CommandError::new(
            "INVALID_RESPONSE",
            "Respuesta inesperada para Enroll",
        )),
    }
}

pub async fn confirm_safety_code(runtime_dir: &Path, matches: bool) -> Result<(), CommandError> {
    match send_request(runtime_dir, Request::ConfirmSafetyCode { matches }).await? {
        Response::ConfirmSafetyCode => Ok(()),
        _ => Err(CommandError::new(
            "INVALID_RESPONSE",
            "Respuesta inesperada para ConfirmSafetyCode",
        )),
    }
}

pub async fn consent_decision(
    runtime_dir: &Path,
    accept: Option<ConsentFlags>,
) -> Result<(), CommandError> {
    match send_request(runtime_dir, Request::ConsentDecision { accept }).await? {
        Response::ConsentDecision => Ok(()),
        _ => Err(CommandError::new(
            "INVALID_RESPONSE",
            "Respuesta inesperada para ConsentDecision",
        )),
    }
}

pub async fn withdraw(runtime_dir: &Path) -> Result<(), CommandError> {
    match send_request(runtime_dir, Request::Withdraw).await? {
        Response::Withdraw => Ok(()),
        _ => Err(CommandError::new(
            "INVALID_RESPONSE",
            "Respuesta inesperada para Withdraw",
        )),
    }
}

pub async fn data_request(
    runtime_dir: &Path,
    categories: Option<Vec<String>>,
) -> Result<DataRequestDto, CommandError> {
    match send_request(runtime_dir, Request::DataRequest { categories }).await? {
        Response::DataRequest {
            request_id,
            status,
            estimated_completion,
        } => Ok(DataRequestDto {
            request_id,
            status,
            estimated_completion,
        }),
        _ => Err(CommandError::new(
            "INVALID_RESPONSE",
            "Respuesta inesperada para DataRequest",
        )),
    }
}

pub async fn data_deletion(
    runtime_dir: &Path,
    categories: Option<Vec<String>>,
    reason: Option<String>,
) -> Result<DataDeletionDto, CommandError> {
    match send_request(runtime_dir, Request::DataDeletion { categories, reason }).await? {
        Response::DataDeletion {
            request_id,
            status,
            message,
        } => Ok(DataDeletionDto {
            request_id,
            status,
            message,
        }),
        _ => Err(CommandError::new(
            "INVALID_RESPONSE",
            "Respuesta inesperada para DataDeletion",
        )),
    }
}

pub async fn unlink(runtime_dir: &Path, confirm: String) -> Result<(), CommandError> {
    match send_request(runtime_dir, Request::Unlink { confirm }).await? {
        Response::Unlink => Ok(()),
        _ => Err(CommandError::new(
            "INVALID_RESPONSE",
            "Respuesta inesperada para Unlink",
        )),
    }
}

pub async fn diagnostics(runtime_dir: &Path) -> Result<DiagnosticsInfo, CommandError> {
    match send_request(runtime_dir, Request::Diagnostics).await? {
        Response::Diagnostics { diagnostics } => Ok(*diagnostics),
        _ => Err(CommandError::new(
            "INVALID_RESPONSE",
            "Respuesta inesperada para Diagnostics",
        )),
    }
}

pub async fn open_window_hint(
    runtime_dir: &Path,
    screen: WindowScreenHint,
) -> Result<(), CommandError> {
    match send_request(runtime_dir, Request::OpenWindowHint { screen }).await? {
        Response::OpenWindowHint => Ok(()),
        _ => Err(CommandError::new(
            "INVALID_RESPONSE",
            "Respuesta inesperada para OpenWindowHint",
        )),
    }
}

/// Answers the tray's *Salir* confirmation dialog. The service accepts it only for the pending
/// quit request identified by `nonce` (PROC-04); an unsolicited stop is refused with `NOT_ALLOWED`.
pub async fn confirm_quit(runtime_dir: &Path, nonce: String) -> Result<(), CommandError> {
    match send_request(runtime_dir, Request::ConfirmQuit { nonce }).await? {
        Response::ConfirmQuit => Ok(()),
        _ => Err(CommandError::new(
            "INVALID_RESPONSE",
            "Respuesta inesperada para ConfirmQuit",
        )),
    }
}

/// Dismisses the tray's *Salir* confirmation dialog.
pub async fn cancel_quit(runtime_dir: &Path, nonce: String) -> Result<(), CommandError> {
    match send_request(runtime_dir, Request::CancelQuit { nonce }).await? {
        Response::CancelQuit => Ok(()),
        _ => Err(CommandError::new(
            "INVALID_RESPONSE",
            "Respuesta inesperada para CancelQuit",
        )),
    }
}

fn unexpected(op: &str) -> CommandError {
    CommandError::new(
        "INVALID_RESPONSE",
        format!("Respuesta inesperada para {op}"),
    )
}

/// Embedded legal texts with version and hash (LEG-04).
pub async fn legal_documents(runtime_dir: &Path) -> Result<Vec<LegalDocument>, CommandError> {
    match send_request(runtime_dir, Request::LegalDocuments).await? {
        Response::LegalDocuments { documents } => Ok(documents),
        _ => Err(unexpected("LegalDocuments")),
    }
}

/// Accepts the displayed terms and privacy policy (LEG-04/05). Returns the accepted version.
pub async fn accept_terms(
    runtime_dir: &Path,
    version: String,
    hash: String,
    privacy_version: String,
    privacy_hash: String,
) -> Result<String, CommandError> {
    let req = Request::AcceptTerms {
        version,
        hash,
        privacy_version,
        privacy_hash,
    };
    match send_request(runtime_dir, req).await? {
        Response::AcceptTerms {
            terms_accepted_version,
        } => Ok(terms_accepted_version),
        _ => Err(unexpected("AcceptTerms")),
    }
}

/// Creates the attendance PIN (SEC-19). The PIN stays zeroized (`SecretPin`) end to end.
pub async fn set_pin(runtime_dir: &Path, pin: SecretPin) -> Result<(), CommandError> {
    match send_request(runtime_dir, Request::SetPin { pin }).await? {
        Response::SetPin => Ok(()),
        _ => Err(unexpected("SetPin")),
    }
}

/// Changes the attendance PIN after checking the current one.
pub async fn change_pin(
    runtime_dir: &Path,
    current: SecretPin,
    new: SecretPin,
) -> Result<(), CommandError> {
    match send_request(runtime_dir, Request::ChangePin { current, new }).await? {
        Response::ChangePin => Ok(()),
        _ => Err(unexpected("ChangePin")),
    }
}

/// Asks the server to email a PIN reset code.
pub async fn request_pin_reset(runtime_dir: &Path) -> Result<(), CommandError> {
    match send_request(runtime_dir, Request::RequestPinReset).await? {
        Response::RequestPinReset => Ok(()),
        _ => Err(unexpected("RequestPinReset")),
    }
}

/// Verifies the emailed reset code. Returns the seconds during which `SetPin` is allowed.
pub async fn verify_pin_reset(runtime_dir: &Path, code: String) -> Result<u64, CommandError> {
    match send_request(runtime_dir, Request::VerifyPinReset { code }).await? {
        Response::VerifyPinReset {
            set_pin_window_secs,
        } => Ok(set_pin_window_secs),
        _ => Err(unexpected("VerifyPinReset")),
    }
}

/// Records a mark confirmed with the PIN (ATT-01). Returns the local journal record.
pub async fn mark(
    runtime_dir: &Path,
    r#type: MarkType,
    pin: SecretPin,
) -> Result<MarkRecord, CommandError> {
    match send_request(runtime_dir, Request::Mark { r#type, pin }).await? {
        Response::Mark { mark } => Ok(*mark),
        _ => Err(unexpected("Mark")),
    }
}

/// Today's attendance view.
pub async fn today(runtime_dir: &Path) -> Result<TodayView, CommandError> {
    match send_request(runtime_dir, Request::Today).await? {
        Response::Today { today } => Ok(*today),
        _ => Err(unexpected("Today")),
    }
}

/// Server day summaries of a range (ATT-09).
pub async fn days(
    runtime_dir: &Path,
    from: String,
    to: String,
) -> Result<Vec<DaySummary>, CommandError> {
    match send_request(runtime_dir, Request::Days { from, to }).await? {
        Response::Days { days } => Ok(days),
        _ => Err(unexpected("Days")),
    }
}

/// Agreed schedule in force and the next version (RF-AGENT-16-2).
pub async fn schedule(runtime_dir: &Path) -> Result<MyScheduleView, CommandError> {
    match send_request(runtime_dir, Request::Schedule).await? {
        Response::Schedule { schedule } => Ok(*schedule),
        _ => Err(unexpected("Schedule")),
    }
}

/// Marks of a range (server online, local journal offline).
pub async fn marks(
    runtime_dir: &Path,
    from: String,
    to: String,
    page: Option<u32>,
) -> Result<MarksPageDto, CommandError> {
    match send_request(runtime_dir, Request::Marks { from, to, page }).await? {
        Response::Marks {
            source,
            marks,
            local,
            pagination,
        } => Ok(MarksPageDto {
            source,
            marks,
            local,
            pagination,
        }),
        _ => Err(unexpected("Marks")),
    }
}

/// Corrections proposed by the employer (ATT-05).
pub async fn corrections(runtime_dir: &Path) -> Result<Vec<Correction>, CommandError> {
    match send_request(runtime_dir, Request::Corrections).await? {
        Response::Corrections { corrections } => Ok(corrections),
        _ => Err(unexpected("Corrections")),
    }
}

/// Accepts or opposes a pending correction (ATT-05).
pub async fn respond_correction(
    runtime_dir: &Path,
    id: String,
    accept: bool,
) -> Result<Correction, CommandError> {
    match send_request(runtime_dir, Request::RespondCorrection { id, accept }).await? {
        Response::RespondCorrection { correction } => Ok(*correction),
        _ => Err(unexpected("RespondCorrection")),
    }
}
