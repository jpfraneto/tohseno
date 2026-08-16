//! Small, versioned crypto building blocks with Apple CryptoKit equivalents.

use crate::{require, CompanionError, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

pub const ENVELOPE_KEY_DOMAIN: &[u8] = b"tohseno.companion.envelope-key.v1";
pub const PAIRING_CONFIRMATION_DOMAIN: &[u8] = b"tohseno.companion.pairing-confirmation.v1";
pub const PAIRING_RESPONSE_KEY_DOMAIN: &[u8] = b"tohseno.companion.pairing-response-key.v1";

pub fn base64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn decode_base64url(label: &str, value: &str, maximum: usize) -> Result<Vec<u8>> {
    require(
        !value.is_empty() && value.len() <= maximum.saturating_mul(2).saturating_add(8),
        format!("{label} is empty or too large"),
    )?;
    let decoded = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| CompanionError::Invalid(format!("{label} is not canonical base64url")))?;
    require(decoded.len() <= maximum, format!("{label} is too large"))?;
    require(
        base64url(&decoded) == value,
        format!("{label} is not canonical base64url"),
    )?;
    Ok(decoded)
}

pub fn decode_array<const N: usize>(label: &str, value: &str) -> Result<[u8; N]> {
    decode_base64url(label, value, N)?
        .try_into()
        .map_err(|_| CompanionError::Invalid(format!("{label} has the wrong byte length")))
}

pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub fn sha256_many(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

pub fn x25519(secret: &StaticSecret, public_key: &[u8; 32]) -> Result<[u8; 32]> {
    let public = PublicKey::from(*public_key);
    let shared = secret.diffie_hellman(&public).to_bytes();
    require(shared != [0_u8; 32], "X25519 shared secret is all zero")?;
    Ok(shared)
}

pub fn derive_key(
    shared_secret: &[u8; 32],
    salt: &[u8],
    domain: &[u8],
) -> Result<Zeroizing<[u8; 32]>> {
    let hkdf = Hkdf::<Sha256>::new(Some(salt), shared_secret);
    let mut output = Zeroizing::new([0_u8; 32]);
    hkdf.expand(domain, output.as_mut())
        .map_err(|_| CompanionError::Crypto("HKDF-SHA-256 expansion"))?;
    Ok(output)
}

pub fn encrypt(
    key: &[u8; 32],
    nonce: &[u8; 12],
    plaintext: &[u8],
    associated_data: &[u8],
) -> Result<Vec<u8>> {
    let nonce = Nonce::from(*nonce);
    ChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| CompanionError::Crypto("ChaCha20-Poly1305 key"))?
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad: associated_data,
            },
        )
        .map_err(|_| CompanionError::Crypto("ChaCha20-Poly1305 encryption"))
}

pub fn decrypt(
    key: &[u8; 32],
    nonce: &[u8; 12],
    ciphertext: &[u8],
    associated_data: &[u8],
) -> Result<Vec<u8>> {
    let nonce = Nonce::from(*nonce);
    ChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| CompanionError::Crypto("ChaCha20-Poly1305 key"))?
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext,
                aad: associated_data,
            },
        )
        .map_err(|_| CompanionError::Crypto("ChaCha20-Poly1305 authentication"))
}

pub fn hmac_sha256(key: &[u8], domain: &[u8], message: &[u8]) -> Result<[u8; 32]> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key)
        .map_err(|_| CompanionError::Crypto("HMAC-SHA-256 key"))?;
    mac.update(domain);
    mac.update(&[0]);
    mac.update(message);
    Ok(mac.finalize().into_bytes().into())
}

pub fn verify_hmac_sha256(
    key: &[u8],
    domain: &[u8],
    message: &[u8],
    expected: &[u8; 32],
) -> Result<()> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key)
        .map_err(|_| CompanionError::Crypto("HMAC-SHA-256 key"))?;
    mac.update(domain);
    mac.update(&[0]);
    mac.update(message);
    mac.verify_slice(expected)
        .map_err(|_| CompanionError::Crypto("HMAC-SHA-256 verification"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_is_unpadded_and_canonical() {
        assert_eq!(base64url(&[0xff, 0xee]), "_-4");
        assert_eq!(decode_array::<2>("fixture", "_-4").unwrap(), [0xff, 0xee]);
        assert!(decode_base64url("fixture", "_-4=", 2).is_err());
    }

    #[test]
    fn chacha_authentication_rejects_tampering() {
        let key = [3_u8; 32];
        let nonce = [4_u8; 12];
        let mut ciphertext = encrypt(&key, &nonce, b"private", b"header").unwrap();
        assert_eq!(
            decrypt(&key, &nonce, &ciphertext, b"header").unwrap(),
            b"private"
        );
        ciphertext[0] ^= 1;
        assert!(decrypt(&key, &nonce, &ciphertext, b"header").is_err());
    }
}
