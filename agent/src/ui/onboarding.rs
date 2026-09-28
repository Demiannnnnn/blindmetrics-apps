use serde::{Deserialize, Serialize};
use telemetry_crypto_core::decode_public_key;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnrollmentPackage {
    pub api_url: String,
    pub worker_id: String,
    pub admin_public_key: String,
    pub organization: String,
}

pub struct OnboardingManager;

impl OnboardingManager {
    /// Validates and parses the enrollment JSON string provided by the Admin.
    pub fn parse_enrollment_json(json_str: &str) -> Result<EnrollmentPackage, String> {
        let pkg: EnrollmentPackage = serde_json::from_str(json_str)
            .map_err(|e| format!("Invalid enrollment JSON format: {}", e))?;

        if !pkg.api_url.starts_with("http://") && !pkg.api_url.starts_with("https://") {
            return Err("Invalid API URL: must start with http:// or https://".into());
        }

        if pkg.worker_id.trim().is_empty() {
            return Err("Worker ID cannot be empty".into());
        }

        // Validate that admin_public_key is a valid 32-byte X25519 key
        decode_public_key(&pkg.admin_public_key)
            .map_err(|e| format!("Invalid Admin public key in enrollment package: {}", e))?;

        Ok(pkg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use telemetry_crypto_core::generate_admin_keypair;

    #[test]
    fn test_valid_enrollment_parsing() {
        let (_, admin_pub) = generate_admin_keypair();
        let json_input = format!(
            r#"{{
                "api_url": "https://telemetry.acme.com/api/v1",
                "worker_id": "01912345-6789-7abc-def0-123456789def",
                "admin_public_key": "{}",
                "organization": "Acme Corp"
            }}"#,
            admin_pub
        );

        let pkg = OnboardingManager::parse_enrollment_json(&json_input).expect("Failed to parse");
        assert_eq!(pkg.organization, "Acme Corp");
        assert_eq!(pkg.admin_public_key, admin_pub);
    }

    #[test]
    fn test_invalid_key_fails_enrollment() {
        let json_input = r#"{
            "api_url": "https://telemetry.acme.com/api/v1",
            "worker_id": "worker-1",
            "admin_public_key": "invalid-key-short",
            "organization": "Acme Corp"
        }"#;

        let result = OnboardingManager::parse_enrollment_json(json_input);
        assert!(result.is_err());
    }
}
