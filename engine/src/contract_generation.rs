//! Contract-generation resolution for identity and public authority.
//!
//! A committed build definition is not evidence that contracts were deployed
//! or authorized. The engine embeds the exact versioned definition and keeps
//! the generation inactive until a release also carries the complete client
//! trust root: the owner-approved release-authority policy digest, the policy
//! instance, and a threshold-signed activation binding this exact definition
//! on its exact chain. A release without a trust root always resolves
//! inactive; a release whose trust root fails any verification step refuses
//! to resolve at all rather than silently falling back to inactive.

use tohseno_protocol::contract_activation::{ReleaseAuthorityPolicy, SignedContractActivation};
use tohseno_protocol::contract_generation::ContractGeneration;
use tohseno_protocol::digest::Bytes32;
use tohseno_protocol::identity::ROBINHOOD_CHAIN_ID;

pub const CURRENT_CONTRACT_GENERATION: &str = "0.8.0";
pub const CURRENT_GENERATION_REPOSITORY_PATH: &str = "contracts/generations/0.8.0/generation.json";

const CURRENT_GENERATION_JSON: &[u8] =
    include_bytes!("../../contracts/generations/0.8.0/generation.json");

/// The compiled-in client trust root, established by the owner ceremony of
/// 2026-08-02: the owner-approved policy digest (canonical lowercase `0x`
/// hex) plus the policy instance and threshold-signed activation published
/// under `release/contract-activations/`. Resolution verifies the complete
/// chain on every call; shipping a partial or non-verifying trust root
/// refuses to resolve rather than degrading.
pub const TRUSTED_RELEASE_AUTHORITY_POLICY_DIGEST_HEX: Option<&str> =
    Some("0xf14410692ebe34f6855b8dbec5cb08733aa737f1cd86f385694e4fb575df943c");
const TRUSTED_RELEASE_AUTHORITY_POLICY_JSON: Option<&[u8]> = Some(include_bytes!(
    "../../release/contract-activations/release-authority-policy.json"
));
const SIGNED_CONTRACT_ACTIVATION_JSON: Option<&[u8]> = Some(include_bytes!(
    "../../release/contract-activations/signed-contract-activation-1.json"
));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractGenerationState {
    /// There is no embedded release-authority trust root and no signed
    /// activation. Build facts alone never authorize identity creation,
    /// public signing, or public mutation.
    Inactive,
    /// The embedded trust root verified end-to-end: the embedded policy
    /// reproduces the pinned digest, and a threshold of its authorities
    /// signed an activation that binds this exact generation definition on
    /// its exact chain.
    Active,
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
        self.state == ContractGenerationState::Active
    }

    pub fn allows_public_signing(&self) -> bool {
        self.state == ContractGenerationState::Active
    }

    pub fn inactive_reason(&self) -> &'static str {
        "the 0.8.0 candidate is deployed inactive on Robinhood Chain, and this build embeds no trusted release-authority root or signed chain activation"
    }
}

struct EmbeddedTrustRoot<'a> {
    pinned_policy_digest_hex: &'a str,
    policy_json: &'a [u8],
    signed_activation_json: &'a [u8],
}

pub fn resolve_current_contract_generation(
) -> Result<ResolvedContractGeneration, ContractGenerationError> {
    let trust_root = match (
        TRUSTED_RELEASE_AUTHORITY_POLICY_DIGEST_HEX,
        TRUSTED_RELEASE_AUTHORITY_POLICY_JSON,
        SIGNED_CONTRACT_ACTIVATION_JSON,
    ) {
        (None, None, None) => None,
        (Some(pinned_policy_digest_hex), Some(policy_json), Some(signed_activation_json)) => {
            Some(EmbeddedTrustRoot {
                pinned_policy_digest_hex,
                policy_json,
                signed_activation_json,
            })
        }
        _ => {
            return Err(ContractGenerationError::TrustRootInvalid(
                "this build embeds a partial trust root; an activating release carries the pinned policy digest, the policy, and the signed activation together"
                    .into(),
            ))
        }
    };
    resolve_with_trust_root(trust_root)
}

fn resolve_with_trust_root(
    trust_root: Option<EmbeddedTrustRoot<'_>>,
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
    let Some(trust_root) = trust_root else {
        return Ok(ResolvedContractGeneration {
            definition,
            definition_digest,
            trusted_release_authority_policy_digest: None,
            signed_activation_head: None,
            state: ContractGenerationState::Inactive,
        });
    };
    let (policy_digest, activation_head) = verify_trust_root(&definition, &trust_root)?;
    Ok(ResolvedContractGeneration {
        definition,
        definition_digest,
        trusted_release_authority_policy_digest: Some(policy_digest),
        signed_activation_head: Some(activation_head),
        state: ContractGenerationState::Active,
    })
}

/// Verifies the complete activation chain under the pinned digest. Every
/// failure is a hard error: a client carrying a trust root that does not
/// verify must refuse to run public paths, not degrade to inactive.
fn verify_trust_root(
    definition: &ContractGeneration,
    trust_root: &EmbeddedTrustRoot<'_>,
) -> Result<(Bytes32, Bytes32), ContractGenerationError> {
    let untrusted = |reason: String| ContractGenerationError::TrustRootInvalid(reason);
    let pinned = Bytes32::from_hex(
        "trusted_release_authority_policy_digest",
        trust_root.pinned_policy_digest_hex,
    )
    .map_err(|error| untrusted(format!("the pinned policy digest is malformed: {error}")))?;
    if pinned == Bytes32::ZERO {
        return Err(untrusted(
            "the pinned policy digest must not be zero".into(),
        ));
    }
    let policy =
        tohseno_protocol::canonical::from_slice::<ReleaseAuthorityPolicy>(trust_root.policy_json)
            .map_err(|error| {
            untrusted(format!(
                "the embedded release-authority policy is not strict canonical JSON: {error}"
            ))
        })?;
    let policy_digest = policy
        .digest()
        .map_err(|error| untrusted(format!("the embedded policy digest failed: {error}")))?;
    if policy_digest != pinned {
        return Err(untrusted(
            "the embedded release-authority policy does not reproduce the pinned digest".into(),
        ));
    }
    let signed = tohseno_protocol::canonical::from_slice::<SignedContractActivation>(
        trust_root.signed_activation_json,
    )
    .map_err(|error| {
        untrusted(format!(
            "the embedded signed activation is not strict canonical JSON: {error}"
        ))
    })?;
    signed
        .verify_for_generation(&policy, definition)
        .map_err(|error| {
            untrusted(format!(
                "the embedded activation does not verify under the pinned policy for this generation: {error}"
            ))
        })?;
    let activation_head = signed
        .activation
        .signing_digest()
        .map_err(|error| untrusted(format!("the activation head digest failed: {error}")))?;
    Ok((policy_digest, activation_head))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractGenerationError {
    InvalidDefinition(String),
    TrustRootInvalid(String),
}

impl std::fmt::Display for ContractGenerationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDefinition(reason) => {
                write!(formatter, "contract generation resolution failed: {reason}")
            }
            Self::TrustRootInvalid(reason) => {
                write!(
                    formatter,
                    "contract generation trust root rejected: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for ContractGenerationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::hazmat::PrehashSigner;
    use p256::ecdsa::{Signature, SigningKey};
    use tohseno_protocol::canonical;
    use tohseno_protocol::contract_activation::{
        ActivatedContract, ChainBlock, ContractActivation, DeploymentObservation, ReleaseAuthority,
        ReleaseAuthorityApproval, ReleaseAuthorityPurpose, CONTRACT_ACTIVATION_PROTOCOL,
        CONTRACT_ACTIVATION_SCHEMA, RELEASE_AUTHORITY_POLICY_SCHEMA,
        SIGNED_CONTRACT_ACTIVATION_SCHEMA,
    };
    use tohseno_protocol::record::CanonicalTimestamp;
    use tohseno_protocol::signature::{
        DetachedP256Signature, P256PublicKey, P256Signature, SignatureAlgorithm,
    };

    #[test]
    fn shipped_build_resolves_the_active_generation_under_the_pinned_trust_root() {
        let resolved = resolve_current_contract_generation().unwrap();
        assert_eq!(resolved.definition.generation, "0.8.0");
        assert_ne!(resolved.definition_digest, Bytes32::ZERO);
        assert_eq!(resolved.state, ContractGenerationState::Active);
        assert!(resolved.allows_new_builder_identity());
        assert!(resolved.allows_public_signing());
        assert_eq!(
            resolved
                .trusted_release_authority_policy_digest
                .unwrap()
                .to_hex(),
            TRUSTED_RELEASE_AUTHORITY_POLICY_DIGEST_HEX.unwrap()
        );
        assert_eq!(
            resolved.signed_activation_head.unwrap().to_hex(),
            "0x2b640260595def403343810d0dc4ee231e1faff427581be4f7b40cff4c189d28"
        );
    }

    #[test]
    fn shipped_build_embeds_the_complete_trust_root() {
        // The 2026-08-02 activating commit flipped all three constants
        // together; a partial trust root must never ship.
        assert!(TRUSTED_RELEASE_AUTHORITY_POLICY_DIGEST_HEX.is_some());
        assert!(TRUSTED_RELEASE_AUTHORITY_POLICY_JSON.is_some());
        assert!(SIGNED_CONTRACT_ACTIVATION_JSON.is_some());
    }

    #[test]
    fn a_build_without_a_trust_root_still_resolves_inactive() {
        let resolved = resolve_with_trust_root(None).unwrap();
        assert_eq!(resolved.state, ContractGenerationState::Inactive);
        assert_eq!(resolved.trusted_release_authority_policy_digest, None);
        assert_eq!(resolved.signed_activation_head, None);
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

    fn public_key(signing_key: &SigningKey) -> P256PublicKey {
        let point = signing_key.verifying_key().to_encoded_point(false);
        let copy = |bytes: &[u8]| {
            let mut value = [0_u8; 32];
            value.copy_from_slice(bytes);
            Bytes32::new(value)
        };
        P256PublicKey {
            x: copy(point.x().unwrap()),
            y: copy(point.y().unwrap()),
        }
    }

    fn sign(signing_key: &SigningKey, digest: Bytes32) -> P256Signature {
        let signature: Signature = signing_key.sign_prehash(digest.as_bytes()).unwrap();
        let signature = signature.normalize_s().unwrap_or(signature);
        P256Signature {
            r: Bytes32::new(signature.r().to_bytes().into()),
            s: Bytes32::new(signature.s().to_bytes().into()),
        }
    }

    fn policy_and_keys(scalars: [u8; 3]) -> (ReleaseAuthorityPolicy, Vec<(Bytes32, SigningKey)>) {
        let mut pairs = scalars
            .into_iter()
            .map(|scalar| {
                let key = SigningKey::from_bytes((&[scalar; 32]).into()).unwrap();
                let authority = ReleaseAuthority::from_public_key(public_key(&key)).unwrap();
                (authority, key)
            })
            .collect::<Vec<_>>();
        pairs.sort_by_key(|(authority, _)| authority.key_id);
        let keys = pairs
            .iter()
            .map(|(authority, key)| (authority.key_id, key.clone()))
            .collect();
        let policy = ReleaseAuthorityPolicy {
            schema: RELEASE_AUTHORITY_POLICY_SCHEMA.into(),
            protocol: CONTRACT_ACTIVATION_PROTOCOL.into(),
            protocol_major: 2,
            purpose: ReleaseAuthorityPurpose::ContractGenerationActivation,
            threshold: 2,
            authorities: pairs.into_iter().map(|(authority, _)| authority).collect(),
            issued_at: CanonicalTimestamp::parse("2026-08-01T00:00:00Z").unwrap(),
        };
        policy.validate().unwrap();
        (policy, keys)
    }

    fn activation_for_embedded_generation(policy: &ReleaseAuthorityPolicy) -> ContractActivation {
        let generation: ContractGeneration =
            canonical::from_slice(CURRENT_GENERATION_JSON).unwrap();
        ContractActivation {
            schema: CONTRACT_ACTIVATION_SCHEMA.into(),
            protocol: CONTRACT_ACTIVATION_PROTOCOL.into(),
            protocol_major: 2,
            generation: generation.generation.clone(),
            activation_sequence: 1,
            previous_activation: None,
            generation_definition_sha256: generation.digest().unwrap(),
            authority_policy_sha256: policy.digest().unwrap(),
            chain_id: generation.chain.chain_id,
            builder_account_runtime_keccak256: Bytes32::new([0x51; 32]),
            factory: ActivatedContract {
                address: generation.create2.builder_account_factory.predicted_address,
                runtime_code_keccak256: generation
                    .contracts
                    .builder_account_factory
                    .runtime_code_keccak256,
                deployment: DeploymentObservation {
                    transaction_hash: Bytes32::new([0x11; 32]),
                    block_number: 24_677_436,
                    block_hash: Bytes32::new([0x12; 32]),
                },
            },
            registry: ActivatedContract {
                address: generation.create2.shot_registry.predicted_address,
                runtime_code_keccak256: Bytes32::new([0x52; 32]),
                deployment: DeploymentObservation {
                    transaction_hash: Bytes32::new([0x21; 32]),
                    block_number: 24_679_962,
                    block_hash: Bytes32::new([0x22; 32]),
                },
            },
            activation_block: ChainBlock {
                block_number: 24_700_000,
                block_hash: Bytes32::new([0x31; 32]),
            },
            p256_probe_sha256: Bytes32::new([0x41; 32]),
            issued_at: CanonicalTimestamp::parse("2026-08-01T01:00:00Z").unwrap(),
        }
    }

    fn signed_activation(
        activation: ContractActivation,
        keys: &[(Bytes32, SigningKey)],
        approvals: usize,
    ) -> SignedContractActivation {
        let digest = activation.signing_digest().unwrap();
        let approvals = keys
            .iter()
            .take(approvals)
            .map(|(key_id, key)| ReleaseAuthorityApproval {
                key_id: *key_id,
                authorization: DetachedP256Signature {
                    algorithm: SignatureAlgorithm::P256,
                    digest,
                    signature: sign(key, digest),
                    low_s: true,
                },
            })
            .collect();
        SignedContractActivation {
            schema: SIGNED_CONTRACT_ACTIVATION_SCHEMA.into(),
            activation,
            approvals,
        }
    }

    fn resolve_with(
        pinned_hex: &str,
        policy: &ReleaseAuthorityPolicy,
        signed: &SignedContractActivation,
    ) -> Result<ResolvedContractGeneration, ContractGenerationError> {
        resolve_with_trust_root(Some(EmbeddedTrustRoot {
            pinned_policy_digest_hex: pinned_hex,
            policy_json: &canonical::to_vec(policy).unwrap(),
            signed_activation_json: &canonical::to_vec(signed).unwrap(),
        }))
    }

    #[test]
    fn a_complete_trust_root_activates_the_generation() {
        let (policy, keys) = policy_and_keys([1, 2, 3]);
        let signed = signed_activation(activation_for_embedded_generation(&policy), &keys, 2);
        let pinned = policy.digest().unwrap().to_hex();
        let resolved = resolve_with(&pinned, &policy, &signed).unwrap();
        assert_eq!(resolved.state, ContractGenerationState::Active);
        assert!(resolved.allows_new_builder_identity());
        assert!(resolved.allows_public_signing());
        assert_eq!(
            resolved.trusted_release_authority_policy_digest,
            Some(policy.digest().unwrap())
        );
        assert_eq!(
            resolved.signed_activation_head,
            Some(signed.activation.signing_digest().unwrap())
        );
    }

    #[test]
    fn an_activation_under_a_policy_other_than_the_pinned_one_never_activates() {
        // A fully valid policy and threshold signature chain, but the client
        // pinned a different policy digest: must refuse, not fall back.
        let (policy, keys) = policy_and_keys([1, 2, 3]);
        let (other_policy, _) = policy_and_keys([4, 5, 6]);
        let signed = signed_activation(activation_for_embedded_generation(&policy), &keys, 2);
        let pinned = other_policy.digest().unwrap().to_hex();
        let error = resolve_with(&pinned, &policy, &signed).unwrap_err();
        assert!(matches!(
            error,
            ContractGenerationError::TrustRootInvalid(_)
        ));
    }

    #[test]
    fn a_threshold_shortfall_never_activates() {
        let (policy, keys) = policy_and_keys([1, 2, 3]);
        let signed = signed_activation(activation_for_embedded_generation(&policy), &keys, 1);
        let pinned = policy.digest().unwrap().to_hex();
        let error = resolve_with(&pinned, &policy, &signed).unwrap_err();
        assert!(matches!(
            error,
            ContractGenerationError::TrustRootInvalid(_)
        ));
    }

    #[test]
    fn an_activation_for_different_chain_evidence_never_activates() {
        let (policy, keys) = policy_and_keys([1, 2, 3]);
        let mut activation = activation_for_embedded_generation(&policy);
        activation.factory.address = tohseno_protocol::digest::Address20::from_bytes([0x99; 20]);
        let signed = signed_activation(activation, &keys, 2);
        let pinned = policy.digest().unwrap().to_hex();
        let error = resolve_with(&pinned, &policy, &signed).unwrap_err();
        assert!(matches!(
            error,
            ContractGenerationError::TrustRootInvalid(_)
        ));
    }

    #[test]
    fn the_pinned_digest_matches_the_embedded_policy_exactly() {
        // Defense in depth for the activating commit itself: the embedded
        // policy's independently recomputed digest must equal the pinned
        // constant, or resolution would refuse at startup.
        let policy: ReleaseAuthorityPolicy =
            canonical::from_slice(TRUSTED_RELEASE_AUTHORITY_POLICY_JSON.unwrap()).unwrap();
        assert_eq!(
            policy.digest().unwrap().to_hex(),
            TRUSTED_RELEASE_AUTHORITY_POLICY_DIGEST_HEX.unwrap()
        );
        assert_eq!(policy.threshold, 2);
        assert_eq!(policy.authorities.len(), 3);
    }
}
