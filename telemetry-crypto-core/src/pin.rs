//! Worker PIN policy validation and Argon2id verifier (SEC-19, SEC-20).
//!
//! The PIN is the primary identification method on the enrolled device (R38 art. 7 g).
//! It is verified locally against an Argon2id verifier stored in the OS keystore;
//! it is never transmitted to the server or stored in clear (SEC-19).

use crate::error::CryptoError;
use crate::rut::Rut;
use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use thiserror::Error;
use zeroize::Zeroizing;

/// Minimum PIN length (digits).
pub const MIN_PIN_LEN: usize = 6;
/// Maximum PIN length (digits).
pub const MAX_PIN_LEN: usize = 8;

/// Default Argon2id memory cost for PIN verifier: 19 MiB (19456 KiB).
pub const DEFAULT_PIN_ARGON2_MEMORY_KIB: u32 = 19_456;
/// Default Argon2id time cost (passes) for PIN verifier: 2.
pub const DEFAULT_PIN_ARGON2_TIME_COST: u32 = 2;
/// Default Argon2id parallelism for PIN verifier: 1 lane.
pub const DEFAULT_PIN_ARGON2_PARALLELISM: u32 = 1;
/// Length of the salt in bytes.
pub const PIN_SALT_LEN: usize = 16;
/// Length of the derived Argon2 hash in bytes.
pub const PIN_HASH_LEN: usize = 32;

/// Accepted parameter bounds for PIN verification.
pub const MIN_PIN_MEMORY_KIB: u32 = 8_192;
pub const MAX_PIN_MEMORY_KIB: u32 = 262_144;
pub const MIN_PIN_TIME_COST: u32 = 1;
pub const MAX_PIN_TIME_COST: u32 = 10;
pub const MIN_PIN_PARALLELISM: u32 = 1;
pub const MAX_PIN_PARALLELISM: u32 = 4;

/// Errors indicating why a PIN failed the security policy (SEC-20).
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum PinPolicyError {
    #[error("PIN must contain only digits")]
    NonDigits,
    #[error("PIN must have between 6 and 8 digits (too short)")]
    TooShort,
    #[error("PIN must have between 6 and 8 digits (too long)")]
    TooLong,
    #[error("PIN cannot have all digits equal")]
    AllDigitsEqual,
    #[error("PIN cannot be an ascending or descending sequence")]
    SequentialDigits,
    #[error("PIN cannot match or share a prefix/suffix of 6+ digits with your RUN")]
    MatchesWorkerRun,
}

/// Validates that a proposed PIN adheres to the security policy (SEC-20).
///
/// Rules:
/// - 6 to 8 ASCII digits.
/// - Not all digits identical (e.g. `111111`).
/// - Not a complete ascending or descending run (e.g. `123456`, `98765432`).
/// - Not equal to worker RUN body digits, nor a prefix/suffix of 6+ digits of it.
///
/// # Errors
/// Returns [`PinPolicyError`] describing the violation.
pub fn validate_pin(pin: &str, worker_run: Option<&str>) -> Result<(), PinPolicyError> {
    if !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(PinPolicyError::NonDigits);
    }

    if pin.len() < MIN_PIN_LEN {
        return Err(PinPolicyError::TooShort);
    }
    if pin.len() > MAX_PIN_LEN {
        return Err(PinPolicyError::TooLong);
    }

    let bytes = pin.as_bytes();

    // Check all digits equal
    let first = bytes[0];
    if bytes.iter().all(|&b| b == first) {
        return Err(PinPolicyError::AllDigitsEqual);
    }

    // Check monotonic ascending or descending run
    let is_ascending = bytes.windows(2).all(|w| w[1] == w[0] + 1);
    let is_descending = bytes.windows(2).all(|w| w[0] == w[1] + 1);
    if is_ascending || is_descending {
        return Err(PinPolicyError::SequentialDigits);
    }

    // Check against worker RUN
    if let Some(run_str) = worker_run {
        let body_digits = if let Ok(rut) = Rut::parse(run_str) {
            rut.body_digits()
        } else {
            // Fallback: extract ASCII digits before hyphen or all digits
            let (before_hyphen, _) = run_str.split_once('-').unwrap_or((run_str, ""));
            let clean: String = before_hyphen
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            clean
        };

        if !body_digits.is_empty() {
            // Reject if equal, or if PIN is a prefix or suffix of 6+ digits of RUN body
            if pin == body_digits || body_digits.starts_with(pin) || body_digits.ends_with(pin) {
                return Err(PinPolicyError::MatchesWorkerRun);
            }
        }
    }

    Ok(())
}

/// PHC-free JSON representation of the local PIN verifier stored in the OS keystore (SEC-19).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PinVerifier {
    /// Format version, always 1.
    pub v: u8,
    /// Base64 encoded salt (16 bytes).
    pub salt_b64: String,
    /// Argon2id memory cost in KiB.
    pub m_kib: u32,
    /// Argon2id time cost (passes).
    pub t: u32,
    /// Argon2id parallelism (lanes).
    pub p: u32,
    /// Base64 encoded derived hash (32 bytes).
    pub hash_b64: String,
}

impl PinVerifier {
    /// Derives a new [`PinVerifier`] from the worker PIN using Argon2id v19 (SEC-19).
    ///
    /// Parameters: `m = 19456 KiB`, `t = 2`, `p = 1`, random 16-byte salt, 32-byte hash.
    ///
    /// # Errors
    /// Returns [`CryptoError`] if Argon2 parameter initialization or hashing fails.
    pub fn create(pin: &Zeroizing<String>) -> Result<Self, CryptoError> {
        let mut salt = [0u8; PIN_SALT_LEN];
        OsRng.fill_bytes(&mut salt);

        let argon_params = Params::new(
            DEFAULT_PIN_ARGON2_MEMORY_KIB,
            DEFAULT_PIN_ARGON2_TIME_COST,
            DEFAULT_PIN_ARGON2_PARALLELISM,
            Some(PIN_HASH_LEN),
        )
        .map_err(|e| CryptoError::InsecureKdfParams(e.to_string()))?;

        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
        let mut hash = Zeroizing::new([0u8; PIN_HASH_LEN]);

        argon
            .hash_password_into(pin.as_bytes(), &salt, hash.as_mut())
            .map_err(|_| CryptoError::KeyDerivationFailed)?;

        Ok(Self {
            v: 1,
            salt_b64: BASE64_STANDARD.encode(salt),
            m_kib: DEFAULT_PIN_ARGON2_MEMORY_KIB,
            t: DEFAULT_PIN_ARGON2_TIME_COST,
            p: DEFAULT_PIN_ARGON2_PARALLELISM,
            hash_b64: BASE64_STANDARD.encode(hash.as_ref()),
        })
    }

    /// Verifies the candidate PIN in constant time using [`subtle::ConstantTimeEq`] (SEC-19).
    ///
    /// Parameters outside accepted bounds (`m: 8192..=262144, t: 1..=10, p: 1..=4, v == 1`)
    /// immediately return `false` without computing the derivation.
    #[must_use]
    pub fn verify(&self, pin: &Zeroizing<String>) -> bool {
        if self.v != 1 {
            return false;
        }

        if self.m_kib < MIN_PIN_MEMORY_KIB
            || self.m_kib > MAX_PIN_MEMORY_KIB
            || self.t < MIN_PIN_TIME_COST
            || self.t > MAX_PIN_TIME_COST
            || self.p < MIN_PIN_PARALLELISM
            || self.p > MAX_PIN_PARALLELISM
        {
            return false;
        }

        let salt = match BASE64_STANDARD.decode(&self.salt_b64) {
            Ok(s) if s.len() >= 16 && s.len() <= 64 => s,
            _ => return false,
        };

        let expected_hash = match BASE64_STANDARD.decode(&self.hash_b64) {
            Ok(h) if h.len() == PIN_HASH_LEN => h,
            _ => return false,
        };

        let argon_params = match Params::new(self.m_kib, self.t, self.p, Some(PIN_HASH_LEN)) {
            Ok(p) => p,
            Err(_) => return false,
        };

        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
        let mut derived = Zeroizing::new([0u8; PIN_HASH_LEN]);

        if argon
            .hash_password_into(pin.as_bytes(), &salt, derived.as_mut())
            .is_err()
        {
            return false;
        }

        bool::from(derived.as_slice().ct_eq(expected_hash.as_slice()))
    }

    /// Convenience verifier for string slices.
    #[must_use]
    pub fn verify_str(&self, pin: &str) -> bool {
        let z = Zeroizing::new(pin.to_string());
        self.verify(&z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_policy_rejects_runs_and_repeats() {
        assert_eq!(
            validate_pin("111111", None).unwrap_err(),
            PinPolicyError::AllDigitsEqual
        );
        assert_eq!(
            validate_pin("99999999", None).unwrap_err(),
            PinPolicyError::AllDigitsEqual
        );
        assert_eq!(
            validate_pin("123456", None).unwrap_err(),
            PinPolicyError::SequentialDigits
        );
        assert_eq!(
            validate_pin("98765432", None).unwrap_err(),
            PinPolicyError::SequentialDigits
        );
        assert_eq!(
            validate_pin("12345", None).unwrap_err(),
            PinPolicyError::TooShort
        );
        assert_eq!(
            validate_pin("123456789", None).unwrap_err(),
            PinPolicyError::TooLong
        );
        assert_eq!(
            validate_pin("123a56", None).unwrap_err(),
            PinPolicyError::NonDigits
        );

        // Valid PINs
        assert!(validate_pin("839201", None).is_ok());
        assert!(validate_pin("48192031", None).is_ok());
    }

    #[test]
    fn test_pin_policy_rejects_run_digits() {
        let worker_run = "12.345.678-5";
        // Prefix of 6 digits of 12345678
        assert_eq!(
            validate_pin("123456", Some(worker_run)).unwrap_err(),
            PinPolicyError::SequentialDigits // also sequential
        );

        // Non-sequential prefix/suffix
        let worker_run_2 = "94.827.135-0"; // body: 94827135
        assert_eq!(
            validate_pin("948271", Some(worker_run_2)).unwrap_err(),
            PinPolicyError::MatchesWorkerRun
        );
        assert_eq!(
            validate_pin("827135", Some(worker_run_2)).unwrap_err(),
            PinPolicyError::MatchesWorkerRun
        );
        assert_eq!(
            validate_pin("94827135", Some(worker_run_2)).unwrap_err(),
            PinPolicyError::MatchesWorkerRun
        );

        // Allowed: does not match prefix/suffix
        assert!(validate_pin("482713", Some(worker_run_2)).is_ok());
        assert!(validate_pin("839201", Some(worker_run_2)).is_ok());
    }

    #[test]
    fn test_pin_verifier_roundtrip() {
        let pin = Zeroizing::new("839201".to_string());
        let verifier = PinVerifier::create(&pin).expect("create verifier");
        assert!(verifier.verify(&pin));
        assert!(verifier.verify_str("839201"));

        // JSON roundtrip
        let json = serde_json::to_string(&verifier).expect("serialize verifier");
        let deserialized: PinVerifier = serde_json::from_str(&json).expect("deserialize verifier");
        assert_eq!(verifier, deserialized);
        assert!(deserialized.verify(&pin));
    }

    #[test]
    fn test_pin_verifier_wrong_pin() {
        let pin = Zeroizing::new("839201".to_string());
        let verifier = PinVerifier::create(&pin).expect("create verifier");
        let wrong_pin = Zeroizing::new("839202".to_string());
        assert!(!verifier.verify(&wrong_pin));
        assert!(!verifier.verify_str("839202"));
    }

    #[test]
    fn test_pin_verifier_rejects_out_of_range_params() {
        let pin = Zeroizing::new("839201".to_string());
        let mut verifier = PinVerifier::create(&pin).expect("create verifier");

        // Out of range m_kib (too low)
        verifier.m_kib = 4096;
        assert!(!verifier.verify(&pin));

        // Out of range m_kib (too high)
        verifier.m_kib = 300_000;
        assert!(!verifier.verify(&pin));

        // Restore m_kib, alter time_cost
        verifier.m_kib = DEFAULT_PIN_ARGON2_MEMORY_KIB;
        verifier.t = 0;
        assert!(!verifier.verify(&pin));

        verifier.t = 11;
        assert!(!verifier.verify(&pin));

        // Alter parallelism
        verifier.t = DEFAULT_PIN_ARGON2_TIME_COST;
        verifier.p = 5;
        assert!(!verifier.verify(&pin));

        // Alter version
        verifier.p = DEFAULT_PIN_ARGON2_PARALLELISM;
        verifier.v = 2;
        assert!(!verifier.verify(&pin));
    }
}
