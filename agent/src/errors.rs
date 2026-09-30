use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("HTTP client error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("API error ({status}): {code} - {reason:?}")]
    Api {
        status: u16,
        code: String,
        reason: Option<String>,
        /// `Retry-After` header or `details.retry_after_seconds` (TASK-INT-02).
        retry_after_seconds: Option<u64>,
        /// Raw `error.details` (e.g. `current_consent_version`, `categories`).
        details: Option<serde_json::Value>,
    },

    #[error("Server returned status {0}")]
    ServerError(u16),

    #[error("Authentication required or session expired")]
    Unauthorized,

    #[error("Crypto error: {0}")]
    Crypto(#[from] telemetry_crypto_core::CryptoError),
}

impl AgentError {
    /// `details.reason` of an API error.
    pub fn reason(&self) -> Option<&str> {
        match self {
            AgentError::Api { reason, .. } => reason.as_deref(),
            _ => None,
        }
    }

    /// `error.code` of an API error.
    pub fn code(&self) -> Option<&str> {
        match self {
            AgentError::Api { code, .. } => Some(code.as_str()),
            _ => None,
        }
    }

    /// True for `402`/`403` answers caused by the employer's subscription (BILL-03).
    pub fn is_subscription_block(&self) -> bool {
        matches!(
            self.code(),
            Some("SUBSCRIPTION_SUSPENDED") | Some("SUBSCRIPTION_CANCELLED")
        )
    }
}
