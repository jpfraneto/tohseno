//! Contract-generation resolution for identity and public authority.
//!
//! A committed build definition is not evidence that contracts were deployed
//! or authorized. The engine therefore embeds the exact versioned definition
//! but keeps the generation inactive until a future release carries both a
//! trusted release-authority policy and a valid signed activation chain.

use tohseno_protocol::contract_generation::ContractGeneration;
use tohseno_protocol::digest::Bytes32;
use tohseno_protocol::identity::ROBINHOOD_CHAIN_ID;

pub const CURRENT_CONTRACT_GENERATION: &str = "0.8.0";
pub const CURRENT_GENERATION_REPOSITORY_PATH: &str = "contracts/generations/0.8.0/generation.json";

const CURRENT_GENERATION_JSON: &[u8] =
    include_bytes!("../../contracts/generations/0.8.0/generation.json");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractGenerationState {
    /// There is no embedded release-authority trust root and no signed
    /// activation. Build facts alone never authorize identity creation,
    /// public signing, or public mutation.
    Inactive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedContractGeneration {
    pub definition: ContractGeneration,
    pub definition_digest: Bytes32,
    pub trusted_release_authority_policy_digest: Option<Bytes32>,
    pub signed_activation_head: Option<Bytes32>,
    pub state: ContractGenerationState,
}

impl ResolvedContractGeneration {
    pub fn allows_new_builder_identity(&self) -> bool {
        false
    }

    pub fn allows_public_signing(&self) -> bool {
        false
    }

    pub fn inactive_reason(&self) -> &'static str {
        "the 0.8.0 build definition is committed, but no trusted release-authority root or signed chain activation exists"
    }
}

pub fn resolve_current_contract_generation(
) -> Result<ResolvedContractGeneration, ContractGenerationError> {
    let definition =
        tohseno_protocol::canonical::from_slice::<ContractGeneration>(CURRENT_GENERATION_JSON)
            .map_err(|error| {
                ContractGenerationError::InvalidDefinition(format!(
            "embedded {CURRENT_GENERATION_REPOSITORY_PATH} is not strict generation JSON: {error}"
        ))
            })?;
    definition.validate().map_err(|error| {
        ContractGenerationError::InvalidDefinition(format!(
            "embedded {CURRENT_GENERATION_REPOSITORY_PATH} is invalid: {error}"
        ))
    })?;
    if definition.generation != CURRENT_CONTRACT_GENERATION
        || definition.protocol_major != 2
        || definition.chain.chain_id != ROBINHOOD_CHAIN_ID
    {
        return Err(ContractGenerationError::InvalidDefinition(
            "embedded generation coordinates are not TOHSENO protocol 2, generation 0.8.0, on eip155:4663"
                .into(),
        ));
    }
    let definition_digest = definition.digest().map_err(|error| {
        ContractGenerationError::InvalidDefinition(format!(
            "embedded generation digest could not be reproduced: {error}"
        ))
    })?;
    Ok(ResolvedContractGeneration {
        definition,
        definition_digest,
        trusted_release_authority_policy_digest: None,
        signed_activation_head: None,
        state: ContractGenerationState::Inactive,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractGenerationError {
    InvalidDefinition(String),
}

impl std::fmt::Display for ContractGenerationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDefinition(reason) => {
                write!(formatter, "contract generation resolution failed: {reason}")
            }
        }
    }
}

impl std::error::Error for ContractGenerationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_generation_is_valid_but_inactive_without_activation_authority() {
        let resolved = resolve_current_contract_generation().unwrap();
        assert_eq!(resolved.definition.generation, "0.8.0");
        assert_ne!(resolved.definition_digest, Bytes32::ZERO);
        assert_eq!(resolved.trusted_release_authority_policy_digest, None);
        assert_eq!(resolved.signed_activation_head, None);
        assert_eq!(resolved.state, ContractGenerationState::Inactive);
        assert!(!resolved.allows_new_builder_identity());
        assert!(!resolved.allows_public_signing());
        assert!(resolved.inactive_reason().contains("no trusted"));
        assert!(resolved
            .inactive_reason()
            .contains("signed chain activation"));
    }

    #[test]
    fn resolver_is_bound_only_to_the_versioned_generation_definition() {
        assert_eq!(
            CURRENT_GENERATION_REPOSITORY_PATH,
            "contracts/generations/0.8.0/generation.json"
        );
        assert!(!CURRENT_GENERATION_REPOSITORY_PATH.contains("next"));
        assert!(!CURRENT_GENERATION_REPOSITORY_PATH.contains("deployments/"));
        assert!(!CURRENT_GENERATION_REPOSITORY_PATH.contains("bytecode/"));
    }
}
