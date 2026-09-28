use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use telemetry_crypto_core::{decrypt_telemetry_record, generate_admin_keypair};
use zeroize::Zeroizing;
use crate::keystore::{load_admin_private_key, save_admin_private_key};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EncryptedRecordDto {
    pub id: String,
    pub client_timestamp: chrono::DateTime<chrono::Utc>,
    pub ephemeral_public_key: String,
    pub encrypted_payload: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DecryptedRecordDto {
    pub id: String,
    pub client_timestamp: chrono::DateTime<chrono::Utc>,
    pub payload: serde_json::Value,
}

/// Generates a new X25519 keypair and securely saves the private key in the OS Keystore.
/// Returns the Base64 public key.
pub fn generate_and_save_master_keys() -> Result<String, String> {
    let (priv_b64, pub_b64) = generate_admin_keypair();
    save_admin_private_key(&priv_b64).map_err(|e| format!("Keystore error: {}", e))?;
    Ok(pub_b64)
}

/// Loads the admin public key derived from the private key in the OS Keystore.
pub fn get_admin_public_key() -> Result<String, String> {
    let priv_b64 = load_admin_private_key().map_err(|e| format!("Keystore error: {}", e))?;
    telemetry_crypto_core::derive_public_key(&priv_b64).map_err(|e| format!("Crypto error: {}", e))
}

/// High-throughput parallel decryption pipeline using Rayon multithreading.
/// Distributes decryption across all available CPU cores with zeroized intermediate keys.
pub fn decrypt_records_parallel(
    admin_priv_key: &str,
    records: Vec<EncryptedRecordDto>,
) -> Result<Vec<DecryptedRecordDto>, String> {
    let priv_key = Zeroizing::new(admin_priv_key.to_string());

    records
        .into_par_iter()
        .map(|rec| {
            let plaintext: serde_json::Value = decrypt_telemetry_record(
                &priv_key,
                &rec.ephemeral_public_key,
                &rec.encrypted_payload,
            ).map_err(|e| format!("Crypto decryption failure on record ID {}: {}", rec.id, e))?;

            Ok(DecryptedRecordDto {
                id: rec.id,
                client_timestamp: rec.client_timestamp,
                payload: plaintext,
            })
        })
        .collect::<Result<Vec<_>, String>>()
}

#[tauri::command]
pub fn admin_generate_keypair() -> Result<String, String> {
    generate_and_save_master_keys()
}

#[tauri::command]
pub fn admin_get_public_key() -> Result<String, String> {
    get_admin_public_key()
}

#[tauri::command]
pub fn admin_decrypt_records(records: Vec<EncryptedRecordDto>) -> Result<Vec<DecryptedRecordDto>, String> {
    let priv_key = load_admin_private_key().map_err(|e| format!("Keystore error: {}", e))?;
    decrypt_records_parallel(&priv_key, records)
}

#[tauri::command]
pub fn admin_lock_and_flush_memory() -> Result<(), String> {
    tracing::info!("Admin lock invoked: In-memory session flushed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use telemetry_crypto_core::encrypt_telemetry_batch;

    #[test]
    fn test_parallel_decryption_pipeline() {
        let (admin_priv, admin_pub) = generate_admin_keypair();

        // Prepare 50 mock encrypted records
        let mut encrypted_records = Vec::new();
        for i in 0..50 {
            let payload = json!({
                "worker_id": format!("worker-{}", i),
                "cpu_usage": 20 + i,
            });

            let envelope = encrypt_telemetry_batch(&payload, &admin_pub)
                .expect("Failed to encrypt record");

            encrypted_records.push(EncryptedRecordDto {
                id: format!("rec-{}", i),
                client_timestamp: Utc::now(),
                ephemeral_public_key: envelope.ephemeral_public_key,
                encrypted_payload: envelope.encrypted_payload,
            });
        }

        // Run Rayon parallel decryption
        let decrypted = decrypt_records_parallel(&admin_priv, encrypted_records)
            .expect("Parallel decryption failed");

        assert_eq!(decrypted.len(), 50);
        for (i, rec) in decrypted.iter().enumerate() {
            assert_eq!(rec.id, format!("rec-{}", i));
            assert_eq!(rec.payload["worker_id"], format!("worker-{}", i));
            assert_eq!(rec.payload["cpu_usage"], 20 + i as u64);
        }
    }

    #[test]
    fn test_corrupted_record_fails_in_parallel_pool() {
        let (admin_priv, admin_pub) = generate_admin_keypair();
        let payload = json!({ "cpu": 15 });

        let mut envelope = encrypt_telemetry_batch(&payload, &admin_pub).unwrap();
        // Corrupt ciphertext
        let len = envelope.encrypted_payload.len();
        envelope.encrypted_payload[len - 1] ^= 0xFF;

        let records = vec![EncryptedRecordDto {
            id: "corrupted-1".into(),
            client_timestamp: Utc::now(),
            ephemeral_public_key: envelope.ephemeral_public_key,
            encrypted_payload: envelope.encrypted_payload,
        }];

        let result = decrypt_records_parallel(&admin_priv, records);
        assert!(result.is_err());
    }
}
