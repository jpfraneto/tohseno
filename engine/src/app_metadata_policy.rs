//! Release policy for embedded Apple metadata.
//!
//! `tohseno.app-metadata/2` shipped in v0.7.1 with an optional legacy registry
//! reference, so its generic protocol decoder remains frozen for offline
//! compatibility. Current engine paths apply this additional policy before
//! generating, accepting, or verifying v2 metadata. A future activated
//! registry receipt requires a successor metadata schema and versioned Fascia.

use crate::contract_generation::{resolve_current_contract_generation, ContractGenerationState};
use tohseno_protocol::app_metadata::AppMetadataV2;

pub(crate) fn validate_current_app_metadata_v2(
    metadata: &AppMetadataV2,
) -> Result<(), AppMetadataPolicyError> {
    metadata
        .validate()
        .map_err(|error| AppMetadataPolicyError::InvalidMetadata(error.to_string()))?;
    let generation = resolve_current_contract_generation()
        .map_err(|error| AppMetadataPolicyError::Generation(error.to_string()))?;
    match generation.state {
        ContractGenerationState::Inactive if metadata.registry.is_some() => {
            Err(AppMetadataPolicyError::InactiveRegistryEvidence)
        }
        ContractGenerationState::Inactive => Ok(()),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AppMetadataPolicyError {
    InvalidMetadata(String),
    Generation(String),
    InactiveRegistryEvidence,
}

impl std::fmt::Display for AppMetadataPolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMetadata(reason) => {
                write!(formatter, "embedded app metadata is invalid: {reason}")
            }
            Self::Generation(reason) => {
                write!(formatter, "contract generation could not be resolved: {reason}")
            }
            Self::InactiveRegistryEvidence => write!(
                formatter,
                "app-metadata/2 registry evidence is forbidden while no contract generation is active; an activated witness requires a successor metadata schema"
            ),
        }
    }
}

impl std::error::Error for AppMetadataPolicyError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tohseno_protocol::app_metadata::AppMetadataRegistryReference;
    use tohseno_protocol::digest::{Address20, Bytes32};

    fn fixture() -> AppMetadataV2 {
        tohseno_protocol::canonical::from_slice(include_bytes!(
            "../../protocol/test-vectors/app-metadata-v2.json"
        ))
        .unwrap()
    }

    #[test]
    fn inactive_generation_accepts_only_absent_v2_registry_evidence() {
        let metadata = fixture();
        assert_eq!(metadata.registry, None);
        validate_current_app_metadata_v2(&metadata).unwrap();

        let mut compatibility_value = metadata;
        compatibility_value.registry = Some(AppMetadataRegistryReference {
            chain_id: 4_663,
            contract: Address20::from_bytes([0x66; 20]),
            transaction: Some(Bytes32::new([0x77; 32])),
        });

        // The shipped /2 decoder stays compatible. Current engine policy,
        // rather than a silent wire-schema mutation, closes publication.
        compatibility_value.validate().unwrap();
        assert_eq!(
            validate_current_app_metadata_v2(&compatibility_value),
            Err(AppMetadataPolicyError::InactiveRegistryEvidence)
        );
    }
}
