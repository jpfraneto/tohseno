//! Client trust resolution for the separately deployed additive Claims contract.
//!
//! Absence is a normal dark-launch state. A future activating commit must land
//! the complete signed evidence and its exact digest together; partial or
//! non-verifying evidence is a hard error.

use tohseno_network::claims_activation::SignedClaimsActivation;
use tohseno_protocol::contract_activation::ReleaseAuthorityPolicy;
use tohseno_protocol::digest::{Address20, Bytes32};

use crate::contract_generation::{
    resolve_current_contract_generation, ContractGenerationState,
    TRUSTED_RELEASE_AUTHORITY_POLICY_DIGEST_HEX,
};

pub const CLAIMS_ACTIVATION_REPOSITORY_PATH: &str =
    "release/claims-activations/signed-claims-activation-1.json";

// Dark until the governed deployment and threshold ceremony create the exact
// evidence named above. Never replace this with an address-only toggle.
const SIGNED_CLAIMS_ACTIVATION_JSON: Option<&[u8]> = None;
const PINNED_CLAIMS_ACTIVATION_DIGEST_HEX: Option<&str> = None;
const TRUSTED_RELEASE_AUTHORITY_POLICY_JSON: &[u8] =
    include_bytes!("../../release/contract-activations/release-authority-policy.json");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimsContractState {
    Inactive,
    Active,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedClaimsContract {
    pub state: ClaimsContractState,
    pub claims_contract: Option<Address20>,
    pub shot_registry: Option<Address20>,
    pub activation_signing_digest: Option<Bytes32>,
    pub runtime_code_keccak256: Option<Bytes32>,
    pub deployment_block: Option<u64>,
}

impl ResolvedClaimsContract {
    pub fn inactive_reason(&self) -> &'static str {
        "this build carries no threshold-verified activation for TohsenoClaimsV1"
    }
}

pub fn resolve_claims_contract() -> Result<ResolvedClaimsContract, ClaimsActivationError> {
    let generation = resolve_current_contract_generation()
        .map_err(|error| ClaimsActivationError::Invalid(error.to_string()))?;
    if generation.state != ContractGenerationState::Active {
        return Err(ClaimsActivationError::Invalid(
            "Claims cannot activate without the trusted generation-0.8 Registry".into(),
        ));
    }
    let (Some(json), Some(pinned_hex)) = (
        SIGNED_CLAIMS_ACTIVATION_JSON,
        PINNED_CLAIMS_ACTIVATION_DIGEST_HEX,
    ) else {
        if SIGNED_CLAIMS_ACTIVATION_JSON.is_some() || PINNED_CLAIMS_ACTIVATION_DIGEST_HEX.is_some()
        {
            return Err(ClaimsActivationError::Invalid(
                "this build contains a partial Claims activation trust root".into(),
            ));
        }
        return Ok(ResolvedClaimsContract {
            state: ClaimsContractState::Inactive,
            claims_contract: None,
            shot_registry: None,
            activation_signing_digest: None,
            runtime_code_keccak256: None,
            deployment_block: None,
        });
    };
    let policy: ReleaseAuthorityPolicy =
        tohseno_protocol::canonical::from_slice(TRUSTED_RELEASE_AUTHORITY_POLICY_JSON)
            .map_err(|error| ClaimsActivationError::Invalid(error.to_string()))?;
    let pinned_policy = Bytes32::from_hex(
        "trusted_release_authority_policy_digest",
        TRUSTED_RELEASE_AUTHORITY_POLICY_DIGEST_HEX.ok_or_else(|| {
            ClaimsActivationError::Invalid("generation-0.8 policy pin is absent".into())
        })?,
    )
    .map_err(|error| ClaimsActivationError::Invalid(error.to_string()))?;
    if policy
        .digest()
        .map_err(|error| ClaimsActivationError::Invalid(error.to_string()))?
        != pinned_policy
    {
        return Err(ClaimsActivationError::Invalid(
            "Claims authority policy differs from the client policy pin".into(),
        ));
    }
    let signed: SignedClaimsActivation = tohseno_protocol::canonical::from_slice(json)
        .map_err(|error| ClaimsActivationError::Invalid(error.to_string()))?;
    signed
        .verify(&policy)
        .map_err(|error| ClaimsActivationError::Invalid(error.to_string()))?;
    let digest = signed
        .activation
        .signing_digest()
        .map_err(|error| ClaimsActivationError::Invalid(error.to_string()))?;
    let pinned = Bytes32::from_hex("claims_activation_signing_digest", pinned_hex)
        .map_err(|error| ClaimsActivationError::Invalid(error.to_string()))?;
    if digest != pinned
        || signed.activation.shot_registry
            != generation
                .definition
                .create2
                .shot_registry
                .predicted_address
    {
        return Err(ClaimsActivationError::Invalid(
            "Claims activation digest or Registry binding differs from the client pin".into(),
        ));
    }
    Ok(ResolvedClaimsContract {
        state: ClaimsContractState::Active,
        claims_contract: Some(signed.activation.claims_contract),
        shot_registry: Some(signed.activation.shot_registry),
        activation_signing_digest: Some(digest),
        runtime_code_keccak256: Some(signed.activation.runtime_code_keccak256),
        deployment_block: Some(signed.activation.deployment.block_number),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClaimsActivationError {
    Invalid(String),
}

impl std::fmt::Display for ClaimsActivationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(reason) => write!(formatter, "Claims activation rejected: {reason}"),
        }
    }
}

impl std::error::Error for ClaimsActivationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_client_keeps_claims_dark_without_complete_signed_activation() {
        let resolved = resolve_claims_contract().expect("dark state");
        assert_eq!(resolved.state, ClaimsContractState::Inactive);
        assert_eq!(resolved.claims_contract, None);
        assert_eq!(resolved.activation_signing_digest, None);
    }
}
