//! Browser-compatible AES-256-GCM envelope decryption.

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};

pub const INTENT_ENVELOPE_AAD: &[u8] = b"tohseno.intent-envelope/1";

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IntentEnvelopeError;

impl std::fmt::Display for IntentEnvelopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("encrypted intention could not be authenticated")
    }
}

impl std::error::Error for IntentEnvelopeError {}

pub fn decrypt_intent_envelope(
    ciphertext: &[u8],
    key: &[u8],
    nonce: &[u8],
    associated_data: &[u8],
) -> Result<Vec<u8>, IntentEnvelopeError> {
    if key.len() != 32 || nonce.len() != 12 || associated_data != INTENT_ENVELOPE_AAD {
        return Err(IntentEnvelopeError);
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| IntentEnvelopeError)?;
    let nonce_bytes: &[u8; 12] = nonce.try_into().map_err(|_| IntentEnvelopeError)?;
    let nonce = Nonce::from(*nonce_bytes);
    cipher
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext,
                aad: associated_data,
            },
        )
        .map_err(|_| IntentEnvelopeError)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::aead::Aead;

    #[test]
    fn fixed_browser_compatibility_vector() {
        let key = [0x11_u8; 32];
        let nonce = [0x22_u8; 12];
        let plaintext = b"TOHSENO browser AES-GCM vector";
        let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
        let nonce = Nonce::from(nonce);
        let encrypted = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad: INTENT_ENVELOPE_AAD,
                },
            )
            .unwrap();
        assert_eq!(
            hex(&encrypted),
            "43b84f1a8581d07f874db14b3bc39bf55ed69ecf2dc2e16b242a699c669342e05245593e2017eda2b91985b0a99f"
        );
        assert_eq!(
            decrypt_intent_envelope(&encrypted, &key, &nonce, INTENT_ENVELOPE_AAD).unwrap(),
            plaintext
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
