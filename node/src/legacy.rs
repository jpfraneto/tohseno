//! Retired v0.7 offline BuilderID reproduction.
//!
//! The v0.7 contracts were never deployed and will never become an active
//! public generation. This helper remains solely so a holder can reproduce a
//! BuilderID embedded in a frozen private v0.7 artifact. Node authority
//! classification never calls it.

use crate::{NodeError, Result};
use serde::Deserialize;
use tohseno_protocol::digest::Address20;
use tohseno_protocol::identity::{
    initial_builder_account_salt, predict_builder_account, BuilderId, ROBINHOOD_CHAIN_ID,
};
use tohseno_protocol::signature::P256PublicKey;

const DEPLOYMENT_PLAN_JSON: &str =
    include_str!("../../contracts/deployments/robinhood-mainnet-genesis.json");
const BUILDER_ACCOUNT_CREATION_HEX: &str =
    include_str!("../../contracts/bytecode/BuilderAccount.creation.hex");
const RETIRED_VERSION: &str = "0.7.0";
const RETIRED_EMBEDDED_STATUS: &str = "planned, undeployed, non-canonical and unaudited";

/// Reproduces a frozen v0.7 BuilderID for offline inspection only.
///
/// The result is not an active BuilderID and must never promote a public
/// lineage record to candidate-authority `verified`.
pub fn predict_retired_v07_builder_id(initial_key: &P256PublicKey) -> Result<BuilderId> {
    let plan: RetiredDeploymentPlan = serde_json::from_str(DEPLOYMENT_PLAN_JSON)?;
    validate_plan(&plan)?;
    let creation_bytecode = decode_creation_bytecode(BUILDER_ACCOUNT_CREATION_HEX)?;
    let salt = initial_builder_account_salt(initial_key)?;
    Ok(predict_builder_account(
        plan.contracts.builder_account_factory.planned_address,
        salt,
        initial_key,
        &creation_bytecode,
    )?)
}

#[derive(Deserialize)]
struct RetiredDeploymentPlan {
    schema: String,
    protocol: String,
    candidate: CandidateDeclaration,
    chain: ChainDeclaration,
    contracts: CandidateContracts,
    source_commit: Option<String>,
}

#[derive(Deserialize)]
struct CandidateDeclaration {
    version: String,
    codename: String,
    status: String,
}

#[derive(Deserialize)]
struct ChainDeclaration {
    chain_id: u64,
}

#[derive(Deserialize)]
struct CandidateContracts {
    #[serde(rename = "BuilderAccountFactory")]
    builder_account_factory: ContractDeclaration,
}

#[derive(Deserialize)]
struct ContractDeclaration {
    deployed: bool,
    planned_address: Address20,
}

fn validate_plan(plan: &RetiredDeploymentPlan) -> Result<()> {
    if plan.schema != "tohseno.deployment-plan/1"
        || plan.protocol != "tohseno"
        || plan.candidate.version != RETIRED_VERSION
        || plan.candidate.codename != "GENESIS"
        || plan.candidate.status != RETIRED_EMBEDDED_STATUS
        || plan.chain.chain_id != ROBINHOOD_CHAIN_ID
        || plan
            .contracts
            .builder_account_factory
            .planned_address
            .as_bytes()
            .iter()
            .all(|byte| *byte == 0)
        || plan.source_commit.is_some()
        || plan.contracts.builder_account_factory.deployed
    {
        return Err(NodeError::Protocol(
            "embedded retired v0.7 identity inputs changed unexpectedly".into(),
        ));
    }
    Ok(())
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
