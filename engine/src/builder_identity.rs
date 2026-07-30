//! Local BuilderID lifecycle and public descriptor storage.

use crate::apple_identity::{AppleDeviceIdentity, AppleIdentityBridge, AppleIdentityError};
use crate::ledger::Ledger;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tohseno_protocol::digest::{sha256, Address20, Bytes32};
use tohseno_protocol::identity::{
    initial_builder_account_salt, predict_builder_account, BuilderDeviceKey, BuilderId,
    RecoveryAuthority, ROBINHOOD_CHAIN_ID,
};
use tohseno_protocol::signature::P256PublicKey;
use tohseno_protocol::signature::{DetachedP256Signature, SignatureAlgorithm, SignatureSidecar};

const BUILDER_SCHEMA: &str = "tohseno.builder/1";
pub const CURRENT_BUILDER_ACCOUNT_GENERATION: &str = "0.8.0";
const LEGACY_BUILDER_ACCOUNT_GENERATION: &str = "0.7.0";
const KEY_TAG_DOMAIN: &[u8] = b"TOHSENO-LOCAL-KEY-TAG-V1\0";
const CURRENT_DEPLOYMENT_PLAN: &str =
    include_str!("../../contracts/deployments/robinhood-mainnet-v0.8.0.json");
const LEGACY_DEPLOYMENT_PLAN: &str =
    include_str!("../../contracts/deployments/robinhood-mainnet-genesis.json");
const CURRENT_BUILDER_ACCOUNT_CREATION_HEX: &str =
    include_str!("../../contracts/bytecode/BuilderAccount.v0.8.0.creation.hex");
const LEGACY_BUILDER_ACCOUNT_CREATION_HEX: &str =
    include_str!("../../contracts/bytecode/BuilderAccount.creation.hex");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BuilderAccountGeneration {
    V0_7_0,
    V0_8_0,
}

impl BuilderAccountGeneration {
    pub(crate) fn version(self) -> &'static str {
        match self {
            Self::V0_7_0 => LEGACY_BUILDER_ACCOUNT_GENERATION,
            Self::V0_8_0 => CURRENT_BUILDER_ACCOUNT_GENERATION,
        }
    }

    pub(crate) fn is_current(self) -> bool {
        self == Self::V0_8_0
    }

    fn deployment_plan(self) -> &'static str {
        match self {
            Self::V0_7_0 => LEGACY_DEPLOYMENT_PLAN,
            Self::V0_8_0 => CURRENT_DEPLOYMENT_PLAN,
        }
    }

    fn creation_hex(self) -> &'static str {
        match self {
            Self::V0_7_0 => LEGACY_BUILDER_ACCOUNT_CREATION_HEX,
            Self::V0_8_0 => CURRENT_BUILDER_ACCOUNT_CREATION_HEX,
        }
    }
}

const SUPPORTED_BUILDER_ACCOUNT_GENERATIONS: [BuilderAccountGeneration; 2] = [
    BuilderAccountGeneration::V0_7_0,
    BuilderAccountGeneration::V0_8_0,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderDeploymentStatus {
    Predicted,
    Deployed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderIdentity {
    pub schema: String,
    pub candidate_version: String,
    pub chain_id: u64,
    pub factory_address: Address20,
    pub account_salt: Bytes32,
    pub account_address: Address20,
    pub builder_id: BuilderId,
    pub device: BuilderDeviceKey,
    pub local_key_tag: String,
    pub key_backend: String,
    pub security_level: String,
    pub test_only: bool,
    pub deployment_status: BuilderDeploymentStatus,
    pub recovery: Option<RecoveryAuthority>,
    pub created_at_unix: u64,
}

impl BuilderIdentity {
    pub(crate) fn generation(&self) -> Result<BuilderAccountGeneration, BuilderIdentityError> {
        generation_for_version(&self.candidate_version)
    }

    pub fn is_current_generation(&self) -> Result<bool, BuilderIdentityError> {
        Ok(self.generation()?.is_current())
    }

    pub fn validate(&self) -> Result<(), BuilderIdentityError> {
        if self.schema != BUILDER_SCHEMA || self.chain_id != ROBINHOOD_CHAIN_ID {
            return Err(BuilderIdentityError::InvalidDescriptor(
                "schema, candidate version, or chain ID is wrong".into(),
            ));
        }
        let generation = self.generation()?;
        if self.builder_id.account() != self.account_address {
            return Err(BuilderIdentityError::InvalidDescriptor(
                "BuilderID and account address disagree".into(),
            ));
        }
        self.device
            .validate()
            .map_err(|error| BuilderIdentityError::Protocol(error.to_string()))?;
        if self.local_key_tag.is_empty()
            || self.key_backend.is_empty()
            || self.security_level.is_empty()
            || self.created_at_unix == 0
        {
            return Err(BuilderIdentityError::InvalidDescriptor(
                "local key metadata or timestamp is empty".into(),
            ));
        }
        if self.test_only != (self.key_backend == "software_test") {
            return Err(BuilderIdentityError::InvalidDescriptor(
                "test-only marker and key backend disagree".into(),
            ));
        }
        if let Some(recovery) = &self.recovery {
            recovery
                .validate()
                .map_err(|error| BuilderIdentityError::Protocol(error.to_string()))?;
        }

        let network = candidate_network(generation)?;
        if self.factory_address != network.factory_address {
            return Err(BuilderIdentityError::InvalidDescriptor(
                "factory address differs from the declared contract generation".into(),
            ));
        }
        let predicted = predict_builder_account(
            self.factory_address,
            self.account_salt,
            &self.device.public_key,
            &builder_account_creation_bytecode_for(generation)?,
        )
        .map_err(|error| BuilderIdentityError::Protocol(error.to_string()))?;
        if predicted != self.builder_id {
            return Err(BuilderIdentityError::InvalidDescriptor(
                "stored BuilderID does not reproduce from CREATE2 inputs".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct BuilderIdentityManager {
    root: PathBuf,
}

impl BuilderIdentityManager {
    pub fn for_ledger(ledger: &Ledger) -> Self {
        Self {
            root: ledger.machine_root().join("identity"),
        }
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn path(&self) -> PathBuf {
        self.root.join("builder.json")
    }

    pub fn load(&self) -> Result<BuilderIdentity, BuilderIdentityError> {
        let path = self.path();
        reject_symlink(&path)?;
        let identity = serde_json::from_slice::<BuilderIdentity>(&fs::read(&path)?)?;
        identity.validate()?;
        Ok(identity)
    }

    /// The helper bridge, preferring a previously earned Enclave-capable
    /// bundle: keys made by the earned bundle live in its keychain access
    /// group, which the bare helper cannot see.
    fn bridge(&self) -> Result<AppleIdentityBridge, AppleIdentityError> {
        if let Some(earned) = crate::enclave::earned_helper(&self.root) {
            return AppleIdentityBridge::at(earned);
        }
        AppleIdentityBridge::discover()
    }

    pub fn ensure(&self) -> Result<BuilderIdentity, BuilderIdentityError> {
        let bridge = self.bridge()?;
        match self.ensure_with_bridge(&bridge) {
            Err(BuilderIdentityError::Apple(AppleIdentityError::HelperFailure {
                ref code,
                ..
            })) if code == "secure_enclave_unavailable" => {
                // The bare helper cannot reach the Enclave; earn a
                // provisioned bundle with the builder's own Xcode session
                // and try once more.
                let earned =
                    crate::enclave::earn(bridge.executable(), &self.root).map_err(|error| {
                        BuilderIdentityError::InvalidConfiguration(format!(
                            "Secure Enclave access could not be earned: {error}"
                        ))
                    })?;
                let bridge = AppleIdentityBridge::at(earned)?;
                self.ensure_with_bridge(&bridge)
            }
            outcome => outcome,
        }
    }

    pub fn ensure_with_bridge(
        &self,
        bridge: &AppleIdentityBridge,
    ) -> Result<BuilderIdentity, BuilderIdentityError> {
        if self.path().exists() {
            let identity = self.load()?;
            let observed = bridge.public(&identity.local_key_tag)?;
            verify_helper_identity(&identity, &observed)?;
            return Ok(identity);
        }

        fs::create_dir_all(self.root.join("devices"))?;
        let tag = local_key_tag(&self.root)?;
        let software_test = match std::env::var("TOHSENO_IDENTITY_BACKEND") {
            Ok(value) if value == "software-test" => true,
            Ok(value) if value == "secure-enclave" => false,
            Ok(_) => {
                return Err(BuilderIdentityError::InvalidConfiguration(
                    "TOHSENO_IDENTITY_BACKEND must be secure-enclave or software-test".into(),
                ))
            }
            Err(std::env::VarError::NotPresent) => false,
            Err(error) => {
                return Err(BuilderIdentityError::InvalidConfiguration(
                    error.to_string(),
                ))
            }
        };
        let helper_identity = match bridge.public(&tag) {
            Ok(identity) => identity,
            Err(AppleIdentityError::HelperFailure { code, .. }) if code == "identity_not_found" => {
                bridge.create(&tag, software_test)?
            }
            Err(error) => return Err(error.into()),
        };
        if helper_identity.test_only && !software_test {
            return Err(BuilderIdentityError::InvalidDescriptor(
                "a software test key cannot become production authority".into(),
            ));
        }
        helper_identity
            .public_key
            .validate()
            .map_err(|error| BuilderIdentityError::Protocol(error.to_string()))?;
        let device = BuilderDeviceKey::from_public_key(helper_identity.public_key.clone())
            .map_err(|error| BuilderIdentityError::Protocol(error.to_string()))?;
        let account_salt = initial_builder_account_salt(&device.public_key)
            .map_err(|error| BuilderIdentityError::Protocol(error.to_string()))?;
        let generation = BuilderAccountGeneration::V0_8_0;
        let network = candidate_network(generation)?;
        let builder_id = predict_builder_account(
            network.factory_address,
            account_salt,
            &device.public_key,
            &builder_account_creation_bytecode_for(generation)?,
        )
        .map_err(|error| BuilderIdentityError::Protocol(error.to_string()))?;
        let identity = BuilderIdentity {
            schema: BUILDER_SCHEMA.into(),
            candidate_version: generation.version().into(),
            chain_id: ROBINHOOD_CHAIN_ID,
            factory_address: network.factory_address,
            account_salt,
            account_address: builder_id.account(),
            builder_id,
            device,
            local_key_tag: tag,
            key_backend: helper_identity.backend,
            security_level: helper_identity.security_level,
            test_only: helper_identity.test_only,
            deployment_status: BuilderDeploymentStatus::Predicted,
            recovery: None,
            created_at_unix: now_unix(),
        };
        identity.validate()?;

        atomic_json(&self.path(), &identity)?;
        let device_path = self
            .root
            .join("devices")
            .join(format!("{}.json", identity.device.key_id));
        atomic_json(&device_path, &identity.device)?;
        Ok(identity)
    }

    pub fn save(&self, identity: &BuilderIdentity) -> Result<(), BuilderIdentityError> {
        identity.validate()?;
        atomic_json(&self.path(), identity)
    }

    pub fn sign_record_digest(
        &self,
        identity: &BuilderIdentity,
        digest: Bytes32,
    ) -> Result<SignatureSidecar, BuilderIdentityError> {
        let bridge = self.bridge()?;
        self.sign_record_digest_with_bridge(identity, digest, &bridge)
    }

    pub fn sign_record_digest_with_bridge(
        &self,
        identity: &BuilderIdentity,
        digest: Bytes32,
        bridge: &AppleIdentityBridge,
    ) -> Result<SignatureSidecar, BuilderIdentityError> {
        // Local, private Shot records may be signed by a visibly test-only
        // DeviceKey so a Mac without Secure Enclave access can still create,
        // verify, and evolve private Shots. Public actions never accept one.
        let detached = self.sign_digest_with_bridge_internal(identity, digest, bridge, true)?;
        Ok(SignatureSidecar {
            schema: SignatureSidecar::SCHEMA.into(),
            algorithm: SignatureAlgorithm::P256,
            digest,
            public_key: identity.device.public_key.clone(),
            signature: detached.signature,
            low_s: true,
        })
    }

    pub fn sign_digest(
        &self,
        identity: &BuilderIdentity,
        digest: Bytes32,
    ) -> Result<DetachedP256Signature, BuilderIdentityError> {
        let bridge = self.bridge()?;
        self.sign_digest_with_bridge(identity, digest, &bridge)
    }

    pub fn sign_digest_with_bridge(
        &self,
        identity: &BuilderIdentity,
        digest: Bytes32,
        bridge: &AppleIdentityBridge,
    ) -> Result<DetachedP256Signature, BuilderIdentityError> {
        self.sign_digest_with_bridge_internal(identity, digest, bridge, false)
    }

    fn sign_digest_with_bridge_internal(
        &self,
        identity: &BuilderIdentity,
        digest: Bytes32,
        bridge: &AppleIdentityBridge,
        local_record_only: bool,
    ) -> Result<DetachedP256Signature, BuilderIdentityError> {
        identity.validate()?;
        if identity.test_only && !local_record_only {
            return Err(BuilderIdentityError::InvalidConfiguration(
                "software-test DeviceKeys cannot authorize public actions".into(),
            ));
        }
        let response = bridge.sign(&identity.local_key_tag, digest)?;
        verify_helper_identity(identity, &response.identity)?;
        if response.algorithm != "p256" || response.digest != digest || !response.low_s {
            return Err(BuilderIdentityError::InvalidDescriptor(
                "Apple helper returned the wrong algorithm, digest, or low-s marker".into(),
            ));
        }
        let detached = DetachedP256Signature {
            algorithm: SignatureAlgorithm::P256,
            digest,
            signature: response.signature,
            low_s: response.low_s,
        };
        detached
            .verify(&identity.device.public_key)
            .map_err(|error| BuilderIdentityError::Protocol(error.to_string()))?;
        Ok(detached)
    }
}

/// Reproduces the candidate BuilderID controlled by an initial DeviceKey.
///
/// This is intentionally pure and state-independent so an offline verifier can
/// distinguish a valid signature from an authorized signature. Device-key
/// rotation is not accepted by this candidate until a complete authorization
/// proof is carried with the Evolution.
#[cfg(test)]
pub(crate) fn initial_device_builder_id(
    public_key: &P256PublicKey,
) -> Result<BuilderId, BuilderIdentityError> {
    initial_device_builder_id_for(BuilderAccountGeneration::V0_8_0, public_key)
}

pub(crate) fn resolve_initial_device_builder_id(
    claimed: BuilderId,
    public_key: &P256PublicKey,
) -> Result<BuilderAccountGeneration, BuilderIdentityError> {
    let mut matched = None;
    for generation in SUPPORTED_BUILDER_ACCOUNT_GENERATIONS {
        if initial_device_builder_id_for(generation, public_key)? == claimed {
            if matched.is_some() {
                return Err(BuilderIdentityError::InvalidDescriptor(
                    "BuilderID ambiguously matches more than one supported contract generation"
                        .into(),
                ));
            }
            matched = Some(generation);
        }
    }
    matched.ok_or_else(|| {
        BuilderIdentityError::InvalidDescriptor(
            "BuilderID does not reproduce under any supported contract generation".into(),
        )
    })
}

fn initial_device_builder_id_for(
    generation: BuilderAccountGeneration,
    public_key: &P256PublicKey,
) -> Result<BuilderId, BuilderIdentityError> {
    let device = BuilderDeviceKey::from_public_key(public_key.clone())
        .map_err(|error| BuilderIdentityError::Protocol(error.to_string()))?;
    let network = candidate_network(generation)?;
    predict_builder_account(
        network.factory_address,
        initial_builder_account_salt(&device.public_key)
            .map_err(|error| BuilderIdentityError::Protocol(error.to_string()))?,
        public_key,
        &builder_account_creation_bytecode_for(generation)?,
    )
    .map_err(|error| BuilderIdentityError::Protocol(error.to_string()))
}

#[derive(Clone, Copy, Debug)]
struct CandidateNetwork {
    factory_address: Address20,
}

fn generation_for_version(version: &str) -> Result<BuilderAccountGeneration, BuilderIdentityError> {
    match version {
        LEGACY_BUILDER_ACCOUNT_GENERATION => Ok(BuilderAccountGeneration::V0_7_0),
        CURRENT_BUILDER_ACCOUNT_GENERATION => Ok(BuilderAccountGeneration::V0_8_0),
        _ => Err(BuilderIdentityError::InvalidDescriptor(format!(
            "unsupported BuilderAccount contract generation {version}"
        ))),
    }
}

fn candidate_network(
    generation: BuilderAccountGeneration,
) -> Result<CandidateNetwork, BuilderIdentityError> {
    let value: serde_json::Value = serde_json::from_str(generation.deployment_plan())?;
    if value.pointer("/schema").and_then(serde_json::Value::as_str)
        != Some("tohseno.deployment-plan/1")
        || value
            .pointer("/chain/chain_id")
            .and_then(serde_json::Value::as_u64)
            != Some(ROBINHOOD_CHAIN_ID)
        || value
            .pointer("/candidate/version")
            .and_then(serde_json::Value::as_str)
            != Some(generation.version())
    {
        return Err(BuilderIdentityError::InvalidDescriptor(
            "embedded deployment plan has the wrong schema, generation, or chain".into(),
        ));
    }
    let factory = value
        .pointer("/contracts/BuilderAccountFactory/planned_address")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            BuilderIdentityError::InvalidDescriptor(
                "embedded deployment plan has no factory address".into(),
            )
        })?;
    let factory_address = serde_json::from_str(&format!("\"{factory}\""))?;
    Ok(CandidateNetwork { factory_address })
}

pub(crate) fn builder_account_creation_bytecode() -> Result<Vec<u8>, BuilderIdentityError> {
    builder_account_creation_bytecode_for(BuilderAccountGeneration::V0_8_0)
}

fn builder_account_creation_bytecode_for(
    generation: BuilderAccountGeneration,
) -> Result<Vec<u8>, BuilderIdentityError> {
    let text = generation.creation_hex().trim();
    let encoded = text.strip_prefix("0x").ok_or_else(|| {
        BuilderIdentityError::InvalidDescriptor(
            "BuilderAccount creation bytecode needs a 0x prefix".into(),
        )
    })?;
    if encoded.is_empty()
        || encoded.len() % 2 != 0
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(BuilderIdentityError::InvalidDescriptor(
            "BuilderAccount creation bytecode is not canonical lowercase hex".into(),
        ));
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("hex is ASCII");
            u8::from_str_radix(pair, 16).map_err(|error| {
                BuilderIdentityError::InvalidDescriptor(format!(
                    "invalid BuilderAccount creation bytecode: {error}"
                ))
            })
        })
        .collect()
}

fn local_key_tag(identity_root: &Path) -> Result<String, BuilderIdentityError> {
    let absolute = if identity_root.is_absolute() {
        identity_root.to_path_buf()
    } else {
        std::env::current_dir()?.join(identity_root)
    };
    let mut input = Vec::with_capacity(KEY_TAG_DOMAIN.len() + absolute.as_os_str().len());
    input.extend_from_slice(KEY_TAG_DOMAIN);
    input.extend_from_slice(absolute.to_string_lossy().as_bytes());
    let digest = sha256(&input).to_string();
    Ok(format!("org.tohseno.builder.device.{}", &digest[2..34]))
}

fn verify_helper_identity(
    expected: &BuilderIdentity,
    observed: &AppleDeviceIdentity,
) -> Result<(), BuilderIdentityError> {
    if observed.tag != expected.local_key_tag
        || observed.public_key != expected.device.public_key
        || observed.backend != expected.key_backend
        || observed.security_level != expected.security_level
        || observed.test_only != expected.test_only
    {
        return Err(BuilderIdentityError::InvalidDescriptor(
            "Keychain DeviceKey no longer matches builder.json".into(),
        ));
    }
    Ok(())
}

fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<(), BuilderIdentityError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    reject_symlink(path)?;
    let bytes = serde_json::to_vec_pretty(value)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| BuilderIdentityError::InvalidDescriptor("invalid state path".into()))?;
    for ordinal in 1_u32.. {
        let temporary =
            path.with_file_name(format!(".{file_name}.tmp-{}-{ordinal}", std::process::id()));
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
        {
            Ok(mut file) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    file.set_permissions(fs::Permissions::from_mode(0o600))?;
                }
                file.write_all(&bytes)?;
                file.write_all(b"\n")?;
                file.sync_all()?;
                fs::rename(&temporary, path)?;
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    unreachable!()
}

fn reject_symlink(path: &Path) -> Result<(), BuilderIdentityError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(BuilderIdentityError::InvalidDescriptor(format!(
                "refusing symlinked identity state {}",
                path.display()
            )))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug)]
pub enum BuilderIdentityError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Apple(AppleIdentityError),
    Protocol(String),
    InvalidConfiguration(String),
    InvalidDescriptor(String),
}

impl std::fmt::Display for BuilderIdentityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Apple(error) => write!(formatter, "{error}"),
            Self::Protocol(error) => write!(formatter, "protocol identity failed: {error}"),
            Self::InvalidConfiguration(error) => {
                write!(formatter, "invalid identity configuration: {error}")
            }
            Self::InvalidDescriptor(error) => {
                write!(formatter, "invalid BuilderID descriptor: {error}")
            }
        }
    }
}

impl std::error::Error for BuilderIdentityError {}

impl From<std::io::Error> for BuilderIdentityError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for BuilderIdentityError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<AppleIdentityError> for BuilderIdentityError {
    fn from(value: AppleIdentityError) -> Self {
        Self::Apple(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn supported_generation_inputs_are_self_consistent_and_distinct() {
        for generation in SUPPORTED_BUILDER_ACCOUNT_GENERATIONS {
            let network = candidate_network(generation).unwrap();
            assert_ne!(network.factory_address.as_bytes(), &[0; 20]);
            assert!(
                builder_account_creation_bytecode_for(generation)
                    .unwrap()
                    .len()
                    > 1_000
            );
        }
        assert_ne!(
            candidate_network(BuilderAccountGeneration::V0_7_0)
                .unwrap()
                .factory_address,
            candidate_network(BuilderAccountGeneration::V0_8_0)
                .unwrap()
                .factory_address
        );
    }

    #[test]
    fn claimed_builder_id_resolves_one_exact_generation() {
        let public_key = P256PublicKey {
            x: Bytes32::from_hex(
                "x",
                "0x6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
            )
            .unwrap(),
            y: Bytes32::from_hex(
                "y",
                "0x4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
            )
            .unwrap(),
        };
        let legacy =
            initial_device_builder_id_for(BuilderAccountGeneration::V0_7_0, &public_key).unwrap();
        let current =
            initial_device_builder_id_for(BuilderAccountGeneration::V0_8_0, &public_key).unwrap();
        assert_ne!(legacy, current);
        assert_eq!(
            resolve_initial_device_builder_id(legacy, &public_key).unwrap(),
            BuilderAccountGeneration::V0_7_0
        );
        assert_eq!(
            resolve_initial_device_builder_id(current, &public_key).unwrap(),
            BuilderAccountGeneration::V0_8_0
        );
        assert!(resolve_initial_device_builder_id(
            BuilderId::new(Address20::from_bytes([0x55; 20])),
            &public_key
        )
        .is_err());
    }

    #[test]
    fn frozen_v0_7_descriptor_loads_without_rewrite() {
        let public_key = P256PublicKey {
            x: Bytes32::from_hex(
                "x",
                "0x6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
            )
            .unwrap(),
            y: Bytes32::from_hex(
                "y",
                "0x4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
            )
            .unwrap(),
        };
        let device = BuilderDeviceKey::from_public_key(public_key.clone()).unwrap();
        let account_salt = initial_builder_account_salt(&public_key).unwrap();
        let generation = BuilderAccountGeneration::V0_7_0;
        let network = candidate_network(generation).unwrap();
        let builder_id = initial_device_builder_id_for(generation, &public_key).unwrap();
        let identity = BuilderIdentity {
            schema: BUILDER_SCHEMA.into(),
            candidate_version: generation.version().into(),
            chain_id: ROBINHOOD_CHAIN_ID,
            factory_address: network.factory_address,
            account_salt,
            account_address: builder_id.account(),
            builder_id,
            device,
            local_key_tag: "org.tohseno.builder.device.legacy".into(),
            key_backend: "software_test".into(),
            security_level: "software_test".into(),
            test_only: true,
            deployment_status: BuilderDeploymentStatus::Predicted,
            recovery: None,
            created_at_unix: 1,
        };
        let directory = tempfile::tempdir().unwrap();
        let manager = BuilderIdentityManager::at(directory.path());
        manager.save(&identity).unwrap();
        let before = fs::read(manager.path()).unwrap();
        assert_eq!(manager.load().unwrap(), identity);
        assert_eq!(fs::read(manager.path()).unwrap(), before);
    }

    #[test]
    fn repeated_ensure_preserves_builder_id_and_never_stores_private_key() {
        let directory = tempfile::tempdir().unwrap();
        let helper = directory.path().join("helper");
        let state = directory.path().join("helper-state");
        let script = format!(
            r#"#!/bin/sh
set -eu
command="$1"
if [ "$command" = "public" ] && [ ! -f "{state}" ]; then
  printf '%s\n' '{{"error":{{"code":"identity_not_found","message":"missing"}},"ok":false,"schema":"tohseno.apple-identity/1"}}' >&2
  exit 1
fi
if [ "$command" = "create" ]; then
  : > "{state}"
fi
printf '%s\n' '{{"command":"'"$command"'","ok":true,"result":{{"backend":"software_test","key_id":"sha256:test","public_key":{{"x":"0x6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296","y":"0x4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"}},"security_level":"software_test","tag":"'"$3"'","test_only":true}},"schema":"tohseno.apple-identity/1"}}'
"#,
            state = state.display(),
        );
        fs::write(&helper, script).unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        let bridge = AppleIdentityBridge::at(helper).unwrap();
        let manager = BuilderIdentityManager::at(directory.path().join("identity"));
        std::env::set_var("TOHSENO_IDENTITY_BACKEND", "software-test");
        let first = manager.ensure_with_bridge(&bridge).unwrap();
        let second = manager.ensure_with_bridge(&bridge).unwrap();
        std::env::remove_var("TOHSENO_IDENTITY_BACKEND");

        assert_eq!(first.builder_id, second.builder_id);
        assert_eq!(first.candidate_version, CURRENT_BUILDER_ACCOUNT_GENERATION);
        let stored = fs::read_to_string(manager.path()).unwrap();
        assert!(!stored.contains("private"));
        assert!(!stored.contains("mnemonic"));
        assert!(first.test_only);
        let signing_error = manager
            .sign_digest_with_bridge(&first, Bytes32::new([7; 32]), &bridge)
            .unwrap_err();
        assert!(signing_error
            .to_string()
            .contains("cannot authorize public actions"));
        // Local record signing stays available so a Mac without Secure
        // Enclave access can still complete private Shots: the policy gate
        // must not fire, even though this stub helper cannot actually sign.
        let record_error = manager
            .sign_record_digest_with_bridge(&first, Bytes32::new([7; 32]), &bridge)
            .unwrap_err();
        assert!(!record_error.to_string().contains("cannot authorize"));

        let replacement_public_key = P256PublicKey {
            x: Bytes32::from_hex(
                "replacement.x",
                "0x6ff03b949241ce1dadd43519e6960e0a85b41a69a05c328103aa2bce1594ca16",
            )
            .unwrap(),
            y: Bytes32::from_hex(
                "replacement.y",
                "0x3c4f753a55bf01dc53f6c0b0c7eee78b40c6ff7d25a96e2282b989cef71c144a",
            )
            .unwrap(),
        };
        let mut unproven_replacement = first.clone();
        unproven_replacement.device =
            BuilderDeviceKey::from_public_key(replacement_public_key).unwrap();
        let descriptor_error = manager.save(&unproven_replacement).unwrap_err();
        assert!(descriptor_error
            .to_string()
            .contains("stored BuilderID does not reproduce"));
        let signing_error = manager
            .sign_digest_with_bridge(&unproven_replacement, Bytes32::new([8; 32]), &bridge)
            .unwrap_err();
        assert!(signing_error
            .to_string()
            .contains("stored BuilderID does not reproduce"));
    }

    #[test]
    fn account_salt_is_stable_and_domain_separated() {
        let public_key = tohseno_protocol::signature::P256PublicKey {
            x: Bytes32::from_hex(
                "x",
                "0x6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
            )
            .unwrap(),
            y: Bytes32::from_hex(
                "y",
                "0x4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
            )
            .unwrap(),
        };
        let device = BuilderDeviceKey::from_public_key(public_key).unwrap();
        assert_eq!(
            initial_builder_account_salt(&device.public_key).unwrap(),
            initial_builder_account_salt(&device.public_key).unwrap()
        );
        assert_ne!(
            initial_builder_account_salt(&device.public_key).unwrap(),
            device.key_id
        );
    }
}
