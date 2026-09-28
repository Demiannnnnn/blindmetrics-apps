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
}
