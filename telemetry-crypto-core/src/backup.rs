use crate::error::CryptoError;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

type HmacSha256 = Hmac<Sha256>;

pub const DEFAULT_BACKUP_ITERATIONS: u32 = 100_000;
pub const MIN_BACKUP_PASSPHRASE_LEN: usize = 12;
pub const BACKUP_FORMAT_TAG: &str = "telemetry-vault-v1";

/// Structured representation of an encrypted master key backup.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EncryptedKeyBackup {
    /// Format identifier and version schema
    pub format: String,
    /// PBKDF2 iteration count (minimum 100,000 required)
    pub iterations: u32,
    /// Base64-encoded 16-byte random salt
    pub salt: String,
    /// Base64-encoded encrypted payload: [ Nonce (12B) | Auth Tag (16B) | Ciphertext (NB) ]
    pub encrypted_payload: String,
}

/// Derives a 256-bit key from a passphrase and salt using PBKDF2-HMAC-SHA256 (RFC 8018).
pub fn pbkdf2_hmac_sha256(
    passphrase: &[u8],
    salt: &[u8],
    iterations: u32,
) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    if iterations == 0 {
        return Err(CryptoError::KeyDerivationFailed);
    }

    // Single 32-byte block (i = 1) because dkLen = 32 bytes == hLen (SHA-256)
    let mut mac =
        HmacSha256::new_from_slice(passphrase).map_err(|_| CryptoError::KeyDerivationFailed)?;
    mac.update(salt);
    mac.update(&1u32.to_be_bytes());
    let mut u = mac.finalize().into_bytes();
    let mut t = u;

    for _ in 1..iterations {
        let mut mac =
            HmacSha256::new_from_slice(passphrase).map_err(|_| CryptoError::KeyDerivationFailed)?;
        mac.update(&u);
        u = mac.finalize().into_bytes();
        for (t_byte, u_byte) in t.iter_mut().zip(u.iter()) {
            *t_byte ^= u_byte;
        }
    }

    let mut derived = Zeroizing::new([0u8; 32]);
    derived.copy_from_slice(&t);
    Ok(derived)
}

/// Exports the Base64-encoded Admin private key into an encrypted backup blob (.vault).
///
/// Encrypts `admin_private_key_b64` using AES-256-GCM with a 256-bit key derived
/// from `passphrase` via PBKDF2-HMAC-SHA256 (100,000 iterations and 16-byte random salt).
pub fn export_encrypted_admin_key(
    admin_private_key_b64: &str,
    passphrase: &str,
) -> Result<Vec<u8>, CryptoError> {
    if passphrase.len() < MIN_BACKUP_PASSPHRASE_LEN {
        return Err(CryptoError::InvalidPassphrase(format!(
            "Passphrase must have at least {} characters",
            MIN_BACKUP_PASSPHRASE_LEN
        )));
    }

    // Verify input is a valid 32-byte X25519 private key
    let _ = crate::asymmetric::decode_private_key(admin_private_key_b64)?;

    let mut salt = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut salt);

    let derived_key = pbkdf2_hmac_sha256(passphrase.as_bytes(), &salt, DEFAULT_BACKUP_ITERATIONS)?;
    let encrypted_bytes =
        crate::symmetric::encrypt_bytes(&derived_key, admin_private_key_b64.as_bytes())?;

    let backup = EncryptedKeyBackup {
        format: BACKUP_FORMAT_TAG.to_string(),
        iterations: DEFAULT_BACKUP_ITERATIONS,
        salt: BASE64_STANDARD.encode(salt),
        encrypted_payload: BASE64_STANDARD.encode(encrypted_bytes),
    };

    let json_bytes = serde_json::to_vec_pretty(&backup)?;
    Ok(json_bytes)
}

/// Imports and decrypts an encrypted master key backup blob (.vault).
///
/// Verifies iteration count (>= 100,000), derives the key via PBKDF2-HMAC-SHA256,
/// decrypts with AES-256-GCM, and validates the reconstructed private key.
pub fn import_encrypted_admin_key(
    backup_data: &[u8],
    passphrase: &str,
) -> Result<String, CryptoError> {
    if passphrase.len() < MIN_BACKUP_PASSPHRASE_LEN {
        return Err(CryptoError::InvalidPassphrase(format!(
            "Passphrase must have at least {} characters",
            MIN_BACKUP_PASSPHRASE_LEN
        )));
    }

    let backup: EncryptedKeyBackup = serde_json::from_slice(backup_data)
        .map_err(|e| CryptoError::InvalidBackup(format!("Malformed backup container: {}", e)))?;

    if backup.format != BACKUP_FORMAT_TAG {
        return Err(CryptoError::InvalidBackup(format!(
            "Unsupported backup format: {}",
            backup.format
        )));
    }

    if backup.iterations < 100_000 {
        return Err(CryptoError::InvalidBackup(
            "Iteration count must be >= 100,000".into(),
        ));
    }

    let salt = BASE64_STANDARD
        .decode(&backup.salt)
        .map_err(|e| CryptoError::InvalidBackup(format!("Corrupted salt: {}", e)))?;

    let encrypted_payload = BASE64_STANDARD
        .decode(&backup.encrypted_payload)
        .map_err(|e| CryptoError::InvalidBackup(format!("Corrupted encrypted payload: {}", e)))?;

    let derived_key = pbkdf2_hmac_sha256(passphrase.as_bytes(), &salt, backup.iterations)?;
    let decrypted_bytes = crate::symmetric::decrypt_bytes(&derived_key, &encrypted_payload)?;

    let priv_key_str = String::from_utf8(decrypted_bytes).map_err(|_| CryptoError::AeadError)?;

    // Validate that the decrypted string is indeed a valid X25519 private key
    let _ = crate::asymmetric::decode_private_key(&priv_key_str)?;

    Ok(priv_key_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asymmetric::{derive_public_key, generate_admin_keypair};

    #[test]
    fn test_pbkdf2_hmac_sha256_vector() {
        // RFC 6070 test vector adapted for HMAC-SHA256 (c=1)
        let pass = b"password";
        let salt = b"salt";
        let dk = pbkdf2_hmac_sha256(pass, salt, 1).expect("PBKDF2 failed");
        let hex_dk = hex::encode(*dk);
        assert_eq!(
            hex_dk,
            "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
        );
    }

    #[test]
    fn test_backup_roundtrip_valid_passphrase() {
        let (admin_priv, admin_pub) = generate_admin_keypair();
        let passphrase = "correct-horse-battery-staple-123";

        let backup_blob =
            export_encrypted_admin_key(&admin_priv, passphrase).expect("Export failed");
        assert!(!backup_blob.is_empty());

        let imported_priv =
            import_encrypted_admin_key(&backup_blob, passphrase).expect("Import failed");
        assert_eq!(admin_priv, imported_priv);

        let derived_pub = derive_public_key(&imported_priv).expect("Derive pubkey failed");
        assert_eq!(admin_pub, derived_pub);
    }

    #[test]
    fn test_backup_encryption_invalid_passphrase() {
        let (admin_priv, _) = generate_admin_keypair();
        let passphrase = "correct-horse-battery-staple-123";
        let wrong_passphrase = "wrong-horse-battery-staple-456";

        let backup_blob =
            export_encrypted_admin_key(&admin_priv, passphrase).expect("Export failed");

        let err = import_encrypted_admin_key(&backup_blob, wrong_passphrase).unwrap_err();
        assert!(
            matches!(err, CryptoError::AeadError),
            "Expected AeadError on wrong passphrase, got: {:?}",
            err
        );
    }

    #[test]
    fn test_backup_too_short_passphrase() {
        let (admin_priv, _) = generate_admin_keypair();
        let short_passphrase = "short";

        let export_err = export_encrypted_admin_key(&admin_priv, short_passphrase).unwrap_err();
        assert!(matches!(export_err, CryptoError::InvalidPassphrase(_)));

        let dummy_blob = b"{}";
        let import_err = import_encrypted_admin_key(dummy_blob, short_passphrase).unwrap_err();
        assert!(matches!(import_err, CryptoError::InvalidPassphrase(_)));
    }

    #[test]
    fn test_backup_tampered_payload_rejected() {
        let (admin_priv, _) = generate_admin_keypair();
        let passphrase = "secure-password-with-sufficient-entropy-99";

        let backup_bytes =
            export_encrypted_admin_key(&admin_priv, passphrase).expect("Export failed");
        let mut backup: EncryptedKeyBackup = serde_json::from_slice(&backup_bytes).unwrap();

        // Tamper with encrypted_payload base64
        let mut payload_bytes = BASE64_STANDARD.decode(&backup.encrypted_payload).unwrap();
        let last = payload_bytes.len() - 1;
        payload_bytes[last] ^= 0x01;
        backup.encrypted_payload = BASE64_STANDARD.encode(&payload_bytes);

        let tampered_bytes = serde_json::to_vec(&backup).unwrap();
        let res = import_encrypted_admin_key(&tampered_bytes, passphrase);
        assert!(matches!(res, Err(CryptoError::AeadError)));
    }
}
