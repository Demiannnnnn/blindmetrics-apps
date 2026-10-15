pub mod attendance;
pub mod devices;
pub mod employer;
pub mod keys;
pub mod legal;
pub mod reports;
pub mod session;
pub mod subscription;
pub mod telemetry;
pub mod vault;
pub mod workers;

use crate::client::ApiError;
use serde::{Deserialize, Serialize};

/// Error returned by every Tauri command (conventions.md, RF-INT-02-2).
///
/// `code` is the API `error.code` or a local code; `reason` is the API `details.reason`;
/// `details` only contains whitelisted fields. Never tokens, keys or raw server bodies.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Box<serde_json::Value>>,
}

impl CommandError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            reason: None,
            details: None,
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new("VALIDATION_ERROR", message)
    }

    /// True when `code` or `reason` equals `value`.
    pub fn is(&self, value: &str) -> bool {
        self.code == value || self.reason.as_deref() == Some(value)
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.reason {
            Some(reason) => write!(f, "{} ({}): {}", self.code, reason, self.message),
            None => write!(f, "{}: {}", self.code, self.message),
        }
    }
}

impl std::error::Error for CommandError {}

impl From<ApiError> for CommandError {
    fn from(err: ApiError) -> Self {
        let details = err.safe_details();
        Self {
            code: err.code,
            message: err.message,
            reason: err.reason,
            details,
        }
    }
}

impl From<String> for CommandError {
    fn from(s: String) -> Self {
        Self::new("INTERNAL_ERROR", s)
    }
}

impl From<&str> for CommandError {
    fn from(s: &str) -> Self {
        Self::new("INTERNAL_ERROR", s)
    }
}

impl From<telemetry_crypto_core::CryptoError> for CommandError {
    fn from(e: telemetry_crypto_core::CryptoError) -> Self {
        Self::new("CRYPTO_ERROR", e.to_string())
    }
}

impl From<crate::keystore::KeystoreError> for CommandError {
    fn from(e: crate::keystore::KeystoreError) -> Self {
        Self::new("KEYSTORE_ERROR", format!("OS keystore error: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_error_from_api_error_keeps_reason_and_safe_details() {
        let api = ApiError::from_parts(
            409,
            None,
            br#"{"error":{"code":"CONFLICT","message":"m","details":{"reason":"ENROLLMENT_ALREADY_COMPLETED","hint":"REVOKE_FIRST","status":"ACTIVE","secret":"x"}}}"#,
        );
        let err = CommandError::from(api);
        assert_eq!(err.code, "CONFLICT");
        assert_eq!(err.reason.as_deref(), Some("ENROLLMENT_ALREADY_COMPLETED"));
        assert!(err.is("ENROLLMENT_ALREADY_COMPLETED") && err.is("CONFLICT"));
        let details = err.details.unwrap();
        assert_eq!(details["hint"], "REVOKE_FIRST");
        assert!(details.get("secret").is_none());

        let json = serde_json::to_value(CommandError::new("X", "y")).unwrap();
        assert!(json.get("reason").is_none() && json.get("details").is_none());
    }

    #[test]
    fn test_command_error_serialization_shape_unchanged() {
        let err = CommandError {
            code: "SAMPLE_CODE".to_string(),
            message: "A sample message".to_string(),
            reason: Some("SAMPLE_REASON".to_string()),
            details: Some(Box::new(serde_json::json!({
                "key": "value",
                "nested": { "count": 42 }
            }))),
        };
        let value = serde_json::to_value(&err).expect("serialization succeeds");
        assert_eq!(
            value,
            serde_json::json!({
                "code": "SAMPLE_CODE",
                "message": "A sample message",
                "reason": "SAMPLE_REASON",
                "details": {
                    "key": "value",
                    "nested": { "count": 42 }
                }
            })
        );

        let min_err = CommandError::new("MIN_CODE", "Min message");
        let min_value = serde_json::to_value(&min_err).expect("serialization succeeds");
        assert_eq!(
            min_value,
            serde_json::json!({
                "code": "MIN_CODE",
                "message": "Min message"
            })
        );
        assert!(min_value.get("reason").is_none());
        assert!(min_value.get("details").is_none());

        let deserialized: CommandError =
            serde_json::from_value(value).expect("deserialization succeeds");
        assert_eq!(err, deserialized);
    }
}
