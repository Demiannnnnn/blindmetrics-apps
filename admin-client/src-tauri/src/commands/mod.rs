pub mod keys;
pub mod session;
pub mod telemetry;
pub mod workers;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

impl From<String> for CommandError {
    fn from(s: String) -> Self {
        Self {
            code: "INTERNAL_ERROR".into(),
            message: s,
        }
    }
}

impl From<&str> for CommandError {
    fn from(s: &str) -> Self {
        Self {
            code: "INTERNAL_ERROR".into(),
            message: s.into(),
        }
    }
}

impl From<telemetry_crypto_core::CryptoError> for CommandError {
    fn from(e: telemetry_crypto_core::CryptoError) -> Self {
        Self {
            code: "CRYPTO_ERROR".into(),
            message: e.to_string(),
        }
    }
}
