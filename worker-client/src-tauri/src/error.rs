use serde::{Deserialize, Serialize};

/// Error format returned by Tauri commands ({ code, message }).
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error, PartialEq, Eq)]
#[error("[{code}] {message}")]
pub struct CommandError {
    pub code: String,
    pub message: String,
    /// Seconds before the action may be retried (PIN delay, rate limit), when the service says so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_secs: Option<u64>,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retry_after_secs: None,
        }
    }

    /// Same as [`CommandError::new`] with the retry delay the service returned.
    pub fn with_retry(mut self, retry_after_secs: Option<u64>) -> Self {
        self.retry_after_secs = retry_after_secs;
        self
    }
}

impl From<agent_ipc::IpcError> for CommandError {
    fn from(err: agent_ipc::IpcError) -> Self {
        match err {
            agent_ipc::IpcError::ServiceNotRunning(msg) => {
                CommandError::new("SERVICE_NOT_RUNNING", msg)
            }
            agent_ipc::IpcError::Unauthorized(msg) => CommandError::new("UNAUTHORIZED", msg),
            agent_ipc::IpcError::ProtocolMismatch(msg) => {
                CommandError::new("PROTOCOL_MISMATCH", msg)
            }
            agent_ipc::IpcError::Timeout(msg) => CommandError::new("TIMEOUT", msg),
            agent_ipc::IpcError::Io(msg) => CommandError::new("IO_ERROR", msg),
            agent_ipc::IpcError::Rejected { code, message } => CommandError::new(code, message),
            agent_ipc::IpcError::CallError {
                code,
                message,
                retry_after_secs,
            } => CommandError::new(code, message).with_retry(retry_after_secs),
            agent_ipc::IpcError::Framing(msg) => CommandError::new("FRAMING_ERROR", msg),
        }
    }
}
