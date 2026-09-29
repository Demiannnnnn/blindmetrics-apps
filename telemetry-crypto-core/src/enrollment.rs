use crate::asymmetric::decode_public_key;
use crate::error::CryptoError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const ENROLLMENT_MANIFEST_VERSION: u8 = 1;
pub const ENROLLMENT_QR_TYPE: &str = "telemetry-enroll";
pub const ENROLLMENT_QR_VERSION: u8 = 1;
const COMMITMENT_DOMAIN: &[u8] = b"telemetry-enroll-v1\0";

/// Explicit boolean consent matrix configured by Admin.
/// Canonical field ordering is strictly required for commitment hash consistency.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsentFlags {
    pub system_activity: bool,
    pub network_activity: bool,
    pub productivity_basic: bool,
    pub productivity_screenshots: bool,
    pub file_activity: bool,
    pub location: bool,
}

/// Admin-defined explanations of why each category is collected.
/// Canonical field ordering is strictly required for commitment hash consistency.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsentDescriptions {
    pub system_activity: Option<String>,
    pub network_activity: Option<String>,
    pub productivity_basic: Option<String>,
    pub productivity_screenshots: Option<String>,
    pub file_activity: Option<String>,
    pub location: Option<String>,
}

impl ConsentDescriptions {
    /// Normalizes descriptions by dropping descriptions for any disabled category.
    pub fn retain_enabled(&mut self, flags: &ConsentFlags) {
        if !flags.system_activity {
            self.system_activity = None;
        }
        if !flags.network_activity {
            self.network_activity = None;
        }
        if !flags.productivity_basic {
            self.productivity_basic = None;
        }
        if !flags.productivity_screenshots {
            self.productivity_screenshots = None;
        }
        if !flags.file_activity {
            self.file_activity = None;
        }
        if !flags.location {
            self.location = None;
        }
    }
}

/// Canonical enrollment manifest binding administrative key and consent parameters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnrollmentManifest {
    pub v: u8,
    pub admin_public_key: String,
    pub consent_version: i32,
    pub consent_flags: ConsentFlags,
    pub consent_descriptions: ConsentDescriptions,
}

impl EnrollmentManifest {
    /// Constructs and normalizes a new enrollment manifest.
    /// Rejects invalid public keys or consent versions < 1.
    pub fn new(
        admin_public_key: String,
        consent_version: i32,
        consent_flags: ConsentFlags,
        mut consent_descriptions: ConsentDescriptions,
    ) -> Result<Self, CryptoError> {
        if consent_version < 1 {
            return Err(CryptoError::InvalidConsentVersion);
        }
        decode_public_key(&admin_public_key)?;
        consent_descriptions.retain_enabled(&consent_flags);

        Ok(Self {
            v: ENROLLMENT_MANIFEST_VERSION,
            admin_public_key,
            consent_version,
            consent_flags,
            consent_descriptions,
        })
    }
}

/// Computes the SHA-256 cryptographic commitment over the canonical manifest JSON.
/// Domain separation: `SHA-256(b"telemetry-enroll-v1\0" || JSON)`.
pub fn manifest_commitment(manifest: &EnrollmentManifest) -> Result<String, CryptoError> {
    let json =
        serde_json::to_vec(manifest).map_err(|e| CryptoError::Serialization(e.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(COMMITMENT_DOMAIN);
    hasher.update(&json);
    Ok(hex::encode(hasher.finalize()))
}

/// Verifies whether a manifest produces the expected commitment in constant time.
pub fn verify_commitment(manifest: &EnrollmentManifest, expected_hex: &str) -> bool {
    let computed = match manifest_commitment(manifest) {
        Ok(c) => c,
        Err(_) => return false,
    };

    if computed.len() != expected_hex.len() {
        return false;
    }

    computed.as_bytes().ct_eq(expected_hex.as_bytes()).into()
}

/// Computes a human-readable 12-digit safety code from the commitment hash.
/// Formatted as "dddd dddd dddd".
pub fn safety_code(commitment_hex: &str) -> Result<String, CryptoError> {
    if commitment_hex.len() != 64 {
        return Err(CryptoError::InvalidCommitment);
    }
    let bytes = hex::decode(commitment_hex).map_err(|_| CryptoError::InvalidCommitment)?;
    let head: [u8; 8] = bytes[0..8]
        .try_into()
        .map_err(|_| CryptoError::InvalidCommitment)?;

    let num = u64::from_be_bytes(head) % 1_000_000_000_000;
    let s = format!("{:012}", num);
    Ok(format!("{} {} {}", &s[0..4], &s[4..8], &s[8..12]))
}

/// Computes the public key fingerprint matching the server representation.
/// `SHA-256(32 raw decoded bytes)` encoded as 64 lowercase hex characters.
pub fn public_key_fingerprint(admin_public_key_b64: &str) -> Result<String, CryptoError> {
    let pub_key = decode_public_key(admin_public_key_b64)?;
    let mut hasher = Sha256::new();
    hasher.update(pub_key.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

/// QR payload container scanned by the worker during physical enrollment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnrollmentQr {
    pub t: String,
    pub v: u8,
    pub api_url: String,
    pub code: String,
    pub commitment: String,
}

impl EnrollmentQr {
    pub fn new(api_url: String, code: String, commitment: String) -> Result<Self, CryptoError> {
        Self::validate_fields(
            ENROLLMENT_QR_TYPE,
            ENROLLMENT_QR_VERSION,
            &code,
            &commitment,
        )?;
        Ok(Self {
            t: ENROLLMENT_QR_TYPE.to_string(),
            v: ENROLLMENT_QR_VERSION,
            api_url,
            code,
            commitment,
        })
    }

    pub fn to_json(&self) -> Result<String, CryptoError> {
        serde_json::to_string(self).map_err(|e| CryptoError::Serialization(e.to_string()))
    }

    pub fn parse(json_str: &str) -> Result<Self, CryptoError> {
        let qr: Self = serde_json::from_str(json_str)
            .map_err(|e| CryptoError::InvalidQr(format!("JSON parsing error: {}", e)))?;

        Self::validate_fields(&qr.t, qr.v, &qr.code, &qr.commitment)?;
        Ok(qr)
    }

    fn validate_fields(t: &str, v: u8, code: &str, commitment: &str) -> Result<(), CryptoError> {
        if t != ENROLLMENT_QR_TYPE {
            return Err(CryptoError::InvalidQr(format!("Unsupported type: {}", t)));
        }
        if v != ENROLLMENT_QR_VERSION {
            return Err(CryptoError::InvalidQr(format!(
                "Unsupported version: {}",
                v
            )));
        }
        if commitment.len() != 64 || !commitment.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CryptoError::InvalidCommitment);
        }

        // Validate code format: 3 segments of 4 base32 characters separated by hyphen
        // Allowed charset: 0-9, A-Z excluding I, L, O, U
        let parts: Vec<&str> = code.split('-').collect();
        if parts.len() != 3 {
            return Err(CryptoError::InvalidQr(
                "Invalid code segment structure".into(),
            ));
        }

        for part in parts {
            if part.len() != 4 {
                return Err(CryptoError::InvalidQr("Invalid code segment length".into()));
            }
            for ch in part.chars() {
                let valid = matches!(ch, '0'..='9' | 'A'..='H' | 'J'..='K' | 'M'..='N' | 'P'..='T' | 'V'..='Z');
                if !valid {
                    return Err(CryptoError::InvalidQr(format!(
                        "Invalid character in code: {}",
                        ch
                    )));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asymmetric::generate_admin_keypair;
    use base64::prelude::*;

    #[test]
    fn test_enrollment_commitment_fixed_vector() {
        // Fixed test vector: key = [9u8; 32], version = 1, system+network flags, descriptions "a..."
        let raw_key = [9u8; 32];
        let key_b64 = BASE64_STANDARD.encode(raw_key);

        let flags = ConsentFlags {
            system_activity: true,
            network_activity: true,
            productivity_basic: false,
            productivity_screenshots: false,
            file_activity: false,
            location: false,
        };

        let descriptions = ConsentDescriptions {
            system_activity: Some("System activity monitoring".into()),
            network_activity: Some("Network domain monitoring".into()),
            productivity_basic: None,
            productivity_screenshots: None,
            file_activity: None,
            location: None,
        };

        let manifest = EnrollmentManifest::new(key_b64, 1, flags, descriptions)
            .expect("Failed to build manifest");

        let commitment = manifest_commitment(&manifest).expect("Failed to compute commitment");
        const EXPECTED_HEX: &str = "472eb081f962210500e25b6fe56327e86e8596cdbbb8c839265b7a50d1202bd1";

        // Verify deterministic commitment against canonical protocol hash
        assert_eq!(commitment, EXPECTED_HEX);
        assert!(verify_commitment(&manifest, EXPECTED_HEX));

        // Fixed vector cross-check:
        let expected = manifest_commitment(&manifest).unwrap();
        assert_eq!(commitment, expected);
    }

    #[test]
    fn test_enrollment_commitment_changes_when_key_changes() {
        let (_, key1) = generate_admin_keypair();
        let (_, key2) = generate_admin_keypair();

        let flags = ConsentFlags::default();
        let desc = ConsentDescriptions::default();

        let m1 = EnrollmentManifest::new(key1, 1, flags, desc.clone()).unwrap();
        let m2 = EnrollmentManifest::new(key2, 1, flags, desc).unwrap();

        assert_ne!(
            manifest_commitment(&m1).unwrap(),
            manifest_commitment(&m2).unwrap()
        );
    }

    #[test]
    fn test_enrollment_commitment_changes_when_any_flag_or_description_changes() {
        let (_, key) = generate_admin_keypair();
        let base_flags = ConsentFlags::default();
        let base_desc = ConsentDescriptions::default();
        let base_manifest = EnrollmentManifest::new(key.clone(), 1, base_flags, base_desc).unwrap();
        let base_commitment = manifest_commitment(&base_manifest).unwrap();

        // 1. System flag change
        let mut f1 = base_flags;
        f1.system_activity = true;
        let d1 = ConsentDescriptions {
            system_activity: Some("desc".into()),
            ..Default::default()
        };
        let m1 = EnrollmentManifest::new(key.clone(), 1, f1, d1).unwrap();
        assert_ne!(base_commitment, manifest_commitment(&m1).unwrap());

        // 2. Network flag change
        let mut f2 = base_flags;
        f2.network_activity = true;
        let d2 = ConsentDescriptions {
            network_activity: Some("desc".into()),
            ..Default::default()
        };
        let m2 = EnrollmentManifest::new(key.clone(), 1, f2, d2).unwrap();
        assert_ne!(base_commitment, manifest_commitment(&m2).unwrap());

        // 3. Productivity basic flag change
        let mut f3 = base_flags;
        f3.productivity_basic = true;
        let d3 = ConsentDescriptions {
            productivity_basic: Some("desc".into()),
            ..Default::default()
        };
        let m3 = EnrollmentManifest::new(key.clone(), 1, f3, d3).unwrap();
        assert_ne!(base_commitment, manifest_commitment(&m3).unwrap());

        // 4. Productivity screenshot flag change
        let mut f4 = base_flags;
        f4.productivity_screenshots = true;
        let d4 = ConsentDescriptions {
            productivity_screenshots: Some("desc".into()),
            ..Default::default()
        };
        let m4 = EnrollmentManifest::new(key.clone(), 1, f4, d4).unwrap();
        assert_ne!(base_commitment, manifest_commitment(&m4).unwrap());

        // 5. File activity flag change
        let mut f5 = base_flags;
        f5.file_activity = true;
        let d5 = ConsentDescriptions {
            file_activity: Some("desc".into()),
            ..Default::default()
        };
        let m5 = EnrollmentManifest::new(key.clone(), 1, f5, d5).unwrap();
        assert_ne!(base_commitment, manifest_commitment(&m5).unwrap());

        // 6. Location flag change
        let mut f6 = base_flags;
        f6.location = true;
        let d6 = ConsentDescriptions {
            location: Some("desc".into()),
            ..Default::default()
        };
        let m6 = EnrollmentManifest::new(key, 1, f6, d6).unwrap();
        assert_ne!(base_commitment, manifest_commitment(&m6).unwrap());
    }

    #[test]
    fn test_enrollment_disabled_category_description_is_ignored() {
        let (_, key) = generate_admin_keypair();

        let flags = ConsentFlags {
            system_activity: true,
            network_activity: false,
            ..Default::default()
        };

        // Manifest 1: network has None description
        let desc1 = ConsentDescriptions {
            system_activity: Some("System desc".into()),
            network_activity: None,
            ..Default::default()
        };

        // Manifest 2: network has Some description but its flag is false
        let desc2 = ConsentDescriptions {
            system_activity: Some("System desc".into()),
            network_activity: Some("Ghost network description".into()),
            ..Default::default()
        };

        let m1 = EnrollmentManifest::new(key.clone(), 1, flags, desc1).unwrap();
        let m2 = EnrollmentManifest::new(key, 1, flags, desc2).unwrap();

        // Both manifests must normalize to identical commitments
        assert_eq!(
            manifest_commitment(&m1).unwrap(),
            manifest_commitment(&m2).unwrap()
        );
    }

    #[test]
    fn test_enrollment_safety_code_format_and_determinism() {
        let commitment = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let code1 = safety_code(commitment).expect("Valid safety code");
        let code2 = safety_code(commitment).expect("Valid safety code");

        assert_eq!(code1, code2);
        assert_eq!(code1.len(), 14); // 4 + 1 + 4 + 1 + 4
        let parts: Vec<&str> = code1.split(' ').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].len(), 4);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert!(parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())));
    }

    #[test]
    fn test_enrollment_fingerprint_matches_sha256_of_raw_key() {
        let raw_key = [7u8; 32];
        let key_b64 = BASE64_STANDARD.encode(raw_key);

        let fp = public_key_fingerprint(&key_b64).expect("Fingerprint calculation failed");
        let expected = hex::encode(Sha256::digest(raw_key));
        assert_eq!(fp, expected);
        assert_eq!(fp.len(), 64);
    }

    #[test]
    fn test_enrollment_qr_roundtrip_and_rejects_bad_type_version_code() {
        let commitment = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        let valid_qr = EnrollmentQr::new(
            "https://api.telemetry.local".into(),
            "K7QM-4XRT-9PWD".into(),
            commitment.into(),
        )
        .expect("Valid QR creation");

        let json = valid_qr.to_json().expect("Serialization failed");
        let parsed = EnrollmentQr::parse(&json).expect("Parsing failed");
        assert_eq!(valid_qr, parsed);

        // Reject bad type
        let bad_type = serde_json::json!({
            "t": "wrong-type",
            "v": 1,
            "api_url": "https://api.local",
            "code": "K7QM-4XRT-9PWD",
            "commitment": commitment
        })
        .to_string();
        assert!(EnrollmentQr::parse(&bad_type).is_err());

        // Reject bad version
        let bad_version = serde_json::json!({
            "t": "telemetry-enroll",
            "v": 99,
            "api_url": "https://api.local",
            "code": "K7QM-4XRT-9PWD",
            "commitment": commitment
        })
        .to_string();
        assert!(EnrollmentQr::parse(&bad_version).is_err());

        // Reject bad code (contains invalid character 'I' or 'O')
        let bad_code = serde_json::json!({
            "t": "telemetry-enroll",
            "v": 1,
            "api_url": "https://api.local",
            "code": "K7QI-4XRT-9PWD",
            "commitment": commitment
        })
        .to_string();
        assert!(EnrollmentQr::parse(&bad_code).is_err());
    }

    #[test]
    fn test_enrollment_invalid_public_key_rejected() {
        let invalid_key = "not_a_valid_base64_key";
        let res = EnrollmentManifest::new(
            invalid_key.into(),
            1,
            ConsentFlags::default(),
            ConsentDescriptions::default(),
        );
        assert!(res.is_err());
    }
}
