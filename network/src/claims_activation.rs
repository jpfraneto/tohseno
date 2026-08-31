//! Separate release-authority activation for the additive Claims contract.
//!
//! Generation 0.8 remains the authority for BuilderAccount and ShotRegistry.
//! A Claims deployment is trusted only when this exact source/runtime/deployment
//! observation is threshold-authorized under the already pinned release policy.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tohseno_protocol::builder::MAX_SAFE_JSON_INTEGER;
use tohseno_protocol::canonical;
use tohseno_protocol::contract_activation::{
    DeploymentObservation, ReleaseAuthorityApproval, ReleaseAuthorityPolicy,
};
use tohseno_protocol::digest::{sha256, Address20, Bytes32};
use tohseno_protocol::identity::ROBINHOOD_CHAIN_ID;
use tohseno_protocol::record::CanonicalTimestamp;
use tohseno_protocol::ProtocolError;

use crate::{NetworkError, Result};

pub const CLAIMS_ACTIVATION_SCHEMA: &str = "tohseno.claims-activation/1";
pub const SIGNED_CLAIMS_ACTIVATION_SCHEMA: &str = "tohseno.signed-claims-activation/1";
pub const CLAIMS_ACTIVATION_DOMAIN: &[u8] = b"TOHSENO-CLAIMS-ACTIVATION-V1\0";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimsActivation {
    pub schema: String,
    pub protocol: String,
    pub component: String,
    pub contract_version: u64,
    pub activation_sequence: u64,
    pub previous_activation: Option<Bytes32>,
    pub authority_policy_sha256: Bytes32,
    pub chain_id: u64,
    pub claims_contract: Address20,
    pub shot_registry: Address20,
    pub creation_code_keccak256: Bytes32,
    pub runtime_code_keccak256: Bytes32,
    pub source_commit: String,
    pub source_tree_sha256: Bytes32,
    pub deployment: DeploymentObservation,
    pub issued_at: CanonicalTimestamp,
}

impl ClaimsActivation {
    pub fn validate(&self) -> Result<()> {
        if self.schema != CLAIMS_ACTIVATION_SCHEMA
            || self.protocol != "tohseno"
            || self.component != "TohsenoClaimsV1"
            || self.contract_version != 1
        {
            return invalid("Claims activation identity is invalid");
        }
        if self.activation_sequence == 0 || self.activation_sequence > MAX_SAFE_JSON_INTEGER {
            return invalid("Claims activation sequence is invalid");
        }
        match (self.activation_sequence, self.previous_activation) {
            (1, None) => {}
            (1, Some(_)) | (_, None) => return invalid("Claims activation predecessor is invalid"),
            (_, Some(value)) if value == Bytes32::ZERO => {
                return invalid("Claims activation predecessor must not be zero")
            }
            _ => {}
        }
        if self.chain_id != ROBINHOOD_CHAIN_ID {
            return invalid("Claims activation is not on Robinhood Chain");
        }
        if zero_address(self.claims_contract)
            || zero_address(self.shot_registry)
            || self.claims_contract == self.shot_registry
        {
            return invalid("Claims and ShotRegistry addresses are invalid");
        }
        for digest in [
            self.authority_policy_sha256,
            self.creation_code_keccak256,
            self.runtime_code_keccak256,
            self.source_tree_sha256,
            self.deployment.transaction_hash,
            self.deployment.block_hash,
        ] {
            if digest == Bytes32::ZERO {
                return invalid("Claims activation contains a zero digest");
            }
        }
        if self.deployment.block_number == 0 || self.deployment.block_number > MAX_SAFE_JSON_INTEGER
        {
            return invalid("Claims deployment block is invalid");
        }
        if self.source_commit.len() != 40
            || !self
                .source_commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return invalid("Claims source commit must be one lowercase Git object ID");
        }
        if self.issued_at.unix_timestamp() <= 0 {
            return invalid("Claims activation timestamp is invalid");
        }
        Ok(())
    }

    pub fn signing_digest(&self) -> Result<Bytes32> {
        self.validate()?;
        let canonical = canonical::to_vec(self)?;
        let mut preimage = Vec::with_capacity(CLAIMS_ACTIVATION_DOMAIN.len() + canonical.len());
        preimage.extend_from_slice(CLAIMS_ACTIVATION_DOMAIN);
        preimage.extend_from_slice(&canonical);
        Ok(sha256(&preimage))
    }

    pub fn validate_successor_of(&self, previous: &Self) -> Result<()> {
        self.validate()?;
        previous.validate()?;
        if self.activation_sequence != previous.activation_sequence.saturating_add(1)
            || self.previous_activation != Some(previous.signing_digest()?)
            || self.deployment.block_number <= previous.deployment.block_number
            || self.issued_at.unix_timestamp() < previous.issued_at.unix_timestamp()
        {
            return invalid("Claims activation does not exactly extend its predecessor");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedClaimsActivation {
    pub schema: String,
    pub activation: ClaimsActivation,
    pub approvals: Vec<ReleaseAuthorityApproval>,
}

impl SignedClaimsActivation {
    pub fn verify(&self, policy: &ReleaseAuthorityPolicy) -> Result<()> {
        if self.schema != SIGNED_CLAIMS_ACTIVATION_SCHEMA {
            return invalid("signed Claims activation schema is invalid");
        }
        self.activation.validate()?;
        policy.validate()?;
        if self.activation.authority_policy_sha256 != policy.digest()? {
            return Err(NetworkError::Protocol(ProtocolError::DigestMismatch));
        }
        if self.approvals.len() < policy.threshold as usize
            || self.approvals.len() > policy.authorities.len()
        {
            return invalid("Claims activation does not satisfy the authority threshold");
        }
        let digest = self.activation.signing_digest()?;
        let authorities = policy
            .authorities
            .iter()
            .map(|authority| (authority.key_id, &authority.public_key))
            .collect::<BTreeMap<_, _>>();
        let mut previous = None;
        for approval in &self.approvals {
            if previous.is_some_and(|value| value >= approval.key_id) {
                return invalid("Claims activation approvals are not strictly ordered");
            }
            previous = Some(approval.key_id);
            let key = authorities.get(&approval.key_id).ok_or_else(|| {
                NetworkError::Invalid("Claims activation contains an unknown authority".into())
            })?;
            approval.authorization.validate()?;
            if approval.authorization.digest != digest {
                return Err(NetworkError::Protocol(ProtocolError::DigestMismatch));
            }
            approval.authorization.verify(key)?;
        }
        Ok(())
    }
}

fn zero_address(value: Address20) -> bool {
    value.as_bytes().iter().all(|byte| *byte == 0)
}

fn invalid<T>(message: &str) -> Result<T> {
    Err(NetworkError::Invalid(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::hazmat::PrehashSigner;
    use p256::ecdsa::{Signature, SigningKey};
    use tohseno_protocol::contract_activation::{
        release_authority_key_id, ReleaseAuthority, ReleaseAuthorityPurpose,
        RELEASE_AUTHORITY_POLICY_SCHEMA,
    };
    use tohseno_protocol::signature::{
        DetachedP256Signature, P256PublicKey, P256Signature, SignatureAlgorithm,
    };

    fn key(seed: u8) -> (SigningKey, ReleaseAuthority) {
        let signing = SigningKey::from_bytes((&[seed; 32]).into()).expect("signing key");
        let point = signing.verifying_key().to_encoded_point(false);
        let mut x = [0_u8; 32];
        x.copy_from_slice(point.x().expect("x"));
        let mut y = [0_u8; 32];
        y.copy_from_slice(point.y().expect("y"));
        let public_key = P256PublicKey {
            x: Bytes32::new(x),
            y: Bytes32::new(y),
        };
        let authority = ReleaseAuthority {
            key_id: release_authority_key_id(&public_key),
            public_key,
        };
        (signing, authority)
    }

    fn policy() -> (ReleaseAuthorityPolicy, Vec<SigningKey>) {
        let pairs = [key(1), key(2), key(3)];
        let mut keyed = pairs.into_iter().collect::<Vec<_>>();
        keyed.sort_by_key(|(_, authority)| authority.key_id);
        let keys = keyed.iter().map(|(key, _)| key.clone()).collect();
        let authorities = keyed.into_iter().map(|(_, authority)| authority).collect();
        (
            ReleaseAuthorityPolicy {
                schema: RELEASE_AUTHORITY_POLICY_SCHEMA.into(),
                protocol: "tohseno".into(),
                protocol_major: 2,
                purpose: ReleaseAuthorityPurpose::ContractGenerationActivation,
                threshold: 2,
                authorities,
                issued_at: CanonicalTimestamp::parse("2026-08-30T12:00:00Z").expect("timestamp"),
            },
            keys,
        )
    }

    fn activation(policy: &ReleaseAuthorityPolicy) -> ClaimsActivation {
        ClaimsActivation {
            schema: CLAIMS_ACTIVATION_SCHEMA.into(),
            protocol: "tohseno".into(),
            component: "TohsenoClaimsV1".into(),
            contract_version: 1,
            activation_sequence: 1,
            previous_activation: None,
            authority_policy_sha256: policy.digest().expect("policy digest"),
            chain_id: ROBINHOOD_CHAIN_ID,
            claims_contract: Address20::from_bytes([0x66; 20]),
            shot_registry: Address20::from_bytes([0x77; 20]),
            creation_code_keccak256: Bytes32::new([0x11; 32]),
            runtime_code_keccak256: Bytes32::new([0x22; 32]),
            source_commit: "a".repeat(40),
            source_tree_sha256: Bytes32::new([0x33; 32]),
            deployment: DeploymentObservation {
                transaction_hash: Bytes32::new([0x44; 32]),
                block_number: 12_345,
                block_hash: Bytes32::new([0x55; 32]),
            },
            issued_at: CanonicalTimestamp::parse("2026-08-30T13:00:00Z").expect("timestamp"),
        }
    }

    fn approval(
        key: &SigningKey,
        authority: &ReleaseAuthority,
        digest: Bytes32,
    ) -> ReleaseAuthorityApproval {
        let signature: Signature = key.sign_prehash(digest.as_bytes()).expect("signature");
        let signature = signature.normalize_s().unwrap_or(signature);
        ReleaseAuthorityApproval {
            key_id: authority.key_id,
            authorization: DetachedP256Signature {
                algorithm: SignatureAlgorithm::P256,
                digest,
                signature: P256Signature {
                    r: Bytes32::new(signature.r().to_bytes().into()),
                    s: Bytes32::new(signature.s().to_bytes().into()),
                },
                low_s: true,
            },
        }
    }

    #[test]
    fn threshold_activation_binds_source_runtime_registry_and_deployment() {
        let (policy, keys) = policy();
        let activation = activation(&policy);
        let digest = activation.signing_digest().expect("activation digest");
        let approvals = policy
            .authorities
            .iter()
            .zip(keys.iter())
            .take(2)
            .map(|(authority, key)| approval(key, authority, digest))
            .collect();
        let signed = SignedClaimsActivation {
            schema: SIGNED_CLAIMS_ACTIVATION_SCHEMA.into(),
            activation,
            approvals,
        };
        signed.verify(&policy).expect("verified activation");

        let mut substituted = signed.clone();
        substituted.activation.shot_registry = Address20::from_bytes([0x88; 20]);
        assert!(substituted.verify(&policy).is_err());
    }

    #[test]
    fn partial_unknown_reordered_and_replayed_approvals_fail_closed() {
        let (policy, keys) = policy();
        let activation = activation(&policy);
        let digest = activation.signing_digest().expect("activation digest");
        let one = approval(&keys[0], &policy.authorities[0], digest);
        let two = approval(&keys[1], &policy.authorities[1], digest);

        let partial = SignedClaimsActivation {
            schema: SIGNED_CLAIMS_ACTIVATION_SCHEMA.into(),
            activation: activation.clone(),
            approvals: vec![one.clone()],
        };
        assert!(partial.verify(&policy).is_err());

        let reversed = SignedClaimsActivation {
            schema: SIGNED_CLAIMS_ACTIVATION_SCHEMA.into(),
            activation: activation.clone(),
            approvals: vec![two.clone(), one.clone()],
        };
        assert!(reversed.verify(&policy).is_err());

        let mut replayed = one;
        replayed.key_id = Bytes32::new([0xff; 32]);
        let unknown = SignedClaimsActivation {
            schema: SIGNED_CLAIMS_ACTIVATION_SCHEMA.into(),
            activation,
            approvals: vec![two, replayed],
        };
        assert!(unknown.verify(&policy).is_err());
    }
}
