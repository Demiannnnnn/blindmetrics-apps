//! Canonical attendance mark hash and chain verification (ATT-03, R38 art. 8).
//!
//! Enforces exact canonical field order, domain-separated SHA-256 hashing,
//! and cryptographic chain verification for Chilean labour compliance.

use crate::error::CryptoError;
use crate::rut::Rut;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Domain separation prefix for attendance mark hashing (b"telemetry-mark-v1\0").
pub const ATTENDANCE_MARK_HASH_DOMAIN: &[u8] = b"telemetry-mark-v1\0";

/// Genesis previous hash for the first mark of a worker (64 zeros).
pub const GENESIS_PREV_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Canonical attendance mark types (ATT-01, R38 art. 35).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarkType {
    WorkdayStart,
    WorkdayEnd,
    BreakStart,
    BreakEnd,
}

/// Source origin of the attendance mark (ATT-01, ATT-05).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarkSource {
    Agent,
    Portal,
    Correction,
}

/// Geographic coordinates captured at the moment of the mark (ATT-08, R38 art. 53).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MarkLocation {
    pub lat: f64,
    pub lon: f64,
    pub accuracy_m: f64,
}

/// Canonical representation of an attendance mark for hashing and chain verification (ATT-03).
///
/// **Field order is strictly frozen** to match `security-model.md` §9.3 and `telemetry-api`:
/// 1. `v`
/// 2. `mark_id`
/// 3. `worker_id`
/// 4. `worker_run`
/// 5. `employer_rut`
/// 6. `type` (`r#type`)
/// 7. `server_ts`
/// 8. `device_ts`
/// 9. `offline`
/// 10. `source`
/// 11. `workplace_id`
/// 12. `location`
/// 13. `prev_hash`
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CanonicalMark {
    pub v: u8,
    pub mark_id: String,
    pub worker_id: String,
    pub worker_run: String,
    pub employer_rut: String,
    #[serde(rename = "type")]
    pub r#type: MarkType,
    pub server_ts: String,
    pub device_ts: String,
    pub offline: bool,
    pub source: MarkSource,
    pub workplace_id: Option<String>,
    pub location: Option<MarkLocation>,
    pub prev_hash: String,
}

impl CanonicalMark {
    /// Validates the canonical mark fields before hashing (RF-CRYPTO-04-3).
    ///
    /// # Errors
    /// Returns [`CryptoError::InvalidInput`] if any field fails structural or format validation.
    pub fn validate(&self) -> Result<(), CryptoError> {
        if self.v != 1 {
            return Err(CryptoError::InvalidInput(format!(
                "Canonical mark version must be 1, got {}",
                self.v
            )));
        }

        // Validate timestamps: RFC 3339 UTC with second precision (YYYY-MM-DDTHH:MM:SSZ)
        validate_rfc3339_second_precision(&self.server_ts, "server_ts")?;
        validate_rfc3339_second_precision(&self.device_ts, "device_ts")?;

        // Validate prev_hash: 64 lowercase hex characters
        if self.prev_hash.len() != 64
            || !self
                .prev_hash
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        {
            return Err(CryptoError::InvalidInput(
                "prev_hash must be exactly 64 lowercase hex characters".to_string(),
            ));
        }

        // Validate worker_run: canonical RUT format
        let worker_rut = Rut::parse(&self.worker_run)
            .map_err(|e| CryptoError::InvalidInput(format!("Invalid worker_run: {}", e)))?;
        if worker_rut.format() != self.worker_run {
            return Err(CryptoError::InvalidInput(format!(
                "worker_run '{}' is not in canonical format '{}'",
                self.worker_run,
                worker_rut.format()
            )));
        }

        // Validate employer_rut: canonical RUT format
        let employer_rut = Rut::parse(&self.employer_rut)
            .map_err(|e| CryptoError::InvalidInput(format!("Invalid employer_rut: {}", e)))?;
        if employer_rut.format() != self.employer_rut {
            return Err(CryptoError::InvalidInput(format!(
                "employer_rut '{}' is not in canonical format '{}'",
                self.employer_rut,
                employer_rut.format()
            )));
        }

        // Validate location if present
        if let Some(loc) = &self.location {
            if !loc.lat.is_finite() || loc.lat < -90.0 || loc.lat > 90.0 {
                return Err(CryptoError::InvalidInput(format!(
                    "location.lat must be in [-90, 90], got {}",
                    loc.lat
                )));
            }
            if !loc.lon.is_finite() || loc.lon < -180.0 || loc.lon > 180.0 {
                return Err(CryptoError::InvalidInput(format!(
                    "location.lon must be in [-180, 180], got {}",
                    loc.lon
                )));
            }
            if !loc.accuracy_m.is_finite() || loc.accuracy_m < 0.0 {
                return Err(CryptoError::InvalidInput(format!(
                    "location.accuracy_m must be non-negative, got {}",
                    loc.accuracy_m
                )));
            }
        }

        Ok(())
    }
}

fn validate_rfc3339_second_precision(ts: &str, field_name: &str) -> Result<(), CryptoError> {
    if ts.len() != 20 || !ts.ends_with('Z') {
        return Err(CryptoError::InvalidInput(format!(
            "{} '{}' is not RFC 3339 UTC with second precision (expected YYYY-MM-DDTHH:MM:SSZ)",
            field_name, ts
        )));
    }
    NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%SZ").map_err(|_| {
        CryptoError::InvalidInput(format!(
            "{} '{}' failed RFC 3339 date/time parsing",
            field_name, ts
        ))
    })?;
    Ok(())
}

/// Computes the SHA-256 hash (in lowercase hex) of the canonical attendance mark (ATT-03).
///
/// `hash = hex(SHA-256(b"telemetry-mark-v1\0" ‖ serde_json::to_vec(m)))`
///
/// # Errors
/// Returns [`CryptoError::InvalidInput`] if structural validation fails, or
/// [`CryptoError::SerializationError`] if serialization fails.
pub fn mark_hash(m: &CanonicalMark) -> Result<String, CryptoError> {
    m.validate()?;
    let json_bytes = serde_json::to_vec(m)?;

    let mut hasher = Sha256::new();
    hasher.update(ATTENDANCE_MARK_HASH_DOMAIN);
    hasher.update(&json_bytes);
    let digest = hasher.finalize();

    Ok(hex::encode(digest))
}

/// Verifies a mark's hash in constant time using [`subtle::ConstantTimeEq`] (ATT-03, SEC-19).
#[must_use]
pub fn verify_mark_hash(m: &CanonicalMark, expected_hex: &str) -> bool {
    let computed = match mark_hash(m) {
        Ok(h) => h,
        Err(_) => return false,
    };

    let expected = expected_hex.trim().to_ascii_lowercase();
    if expected.len() != 64 {
        return false;
    }

    bool::from(computed.as_bytes().ct_eq(expected.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VECTOR_FILE: &str = include_str!("../tests/vectors/attendance-mark-v1.json");

    #[derive(Deserialize)]
    struct VectorItem {
        mark: CanonicalMark,
        canonical_json: String,
        hash: String,
    }

    #[derive(Deserialize)]
    struct VectorDocument {
        vectors: Vec<VectorItem>,
    }

    #[test]
    fn test_attendance_mark_hash_matches_vector() {
        let doc: VectorDocument = serde_json::from_str(VECTOR_FILE).expect("valid vector json");
        assert!(doc.vectors.len() >= 3);

        for item in &doc.vectors {
            // Check canonical serialization matches expected compact JSON byte-for-byte
            let serialized = serde_json::to_string(&item.mark).expect("serialize mark");
            assert_eq!(serialized, item.canonical_json);

            // Check mark_hash computes exact expected hash
            let computed_hash = mark_hash(&item.mark).expect("mark_hash succeeds");
            assert_eq!(computed_hash, item.hash);

            // Check verify_mark_hash is constant-time true
            assert!(verify_mark_hash(&item.mark, &item.hash));

            // Check verify_mark_hash rejects tampered hash
            let wrong_hash = "f".repeat(64);
            assert!(!verify_mark_hash(&item.mark, &wrong_hash));
        }
    }

    #[test]
    fn test_attendance_chain_prev_hash() {
        let doc: VectorDocument = serde_json::from_str(VECTOR_FILE).expect("valid vector json");
        assert_eq!(doc.vectors[0].mark.prev_hash, GENESIS_PREV_HASH);
        assert_eq!(doc.vectors[1].mark.prev_hash, doc.vectors[0].hash);
        assert_eq!(doc.vectors[2].mark.prev_hash, doc.vectors[1].hash);

        // Tampering mark 1 breaks the chain
        let mut tampered_mark_1 = doc.vectors[0].mark.clone();
        tampered_mark_1.offline = !tampered_mark_1.offline;
        let tampered_hash = mark_hash(&tampered_mark_1).expect("hash tampered");
        assert_ne!(tampered_hash, doc.vectors[1].mark.prev_hash);
    }

    #[test]
    fn test_attendance_rejects_subsecond_timestamp() {
        let doc: VectorDocument = serde_json::from_str(VECTOR_FILE).expect("valid vector json");
        let mut mark = doc.vectors[0].mark.clone();

        // Milliseconds timestamp
        mark.server_ts = "2026-10-11T08:59:58.123Z".to_string();
        assert!(matches!(
            mark_hash(&mark).unwrap_err(),
            CryptoError::InvalidInput(_)
        ));

        // Trailing zeros subseconds
        mark.server_ts = "2026-10-11T08:59:58.000Z".to_string();
        assert!(matches!(
            mark_hash(&mark).unwrap_err(),
            CryptoError::InvalidInput(_)
        ));

        // Non-UTC offset
        mark.server_ts = "2026-10-11T08:59:58+00:00".to_string();
        assert!(matches!(
            mark_hash(&mark).unwrap_err(),
            CryptoError::InvalidInput(_)
        ));
    }

    #[test]
    fn test_attendance_field_order_is_canonical() {
        let doc: VectorDocument = serde_json::from_str(VECTOR_FILE).expect("valid vector json");
        let mark = &doc.vectors[0].mark;
        let json = serde_json::to_string(mark).expect("serialize");

        // Verify keys appear in the exact required order
        let expected_keys = [
            "\"v\":",
            "\"mark_id\":",
            "\"worker_id\":",
            "\"worker_run\":",
            "\"employer_rut\":",
            "\"type\":",
            "\"server_ts\":",
            "\"device_ts\":",
            "\"offline\":",
            "\"source\":",
            "\"workplace_id\":",
            "\"location\":",
            "\"prev_hash\":",
        ];

        let mut last_pos = 0;
        for key in expected_keys {
            let pos = json[last_pos..]
                .find(key)
                .unwrap_or_else(|| panic!("Key {} missing in JSON", key));
            last_pos += pos + key.len();
        }
    }

    #[test]
    fn test_attendance_rejects_invalid_inputs() {
        let doc: VectorDocument = serde_json::from_str(VECTOR_FILE).expect("valid vector json");
        let mut mark = doc.vectors[0].mark.clone();

        // Invalid version
        mark.v = 2;
        assert!(matches!(
            mark_hash(&mark).unwrap_err(),
            CryptoError::InvalidInput(_)
        ));
        mark.v = 1;

        // Invalid RUT format (unformatted)
        mark.worker_run = "12345678-5".to_string();
        assert!(matches!(
            mark_hash(&mark).unwrap_err(),
            CryptoError::InvalidInput(_)
        ));
        mark.worker_run = "12.345.678-5".to_string();

        // Invalid prev_hash (uppercase or too short)
        mark.prev_hash = "A".repeat(64);
        assert!(matches!(
            mark_hash(&mark).unwrap_err(),
            CryptoError::InvalidInput(_)
        ));
        mark.prev_hash = "0".repeat(63);
        assert!(matches!(
            mark_hash(&mark).unwrap_err(),
            CryptoError::InvalidInput(_)
        ));
        mark.prev_hash = GENESIS_PREV_HASH.to_string();

        // Invalid location bounds
        mark.location = Some(MarkLocation {
            lat: 95.0,
            lon: -70.0,
            accuracy_m: 10.0,
        });
        assert!(matches!(
            mark_hash(&mark).unwrap_err(),
            CryptoError::InvalidInput(_)
        ));

        mark.location = Some(MarkLocation {
            lat: -33.0,
            lon: -190.0,
            accuracy_m: 10.0,
        });
        assert!(matches!(
            mark_hash(&mark).unwrap_err(),
            CryptoError::InvalidInput(_)
        ));

        mark.location = Some(MarkLocation {
            lat: -33.0,
            lon: -70.0,
            accuracy_m: -1.0,
        });
        assert!(matches!(
            mark_hash(&mark).unwrap_err(),
            CryptoError::InvalidInput(_)
        ));
    }
}
