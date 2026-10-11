use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Base64 decoding failed: {0}")]
    Base64Error(#[from] base64::DecodeError),

    #[error("Key length invalid: expected {expected} bytes, got {got}")]
    InvalidKeyLength { expected: usize, got: usize },

    #[error("Payload too short: minimum required is 28 bytes (12B Nonce + 16B Tag)")]
    PayloadTooShort,

    #[error("HKDF key derivation failed")]
    KeyDerivationFailed,

    #[error("Decryption failed: corrupted data or invalid authentication tag")]
    AeadError,

    #[error("Serialization failed: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Serialization failed: {0}")]
    Serialization(String),

    #[error("Invalid commitment hash: expected 64-hex string")]
    InvalidCommitment,

    #[error("Invalid enrollment QR payload: {0}")]
    InvalidQr(String),

    #[error("Invalid public key")]
    InvalidPublicKey,

    #[error("Invalid consent version: must be >= 1")]
    InvalidConsentVersion,

    #[error("Invalid passphrase: {0}")]
    InvalidPassphrase(String),

    #[error("Invalid backup format: {0}")]
    InvalidBackup(String),

    #[error("Insecure or unsupported key derivation parameters: {0}")]
    InsecureKdfParams(String),

    #[error("The backup contains a different key than expected")]
    BackupKeyMismatch,

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}
