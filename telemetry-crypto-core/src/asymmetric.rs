use base64::prelude::*;
use rand::rngs::OsRng;
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};
use zeroize::Zeroizing;
use crate::error::CryptoError;

/// Generates a new static X25519 keypair for the Admin console.
/// Returns `(private_key_b64, public_key_b64)`.
pub fn generate_admin_keypair() -> (String, String) {
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);

    let priv_b64 = BASE64_STANDARD.encode(secret.to_bytes());
    let pub_b64 = BASE64_STANDARD.encode(public.as_bytes());
    (priv_b64, pub_b64)
}

/// Decodes a Base64-encoded 32-byte public key.
pub fn decode_public_key(pub_key_b64: &str) -> Result<PublicKey, CryptoError> {
    let bytes = BASE64_STANDARD.decode(pub_key_b64.trim())?;
    if bytes.len() != 32 {
        return Err(CryptoError::InvalidKeyLength {
            expected: 32,
            got: bytes.len(),
        });
    }
    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&bytes);
    Ok(PublicKey::from(key_bytes))
}

/// Decodes a Base64-encoded 32-byte private key.
pub fn decode_private_key(priv_key_b64: &str) -> Result<StaticSecret, CryptoError> {
    let bytes = BASE64_STANDARD.decode(priv_key_b64.trim())?;
    if bytes.len() != 32 {
        return Err(CryptoError::InvalidKeyLength {
            expected: 32,
            got: bytes.len(),
        });
    }
    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&bytes);
    Ok(StaticSecret::from(key_bytes))
}

/// Derives the public key from a private key string.
pub fn derive_public_key(priv_key_b64: &str) -> Result<String, CryptoError> {
    let secret = decode_private_key(priv_key_b64)?;
    let public = PublicKey::from(&secret);
    Ok(BASE64_STANDARD.encode(public.as_bytes()))
}

/// Generates an ephemeral X25519 secret and computes the ECDH shared secret with peer's public key.
/// Returns `(ephemeral_public_key_b64, shared_secret)` with the secret securely zeroized on drop.
pub fn compute_ephemeral_shared_secret(
    peer_public: &PublicKey,
) -> (String, Zeroizing<[u8; 32]>) {
    let eph_secret = EphemeralSecret::random_from_rng(OsRng);
    let eph_public = PublicKey::from(&eph_secret);
    let shared = eph_secret.diffie_hellman(peer_public);

    let eph_pub_b64 = BASE64_STANDARD.encode(eph_public.as_bytes());
    let mut ss_bytes = Zeroizing::new([0u8; 32]);
    ss_bytes.copy_from_slice(shared.as_bytes());
    (eph_pub_b64, ss_bytes)
}

/// Computes the ECDH shared secret using an existing static private key and peer's public key.
pub fn compute_static_shared_secret(
    static_priv: &StaticSecret,
    peer_public: &PublicKey,
) -> Zeroizing<[u8; 32]> {
    let shared = static_priv.diffie_hellman(peer_public);
    let mut ss_bytes = Zeroizing::new([0u8; 32]);
    ss_bytes.copy_from_slice(shared.as_bytes());
    ss_bytes
}
