use super::keys::get_admin_public_key;
use super::CommandError;
use crate::keystore::load_admin_private_key;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use telemetry_crypto_core::decrypt_telemetry_record;
use zeroize::Zeroizing;

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

/// High-throughput parallel decryption pipeline using Rayon multithreading (PERF-07).
/// Distributes decryption across all available CPU cores with zeroized intermediate keys.
pub fn decrypt_records_parallel(
    admin_priv_key: &str,
    records: Vec<EncryptedRecordDto>,
) -> Result<Vec<DecryptedRecordDto>, CommandError> {
    let priv_key = Zeroizing::new(admin_priv_key.to_string());

    records
        .into_par_iter()
        .map(|rec| {
            let plaintext: serde_json::Value = decrypt_telemetry_record(
                &priv_key,
                &rec.ephemeral_public_key,
                &rec.encrypted_payload,
            )
            .map_err(|e| CommandError {
                code: "DECRYPTION_FAILED".into(),
                message: format!("Crypto decryption failure on record ID {}: {}", rec.id, e),
            })?;

            Ok(DecryptedRecordDto {
                id: rec.id,
                client_timestamp: rec.client_timestamp,
                payload: plaintext,
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()
}

#[tauri::command]
pub fn admin_decrypt_records(
    records: Vec<EncryptedRecordDto>,
) -> Result<Vec<DecryptedRecordDto>, CommandError> {
    let priv_key = load_admin_private_key().map_err(|e| CommandError {
        code: "KEYSTORE_ERROR".into(),
        message: format!("Keystore error: {}", e),
    })?;
    decrypt_records_parallel(&priv_key, records)
}

#[tauri::command]
pub fn admin_simulate_and_decrypt() -> Result<DecryptedRecordDto, CommandError> {
    let pub_b64 = match get_admin_public_key() {
        Ok(k) => k,
        Err(_) => super::keys::generate_and_save_master_keys()?,
    };

    let sample = serde_json::json!({
        "device_identifier": "macbook-air-m2-corporate-node",
        "worker_name": "Carlos Valenzuela",
        "cpu_usage_pct": 28.5,
        "ram_usage_pct": 64.2,
        "keystrokes_per_minute": 142,
        "clicks_per_minute": 38,
        "active_window": "VS Code — telemetry-apps/agent",
        "top_fqdns": ["github.com", "crates.io", "docs.rs"],
        "encrypted_at": chrono::Utc::now().to_rfc3339()
    });

    let envelope =
        telemetry_crypto_core::encrypt_telemetry_batch(&sample, &pub_b64).map_err(|e| {
            CommandError {
                code: "ENCRYPTION_ERROR".into(),
                message: format!("Encryption error: {}", e),
            }
        })?;

    let priv_key = load_admin_private_key().map_err(|e| CommandError {
        code: "KEYSTORE_ERROR".into(),
        message: format!("Keystore error: {}", e),
    })?;
    let records = vec![EncryptedRecordDto {
        id: format!(
            "sim-{}",
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
        ),
        client_timestamp: chrono::Utc::now(),
        ephemeral_public_key: envelope.ephemeral_public_key,
        encrypted_payload: envelope.encrypted_payload,
    }];

    let mut decrypted = decrypt_records_parallel(&priv_key, records)?;
    decrypted.pop().ok_or_else(|| CommandError {
        code: "NO_DATA".into(),
        message: "Decryption produced no output".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use telemetry_crypto_core::{encrypt_telemetry_batch, generate_admin_keypair};

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

            let envelope =
                encrypt_telemetry_batch(&payload, &admin_pub).expect("Failed to encrypt record");

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
