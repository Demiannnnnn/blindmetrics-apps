use crate::error::CryptoError;
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

pub const HKDF_SALT: &[u8] = b"telemetry-v1";
pub const HKDF_INFO: &[u8] = b"aes256gcm";

/// Derives a 256-bit symmetric encryption key from a shared secret using HKDF-SHA256.
/// The resulting key is securely wrapped in `Zeroizing` to ensure memory sanitization on drop.
pub fn derive_shared_key(shared_secret: &[u8; 32]) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), shared_secret);
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(HKDF_INFO, okm.as_mut())
        .map_err(|_| CryptoError::KeyDerivationFailed)?;
    Ok(okm)
}
