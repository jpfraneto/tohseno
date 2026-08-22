//! Verification for private, server-signed TOHSENO Pro receipts.
//!
//! Receipt bytes are deliberately outside public protocol lineage. The billing
//! server signs one canonical payload and the local factory verifies it with a
//! separately configured, pinned P-256 public key before entitlement changes.

use crate::entitlement::{SubscriptionPlan, VerifiedSubscription};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub const RECEIPT_SCHEMA: &str = "tohseno.private-entitlement-receipt/1";
pub const RECEIPT_ENVELOPE_SCHEMA: &str = "tohseno.private-entitlement-envelope/1";
const INSTALLATION_DOMAIN: &[u8] = b"tohseno.billing.installation.v1\0";
const MAX_RECEIPT_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntitlementReceiptPayload {
    pub schema: String,
    pub receipt_id: String,
    pub entitlement_id: String,
    pub installation_binding: String,
    pub plan: SubscriptionPlan,
    pub issued_at: String,
    pub paid_through: String,
    pub cancellation_at_period_end: bool,
    pub provider_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedEntitlementReceipt {
    pub schema: String,
    pub payload_base64url: String,
    pub signature_base64url: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ReceiptError {
    Invalid(&'static str),
    WrongInstallation,
    Signature,
}

impl std::fmt::Display for ReceiptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::WrongInstallation => {
                formatter.write_str("entitlement receipt belongs to another installation")
            }
            Self::Signature => formatter.write_str("entitlement receipt signature is invalid"),
        }
    }
}

impl std::error::Error for ReceiptError {}

pub fn installation_binding(installation_id: &str) -> Result<String, ReceiptError> {
    if installation_id.is_empty()
        || installation_id.len() > 128
        || !installation_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ReceiptError::Invalid("installation identity is invalid"));
    }
    let mut digest = Sha256::new();
    digest.update(INSTALLATION_DOMAIN);
    digest.update(installation_id.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(digest.finalize()))
}

pub fn verify_receipt(
    envelope_bytes: &[u8],
    pinned_sec1_public_key_base64url: &str,
    installation_id: &str,
    now: OffsetDateTime,
) -> Result<VerifiedSubscription, ReceiptError> {
    if envelope_bytes.is_empty() || envelope_bytes.len() > MAX_RECEIPT_BYTES {
        return Err(ReceiptError::Invalid(
            "entitlement receipt is empty or oversized",
        ));
    }
    let envelope: SignedEntitlementReceipt = serde_json::from_slice(envelope_bytes)
        .map_err(|_| ReceiptError::Invalid("entitlement receipt envelope is invalid"))?;
    if envelope.schema != RECEIPT_ENVELOPE_SCHEMA {
        return Err(ReceiptError::Invalid(
            "entitlement receipt envelope schema is unsupported",
        ));
    }
    let payload = URL_SAFE_NO_PAD
        .decode(&envelope.payload_base64url)
        .map_err(|_| ReceiptError::Invalid("entitlement receipt payload encoding is invalid"))?;
    if payload.is_empty() || payload.len() > MAX_RECEIPT_BYTES {
        return Err(ReceiptError::Invalid(
            "entitlement receipt payload is empty or oversized",
        ));
    }
    let receipt: EntitlementReceiptPayload = serde_json::from_slice(&payload)
        .map_err(|_| ReceiptError::Invalid("entitlement receipt payload is invalid"))?;
    let canonical = tohseno_protocol::canonical::to_vec(&receipt)
        .map_err(|_| ReceiptError::Invalid("entitlement receipt payload is not canonical"))?;
    if canonical != payload || receipt.schema != RECEIPT_SCHEMA {
        return Err(ReceiptError::Invalid(
            "entitlement receipt payload is noncanonical",
        ));
    }
    validate_identifier(&receipt.receipt_id, "receipt identifier is invalid")?;
    validate_identifier(&receipt.entitlement_id, "entitlement identifier is invalid")?;
    if receipt.provider_revision == 0 {
        return Err(ReceiptError::Invalid(
            "entitlement provider revision is invalid",
        ));
    }
    if receipt.installation_binding != installation_binding(installation_id)? {
        return Err(ReceiptError::WrongInstallation);
    }
    let public_key = URL_SAFE_NO_PAD
        .decode(pinned_sec1_public_key_base64url)
        .map_err(|_| ReceiptError::Invalid("billing verification key encoding is invalid"))?;
    let verifying_key = VerifyingKey::from_sec1_bytes(&public_key)
        .map_err(|_| ReceiptError::Invalid("billing verification key is invalid"))?;
    let signature_bytes = URL_SAFE_NO_PAD
        .decode(&envelope.signature_base64url)
        .map_err(|_| ReceiptError::Invalid("entitlement receipt signature encoding is invalid"))?;
    let signature = Signature::from_slice(&signature_bytes).map_err(|_| ReceiptError::Signature)?;
    verifying_key
        .verify(&payload, &signature)
        .map_err(|_| ReceiptError::Signature)?;

    let issued_at = parse_timestamp(&receipt.issued_at)?;
    let paid_through = parse_timestamp(&receipt.paid_through)?;
    if issued_at > now || paid_through <= now || paid_through <= issued_at {
        return Err(ReceiptError::Invalid(
            "entitlement receipt period is invalid",
        ));
    }
    let receipt_digest = hex_sha256(envelope_bytes);
    Ok(VerifiedSubscription {
        entitlement_id: receipt.entitlement_id,
        plan: receipt.plan,
        issued_at: receipt.issued_at,
        paid_through: receipt.paid_through,
        cancellation_at_period_end: receipt.cancellation_at_period_end,
        provider_revision: receipt.provider_revision,
        receipt_digest,
    })
}

fn parse_timestamp(value: &str) -> Result<OffsetDateTime, ReceiptError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| ReceiptError::Invalid("entitlement receipt timestamp is invalid"))
}

fn validate_identifier(value: &str, error: &'static str) -> Result<(), ReceiptError> {
    if value.is_empty()
        || value.len() > 160
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ReceiptError::Invalid(error));
    }
    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::Signer;
    use p256::ecdsa::{Signature, SigningKey};
    use time::Duration;

    fn fixture() -> (Vec<u8>, String, OffsetDateTime) {
        let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
        let key = SigningKey::from_bytes((&[7_u8; 32]).into()).unwrap();
        let payload = EntitlementReceiptPayload {
            schema: RECEIPT_SCHEMA.into(),
            receipt_id: "receipt_fixture_1".into(),
            entitlement_id: "entitlement_fixture_1".into(),
            installation_binding: installation_binding("workspace_fixture").unwrap(),
            plan: SubscriptionPlan::Yearly,
            issued_at: (now - Duration::hours(1)).format(&Rfc3339).unwrap(),
            paid_through: (now + Duration::days(365)).format(&Rfc3339).unwrap(),
            cancellation_at_period_end: false,
            provider_revision: 1,
        };
        let payload = tohseno_protocol::canonical::to_vec(&payload).unwrap();
        let signature: Signature = key.sign(&payload);
        let envelope = SignedEntitlementReceipt {
            schema: RECEIPT_ENVELOPE_SCHEMA.into(),
            payload_base64url: URL_SAFE_NO_PAD.encode(&payload),
            signature_base64url: URL_SAFE_NO_PAD.encode(signature.to_bytes()),
        };
        let public = key.verifying_key().to_encoded_point(true);
        (
            serde_json::to_vec(&envelope).unwrap(),
            URL_SAFE_NO_PAD.encode(public.as_bytes()),
            now,
        )
    }

    #[test]
    fn authentic_receipt_unlocks_only_the_bound_installation() {
        let (bytes, public, now) = fixture();
        let subscription = verify_receipt(&bytes, &public, "workspace_fixture", now).unwrap();
        assert_eq!(subscription.plan, SubscriptionPlan::Yearly);
        assert!(matches!(
            verify_receipt(&bytes, &public, "workspace_other", now),
            Err(ReceiptError::WrongInstallation)
        ));
    }

    #[test]
    fn tamper_wrong_key_expiry_and_unknown_fields_fail_closed() {
        let (bytes, public, now) = fixture();
        let mut tampered = bytes.clone();
        let index = tampered.iter().position(|byte| *byte == b'A').unwrap();
        tampered[index] = b'B';
        assert!(verify_receipt(&tampered, &public, "workspace_fixture", now).is_err());

        let wrong = SigningKey::from_bytes((&[8_u8; 32]).into()).unwrap();
        let wrong_public = URL_SAFE_NO_PAD.encode(wrong.verifying_key().to_encoded_point(true));
        assert_eq!(
            verify_receipt(&bytes, &wrong_public, "workspace_fixture", now),
            Err(ReceiptError::Signature)
        );
        assert!(verify_receipt(
            &bytes,
            &public,
            "workspace_fixture",
            now + Duration::days(366)
        )
        .is_err());

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["unexpected"] = serde_json::json!(true);
        assert!(verify_receipt(
            &serde_json::to_vec(&value).unwrap(),
            &public,
            "workspace_fixture",
            now
        )
        .is_err());
    }
}
