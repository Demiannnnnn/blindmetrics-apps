use crate::core::metrics::MetricPayload;
use telemetry_crypto_core::{encrypt_telemetry_batch, CryptoError, EncryptedEnvelope};

pub struct EncryptionPipeline {
    admin_public_key: String,
}

impl EncryptionPipeline {
    pub fn new(admin_public_key: String) -> Self {
        Self { admin_public_key }
    }

    /// Encrypts a slice of metric payloads using the configured Admin public key.
    /// Uses ephemeral ECDH + HKDF-SHA256 + AES-256-GCM under the hood.
    pub fn encrypt_batch(&self, batch: &[MetricPayload]) -> Result<EncryptedEnvelope, CryptoError> {
        if batch.is_empty() {
            return Err(CryptoError::PayloadTooShort);
        }

        encrypt_telemetry_batch(&batch, &self.admin_public_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::metrics::ProductivityBasicItem;
    use chrono::Utc;
    use telemetry_crypto_core::{decrypt_telemetry_record, generate_admin_keypair};

    #[test]
    fn test_encryption_pipeline_integration() {
        let (admin_priv, admin_pub) = generate_admin_keypair();
        let pipeline = EncryptionPipeline::new(admin_pub);

        let batch = vec![MetricPayload::ProductivityBasic(ProductivityBasicItem {
            minute: Utc::now(),
            keystrokes: 55,
            clicks: 22,
        })];

        let envelope = pipeline.encrypt_batch(&batch).expect("Encryption failed");
        assert!(!envelope.ephemeral_public_key.is_empty());

        let decrypted: Vec<MetricPayload> = decrypt_telemetry_record(
            &admin_priv,
            &envelope.ephemeral_public_key,
            &envelope.encrypted_payload,
        )
        .expect("Decryption failed");

        assert_eq!(batch, decrypted);
    }
}
