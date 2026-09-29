use super::keys::get_admin_public_key;
use super::CommandError;
use crate::client::{get_api_client, CollectionResponse};
use crate::keystore::load_admin_private_key;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApiTelemetryRecord {
    pub id: String,
    pub client_timestamp: chrono::DateTime<chrono::Utc>,
    pub ephemeral_public_key: String,
    pub encrypted_payload: serde_json::Value,
    #[serde(default)]
    pub subcategory: Option<String>,
}

/// Helper to decode either base64-encoded string or byte array payload.
pub fn parse_encrypted_payload(val: &serde_json::Value) -> Result<Vec<u8>, CommandError> {
    match val {
        serde_json::Value::String(s) => STANDARD.decode(s).map_err(|e| CommandError {
            code: "BASE64_DECODE_ERROR".into(),
            message: format!("Invalid base64 payload: {}", e),
        }),
        serde_json::Value::Array(arr) => {
            let bytes: Result<Vec<u8>, _> = arr
                .iter()
                .map(|v| {
                    v.as_u64()
                        .and_then(|n| if n <= 255 { Some(n as u8) } else { None })
                        .ok_or_else(|| CommandError {
                            code: "INVALID_BYTE_ARRAY".into(),
                            message: "Invalid byte value in payload".into(),
                        })
                })
                .collect();
            bytes
        }
        _ => Err(CommandError {
            code: "INVALID_PAYLOAD_FORMAT".into(),
            message: "Payload is neither base64 string nor byte array".into(),
        }),
    }
}

/// Strict high-throughput parallel decryption pipeline using Rayon multithreading (PERF-07).
/// Fails the batch if any record fails decryption.
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

/// Tolerant parallel decryption pipeline (RF-ADMIN-04-2).
/// Individual corrupted records are skipped and logged with their IDs, allowing valid records to be recovered.
pub fn decrypt_records_parallel_tolerant(
    admin_priv_key: &str,
    records: Vec<EncryptedRecordDto>,
) -> (Vec<DecryptedRecordDto>, Vec<String>) {
    let priv_key = Zeroizing::new(admin_priv_key.to_string());

    let results: Vec<Result<DecryptedRecordDto, String>> = records
        .into_par_iter()
        .map(|rec| {
            match decrypt_telemetry_record(
                &priv_key,
                &rec.ephemeral_public_key,
                &rec.encrypted_payload,
            ) {
                Ok(plaintext) => Ok(DecryptedRecordDto {
                    id: rec.id,
                    client_timestamp: rec.client_timestamp,
                    payload: plaintext,
                }),
                Err(e) => {
                    tracing::warn!(
                        "Failed to decrypt record ID {}: tag mismatch or error: {}",
                        rec.id,
                        e
                    );
                    Err(rec.id)
                }
            }
        })
        .collect();

    let mut decrypted = Vec::new();
    let mut failed = Vec::new();

    for res in results {
        match res {
            Ok(d) => decrypted.push(d),
            Err(id) => failed.push(id),
        }
    }

    (decrypted, failed)
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

/// Fetches encrypted telemetry batches from the API and decrypts them in parallel in memory (RF-ADMIN-04-1..4).
#[tauri::command]
pub async fn fetch_and_decrypt_telemetry(
    worker_id: String,
    category: String,
    from: Option<String>,
    to: Option<String>,
    subcategory: Option<String>,
    page: Option<u32>,
    per_page: Option<u32>,
) -> Result<Vec<DecryptedRecordDto>, CommandError> {
    let mut query_params = Vec::new();
    query_params.push(format!("worker_id={}", worker_id));

    if let Some(f) = from {
        query_params.push(format!("from={}", f));
    }
    if let Some(t) = to {
        query_params.push(format!("to={}", t));
    }
    if let Some(sub) = subcategory {
        query_params.push(format!("subcategory={}", sub));
    }
    if let Some(p) = page {
        query_params.push(format!("page={}", p));
    }
    if let Some(pp) = per_page {
        query_params.push(format!("per_page={}", pp));
    }

    let endpoint = format!("/telemetry/{}?{}", category, query_params.join("&"));
    let api_client = get_api_client();
    let resp = api_client
        .get_with_auth(&endpoint)
        .await
        .map_err(|e| CommandError {
            code: "NETWORK_ERROR".into(),
            message: e,
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err_text = resp.text().await.unwrap_or_default();
        return Err(CommandError {
            code: format!("API_ERROR_{}", status.as_u16()),
            message: format!("Telemetry retrieval failed ({}): {}", status, err_text),
        });
    }

    let collection: CollectionResponse<ApiTelemetryRecord> =
        resp.json().await.map_err(|e| CommandError {
            code: "PARSE_ERROR".into(),
            message: format!("Failed to parse telemetry API response: {}", e),
        })?;

    let mut encrypted_dtos = Vec::with_capacity(collection.data.len());
    for item in collection.data {
        let payload = parse_encrypted_payload(&item.encrypted_payload)?;
        encrypted_dtos.push(EncryptedRecordDto {
            id: item.id,
            client_timestamp: item.client_timestamp,
            ephemeral_public_key: item.ephemeral_public_key,
            encrypted_payload: payload,
        });
    }

    let priv_key = load_admin_private_key().map_err(|e| CommandError {
        code: "KEYSTORE_ERROR".into(),
        message: format!("Keystore error: {}", e),
    })?;

    // Offload compute-intensive parallel decryption to worker thread pool
    let (decrypted, failed) = tokio::task::spawn_blocking(move || {
        decrypt_records_parallel_tolerant(&priv_key, encrypted_dtos)
    })
    .await
    .map_err(|e| CommandError {
        code: "THREAD_JOIN_ERROR".into(),
        message: e.to_string(),
    })?;

    tracing::info!(
        "Successfully decrypted {} records for worker {} in memory ({} failed)",
        decrypted.len(),
        worker_id,
        failed.len()
    );

    Ok(decrypted)
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

        // Prepare 100 mock encrypted records (DoD 1 / Section 8)
        let mut encrypted_records = Vec::new();
        for i in 0..100 {
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

        assert_eq!(decrypted.len(), 100);
        for (i, rec) in decrypted.iter().enumerate() {
            assert_eq!(rec.id, format!("rec-{}", i));
            assert_eq!(rec.payload["worker_id"], format!("worker-{}", i));
            assert_eq!(rec.payload["cpu_usage"], 20 + i as u64);
        }
    }

    #[test]
    fn test_parallel_decryption_1000_records() {
        let (admin_priv, admin_pub) = generate_admin_keypair();

        let mut encrypted_records = Vec::with_capacity(1000);
        for i in 0..1000 {
            let payload = json!({
                "worker_id": format!("worker-{}", i),
                "cpu_usage": 20 + (i % 80),
                "event": "HEARTBEAT"
            });
            let envelope = encrypt_telemetry_batch(&payload, &admin_pub).unwrap();
            encrypted_records.push(EncryptedRecordDto {
                id: format!("rec-{}", i),
                client_timestamp: Utc::now(),
                ephemeral_public_key: envelope.ephemeral_public_key,
                encrypted_payload: envelope.encrypted_payload,
            });
        }

        let start = std::time::Instant::now();
        let decrypted = decrypt_records_parallel(&admin_priv, encrypted_records)
            .expect("Parallel decryption of 1000 records failed");
        let elapsed = start.elapsed();

        assert_eq!(decrypted.len(), 1000);
        println!("Decrypted 1000 records in {:?}", elapsed);
    }

    #[test]
    fn test_tampered_record_in_batch() {
        let (admin_priv, admin_pub) = generate_admin_keypair();

        let mut encrypted_records = Vec::new();
        for i in 0..10 {
            let payload = json!({ "index": i });
            let mut envelope = encrypt_telemetry_batch(&payload, &admin_pub).unwrap();

            // Tamper record at index 5
            if i == 5 {
                let len = envelope.encrypted_payload.len();
                envelope.encrypted_payload[len - 1] ^= 0xFF;
            }

            encrypted_records.push(EncryptedRecordDto {
                id: format!("rec-{}", i),
                client_timestamp: Utc::now(),
                ephemeral_public_key: envelope.ephemeral_public_key,
                encrypted_payload: envelope.encrypted_payload,
            });
        }

        let (decrypted, failed) = decrypt_records_parallel_tolerant(&admin_priv, encrypted_records);

        // 9 succeeded, exactly 1 failed (rec-5)
        assert_eq!(decrypted.len(), 9);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0], "rec-5");
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
