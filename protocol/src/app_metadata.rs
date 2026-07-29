use crate::builder::MAX_SAFE_JSON_INTEGER;
use crate::digest::{Address20, Bytes32, ShotId};
use crate::fascia::{AppleSurface, Capability, DistributionState, FasciaManifest, StorageKind};
use crate::identity::{BuilderId, ROBINHOOD_CHAIN_ID};
use crate::record::{FactoryDescriptor, ShotOrigin, ShotRecord, APPLE_FASCIA_ID, PROTOCOL_NAME};
use crate::text::{invalid, validate_bundle_id, validate_token};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const APP_METADATA_SCHEMA: &str = "tohseno.app-metadata/1";

/// The one public metadata object embedded in every generated Apple app.
///
/// Its serialized representation is consumed directly by the normative Swift
/// `TohsenoMetadata` decoder. Keep the shared exact-byte fixture in
/// `test-vectors/app-metadata-v1.json` synchronized with this type.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppMetadata {
    #[serde(rename = "protocol")]
    pub protocol_name: String,
    pub schema: String,
    pub fascia: String,
    pub shot_id: ShotId,
    pub builder_id: BuilderId,
    pub sequence: u32,
    pub previous: Option<Bytes32>,
    pub origin: Option<ShotOrigin>,
    pub evolution_commitment: Bytes32,
    pub source_tree_sha256: Bytes32,
    pub fascia_sha256: Bytes32,
    pub bundle_id: String,
    pub bundle_version: u32,
    pub factory: FactoryDescriptor,
    pub distribution: AppMetadataDistribution,
    pub capabilities: Vec<AppMetadataCapabilityDeclaration>,
    pub network: Vec<AppMetadataNetworkDeclaration>,
    pub registry: Option<AppMetadataRegistryReference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppMetadataDistribution {
    pub state: DistributionState,
    pub supported_apple_surfaces: Vec<AppleSurface>,
    pub app_store_id: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppMetadataCapability {
    LocalStorage,
    NetworkAccess,
    PrivateCloudkitSync,
    Storekit,
    Notifications,
    Camera,
    Microphone,
    Location,
    Contacts,
    Health,
    Bluetooth,
    OtherAppleEntitlements,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppMetadataCapabilityDeclaration {
    pub capability: AppMetadataCapability,
    pub purpose: String,
    pub details: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppMetadataNetworkDeclaration {
    pub endpoint: String,
    pub purpose: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppMetadataRegistryReference {
    pub chain_id: u64,
    pub contract: Address20,
    pub transaction: Option<Bytes32>,
}

impl AppMetadata {
    /// Constructs the embedded object only from already validated signed facts
    /// and the concrete per-Shot Fascia declaration.
    pub fn for_record(
        record: &ShotRecord,
        evolution_commitment: Bytes32,
        fascia: &FasciaManifest,
    ) -> Result<Self> {
        record.validate()?;
        fascia.validate()?;
        if fascia.protocol != record.protocol
            || fascia.fascia != record.fascia
            || fascia.distribution.bundle_id != record.bundle_id
            || fascia.distribution.bundle_version != record.bundle_version
        {
            return Err(invalid(
                "app_metadata",
                "record and Fascia identity or distribution facts do not agree",
            ));
        }
        if record.commitment()? != evolution_commitment {
            return Err(invalid(
                "app_metadata.evolution_commitment",
                "must equal the canonical signed Shot record commitment",
            ));
        }

        let capabilities = fascia
            .capabilities
            .iter()
            .map(|declaration| {
                let capability = AppMetadataCapability::from(declaration.capability);
                let details = match declaration.capability {
                    Capability::OtherAppleEntitlement => {
                        declaration.entitlement.iter().cloned().collect()
                    }
                    Capability::PrivateCloudkitSync => fascia
                        .storage
                        .iter()
                        .filter(|storage| storage.kind == StorageKind::PrivateCloudkit)
                        .map(|storage| storage.purpose.clone())
                        .collect(),
                    _ => Vec::new(),
                };
                AppMetadataCapabilityDeclaration {
                    capability,
                    purpose: declaration.purpose.clone(),
                    details,
                }
            })
            .collect();
        let metadata = Self {
            protocol_name: PROTOCOL_NAME.into(),
            schema: APP_METADATA_SCHEMA.into(),
            fascia: APPLE_FASCIA_ID.into(),
            shot_id: record.shot_id,
            builder_id: record.builder_id,
            sequence: record.sequence,
            previous: record.previous,
            origin: record.origin.clone(),
            evolution_commitment,
            source_tree_sha256: record.source_tree_sha256,
            fascia_sha256: record.fascia_sha256,
            bundle_id: record.bundle_id.clone(),
            bundle_version: record.bundle_version,
            factory: record.factory.clone(),
            distribution: AppMetadataDistribution {
                state: fascia.distribution.state,
                supported_apple_surfaces: fascia.distribution.surfaces.clone(),
                app_store_id: fascia.distribution.app_store_id,
            },
            capabilities,
            network: fascia
                .network
                .iter()
                .map(|declaration| AppMetadataNetworkDeclaration {
                    endpoint: declaration.endpoint.clone(),
                    purpose: declaration.purpose.clone(),
                })
                .collect(),
            registry: None,
        };
        metadata.validate()?;
        Ok(metadata)
    }

    pub fn validate(&self) -> Result<()> {
        if self.protocol_name != PROTOCOL_NAME {
            return Err(invalid(
                "app_metadata.protocol",
                format!("must be {PROTOCOL_NAME}"),
            ));
        }
        if self.schema != APP_METADATA_SCHEMA {
            return Err(invalid(
                "app_metadata.schema",
                format!("must be {APP_METADATA_SCHEMA}"),
            ));
        }
        if self.fascia != APPLE_FASCIA_ID {
            return Err(invalid(
                "app_metadata.fascia",
                format!("must be {APPLE_FASCIA_ID}"),
            ));
        }
        if self.shot_id.is_zero() {
            return Err(invalid("app_metadata.shot_id", "must not be zero"));
        }
        if self.evolution_commitment == Bytes32::ZERO
            || self.source_tree_sha256 == Bytes32::ZERO
            || self.fascia_sha256 == Bytes32::ZERO
        {
            return Err(invalid(
                "app_metadata.commitment",
                "evolution, source-tree, and Fascia commitments must be nonzero",
            ));
        }
        self.builder_id.validate()?;
        validate_bundle_id("app_metadata.bundle_id", &self.bundle_id)?;
        if self.sequence == 0 || self.bundle_version != self.sequence {
            return Err(invalid(
                "app_metadata.bundle_version",
                "sequence must be positive and bundle_version must equal sequence",
            ));
        }
        match (&self.origin, self.sequence, self.previous) {
            (None, 1, None) => {}
            (None, 1, Some(_)) => {
                return Err(invalid(
                    "app_metadata.previous",
                    "must be null for sequence 1",
                ))
            }
            (None, _, Some(_)) => {}
            (None, _, None) => {
                return Err(invalid(
                    "app_metadata.previous",
                    "must identify the preceding Evolution",
                ))
            }
            (
                Some(ShotOrigin::LegacyAdoption {
                    legacy_latest_shot,
                    legacy_source_sha256,
                }),
                sequence,
                None,
            ) if *legacy_latest_shot > 0
                && legacy_latest_shot.checked_add(1) == Some(sequence)
                && *legacy_source_sha256 != Bytes32::ZERO => {}
            (Some(_), _, _) => {
                return Err(invalid(
                    "app_metadata.origin",
                    "must describe a null-previous legacy root at legacy_latest_shot + 1",
                ))
            }
        }
        self.factory.validate()?;

        let surfaces = self
            .distribution
            .supported_apple_surfaces
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if surfaces.is_empty() || surfaces.len() != self.distribution.supported_apple_surfaces.len()
        {
            return Err(invalid(
                "app_metadata.distribution.supported_apple_surfaces",
                "must be nonempty and unique",
            ));
        }
        match (self.distribution.state, self.distribution.app_store_id) {
            (DistributionState::AppStore, Some(identifier))
                if identifier > 0 && identifier <= MAX_SAFE_JSON_INTEGER => {}
            (DistributionState::AppStore, _) => {
                return Err(invalid(
                    "app_metadata.distribution.app_store_id",
                    "is required for App Store state",
                ))
            }
            (_, None) => {}
            (_, Some(_)) => {
                return Err(invalid(
                    "app_metadata.distribution.app_store_id",
                    "is allowed only for App Store state",
                ))
            }
        }

        let mut capabilities = BTreeSet::new();
        for declaration in &self.capabilities {
            validate_token(
                "app_metadata.capability.purpose",
                &declaration.purpose,
                1,
                500,
            )?;
            if !capabilities.insert(declaration.capability) {
                return Err(invalid(
                    "app_metadata.capabilities",
                    "must not repeat a capability",
                ));
            }
            for detail in &declaration.details {
                validate_token("app_metadata.capability.details", detail, 1, 500)?;
            }
            if matches!(
                declaration.capability,
                AppMetadataCapability::PrivateCloudkitSync
                    | AppMetadataCapability::OtherAppleEntitlements
            ) && declaration.details.is_empty()
            {
                return Err(invalid(
                    "app_metadata.capability.details",
                    "private CloudKit and other Apple entitlements require details",
                ));
            }
        }
        let mut endpoints = BTreeSet::new();
        for declaration in &self.network {
            validate_token(
                "app_metadata.network.endpoint",
                &declaration.endpoint,
                1,
                2048,
            )?;
            validate_token("app_metadata.network.purpose", &declaration.purpose, 1, 500)?;
            if !endpoints.insert(&declaration.endpoint) {
                return Err(invalid(
                    "app_metadata.network",
                    "must not repeat an endpoint",
                ));
            }
        }
        if capabilities.contains(&AppMetadataCapability::NetworkAccess) == self.network.is_empty() {
            return Err(invalid(
                "app_metadata.network",
                "network_access and declared endpoints must agree",
            ));
        }
        if let Some(registry) = &self.registry {
            if registry.chain_id != ROBINHOOD_CHAIN_ID {
                return Err(invalid(
                    "app_metadata.registry.chain_id",
                    format!("must be {ROBINHOOD_CHAIN_ID}"),
                ));
            }
            if registry.contract.as_bytes().iter().all(|byte| *byte == 0)
                || registry.transaction == Some(Bytes32::ZERO)
            {
                return Err(invalid(
                    "app_metadata.registry",
                    "contract and present transaction must be nonzero",
                ));
            }
        }
        Ok(())
    }
}

impl From<Capability> for AppMetadataCapability {
    fn from(value: Capability) -> Self {
        match value {
            Capability::LocalStorage => Self::LocalStorage,
            Capability::NetworkAccess => Self::NetworkAccess,
            Capability::PrivateCloudkitSync => Self::PrivateCloudkitSync,
            Capability::Storekit => Self::Storekit,
            Capability::Notifications => Self::Notifications,
            Capability::Camera => Self::Camera,
            Capability::Microphone => Self::Microphone,
            Capability::Location => Self::Location,
            Capability::Contacts => Self::Contacts,
            Capability::Health => Self::Health,
            Capability::Bluetooth => Self::Bluetooth,
            Capability::OtherAppleEntitlement => Self::OtherAppleEntitlements,
        }
    }
}
