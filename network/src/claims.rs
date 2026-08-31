use serde::{Deserialize, Serialize};
use thiserror::Error;
use tohseno_protocol::actions::{eip712_digest, keccak256, type_hash, Eip712Domain};
use tohseno_protocol::builder::MAX_SAFE_JSON_INTEGER;
use tohseno_protocol::digest::{Address20, Bytes32, ShotId};
use tohseno_protocol::identity::ROBINHOOD_CHAIN_ID;

pub const CLAIMS_DOMAIN: &str = "TOHSENO Claims";
pub const CLAIMS_EIP712_VERSION: &str = "1";
pub const OPEN_CLAIM_EDITION_TYPE: &str = "OpenClaimEdition(address shotRegistry,bytes32 shotId,uint64 maxClaims,uint64 closesAt,address controller,uint64 nonce,uint64 deadline)";
pub const CLAIM_SOFTWARE_TYPE: &str = "ClaimSoftware(address shotRegistry,bytes32 shotId,address claimant,bytes32 releaseDigest,bytes32 checkpointDigest,bytes32 gestureCommitment,uint64 nonce,uint64 deadline)";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimEditionPolicy {
    pub max_claims: u64,
    pub closes_at: u64,
}

impl ClaimEditionPolicy {
    pub const OPEN: Self = Self {
        max_claims: 0,
        closes_at: 0,
    };

    pub fn validate(self) -> Result<(), ClaimsEncodingError> {
        safe_u64("max_claims", self.max_claims)?;
        safe_u64("closes_at", self.closes_at)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenClaimEditionAction {
    pub shot_registry: Address20,
    pub shot_id: ShotId,
    pub max_claims: u64,
    pub closes_at: u64,
    pub controller: Address20,
    pub nonce: u64,
    pub deadline: u64,
}

impl OpenClaimEditionAction {
    pub fn validate(&self, active_registry: Address20) -> Result<(), ClaimsEncodingError> {
        nonzero_address("shot_registry", self.shot_registry)?;
        if self.shot_registry != active_registry {
            return Err(ClaimsEncodingError::Invalid("shot_registry"));
        }
        if self.shot_id.is_zero() {
            return Err(ClaimsEncodingError::Invalid("shot_id"));
        }
        nonzero_address("controller", self.controller)?;
        ClaimEditionPolicy {
            max_claims: self.max_claims,
            closes_at: self.closes_at,
        }
        .validate()?;
        safe_u64("nonce", self.nonce)?;
        positive_safe_u64("deadline", self.deadline)
    }

    pub fn struct_hash(&self, active_registry: Address20) -> Result<Bytes32, ClaimsEncodingError> {
        self.validate(active_registry)?;
        Ok(hash_words(&[
            type_hash(OPEN_CLAIM_EDITION_TYPE),
            address_word(self.shot_registry),
            self.shot_id.bytes(),
            u64_word(self.max_claims),
            u64_word(self.closes_at),
            address_word(self.controller),
            u64_word(self.nonce),
            u64_word(self.deadline),
        ]))
    }

    pub fn digest(
        &self,
        domain: &Eip712Domain,
        active_registry: Address20,
    ) -> Result<Bytes32, ClaimsEncodingError> {
        validate_domain(domain)?;
        Ok(eip712_digest(
            domain.separator(),
            self.struct_hash(active_registry)?,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimSoftwareAction {
    pub shot_registry: Address20,
    pub shot_id: ShotId,
    pub claimant: Address20,
    pub release_digest: Bytes32,
    pub checkpoint_digest: Bytes32,
    pub gesture_commitment: Bytes32,
    pub nonce: u64,
    pub deadline: u64,
}

impl ClaimSoftwareAction {
    pub fn validate(&self, active_registry: Address20) -> Result<(), ClaimsEncodingError> {
        nonzero_address("shot_registry", self.shot_registry)?;
        if self.shot_registry != active_registry {
            return Err(ClaimsEncodingError::Invalid("shot_registry"));
        }
        if self.shot_id.is_zero() {
            return Err(ClaimsEncodingError::Invalid("shot_id"));
        }
        nonzero_address("claimant", self.claimant)?;
        nonzero_bytes32("release_digest", self.release_digest)?;
        nonzero_bytes32("checkpoint_digest", self.checkpoint_digest)?;
        nonzero_bytes32("gesture_commitment", self.gesture_commitment)?;
        safe_u64("nonce", self.nonce)?;
        positive_safe_u64("deadline", self.deadline)
    }

    pub fn struct_hash(&self, active_registry: Address20) -> Result<Bytes32, ClaimsEncodingError> {
        self.validate(active_registry)?;
        Ok(hash_words(&[
            type_hash(CLAIM_SOFTWARE_TYPE),
            address_word(self.shot_registry),
            self.shot_id.bytes(),
            address_word(self.claimant),
            self.release_digest,
            self.checkpoint_digest,
            self.gesture_commitment,
            u64_word(self.nonce),
            u64_word(self.deadline),
        ]))
    }

    pub fn digest(
        &self,
        domain: &Eip712Domain,
        active_registry: Address20,
    ) -> Result<Bytes32, ClaimsEncodingError> {
        validate_domain(domain)?;
        Ok(eip712_digest(
            domain.separator(),
            self.struct_hash(active_registry)?,
        ))
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ClaimsEncodingError {
    #[error("{0} is invalid for Tohseno Claims v1")]
    Invalid(&'static str),
}

fn validate_domain(domain: &Eip712Domain) -> Result<(), ClaimsEncodingError> {
    if domain.name != CLAIMS_DOMAIN {
        return Err(ClaimsEncodingError::Invalid("domain.name"));
    }
    if domain.version != CLAIMS_EIP712_VERSION {
        return Err(ClaimsEncodingError::Invalid("domain.version"));
    }
    if domain.chain_id != ROBINHOOD_CHAIN_ID {
        return Err(ClaimsEncodingError::Invalid("domain.chain_id"));
    }
    nonzero_address("domain.verifying_contract", domain.verifying_contract)
}

fn nonzero_address(field: &'static str, address: Address20) -> Result<(), ClaimsEncodingError> {
    if address.as_bytes().iter().all(|byte| *byte == 0) {
        return Err(ClaimsEncodingError::Invalid(field));
    }
    Ok(())
}

fn nonzero_bytes32(field: &'static str, value: Bytes32) -> Result<(), ClaimsEncodingError> {
    if value == Bytes32::ZERO {
        return Err(ClaimsEncodingError::Invalid(field));
    }
    Ok(())
}

fn positive_safe_u64(field: &'static str, value: u64) -> Result<(), ClaimsEncodingError> {
    if value == 0 {
        return Err(ClaimsEncodingError::Invalid(field));
    }
    safe_u64(field, value)
}

fn safe_u64(field: &'static str, value: u64) -> Result<(), ClaimsEncodingError> {
    if value > MAX_SAFE_JSON_INTEGER {
        return Err(ClaimsEncodingError::Invalid(field));
    }
    Ok(())
}

fn hash_words(words: &[Bytes32]) -> Bytes32 {
    let mut bytes = Vec::with_capacity(words.len() * 32);
    for word in words {
        bytes.extend_from_slice(word.as_bytes());
    }
    keccak256(&bytes)
}

fn address_word(address: Address20) -> Bytes32 {
    let mut bytes = [0_u8; 32];
    bytes[12..].copy_from_slice(address.as_bytes());
    Bytes32::new(bytes)
}

fn u64_word(value: u64) -> Bytes32 {
    let mut bytes = [0_u8; 32];
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    Bytes32::new(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes32(byte: u8) -> Bytes32 {
        Bytes32::new([byte; 32])
    }

    fn address(byte: u8) -> Address20 {
        Address20::from_bytes([byte; 20])
    }

    fn domain() -> Eip712Domain {
        Eip712Domain {
            name: CLAIMS_DOMAIN.into(),
            version: CLAIMS_EIP712_VERSION.into(),
            chain_id: ROBINHOOD_CHAIN_ID,
            verifying_contract: address(0x66),
        }
    }

    #[test]
    fn actions_are_bound_to_claims_contract_registry_and_chain() {
        let registry = address(0x3f);
        let action = OpenClaimEditionAction {
            shot_registry: registry,
            shot_id: ShotId::from_bytes([0x11; 32]),
            max_claims: 888,
            closes_at: 2_000_000_000,
            controller: address(0x22),
            nonce: 7,
            deadline: 2_000_000_100,
        };
        let digest = action.digest(&domain(), registry).expect("digest");
        let mut wrong_chain = domain();
        wrong_chain.chain_id += 1;
        assert_eq!(
            action.digest(&wrong_chain, registry),
            Err(ClaimsEncodingError::Invalid("domain.chain_id"))
        );
        assert_eq!(
            action.digest(&domain(), address(0x40)),
            Err(ClaimsEncodingError::Invalid("shot_registry"))
        );
        assert_ne!(digest, Bytes32::ZERO);
    }

    #[test]
    fn claim_binds_the_complete_encounter() {
        let registry = address(0x3f);
        let action = ClaimSoftwareAction {
            shot_registry: registry,
            shot_id: ShotId::from_bytes([0x11; 32]),
            claimant: address(0x44),
            release_digest: bytes32(0x55),
            checkpoint_digest: bytes32(0x77),
            gesture_commitment: bytes32(0x88),
            nonce: 9,
            deadline: 2_000_000_100,
        };
        let digest = action.digest(&domain(), registry).expect("digest");
        let mut changed = action.clone();
        changed.release_digest = bytes32(0x56);
        assert_ne!(
            digest,
            changed.digest(&domain(), registry).expect("changed")
        );
    }

    #[test]
    fn frozen_vectors_match_rust_action_encoding() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../fixtures/claim-actions-v1.json"))
                .expect("fixture");
        let registry = Address20::from_bytes([
            0x3f, 0xe6, 0x50, 0x8b, 0xa2, 0x66, 0x0b, 0xc5, 0x75, 0x08, 0x00, 0x24, 0xf4, 0x02,
            0xc1, 0x92, 0xa2, 0xe0, 0x35, 0xa0,
        ]);
        let domain = domain();
        let open = OpenClaimEditionAction {
            shot_registry: registry,
            shot_id: ShotId::from_bytes([0x11; 32]),
            max_claims: 888,
            closes_at: 2_000_000_000,
            controller: address(0x22),
            nonce: 7,
            deadline: 2_000_000_100,
        };
        let claim = ClaimSoftwareAction {
            shot_registry: registry,
            shot_id: ShotId::from_bytes([0x11; 32]),
            claimant: address(0x44),
            release_digest: bytes32(0x55),
            checkpoint_digest: bytes32(0x77),
            gesture_commitment: Bytes32::from_hex(
                "gesture_commitment",
                "0x23ff9441e61d47a40c542827940bf16cf1f96311e8435c0b8920e97e97861e87",
            )
            .expect("gesture"),
            nonce: 9,
            deadline: 2_000_000_100,
        };
        assert_eq!(fixture["domain_separator"], domain.separator().to_hex());
        assert_eq!(
            fixture["open_claim_edition"]["type_hash"],
            type_hash(OPEN_CLAIM_EDITION_TYPE).to_hex()
        );
        assert_eq!(
            fixture["open_claim_edition"]["struct_hash"],
            open.struct_hash(registry).expect("open struct").to_hex()
        );
        assert_eq!(
            fixture["open_claim_edition"]["digest"],
            open.digest(&domain, registry)
                .expect("open digest")
                .to_hex()
        );
        assert_eq!(
            fixture["claim_software"]["type_hash"],
            type_hash(CLAIM_SOFTWARE_TYPE).to_hex()
        );
        assert_eq!(
            fixture["claim_software"]["struct_hash"],
            claim.struct_hash(registry).expect("claim struct").to_hex()
        );
        assert_eq!(
            fixture["claim_software"]["digest"],
            claim
                .digest(&domain, registry)
                .expect("claim digest")
                .to_hex()
        );
    }
}
