use crate::builder::MAX_SAFE_JSON_INTEGER;
use crate::canonical;
use crate::digest::{Bytes32, ShotId};
use crate::identity::InstallationIdentity;
use crate::signature::DetachedP256Signature;
use crate::text::{invalid, validate_token};
use crate::{ProtocolError, Result};
use serde::{Deserialize, Deserializer, Serialize};

pub const CONTINUITY_SCHEMA: &str = "tohseno.continuity/1";
pub const CONTINUITY_STATEMENT_SCHEMA: &str = "tohseno.continuity-statement/1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuityAudience {
    pub shot_id: ShotId,
    /// Null is the explicit Shot-wide audience. A populated value narrows the
    /// proof to one recipient installation.
    #[serde(deserialize_with = "deserialize_explicit_installation_id")]
    pub installation_id: Option<Bytes32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuityStatement {
    pub schema: String,
    pub issuer: InstallationIdentity,
    pub audience: ContinuityAudience,
    pub originating_shot_id: ShotId,
    /// Capability-like tokens only. Claims contain no arbitrary profile data.
    pub claims: Vec<String>,
    pub nonce: Bytes32,
    pub issued_at: u64,
    pub expires_at: u64,
}

impl ContinuityStatement {
    pub fn validate(&self) -> Result<()> {
        if self.schema != CONTINUITY_STATEMENT_SCHEMA {
            return Err(invalid(
                "continuity.statement.schema",
                format!("must be {CONTINUITY_STATEMENT_SCHEMA}"),
            ));
        }
        self.issuer.validate()?;
        if self.audience.shot_id.is_zero() || self.originating_shot_id.is_zero() {
            return Err(invalid(
                "continuity.shot_id",
                "audience and originating ShotIDs must not be zero",
            ));
        }
        if self.claims.is_empty() || self.claims.len() > 16 {
            return Err(invalid(
                "continuity.claims",
                "must contain 1..=16 scoped claim tokens",
            ));
        }
        for (index, claim) in self.claims.iter().enumerate() {
            validate_claim(claim)?;
            if index > 0 && self.claims[index - 1] >= *claim {
                return Err(invalid(
                    "continuity.claims",
                    "must be unique and strictly sorted in ASCII lexicographic order",
                ));
            }
        }
        if self.nonce == Bytes32::ZERO {
            return Err(invalid(
                "continuity.nonce",
                "must be a random nonzero value",
            ));
        }
        if self.issued_at == 0
            || self.expires_at <= self.issued_at
            || self.expires_at > MAX_SAFE_JSON_INTEGER
        {
            return Err(invalid(
                "continuity.expires_at",
                "must be a JavaScript-safe Unix timestamp after issued_at",
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<Bytes32> {
        self.validate()?;
        canonical::sha256_commitment(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuityEnvelope {
    pub schema: String,
    pub statement: ContinuityStatement,
    pub signature: DetachedP256Signature,
}

impl ContinuityEnvelope {
    pub fn verify_at(&self, unix_time: u64) -> Result<()> {
        if self.schema != CONTINUITY_SCHEMA {
            return Err(invalid(
                "continuity.schema",
                format!("must be {CONTINUITY_SCHEMA}"),
            ));
        }
        let digest = self.statement.digest()?;
        if self.signature.digest != digest {
            return Err(ProtocolError::DigestMismatch);
        }
        self.signature.verify(&self.statement.issuer.public_key)?;
        if unix_time < self.statement.issued_at || unix_time >= self.statement.expires_at {
            return Err(invalid(
                "continuity.expiration",
                "proof is not active at the supplied time",
            ));
        }
        Ok(())
    }
}

fn validate_claim(value: &str) -> Result<()> {
    validate_token("continuity.claim", value, 1, 64)?;
    let mut previous_separator = false;
    for (index, byte) in value.bytes().enumerate() {
        let separator = matches!(byte, b'.' | b'_' | b'-');
        if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || separator)
            || (separator && (index == 0 || previous_separator))
        {
            return Err(invalid(
                "continuity.claim",
                "must be a lowercase scoped token",
            ));
        }
        previous_separator = separator;
    }
    if previous_separator {
        return Err(invalid("continuity.claim", "must not end with punctuation"));
    }
    Ok(())
}

fn deserialize_explicit_installation_id<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Bytes32>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Bytes32>::deserialize(deserializer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signature::P256PublicKey;

    #[test]
    fn statement_requires_explicit_narrow_scope_and_expiry() {
        let statement = ContinuityStatement {
            schema: CONTINUITY_STATEMENT_SCHEMA.into(),
            issuer: InstallationIdentity {
                installation_id: Bytes32::ZERO,
                public_key: P256PublicKey {
                    x: Bytes32::ZERO,
                    y: Bytes32::ZERO,
                },
            },
            audience: ContinuityAudience {
                shot_id: ShotId::from_bytes([1; 32]),
                installation_id: None,
            },
            originating_shot_id: ShotId::from_bytes([2; 32]),
            claims: vec![],
            nonce: Bytes32::ZERO,
            issued_at: 10,
            expires_at: 10,
        };
        assert!(statement.validate().is_err());
    }
}
