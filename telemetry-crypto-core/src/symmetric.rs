use crate::error::CryptoError;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;

/// Encrypts arbitrary plaintext bytes with AES-256-GCM using the provided 256-bit DEK.
/// Produces a payload formatted as: `[ Nonce (12B) | Auth Tag (16B) | Ciphertext (NB) ]`.
pub fn encrypt_bytes(dek: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(dek).map_err(|_| CryptoError::AeadError)?;
    let mut nonce_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext_with_tag = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| CryptoError::AeadError)?;

    if ciphertext_with_tag.len() < 16 {
        return Err(CryptoError::AeadError);
    }

    let tag = &ciphertext_with_tag[ciphertext_with_tag.len() - 16..];
    let ciphertext = &ciphertext_with_tag[..ciphertext_with_tag.len() - 16];

    let mut output = Vec::with_capacity(12 + 16 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(tag);
    output.extend_from_slice(ciphertext);
    Ok(output)
}

/// Decrypts a payload formatted as `[ Nonce (12B) | Auth Tag (16B) | Ciphertext (NB) ]`
/// using AES-256-GCM and the provided 256-bit DEK.
pub fn decrypt_bytes(dek: &[u8; 32], encrypted_payload: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if encrypted_payload.len() < 28 {
        return Err(CryptoError::PayloadTooShort);
    }

    let nonce_bytes = &encrypted_payload[..12];
    let tag = &encrypted_payload[12..28];
    let ciphertext = &encrypted_payload[28..];

    let nonce = Nonce::from_slice(nonce_bytes);

    // Aes256Gcm expects ciphertext followed by 16-byte authentication tag
    let mut ct_and_tag = Vec::with_capacity(ciphertext.len() + 16);
    ct_and_tag.extend_from_slice(ciphertext);
    ct_and_tag.extend_from_slice(tag);

    let cipher = Aes256Gcm::new_from_slice(dek).map_err(|_| CryptoError::AeadError)?;
    cipher
        .decrypt(nonce, ct_and_tag.as_ref())
        .map_err(|_| CryptoError::AeadError)
}
