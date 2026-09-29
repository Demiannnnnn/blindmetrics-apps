pub mod asymmetric;
pub mod error;
pub mod kdf;
pub mod payloads;
pub mod symmetric;

pub use payloads::*;

pub use asymmetric::{
    decode_private_key, decode_public_key, derive_public_key, generate_admin_keypair,
};
pub use error::CryptoError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// High-level encrypted container matching the API specification and database schema.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedEnvelope {
    /// Base64-encoded ephemeral public key (X25519)
    pub ephemeral_public_key: String,
    /// Binary format: `[ Nonce (12B) | Auth Tag (16B) | Ciphertext (NB) ]`
    pub encrypted_payload: Vec<u8>,
}

/// Encrypts an arbitrary serializable telemetry batch using Hybrid Public Key Encryption (HPKE).
/// 1. Generates an ephemeral X25519 keypair.
/// 2. Derives a shared secret via ECDH with the Admin's public key.
/// 3. Derives an AES-256-GCM DEK using HKDF-SHA256 with domain separation.
/// 4. Encrypts the serialized JSON payload with AES-256-GCM.
/// 5. Zeroizes ephemeral private key, shared secret, and DEK from RAM.
pub fn encrypt_telemetry_batch<T: Serialize>(
    payload: &T,
    admin_public_key_b64: &str,
) -> Result<EncryptedEnvelope, CryptoError> {
    let admin_public = asymmetric::decode_public_key(admin_public_key_b64)?;
    let (eph_pub_b64, shared_secret) = asymmetric::compute_ephemeral_shared_secret(&admin_public);
    let dek = kdf::derive_shared_key(&shared_secret)?;

    let json_bytes = serde_json::to_vec(payload)?;
    let encrypted_payload = symmetric::encrypt_bytes(&dek, &json_bytes)?;

    Ok(EncryptedEnvelope {
        ephemeral_public_key: eph_pub_b64,
        encrypted_payload,
    })
}

/// Decrypts a telemetry record on the Admin workstation using the Admin's master private key.
/// 1. Reconstructs the shared secret via ECDH between Admin private key and the ephemeral public key.
/// 2. Derives the AES-256-GCM DEK using HKDF-SHA256.
/// 3. Authenticates and decrypts the ciphertext.
/// 4. Deserializes the plaintext into the requested target type.
/// 5. Zeroizes intermediate keys and secrets from RAM.
pub fn decrypt_telemetry_record<T: DeserializeOwned>(
    admin_private_key_b64: &str,
    ephemeral_public_key_b64: &str,
    encrypted_payload: &[u8],
) -> Result<T, CryptoError> {
    let admin_private = asymmetric::decode_private_key(admin_private_key_b64)?;
    let eph_public = asymmetric::decode_public_key(ephemeral_public_key_b64)?;

    let shared_secret = asymmetric::compute_static_shared_secret(&admin_private, &eph_public);
    let dek = kdf::derive_shared_key(&shared_secret)?;

    let plaintext_bytes = symmetric::decrypt_bytes(&dek, encrypted_payload)?;
    let result = serde_json::from_slice(&plaintext_bytes)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct MockTelemetry {
        worker_id: String,
        cpu_usage: u32,
        domains: Vec<String>,
    }

    #[test]
    fn test_admin_keypair_generation() {
        let (priv_b64, pub_b64) = generate_admin_keypair();
        assert_ne!(priv_b64, pub_b64);

        let derived_pub = derive_public_key(&priv_b64).expect("Derive public key failed");
        assert_eq!(pub_b64, derived_pub);
    }

    #[test]
    fn test_roundtrip_encryption_decryption() {
        let (admin_priv, admin_pub) = generate_admin_keypair();

        let original = MockTelemetry {
            worker_id: "01912345-6789-7abc-def0-123456789def".into(),
            cpu_usage: 42,
            domains: vec!["github.com".into(), "rust-lang.org".into()],
        };

        let envelope = encrypt_telemetry_batch(&original, &admin_pub).expect("Encryption failed");

        assert!(!envelope.ephemeral_public_key.is_empty());
        assert!(envelope.encrypted_payload.len() >= 28);

        let decrypted: MockTelemetry = decrypt_telemetry_record(
            &admin_priv,
            &envelope.ephemeral_public_key,
            &envelope.encrypted_payload,
        )
        .expect("Decryption failed");

        assert_eq!(original, decrypted);
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let (admin_priv, admin_pub) = generate_admin_keypair();
        let original = MockTelemetry {
            worker_id: "test-worker".into(),
            cpu_usage: 10,
            domains: vec![],
        };

        let mut envelope = encrypt_telemetry_batch(&original, &admin_pub).unwrap();
        // Flip one bit in the ciphertext part (offset 28+)
        let last_idx = envelope.encrypted_payload.len() - 1;
        envelope.encrypted_payload[last_idx] ^= 0x01;

        let result: Result<MockTelemetry, _> = decrypt_telemetry_record(
            &admin_priv,
            &envelope.ephemeral_public_key,
            &envelope.encrypted_payload,
        );

        assert!(matches!(result, Err(CryptoError::AeadError)));
    }

    #[test]
    fn test_tampered_tag_fails() {
        let (admin_priv, admin_pub) = generate_admin_keypair();
        let original = MockTelemetry {
            worker_id: "test-worker".into(),
            cpu_usage: 10,
            domains: vec![],
        };

        let mut envelope = encrypt_telemetry_batch(&original, &admin_pub).unwrap();
        // Flip one bit in the tag (offset 12..28)
        envelope.encrypted_payload[15] ^= 0xFF;

        let result: Result<MockTelemetry, _> = decrypt_telemetry_record(
            &admin_priv,
            &envelope.ephemeral_public_key,
            &envelope.encrypted_payload,
        );

        assert!(matches!(result, Err(CryptoError::AeadError)));
    }

    #[test]
    fn test_payload_too_short() {
        let (admin_priv, _) = generate_admin_keypair();
        let short_payload = vec![0u8; 20]; // Less than 28 bytes
        let (_, dummy_eph) = generate_admin_keypair();

        let result: Result<MockTelemetry, _> =
            decrypt_telemetry_record(&admin_priv, &dummy_eph, &short_payload);

        assert!(matches!(result, Err(CryptoError::PayloadTooShort)));
    }
}
