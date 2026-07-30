//! GENESIS candidate policy above the neutral lineage reducer.
//!
//! The neutral protocol intentionally accepts the initial controller/key
//! binding declared by a commitment. This module independently reproduces the
//! pinned counterfactual BuilderAccount before a node calls that initial
//! binding candidate-authorized. GENESIS does not yet define a candidate
//! ownership-transfer proof, so neutral validity after an ownership action is
//! deliberately not promoted to candidate authority.

use crate::model::{CandidateContractConfiguration, PlannedContract};
use crate::{NodeError, Result};
use serde::Deserialize;
use tohseno_protocol::digest::{sha256, Address20, Bytes32};
use tohseno_protocol::identity::{
    initial_builder_account_salt, predict_builder_account, BuilderId, ROBINHOOD_CHAIN_ID,
};
use tohseno_protocol::lineage::{LineagePayload, SignedLineageAction};
use tohseno_protocol::signature::P256PublicKey;

const CURRENT_DEPLOYMENT_PLAN_JSON: &str =
    include_str!("../../contracts/deployments/robinhood-mainnet-v0.8.0.json");
const LEGACY_DEPLOYMENT_PLAN_JSON: &str =
    include_str!("../../contracts/deployments/robinhood-mainnet-genesis.json");
const CURRENT_BUILDER_ACCOUNT_CREATION_HEX: &str =
    include_str!("../../contracts/bytecode/BuilderAccount.v0.8.0.creation.hex");
const LEGACY_BUILDER_ACCOUNT_CREATION_HEX: &str =
    include_str!("../../contracts/bytecode/BuilderAccount.creation.hex");
const CONFIGURATION_SCHEMA: &str = "tohseno.node-contract-configuration/1";
const EXPECTED_CANDIDATE_VERSION: &str = "0.8.0";
const LEGACY_CANDIDATE_VERSION: &str = "0.7.0";
const EXPECTED_CANDIDATE_STATUS: &str = "planned, undeployed, non-canonical and unaudited";
const AUTHORITY_POLICY: &str = "initial authority only: neutral lineage reduction plus exact-match BuilderAccount CREATE2 prediction across the frozen v0.7 and current v0.8 generations; ownership actions and their descendants remain authority-unresolved because no complete ownership-transfer authorization proof is available";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BranchAuthority {
    Verified,
    OwnershipTransferUnresolved { sequence: u64 },
}

#[derive(Clone)]
pub(crate) struct CandidatePolicy {
    configuration: CandidateContractConfiguration,
    current_generation: AuthorityGeneration,
    verification_generations: Vec<AuthorityGeneration>,
}

#[derive(Clone)]
struct AuthorityGeneration {
    version: String,
    factory_address: Address20,
    builder_account_creation_bytecode: Vec<u8>,
}

impl CandidatePolicy {
    pub fn embedded() -> Result<Self> {
        let plan: DeploymentPlan = serde_json::from_str(CURRENT_DEPLOYMENT_PLAN_JSON)?;
        validate_plan(&plan, EXPECTED_CANDIDATE_VERSION, "WITNESS_V2")?;
        let current_generation = AuthorityGeneration {
            version: plan.candidate.version.clone(),
            factory_address: plan.contracts.builder_account_factory.planned_address,
            builder_account_creation_bytecode: decode_creation_bytecode(
                CURRENT_BUILDER_ACCOUNT_CREATION_HEX,
            )?,
        };
        let legacy_plan: DeploymentPlan = serde_json::from_str(LEGACY_DEPLOYMENT_PLAN_JSON)?;
        validate_plan(&legacy_plan, LEGACY_CANDIDATE_VERSION, "GENESIS")?;
        let legacy_generation = AuthorityGeneration {
            version: legacy_plan.candidate.version,
            factory_address: legacy_plan
                .contracts
                .builder_account_factory
                .planned_address,
            builder_account_creation_bytecode: decode_creation_bytecode(
                LEGACY_BUILDER_ACCOUNT_CREATION_HEX,
            )?,
        };
        let configuration = CandidateContractConfiguration {
            schema: CONFIGURATION_SCHEMA.into(),
            candidate_version: plan.candidate.version,
            candidate_status: plan.candidate.status,
            chain_name: plan.chain.name,
            chain_id: plan.chain.chain_id,
            p256verify: plan.chain.p256verify,
            create2_deployer: plan.create2.deployer,
            deployer_code_must_be_verified_before_broadcast: plan
                .create2
                .deployer_code_must_be_verified_before_broadcast,
            builder_account_factory: planned(&plan.contracts.builder_account_factory),
            shot_registry: planned(&plan.contracts.shot_registry),
            shot_relations: planned(&plan.contracts.shot_relations),
            builder_account_creation_bytecode_sha256: sha256(
                &current_generation.builder_account_creation_bytecode,
            ),
            initial_authority_policy: AUTHORITY_POLICY.into(),
        };
        Ok(Self {
            configuration,
            current_generation: current_generation.clone(),
            verification_generations: vec![legacy_generation, current_generation],
        })
    }

    pub fn configuration(&self) -> &CandidateContractConfiguration {
        &self.configuration
    }

    pub fn predict_builder_id(&self, initial_key: &P256PublicKey) -> Result<BuilderId> {
        self.predict_builder_id_for(&self.current_generation, initial_key)
    }

    fn predict_builder_id_for(
        &self,
        generation: &AuthorityGeneration,
        initial_key: &P256PublicKey,
    ) -> Result<BuilderId> {
        let salt = initial_builder_account_salt(initial_key)?;
        Ok(predict_builder_account(
            generation.factory_address,
            salt,
            initial_key,
            &generation.builder_account_creation_bytecode,
        )?)
    }

    pub fn assess_branch(&self, branch: &[SignedLineageAction]) -> Result<BranchAuthority> {
        let commitment = branch.first().ok_or_else(|| {
            NodeError::Causal("candidate authority requires a nonempty lineage branch".into())
        })?;
        self.verify_commitment(commitment)?;
        if let Some(ownership) = branch
            .iter()
            .find(|action| matches!(action.action.payload, LineagePayload::Ownership(_)))
        {
            return Ok(BranchAuthority::OwnershipTransferUnresolved {
                sequence: ownership.action.sequence,
            });
        }
        Ok(BranchAuthority::Verified)
    }

    fn verify_commitment(&self, action: &SignedLineageAction) -> Result<()> {
        let LineagePayload::Commitment(commitment) = &action.action.payload else {
            return Err(NodeError::Causal(
                "complete lineage does not begin with a commitment".into(),
            ));
        };
        let mut matched = None;
        for generation in &self.verification_generations {
            let predicted =
                self.predict_builder_id_for(generation, &commitment.initial_controller_key)?;
            if predicted == commitment.initial_controller && predicted == action.action.actor {
                if matched.is_some() {
                    return Err(NodeError::Causal(
                        "commitment controller ambiguously matches multiple BuilderAccount generations"
                            .into(),
                    ));
                }
                matched = Some(generation.version.as_str());
            }
        }
        if matched.is_none() {
            return Err(NodeError::Causal(format!(
                "commitment controller {} does not reproduce a pinned candidate BuilderAccount under any supported contract generation",
                commitment.initial_controller
            )));
        }
        Ok(())
    }
}

pub fn candidate_contract_configuration() -> Result<CandidateContractConfiguration> {
    Ok(CandidatePolicy::embedded()?.configuration)
}

pub fn predict_candidate_builder_id(initial_key: &P256PublicKey) -> Result<BuilderId> {
    CandidatePolicy::embedded()?.predict_builder_id(initial_key)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeploymentPlan {
    schema: String,
    protocol: String,
    candidate: CandidateDeclaration,
    chain: ChainDeclaration,
    contracts: CandidateContracts,
    create2: Create2Declaration,
    source_commit: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateDeclaration {
    version: String,
    codename: String,
    status: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChainDeclaration {
    chain_id: u64,
    name: String,
    p256verify: Address20,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create2Declaration {
    deployer: Address20,
    deployer_code_must_be_verified_before_broadcast: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateContracts {
    #[serde(rename = "BuilderAccountFactory")]
    builder_account_factory: ContractDeclaration,
    #[serde(rename = "ShotRegistry")]
    shot_registry: ContractDeclaration,
    #[serde(rename = "ShotRelations")]
    shot_relations: ContractDeclaration,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContractDeclaration {
    constructor_arguments: Vec<Address20>,
    deployed: bool,
    deployment_order: u8,
    init_code_hash: Bytes32,
    planned_address: Address20,
    runtime_code_hash: Option<Bytes32>,
    salt: Bytes32,
    transaction_hash: Option<Bytes32>,
}

fn validate_plan(
    plan: &DeploymentPlan,
    expected_version: &str,
    expected_codename: &str,
) -> Result<()> {
    if plan.schema != "tohseno.deployment-plan/1"
        || plan.protocol != "tohseno"
        || plan.candidate.version != expected_version
        || plan.candidate.codename != expected_codename
        || plan.candidate.status != EXPECTED_CANDIDATE_STATUS
        || plan.chain.chain_id != ROBINHOOD_CHAIN_ID
        || plan.chain.name != "Robinhood Chain mainnet"
        || plan
            .chain
            .p256verify
            .as_bytes()
            .iter()
            .all(|byte| *byte == 0)
        || plan
            .create2
            .deployer
            .as_bytes()
            .iter()
            .all(|byte| *byte == 0)
        || !plan.create2.deployer_code_must_be_verified_before_broadcast
        || plan.source_commit.is_some()
    {
        return Err(NodeError::Protocol(
            "embedded candidate deployment plan has unexpected identity or status".into(),
        ));
    }
    for (expected_order, contract) in [
        (1, &plan.contracts.builder_account_factory),
        (2, &plan.contracts.shot_registry),
        (3, &plan.contracts.shot_relations),
    ] {
        if contract.deployed
            || contract.deployment_order != expected_order
            || contract
                .planned_address
                .as_bytes()
                .iter()
                .all(|byte| *byte == 0)
            || contract.init_code_hash == Bytes32::ZERO
            || contract.salt == Bytes32::ZERO
            || contract.runtime_code_hash.is_some()
            || contract.transaction_hash.is_some()
        {
            return Err(NodeError::Protocol(
                "embedded candidate contract is not an honest planned/undeployed coordinate".into(),
            ));
        }
    }
    if !plan
        .contracts
        .builder_account_factory
        .constructor_arguments
        .is_empty()
        || !plan
            .contracts
            .shot_registry
            .constructor_arguments
            .is_empty()
        || plan.contracts.shot_relations.constructor_arguments
            != vec![plan.contracts.shot_registry.planned_address]
    {
        return Err(NodeError::Protocol(
            "embedded candidate contract constructor graph is inconsistent".into(),
        ));
    }
    Ok(())
}

fn planned(contract: &ContractDeclaration) -> PlannedContract {
    PlannedContract {
        deployment_order: contract.deployment_order,
        planned_address: contract.planned_address,
        salt: contract.salt,
        init_code_hash: contract.init_code_hash,
        deployed: contract.deployed,
        runtime_code_hash: contract.runtime_code_hash,
        transaction_hash: contract.transaction_hash,
    }
}

fn decode_creation_bytecode(text: &str) -> Result<Vec<u8>> {
    let encoded = text
        .trim()
        .strip_prefix("0x")
        .ok_or_else(|| NodeError::Protocol("BuilderAccount bytecode lacks 0x prefix".into()))?;
    if encoded.is_empty()
        || encoded.len() % 2 != 0
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(NodeError::Protocol(
            "BuilderAccount bytecode is not canonical lowercase hex".into(),
        ));
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair)
                .map_err(|error| NodeError::Protocol(error.to_string()))?;
            u8::from_str_radix(pair, 16)
                .map_err(|error| NodeError::Protocol(format!("invalid bytecode hex: {error}")))
        })
        .collect()
}
