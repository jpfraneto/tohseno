use crate::{NetworkError, Result};
use serde::{Deserialize, Serialize};
use tohseno_protocol::canonical;
use tohseno_protocol::digest::{Address20, Bytes32, ShotId};
use tohseno_protocol::identity::{BuilderId, ROBINHOOD_CHAIN_ID};
use tohseno_protocol::record::CanonicalTimestamp;
use tohseno_protocol::signature::{DetachedP256Signature, P256PublicKey};

pub const CATALOG_RELEASE_SCHEMA_V1: &str = "tohseno.catalog-release/1";
pub const CATALOG_RELEASE_SCHEMA: &str = "tohseno.catalog-release/2";
pub const SIGNED_CATALOG_RELEASE_SCHEMA: &str = "tohseno.signed-catalog-release/1";
pub const CONTRACT_GENERATION: &str = "0.8.0";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogGeneration {
    pub contract_generation: String,
    pub chain_id: u64,
    pub builder_account_factory: Address20,
    pub shot_registry: Address20,
    pub activation_signing_digest: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogDisplay {
    pub name: String,
    pub description: String,
    pub icon_sha256: Option<Bytes32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_byte_length: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_media_type: Option<PublicImageMediaType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub screenshots: Vec<CatalogScreenshot>,
    pub builder_handle: Option<String>,
    pub app_slug: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PublicImageMediaType {
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/jpeg")]
    Jpeg,
}

impl PublicImageMediaType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogScreenshot {
    pub sha256: Bytes32,
    pub byte_length: u64,
    pub media_type: PublicImageMediaType,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceArtifact {
    pub format: SourceArtifactFormat,
    pub sha256: Bytes32,
    pub byte_length: u64,
    pub source_tree_sha256: Bytes32,
    pub file_count: u64,
    pub uncompressed_byte_length: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceArtifactFormat {
    DeterministicTar,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XcodeBuildRecipe {
    pub container_kind: XcodeContainerKind,
    pub container_path: String,
    pub scheme: String,
    pub original_bundle_identifier: String,
    pub minimum_ios: String,
    pub device_families: Vec<String>,
    pub dependency_locks: Vec<DependencyLock>,
    pub safety: BuildSafety,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XcodeContainerKind {
    Project,
    Workspace,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyLock {
    pub path: String,
    pub sha256: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildSafety {
    pub classification: BuildSafetyClassification,
    pub reasons: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildSafetyClassification {
    Green,
    RequiresMacReview,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleasePermissions {
    pub install_allowed: bool,
    pub fork_allowed: bool,
    pub distributor_rights_declared: bool,
    pub spdx_license: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogParentRelease {
    pub parent_shot_id: ShotId,
    pub parent_release_digest: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogRelease {
    pub schema: String,
    pub generation: CatalogGeneration,
    pub shot_id: ShotId,
    pub builder_id: BuilderId,
    pub release_id: Bytes32,
    pub published_at: CanonicalTimestamp,
    pub display: CatalogDisplay,
    pub source: SourceArtifact,
    pub build: XcodeBuildRecipe,
    pub permissions: ReleasePermissions,
    pub parent: Option<CatalogParentRelease>,
    pub checkpoint_sequence: u64,
    pub public_checkpoint_digest: Bytes32,
}

impl CatalogRelease {
    pub fn validate(&self) -> Result<()> {
        if self.schema != CATALOG_RELEASE_SCHEMA && self.schema != CATALOG_RELEASE_SCHEMA_V1 {
            return invalid(format!(
                "schema must be {CATALOG_RELEASE_SCHEMA_V1} or {CATALOG_RELEASE_SCHEMA}"
            ));
        }
        self.generation.validate()?;
        if self.shot_id.is_zero() {
            return invalid("shot_id must not be zero");
        }
        self.builder_id.validate()?;
        nonzero("release_id", self.release_id)?;
        if self.published_at.unix_timestamp() <= 0 {
            return invalid("published_at must be after the Unix epoch");
        }
        self.display.validate(&self.schema)?;
        self.source.validate()?;
        self.build.validate()?;
        self.permissions.validate()?;
        if let Some(parent) = &self.parent {
            if parent.parent_shot_id.is_zero() {
                return invalid("parent_shot_id must not be zero");
            }
            if parent.parent_shot_id == self.shot_id {
                return invalid("a fork cannot name itself as parent");
            }
            nonzero("parent_release_digest", parent.parent_release_digest)?;
        }
        if self.checkpoint_sequence == 0 || self.checkpoint_sequence > MAX_SAFE_INTEGER {
            return invalid("checkpoint_sequence must be a positive JavaScript-safe integer");
        }
        nonzero("public_checkpoint_digest", self.public_checkpoint_digest)?;
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        Ok(canonical::to_vec(self)?)
    }

    pub fn digest(&self) -> Result<Bytes32> {
        self.validate()?;
        Ok(canonical::sha256_commitment(self)?)
    }
}

impl CatalogGeneration {
    fn validate(&self) -> Result<()> {
        if self.contract_generation != CONTRACT_GENERATION {
            return invalid(format!("contract_generation must be {CONTRACT_GENERATION}"));
        }
        if self.chain_id != ROBINHOOD_CHAIN_ID {
            return invalid(format!("chain_id must be {ROBINHOOD_CHAIN_ID}"));
        }
        for (name, address) in [
            ("builder_account_factory", self.builder_account_factory),
            ("shot_registry", self.shot_registry),
        ] {
            if address.as_bytes().iter().all(|byte| *byte == 0) {
                return invalid(format!("{name} must not be zero"));
            }
        }
        if self.builder_account_factory == self.shot_registry {
            return invalid("factory and registry must differ");
        }
        nonzero("activation_signing_digest", self.activation_signing_digest)
    }
}

impl CatalogDisplay {
    fn validate(&self, release_schema: &str) -> Result<()> {
        bounded_text("display.name", &self.name, 1, 160)?;
        bounded_text("display.description", &self.description, 1, 2_000)?;
        if let Some(value) = self.icon_sha256 {
            nonzero("display.icon_sha256", value)?;
        }
        if release_schema == CATALOG_RELEASE_SCHEMA_V1 {
            if self.icon_byte_length.is_some()
                || self.icon_media_type.is_some()
                || !self.screenshots.is_empty()
            {
                return invalid("catalog release v1 cannot carry public presentation media");
            }
        } else {
            if self.icon_sha256.is_none()
                || self.icon_media_type.is_none()
                || !matches!(self.icon_byte_length, Some(1..=5_242_880))
            {
                return invalid("catalog release v2 requires one bounded app icon");
            }
            if self.screenshots.len() > 8 {
                return invalid("display.screenshots exceeds the eight-image bound");
            }
            let mut digests = std::collections::BTreeSet::new();
            for screenshot in &self.screenshots {
                nonzero("display.screenshot.sha256", screenshot.sha256)?;
                if screenshot.byte_length == 0 || screenshot.byte_length > 10 * 1024 * 1024 {
                    return invalid("display screenshot is outside its byte bound");
                }
                if !digests.insert(screenshot.sha256) {
                    return invalid("display screenshots must have unique content digests");
                }
            }
        }
        if let Some(value) = &self.builder_handle {
            identifier("display.builder_handle", value, 2, 32)?;
        }
        if let Some(value) = &self.app_slug {
            identifier("display.app_slug", value, 2, 64)?;
        }
        Ok(())
    }
}

impl SourceArtifact {
    fn validate(&self) -> Result<()> {
        if self.format != SourceArtifactFormat::DeterministicTar {
            return invalid("unsupported source artifact format");
        }
        nonzero("source.sha256", self.sha256)?;
        nonzero("source.source_tree_sha256", self.source_tree_sha256)?;
        if self.byte_length == 0
            || self.byte_length > 512 * 1024 * 1024
            || self.uncompressed_byte_length == 0
            || self.uncompressed_byte_length > 2 * 1024 * 1024 * 1024
            || self.file_count == 0
            || self.file_count > 100_000
        {
            return invalid("source artifact bounds are invalid");
        }
        Ok(())
    }
}

impl XcodeBuildRecipe {
    fn validate(&self) -> Result<()> {
        relative_path("build.container_path", &self.container_path)?;
        bounded_text("build.scheme", &self.scheme, 1, 256)?;
        bundle_id(&self.original_bundle_identifier)?;
        if self.minimum_ios.is_empty()
            || self.minimum_ios.len() > 32
            || !self
                .minimum_ios
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
        {
            return invalid("build.minimum_ios is invalid");
        }
        if self.device_families.is_empty() || self.device_families.len() > 8 {
            return invalid("build.device_families must contain 1..=8 values");
        }
        let mut prior = None;
        for value in &self.device_families {
            bounded_text("build.device_family", value, 1, 64)?;
            if prior.is_some_and(|item: &String| item >= value) {
                return invalid("build.device_families must be sorted and unique");
            }
            prior = Some(value);
        }
        if self.dependency_locks.len() > 128 {
            return invalid("too many dependency locks");
        }
        let mut prior_path = None;
        for lock in &self.dependency_locks {
            relative_path("build.dependency_lock.path", &lock.path)?;
            nonzero("build.dependency_lock.sha256", lock.sha256)?;
            if prior_path.is_some_and(|item: &String| item >= &lock.path) {
                return invalid("dependency locks must be sorted and unique");
            }
            prior_path = Some(&lock.path);
        }
        self.safety.validate()
    }
}

impl BuildSafety {
    fn validate(&self) -> Result<()> {
        if self.reasons.len() > 64 {
            return invalid("too many build-safety reasons");
        }
        let mut prior = None;
        for reason in &self.reasons {
            bounded_text("build.safety.reason", reason, 1, 512)?;
            if prior.is_some_and(|item: &String| item >= reason) {
                return invalid("build-safety reasons must be sorted and unique");
            }
            prior = Some(reason);
        }
        if self.classification == BuildSafetyClassification::Green && !self.reasons.is_empty() {
            return invalid("a green build profile cannot have review reasons");
        }
        if self.classification != BuildSafetyClassification::Green && self.reasons.is_empty() {
            return invalid("a non-green build profile must explain why");
        }
        Ok(())
    }
}

impl ReleasePermissions {
    fn validate(&self) -> Result<()> {
        if !self.install_allowed {
            return invalid("catalog release must be installable");
        }
        if !self.distributor_rights_declared {
            return invalid("Builder must affirm distribution rights");
        }
        if let Some(value) = &self.spdx_license {
            if value.is_empty()
                || value.len() > 96
                || !value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric()
                        || matches!(byte, b'.' | b'-' | b'+' | b'(' | b')' | b' ')
                })
            {
                return invalid("SPDX license expression is invalid");
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedCatalogRelease {
    pub schema: String,
    pub release: CatalogRelease,
    pub signer: P256PublicKey,
    pub authorization: DetachedP256Signature,
}

impl SignedCatalogRelease {
    pub fn verify(&self) -> Result<()> {
        if self.schema != SIGNED_CATALOG_RELEASE_SCHEMA {
            return invalid(format!("schema must be {SIGNED_CATALOG_RELEASE_SCHEMA}"));
        }
        self.release.validate()?;
        self.signer.validate()?;
        self.authorization.validate()?;
        let digest = self.release.digest()?;
        if digest != self.authorization.digest {
            return invalid("catalog signature digest differs from the canonical release");
        }
        self.authorization.verify(&self.signer)?;
        Ok(())
    }
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(NetworkError::Invalid(message.into()))
}

fn nonzero(name: &str, value: Bytes32) -> Result<()> {
    if value == Bytes32::ZERO {
        invalid(format!("{name} must not be zero"))
    } else {
        Ok(())
    }
}

fn bounded_text(name: &str, value: &str, minimum: usize, maximum: usize) -> Result<()> {
    if !(minimum..=maximum).contains(&value.len()) || value.chars().any(char::is_control) {
        invalid(format!("{name} is outside its text bound"))
    } else {
        Ok(())
    }
}

fn identifier(name: &str, value: &str, minimum: usize, maximum: usize) -> Result<()> {
    if !(minimum..=maximum).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || value.starts_with('-')
        || value.ends_with('-')
        || value.contains("--")
    {
        invalid(format!("{name} is invalid"))
    } else {
        Ok(())
    }
}

fn relative_path(name: &str, value: &str) -> Result<()> {
    let path = std::path::Path::new(value);
    if value.is_empty()
        || value.contains('\\')
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        invalid(format!("{name} must be a safe relative path"))
    } else {
        Ok(())
    }
}

fn bundle_id(value: &str) -> Result<()> {
    if value.len() > 255
        || value.split('.').count() < 2
        || value.split('.').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        invalid("original bundle identifier is invalid")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tohseno_protocol::digest::Address20;

    #[test]
    fn rejects_catalog_metadata_that_could_escape_or_misbind() {
        assert!(relative_path("path", "../App.xcodeproj").is_err());
        assert!(relative_path("path", "/App.xcodeproj").is_err());
        assert!(identifier("slug", "Global Name", 2, 64).is_err());
        assert!(bundle_id("not-a-bundle").is_err());
        let generation = CatalogGeneration {
            contract_generation: CONTRACT_GENERATION.into(),
            chain_id: ROBINHOOD_CHAIN_ID,
            builder_account_factory: Address20::from_bytes([1; 20]),
            shot_registry: Address20::from_bytes([1; 20]),
            activation_signing_digest: Bytes32::new([2; 32]),
        };
        assert!(generation.validate().is_err());
    }

    #[test]
    fn catalog_v2_binds_one_icon_and_bounded_unique_screenshots() {
        let icon = Bytes32::new([1; 32]);
        let screenshot = CatalogScreenshot {
            sha256: Bytes32::new([2; 32]),
            byte_length: 128,
            media_type: PublicImageMediaType::Jpeg,
        };
        let display = CatalogDisplay {
            name: "Field Notes".into(),
            description: "A small native notebook.".into(),
            icon_sha256: Some(icon),
            icon_byte_length: Some(256),
            icon_media_type: Some(PublicImageMediaType::Png),
            screenshots: vec![screenshot.clone()],
            builder_handle: None,
            app_slug: Some("field-notes".into()),
        };
        assert!(display.validate(CATALOG_RELEASE_SCHEMA).is_ok());
        assert!(display.validate(CATALOG_RELEASE_SCHEMA_V1).is_err());

        let mut duplicate = display.clone();
        duplicate.screenshots.push(screenshot);
        assert!(duplicate.validate(CATALOG_RELEASE_SCHEMA).is_err());

        let mut missing_icon = display;
        missing_icon.icon_sha256 = None;
        assert!(missing_icon.validate(CATALOG_RELEASE_SCHEMA).is_err());
    }
}
