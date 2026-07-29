//! Explicit mutation boundary for relaying already-closed public artifacts.
//!
//! This module deliberately accepts no raw private key, mnemonic, password,
//! unlocked RPC account, or arbitrary command. It can select only a named
//! Foundry keystore or an attached Ledger/Trezor.

use crate::public_actions::{BuilderAccountDeploymentRequest, SignedPublicActionPackage};
use crate::public_network::{CurlTransport, ReadOnlyRpcRequest, ReadOnlyRpcTransport, RpcUrl};
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use tohseno_protocol::digest::{sha256, Address20, Bytes32};
use tohseno_protocol::identity::ROBINHOOD_CHAIN_ID;

pub const EXACT_MAINNET_CONFIRMATION: &str =
    "I UNDERSTAND THIS WILL BROADCAST TO ROBINHOOD CHAIN MAINNET 4663";
pub const EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION: &str =
    "I UNDERSTAND THIS WILL IRREVERSIBLY DEPLOY MY BUILDERACCOUNT TO ROBINHOOD CHAIN MAINNET 4663";
const MAX_CAST_OUTPUT_BYTES: usize = 1024 * 1024;
const TRANSACTION_LOOKUP_ID: u64 = 7_950_001;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelayerWallet {
    FoundryAccount(String),
    Ledger,
    Trezor,
}

#[derive(Clone, Debug)]
pub struct SubmissionConfig {
    pub rpc_url: RpcUrl,
    pub confirmation: String,
    pub wallet: RelayerWallet,
}

impl SubmissionConfig {
    pub fn validate(&self) -> Result<(), PublicSubmissionError> {
        if self.confirmation != EXACT_MAINNET_CONFIRMATION {
            return Err(PublicSubmissionError::Guard(
                "the exact experimental-mainnet confirmation is required".into(),
            ));
        }
        if let RelayerWallet::FoundryAccount(name) = &self.wallet {
            if name.is_empty()
                || name.len() > 128
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            {
                return Err(PublicSubmissionError::Guard(
                    "Foundry account must be a simple keystore filename".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmittedTransaction {
    pub transaction_hash: Bytes32,
    pub sender: Address20,
    pub target: Address20,
    pub block_hash: Bytes32,
    pub block_number: u64,
    pub status: u64,
}

pub fn submit_signed_action(
    config: &SubmissionConfig,
    package: &SignedPublicActionPackage,
) -> Result<SubmittedTransaction, PublicSubmissionError> {
    package
        .verify()
        .map_err(|error| PublicSubmissionError::Guard(error.to_string()))?;
    submit_prepared_calldata(config, package.target, &package.calldata)
}

pub fn submit_builder_account_deployment(
    config: &SubmissionConfig,
    request: &BuilderAccountDeploymentRequest,
    builder_account_deployment_confirmation: Option<&str>,
) -> Result<SubmittedTransaction, PublicSubmissionError> {
    validate_builder_account_deployment_confirmation(builder_account_deployment_confirmation)?;
    request
        .verify()
        .map_err(|error| PublicSubmissionError::Guard(error.to_string()))?;
    submit_prepared_calldata(config, request.target, &request.calldata)
}

fn validate_builder_account_deployment_confirmation(
    confirmation: Option<&str>,
) -> Result<(), PublicSubmissionError> {
    if confirmation != Some(EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION) {
        return Err(PublicSubmissionError::Guard(
            "the exact irreversible BuilderAccount deployment confirmation is required".into(),
        ));
    }
    Ok(())
}

fn submit_prepared_calldata(
    config: &SubmissionConfig,
    target: Address20,
    calldata: &str,
) -> Result<SubmittedTransaction, PublicSubmissionError> {
    config.validate()?;
    validate_calldata(calldata)?;
    let executable = discover_cast()?;
    let initial_executable = executable_snapshot(&executable)?;

    let mut command = Command::new(&executable);
    command
        .env_clear()
        .args([
            "send",
            &target.to_string(),
            calldata,
            "--rpc-url",
            config.rpc_url.as_str(),
            "--chain",
            &ROBINHOOD_CHAIN_ID.to_string(),
            "--confirmations",
            "1",
            "--json",
            "--color",
            "never",
        ])
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    #[cfg(unix)]
    command.current_dir("/");
    if let Some(home) = std::env::var_os("HOME") {
        command.env("HOME", home);
    }
    match &config.wallet {
        RelayerWallet::FoundryAccount(name) => {
            command.args(["--account", name]);
        }
        RelayerWallet::Ledger => {
            command.arg("--ledger");
        }
        RelayerWallet::Trezor => {
            command.arg("--trezor");
        }
    }

    let final_executable = executable_snapshot(&executable)?;
    if initial_executable != final_executable {
        return Err(PublicSubmissionError::CastUnavailable);
    }
    let mut child = command
        .spawn()
        .map_err(|error| PublicSubmissionError::Process(error.to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| PublicSubmissionError::Process("cast stdout was unavailable".into()))?;
    let reader = thread::spawn(move || read_bounded_and_drain(stdout, MAX_CAST_OUTPUT_BYTES));
    let status = child
        .wait()
        .map_err(|error| PublicSubmissionError::Process(error.to_string()))?;
    let output = reader
        .join()
        .map_err(|_| PublicSubmissionError::Process("cast output reader panicked".into()))??;
    if !status.success() {
        return Err(PublicSubmissionError::Process(format!(
            "cast exited with status {}",
            status.code().unwrap_or(-1)
        )));
    }
    let receipt = parse_receipt(&output, target)?;
    retain_post_broadcast_context(
        &receipt,
        verify_transaction_envelope(config, &receipt, calldata),
    )?;
    Ok(receipt)
}

fn retain_post_broadcast_context(
    receipt: &SubmittedTransaction,
    verification: Result<(), PublicSubmissionError>,
) -> Result<(), PublicSubmissionError> {
    verification.map_err(|error| {
        PublicSubmissionError::Receipt(format!(
            "transaction {} was reported in block {}, but post-broadcast verification failed: {}",
            receipt.transaction_hash,
            receipt.block_number,
            error_detail(error)
        ))
    })
}

fn error_detail(error: PublicSubmissionError) -> String {
    match error {
        PublicSubmissionError::Guard(message)
        | PublicSubmissionError::Process(message)
        | PublicSubmissionError::Receipt(message) => message,
        PublicSubmissionError::CastUnavailable => "fixed cast executable unavailable".into(),
        PublicSubmissionError::Io(error) => error.to_string(),
    }
}

fn validate_calldata(value: &str) -> Result<(), PublicSubmissionError> {
    let Some(hex) = value.strip_prefix("0x") else {
        return Err(PublicSubmissionError::Guard(
            "prepared calldata must have a lowercase 0x prefix".into(),
        ));
    };
    if hex.len() < 8
        || hex.len() % 2 != 0
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(PublicSubmissionError::Guard(
            "prepared calldata must be canonical lowercase hex".into(),
        ));
    }
    Ok(())
}

fn discover_cast() -> Result<PathBuf, PublicSubmissionError> {
    let mut candidates = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(PathBuf::from(home).join(".foundry/bin/cast"));
    }
    candidates.extend([
        PathBuf::from("/opt/homebrew/bin/cast"),
        PathBuf::from("/usr/local/bin/cast"),
    ]);
    candidates
        .into_iter()
        .find(|candidate| {
            fs::symlink_metadata(candidate)
                .map(|metadata| fixed_executable(&metadata))
                .unwrap_or(false)
        })
        .ok_or(PublicSubmissionError::CastUnavailable)
}

#[cfg(unix)]
fn fixed_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let owner = metadata.uid();
    metadata.is_file()
        && !metadata.file_type().is_symlink()
        && metadata.permissions().mode() & 0o111 != 0
        && metadata.permissions().mode() & 0o022 == 0
        && metadata.nlink() == 1
        && (owner == unsafe { libc::geteuid() } || owner == 0)
}

#[cfg(not(unix))]
fn fixed_executable(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && !metadata.file_type().is_symlink()
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.created().ok() == right.created().ok()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExecutableSnapshot {
    digest: Bytes32,
    length: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    owner: u32,
    #[cfg(unix)]
    group: u32,
    #[cfg(unix)]
    modified_seconds: i64,
    #[cfg(unix)]
    modified_nanoseconds: i64,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
}

fn executable_snapshot(path: &PathBuf) -> Result<ExecutableSnapshot, PublicSubmissionError> {
    let path_metadata = fs::symlink_metadata(path)?;
    if !fixed_executable(&path_metadata)
        || path_metadata.len() == 0
        || path_metadata.len() > 128 * 1024 * 1024
    {
        return Err(PublicSubmissionError::CastUnavailable);
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(path)?;
    let open_metadata = file.metadata()?;
    if !same_file(&path_metadata, &open_metadata) || !fixed_executable(&open_metadata) {
        return Err(PublicSubmissionError::CastUnavailable);
    }
    let mut bytes = Vec::with_capacity(open_metadata.len() as usize);
    file.read_to_end(&mut bytes)?;
    if bytes.len() as u64 != open_metadata.len() {
        return Err(PublicSubmissionError::CastUnavailable);
    }
    let final_metadata = fs::symlink_metadata(path)?;
    if !same_file(&open_metadata, &final_metadata) || !fixed_executable(&final_metadata) {
        return Err(PublicSubmissionError::CastUnavailable);
    }
    Ok(ExecutableSnapshot {
        digest: sha256(&bytes),
        length: open_metadata.len(),
        #[cfg(unix)]
        device: {
            use std::os::unix::fs::MetadataExt;
            open_metadata.dev()
        },
        #[cfg(unix)]
        inode: {
            use std::os::unix::fs::MetadataExt;
            open_metadata.ino()
        },
        #[cfg(unix)]
        mode: {
            use std::os::unix::fs::MetadataExt;
            open_metadata.mode()
        },
        #[cfg(unix)]
        owner: {
            use std::os::unix::fs::MetadataExt;
            open_metadata.uid()
        },
        #[cfg(unix)]
        group: {
            use std::os::unix::fs::MetadataExt;
            open_metadata.gid()
        },
        #[cfg(unix)]
        modified_seconds: {
            use std::os::unix::fs::MetadataExt;
            open_metadata.mtime()
        },
        #[cfg(unix)]
        modified_nanoseconds: {
            use std::os::unix::fs::MetadataExt;
            open_metadata.mtime_nsec()
        },
        #[cfg(unix)]
        changed_seconds: {
            use std::os::unix::fs::MetadataExt;
            open_metadata.ctime()
        },
        #[cfg(unix)]
        changed_nanoseconds: {
            use std::os::unix::fs::MetadataExt;
            open_metadata.ctime_nsec()
        },
    })
}

fn read_bounded_and_drain(
    mut input: impl Read,
    limit: usize,
) -> Result<Vec<u8>, PublicSubmissionError> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    let mut too_large = false;
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|error| PublicSubmissionError::Process(error.to_string()))?;
        if count == 0 {
            break;
        }
        if output.len().saturating_add(count) <= limit {
            output.extend_from_slice(&buffer[..count]);
        } else {
            too_large = true;
        }
    }
    if too_large {
        return Err(PublicSubmissionError::Process(
            "cast output exceeded 1 MiB".into(),
        ));
    }
    Ok(output)
}

fn parse_receipt(
    bytes: &[u8],
    expected_target: Address20,
) -> Result<SubmittedTransaction, PublicSubmissionError> {
    let value = serde_json::from_slice::<UniqueJson>(bytes)
        .map_err(|_| PublicSubmissionError::Receipt("cast did not return JSON".into()))?;
    let object = value
        .0
        .as_object()
        .ok_or_else(|| PublicSubmissionError::Receipt("receipt was not an object".into()))?;
    let transaction_hash = object
        .get("transactionHash")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| PublicSubmissionError::Receipt("receipt has no transaction hash".into()))
        .and_then(parse_bytes32)?;
    let receipt = (|| {
        let target = object
            .get("to")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| PublicSubmissionError::Receipt("receipt has no target".into()))
            .and_then(parse_address)?;
        let sender = object
            .get("from")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| PublicSubmissionError::Receipt("receipt has no sender".into()))
            .and_then(parse_address)?;
        let block_hash = object
            .get("blockHash")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| PublicSubmissionError::Receipt("receipt has no block hash".into()))
            .and_then(parse_bytes32)?;
        let block_number = object
            .get("blockNumber")
            .ok_or_else(|| PublicSubmissionError::Receipt("receipt has no block number".into()))
            .and_then(parse_quantity_value)?;
        let status = object
            .get("status")
            .ok_or_else(|| PublicSubmissionError::Receipt("receipt has no status".into()))
            .and_then(parse_quantity_value)?;
        Ok(SubmittedTransaction {
            transaction_hash,
            sender,
            target,
            block_hash,
            block_number,
            status,
        })
    })()
    .map_err(|error| {
        PublicSubmissionError::Receipt(format!(
            "transaction {transaction_hash} was returned by cast, but its receipt was invalid: {}",
            error_detail(error)
        ))
    })?;
    retain_post_broadcast_context(&receipt, validate_receipt(&receipt, expected_target))?;
    Ok(receipt)
}

fn validate_receipt(
    receipt: &SubmittedTransaction,
    expected_target: Address20,
) -> Result<(), PublicSubmissionError> {
    if receipt.transaction_hash == Bytes32::ZERO
        || receipt.sender.as_bytes().iter().all(|byte| *byte == 0)
        || receipt.target != expected_target
        || receipt.block_hash == Bytes32::ZERO
        || receipt.block_number == 0
        || receipt.status != 1
    {
        return Err(PublicSubmissionError::Receipt(
            "receipt hash, sender, target, block, or success status did not match the relayed transaction"
                .into(),
        ));
    }
    Ok(())
}

fn verify_transaction_envelope(
    config: &SubmissionConfig,
    receipt: &SubmittedTransaction,
    expected_calldata: &str,
) -> Result<(), PublicSubmissionError> {
    let transport = CurlTransport::discover(config.rpc_url.clone())
        .map_err(|error| PublicSubmissionError::Receipt(error.to_string()))?;
    let response = transport
        .execute(&ReadOnlyRpcRequest::GetTransactionByHash {
            id: TRANSACTION_LOOKUP_ID,
            transaction_hash: receipt.transaction_hash,
        })
        .map_err(|error| PublicSubmissionError::Receipt(error.to_string()))?;
    parse_transaction_envelope(&response, TRANSACTION_LOOKUP_ID, receipt, expected_calldata)
}

fn parse_transaction_envelope(
    bytes: &[u8],
    expected_id: u64,
    receipt: &SubmittedTransaction,
    expected_calldata: &str,
) -> Result<(), PublicSubmissionError> {
    let value = serde_json::from_slice::<UniqueJson>(bytes)
        .map_err(|_| PublicSubmissionError::Receipt("transaction lookup was not JSON".into()))?;
    let envelope = value.0.as_object().ok_or_else(|| {
        PublicSubmissionError::Receipt("transaction lookup was not an object".into())
    })?;
    if envelope.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || envelope.get("id").and_then(Value::as_u64) != Some(expected_id)
        || envelope.get("error").is_some_and(|error| !error.is_null())
    {
        return Err(PublicSubmissionError::Receipt(
            "transaction lookup envelope did not match the request".into(),
        ));
    }
    let transaction = envelope
        .get("result")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            PublicSubmissionError::Receipt("mined transaction was not returned by hash".into())
        })?;
    let hash = required_string(transaction, "hash").and_then(parse_bytes32)?;
    let target = required_string(transaction, "to").and_then(parse_address)?;
    let sender = required_string(transaction, "from").and_then(parse_address)?;
    let block_hash = required_string(transaction, "blockHash").and_then(parse_bytes32)?;
    let block_number = transaction
        .get("blockNumber")
        .ok_or_else(|| PublicSubmissionError::Receipt("transaction has no block number".into()))
        .and_then(parse_quantity_value)?;
    let chain_id = transaction
        .get("chainId")
        .ok_or_else(|| PublicSubmissionError::Receipt("transaction has no chain ID".into()))
        .and_then(parse_quantity_value)?;
    let value = transaction
        .get("value")
        .ok_or_else(|| PublicSubmissionError::Receipt("transaction has no value".into()))
        .and_then(parse_quantity_value)?;
    let input = required_string(transaction, "input")?;
    if hash != receipt.transaction_hash
        || target != receipt.target
        || sender != receipt.sender
        || block_hash != receipt.block_hash
        || block_number != receipt.block_number
        || chain_id != ROBINHOOD_CHAIN_ID
        || value != 0
        || input != expected_calldata
    {
        return Err(PublicSubmissionError::Receipt(
            "transaction hash, sender, chain, target, value, block, or calldata did not match the submitted artifact"
                .into(),
        ));
    }
    Ok(())
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, PublicSubmissionError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| PublicSubmissionError::Receipt(format!("transaction has no valid {field}")))
}

fn parse_address(value: &str) -> Result<Address20, PublicSubmissionError> {
    if value.len() != 42
        || !value.starts_with("0x")
        || !value[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(PublicSubmissionError::Receipt(
            "invalid receipt target".into(),
        ));
    }
    let canonical = value.to_ascii_lowercase();
    serde_json::from_str(&format!("\"{canonical}\""))
        .map_err(|_| PublicSubmissionError::Receipt("invalid receipt target".into()))
}

fn parse_bytes32(value: &str) -> Result<Bytes32, PublicSubmissionError> {
    serde_json::from_str(&format!("\"{value}\""))
        .map_err(|_| PublicSubmissionError::Receipt("invalid transaction hash".into()))
}

fn parse_quantity_value(value: &serde_json::Value) -> Result<u64, PublicSubmissionError> {
    if let Some(value) = value.as_u64() {
        return Ok(value);
    }
    let value = value
        .as_str()
        .ok_or_else(|| PublicSubmissionError::Receipt("invalid receipt quantity".into()))?;
    let hex = value
        .strip_prefix("0x")
        .ok_or_else(|| PublicSubmissionError::Receipt("receipt quantity is not hex".into()))?;
    if hex.is_empty()
        || (hex.len() > 1 && hex.starts_with('0'))
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(PublicSubmissionError::Receipt(
            "receipt quantity is not canonical".into(),
        ));
    }
    u64::from_str_radix(hex, 16)
        .map_err(|_| PublicSubmissionError::Receipt("receipt quantity is too large".into()))
}

struct UniqueJson(Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

struct UniqueJsonVisitor;

impl<'de> Visitor<'de> for UniqueJsonVisitor {
    type Value = UniqueJson;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
        D: serde::Deserializer<'de>,
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

#[derive(Debug)]
pub enum PublicSubmissionError {
    Guard(String),
    CastUnavailable,
    Process(String),
    Receipt(String),
    Io(io::Error),
}

impl std::fmt::Display for PublicSubmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Guard(message) => write!(formatter, "public submission refused: {message}"),
            Self::CastUnavailable => {
                formatter.write_str("public submission refused: fixed cast executable unavailable")
            }
            Self::Process(message) => write!(formatter, "public submission failed: {message}"),
            Self::Receipt(message) => {
                write!(formatter, "public receipt verification failed: {message}")
            }
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for PublicSubmissionError {}

impl From<io::Error> for PublicSubmissionError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submission_guard_accepts_only_exact_confirmation_and_named_wallet() {
        let rpc_url = RpcUrl::parse("https://rpc.mainnet.chain.robinhood.com").unwrap();
        let valid = SubmissionConfig {
            rpc_url: rpc_url.clone(),
            confirmation: EXACT_MAINNET_CONFIRMATION.into(),
            wallet: RelayerWallet::FoundryAccount("relay-mainnet".into()),
        };
        valid.validate().unwrap();
        for invalid in ["", "../key", "key name", "key/password"] {
            let config = SubmissionConfig {
                rpc_url: rpc_url.clone(),
                confirmation: EXACT_MAINNET_CONFIRMATION.into(),
                wallet: RelayerWallet::FoundryAccount(invalid.into()),
            };
            assert!(config.validate().is_err(), "{invalid}");
        }
        for invalid in [
            None,
            Some(EXACT_MAINNET_CONFIRMATION),
            Some(
                "I UNDERSTAND THIS WILL IRREVERSIBLY DEPLOY MY BUILDERACCOUNT TO ROBINHOOD CHAIN MAINNET 4663 ",
            ),
        ] {
            assert!(validate_builder_account_deployment_confirmation(invalid).is_err());
        }
        validate_builder_account_deployment_confirmation(Some(
            EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION,
        ))
        .unwrap();
    }

    #[test]
    fn receipt_requires_success_hash_and_block() {
        let target = Address20::from_bytes([2; 20]);
        let receipt = parse_receipt(
            br#"{"transactionHash":"0x0101010101010101010101010101010101010101010101010101010101010101","from":"0x0404040404040404040404040404040404040404","to":"0x0202020202020202020202020202020202020202","blockHash":"0x0303030303030303030303030303030303030303030303030303030303030303","blockNumber":"0x10","status":"0x1"}"#,
            target,
        )
        .unwrap();
        assert_eq!(receipt.block_hash, Bytes32::new([3; 32]));
        assert_eq!(receipt.block_number, 16);
        assert_eq!(receipt.status, 1);
        let reverted = parse_receipt(
            br#"{"transactionHash":"0x0101010101010101010101010101010101010101010101010101010101010101","from":"0x0404040404040404040404040404040404040404","to":"0x0202020202020202020202020202020202020202","blockHash":"0x0303030303030303030303030303030303030303030303030303030303030303","blockNumber":"0x10","status":"0x0"}"#,
            target,
        )
        .unwrap_err()
        .to_string();
        assert!(
            reverted.contains("0x0101010101010101010101010101010101010101010101010101010101010101")
        );
        assert!(reverted.contains("block 16"));
        let wrong_target = parse_receipt(
            br#"{"transactionHash":"0x0101010101010101010101010101010101010101010101010101010101010101","from":"0x0404040404040404040404040404040404040404","to":"0x0202020202020202020202020202020202020202","blockHash":"0x0303030303030303030303030303030303030303030303030303030303030303","blockNumber":"0x10","status":"0x1"}"#,
            Address20::from_bytes([9; 20]),
        )
        .unwrap_err()
        .to_string();
        assert!(wrong_target
            .contains("0x0101010101010101010101010101010101010101010101010101010101010101"));
        assert!(wrong_target.contains("block 16"));
        assert!(parse_receipt(
            br#"{"transactionHash":"0x0101010101010101010101010101010101010101010101010101010101010101","from":"0x0404040404040404040404040404040404040404","to":"0x0202020202020202020202020202020202020202","blockHash":"0x0303030303030303030303030303030303030303030303030303030303030303","blockNumber":"0x10","status":"0x1","status":"0x1"}"#,
            target,
        )
        .is_err());
        assert!(parse_receipt(
            br#"{"transactionHash":"0x0000000000000000000000000000000000000000000000000000000000000000","from":"0x0404040404040404040404040404040404040404","to":"0x0202020202020202020202020202020202020202","blockHash":"0x0303030303030303030303030303030303030303030303030303030303030303","blockNumber":"0x10","status":"0x1"}"#,
            target,
        )
        .is_err());
    }

    #[test]
    fn transaction_lookup_binds_hash_chain_target_value_block_and_calldata() {
        let transaction_hash = Bytes32::new([1; 32]);
        let target = Address20::from_bytes([2; 20]);
        let receipt = SubmittedTransaction {
            transaction_hash,
            sender: Address20::from_bytes([4; 20]),
            target,
            block_hash: Bytes32::new([3; 32]),
            block_number: 16,
            status: 1,
        };
        let calldata = "0x12345678";
        let valid = serde_json::json!({
            "jsonrpc": "2.0",
            "id": TRANSACTION_LOOKUP_ID,
            "result": {
                "hash": transaction_hash.to_string(),
                "from": Address20::from_bytes([4; 20]).to_string(),
                "to": target.to_string(),
                "blockHash": Bytes32::new([3; 32]).to_string(),
                "blockNumber": "0x10",
                "chainId": "0x1237",
                "value": "0x0",
                "input": calldata
            }
        });
        parse_transaction_envelope(
            &serde_json::to_vec(&valid).unwrap(),
            TRANSACTION_LOOKUP_ID,
            &receipt,
            calldata,
        )
        .unwrap();

        for (pointer, replacement) in [
            ("/result/hash", serde_json::json!(Bytes32::new([9; 32]))),
            (
                "/result/from",
                serde_json::json!(Address20::from_bytes([0; 20])),
            ),
            (
                "/result/to",
                serde_json::json!(Address20::from_bytes([9; 20])),
            ),
            (
                "/result/blockHash",
                serde_json::json!(Bytes32::new([9; 32])),
            ),
            ("/result/blockNumber", serde_json::json!("0x11")),
            ("/result/chainId", serde_json::json!("0x1")),
            ("/result/value", serde_json::json!("0x1")),
            ("/result/input", serde_json::json!("0x87654321")),
        ] {
            let mut changed = valid.clone();
            *changed.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                parse_transaction_envelope(
                    &serde_json::to_vec(&changed).unwrap(),
                    TRANSACTION_LOOKUP_ID,
                    &receipt,
                    calldata,
                )
                .is_err(),
                "{pointer}"
            );
        }
        assert!(parse_transaction_envelope(
            br#"{"jsonrpc":"2.0","id":7950001,"result":{"hash":"0x0101010101010101010101010101010101010101010101010101010101010101","hash":"0x0101010101010101010101010101010101010101010101010101010101010101"}}"#,
            TRANSACTION_LOOKUP_ID,
            &receipt,
            calldata,
        )
        .is_err());

        let error = retain_post_broadcast_context(
            &receipt,
            Err(PublicSubmissionError::Receipt(
                "transaction lookup failed".into(),
            )),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains(&transaction_hash.to_string()));
        assert!(error.contains("block 16"));
        assert!(error.contains("transaction lookup failed"));
    }
}
