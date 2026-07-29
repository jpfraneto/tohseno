//! Read-only Robinhood Chain verification.
//!
//! The transport vocabulary contains only `eth_chainId`, `eth_getCode`, and
//! `eth_call`. There is intentionally no transaction, signing, wallet, or raw
//! JSON-RPC escape hatch in this module.

use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Number, Value};
use sha3::{Digest as _, Keccak256};
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tohseno_protocol::digest::{Address20, Bytes32};
use tohseno_protocol::identity::ROBINHOOD_CHAIN_ID;
use tohseno_protocol::record::ShotRecord;

pub const NETWORK_STATUS_SCHEMA: &str = "tohseno.network-status/1";
pub const PUBLIC_SHOT_VERIFICATION_SCHEMA: &str = "tohseno.public-shot-verification/1";
pub const DEFAULT_ROBINHOOD_RPC_URL: &str = "https://rpc.mainnet.chain.robinhood.com";

const DEPLOYMENT_PLAN_JSON: &str =
    include_str!("../../contracts/deployments/robinhood-mainnet-genesis.json");
const DEPLOYMENT_PLAN_SCHEMA: &str = "tohseno.deployment-plan/1";
const MAX_DEPLOYMENT_PLAN_BYTES: usize = 128 * 1024;
const MAX_RPC_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_RPC_ERROR_BYTES: usize = 8 * 1024;
const CURL_TIMEOUT: Duration = Duration::from_secs(12);
const RPC_URL_LIMIT: usize = 2_048;
const REPORT_TEXT_LIMIT: usize = 4_096;

const EXPECTED_DEPLOYER: &str = "0x4e59b44847b379578588920ca78fbf26c0b4956c";
const EXPECTED_DEPLOYER_CODE: &str = "0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe03601600081602082378035828234f58015156039578182fd5b8082525050506014600cf3";
const EXPECTED_DEPLOYER_HASH: &str =
    "0x2fa86add0aed31f33a762c9d88e807c475bd51d0f52bd0955754b2608f7e4989";
const EXPECTED_FACTORY_ADDRESS: &str = "0xb802f0ef1595734f2529f602f2473d829d6aafaf";
const EXPECTED_REGISTRY_ADDRESS: &str = "0x5daf4fa6c285afb4b19978ad56a3892e7676cc68";
const EXPECTED_RELATIONS_ADDRESS: &str = "0xb7dc8acfbfc5d93146e4e88d12e5223a5e6a3b83";
const EXPECTED_FACTORY_SALT: &str =
    "0x2d32554fb15a503d75b83ce5d8d53a828c7420f84de1ee1c80af7ee773521800";
const EXPECTED_REGISTRY_SALT: &str =
    "0xbcf41492063a04daa488cd46b8e0e62d6cca2e1da41f58f5bfae84ad42ab6a0f";
const EXPECTED_RELATIONS_SALT: &str =
    "0xecb33008d6d462cb873510b5bd93291242ca14f4156a2fae7d9f04dd9a956c25";
const EXPECTED_FACTORY_INIT_HASH: &str =
    "0xc54e36542c975b6bde3868afded9d3d342e01defdfd0b2c8bc3e25c417526b28";
const EXPECTED_REGISTRY_INIT_HASH: &str =
    "0x8d76b602133b97f4d0adb171cb0a8339f77be15e76b64d3cb2c4077434ffb482";
const EXPECTED_RELATIONS_INIT_HASH: &str =
    "0x50d9e77a0d400b506cc3e0ea439a508a2f243f430245085ee508461b1ff66ea5";
const EXPECTED_FACTORY_RUNTIME_HASH: &str =
    "0x1f44f9fa643277e05f5a9d1f6a05b4cee9264c261a423021c5e0c7f5da3b312a";
const EXPECTED_REGISTRY_RUNTIME_HASH: &str =
    "0xac64e4933d88d40c18af598f7ebf7bc8f7b829e1a61acb8e380d4ac670f31478";
const EXPECTED_RELATIONS_RUNTIME_HASH: &str =
    "0x909ba083f6b186b08f80d5ea465878f7a0c909f1c65b11d2ed8ca11a40669de5";
const EXPECTED_BUILDER_ACCOUNT_RUNTIME_HASH: &str =
    "0xf6986d8ed407dbcf79756d6b08c157998f79771c91996e82b09edab2f2696cba";
const EXPECTED_FACTORY_RUNTIME_SIZE: usize = 9_846;
const EXPECTED_REGISTRY_RUNTIME_SIZE: usize = 8_203;
const EXPECTED_RELATIONS_RUNTIME_SIZE: usize = 9_199;
const EXPECTED_BUILDER_ACCOUNT_RUNTIME_SIZE: usize = 8_132;
const P256VERIFY_ADDRESS: &str = "0x0000000000000000000000000000000000000100";
const P256_KNOWN_INPUT: &str = "0xbb5a52f42f9c9261ed4361f59422a1e30036e7c32b270c8807a419feca6050232ba3a8be6b94d5ec80a6d9d1190a436effe50d85a1eee859b8cc6af9bd5c2e184cd60b855d442f5b3c7b11eb6c4e0ae7525fe710fab9aa7c77a67f79e6fadd762927b10512bae3eddcfe467828128bad2903269919f7086069c8c4df6c732838c7787964eaac00e5921fb1498a60f4606766b3d9685001558d1a974e7341513e";
const P256_KNOWN_OUTPUT: &str =
    "0x0000000000000000000000000000000000000000000000000000000000000001";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentPlan {
    pub schema: String,
    pub protocol: String,
    pub candidate: CandidateDeclaration,
    pub chain: ChainDeclaration,
    pub create2: Create2Declaration,
    pub source_commit: Option<String>,
    pub contracts: CandidateContracts,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateDeclaration {
    pub version: String,
    pub codename: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainDeclaration {
    pub name: String,
    pub chain_id: u64,
    pub p256verify: Address20,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create2Declaration {
    pub deployer: Address20,
    pub deployer_code_must_be_verified_before_broadcast: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateContracts {
    #[serde(rename = "BuilderAccountFactory")]
    pub builder_account_factory: ContractDeclaration,
    #[serde(rename = "ShotRegistry")]
    pub shot_registry: ContractDeclaration,
    #[serde(rename = "ShotRelations")]
    pub shot_relations: ContractDeclaration,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractDeclaration {
    pub constructor_arguments: Vec<Address20>,
    pub deployed: bool,
    pub deployment_order: u8,
    pub init_code_hash: Bytes32,
    pub planned_address: Address20,
    pub runtime_code_hash: Option<Bytes32>,
    pub salt: Bytes32,
    pub transaction_hash: Option<Bytes32>,
}

impl DeploymentPlan {
    pub fn parse(bytes: &[u8]) -> Result<Self, PublicNetworkError> {
        if bytes.len() > MAX_DEPLOYMENT_PLAN_BYTES {
            return Err(PublicNetworkError::DeploymentPlan(format!(
                "deployment plan exceeds {MAX_DEPLOYMENT_PLAN_BYTES} bytes"
            )));
        }
        let unique = serde_json::from_slice::<UniqueJson>(bytes).map_err(|error| {
            PublicNetworkError::DeploymentPlan(format!("invalid strict JSON: {error}"))
        })?;
        let plan = serde_json::from_value::<Self>(unique.0).map_err(|error| {
            PublicNetworkError::DeploymentPlan(format!("schema error: {error}"))
        })?;
        plan.validate()?;
        Ok(plan)
    }

    pub fn validate(&self) -> Result<(), PublicNetworkError> {
        let reject = |reason: &str| PublicNetworkError::DeploymentPlan(reason.into());
        if self.schema != DEPLOYMENT_PLAN_SCHEMA
            || self.protocol != "tohseno"
            || self.candidate.version != "1.0.0-rc.1"
            || self.candidate.codename != "GENESIS"
            || self.candidate.status != "planned, undeployed, non-canonical and unaudited"
        {
            return Err(reject(
                "candidate identity does not match GENESIS 1.0.0-rc.1",
            ));
        }
        if self.chain.name != "Robinhood Chain mainnet"
            || self.chain.chain_id != ROBINHOOD_CHAIN_ID
            || self.chain.p256verify != address(P256VERIFY_ADDRESS)?
        {
            return Err(reject(
                "chain declaration does not match Robinhood Chain mainnet",
            ));
        }
        if self.create2.deployer != address(EXPECTED_DEPLOYER)?
            || !self.create2.deployer_code_must_be_verified_before_broadcast
        {
            return Err(reject("CREATE2 deployer declaration is not pinned"));
        }
        if let Some(commit) = &self.source_commit {
            if commit.len() != 40
                || !commit
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(reject(
                    "source_commit must be null or a lowercase Git SHA-1",
                ));
            }
        }

        validate_contract(
            "BuilderAccountFactory",
            &self.contracts.builder_account_factory,
            1,
            &[],
            EXPECTED_FACTORY_SALT,
            EXPECTED_FACTORY_INIT_HASH,
            EXPECTED_FACTORY_ADDRESS,
        )?;
        validate_contract(
            "ShotRegistry",
            &self.contracts.shot_registry,
            2,
            &[],
            EXPECTED_REGISTRY_SALT,
            EXPECTED_REGISTRY_INIT_HASH,
            EXPECTED_REGISTRY_ADDRESS,
        )?;
        validate_contract(
            "ShotRelations",
            &self.contracts.shot_relations,
            3,
            &[address(EXPECTED_REGISTRY_ADDRESS)?],
            EXPECTED_RELATIONS_SALT,
            EXPECTED_RELATIONS_INIT_HASH,
            EXPECTED_RELATIONS_ADDRESS,
        )?;
        Ok(())
    }
}

pub fn embedded_deployment_plan() -> Result<DeploymentPlan, PublicNetworkError> {
    DeploymentPlan::parse(DEPLOYMENT_PLAN_JSON.as_bytes())
}

fn validate_contract(
    label: &str,
    contract: &ContractDeclaration,
    order: u8,
    constructor_arguments: &[Address20],
    expected_salt: &str,
    expected_init_hash: &str,
    expected_address: &str,
) -> Result<(), PublicNetworkError> {
    let malformed = |reason: &str| PublicNetworkError::DeploymentPlan(format!("{label}: {reason}"));
    if contract.deployment_order != order
        || contract.constructor_arguments != constructor_arguments
        || contract.salt != bytes32(expected_salt)?
        || contract.init_code_hash != bytes32(expected_init_hash)?
        || contract.planned_address != address(expected_address)?
    {
        return Err(malformed("coordinates differ from the pinned candidate"));
    }
    if contract.deployed
        || contract.transaction_hash.is_some()
        || contract.runtime_code_hash.is_some()
    {
        return Err(malformed(
            "embedded plan must remain an undeployed baseline without transaction evidence",
        ));
    }
    let predicted = create2_address(
        address(EXPECTED_DEPLOYER)?,
        contract.salt,
        contract.init_code_hash,
    );
    if predicted != contract.planned_address {
        return Err(malformed("planned address does not reproduce CREATE2"));
    }
    Ok(())
}

fn create2_address(deployer: Address20, salt: Bytes32, init_hash: Bytes32) -> Address20 {
    let mut input = Vec::with_capacity(85);
    input.push(0xff);
    input.extend_from_slice(deployer.as_bytes());
    input.extend_from_slice(salt.as_bytes());
    input.extend_from_slice(init_hash.as_bytes());
    let digest = Keccak256::digest(input);
    let mut output = [0_u8; 20];
    output.copy_from_slice(&digest[12..]);
    Address20::from_bytes(output)
}

#[derive(Clone, Eq, PartialEq)]
pub struct RpcUrl(String);

impl fmt::Debug for RpcUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RpcUrl(<redacted>)")
    }
}

impl RpcUrl {
    pub fn parse(value: impl Into<String>) -> Result<Self, PublicNetworkError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > RPC_URL_LIMIT
            || !value.is_ascii()
            || value.chars().any(char::is_whitespace)
            || value.contains('\\')
            || value.contains('#')
        {
            return Err(PublicNetworkError::InvalidRpcUrl);
        }
        let authority_and_path = value
            .strip_prefix("https://")
            .or_else(|| value.strip_prefix("http://"))
            .ok_or(PublicNetworkError::InvalidRpcUrl)?;
        let authority_end = authority_and_path
            .find(['/', '?'])
            .unwrap_or(authority_and_path.len());
        let authority = &authority_and_path[..authority_end];
        if authority.is_empty() || authority.contains('@') {
            return Err(PublicNetworkError::InvalidRpcUrl);
        }
        validate_authority(authority)?;
        if authority_and_path[authority_end..]
            .bytes()
            .any(|byte| !(0x21..=0x7e).contains(&byte))
        {
            return Err(PublicNetworkError::InvalidRpcUrl);
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_authority(authority: &str) -> Result<(), PublicNetworkError> {
    let (host, port) = if authority.starts_with('[') {
        let end = authority
            .find(']')
            .ok_or(PublicNetworkError::InvalidRpcUrl)?;
        let host = &authority[1..end];
        let remainder = &authority[end + 1..];
        let port = if remainder.is_empty() {
            None
        } else {
            Some(
                remainder
                    .strip_prefix(':')
                    .ok_or(PublicNetworkError::InvalidRpcUrl)?,
            )
        };
        if host.is_empty()
            || !host
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || matches!(byte, b':' | b'.'))
        {
            return Err(PublicNetworkError::InvalidRpcUrl);
        }
        (host, port)
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        if host.contains(':') {
            return Err(PublicNetworkError::InvalidRpcUrl);
        }
        (host, Some(port))
    } else {
        (authority, None)
    };
    if host.is_empty()
        || host.starts_with('.')
        || host.ends_with('.')
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return Err(PublicNetworkError::InvalidRpcUrl);
    }
    if let Some(port) = port {
        let port = port
            .parse::<u16>()
            .map_err(|_| PublicNetworkError::InvalidRpcUrl)?;
        if port == 0 {
            return Err(PublicNetworkError::InvalidRpcUrl);
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReadOnlyRpcRequest {
    ChainId {
        id: u64,
    },
    BlockNumber {
        id: u64,
    },
    GetCode {
        id: u64,
        address: Address20,
        block_number: Option<u64>,
    },
    Call {
        id: u64,
        to: Address20,
        data: Vec<u8>,
        block_number: Option<u64>,
    },
    GetTransactionByHash {
        id: u64,
        transaction_hash: Bytes32,
    },
}

impl ReadOnlyRpcRequest {
    fn id(&self) -> u64 {
        match self {
            Self::ChainId { id }
            | Self::BlockNumber { id }
            | Self::GetCode { id, .. }
            | Self::Call { id, .. }
            | Self::GetTransactionByHash { id, .. } => *id,
        }
    }

    fn json(&self) -> Result<Vec<u8>, PublicNetworkError> {
        let value = match self {
            Self::ChainId { id } => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "eth_chainId",
                "params": []
            }),
            Self::BlockNumber { id } => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "eth_blockNumber",
                "params": []
            }),
            Self::GetCode {
                id,
                address,
                block_number,
            } => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "eth_getCode",
                "params": [address.to_string(), block_tag(*block_number)]
            }),
            Self::Call {
                id,
                to,
                data,
                block_number,
            } => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "eth_call",
                "params": [{
                    "to": to.to_string(),
                    "data": encode_hex(data)
                }, block_tag(*block_number)]
            }),
            Self::GetTransactionByHash {
                id,
                transaction_hash,
            } => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "eth_getTransactionByHash",
                "params": [transaction_hash.to_string()]
            }),
        };
        serde_json::to_vec(&value)
            .map_err(|error| PublicNetworkError::Rpc(format!("request encoding failed: {error}")))
    }
}

fn block_tag(block_number: Option<u64>) -> String {
    block_number
        .map(|number| format!("0x{number:x}"))
        .unwrap_or_else(|| "latest".into())
}

pub trait ReadOnlyRpcTransport {
    fn execute(&self, request: &ReadOnlyRpcRequest) -> Result<Vec<u8>, RpcTransportError>;
}

#[derive(Clone)]
pub struct CurlTransport {
    executable: PathBuf,
    url: RpcUrl,
}

impl fmt::Debug for CurlTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CurlTransport")
            .field("executable", &self.executable)
            .field("url", &"<redacted>")
            .finish()
    }
}

impl CurlTransport {
    pub fn discover(url: RpcUrl) -> Result<Self, PublicNetworkError> {
        for candidate in [
            Path::new("/usr/bin/curl"),
            Path::new("/opt/homebrew/bin/curl"),
            Path::new("/usr/local/bin/curl"),
        ] {
            let Ok(metadata) = fs::symlink_metadata(candidate) else {
                continue;
            };
            if fixed_executable(&metadata) {
                return Ok(Self {
                    executable: candidate.to_path_buf(),
                    url,
                });
            }
        }
        Err(PublicNetworkError::CurlUnavailable)
    }
}

#[cfg(unix)]
fn fixed_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.is_file()
        && !metadata.file_type().is_symlink()
        && metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn fixed_executable(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && !metadata.file_type().is_symlink()
}

impl ReadOnlyRpcTransport for CurlTransport {
    fn execute(&self, request: &ReadOnlyRpcRequest) -> Result<Vec<u8>, RpcTransportError> {
        let body = request
            .json()
            .map_err(|_| RpcTransportError::RequestEncoding)?;
        let mut child = Command::new(&self.executable)
            .args([
                "--disable",
                "--silent",
                "--show-error",
                "--fail-with-body",
                "--proto",
                "=http,https",
                "--connect-timeout",
                "5",
                "--max-time",
                "10",
                "--max-filesize",
                "1048576",
                "--request",
                "POST",
                "--header",
                "Content-Type: application/json",
                "--data-binary",
                "@-",
                "--url",
                self.url.as_str(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| RpcTransportError::Spawn)?;
        let mut stdin = child.stdin.take().ok_or(RpcTransportError::Spawn)?;
        if stdin.write_all(&body).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RpcTransportError::Input);
        }
        drop(stdin);

        let stdout = child.stdout.take().ok_or(RpcTransportError::Spawn)?;
        let stderr = child.stderr.take().ok_or(RpcTransportError::Spawn)?;
        let stdout_reader = thread::spawn(move || read_limited(stdout, MAX_RPC_RESPONSE_BYTES));
        let stderr_reader = thread::spawn(move || read_limited(stderr, MAX_RPC_ERROR_BYTES));
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() < CURL_TIMEOUT => {
                    thread::sleep(Duration::from_millis(10));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Err(RpcTransportError::Timeout);
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Err(RpcTransportError::Wait);
                }
            }
        };
        let stdout = stdout_reader
            .join()
            .map_err(|_| RpcTransportError::Output)?
            .map_err(|_| RpcTransportError::Output)?;
        // Drain stderr without ever surfacing or logging it: RPC gateways can
        // reflect confidential URL material in diagnostics.
        let _ = stderr_reader.join();
        if !status.success() {
            return Err(RpcTransportError::HttpFailure(status.code()));
        }
        if stdout.len() > MAX_RPC_RESPONSE_BYTES {
            return Err(RpcTransportError::OutputTooLarge);
        }
        Ok(stdout)
    }
}

fn read_limited(reader: impl Read, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    reader
        .take(u64::try_from(limit).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut output)?;
    Ok(output)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RpcTransportError {
    RequestEncoding,
    Spawn,
    Input,
    Timeout,
    Wait,
    Output,
    OutputTooLarge,
    HttpFailure(Option<i32>),
}

impl fmt::Display for RpcTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestEncoding => formatter.write_str("RPC request encoding failed"),
            Self::Spawn => formatter.write_str("the fixed curl executable could not start"),
            Self::Input => formatter.write_str("the RPC request body could not be written"),
            Self::Timeout => formatter.write_str("the read-only RPC request timed out"),
            Self::Wait => formatter.write_str("the curl process could not be observed"),
            Self::Output => formatter.write_str("the RPC response could not be read"),
            Self::OutputTooLarge => formatter.write_str("the RPC response exceeded its limit"),
            Self::HttpFailure(Some(code)) => {
                write!(
                    formatter,
                    "the RPC HTTP request failed with curl status {code}"
                )
            }
            Self::HttpFailure(None) => formatter.write_str("the RPC HTTP request was terminated"),
        }
    }
}

impl std::error::Error for RpcTransportError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicCheckStatus {
    Pass,
    Fail,
    NotChecked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicCheck {
    pub id: String,
    pub status: PublicCheckStatus,
    pub expected: String,
    pub observed: String,
    pub evidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkStatusReport {
    pub schema: String,
    pub ready: bool,
    pub chain_id: Option<u64>,
    pub checks: Vec<PublicCheck>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicShotVerificationReport {
    pub schema: String,
    pub verified: bool,
    pub shot_id: String,
    pub sequence: u32,
    pub evolution_commitment: Option<Bytes32>,
    pub observed: ObservedPublicShotState,
    pub network: NetworkStatusReport,
    pub checks: Vec<PublicCheck>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedPublicShotState {
    pub controller: Option<Address20>,
    pub head: Option<Bytes32>,
    pub sequence: Option<u64>,
    pub relations_registry: Option<Address20>,
}

pub const PUBLIC_PREPARATION_SCHEMA: &str = "tohseno.public-preparation-read/1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderAccountCodeState {
    Missing,
    Exact,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderAccountObservation {
    pub account: Address20,
    pub queried_key_id: Bytes32,
    pub code_state: BuilderAccountCodeState,
    pub runtime_size: usize,
    pub runtime_keccak256: Option<Bytes32>,
    pub protocol_permission: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryPreparationState {
    pub controller: Address20,
    pub head: Bytes32,
    pub sequence: u64,
    pub shot_nonce: u64,
    pub create_nonce: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationsPreparationState {
    pub nonce: u64,
    pub handle_by_shot: Bytes32,
    pub shot_by_requested_handle: Option<Bytes32>,
    pub appcoin_chain_id: u64,
    pub appcoin_token: Address20,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationRead {
    None,
    Handle(Bytes32),
    Appcoin,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicPreparationRead {
    pub schema: String,
    pub block_number: Option<u64>,
    pub read_complete: bool,
    pub network: NetworkStatusReport,
    pub builder_account: Option<BuilderAccountObservation>,
    pub registry: Option<RegistryPreparationState>,
    pub relations: Option<RelationsPreparationState>,
    pub error: Option<String>,
}

#[derive(Clone)]
struct CodeExpectation {
    digest: Bytes32,
    size: usize,
    exact: Option<Vec<u8>>,
}

#[derive(Clone)]
struct CodeExpectations {
    deployer: CodeExpectation,
    factory: CodeExpectation,
    registry: CodeExpectation,
    relations: CodeExpectation,
}

fn production_code_expectations() -> Result<CodeExpectations, PublicNetworkError> {
    let deployer_code = decode_data_hex(EXPECTED_DEPLOYER_CODE, 1024, false)?;
    Ok(CodeExpectations {
        deployer: CodeExpectation {
            digest: bytes32(EXPECTED_DEPLOYER_HASH)?,
            size: deployer_code.len(),
            exact: Some(deployer_code),
        },
        factory: CodeExpectation {
            digest: bytes32(EXPECTED_FACTORY_RUNTIME_HASH)?,
            size: EXPECTED_FACTORY_RUNTIME_SIZE,
            exact: None,
        },
        registry: CodeExpectation {
            digest: bytes32(EXPECTED_REGISTRY_RUNTIME_HASH)?,
            size: EXPECTED_REGISTRY_RUNTIME_SIZE,
            exact: None,
        },
        relations: CodeExpectation {
            digest: bytes32(EXPECTED_RELATIONS_RUNTIME_HASH)?,
            size: EXPECTED_RELATIONS_RUNTIME_SIZE,
            exact: None,
        },
    })
}

pub fn network_status<T: ReadOnlyRpcTransport>(
    transport: &T,
    plan: &DeploymentPlan,
) -> NetworkStatusReport {
    match production_code_expectations() {
        Ok(expectations) => network_status_with_expectations(transport, plan, &expectations),
        Err(error) => NetworkStatusReport {
            schema: NETWORK_STATUS_SCHEMA.into(),
            ready: false,
            chain_id: None,
            checks: vec![failed(
                "candidate.constants",
                "internally valid pinned candidate constants",
                &error.to_string(),
                "embedded:public_network.rs",
            )],
        },
    }
}

fn network_status_with_expectations<T: ReadOnlyRpcTransport>(
    transport: &T,
    plan: &DeploymentPlan,
    expectations: &CodeExpectations,
) -> NetworkStatusReport {
    network_status_with_expectations_at(transport, plan, expectations, None)
}

fn network_status_with_expectations_at<T: ReadOnlyRpcTransport>(
    transport: &T,
    plan: &DeploymentPlan,
    expectations: &CodeExpectations,
    block_number: Option<u64>,
) -> NetworkStatusReport {
    let mut checks = Vec::new();
    if let Err(error) = plan.validate() {
        checks.push(failed(
            "candidate.plan",
            "strict embedded GENESIS deployment plan",
            &error.to_string(),
            "embedded:contracts/deployments/robinhood-mainnet-genesis.json",
        ));
        return NetworkStatusReport {
            schema: NETWORK_STATUS_SCHEMA.into(),
            ready: false,
            chain_id: None,
            checks,
        };
    }
    checks.push(passed(
        "candidate.plan",
        "strict embedded GENESIS deployment plan",
        "planned, undeployed baseline parsed and CREATE2 coordinates reproduced",
        "embedded:contracts/deployments/robinhood-mainnet-genesis.json",
    ));

    let chain = rpc_quantity(
        transport,
        &ReadOnlyRpcRequest::ChainId { id: 1 },
        "eth_chainId",
    );
    let chain_id = chain.as_ref().ok().copied();
    let chain_matches = match chain {
        Ok(value) if value == ROBINHOOD_CHAIN_ID => Ok(format!("{value}")),
        Ok(value) => Err(format!("wrong chain ID {value}")),
        Err(error) => Err(error.to_string()),
    };
    checks.push(result_check(
        "network.chain_id",
        &ROBINHOOD_CHAIN_ID.to_string(),
        &chain_matches,
        "rpc:eth_chainId",
    ));
    if chain_id != Some(ROBINHOOD_CHAIN_ID) {
        for (id, expected, evidence) in deferred_network_checks(plan) {
            checks.push(not_checked(
                id,
                &expected,
                "refused because chain 4663 was not established",
                &evidence,
            ));
        }
        return NetworkStatusReport {
            schema: NETWORK_STATUS_SCHEMA.into(),
            ready: false,
            chain_id,
            checks,
        };
    }

    let deployer = verify_code(
        transport,
        plan.create2.deployer,
        &expectations.deployer,
        2,
        block_number,
    );
    checks.push(result_check(
        "network.code.deployer",
        &format!(
            "{} bytes, keccak256 {}",
            expectations.deployer.size, expectations.deployer.digest
        ),
        &deployer,
        &format!("rpc:eth_getCode:{}", plan.create2.deployer),
    ));
    let factory = verify_code(
        transport,
        plan.contracts.builder_account_factory.planned_address,
        &expectations.factory,
        3,
        block_number,
    );
    checks.push(result_check(
        "network.code.builder_account_factory",
        &format!(
            "{} bytes, keccak256 {}",
            expectations.factory.size, expectations.factory.digest
        ),
        &factory,
        &format!(
            "rpc:eth_getCode:{}",
            plan.contracts.builder_account_factory.planned_address
        ),
    ));
    let registry = verify_code(
        transport,
        plan.contracts.shot_registry.planned_address,
        &expectations.registry,
        4,
        block_number,
    );
    checks.push(result_check(
        "network.code.shot_registry",
        &format!(
            "{} bytes, keccak256 {}",
            expectations.registry.size, expectations.registry.digest
        ),
        &registry,
        &format!(
            "rpc:eth_getCode:{}",
            plan.contracts.shot_registry.planned_address
        ),
    ));
    let relations = verify_code(
        transport,
        plan.contracts.shot_relations.planned_address,
        &expectations.relations,
        5,
        block_number,
    );
    checks.push(result_check(
        "network.code.shot_relations",
        &format!(
            "{} bytes, keccak256 {}",
            expectations.relations.size, expectations.relations.digest
        ),
        &relations,
        &format!(
            "rpc:eth_getCode:{}",
            plan.contracts.shot_relations.planned_address
        ),
    ));

    let p256_input = decode_data_hex(P256_KNOWN_INPUT, 160, false);
    let p256: Result<String, PublicNetworkError> = match p256_input {
        Ok(input) => rpc_call_at(
            transport,
            plan.chain.p256verify,
            input,
            6,
            "P256VERIFY known vector",
            block_number,
        )
        .and_then(|output| {
            let expected = decode_data_hex(P256_KNOWN_OUTPUT, 32, false)?;
            if output == expected {
                Ok("exact 32-byte integer 1".into())
            } else {
                Err(PublicNetworkError::Rpc(format!(
                    "expected {P256_KNOWN_OUTPUT}, observed {}",
                    encode_hex(&output)
                )))
            }
        }),
        Err(error) => Err(error),
    };
    let p256 = p256.map_err(|error| error.to_string());
    checks.push(result_check(
        "network.p256verify",
        "EIP-7951 known vector returns exactly 32-byte integer 1",
        &p256,
        "rpc:eth_call:0x0000000000000000000000000000000000000100",
    ));

    let relation_binding = if registry.is_ok() && relations.is_ok() {
        rpc_call_at(
            transport,
            plan.contracts.shot_relations.planned_address,
            abi_no_args("registry()"),
            7,
            "ShotRelations.registry",
            block_number,
        )
        .and_then(|output| decode_abi_address(&output))
        .and_then(|observed| {
            if observed == plan.contracts.shot_registry.planned_address {
                Ok(format!("{observed}"))
            } else {
                Err(PublicNetworkError::Rpc(format!(
                    "relations points to {observed}"
                )))
            }
        })
        .map_err(|error| error.to_string())
    } else {
        Err("registry or relations runtime code was not verified".into())
    };
    checks.push(if registry.is_ok() && relations.is_ok() {
        result_check(
            "network.relations.registry",
            &plan.contracts.shot_registry.planned_address.to_string(),
            &relation_binding,
            "rpc:eth_call:ShotRelations.registry()",
        )
    } else {
        not_checked(
            "network.relations.registry",
            &plan.contracts.shot_registry.planned_address.to_string(),
            "registry or relations runtime code was not verified",
            "rpc:eth_call:ShotRelations.registry()",
        )
    });

    let ready = checks
        .iter()
        .all(|check| check.status == PublicCheckStatus::Pass);
    NetworkStatusReport {
        schema: NETWORK_STATUS_SCHEMA.into(),
        ready,
        chain_id,
        checks,
    }
}

fn deferred_network_checks(plan: &DeploymentPlan) -> Vec<(&'static str, String, String)> {
    vec![
        (
            "network.code.deployer",
            "exact deterministic deployer runtime".into(),
            format!("rpc:eth_getCode:{}", plan.create2.deployer),
        ),
        (
            "network.code.builder_account_factory",
            "exact BuilderAccountFactory runtime".into(),
            format!(
                "rpc:eth_getCode:{}",
                plan.contracts.builder_account_factory.planned_address
            ),
        ),
        (
            "network.code.shot_registry",
            "exact ShotRegistry runtime".into(),
            format!(
                "rpc:eth_getCode:{}",
                plan.contracts.shot_registry.planned_address
            ),
        ),
        (
            "network.code.shot_relations",
            "exact ShotRelations runtime".into(),
            format!(
                "rpc:eth_getCode:{}",
                plan.contracts.shot_relations.planned_address
            ),
        ),
        (
            "network.p256verify",
            "EIP-7951 known-vector success".into(),
            "rpc:eth_call:0x0000000000000000000000000000000000000100".into(),
        ),
        (
            "network.relations.registry",
            plan.contracts.shot_registry.planned_address.to_string(),
            "rpc:eth_call:ShotRelations.registry()".into(),
        ),
    ]
}

fn verify_code<T: ReadOnlyRpcTransport>(
    transport: &T,
    address: Address20,
    expected: &CodeExpectation,
    id: u64,
    block_number: Option<u64>,
) -> Result<String, String> {
    let code = rpc_data(
        transport,
        &ReadOnlyRpcRequest::GetCode {
            id,
            address,
            block_number,
        },
        "eth_getCode",
        MAX_RPC_RESPONSE_BYTES / 2,
        true,
    )
    .map_err(|error| error.to_string())?;
    if code.is_empty() {
        return Err("no code at the planned address; candidate is undeployed".into());
    }
    let digest = Bytes32::new(Keccak256::digest(&code).into());
    if code.len() != expected.size || digest != expected.digest {
        return Err(format!(
            "observed {} bytes with keccak256 {digest}",
            code.len()
        ));
    }
    if expected.exact.as_ref().is_some_and(|exact| exact != &code) {
        return Err("runtime hash matched but exact pinned bytes differed".into());
    }
    Ok(format!("{} bytes with keccak256 {digest}", code.len()))
}

pub fn read_public_preparation<T: ReadOnlyRpcTransport>(
    transport: &T,
    plan: &DeploymentPlan,
    record: &ShotRecord,
    controller: Address20,
    signer_key_id: Bytes32,
    relation: RelationRead,
) -> PublicPreparationRead {
    let block_number = match rpc_quantity(
        transport,
        &ReadOnlyRpcRequest::BlockNumber { id: 190 },
        "eth_blockNumber",
    ) {
        Ok(value) => value,
        Err(error) => {
            return PublicPreparationRead {
                schema: PUBLIC_PREPARATION_SCHEMA.into(),
                block_number: None,
                read_complete: false,
                network: NetworkStatusReport {
                    schema: NETWORK_STATUS_SCHEMA.into(),
                    ready: false,
                    chain_id: None,
                    checks: vec![failed(
                        "network.block_number",
                        "a concrete block for one coherent preparation snapshot",
                        &error.to_string(),
                        "rpc:eth_blockNumber",
                    )],
                },
                builder_account: None,
                registry: None,
                relations: None,
                error: Some("a concrete preparation block could not be established".into()),
            }
        }
    };
    let expectations = match production_code_expectations() {
        Ok(expectations) => expectations,
        Err(error) => {
            return PublicPreparationRead {
                schema: PUBLIC_PREPARATION_SCHEMA.into(),
                block_number: Some(block_number),
                read_complete: false,
                network: NetworkStatusReport {
                    schema: NETWORK_STATUS_SCHEMA.into(),
                    ready: false,
                    chain_id: None,
                    checks: vec![failed(
                        "candidate.constants",
                        "internally valid pinned candidate constants",
                        &error.to_string(),
                        "embedded:public_network.rs",
                    )],
                },
                builder_account: None,
                registry: None,
                relations: None,
                error: Some(error.to_string()),
            }
        }
    };
    let network =
        network_status_with_expectations_at(transport, plan, &expectations, Some(block_number));
    if !network.ready {
        return PublicPreparationRead {
            schema: PUBLIC_PREPARATION_SCHEMA.into(),
            block_number: Some(block_number),
            read_complete: false,
            network,
            builder_account: None,
            registry: None,
            relations: None,
            error: Some(
                "chain 4663 and every pinned candidate runtime must pass before signing".into(),
            ),
        };
    }
    if record.builder_id.account() != controller {
        return PublicPreparationRead {
            schema: PUBLIC_PREPARATION_SCHEMA.into(),
            block_number: Some(block_number),
            read_complete: false,
            network,
            builder_account: None,
            registry: None,
            relations: None,
            error: Some("local record controller does not match the loaded BuilderID".into()),
        };
    }

    let builder_expectation = match bytes32(EXPECTED_BUILDER_ACCOUNT_RUNTIME_HASH) {
        Ok(digest) => CodeExpectation {
            digest,
            size: EXPECTED_BUILDER_ACCOUNT_RUNTIME_SIZE,
            exact: None,
        },
        Err(error) => {
            return PublicPreparationRead {
                schema: PUBLIC_PREPARATION_SCHEMA.into(),
                block_number: Some(block_number),
                read_complete: false,
                network,
                builder_account: None,
                registry: None,
                relations: None,
                error: Some(error.to_string()),
            }
        }
    };
    read_public_preparation_with_builder_expectation(
        transport,
        plan,
        record,
        controller,
        signer_key_id,
        relation,
        &builder_expectation,
        block_number,
        network,
    )
}

#[allow(clippy::too_many_arguments)]
fn read_public_preparation_with_builder_expectation<T: ReadOnlyRpcTransport>(
    transport: &T,
    plan: &DeploymentPlan,
    record: &ShotRecord,
    controller: Address20,
    signer_key_id: Bytes32,
    relation: RelationRead,
    builder_expectation: &CodeExpectation,
    block_number: u64,
    network: NetworkStatusReport,
) -> PublicPreparationRead {
    let result = (|| {
        let builder_account = observe_builder_account(
            transport,
            controller,
            signer_key_id,
            builder_expectation,
            block_number,
        )?;
        let shot = *record.shot_id.bytes().as_bytes();
        let registry = plan.contracts.shot_registry.planned_address;
        let observed_controller = rpc_call_at(
            transport,
            registry,
            abi_bytes32("controllerOf(bytes32)", shot),
            201,
            "ShotRegistry.controllerOf",
            Some(block_number),
        )
        .and_then(|output| decode_abi_address(&output))?;
        let head = rpc_call_at(
            transport,
            registry,
            abi_bytes32("headOf(bytes32)", shot),
            202,
            "ShotRegistry.headOf",
            Some(block_number),
        )
        .and_then(|output| decode_abi_bytes32(&output))?;
        let sequence = rpc_call_at(
            transport,
            registry,
            abi_bytes32("sequenceOf(bytes32)", shot),
            203,
            "ShotRegistry.sequenceOf",
            Some(block_number),
        )
        .and_then(|output| decode_abi_u64(&output))?;
        let shot_nonce = rpc_call_at(
            transport,
            registry,
            abi_bytes32("nonceOf(bytes32)", shot),
            204,
            "ShotRegistry.nonceOf",
            Some(block_number),
        )
        .and_then(|output| decode_abi_u64(&output))?;
        let create_nonce = rpc_call_at(
            transport,
            registry,
            abi_address_argument("createNonces(address)", controller),
            205,
            "ShotRegistry.createNonces",
            Some(block_number),
        )
        .and_then(|output| decode_abi_u64(&output))?;
        let registry = RegistryPreparationState {
            controller: observed_controller,
            head,
            sequence,
            shot_nonce,
            create_nonce,
        };

        let relations = match relation {
            RelationRead::None => None,
            RelationRead::Handle(handle_hash) => Some(observe_relations(
                transport,
                plan.contracts.shot_relations.planned_address,
                shot,
                Some(handle_hash),
                block_number,
            )?),
            RelationRead::Appcoin => Some(observe_relations(
                transport,
                plan.contracts.shot_relations.planned_address,
                shot,
                None,
                block_number,
            )?),
        };
        Ok::<_, PublicNetworkError>((builder_account, registry, relations))
    })();

    match result {
        Ok((builder_account, registry, relations)) => PublicPreparationRead {
            schema: PUBLIC_PREPARATION_SCHEMA.into(),
            block_number: Some(block_number),
            read_complete: true,
            network,
            builder_account: Some(builder_account),
            registry: Some(registry),
            relations,
            error: None,
        },
        Err(error) => PublicPreparationRead {
            schema: PUBLIC_PREPARATION_SCHEMA.into(),
            block_number: Some(block_number),
            read_complete: false,
            network,
            builder_account: None,
            registry: None,
            relations: None,
            error: Some(bounded(&error.to_string())),
        },
    }
}

fn observe_builder_account<T: ReadOnlyRpcTransport>(
    transport: &T,
    account: Address20,
    signer_key_id: Bytes32,
    expectation: &CodeExpectation,
    block_number: u64,
) -> Result<BuilderAccountObservation, PublicNetworkError> {
    let code = rpc_data(
        transport,
        &ReadOnlyRpcRequest::GetCode {
            id: 200,
            address: account,
            block_number: Some(block_number),
        },
        "eth_getCode",
        MAX_RPC_RESPONSE_BYTES / 2,
        true,
    )?;
    if code.is_empty() {
        return Ok(BuilderAccountObservation {
            account,
            queried_key_id: signer_key_id,
            code_state: BuilderAccountCodeState::Missing,
            runtime_size: 0,
            runtime_keccak256: None,
            protocol_permission: None,
        });
    }
    let digest = Bytes32::new(Keccak256::digest(&code).into());
    if code.len() != expectation.size || digest != expectation.digest {
        return Err(PublicNetworkError::Rpc(format!(
            "BuilderAccount at {account} has {} bytes and keccak256 {digest}, not the pinned runtime",
            code.len()
        )));
    }
    let permission = rpc_call_at(
        transport,
        account,
        abi_bytes32_u32("hasPermission(bytes32,uint32)", signer_key_id, 1),
        206,
        "BuilderAccount.hasPermission",
        Some(block_number),
    )
    .and_then(|output| decode_abi_bool(&output))?;
    Ok(BuilderAccountObservation {
        account,
        queried_key_id: signer_key_id,
        code_state: BuilderAccountCodeState::Exact,
        runtime_size: code.len(),
        runtime_keccak256: Some(digest),
        protocol_permission: Some(permission),
    })
}

fn observe_relations<T: ReadOnlyRpcTransport>(
    transport: &T,
    relations: Address20,
    shot: [u8; 32],
    requested_handle: Option<Bytes32>,
    block_number: u64,
) -> Result<RelationsPreparationState, PublicNetworkError> {
    let nonce = rpc_call_at(
        transport,
        relations,
        abi_bytes32("nonces(bytes32)", shot),
        207,
        "ShotRelations.nonces",
        Some(block_number),
    )
    .and_then(|output| decode_abi_u64(&output))?;
    let handle_by_shot = rpc_call_at(
        transport,
        relations,
        abi_bytes32("handleByShot(bytes32)", shot),
        208,
        "ShotRelations.handleByShot",
        Some(block_number),
    )
    .and_then(|output| decode_abi_bytes32(&output))?;
    let shot_by_requested_handle = requested_handle
        .map(|handle| {
            rpc_call_at(
                transport,
                relations,
                abi_bytes32("shotByHandle(bytes32)", *handle.as_bytes()),
                209,
                "ShotRelations.shotByHandle",
                Some(block_number),
            )
            .and_then(|output| decode_abi_bytes32(&output))
        })
        .transpose()?;
    let (appcoin_chain_id, appcoin_token) = rpc_call_at(
        transport,
        relations,
        abi_bytes32("appcoinOf(bytes32)", shot),
        210,
        "ShotRelations.appcoinOf",
        Some(block_number),
    )
    .and_then(|output| decode_abi_u64_address(&output))?;
    Ok(RelationsPreparationState {
        nonce,
        handle_by_shot,
        shot_by_requested_handle,
        appcoin_chain_id,
        appcoin_token,
    })
}

pub fn verify_public_shot<T: ReadOnlyRpcTransport>(
    transport: &T,
    plan: &DeploymentPlan,
    record: &ShotRecord,
) -> PublicShotVerificationReport {
    match production_code_expectations() {
        Ok(expectations) => {
            verify_public_shot_with_expectations(transport, plan, record, &expectations)
        }
        Err(error) => PublicShotVerificationReport {
            schema: PUBLIC_SHOT_VERIFICATION_SCHEMA.into(),
            verified: false,
            shot_id: record.shot_id.to_string(),
            sequence: record.sequence,
            evolution_commitment: None,
            observed: ObservedPublicShotState::default(),
            network: NetworkStatusReport {
                schema: NETWORK_STATUS_SCHEMA.into(),
                ready: false,
                chain_id: None,
                checks: vec![failed(
                    "candidate.constants",
                    "internally valid pinned candidate constants",
                    &error.to_string(),
                    "embedded:public_network.rs",
                )],
            },
            checks: vec![not_checked(
                "public.record",
                "validated local Shot record",
                "candidate constants were invalid",
                "local:TOHSENO/shot.json",
            )],
        },
    }
}

fn verify_public_shot_with_expectations<T: ReadOnlyRpcTransport>(
    transport: &T,
    plan: &DeploymentPlan,
    record: &ShotRecord,
    expectations: &CodeExpectations,
) -> PublicShotVerificationReport {
    let network = network_status_with_expectations(transport, plan, expectations);
    let mut checks = Vec::new();
    let record_validation = record
        .validate()
        .map(|_| "closed local Shot record validated".to_owned())
        .map_err(|error| error.to_string());
    let record_valid = record_validation.is_ok();
    checks.push(result_check(
        "public.record",
        "valid local tohseno.shot/1 record",
        &record_validation,
        "local:TOHSENO/shot.json",
    ));
    let commitment = record.commitment().map_err(|error| error.to_string());
    checks.push(result_check(
        "public.local_head",
        "RFC 8785 SHA-256 Evolution commitment",
        &commitment
            .as_ref()
            .map(|digest| format!("recomputed {digest}")),
        "local:TOHSENO/shot.json",
    ));
    let commitment_value = commitment.ok();

    let chain_ready = network.chain_id == Some(ROBINHOOD_CHAIN_ID);
    let registry_ready = check_passed(&network, "network.code.shot_registry");
    let relations_ready = check_passed(&network, "network.code.shot_relations");
    if !record_valid || commitment_value.is_none() {
        for (id, expected, evidence) in [
            (
                "public.controller",
                "controller from a valid local record".into(),
                "rpc:eth_call:ShotRegistry.controllerOf(bytes32)",
            ),
            (
                "public.head",
                "commitment from a valid local record".into(),
                "rpc:eth_call:ShotRegistry.headOf(bytes32)",
            ),
            (
                "public.sequence",
                "sequence from a valid local record".into(),
                "rpc:eth_call:ShotRegistry.sequenceOf(bytes32)",
            ),
            (
                "public.relations_binding",
                plan.contracts.shot_registry.planned_address.to_string(),
                "rpc:eth_call:ShotRelations.registry()",
            ),
        ] {
            checks.push(not_checked(
                id,
                &expected,
                "local Shot record did not validate",
                evidence,
            ));
        }
        return PublicShotVerificationReport {
            schema: PUBLIC_SHOT_VERIFICATION_SCHEMA.into(),
            verified: false,
            shot_id: record.shot_id.to_string(),
            sequence: record.sequence,
            evolution_commitment: commitment_value,
            observed: ObservedPublicShotState::default(),
            network,
            checks,
        };
    }
    if !chain_ready || !registry_ready {
        let reason = if !chain_ready {
            "chain 4663 was not established"
        } else {
            "ShotRegistry is undeployed or its runtime identity failed"
        };
        for (id, expected, evidence) in [
            (
                "public.controller",
                record.builder_id.account().to_string(),
                "rpc:eth_call:ShotRegistry.controllerOf(bytes32)",
            ),
            (
                "public.head",
                commitment_value
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "valid local commitment".into()),
                "rpc:eth_call:ShotRegistry.headOf(bytes32)",
            ),
            (
                "public.sequence",
                record.sequence.to_string(),
                "rpc:eth_call:ShotRegistry.sequenceOf(bytes32)",
            ),
        ] {
            checks.push(not_checked(id, &expected, reason, evidence));
        }
        checks.push(not_checked(
            "public.relations_binding",
            &plan.contracts.shot_registry.planned_address.to_string(),
            if relations_ready {
                reason
            } else {
                "ShotRelations is undeployed or its runtime identity failed"
            },
            "rpc:eth_call:ShotRelations.registry()",
        ));
        return PublicShotVerificationReport {
            schema: PUBLIC_SHOT_VERIFICATION_SCHEMA.into(),
            verified: false,
            shot_id: record.shot_id.to_string(),
            sequence: record.sequence,
            evolution_commitment: commitment_value,
            observed: ObservedPublicShotState::default(),
            network,
            checks,
        };
    }

    let argument = *record.shot_id.bytes().as_bytes();
    let registry = plan.contracts.shot_registry.planned_address;
    let controller = rpc_call(
        transport,
        registry,
        abi_bytes32("controllerOf(bytes32)", argument),
        101,
        "ShotRegistry.controllerOf",
    )
    .and_then(|output| decode_abi_address(&output))
    .map_err(|error| error.to_string());
    let observed_controller = controller.as_ref().ok().copied();
    let controller_match = controller.and_then(|observed| {
        let expected = record.builder_id.account();
        if observed == expected {
            Ok(observed.to_string())
        } else {
            Err(format!("expected {expected}, observed {observed}"))
        }
    });
    checks.push(result_check(
        "public.controller",
        &record.builder_id.account().to_string(),
        &controller_match,
        "rpc:eth_call:ShotRegistry.controllerOf(bytes32)",
    ));

    let head = rpc_call(
        transport,
        registry,
        abi_bytes32("headOf(bytes32)", argument),
        102,
        "ShotRegistry.headOf",
    )
    .and_then(|output| decode_abi_bytes32(&output))
    .map_err(|error| error.to_string());
    let observed_head = head.as_ref().ok().copied();
    let head_match = match (commitment_value, head) {
        (Some(expected), Ok(observed)) if expected == observed => Ok(observed.to_string()),
        (Some(expected), Ok(observed)) => Err(format!("expected {expected}, observed {observed}")),
        (_, Err(error)) => Err(error),
        (None, _) => Err("valid local commitment was unavailable".into()),
    };
    checks.push(result_check(
        "public.head",
        &commitment_value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "valid local commitment".into()),
        &head_match,
        "rpc:eth_call:ShotRegistry.headOf(bytes32)",
    ));

    let sequence = rpc_call(
        transport,
        registry,
        abi_bytes32("sequenceOf(bytes32)", argument),
        103,
        "ShotRegistry.sequenceOf",
    )
    .and_then(|output| decode_abi_u64(&output))
    .map_err(|error| error.to_string());
    let observed_sequence = sequence.as_ref().ok().copied();
    let sequence_match = sequence.and_then(|observed| {
        if observed == u64::from(record.sequence) {
            Ok(observed.to_string())
        } else {
            Err(format!("expected {}, observed {observed}", record.sequence))
        }
    });
    checks.push(result_check(
        "public.sequence",
        &record.sequence.to_string(),
        &sequence_match,
        "rpc:eth_call:ShotRegistry.sequenceOf(bytes32)",
    ));

    let observed_relations_registry = if relations_ready {
        rpc_call(
            transport,
            plan.contracts.shot_relations.planned_address,
            abi_no_args("registry()"),
            104,
            "ShotRelations.registry",
        )
        .and_then(|output| decode_abi_address(&output))
        .map_err(|error| error.to_string())
    } else {
        Err("ShotRelations is undeployed or its runtime identity failed".into())
    };
    let relations_registry = observed_relations_registry.as_ref().ok().copied();
    let relations_binding = observed_relations_registry.and_then(|observed| {
        if observed == registry {
            Ok(observed.to_string())
        } else {
            Err(format!("expected {registry}, observed {observed}"))
        }
    });
    checks.push(if relations_ready {
        result_check(
            "public.relations_binding",
            &registry.to_string(),
            &relations_binding,
            "rpc:eth_call:ShotRelations.registry()",
        )
    } else {
        not_checked(
            "public.relations_binding",
            &registry.to_string(),
            "ShotRelations is undeployed or its runtime identity failed",
            "rpc:eth_call:ShotRelations.registry()",
        )
    });

    let verified = network.ready
        && checks
            .iter()
            .all(|check| check.status == PublicCheckStatus::Pass);
    PublicShotVerificationReport {
        schema: PUBLIC_SHOT_VERIFICATION_SCHEMA.into(),
        verified,
        shot_id: record.shot_id.to_string(),
        sequence: record.sequence,
        evolution_commitment: commitment_value,
        observed: ObservedPublicShotState {
            controller: observed_controller,
            head: observed_head,
            sequence: observed_sequence,
            relations_registry,
        },
        network,
        checks,
    }
}

fn check_passed(report: &NetworkStatusReport, id: &str) -> bool {
    report
        .checks
        .iter()
        .any(|check| check.id == id && check.status == PublicCheckStatus::Pass)
}

fn abi_no_args(signature: &str) -> Vec<u8> {
    Keccak256::digest(signature.as_bytes())[..4].to_vec()
}

fn abi_bytes32(signature: &str, argument: [u8; 32]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(36);
    encoded.extend_from_slice(&Keccak256::digest(signature.as_bytes())[..4]);
    encoded.extend_from_slice(&argument);
    encoded
}

fn abi_address_argument(signature: &str, argument: Address20) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(36);
    encoded.extend_from_slice(&Keccak256::digest(signature.as_bytes())[..4]);
    encoded.extend_from_slice(&[0_u8; 12]);
    encoded.extend_from_slice(argument.as_bytes());
    encoded
}

fn abi_bytes32_u32(signature: &str, first: Bytes32, second: u32) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(68);
    encoded.extend_from_slice(&Keccak256::digest(signature.as_bytes())[..4]);
    encoded.extend_from_slice(first.as_bytes());
    encoded.extend_from_slice(&[0_u8; 28]);
    encoded.extend_from_slice(&second.to_be_bytes());
    encoded
}

fn decode_abi_address(output: &[u8]) -> Result<Address20, PublicNetworkError> {
    if output.len() != 32 || output[..12].iter().any(|byte| *byte != 0) {
        return Err(PublicNetworkError::Rpc(
            "ABI address output must be exactly 32 canonical bytes".into(),
        ));
    }
    let mut address = [0_u8; 20];
    address.copy_from_slice(&output[12..]);
    Ok(Address20::from_bytes(address))
}

fn decode_abi_bool(output: &[u8]) -> Result<bool, PublicNetworkError> {
    if output.len() != 32 || output[..31].iter().any(|byte| *byte != 0) || output[31] > 1 {
        return Err(PublicNetworkError::Rpc(
            "ABI bool output was not an exact 32-byte zero or one".into(),
        ));
    }
    Ok(output[31] == 1)
}

fn decode_abi_bytes32(output: &[u8]) -> Result<Bytes32, PublicNetworkError> {
    if output.len() != 32 {
        return Err(PublicNetworkError::Rpc(
            "ABI bytes32 output must be exactly 32 bytes".into(),
        ));
    }
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(output);
    Ok(Bytes32::new(bytes))
}

fn decode_abi_u64(output: &[u8]) -> Result<u64, PublicNetworkError> {
    if output.len() != 32 || output[..24].iter().any(|byte| *byte != 0) {
        return Err(PublicNetworkError::Rpc(
            "ABI uint64 output must be exactly 32 canonical bytes".into(),
        ));
    }
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&output[24..]);
    Ok(u64::from_be_bytes(bytes))
}

fn decode_abi_u64_address(output: &[u8]) -> Result<(u64, Address20), PublicNetworkError> {
    if output.len() != 64 {
        return Err(PublicNetworkError::Rpc(
            "ABI uint256/address tuple output was not exactly 64 bytes".into(),
        ));
    }
    if output[..24].iter().any(|byte| *byte != 0) || output[32..44].iter().any(|byte| *byte != 0) {
        return Err(PublicNetworkError::Rpc(
            "ABI appcoin tuple exceeded supported uint64/address widths".into(),
        ));
    }
    let mut chain = [0_u8; 8];
    chain.copy_from_slice(&output[24..32]);
    let mut address = [0_u8; 20];
    address.copy_from_slice(&output[44..64]);
    Ok((u64::from_be_bytes(chain), Address20::from_bytes(address)))
}

fn rpc_quantity<T: ReadOnlyRpcTransport>(
    transport: &T,
    request: &ReadOnlyRpcRequest,
    evidence: &str,
) -> Result<u64, PublicNetworkError> {
    let value = rpc_string(transport, request, evidence)?;
    parse_quantity(&value)
}

fn rpc_data<T: ReadOnlyRpcTransport>(
    transport: &T,
    request: &ReadOnlyRpcRequest,
    evidence: &str,
    max_bytes: usize,
    allow_empty: bool,
) -> Result<Vec<u8>, PublicNetworkError> {
    let value = rpc_string(transport, request, evidence)?;
    decode_data_hex(&value, max_bytes, allow_empty)
}

fn rpc_call<T: ReadOnlyRpcTransport>(
    transport: &T,
    to: Address20,
    data: Vec<u8>,
    id: u64,
    evidence: &str,
) -> Result<Vec<u8>, PublicNetworkError> {
    rpc_call_at(transport, to, data, id, evidence, None)
}

fn rpc_call_at<T: ReadOnlyRpcTransport>(
    transport: &T,
    to: Address20,
    data: Vec<u8>,
    id: u64,
    evidence: &str,
    block_number: Option<u64>,
) -> Result<Vec<u8>, PublicNetworkError> {
    rpc_data(
        transport,
        &ReadOnlyRpcRequest::Call {
            id,
            to,
            data,
            block_number,
        },
        evidence,
        MAX_RPC_RESPONSE_BYTES / 2,
        true,
    )
}

fn rpc_string<T: ReadOnlyRpcTransport>(
    transport: &T,
    request: &ReadOnlyRpcRequest,
    evidence: &str,
) -> Result<String, PublicNetworkError> {
    let bytes = transport
        .execute(request)
        .map_err(|error| PublicNetworkError::Transport {
            evidence: evidence.into(),
            reason: error.to_string(),
        })?;
    if bytes.len() > MAX_RPC_RESPONSE_BYTES {
        return Err(PublicNetworkError::Rpc(format!(
            "{evidence}: response exceeded {MAX_RPC_RESPONSE_BYTES} bytes"
        )));
    }
    let unique = serde_json::from_slice::<UniqueJson>(&bytes)
        .map_err(|error| PublicNetworkError::Rpc(format!("{evidence}: malformed JSON: {error}")))?;
    let object = unique.0.as_object().ok_or_else(|| {
        PublicNetworkError::Rpc(format!("{evidence}: response must be a JSON object"))
    })?;
    if object.contains_key("result") == object.contains_key("error") {
        return Err(PublicNetworkError::Rpc(format!(
            "{evidence}: response must contain exactly one of result or error"
        )));
    }
    let response = serde_json::from_value::<RpcResponse>(unique.0)
        .map_err(|error| PublicNetworkError::Rpc(format!("{evidence}: schema error: {error}")))?;
    if response.jsonrpc != "2.0" || response.id != request.id() {
        return Err(PublicNetworkError::Rpc(format!(
            "{evidence}: JSON-RPC version or response id mismatch"
        )));
    }
    match (response.result, response.error) {
        (Some(result), None) => Ok(result),
        (None, Some(error)) => Err(PublicNetworkError::Rpc(format!(
            "{evidence}: RPC error code {}",
            error.code
        ))),
        _ => Err(PublicNetworkError::Rpc(format!(
            "{evidence}: response must contain exactly one of result or error"
        ))),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcResponse {
    jsonrpc: String,
    id: u64,
    result: Option<String>,
    error: Option<RpcErrorObject>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcErrorObject {
    code: i64,
    #[allow(dead_code)]
    message: String,
    #[allow(dead_code)]
    data: Option<Value>,
}

fn parse_quantity(value: &str) -> Result<u64, PublicNetworkError> {
    let digits = value
        .strip_prefix("0x")
        .ok_or_else(|| PublicNetworkError::Rpc("quantity must begin with 0x".into()))?;
    if digits.is_empty()
        || (digits.len() > 1 && digits.starts_with('0'))
        || !digits
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(PublicNetworkError::Rpc(
            "quantity is not canonical lowercase hexadecimal".into(),
        ));
    }
    u64::from_str_radix(digits, 16)
        .map_err(|_| PublicNetworkError::Rpc("quantity exceeds u64".into()))
}

fn decode_data_hex(
    value: &str,
    max_bytes: usize,
    allow_empty: bool,
) -> Result<Vec<u8>, PublicNetworkError> {
    let digits = value
        .strip_prefix("0x")
        .ok_or_else(|| PublicNetworkError::Rpc("data must begin with 0x".into()))?;
    if digits.len() % 2 != 0
        || (!allow_empty && digits.is_empty())
        || digits.len() / 2 > max_bytes
        || !digits
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(PublicNetworkError::Rpc(
            "data is malformed, non-canonical, or exceeds its limit".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(digits.len() / 2);
    for pair in digits.as_bytes().chunks_exact(2) {
        let high = hex_nibble(pair[0])
            .ok_or_else(|| PublicNetworkError::Rpc("data contains invalid hexadecimal".into()))?;
        let low = hex_nibble(pair[1])
            .ok_or_else(|| PublicNetworkError::Rpc("data contains invalid hexadecimal".into()))?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(2 + bytes.len() * 2);
    output.push_str("0x");
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn address(value: &str) -> Result<Address20, PublicNetworkError> {
    serde_json::from_str::<Address20>(&format!("\"{value}\""))
        .map_err(|_| PublicNetworkError::DeploymentPlan(format!("invalid pinned address {value}")))
}

fn bytes32(value: &str) -> Result<Bytes32, PublicNetworkError> {
    Bytes32::from_hex("deployment", value)
        .map_err(|_| PublicNetworkError::DeploymentPlan("invalid pinned bytes32".into()))
}

fn result_check<T, E>(
    id: &str,
    expected: &str,
    result: &Result<T, E>,
    evidence: &str,
) -> PublicCheck
where
    T: fmt::Display,
    E: fmt::Display,
{
    match result {
        Ok(observed) => passed(id, expected, &observed.to_string(), evidence),
        Err(observed) => failed(id, expected, &observed.to_string(), evidence),
    }
}

fn passed(id: &str, expected: &str, observed: &str, evidence: &str) -> PublicCheck {
    public_check(id, PublicCheckStatus::Pass, expected, observed, evidence)
}

fn failed(id: &str, expected: &str, observed: &str, evidence: &str) -> PublicCheck {
    public_check(id, PublicCheckStatus::Fail, expected, observed, evidence)
}

fn not_checked(id: &str, expected: &str, observed: &str, evidence: &str) -> PublicCheck {
    public_check(
        id,
        PublicCheckStatus::NotChecked,
        expected,
        observed,
        evidence,
    )
}

fn public_check(
    id: &str,
    status: PublicCheckStatus,
    expected: &str,
    observed: &str,
    evidence: &str,
) -> PublicCheck {
    PublicCheck {
        id: bounded(id),
        status,
        expected: bounded(expected),
        observed: bounded(observed),
        evidence: bounded(evidence),
    }
}

fn bounded(value: &str) -> String {
    let mut output = value.chars().take(REPORT_TEXT_LIMIT).collect::<String>();
    if output.len() < value.len() {
        output.push('…');
    }
    output
}

#[derive(Debug)]
pub enum PublicNetworkError {
    DeploymentPlan(String),
    InvalidRpcUrl,
    CurlUnavailable,
    Transport { evidence: String, reason: String },
    Rpc(String),
}

impl fmt::Display for PublicNetworkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeploymentPlan(reason) => write!(formatter, "deployment plan: {reason}"),
            Self::InvalidRpcUrl => formatter.write_str(
                "RPC URL must be a bounded explicit http:// or https:// URL without credentials",
            ),
            Self::CurlUnavailable => {
                formatter.write_str("no fixed supported curl executable was found")
            }
            Self::Transport { evidence, reason } => {
                write!(formatter, "{evidence}: {reason}")
            }
            Self::Rpc(reason) => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for PublicNetworkError {}

struct UniqueJson(Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

struct UniqueJsonVisitor;

impl<'de> Visitor<'de> for UniqueJsonVisitor {
    type Value = UniqueJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(Number::from(value))))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(Number::from(value))))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Number::from_f64(value)
            .map(Value::Number)
            .map(UniqueJson)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::String(value.into())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        UniqueJson::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(1024));
        while let Some(value) = sequence.next_element::<UniqueJson>()? {
            values.push(value.0);
        }
        Ok(UniqueJson(Value::Array(values)))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(<A::Error as serde::de::Error>::custom(format!(
                    "duplicate object key {key:?}"
                )));
            }
            let value = object.next_value::<UniqueJson>()?;
            values.insert(key, value.0);
        }
        Ok(UniqueJson(Value::Object(values)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tohseno_protocol::digest::ShotId;
    use tohseno_protocol::identity::BuilderId;
    use tohseno_protocol::record::{
        CanonicalTimestamp, FactoryDescriptor, APPLE_FASCIA_ID, PROTOCOL_NAME, SHOT_SCHEMA,
    };

    #[derive(Clone)]
    struct FakeTransport {
        chain: String,
        codes: BTreeMap<Address20, Vec<u8>>,
        calls: BTreeMap<(Address20, Vec<u8>), Vec<u8>>,
        raw_response: Option<Vec<u8>>,
        transport_error: Option<RpcTransportError>,
    }

    impl ReadOnlyRpcTransport for FakeTransport {
        fn execute(&self, request: &ReadOnlyRpcRequest) -> Result<Vec<u8>, RpcTransportError> {
            if let Some(error) = &self.transport_error {
                return Err(error.clone());
            }
            if let Some(response) = &self.raw_response {
                return Ok(response.clone());
            }
            let result = match request {
                ReadOnlyRpcRequest::ChainId { .. } => self.chain.clone(),
                ReadOnlyRpcRequest::BlockNumber { .. } => "0x10".into(),
                ReadOnlyRpcRequest::GetCode { address, .. } => self
                    .codes
                    .get(address)
                    .map(|value| encode_hex(value))
                    .unwrap_or_else(|| "0x".into()),
                ReadOnlyRpcRequest::Call { to, data, .. } => self
                    .calls
                    .get(&(*to, data.clone()))
                    .map(|value| encode_hex(value))
                    .unwrap_or_else(|| "0x".into()),
                ReadOnlyRpcRequest::GetTransactionByHash { .. } => {
                    return Err(RpcTransportError::RequestEncoding);
                }
            };
            Ok(serde_json::to_vec(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.id(),
                "result": result
            }))
            .unwrap())
        }
    }

    fn record() -> ShotRecord {
        ShotRecord {
            protocol: PROTOCOL_NAME.into(),
            schema: SHOT_SCHEMA.into(),
            shot_id: ShotId::from_bytes([0x11; 32]),
            slug: "public-fixture".into(),
            builder_id: BuilderId::new(Address20::from_bytes([0x22; 20])),
            sequence: 7,
            previous: Some(Bytes32::new([0x77; 32])),
            fascia: APPLE_FASCIA_ID.into(),
            bundle_id: "com.example.public-fixture".into(),
            bundle_version: 7,
            genesis_input_sha256: Bytes32::new([0x33; 32]),
            source_tree_sha256: Bytes32::new([0x44; 32]),
            fascia_sha256: Bytes32::new([0x55; 32]),
            factory: FactoryDescriptor {
                implementation: "test/factory".into(),
                version: "1.0.0-test".into(),
                source_commit: "a".repeat(40),
            },
            created_at: CanonicalTimestamp::parse("2026-07-28T00:00:00Z").unwrap(),
            origin: None,
        }
    }

    fn code_expectation(code: &[u8], exact: bool) -> CodeExpectation {
        CodeExpectation {
            digest: Bytes32::new(Keccak256::digest(code).into()),
            size: code.len(),
            exact: exact.then(|| code.to_vec()),
        }
    }

    fn fixture() -> (DeploymentPlan, ShotRecord, FakeTransport, CodeExpectations) {
        let plan = embedded_deployment_plan().unwrap();
        let record = record();
        let deployer_code = b"fake-deployer-runtime".to_vec();
        let factory_code = b"fake-factory-runtime".to_vec();
        let registry_code = b"fake-registry-runtime".to_vec();
        let relations_code = b"fake-relations-runtime".to_vec();
        let expectations = CodeExpectations {
            deployer: code_expectation(&deployer_code, true),
            factory: code_expectation(&factory_code, false),
            registry: code_expectation(&registry_code, false),
            relations: code_expectation(&relations_code, false),
        };
        let mut codes = BTreeMap::new();
        codes.insert(plan.create2.deployer, deployer_code);
        codes.insert(
            plan.contracts.builder_account_factory.planned_address,
            factory_code,
        );
        codes.insert(plan.contracts.shot_registry.planned_address, registry_code);
        codes.insert(
            plan.contracts.shot_relations.planned_address,
            relations_code,
        );
        let mut calls = BTreeMap::new();
        calls.insert(
            (
                plan.chain.p256verify,
                decode_data_hex(P256_KNOWN_INPUT, 160, false).unwrap(),
            ),
            decode_data_hex(P256_KNOWN_OUTPUT, 32, false).unwrap(),
        );
        calls.insert(
            (
                plan.contracts.shot_relations.planned_address,
                abi_no_args("registry()"),
            ),
            abi_address(plan.contracts.shot_registry.planned_address),
        );
        let shot_id = *record.shot_id.bytes().as_bytes();
        calls.insert(
            (
                plan.contracts.shot_registry.planned_address,
                abi_bytes32("controllerOf(bytes32)", shot_id),
            ),
            abi_address(record.builder_id.account()),
        );
        calls.insert(
            (
                plan.contracts.shot_registry.planned_address,
                abi_bytes32("headOf(bytes32)", shot_id),
            ),
            record.commitment().unwrap().as_bytes().to_vec(),
        );
        calls.insert(
            (
                plan.contracts.shot_registry.planned_address,
                abi_bytes32("sequenceOf(bytes32)", shot_id),
            ),
            abi_u64(u64::from(record.sequence)),
        );
        (
            plan,
            record,
            FakeTransport {
                chain: "0x1237".into(),
                codes,
                calls,
                raw_response: None,
                transport_error: None,
            },
            expectations,
        )
    }

    fn abi_address(address: Address20) -> Vec<u8> {
        let mut output = vec![0_u8; 12];
        output.extend_from_slice(address.as_bytes());
        output
    }

    fn abi_u64(value: u64) -> Vec<u8> {
        let mut output = vec![0_u8; 24];
        output.extend_from_slice(&value.to_be_bytes());
        output
    }

    fn abi_u64_address(value: u64, address: Address20) -> Vec<u8> {
        let mut output = abi_u64(value);
        output.extend_from_slice(&abi_address(address));
        output
    }

    fn network_check<'a>(report: &'a NetworkStatusReport, id: &str) -> &'a PublicCheck {
        report.checks.iter().find(|check| check.id == id).unwrap()
    }

    fn public_check_by_id<'a>(
        report: &'a PublicShotVerificationReport,
        id: &str,
    ) -> &'a PublicCheck {
        report.checks.iter().find(|check| check.id == id).unwrap()
    }

    #[test]
    fn embedded_plan_is_strict_and_reproduces_all_create2_addresses() {
        let plan = embedded_deployment_plan().unwrap();
        assert_eq!(
            create2_address(
                plan.create2.deployer,
                plan.contracts.builder_account_factory.salt,
                plan.contracts.builder_account_factory.init_code_hash,
            ),
            plan.contracts.builder_account_factory.planned_address
        );
        assert_eq!(
            create2_address(
                plan.create2.deployer,
                plan.contracts.shot_registry.salt,
                plan.contracts.shot_registry.init_code_hash,
            ),
            plan.contracts.shot_registry.planned_address
        );
        assert_eq!(
            create2_address(
                plan.create2.deployer,
                plan.contracts.shot_relations.salt,
                plan.contracts.shot_relations.init_code_hash,
            ),
            plan.contracts.shot_relations.planned_address
        );

        let mut value: Value = serde_json::from_str(DEPLOYMENT_PLAN_JSON).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), Value::Bool(true));
        assert!(DeploymentPlan::parse(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(DeploymentPlan::parse(br#"{"schema":"a","schema":"b"}"#).is_err());
        assert!(DeploymentPlan::parse(&vec![b' '; MAX_DEPLOYMENT_PLAN_BYTES + 1]).is_err());
    }

    #[test]
    fn runtime_and_probe_constants_remain_tied_to_checked_in_evidence() {
        let deployment_script = include_str!("../../scripts/deploy-candidate.sh");
        for value in [
            EXPECTED_DEPLOYER_HASH,
            EXPECTED_FACTORY_RUNTIME_HASH,
            EXPECTED_REGISTRY_RUNTIME_HASH,
            EXPECTED_RELATIONS_RUNTIME_HASH,
            &format!("factory_runtime_size={EXPECTED_FACTORY_RUNTIME_SIZE}"),
            &format!("registry_runtime_size={EXPECTED_REGISTRY_RUNTIME_SIZE}"),
            &format!("relations_runtime_size={EXPECTED_RELATIONS_RUNTIME_SIZE}"),
        ] {
            assert!(
                deployment_script.contains(value),
                "deployment script lost pinned value {value}"
            );
        }
        let probe: Value =
            serde_json::from_str(include_str!("../../genesis/lifecycle/P256_PROBE.json")).unwrap();
        assert_eq!(probe["input"], P256_KNOWN_INPUT);
        assert_eq!(probe["output"], P256_KNOWN_OUTPUT);
        assert_eq!(probe["chain_id"], ROBINHOOD_CHAIN_ID);
    }

    #[test]
    fn rpc_url_accepts_only_explicit_bounded_http_urls_without_credentials() {
        assert!(RpcUrl::parse("https://rpc.mainnet.chain.robinhood.com").is_ok());
        assert!(RpcUrl::parse("http://127.0.0.1:8545").is_ok());
        assert!(RpcUrl::parse("ftp://example.com").is_err());
        assert!(RpcUrl::parse("https://user:secret@example.com").is_err());
        assert!(RpcUrl::parse("https://example.com/#fragment").is_err());
        assert!(RpcUrl::parse("https://example.com\\evil").is_err());
        assert!(
            RpcUrl::parse(format!("https://example.com/{}", "a".repeat(RPC_URL_LIMIT))).is_err()
        );
    }

    #[test]
    fn abi_encoding_and_decoding_are_canonical() {
        assert_eq!(encode_hex(&abi_no_args("registry()")), "0x7b103999");
        let argument = [0x11; 32];
        assert_eq!(
            &encode_hex(&abi_bytes32("controllerOf(bytes32)", argument))[..10],
            "0xd2b4678b"
        );
        assert_eq!(
            &encode_hex(&abi_bytes32("headOf(bytes32)", argument))[..10],
            "0x65c7d79d"
        );
        assert_eq!(
            &encode_hex(&abi_bytes32("sequenceOf(bytes32)", argument))[..10],
            "0xc6bf5995"
        );
        let address = Address20::from_bytes([0x22; 20]);
        assert_eq!(decode_abi_address(&abi_address(address)).unwrap(), address);
        assert_eq!(decode_abi_u64(&abi_u64(7)).unwrap(), 7);
        assert!(decode_abi_address(&[1; 32]).is_err());
        assert!(decode_abi_u64(&[1; 31]).is_err());
        assert!(decode_abi_bytes32(&[0; 33]).is_err());
        let request = ReadOnlyRpcRequest::Call {
            id: 9,
            to: address,
            data: vec![1, 2, 3, 4],
            block_number: Some(16),
        };
        let json: Value = serde_json::from_slice(&request.json().unwrap()).unwrap();
        assert_eq!(json["params"][1], "0x10");
        let transaction_hash = Bytes32::new([0x33; 32]);
        let transaction_request = ReadOnlyRpcRequest::GetTransactionByHash {
            id: 10,
            transaction_hash,
        };
        let json: Value = serde_json::from_slice(&transaction_request.json().unwrap()).unwrap();
        assert_eq!(json["method"], "eth_getTransactionByHash");
        assert_eq!(json["params"][0], transaction_hash.to_string());
    }

    #[test]
    fn preparation_read_is_block_pinned_and_closes_every_required_nonce() {
        let (plan, record, mut transport, expectations) = fixture();
        let account = record.builder_id.account();
        let signer_key_id = Bytes32::new([0x66; 32]);
        let builder_code = b"fake-builder-account-runtime".to_vec();
        let builder_expectation = code_expectation(&builder_code, false);
        transport.codes.insert(account, builder_code);
        transport.calls.insert(
            (
                account,
                abi_bytes32_u32("hasPermission(bytes32,uint32)", signer_key_id, 1),
            ),
            abi_u64(1),
        );
        let shot = *record.shot_id.bytes().as_bytes();
        let registry = plan.contracts.shot_registry.planned_address;
        transport.calls.insert(
            (registry, abi_bytes32("nonceOf(bytes32)", shot)),
            abi_u64(9),
        );
        transport.calls.insert(
            (
                registry,
                abi_address_argument("createNonces(address)", account),
            ),
            abi_u64(2),
        );
        let relations = plan.contracts.shot_relations.planned_address;
        let requested_handle = Bytes32::new([0x88; 32]);
        transport.calls.insert(
            (relations, abi_bytes32("nonces(bytes32)", shot)),
            abi_u64(4),
        );
        transport.calls.insert(
            (relations, abi_bytes32("handleByShot(bytes32)", shot)),
            Bytes32::ZERO.as_bytes().to_vec(),
        );
        transport.calls.insert(
            (
                relations,
                abi_bytes32("shotByHandle(bytes32)", *requested_handle.as_bytes()),
            ),
            Bytes32::ZERO.as_bytes().to_vec(),
        );
        transport.calls.insert(
            (relations, abi_bytes32("appcoinOf(bytes32)", shot)),
            abi_u64_address(0, Address20::from_bytes([0; 20])),
        );
        let network =
            network_status_with_expectations_at(&transport, &plan, &expectations, Some(16));
        assert!(network.ready);
        let read = read_public_preparation_with_builder_expectation(
            &transport,
            &plan,
            &record,
            account,
            signer_key_id,
            RelationRead::Handle(requested_handle),
            &builder_expectation,
            16,
            network,
        );
        assert!(read.read_complete, "{:?}", read.error);
        assert_eq!(read.block_number, Some(16));
        let builder = read.builder_account.unwrap();
        assert_eq!(builder.code_state, BuilderAccountCodeState::Exact);
        assert_eq!(builder.queried_key_id, signer_key_id);
        assert_eq!(builder.protocol_permission, Some(true));
        let registry = read.registry.unwrap();
        assert_eq!(registry.shot_nonce, 9);
        assert_eq!(registry.create_nonce, 2);
        let relations = read.relations.unwrap();
        assert_eq!(relations.nonce, 4);
        assert_eq!(relations.shot_by_requested_handle, Some(Bytes32::ZERO));

        transport.codes.insert(account, b"wrong-builder".to_vec());
        let network =
            network_status_with_expectations_at(&transport, &plan, &expectations, Some(17));
        let wrong = read_public_preparation_with_builder_expectation(
            &transport,
            &plan,
            &record,
            account,
            signer_key_id,
            RelationRead::None,
            &builder_expectation,
            17,
            network,
        );
        assert!(!wrong.read_complete);
        assert!(wrong
            .error
            .as_deref()
            .is_some_and(|error| error.contains("not the pinned runtime")));
    }

    #[test]
    fn network_status_checks_chain_code_p256_and_relations_binding() {
        let (plan, _, transport, expectations) = fixture();
        let report = network_status_with_expectations(&transport, &plan, &expectations);
        assert!(
            report.ready,
            "{}",
            serde_json::to_string_pretty(&report).unwrap()
        );
    }

    #[test]
    fn wrong_chain_refuses_every_subsequent_network_judgment() {
        let (plan, _, mut transport, expectations) = fixture();
        transport.chain = "0x1".into();
        let report = network_status_with_expectations(&transport, &plan, &expectations);
        assert!(!report.ready);
        assert_eq!(
            network_check(&report, "network.chain_id").status,
            PublicCheckStatus::Fail
        );
        assert_eq!(
            network_check(&report, "network.code.shot_registry").status,
            PublicCheckStatus::NotChecked
        );
    }

    #[test]
    fn absent_and_wrong_contract_code_are_honest_failures() {
        let (plan, _, mut transport, expectations) = fixture();
        transport
            .codes
            .remove(&plan.contracts.shot_registry.planned_address);
        let absent = network_status_with_expectations(&transport, &plan, &expectations);
        let check = network_check(&absent, "network.code.shot_registry");
        assert_eq!(check.status, PublicCheckStatus::Fail);
        assert!(check.observed.contains("undeployed"));

        transport.codes.insert(
            plan.contracts.shot_registry.planned_address,
            b"wrong-runtime".to_vec(),
        );
        let wrong = network_status_with_expectations(&transport, &plan, &expectations);
        assert_eq!(
            network_check(&wrong, "network.code.shot_registry").status,
            PublicCheckStatus::Fail
        );
    }

    #[test]
    fn public_shot_matches_controller_head_sequence_and_relations() {
        let (plan, record, transport, expectations) = fixture();
        let report =
            verify_public_shot_with_expectations(&transport, &plan, &record, &expectations);
        assert!(
            report.verified,
            "{}",
            serde_json::to_string_pretty(&report).unwrap()
        );
        assert_eq!(
            report.observed.controller,
            Some(record.builder_id.account())
        );
        assert_eq!(report.observed.head, Some(record.commitment().unwrap()));
        assert_eq!(report.observed.sequence, Some(u64::from(record.sequence)));
        assert_eq!(
            report.observed.relations_registry,
            Some(plan.contracts.shot_registry.planned_address)
        );
    }

    #[test]
    fn wrong_controller_head_and_sequence_each_fail_independently() {
        let (plan, record, transport, expectations) = fixture();
        let shot_id = *record.shot_id.bytes().as_bytes();

        let mut wrong_controller = transport.clone();
        wrong_controller.calls.insert(
            (
                plan.contracts.shot_registry.planned_address,
                abi_bytes32("controllerOf(bytes32)", shot_id),
            ),
            abi_address(Address20::from_bytes([0x99; 20])),
        );
        let report =
            verify_public_shot_with_expectations(&wrong_controller, &plan, &record, &expectations);
        assert_eq!(
            public_check_by_id(&report, "public.controller").status,
            PublicCheckStatus::Fail
        );

        let mut wrong_head = transport.clone();
        wrong_head.calls.insert(
            (
                plan.contracts.shot_registry.planned_address,
                abi_bytes32("headOf(bytes32)", shot_id),
            ),
            vec![0x99; 32],
        );
        let report =
            verify_public_shot_with_expectations(&wrong_head, &plan, &record, &expectations);
        assert_eq!(
            public_check_by_id(&report, "public.head").status,
            PublicCheckStatus::Fail
        );

        let mut wrong_sequence = transport;
        wrong_sequence.calls.insert(
            (
                plan.contracts.shot_registry.planned_address,
                abi_bytes32("sequenceOf(bytes32)", shot_id),
            ),
            abi_u64(8),
        );
        let report =
            verify_public_shot_with_expectations(&wrong_sequence, &plan, &record, &expectations);
        assert_eq!(
            public_check_by_id(&report, "public.sequence").status,
            PublicCheckStatus::Fail
        );
    }

    #[test]
    fn undeployed_registry_defers_public_state_calls() {
        let (plan, record, mut transport, expectations) = fixture();
        transport
            .codes
            .remove(&plan.contracts.shot_registry.planned_address);
        let report =
            verify_public_shot_with_expectations(&transport, &plan, &record, &expectations);
        assert!(!report.verified);
        assert_eq!(
            public_check_by_id(&report, "public.controller").status,
            PublicCheckStatus::NotChecked
        );
        assert_eq!(
            public_check_by_id(&report, "public.head").status,
            PublicCheckStatus::NotChecked
        );
        assert_eq!(
            public_check_by_id(&report, "public.sequence").status,
            PublicCheckStatus::NotChecked
        );
    }

    #[test]
    fn invalid_local_record_defers_all_public_state_comparisons() {
        let (plan, mut record, transport, expectations) = fixture();
        record.bundle_version = record.sequence + 1;
        let report =
            verify_public_shot_with_expectations(&transport, &plan, &record, &expectations);
        assert!(!report.verified);
        assert_eq!(
            public_check_by_id(&report, "public.record").status,
            PublicCheckStatus::Fail
        );
        for id in [
            "public.controller",
            "public.head",
            "public.sequence",
            "public.relations_binding",
        ] {
            assert_eq!(
                public_check_by_id(&report, id).status,
                PublicCheckStatus::NotChecked
            );
        }
    }

    #[test]
    fn malformed_empty_and_non_one_p256_outputs_fail() {
        let (plan, _, transport, expectations) = fixture();
        let key = (
            plan.chain.p256verify,
            decode_data_hex(P256_KNOWN_INPUT, 160, false).unwrap(),
        );
        for output in [Vec::new(), vec![1], vec![0; 32]] {
            let mut hostile = transport.clone();
            hostile.calls.insert(key.clone(), output);
            let report = network_status_with_expectations(&hostile, &plan, &expectations);
            assert_eq!(
                network_check(&report, "network.p256verify").status,
                PublicCheckStatus::Fail
            );
        }
    }

    #[test]
    fn oversized_malformed_duplicate_and_mismatched_rpc_responses_fail_closed() {
        let (plan, _, transport, expectations) = fixture();
        let hostile = [
            vec![b'x'; MAX_RPC_RESPONSE_BYTES + 1],
            b"{".to_vec(),
            br#"{"jsonrpc":"2.0","id":1,"result":"0x1237","result":"0x1237"}"#.to_vec(),
            br#"{"jsonrpc":"2.0","id":99,"result":"0x1237"}"#.to_vec(),
            br#"{"jsonrpc":"2.0","id":1,"result":"0x01237"}"#.to_vec(),
            br#"{"jsonrpc":"2.0","id":1,"result":null,"error":{"code":-1,"message":"both"}}"#
                .to_vec(),
        ];
        for response in hostile {
            let mut fake = transport.clone();
            fake.raw_response = Some(response);
            let report = network_status_with_expectations(&fake, &plan, &expectations);
            assert!(!report.ready);
            assert_eq!(
                network_check(&report, "network.chain_id").status,
                PublicCheckStatus::Fail
            );
        }
    }

    #[test]
    fn transport_errors_do_not_leak_into_state_calls_or_panic() {
        let (plan, _, mut transport, expectations) = fixture();
        transport.transport_error = Some(RpcTransportError::Timeout);
        let report = network_status_with_expectations(&transport, &plan, &expectations);
        assert!(!report.ready);
        assert!(network_check(&report, "network.chain_id")
            .observed
            .contains("timed out"));
    }
}
