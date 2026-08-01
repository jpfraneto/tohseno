//! Immutable build definitions for one contract generation.
//!
//! A generation definition records reproducible source, compiler, ABI, bytecode,
//! and conditional CREATE2 facts. It is not deployment evidence and carries no
//! activation block, transaction, authority, or trust-root claim.

use crate::builder::MAX_SAFE_JSON_INTEGER;
use crate::canonical;
use crate::digest::{sha256, Address20, Bytes32};
use crate::text::invalid;
use crate::Result;
use serde::{Deserialize, Serialize};
use sha3::{Digest as _, Keccak256};

pub const CONTRACT_GENERATION_SCHEMA: &str = "tohseno.contract-generation/1";
pub const CONTRACT_GENERATION_PROTOCOL: &str = "tohseno";
pub const CONTRACT_GENERATION_PROTOCOL_MAJOR: u64 = 2;
pub const CONTRACT_SOURCE_TREE_LAW: &str = "tohseno.contract-source-tree/1";
pub const CONTRACT_SOURCE_TREE_DOMAIN: &[u8] = b"TOHSENO-CONTRACT-SOURCE-TREE-V1\0";
pub const EIP7951_STANDARD: &str = "EIP-7951";
pub const EIP7951_GAS: u64 = 6_900;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGeneration {
    pub schema: String,
    pub protocol: String,
    pub protocol_major: u64,
    pub generation: String,
    pub chain: ContractGenerationChain,
    pub source: ContractGenerationSource,
    pub build: SolidityBuildProfile,
    pub contracts: ContractGenerationContracts,
    pub create2: ContractGenerationCreate2,
}

impl ContractGeneration {
    pub fn validate(&self) -> Result<()> {
        if self.schema != CONTRACT_GENERATION_SCHEMA {
            return Err(invalid(
                "contract_generation.schema",
                format!("must be {CONTRACT_GENERATION_SCHEMA}"),
            ));
        }
        if self.protocol != CONTRACT_GENERATION_PROTOCOL {
            return Err(invalid(
                "contract_generation.protocol",
                format!("must be {CONTRACT_GENERATION_PROTOCOL}"),
            ));
        }
        if self.protocol_major != CONTRACT_GENERATION_PROTOCOL_MAJOR {
            return Err(invalid(
                "contract_generation.protocol_major",
                format!("must be {CONTRACT_GENERATION_PROTOCOL_MAJOR}"),
            ));
        }
        validate_version("contract_generation.generation", &self.generation)?;
        self.chain.validate()?;
        self.source.validate()?;
        self.build.validate()?;
        self.contracts.validate()?;
        self.create2.validate(&self.contracts)?;
        Ok(())
    }

    /// SHA-256 of the exact RFC 8785 definition bytes.
    pub fn digest(&self) -> Result<Bytes32> {
        self.validate()?;
        canonical::sha256_commitment(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGenerationChain {
    pub chain_id: u64,
    pub p256_verifier: P256VerifierRequirement,
}

impl ContractGenerationChain {
    fn validate(&self) -> Result<()> {
        if self.chain_id == 0 || self.chain_id > MAX_SAFE_JSON_INTEGER {
            return Err(invalid(
                "contract_generation.chain.chain_id",
                "must be a positive JavaScript-safe integer",
            ));
        }
        self.p256_verifier.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct P256VerifierRequirement {
    pub standard: String,
    pub address: Address20,
    pub gas: u64,
}

impl P256VerifierRequirement {
    fn validate(&self) -> Result<()> {
        if self.standard != EIP7951_STANDARD {
            return Err(invalid(
                "contract_generation.chain.p256_verifier.standard",
                format!("must be {EIP7951_STANDARD}"),
            ));
        }
        let mut expected = [0_u8; 20];
        expected[18] = 1;
        if self.address != Address20::from_bytes(expected) {
            return Err(invalid(
                "contract_generation.chain.p256_verifier.address",
                "EIP-7951 is fixed at address 0x100",
            ));
        }
        if self.gas != EIP7951_GAS {
            return Err(invalid(
                "contract_generation.chain.p256_verifier.gas",
                format!("must be final EIP-7951 cost {EIP7951_GAS}"),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGenerationSource {
    pub commit: String,
    pub tree_law: String,
    pub tree_sha256: Bytes32,
    pub files: Vec<BuildArtifact>,
}

impl ContractGenerationSource {
    fn validate(&self) -> Result<()> {
        validate_commit("contract_generation.source.commit", &self.commit)?;
        if self.tree_law != CONTRACT_SOURCE_TREE_LAW {
            return Err(invalid(
                "contract_generation.source.tree_law",
                format!("must be {CONTRACT_SOURCE_TREE_LAW}"),
            ));
        }
        validate_artifact_inventory(
            "contract_generation.source.files",
            &self.files,
            ArtifactKind::Source,
        )?;
        if self.tree_sha256 != contract_source_tree_digest(&self.files)? {
            return Err(invalid(
                "contract_generation.source.tree_sha256",
                "does not match the declared source inventory",
            ));
        }
        Ok(())
    }
}

/// One exact file in either the source inventory or generation directory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildArtifact {
    pub path: String,
    pub sha256: Bytes32,
    pub byte_length: u64,
}

impl BuildArtifact {
    fn validate(&self, field: &'static str, kind: ArtifactKind) -> Result<()> {
        validate_relative_path(field, &self.path)?;
        match kind {
            ArtifactKind::Source => {
                if self.path != "foundry.toml" && !self.path.starts_with("src/") {
                    return Err(invalid(
                        field,
                        "source paths must be foundry.toml or beneath src/",
                    ));
                }
            }
            ArtifactKind::Abi => {
                if !self.path.starts_with("abi/") || !self.path.ends_with(".json") {
                    return Err(invalid(field, "ABI paths must be abi/*.json"));
                }
            }
            ArtifactKind::CreationBytecode => {
                if !self.path.starts_with("bytecode/") || !self.path.ends_with(".creation.hex") {
                    return Err(invalid(
                        field,
                        "creation-bytecode paths must be bytecode/*.creation.hex",
                    ));
                }
            }
        }
        if self.sha256 == Bytes32::ZERO {
            return Err(invalid(field, "sha256 must not be zero"));
        }
        if self.byte_length == 0 || self.byte_length > MAX_SAFE_JSON_INTEGER {
            return Err(invalid(
                field,
                "byte_length must be a positive JavaScript-safe integer",
            ));
        }
        Ok(())
    }
}

/// Computes `tohseno.contract-source-tree/1`.
///
/// The preimage is `TOHSENO-CONTRACT-SOURCE-TREE-V1\0` followed by one UTF-8
/// line per lexicographically ordered file:
/// `<0x-sha256> <decimal-byte-length> <relative-path>\n`.
pub fn contract_source_tree_digest(files: &[BuildArtifact]) -> Result<Bytes32> {
    validate_artifact_inventory(
        "contract_generation.source.files",
        files,
        ArtifactKind::Source,
    )?;
    let mut preimage = Vec::from(CONTRACT_SOURCE_TREE_DOMAIN);
    for file in files {
        preimage.extend_from_slice(file.sha256.to_string().as_bytes());
        preimage.push(b' ');
        preimage.extend_from_slice(file.byte_length.to_string().as_bytes());
        preimage.push(b' ');
        preimage.extend_from_slice(file.path.as_bytes());
        preimage.push(b'\n');
    }
    Ok(sha256(&preimage))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolidityBuildProfile {
    pub profile: String,
    pub solc_version: String,
    pub evm_version: String,
    pub optimizer: bool,
    pub optimizer_runs: u64,
    pub via_ir: bool,
    pub bytecode_hash: String,
    pub cbor_metadata: bool,
    pub forge_version: String,
    pub forge_commit: String,
}

impl SolidityBuildProfile {
    fn validate(&self) -> Result<()> {
        if self.profile != "default" {
            return Err(invalid(
                "contract_generation.build.profile",
                "must be default",
            ));
        }
        validate_dotted_numeric(
            "contract_generation.build.solc_version",
            &self.solc_version,
            3,
        )?;
        if self.evm_version.is_empty()
            || !self
                .evm_version
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        {
            return Err(invalid(
                "contract_generation.build.evm_version",
                "must be a lowercase ASCII EVM fork name",
            ));
        }
        if !self.optimizer
            || self.optimizer_runs == 0
            || self.optimizer_runs > MAX_SAFE_JSON_INTEGER
        {
            return Err(invalid(
                "contract_generation.build.optimizer_runs",
                "the optimizer must be enabled with a positive safe run count",
            ));
        }
        if self.bytecode_hash != "none" || self.cbor_metadata {
            return Err(invalid(
                "contract_generation.build.metadata",
                "reproducible generations require bytecode_hash none and CBOR metadata disabled",
            ));
        }
        if self.forge_version.is_empty()
            || self.forge_version.len() > 64
            || !self
                .forge_version
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
        {
            return Err(invalid(
                "contract_generation.build.forge_version",
                "must be a short printable version token",
            ));
        }
        validate_commit("contract_generation.build.forge_commit", &self.forge_commit)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGenerationContracts {
    pub builder_account: ContractBuild,
    pub builder_account_factory: ContractBuild,
    pub shot_registry: ContractBuild,
}

impl ContractGenerationContracts {
    fn validate(&self) -> Result<()> {
        self.builder_account
            .validate("contract_generation.contracts.builder_account")?;
        self.builder_account_factory
            .validate("contract_generation.contracts.builder_account_factory")?;
        self.shot_registry
            .validate("contract_generation.contracts.shot_registry")?;

        if self.builder_account.abi.path != "abi/BuilderAccount.json"
            || self.builder_account_factory.abi.path != "abi/BuilderAccountFactory.json"
            || self.shot_registry.abi.path != "abi/ShotRegistry.json"
        {
            return Err(invalid(
                "contract_generation.contracts",
                "each ABI path must identify its exact contract",
            ));
        }
        if self.builder_account.creation_bytecode.is_none()
            || self.builder_account_factory.creation_bytecode.is_some()
            || self.shot_registry.creation_bytecode.is_some()
        {
            return Err(invalid(
                "contract_generation.contracts",
                "only BuilderAccount carries a portable creation-bytecode artifact",
            ));
        }
        if self
            .builder_account
            .creation_bytecode
            .as_ref()
            .is_none_or(|artifact| artifact.path != "bytecode/BuilderAccount.creation.hex")
        {
            return Err(invalid(
                "contract_generation.contracts.builder_account.creation_bytecode",
                "must identify the exact BuilderAccount creation-bytecode artifact",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractBuild {
    pub component_version: String,
    pub abi: BuildArtifact,
    pub creation_bytecode: Option<BuildArtifact>,
    pub creation_code_keccak256: Bytes32,
    /// Keccak-256 of the compiler's deployed-bytecode template. Solidity leaves
    /// zero placeholders at immutable-reference offsets; this is therefore not
    /// necessarily the hash of runtime bytes instantiated by a constructor.
    pub runtime_code_keccak256: Bytes32,
}

impl ContractBuild {
    fn validate(&self, field: &'static str) -> Result<()> {
        validate_version(field, &self.component_version)?;
        self.abi.validate(field, ArtifactKind::Abi)?;
        if let Some(bytecode) = &self.creation_bytecode {
            bytecode.validate(field, ArtifactKind::CreationBytecode)?;
        }
        if self.creation_code_keccak256 == Bytes32::ZERO
            || self.runtime_code_keccak256 == Bytes32::ZERO
        {
            return Err(invalid(field, "code hashes must not be zero"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGenerationCreate2 {
    pub deployer: Address20,
    pub builder_account_factory: Create2Coordinate,
    pub shot_registry: Create2Coordinate,
}

impl ContractGenerationCreate2 {
    fn validate(&self, contracts: &ContractGenerationContracts) -> Result<()> {
        if is_zero_address(self.deployer) {
            return Err(invalid(
                "contract_generation.create2.deployer",
                "must not be zero",
            ));
        }
        self.builder_account_factory.validate(
            "contract_generation.create2.builder_account_factory",
            self.deployer,
            contracts.builder_account_factory.creation_code_keccak256,
        )?;
        self.shot_registry.validate(
            "contract_generation.create2.shot_registry",
            self.deployer,
            contracts.shot_registry.creation_code_keccak256,
        )?;
        if self.builder_account_factory.predicted_address == self.shot_registry.predicted_address {
            return Err(invalid(
                "contract_generation.create2",
                "predicted addresses must be distinct",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create2Coordinate {
    pub salt: Bytes32,
    pub init_code_keccak256: Bytes32,
    pub predicted_address: Address20,
}

impl Create2Coordinate {
    fn validate(
        &self,
        field: &'static str,
        deployer: Address20,
        expected_init_code_hash: Bytes32,
    ) -> Result<()> {
        if self.salt == Bytes32::ZERO || self.init_code_keccak256 == Bytes32::ZERO {
            return Err(invalid(field, "salt and init-code hash must not be zero"));
        }
        if self.init_code_keccak256 != expected_init_code_hash {
            return Err(invalid(
                field,
                "init-code hash differs from the contract build",
            ));
        }
        if is_zero_address(self.predicted_address) || self.predicted_address == deployer {
            return Err(invalid(
                field,
                "predicted address must be nonzero and differ from the deployer",
            ));
        }
        if self.predicted_address
            != predict_create2_address(deployer, self.salt, self.init_code_keccak256)
        {
            return Err(invalid(
                field,
                "predicted address does not satisfy EIP-1014",
            ));
        }
        Ok(())
    }
}

pub fn predict_create2_address(
    deployer: Address20,
    salt: Bytes32,
    init_code_keccak256: Bytes32,
) -> Address20 {
    let mut preimage = [0_u8; 85];
    preimage[0] = 0xff;
    preimage[1..21].copy_from_slice(deployer.as_bytes());
    preimage[21..53].copy_from_slice(salt.as_bytes());
    preimage[53..].copy_from_slice(init_code_keccak256.as_bytes());
    let digest: [u8; 32] = Keccak256::digest(preimage).into();
    let mut address = [0_u8; 20];
    address.copy_from_slice(&digest[12..]);
    Address20::from_bytes(address)
}

#[derive(Clone, Copy)]
enum ArtifactKind {
    Source,
    Abi,
    CreationBytecode,
}

fn validate_artifact_inventory(
    field: &'static str,
    files: &[BuildArtifact],
    kind: ArtifactKind,
) -> Result<()> {
    if files.is_empty() {
        return Err(invalid(field, "must not be empty"));
    }
    let mut previous: Option<&str> = None;
    for file in files {
        file.validate(field, kind)?;
        if previous.is_some_and(|value| value >= file.path.as_str()) {
            return Err(invalid(
                field,
                "paths must be unique and strictly lexicographically ordered",
            ));
        }
        previous = Some(&file.path);
    }
    Ok(())
}

fn validate_relative_path(field: &'static str, path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > 255
        || path.starts_with('/')
        || path.contains('\\')
        || path.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
    {
        return Err(invalid(
            field,
            "must be a normalized relative ASCII path without traversal",
        ));
    }
    Ok(())
}

fn validate_version(field: &'static str, value: &str) -> Result<()> {
    validate_dotted_numeric(field, value, 3)
}

fn validate_dotted_numeric(field: &'static str, value: &str, components: usize) -> Result<()> {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() != components
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
        })
    {
        return Err(invalid(
            field,
            format!("must contain {components} canonical decimal components"),
        ));
    }
    Ok(())
}

fn validate_commit(field: &'static str, value: &str) -> Result<()> {
    if value.len() != 40
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            field,
            "must be a 40-character lowercase Git object ID",
        ));
    }
    Ok(())
}

fn is_zero_address(address: Address20) -> bool {
    address.as_bytes().iter().all(|byte| *byte == 0)
}
