use serde::{Deserialize, Serialize};

/// Error format returned by Tauri commands ({ code, message }).
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error, PartialEq, Eq)]
#[error("[{code}] {message}")]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
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
            agent_ipc::IpcError::CallError { code, message, .. } => {
                CommandError::new(code, message)
            }
            agent_ipc::IpcError::Framing(msg) => CommandError::new("FRAMING_ERROR", msg),
        }
    }
}
