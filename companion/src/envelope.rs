//! Recipient-specific opaque envelopes for the content-blind relay.

use crate::canonical;
use crate::crypto::{
    base64url, decode_array, decode_base64url, decrypt, derive_key, encrypt, sha256, x25519,
    ENVELOPE_KEY_DOMAIN,
};
use crate::identity::{CompanionIdentity, TransportIdentity};
use crate::journal::{ReplayDecision, ReplayWindow};
use crate::{
    require, validate_identifier, validate_window, CompanionError, Result, MAX_CLOCK_SKEW_SECONDS,
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

pub const COMPANION_ENVELOPE_SCHEMA: &str = "tohseno.companion-envelope/1";
pub const ENVELOPE_SIGNATURE_DOMAIN: &[u8] = b"tohseno.companion.envelope-signature.v1";
pub const MAX_ENVELOPE_PLAINTEXT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_ENVELOPE_LIFETIME_SECONDS: i64 = 7 * 24 * 60 * 60;
pub const MAX_SAFE_SENDER_SEQUENCE: u64 = (1_u64 << 53) - 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeHeader {
    pub schema: String,
    pub envelope_id: String,
    pub mailbox_id: String,
    pub sender_device_id: String,
    pub recipient_device_id: String,
    pub sender_sequence: u64,
    pub created_at: String,
    pub expires_at: String,
    pub ephemeral_public_key: String,
    pub nonce: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpaqueEnvelope {
    #[serde(flatten)]
    pub header: EnvelopeHeader,
    pub ciphertext: String,
    pub signature: String,
}

impl OpaqueEnvelope {
    /// Validate only relay-visible shape and bounds, without private keys or
    /// interpretation of the ciphertext.
    pub fn validate_relay_shape(&self) -> Result<()> {
        self.header.validate_shape()?;
        let ciphertext = decode_base64url(
            "envelope ciphertext",
            &self.ciphertext,
            MAX_ENVELOPE_PLAINTEXT_BYTES + 16,
        )?;
        require(ciphertext.len() >= 16, "envelope ciphertext is too short")?;
        decode_array::<64>("envelope signature", &self.signature)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvelopeMetadata {
    pub envelope_id: String,
    pub mailbox_id: String,
    pub recipient_device_id: String,
    pub sender_sequence: u64,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Serialize)]
struct UnsignedEnvelope<'a> {
    #[serde(flatten)]
    header: &'a EnvelopeHeader,
    ciphertext: &'a str,
}

pub fn seal_envelope<I: TransportIdentity>(
    sender: &I,
    recipient_agreement_public_key: &[u8; 32],
    metadata: EnvelopeMetadata,
    plaintext: &[u8],
) -> Result<OpaqueEnvelope> {
    let mut ephemeral_secret = Zeroizing::new([0_u8; 32]);
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(ephemeral_secret.as_mut());
    OsRng.fill_bytes(&mut nonce);
    seal_with_material(
        sender,
        recipient_agreement_public_key,
        metadata,
        plaintext,
        *ephemeral_secret,
        nonce,
    )
}

pub fn seal_with_material<I: TransportIdentity>(
    sender: &I,
    recipient_agreement_public_key: &[u8; 32],
    metadata: EnvelopeMetadata,
    plaintext: &[u8],
    ephemeral_secret: [u8; 32],
    nonce: [u8; 12],
) -> Result<OpaqueEnvelope> {
    require(
        !plaintext.is_empty() && plaintext.len() <= MAX_ENVELOPE_PLAINTEXT_BYTES,
        "envelope plaintext is empty or too large",
    )?;
    let ephemeral_secret = StaticSecret::from(ephemeral_secret);
    let ephemeral_public_key = PublicKey::from(&ephemeral_secret).to_bytes();
    let header = EnvelopeHeader {
        schema: COMPANION_ENVELOPE_SCHEMA.into(),
        envelope_id: metadata.envelope_id,
        mailbox_id: metadata.mailbox_id,
        sender_device_id: sender.device_id().into(),
        recipient_device_id: metadata.recipient_device_id,
        sender_sequence: metadata.sender_sequence,
        created_at: metadata.created_at,
        expires_at: metadata.expires_at,
        ephemeral_public_key: base64url(&ephemeral_public_key),
        nonce: base64url(&nonce),
    };
    header.validate_shape()?;
    let aad = canonical::to_vec(&header)?;
    let shared = x25519(&ephemeral_secret, recipient_agreement_public_key)?;
    let salt = sha256(&aad);
    let key = derive_key(&shared, &salt, ENVELOPE_KEY_DOMAIN)?;
    let ciphertext_bytes = encrypt(&key, &nonce, plaintext, &aad)?;
    let ciphertext = base64url(&ciphertext_bytes);
    let unsigned = canonical::to_vec(&UnsignedEnvelope {
        header: &header,
        ciphertext: &ciphertext,
    })?;
    let signature = sender.sign(ENVELOPE_SIGNATURE_DOMAIN, &unsigned);
    Ok(OpaqueEnvelope {
        header,
        ciphertext,
        signature: base64url(&signature),
    })
}

pub fn open_envelope<I: TransportIdentity>(
    envelope: &OpaqueEnvelope,
    expected_sender_signing_public_key: &[u8; 32],
    expected_sender_device_id: &str,
    recipient: &I,
    now: OffsetDateTime,
    replay: &mut ReplayWindow,
) -> Result<Vec<u8>> {
    envelope.validate_relay_shape()?;
    require(
        envelope.header.sender_device_id == expected_sender_device_id,
        "envelope sender device is not the expected paired device",
    )?;
    require(
        envelope.header.recipient_device_id == recipient.device_id(),
        "envelope is addressed to a different recipient",
    )?;
    validate_window(
        &envelope.header.created_at,
        &envelope.header.expires_at,
        now,
        MAX_ENVELOPE_LIFETIME_SECONDS,
        MAX_CLOCK_SKEW_SECONDS,
    )?;
    let ciphertext = decode_base64url(
        "envelope ciphertext",
        &envelope.ciphertext,
        MAX_ENVELOPE_PLAINTEXT_BYTES + 16,
    )?;
    let signature = decode_array::<64>("envelope signature", &envelope.signature)?;
    let unsigned = canonical::to_vec(&UnsignedEnvelope {
        header: &envelope.header,
        ciphertext: &envelope.ciphertext,
    })?;
    CompanionIdentity::verify(
        expected_sender_signing_public_key,
        ENVELOPE_SIGNATURE_DOMAIN,
        &unsigned,
        &signature,
    )?;
    let ephemeral = decode_array::<32>(
        "envelope ephemeral public key",
        &envelope.header.ephemeral_public_key,
    )?;
    let nonce = decode_array::<12>("envelope nonce", &envelope.header.nonce)?;
    let aad = canonical::to_vec(&envelope.header)?;
    let shared = recipient.agree(&ephemeral)?;
    let salt = sha256(&aad);
    let key = derive_key(&shared, &salt, ENVELOPE_KEY_DOMAIN)?;
    let plaintext = decrypt(&key, &nonce, &ciphertext, &aad)?;
    match replay.observe(
        &envelope.header.sender_device_id,
        envelope.header.sender_sequence,
        &envelope.header.envelope_id,
    )? {
        ReplayDecision::New => Ok(plaintext),
        ReplayDecision::Duplicate => {
            Err(CompanionError::Replay("duplicate envelope delivery".into()))
        }
    }
}

impl EnvelopeHeader {
    fn validate_shape(&self) -> Result<()> {
        require(
            self.schema == COMPANION_ENVELOPE_SCHEMA,
            "unsupported companion envelope schema",
        )?;
        validate_uuid_v4(&self.envelope_id)?;
        validate_identifier("mailbox ID", &self.mailbox_id)?;
        validate_identifier("sender device ID", &self.sender_device_id)?;
        validate_identifier("recipient device ID", &self.recipient_device_id)?;
        require(
            (1..=MAX_SAFE_SENDER_SEQUENCE).contains(&self.sender_sequence),
            "sender sequence must be a positive cross-language safe integer",
        )?;
        let created_at = crate::parse_timestamp(&self.created_at)?;
        let expires_at = crate::parse_timestamp(&self.expires_at)?;
        require(
            (expires_at - created_at).whole_seconds() > 0
                && (expires_at - created_at).whole_seconds() <= MAX_ENVELOPE_LIFETIME_SECONDS,
            "envelope lifetime is invalid",
        )?;
        decode_array::<32>("envelope ephemeral public key", &self.ephemeral_public_key)?;
        decode_array::<12>("envelope nonce", &self.nonce)?;
        Ok(())
    }
}

fn validate_uuid_v4(value: &str) -> Result<()> {
    let bytes = value.as_bytes();
    require(
        bytes.len() == 36
            && [8, 13, 18, 23]
                .iter()
                .all(|position| bytes[*position] == b'-')
            && bytes.iter().enumerate().all(|(index, byte)| {
                [8, 13, 18, 23].contains(&index)
                    || byte.is_ascii_digit()
                    || (b'a'..=b'f').contains(byte)
            })
            && bytes[14] == b'4'
            && matches!(bytes[19], b'8' | b'9' | b'a' | b'b'),
        "envelope ID must be a canonical lowercase UUIDv4",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> OffsetDateTime {
        crate::parse_timestamp("2026-08-15T12:01:00Z").unwrap()
    }

    fn fixture(sender: &CompanionIdentity, recipient: &CompanionIdentity) -> OpaqueEnvelope {
        seal_with_material(
            sender,
            &recipient.agreement_public_key(),
            EnvelopeMetadata {
                envelope_id: "11111111-1111-4111-8111-111111111111".into(),
                mailbox_id: "mailbox_fixture".into(),
                recipient_device_id: recipient.device_id().into(),
                sender_sequence: 42,
                created_at: "2026-08-15T12:00:00Z".into(),
                expires_at: "2026-08-16T12:00:00Z".into(),
            },
            br#"{"private":"workspace content"}"#,
            [31_u8; 32],
            [32_u8; 12],
        )
        .unwrap()
    }

    #[test]
    fn x25519_chacha_envelope_round_trip_and_replay_rejection() {
        let (_, sender) = CompanionIdentity::from_entropy([30_u8; 16]).unwrap();
        let (_, recipient) = CompanionIdentity::from_entropy([31_u8; 16]).unwrap();
        let envelope = fixture(&sender, &recipient);
        let mut replay = ReplayWindow::new(128).unwrap();
        assert_eq!(
            open_envelope(
                &envelope,
                &sender.signing_public_key(),
                sender.device_id(),
                &recipient,
                now(),
                &mut replay,
            )
            .unwrap(),
            br#"{"private":"workspace content"}"#
        );
        assert!(open_envelope(
            &envelope,
            &sender.signing_public_key(),
            sender.device_id(),
            &recipient,
            now(),
            &mut replay,
        )
        .is_err());
    }

    #[test]
    fn relay_tampering_and_wrong_recipient_are_rejected() {
        let (_, sender) = CompanionIdentity::from_entropy([32_u8; 16]).unwrap();
        let (_, recipient) = CompanionIdentity::from_entropy([33_u8; 16]).unwrap();
        let (_, attacker) = CompanionIdentity::from_entropy([34_u8; 16]).unwrap();
        let mut envelope = fixture(&sender, &recipient);
        envelope.ciphertext.replace_range(0..1, "A");
        assert!(open_envelope(
            &envelope,
            &sender.signing_public_key(),
            sender.device_id(),
            &recipient,
            now(),
            &mut ReplayWindow::new(128).unwrap(),
        )
        .is_err());

        let envelope = fixture(&sender, &recipient);
        assert!(open_envelope(
            &envelope,
            &sender.signing_public_key(),
            sender.device_id(),
            &attacker,
            now(),
            &mut ReplayWindow::new(128).unwrap(),
        )
        .is_err());
    }

    #[test]
    fn envelope_attack_matrix_rejects_header_signature_expiry_and_sequence_reuse() {
        let (_, sender) = CompanionIdentity::from_entropy([40_u8; 16]).unwrap();
        let (_, recipient) = CompanionIdentity::from_entropy([41_u8; 16]).unwrap();

        for mutation in ["schema", "sender", "recipient", "nonce", "signature"] {
            let mut envelope = fixture(&sender, &recipient);
            match mutation {
                "schema" => envelope.header.schema = "attacker.schema/1".into(),
                "sender" => envelope.header.sender_device_id = "device_attacker".into(),
                "recipient" => envelope.header.recipient_device_id = "device_attacker".into(),
                "nonce" => envelope.header.nonce = base64url(&[99_u8; 12]),
                "signature" => envelope.signature = base64url(&[0_u8; 64]),
                _ => unreachable!(),
            }
            assert!(
                open_envelope(
                    &envelope,
                    &sender.signing_public_key(),
                    sender.device_id(),
                    &recipient,
                    now(),
                    &mut ReplayWindow::new(128).unwrap(),
                )
                .is_err(),
                "{mutation} substitution was accepted"
            );
        }

        assert!(open_envelope(
            &fixture(&sender, &recipient),
            &sender.signing_public_key(),
            sender.device_id(),
            &recipient,
            crate::parse_timestamp("2026-08-16T12:00:31Z").unwrap(),
            &mut ReplayWindow::new(128).unwrap(),
        )
        .is_err());

        let first = fixture(&sender, &recipient);
        let second = seal_with_material(
            &sender,
            &recipient.agreement_public_key(),
            EnvelopeMetadata {
                envelope_id: "22222222-2222-4222-8222-222222222222".into(),
                mailbox_id: first.header.mailbox_id.clone(),
                recipient_device_id: recipient.device_id().into(),
                sender_sequence: first.header.sender_sequence,
                created_at: first.header.created_at.clone(),
                expires_at: first.header.expires_at.clone(),
            },
            b"different authenticated plaintext",
            [42_u8; 32],
            [43_u8; 12],
        )
        .unwrap();
        let mut replay = ReplayWindow::new(128).unwrap();
        open_envelope(
            &first,
            &sender.signing_public_key(),
            sender.device_id(),
            &recipient,
            now(),
            &mut replay,
        )
        .unwrap();
        assert!(open_envelope(
            &second,
            &sender.signing_public_key(),
            sender.device_id(),
            &recipient,
            now(),
            &mut replay,
        )
        .is_err());

        assert!(seal_with_material(
            &sender,
            &recipient.agreement_public_key(),
            EnvelopeMetadata {
                envelope_id: "33333333-3333-4333-8333-333333333333".into(),
                mailbox_id: "mailbox_fixture".into(),
                recipient_device_id: recipient.device_id().into(),
                sender_sequence: 43,
                created_at: "2026-08-15T12:00:00Z".into(),
                expires_at: "2026-08-16T12:00:00Z".into(),
            },
            &vec![0_u8; MAX_ENVELOPE_PLAINTEXT_BYTES + 1],
            [44_u8; 32],
            [45_u8; 12],
        )
        .is_err());
    }
}
