//! Key backup format v2 for `K_admin_priv` (TASK-CRYPTO-03).
//!
//! One format for three places: the `.tkb` file the admin downloads (desktop app or web portal),
//! the zero-knowledge vault of the API (`POST /admins/me/vault/setup`) and the web onboarding
//! (`telemetry-web`, WebCrypto + Argon2id WASM).
//!
//! ```text
//! password = NFC(Master Password)                      (never trimmed, 12+ chars, ≤ 1024 bytes)
//! KEK      = Argon2id(v=0x13, m, t, p, password, salt) → 32 bytes
//! blob     = nonce(12) ‖ tag(16) ‖ AES-256-GCM(KEK, nonce, raw 32-byte key, AAD = "telemetry-key-backup-v2")
//! ```
//!
//! The Master Password and the KEK never leave the client (SEC-12): the server stores only the
//! blob, the Argon2id parameters and the public-key fingerprint. Every secret lives in
//! `Zeroizing<…>` (SEC-03).

use crate::asymmetric::{decode_private_key, derive_public_key};
use crate::enrollment::public_key_fingerprint;
use crate::error::CryptoError;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use chrono::{DateTime, NaiveDate, Utc};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

/// `format` of the `.tkb` container.
pub const KEY_BACKUP_FORMAT: &str = "telemetry-key-backup";
/// `version` of the `.tkb` container.
pub const KEY_BACKUP_VERSION: u8 = 2;
/// AES-GCM associated data: separates key backups from telemetry envelopes.
pub const KEY_BACKUP_AAD: &[u8] = b"telemetry-key-backup-v2";
/// File extension of v2 backups.
pub const KEY_BACKUP_EXTENSION: &str = "tkb";

/// Only Argon2id is accepted.
pub const ARGON2_ALGORITHM: &str = "argon2id";
/// Argon2 version 0x13.
pub const ARGON2_VERSION: u32 = 19;
/// Default memory cost in KiB (64 MiB).
pub const DEFAULT_ARGON2_MEMORY_KIB: u32 = 65_536;
/// Default number of passes.
pub const DEFAULT_ARGON2_TIME_COST: u32 = 3;
/// Default lanes.
pub const DEFAULT_ARGON2_PARALLELISM: u32 = 4;
/// Length of the random salt of new backups.
pub const DEFAULT_SALT_LEN: usize = 16;

/// Accepted ranges — identical to the API (`telemetry-api` TASK-16) so that a backup created
/// anywhere can be restored anywhere, and so that a malicious file or server cannot force a
/// derivation that exhausts memory or time.
pub const MIN_ARGON2_MEMORY_KIB: u32 = 32_768;
pub const MAX_ARGON2_MEMORY_KIB: u32 = 1_048_576;
pub const MIN_ARGON2_TIME_COST: u32 = 2;
pub const MAX_ARGON2_TIME_COST: u32 = 20;
pub const MIN_ARGON2_PARALLELISM: u32 = 1;
pub const MAX_ARGON2_PARALLELISM: u32 = 16;
pub const MIN_SALT_LEN: usize = 16;
pub const MAX_SALT_LEN: usize = 64;

/// Minimum Master Password length in Unicode scalar values.
pub const MIN_MASTER_PASSWORD_LEN: usize = 12;
/// Maximum Master Password length in UTF-8 bytes (after NFC normalization).
pub const MAX_MASTER_PASSWORD_BYTES: usize = 1024;
/// Decoded length of `encrypted_private_key`: nonce 12 + tag 16 + key 32.
pub const ENCRYPTED_PRIVATE_KEY_LEN: usize = 60;
/// Largest backup file accepted by [`import_any_backup`] and [`KeyBackupFile::parse`].
pub const MAX_BACKUP_FILE_BYTES: usize = 64 * 1024;

const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const KEY_LEN: usize = 32;

fn default_algorithm() -> String {
    ARGON2_ALGORITHM.to_string()
}

fn default_version() -> u32 {
    ARGON2_VERSION
}

/// Argon2id parameters, with the same JSON shape as `argon2_params` of the API and `kdf` of
/// the `.tkb` container. `algorithm` and `version` may be omitted (defaults) but are always
/// written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Argon2Params {
    #[serde(default = "default_algorithm")]
    pub algorithm: String,
    #[serde(default = "default_version")]
    pub version: u32,
    /// Memory cost in KiB.
    pub memory_cost: u32,
    /// Number of passes.
    pub time_cost: u32,
    /// Lanes.
    pub parallelism: u32,
    /// Standard base64 (with padding) of 16–64 random bytes.
    pub salt: String,
}

impl Argon2Params {
    /// Default parameters with a fresh random salt (one per backup).
    pub fn generate_default() -> Self {
        let mut salt = [0u8; DEFAULT_SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        Self::with_salt(
            DEFAULT_ARGON2_MEMORY_KIB,
            DEFAULT_ARGON2_TIME_COST,
            DEFAULT_ARGON2_PARALLELISM,
            &salt,
        )
    }

    /// Builds parameters from explicit costs and salt bytes (tests, reference vectors).
    pub fn with_salt(memory_cost: u32, time_cost: u32, parallelism: u32, salt: &[u8]) -> Self {
        Self {
            algorithm: default_algorithm(),
            version: ARGON2_VERSION,
            memory_cost,
            time_cost,
            parallelism,
            salt: BASE64_STANDARD.encode(salt),
        }
    }

    /// Checks every parameter against the accepted ranges and returns the decoded salt.
    ///
    /// Runs **before** any derivation (TASK-CRYPTO-03 §4.3).
    ///
    /// # Errors
    /// [`CryptoError::InsecureKdfParams`] naming the offending field.
    pub fn validate(&self) -> Result<Vec<u8>, CryptoError> {
        if !self.algorithm.eq_ignore_ascii_case(ARGON2_ALGORITHM) {
            return Err(insecure("algorithm must be argon2id"));
        }
        if self.version != ARGON2_VERSION {
            return Err(insecure("version must be 19"));
        }
        if !(MIN_ARGON2_MEMORY_KIB..=MAX_ARGON2_MEMORY_KIB).contains(&self.memory_cost) {
            return Err(insecure("memory_cost out of range"));
        }
        if !(MIN_ARGON2_TIME_COST..=MAX_ARGON2_TIME_COST).contains(&self.time_cost) {
            return Err(insecure("time_cost out of range"));
        }
        if !(MIN_ARGON2_PARALLELISM..=MAX_ARGON2_PARALLELISM).contains(&self.parallelism) {
            return Err(insecure("parallelism out of range"));
        }
        let salt = BASE64_STANDARD
            .decode(self.salt.as_bytes())
            .map_err(|_| insecure("salt is not standard base64"))?;
        if !(MIN_SALT_LEN..=MAX_SALT_LEN).contains(&salt.len()) {
            return Err(insecure("salt must have 16-64 bytes"));
        }
        Ok(salt)
    }
}

fn insecure(reason: &str) -> CryptoError {
    CryptoError::InsecureKdfParams(reason.to_string())
}

fn invalid_backup(reason: &str) -> CryptoError {
    CryptoError::InvalidBackup(reason.to_string())
}

/// Checks the Master Password rules and returns its NFC-normalized UTF-8 bytes.
///
/// The password is **not** trimmed. NFC normalization makes the same password typed on
/// different systems derive the same KEK (the web portal applies `normalize("NFC")`).
///
/// # Errors
/// [`CryptoError::InvalidPassphrase`] when it has fewer than 12 characters or more than
/// 1024 bytes.
pub fn normalize_master_password(password: &str) -> Result<Zeroizing<String>, CryptoError> {
    let normalized = Zeroizing::new(password.nfc().collect::<String>());
    if normalized.chars().count() < MIN_MASTER_PASSWORD_LEN {
        return Err(CryptoError::InvalidPassphrase(format!(
            "Master Password must have at least {} characters",
            MIN_MASTER_PASSWORD_LEN
        )));
    }
    if normalized.len() > MAX_MASTER_PASSWORD_BYTES {
        return Err(CryptoError::InvalidPassphrase(format!(
            "Master Password must have at most {} bytes",
            MAX_MASTER_PASSWORD_BYTES
        )));
    }
    Ok(normalized)
}

/// Derives the 32-byte KEK with Argon2id (RF-CRYPTO-03-2).
///
/// # Errors
/// [`CryptoError::InvalidPassphrase`], [`CryptoError::InsecureKdfParams`] (checked before
/// deriving) or [`CryptoError::KeyDerivationFailed`].
pub fn derive_kek(
    password: &str,
    params: &Argon2Params,
) -> Result<Zeroizing<[u8; KEY_LEN]>, CryptoError> {
    let password = normalize_master_password(password)?;
    let salt = params.validate()?;
    let argon_params = Params::new(
        params.memory_cost,
        params.time_cost,
        params.parallelism,
        Some(KEY_LEN),
    )
    .map_err(|_| insecure("rejected by Argon2"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut kek = Zeroizing::new([0u8; KEY_LEN]);
    argon
        .hash_password_into(password.as_bytes(), &salt, kek.as_mut())
        .map_err(|_| CryptoError::KeyDerivationFailed)?;
    Ok(kek)
}

/// Result of encrypting `K_admin_priv`: what goes to the API vault and into a `.tkb` file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBlob {
    /// Standard base64 of `nonce ‖ tag ‖ ciphertext` (60 bytes).
    pub encrypted_private_key: String,
    pub argon2_params: Argon2Params,
    /// Standard base64 of `K_admin_pub`.
    pub public_key: String,
    /// `hex(SHA-256(K_admin_pub))`, the fingerprint registered on the server.
    pub public_key_fingerprint: String,
}

/// Encrypts the base64 private key under a KEK derived from the Master Password
/// (RF-CRYPTO-03-3). A fresh random nonce is used; pass fresh `params` (new salt) for every
/// backup.
///
/// # Errors
/// Invalid key, password or parameters.
pub fn encrypt_private_key(
    private_key_b64: &str,
    password: &str,
    params: Argon2Params,
) -> Result<VaultBlob, CryptoError> {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    encrypt_private_key_with_nonce(private_key_b64, password, params, nonce)
}

/// Deterministic core of [`encrypt_private_key`]: only the reference vector test passes an
/// explicit nonce. Never reuse a nonce with the same KEK.
#[doc(hidden)]
pub fn encrypt_private_key_with_nonce(
    private_key_b64: &str,
    password: &str,
    params: Argon2Params,
    nonce: [u8; NONCE_LEN],
) -> Result<VaultBlob, CryptoError> {
    let secret = decode_private_key(private_key_b64)?;
    let raw_key = Zeroizing::new(secret.to_bytes());
    let public_key = derive_public_key(private_key_b64)?;
    let public_key_fingerprint = public_key_fingerprint(&public_key)?;

    let kek = derive_kek(password, &params)?;
    let cipher = Aes256Gcm::new_from_slice(kek.as_ref()).map_err(|_| CryptoError::AeadError)?;
    let ct_and_tag = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: raw_key.as_ref(),
                aad: KEY_BACKUP_AAD,
            },
        )
        .map_err(|_| CryptoError::AeadError)?;
    if ct_and_tag.len() != KEY_LEN + TAG_LEN {
        return Err(CryptoError::AeadError);
    }
    let (ciphertext, tag) = ct_and_tag.split_at(KEY_LEN);
    let mut blob = Vec::with_capacity(ENCRYPTED_PRIVATE_KEY_LEN);
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(tag);
    blob.extend_from_slice(ciphertext);

    Ok(VaultBlob {
        encrypted_private_key: BASE64_STANDARD.encode(&blob),
        argon2_params: params,
        public_key,
        public_key_fingerprint,
    })
}

/// Decrypts a v2 blob and returns the private key in the base64 form used by the keystore.
///
/// With `expected_fingerprint`, the recovered key must derive that public-key fingerprint
/// (the one in the container or the one registered on the server); otherwise
/// [`CryptoError::BackupKeyMismatch`].
///
/// # Errors
/// [`CryptoError::AeadError`] for a wrong password **or** a tampered blob (not
/// distinguished), [`CryptoError::InvalidBackup`] for a malformed blob, plus the errors of
/// [`derive_kek`].
pub fn decrypt_private_key(
    encrypted_private_key_b64: &str,
    password: &str,
    params: &Argon2Params,
    expected_fingerprint: Option<&str>,
) -> Result<Zeroizing<String>, CryptoError> {
    let blob = decode_blob(encrypted_private_key_b64)?;
    let kek = derive_kek(password, params)?;

    let (nonce, rest) = blob.split_at(NONCE_LEN);
    let (tag, ciphertext) = rest.split_at(TAG_LEN);
    let mut ct_and_tag = Vec::with_capacity(KEY_LEN + TAG_LEN);
    ct_and_tag.extend_from_slice(ciphertext);
    ct_and_tag.extend_from_slice(tag);

    let cipher = Aes256Gcm::new_from_slice(kek.as_ref()).map_err(|_| CryptoError::AeadError)?;
    let raw_key = Zeroizing::new(
        cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: &ct_and_tag,
                    aad: KEY_BACKUP_AAD,
                },
            )
            .map_err(|_| CryptoError::AeadError)?,
    );
    if raw_key.len() != KEY_LEN {
        return Err(CryptoError::AeadError);
    }
    let private_key_b64 = Zeroizing::new(BASE64_STANDARD.encode(raw_key.as_slice()));

    if let Some(expected) = expected_fingerprint {
        let fingerprint = public_key_fingerprint(&derive_public_key(&private_key_b64)?)?;
        if !fingerprints_equal(&fingerprint, expected) {
            return Err(CryptoError::BackupKeyMismatch);
        }
    }
    Ok(private_key_b64)
}

fn decode_blob(encrypted_private_key_b64: &str) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let blob = Zeroizing::new(
        BASE64_STANDARD
            .decode(encrypted_private_key_b64.trim().as_bytes())
            .map_err(|_| invalid_backup("encrypted_private_key is not standard base64"))?,
    );
    if blob.len() != ENCRYPTED_PRIVATE_KEY_LEN {
        return Err(invalid_backup(
            "encrypted_private_key must decode to 60 bytes",
        ));
    }
    Ok(blob)
}

/// Constant-time, case-insensitive comparison of two hex fingerprints.
pub fn fingerprints_equal(a: &str, b: &str) -> bool {
    let a = a.trim().to_ascii_lowercase();
    let b = b.trim().to_ascii_lowercase();
    a.len() == b.len() && bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}

/// `.tkb` container (RF-CRYPTO-03-4). JSON, UTF-8, unknown fields rejected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyBackupFile {
    pub format: String,
    pub version: u8,
    pub kdf: Argon2Params,
    pub encrypted_private_key: String,
    pub public_key: String,
    pub public_key_fingerprint: String,
    /// Informative (file name, UI); not authenticated.
    #[serde(default)]
    pub organization: String,
    /// Informative; not authenticated.
    pub created_at: DateTime<Utc>,
}

impl KeyBackupFile {
    /// Wraps a [`VaultBlob`] in a `.tkb` container.
    pub fn from_blob(blob: &VaultBlob, organization: &str, created_at: DateTime<Utc>) -> Self {
        Self {
            format: KEY_BACKUP_FORMAT.to_string(),
            version: KEY_BACKUP_VERSION,
            kdf: blob.argon2_params.clone(),
            encrypted_private_key: blob.encrypted_private_key.clone(),
            public_key: blob.public_key.clone(),
            public_key_fingerprint: blob.public_key_fingerprint.clone(),
            organization: organization.trim().to_string(),
            created_at,
        }
    }

    /// Pretty-printed JSON bytes, ready to be written as `telemetry_key_{org}_{date}.tkb`.
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, CryptoError> {
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Parses and structurally validates a `.tkb` file without decrypting it.
    ///
    /// # Errors
    /// [`CryptoError::InvalidBackup`] (size, JSON, `format`, `version`, key or fingerprint
    /// inconsistent, blob length) or [`CryptoError::InsecureKdfParams`].
    pub fn parse(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() > MAX_BACKUP_FILE_BYTES {
            return Err(invalid_backup("backup file is too large"));
        }
        let file: Self = serde_json::from_slice(bytes)
            .map_err(|e| CryptoError::InvalidBackup(format!("malformed .tkb container: {e}")))?;
        if file.format != KEY_BACKUP_FORMAT {
            return Err(invalid_backup("unsupported backup format"));
        }
        if file.version != KEY_BACKUP_VERSION {
            return Err(invalid_backup("unsupported backup version"));
        }
        file.kdf.validate()?;
        decode_blob(&file.encrypted_private_key)?;
        let fingerprint = public_key_fingerprint(&file.public_key)
            .map_err(|_| invalid_backup("public_key is not a valid X25519 key"))?;
        if !fingerprints_equal(&fingerprint, &file.public_key_fingerprint) {
            return Err(invalid_backup(
                "public_key_fingerprint does not match public_key",
            ));
        }
        Ok(file)
    }

    /// Decrypts the private key; it must derive the container's public key.
    pub fn decrypt(&self, password: &str) -> Result<Zeroizing<String>, CryptoError> {
        decrypt_private_key(
            &self.encrypted_private_key,
            password,
            &self.kdf,
            Some(&self.public_key_fingerprint),
        )
    }
}

/// Which backup format a file used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BackupFormat {
    /// `.vault` (`telemetry-vault-v1`, PBKDF2). Import only.
    V1Legacy,
    /// `.tkb` (`telemetry-key-backup` v2, Argon2id).
    V2,
}

/// Key recovered from a backup file.
#[derive(Debug)]
pub struct ImportedKey {
    /// Base64 of the 32-byte private key (keystore format).
    pub private_key_b64: Zeroizing<String>,
    pub public_key: String,
    pub public_key_fingerprint: String,
    pub format: BackupFormat,
}

/// Imports a `.tkb` (v2) or a legacy `.vault` (v1) file (RF-CRYPTO-03-5).
///
/// # Errors
/// [`CryptoError::InvalidBackup`] for unknown or malformed files, [`CryptoError::AeadError`]
/// for a wrong password or a tampered file, [`CryptoError::InsecureKdfParams`],
/// [`CryptoError::InvalidPassphrase`], [`CryptoError::BackupKeyMismatch`].
pub fn import_any_backup(bytes: &[u8], password: &str) -> Result<ImportedKey, CryptoError> {
    if bytes.len() > MAX_BACKUP_FILE_BYTES {
        return Err(invalid_backup("backup file is too large"));
    }
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| invalid_backup("backup file is not JSON"))?;
    let format = value
        .get("format")
        .and_then(|f| f.as_str())
        .ok_or_else(|| invalid_backup("backup file has no format"))?;

    match format {
        KEY_BACKUP_FORMAT => {
            let file = KeyBackupFile::parse(bytes)?;
            let private_key_b64 = file.decrypt(password)?;
            Ok(ImportedKey {
                private_key_b64,
                public_key: file.public_key,
                public_key_fingerprint: file.public_key_fingerprint.to_ascii_lowercase(),
                format: BackupFormat::V2,
            })
        }
        crate::backup::BACKUP_FORMAT_TAG => {
            let private_key_b64 = crate::backup::import_encrypted_admin_key(bytes, password)?;
            let public_key = derive_public_key(&private_key_b64)?;
            let public_key_fingerprint = public_key_fingerprint(&public_key)?;
            Ok(ImportedKey {
                private_key_b64,
                public_key,
                public_key_fingerprint,
                format: BackupFormat::V1Legacy,
            })
        }
        _ => Err(invalid_backup("unsupported backup format")),
    }
}

/// `telemetry_key_{org}_{AAAA-MM-DD}.tkb` (RF-CRYPTO-03-6): `org` lowercased, ASCII letters
/// and digits kept, accents folded (á → a, ñ → n), every other run of characters → `-`,
/// at most 40 characters, `admin` when nothing is left.
pub fn suggested_backup_file_name(organization: &str, date: NaiveDate) -> String {
    let mut slug = String::new();
    for c in organization.nfd() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if unicode_normalization::char::is_combining_mark(c) {
            continue;
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let mut slug: String = slug.trim_matches('-').chars().take(40).collect();
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        slug.push_str("admin");
    }
    format!(
        "telemetry_key_{}_{}.{}",
        slug,
        date.format("%Y-%m-%d"),
        KEY_BACKUP_EXTENSION
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asymmetric::generate_admin_keypair;
    use crate::{decrypt_telemetry_record, encrypt_telemetry_batch};

    const PASSWORD: &str = "correct horse battery staple";

    /// Minimum accepted costs keep the tests fast; the format is the same.
    fn fast_params() -> Argon2Params {
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);
        Argon2Params::with_salt(MIN_ARGON2_MEMORY_KIB, MIN_ARGON2_TIME_COST, 1, &salt)
    }

    #[test]
    fn test_backup_v2_roundtrip_returns_same_key() {
        let (private_key, public_key) = generate_admin_keypair();
        let blob = encrypt_private_key(&private_key, PASSWORD, fast_params()).unwrap();

        assert_eq!(blob.public_key, public_key);
        assert_eq!(
            BASE64_STANDARD
                .decode(&blob.encrypted_private_key)
                .unwrap()
                .len(),
            ENCRYPTED_PRIVATE_KEY_LEN
        );
        let recovered = decrypt_private_key(
            &blob.encrypted_private_key,
            PASSWORD,
            &blob.argon2_params,
            Some(&blob.public_key_fingerprint),
        )
        .unwrap();
        assert_eq!(recovered.as_str(), private_key);
    }

    #[test]
    fn test_backup_v2_default_params_roundtrip() {
        let (private_key, _) = generate_admin_keypair();
        let params = Argon2Params::generate_default();
        assert_eq!(params.memory_cost, 65_536);
        assert_eq!(params.time_cost, 3);
        assert_eq!(params.parallelism, 4);
        assert_eq!(params.validate().unwrap().len(), 16);
        let blob = encrypt_private_key(&private_key, PASSWORD, params).unwrap();
        let recovered = decrypt_private_key(
            &blob.encrypted_private_key,
            PASSWORD,
            &blob.argon2_params,
            None,
        )
        .unwrap();
        assert_eq!(recovered.as_str(), private_key);
    }

    #[test]
    fn test_backup_v2_wrong_password_rejected() {
        let (private_key, _) = generate_admin_keypair();
        let blob = encrypt_private_key(&private_key, PASSWORD, fast_params()).unwrap();
        let err = decrypt_private_key(
            &blob.encrypted_private_key,
            "correct horse battery stapler",
            &blob.argon2_params,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, CryptoError::AeadError), "{err:?}");
    }

    #[test]
    fn test_backup_v2_tampered_blob_rejected() {
        let (private_key, _) = generate_admin_keypair();
        let blob = encrypt_private_key(&private_key, PASSWORD, fast_params()).unwrap();
        let raw = BASE64_STANDARD.decode(&blob.encrypted_private_key).unwrap();
        for index in [0, 12, 30, 59] {
            let mut tampered = raw.clone();
            tampered[index] ^= 0x01;
            let err = decrypt_private_key(
                &BASE64_STANDARD.encode(&tampered),
                PASSWORD,
                &blob.argon2_params,
                None,
            )
            .unwrap_err();
            assert!(
                matches!(err, CryptoError::AeadError),
                "byte {index}: {err:?}"
            );
        }
    }

    #[test]
    fn test_backup_v2_aad_separates_from_telemetry_envelopes() {
        // Same KEK and nonce layout but without the backup AAD must not decrypt.
        let (private_key, _) = generate_admin_keypair();
        let params = fast_params();
        let kek = derive_kek(PASSWORD, &params).unwrap();
        let raw_key = decode_private_key(&private_key).unwrap().to_bytes();
        let envelope = crate::symmetric::encrypt_bytes(&kek, &raw_key).unwrap();
        let err = decrypt_private_key(&BASE64_STANDARD.encode(&envelope), PASSWORD, &params, None)
            .unwrap_err();
        assert!(matches!(err, CryptoError::AeadError), "{err:?}");
    }

    #[test]
    fn test_backup_v2_insecure_params_rejected_before_derivation() {
        let base = fast_params();
        let cases: [fn(&mut Argon2Params); 10] = [
            |p| p.algorithm = "argon2i".into(),
            |p| p.version = 16,
            |p| p.memory_cost = MIN_ARGON2_MEMORY_KIB - 1,
            |p| p.memory_cost = MAX_ARGON2_MEMORY_KIB + 1,
            |p| p.time_cost = 1,
            |p| p.time_cost = 21,
            |p| p.parallelism = 0,
            |p| p.parallelism = 17,
            |p| p.salt = BASE64_STANDARD.encode([7u8; 15]),
            |p| p.salt = "***".into(),
        ];
        for (index, mutate) in cases.iter().enumerate() {
            let mut params = base.clone();
            mutate(&mut params);
            let err = derive_kek(PASSWORD, &params).unwrap_err();
            assert!(
                matches!(err, CryptoError::InsecureKdfParams(_)),
                "case {index}: {err:?}"
            );
        }
        // Upper bound salt and algorithm case-insensitivity are accepted.
        let mut ok = base.clone();
        ok.salt = BASE64_STANDARD.encode([1u8; 64]);
        ok.algorithm = "Argon2ID".into();
        assert!(ok.validate().is_ok());
        // A 1 TiB memory request is refused instantly instead of allocating.
        let mut huge = base;
        huge.memory_cost = u32::MAX;
        assert!(matches!(
            huge.validate(),
            Err(CryptoError::InsecureKdfParams(_))
        ));
    }

    #[test]
    fn test_backup_v2_master_password_rules() {
        assert!(matches!(
            normalize_master_password("short-pass1"),
            Err(CryptoError::InvalidPassphrase(_))
        ));
        assert!(normalize_master_password("exactly-12ch").is_ok());
        // 12 characters, 24 bytes: the rule counts characters, not bytes.
        assert!(normalize_master_password("ñññññññññññ1").is_ok());
        assert!(matches!(
            normalize_master_password(&"a".repeat(1025)),
            Err(CryptoError::InvalidPassphrase(_))
        ));
        // Spaces are significant (never trimmed).
        assert_ne!(
            normalize_master_password(" exactly-12ch").unwrap().as_str(),
            "exactly-12ch"
        );
    }

    #[test]
    fn test_backup_v2_password_is_nfc_normalized() {
        let (private_key, _) = generate_admin_keypair();
        let composed = "contraseña-segura"; // U+00F1
        let decomposed = "contrasen\u{0303}a-segura"; // n + combining tilde
        assert_ne!(composed, decomposed);
        let blob = encrypt_private_key(&private_key, composed, fast_params()).unwrap();
        let recovered = decrypt_private_key(
            &blob.encrypted_private_key,
            decomposed,
            &blob.argon2_params,
            None,
        )
        .unwrap();
        assert_eq!(recovered.as_str(), private_key);
    }

    /// Reference vector shared with `telemetry-web` (TASK-WEB-03) and documented in
    /// TASK-CRYPTO-03 §8. Cross-checked with an independent implementation (Python
    /// `argon2-cffi` + `cryptography`).
    #[test]
    fn test_backup_v2_fixed_vector_matches_reference() {
        let private_key: Vec<u8> = (1u8..=32).collect();
        let private_key_b64 = BASE64_STANDARD.encode(&private_key);
        let salt: Vec<u8> = (0u8..16).collect();
        let nonce: [u8; 12] = [
            0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab,
        ];
        let params = Argon2Params::with_salt(32_768, 2, 1, &salt);

        let kek = derive_kek(PASSWORD, &params).unwrap();
        assert_eq!(hex::encode(kek.as_ref()), VECTOR_KEK_HEX);

        let blob =
            encrypt_private_key_with_nonce(&private_key_b64, PASSWORD, params.clone(), nonce)
                .unwrap();
        assert_eq!(params.salt, "AAECAwQFBgcICQoLDA0ODw==");
        assert_eq!(blob.encrypted_private_key, VECTOR_BLOB_B64);
        assert_eq!(blob.public_key, VECTOR_PUBLIC_KEY_B64);
        assert_eq!(blob.public_key_fingerprint, VECTOR_FINGERPRINT_HEX);

        let recovered = decrypt_private_key(
            VECTOR_BLOB_B64,
            PASSWORD,
            &params,
            Some(VECTOR_FINGERPRINT_HEX),
        )
        .unwrap();
        assert_eq!(recovered.as_str(), private_key_b64);
    }

    const VECTOR_KEK_HEX: &str = "a3eaeba89d069458afff8b7d965ea12de292418ded91b305cb494aa56d361b50";
    const VECTOR_BLOB_B64: &str =
        "oKGio6SlpqeoqaqrT1YY5P0VsEv/kwyY7ROigakSvxJvRQMTiNAaUGwcyir/mCGgjduecElrPGh0i6Ab";
    const VECTOR_PUBLIC_KEY_B64: &str = "B6N8vBQgk8i3VdwbEOhstCY3StFqqFPtC9/AsrhtHHw=";
    const VECTOR_FINGERPRINT_HEX: &str =
        "aaa8fff703b50b2297f4f6e13508f72420d96fd01ebb84cb074449caaef64041";

    #[test]
    fn test_backup_v2_file_roundtrip_and_unknown_fields_rejected() {
        let (private_key, public_key) = generate_admin_keypair();
        let blob = encrypt_private_key(&private_key, PASSWORD, fast_params()).unwrap();
        let created_at = DateTime::parse_from_rfc3339("2026-09-30T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let file = KeyBackupFile::from_blob(&blob, "  Acme SpA ", created_at);
        let bytes = file.to_json_bytes().unwrap();

        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["format"], "telemetry-key-backup");
        assert_eq!(json["version"], 2);
        assert_eq!(json["kdf"]["algorithm"], "argon2id");
        assert_eq!(json["kdf"]["version"], 19);
        assert_eq!(json["organization"], "Acme SpA");
        assert_eq!(json["created_at"], "2026-09-30T12:00:00Z");
        assert_eq!(json["public_key"], public_key.as_str());
        assert!(
            !String::from_utf8_lossy(&bytes).contains(&private_key),
            "the private key must never appear in clear"
        );

        let parsed = KeyBackupFile::parse(&bytes).unwrap();
        assert_eq!(parsed, file);
        assert_eq!(parsed.decrypt(PASSWORD).unwrap().as_str(), private_key);

        let mut extra = json.clone();
        extra["master_password"] = serde_json::json!("oops");
        assert!(matches!(
            KeyBackupFile::parse(&serde_json::to_vec(&extra).unwrap()),
            Err(CryptoError::InvalidBackup(_))
        ));
        let mut extra_kdf = json.clone();
        extra_kdf["kdf"]["pepper"] = serde_json::json!("x");
        assert!(KeyBackupFile::parse(&serde_json::to_vec(&extra_kdf).unwrap()).is_err());
        let mut wrong_version = json.clone();
        wrong_version["version"] = serde_json::json!(3);
        assert!(KeyBackupFile::parse(&serde_json::to_vec(&wrong_version).unwrap()).is_err());
        let mut wrong_format = json.clone();
        wrong_format["format"] = serde_json::json!("telemetry-vault-v1");
        assert!(KeyBackupFile::parse(&serde_json::to_vec(&wrong_format).unwrap()).is_err());
        let mut short_blob = json.clone();
        short_blob["encrypted_private_key"] = serde_json::json!(BASE64_STANDARD.encode([0u8; 59]));
        assert!(KeyBackupFile::parse(&serde_json::to_vec(&short_blob).unwrap()).is_err());
        let mut inconsistent = json;
        inconsistent["public_key_fingerprint"] = serde_json::json!("0".repeat(64));
        assert!(KeyBackupFile::parse(&serde_json::to_vec(&inconsistent).unwrap()).is_err());
        assert!(KeyBackupFile::parse(&vec![b' '; MAX_BACKUP_FILE_BYTES + 1]).is_err());
    }

    #[test]
    fn test_backup_v2_import_any_accepts_v2_and_legacy_v1() {
        let (private_key, public_key) = generate_admin_keypair();

        let blob = encrypt_private_key(&private_key, PASSWORD, fast_params()).unwrap();
        let v2 = KeyBackupFile::from_blob(&blob, "Acme", Utc::now())
            .to_json_bytes()
            .unwrap();
        let imported = import_any_backup(&v2, PASSWORD).unwrap();
        assert_eq!(imported.format, BackupFormat::V2);
        assert_eq!(imported.private_key_b64.as_str(), private_key);
        assert_eq!(imported.public_key, public_key);

        let v1 = crate::backup::export_encrypted_admin_key(&private_key, PASSWORD).unwrap();
        let imported = import_any_backup(&v1, PASSWORD).unwrap();
        assert_eq!(imported.format, BackupFormat::V1Legacy);
        assert_eq!(imported.private_key_b64.as_str(), private_key);
        assert_eq!(
            imported.public_key_fingerprint,
            public_key_fingerprint(&public_key).unwrap()
        );

        assert!(matches!(
            import_any_backup(&v2, "wrong password 123"),
            Err(CryptoError::AeadError)
        ));
        assert!(matches!(
            import_any_backup(br#"{"format":"something-else"}"#, PASSWORD),
            Err(CryptoError::InvalidBackup(_))
        ));
        assert!(matches!(
            import_any_backup(b"not json", PASSWORD),
            Err(CryptoError::InvalidBackup(_))
        ));
    }

    #[test]
    fn test_backup_v2_import_legacy_v1_fixture() {
        // Real v1 file produced by the admin app before TASK-CRYPTO-03.
        let imported =
            import_any_backup(LEGACY_V1_FIXTURE.as_bytes(), LEGACY_V1_PASSPHRASE).unwrap();
        assert_eq!(imported.format, BackupFormat::V1Legacy);
        assert_eq!(imported.private_key_b64.as_str(), LEGACY_V1_PRIVATE_KEY);
    }

    const LEGACY_V1_PASSPHRASE: &str = "legacy-vault-passphrase";
    const LEGACY_V1_PRIVATE_KEY: &str = "QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl8=";
    const LEGACY_V1_FIXTURE: &str = r#"{
  "format": "telemetry-vault-v1",
  "iterations": 100000,
  "salt": "EBESExQVFhcYGRobHB0eHw==",
  "encrypted_payload": "MDEyMzQ1Njc4OTo7uwwaxxwcYTcQbq/Gh3MpJqYmU3tKMVNKpR49dU4Xwpjo2C0qTWkeOqhl+4ttxRRoB90e4PZPo57hEKMY"
}"#;

    #[test]
    fn test_backup_v2_fingerprint_mismatch_rejected() {
        let (private_key, _) = generate_admin_keypair();
        let (_, other_public) = generate_admin_keypair();
        let other_fingerprint = public_key_fingerprint(&other_public).unwrap();
        let blob = encrypt_private_key(&private_key, PASSWORD, fast_params()).unwrap();
        let err = decrypt_private_key(
            &blob.encrypted_private_key,
            PASSWORD,
            &blob.argon2_params,
            Some(&other_fingerprint),
        )
        .unwrap_err();
        assert!(matches!(err, CryptoError::BackupKeyMismatch), "{err:?}");
        // Upper-case fingerprints are accepted.
        assert!(decrypt_private_key(
            &blob.encrypted_private_key,
            PASSWORD,
            &blob.argon2_params,
            Some(&blob.public_key_fingerprint.to_ascii_uppercase()),
        )
        .is_ok());
    }

    #[test]
    fn test_backup_v2_suggested_file_name() {
        let date = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        assert_eq!(
            suggested_backup_file_name("Acme SpA", date),
            "telemetry_key_acme-spa_2026-09-30.tkb"
        );
        assert_eq!(
            suggested_backup_file_name("  Compañía Ñandú & Cía. Ltda. ", date),
            "telemetry_key_compania-nandu-cia-ltda_2026-09-30.tkb"
        );
        assert_eq!(
            suggested_backup_file_name("", date),
            "telemetry_key_admin_2026-09-30.tkb"
        );
        assert_eq!(
            suggested_backup_file_name("../../etc/passwd", date),
            "telemetry_key_etc-passwd_2026-09-30.tkb"
        );
        let long = suggested_backup_file_name(&"x".repeat(100), date);
        assert_eq!(
            long,
            format!("telemetry_key_{}_2026-09-30.tkb", "x".repeat(40))
        );
    }

    #[test]
    fn test_backup_v2_restored_key_decrypts_existing_telemetry() {
        // Criterion of TASK-APP-02: export → import → data accessible.
        let (private_key, public_key) = generate_admin_keypair();
        let envelope =
            encrypt_telemetry_batch(&serde_json::json!({"cpu": 42}), &public_key).unwrap();
        let blob = encrypt_private_key(&private_key, PASSWORD, fast_params()).unwrap();
        let file = KeyBackupFile::from_blob(&blob, "Acme", Utc::now())
            .to_json_bytes()
            .unwrap();
        let restored = import_any_backup(&file, PASSWORD).unwrap();
        let plain: serde_json::Value = decrypt_telemetry_record(
            &restored.private_key_b64,
            &envelope.ephemeral_public_key,
            &envelope.encrypted_payload,
        )
        .unwrap();
        assert_eq!(plain["cpu"], 42);
    }
}
