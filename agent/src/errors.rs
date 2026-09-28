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

    #[error("Server returned status {0}")]
    ServerError(u16),

    #[error("Authentication required or session expired")]
    Unauthorized,

    #[error("Crypto error: {0}")]
    Crypto(#[from] telemetry_crypto_core::CryptoError),
}
