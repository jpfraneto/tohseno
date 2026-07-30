//! Off-chain authorization records for activating immutable contract builds.
//!
//! These types deliberately do not install a trust root. They can prove that
//! a threshold of keys in a supplied release policy approved exact activation
//! evidence. A client must separately decide which policy digest it trusts.

use crate::builder::MAX_SAFE_JSON_INTEGER;
use crate::canonical;
use crate::contract_generation::{ContractGeneration, CONTRACT_GENERATION_PROTOCOL_MAJOR};
use crate::digest::{sha256, Address20, Bytes32};
use crate::record::CanonicalTimestamp;
use crate::signature::{DetachedP256Signature, P256PublicKey};
use crate::text::invalid;
use crate::{ProtocolError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const RELEASE_AUTHORITY_POLICY_SCHEMA: &str = "tohseno.release-authority-policy/1";
pub const CONTRACT_ACTIVATION_SCHEMA: &str = "tohseno.contract-activation/1";
pub const SIGNED_CONTRACT_ACTIVATION_SCHEMA: &str = "tohseno.signed-contract-activation/1";
pub const CONTRACT_ACTIVATION_PROTOCOL: &str = "tohseno";
pub const CONTRACT_ACTIVATION_SIGNING_DOMAIN: &[u8] = b"TOHSENO-CONTRACT-ACTIVATION-V1\0";
pub const RELEASE_AUTHORITY_KEY_DOMAIN: &[u8] = b"TOHSENO-RELEASE-AUTHORITY-KEY-V1\0";
const MAX_RELEASE_AUTHORITIES: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseAuthorityPurpose {
    ContractGenerationActivation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthorityPolicy {
    pub schema: String,
    pub protocol: String,
    pub protocol_major: u64,
    pub purpose: ReleaseAuthorityPurpose,
    pub threshold: u64,
    pub authorities: Vec<ReleaseAuthority>,
    pub issued_at: CanonicalTimestamp,
}

impl ReleaseAuthorityPolicy {
    pub fn validate(&self) -> Result<()> {
        if self.schema != RELEASE_AUTHORITY_POLICY_SCHEMA {
            return Err(invalid(
                "release_authority_policy.schema",
                format!("must be {RELEASE_AUTHORITY_POLICY_SCHEMA}"),
            ));
        }
        if self.protocol != CONTRACT_ACTIVATION_PROTOCOL {
            return Err(invalid(
                "release_authority_policy.protocol",
                format!("must be {CONTRACT_ACTIVATION_PROTOCOL}"),
            ));
        }
        if self.protocol_major != CONTRACT_GENERATION_PROTOCOL_MAJOR {
            return Err(invalid(
                "release_authority_policy.protocol_major",
                format!("must be {CONTRACT_GENERATION_PROTOCOL_MAJOR}"),
            ));
        }
        if self.authorities.is_empty() || self.authorities.len() > MAX_RELEASE_AUTHORITIES {
            return Err(invalid(
                "release_authority_policy.authorities",
                format!("must contain 1..={MAX_RELEASE_AUTHORITIES} keys"),
            ));
        }
        if self.threshold == 0
            || self.threshold > self.authorities.len() as u64
            || self.threshold > MAX_SAFE_JSON_INTEGER
        {
            return Err(invalid(
                "release_authority_policy.threshold",
                "must be between one and the authority count",
            ));
        }
        let mut previous = None;
        for authority in &self.authorities {
            authority.validate()?;
            if previous.is_some_and(|value| value >= authority.key_id) {
                return Err(invalid(
                    "release_authority_policy.authorities",
                    "key IDs must be unique and strictly ordered",
                ));
            }
            previous = Some(authority.key_id);
        }
        if self.issued_at.unix_timestamp() <= 0 {
            return Err(invalid(
                "release_authority_policy.issued_at",
                "must be after the Unix epoch",
            ));
        }
        Ok(())
    }

    /// SHA-256 of the exact RFC 8785 policy bytes.
    pub fn digest(&self) -> Result<Bytes32> {
        self.validate()?;
        canonical::sha256_commitment(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthority {
    pub key_id: Bytes32,
    pub public_key: P256PublicKey,
}

impl ReleaseAuthority {
    pub fn from_public_key(public_key: P256PublicKey) -> Result<Self> {
        public_key.validate()?;
        Ok(Self {
            key_id: release_authority_key_id(&public_key),
            public_key,
        })
    }

    pub fn validate(&self) -> Result<()> {
        self.public_key.validate()?;
        if self.key_id != release_authority_key_id(&self.public_key) {
            return Err(invalid(
                "release_authority.key_id",
                "must use the dedicated release-authority key derivation",
            ));
        }
        Ok(())
    }
}

/// Release-authority key IDs are deliberately distinct from DeviceKey IDs.
pub fn release_authority_key_id(public_key: &P256PublicKey) -> Bytes32 {
    let mut preimage = Vec::with_capacity(RELEASE_AUTHORITY_KEY_DOMAIN.len() + 64);
    preimage.extend_from_slice(RELEASE_AUTHORITY_KEY_DOMAIN);
    preimage.extend_from_slice(public_key.x.as_bytes());
    preimage.extend_from_slice(public_key.y.as_bytes());
    sha256(&preimage)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractActivation {
    pub schema: String,
    pub protocol: String,
    pub protocol_major: u64,
    pub generation: String,
    pub activation_sequence: u64,
    pub previous_activation: Option<Bytes32>,
    pub generation_definition_sha256: Bytes32,
    pub authority_policy_sha256: Bytes32,
    pub chain_id: u64,
    pub builder_account_runtime_keccak256: Bytes32,
    pub factory: ActivatedContract,
    pub registry: ActivatedContract,
    pub activation_block: ChainBlock,
    pub p256_probe_sha256: Bytes32,
    pub issued_at: CanonicalTimestamp,
}

impl ContractActivation {
    pub fn validate(&self) -> Result<()> {
        if self.schema != CONTRACT_ACTIVATION_SCHEMA {
            return Err(invalid(
                "contract_activation.schema",
                format!("must be {CONTRACT_ACTIVATION_SCHEMA}"),
            ));
        }
        if self.protocol != CONTRACT_ACTIVATION_PROTOCOL {
            return Err(invalid(
                "contract_activation.protocol",
                format!("must be {CONTRACT_ACTIVATION_PROTOCOL}"),
            ));
        }
        if self.protocol_major != CONTRACT_GENERATION_PROTOCOL_MAJOR {
            return Err(invalid(
                "contract_activation.protocol_major",
                format!("must be {CONTRACT_GENERATION_PROTOCOL_MAJOR}"),
            ));
        }
        validate_version("contract_activation.generation", &self.generation)?;
        if self.activation_sequence == 0 || self.activation_sequence > MAX_SAFE_JSON_INTEGER {
            return Err(invalid(
                "contract_activation.activation_sequence",
                "must be a positive JavaScript-safe integer",
            ));
        }
        match (self.activation_sequence, self.previous_activation) {
            (1, None) => {}
            (1, Some(_)) => {
                return Err(invalid(
                    "contract_activation.previous_activation",
                    "activation one must not claim a predecessor",
                ))
            }
            (_, None) => {
                return Err(invalid(
                    "contract_activation.previous_activation",
                    "a successor activation must name its predecessor",
                ))
            }
            (_, Some(value)) if value == Bytes32::ZERO => {
                return Err(invalid(
                    "contract_activation.previous_activation",
                    "must not be zero",
                ))
            }
            _ => {}
        }
        for (field, digest) in [
            (
                "contract_activation.generation_definition_sha256",
                self.generation_definition_sha256,
            ),
            (
                "contract_activation.authority_policy_sha256",
                self.authority_policy_sha256,
            ),
            (
                "contract_activation.builder_account_runtime_keccak256",
                self.builder_account_runtime_keccak256,
            ),
            (
                "contract_activation.p256_probe_sha256",
                self.p256_probe_sha256,
            ),
        ] {
            if digest == Bytes32::ZERO {
                return Err(invalid(field, "must not be zero"));
            }
        }
        if self.chain_id == 0 || self.chain_id > MAX_SAFE_JSON_INTEGER {
            return Err(invalid(
                "contract_activation.chain_id",
                "must be a positive JavaScript-safe integer",
            ));
        }
        self.factory.validate("contract_activation.factory")?;
        self.registry.validate("contract_activation.registry")?;
        if self.factory.address == self.registry.address {
            return Err(invalid(
                "contract_activation.registry.address",
                "must differ from the factory",
            ));
        }
        self.activation_block
            .validate("contract_activation.activation_block")?;
        if self.factory.deployment.block_number > self.activation_block.block_number
            || self.registry.deployment.block_number > self.activation_block.block_number
        {
            return Err(invalid(
                "contract_activation.activation_block",
                "must be at or after both deployment observations",
            ));
        }
        if self.issued_at.unix_timestamp() <= 0 {
            return Err(invalid(
                "contract_activation.issued_at",
                "must be after the Unix epoch",
            ));
        }
        Ok(())
    }

    /// Domain-separated digest signed by release-authority keys and named by a
    /// successor's `previous_activation`.
    pub fn signing_digest(&self) -> Result<Bytes32> {
        self.validate()?;
        let canonical = canonical::to_vec(self)?;
        let mut preimage =
            Vec::with_capacity(CONTRACT_ACTIVATION_SIGNING_DOMAIN.len() + canonical.len());
        preimage.extend_from_slice(CONTRACT_ACTIVATION_SIGNING_DOMAIN);
        preimage.extend_from_slice(&canonical);
        Ok(sha256(&preimage))
    }

    pub fn validate_against_generation(&self, generation: &ContractGeneration) -> Result<()> {
        self.validate()?;
        generation.validate()?;
        if self.protocol_major != generation.protocol_major
            || self.generation != generation.generation
            || self.chain_id != generation.chain.chain_id
            || self.generation_definition_sha256 != generation.digest()?
        {
            return Err(invalid(
                "contract_activation.generation_definition_sha256",
                "does not bind the supplied generation exactly",
            ));
        }
        if self.builder_account_runtime_keccak256
            != generation.contracts.builder_account.runtime_code_keccak256
        {
            return Err(invalid(
                "contract_activation.builder_account_runtime_keccak256",
                "does not match the generation",
            ));
        }
        if self.factory.address != generation.create2.builder_account_factory.predicted_address
            || self.factory.runtime_code_keccak256
                != generation
                    .contracts
                    .builder_account_factory
                    .runtime_code_keccak256
        {
            return Err(invalid(
                "contract_activation.factory",
                "does not match the generation",
            ));
        }
        if self.registry.address != generation.create2.shot_registry.predicted_address
            || self.registry.runtime_code_keccak256
                != generation.contracts.shot_registry.runtime_code_keccak256
        {
            return Err(invalid(
                "contract_activation.registry",
                "does not match the generation",
            ));
        }
        Ok(())
    }

    pub fn validate_successor_of(&self, previous: &Self) -> Result<()> {
        self.validate()?;
        previous.validate()?;
        let expected_sequence = previous
            .activation_sequence
            .checked_add(1)
            .ok_or_else(|| invalid("contract_activation.activation_sequence", "overflowed"))?;
        if self.activation_sequence != expected_sequence
            || self.previous_activation != Some(previous.signing_digest()?)
        {
            return Err(invalid(
                "contract_activation.previous_activation",
                "does not extend the prior activation exactly",
            ));
        }
        if self.activation_block.block_number <= previous.activation_block.block_number
            || self.issued_at.unix_timestamp() < previous.issued_at.unix_timestamp()
        {
            return Err(invalid(
                "contract_activation.successor",
                "block must advance and time must not move backward",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivatedContract {
    pub address: Address20,
    pub runtime_code_keccak256: Bytes32,
    pub deployment: DeploymentObservation,
}

impl ActivatedContract {
    fn validate(&self, field: &'static str) -> Result<()> {
        if is_zero_address(self.address) || self.runtime_code_keccak256 == Bytes32::ZERO {
            return Err(invalid(
                field,
                "address and runtime code hash must be nonzero",
            ));
        }
        self.deployment.validate(field)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentObservation {
    pub transaction_hash: Bytes32,
    pub block_number: u64,
    pub block_hash: Bytes32,
}

impl DeploymentObservation {
    fn validate(&self, field: &'static str) -> Result<()> {
        if self.transaction_hash == Bytes32::ZERO || self.block_hash == Bytes32::ZERO {
            return Err(invalid(
                field,
                "transaction and block hashes must be nonzero",
            ));
        }
        if self.block_number == 0 || self.block_number > MAX_SAFE_JSON_INTEGER {
            return Err(invalid(
                field,
                "block number must be a positive JavaScript-safe integer",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainBlock {
    pub block_number: u64,
    pub block_hash: Bytes32,
}

impl ChainBlock {
    fn validate(&self, field: &'static str) -> Result<()> {
        if self.block_number == 0 || self.block_number > MAX_SAFE_JSON_INTEGER {
            return Err(invalid(
                field,
                "block number must be a positive JavaScript-safe integer",
            ));
        }
        if self.block_hash == Bytes32::ZERO {
            return Err(invalid(field, "block hash must not be zero"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthorityApproval {
    pub key_id: Bytes32,
    pub authorization: DetachedP256Signature,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedContractActivation {
    pub schema: String,
    pub activation: ContractActivation,
    pub approvals: Vec<ReleaseAuthorityApproval>,
}

impl SignedContractActivation {
    /// Verifies both the immutable generation binding and the threshold under
    /// the supplied policy. This still does not decide that the policy itself
    /// is trusted.
    pub fn verify_for_generation(
        &self,
        policy: &ReleaseAuthorityPolicy,
        generation: &ContractGeneration,
    ) -> Result<()> {
        self.activation.validate_against_generation(generation)?;
        self.verify(policy)
    }

    /// Verifies a threshold under the supplied policy. It does not decide that
    /// the policy itself is trusted.
    pub fn verify(&self, policy: &ReleaseAuthorityPolicy) -> Result<()> {
        if self.schema != SIGNED_CONTRACT_ACTIVATION_SCHEMA {
            return Err(invalid(
                "signed_contract_activation.schema",
                format!("must be {SIGNED_CONTRACT_ACTIVATION_SCHEMA}"),
            ));
        }
        self.activation.validate()?;
        policy.validate()?;
        if self.activation.authority_policy_sha256 != policy.digest()? {
            return Err(invalid(
                "contract_activation.authority_policy_sha256",
                "does not bind the supplied policy",
            ));
        }
        if self.approvals.len() < policy.threshold as usize
            || self.approvals.len() > policy.authorities.len()
        {
            return Err(invalid(
                "signed_contract_activation.approvals",
                "must satisfy the policy threshold without unknown keys",
            ));
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
                return Err(invalid(
                    "signed_contract_activation.approvals",
                    "key IDs must be unique and strictly ordered",
                ));
            }
            previous = Some(approval.key_id);
            let public_key = authorities.get(&approval.key_id).ok_or_else(|| {
                invalid(
                    "signed_contract_activation.approvals",
                    "contains a key outside the bound policy",
                )
            })?;
            approval.authorization.validate()?;
            if approval.authorization.digest != digest {
                return Err(ProtocolError::DigestMismatch);
            }
            approval.authorization.verify(public_key)?;
        }
        Ok(())
    }
}

fn validate_version(field: &'static str, value: &str) -> Result<()> {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
        })
    {
        return Err(invalid(
            field,
            "must contain three canonical decimal components",
        ));
    }
    Ok(())
}

fn is_zero_address(address: Address20) -> bool {
    address.as_bytes().iter().all(|byte| *byte == 0)
}
