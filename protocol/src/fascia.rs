use crate::builder::MAX_SAFE_JSON_INTEGER;
use crate::canonical;
use crate::digest::Bytes32;
use crate::record::{APPLE_FASCIA_ID, PROTOCOL_NAME};
use crate::text::{invalid, validate_bundle_id, validate_token};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const FASCIA_SCHEMA: &str = "tohseno.fascia/1";
pub const REQUIRED_FASCIA_FILES: &[&str] = &[
    "TOHSENO/shot.json",
    "TOHSENO/signature.json",
    "TOHSENO/fascia.json",
    "TOHSENO/conformance.json",
    "TOHSENO/embedded-provenance.json",
    "TOHSENO/FASCIA.md",
    "TOHSENO/IDENTITY.md",
    "TOHSENO/STORAGE.md",
    "TOHSENO/CONTINUITY.md",
    "TOHSENO/PRIVACY.md",
    "TOHSENO/PROVENANCE.md",
    "TOHSENO/DISTRIBUTION.md",
    "TohsenoFascia/InstallationIdentity.swift",
    "TohsenoFascia/ContinuityEnvelope.swift",
    "TohsenoFascia/LocalPersistence.swift",
    "TohsenoFascia/Provenance.swift",
    "TohsenoFascia/TohsenoMetadata.swift",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
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
    OtherAppleEntitlement,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDeclaration {
    pub capability: Capability,
    pub purpose: String,
    /// Required only for `other_apple_entitlement`.
    pub entitlement: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageKind {
    SwiftData,
    UserDefaults,
    Keychain,
    SecureEnclave,
    Files,
    PrivateCloudkit,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageDeclaration {
    pub kind: StorageKind,
    pub purpose: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDeclaration {
    pub endpoint: String,
    pub purpose: String,
    pub data_categories: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleSurface {
    Iphone,
    Ipad,
    Vision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributionState {
    Local,
    Published,
    AppStore,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DistributionDeclaration {
    pub bundle_id: String,
    pub bundle_version: u32,
    pub surfaces: Vec<AppleSurface>,
    pub state: DistributionState,
    pub app_store_id: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationIdentityDeclaration {
    pub algorithm: String,
    pub scope: String,
    pub hardware_backed_when_available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyDeclaration {
    pub telemetry: bool,
    pub tracking: bool,
    pub account_required: bool,
    pub silent_identity_linkage: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FasciaManifest {
    pub protocol: String,
    pub schema: String,
    pub fascia: String,
    pub required_files: Vec<String>,
    pub installation_identity: InstallationIdentityDeclaration,
    pub capabilities: Vec<CapabilityDeclaration>,
    pub storage: Vec<StorageDeclaration>,
    pub network: Vec<NetworkDeclaration>,
    pub privacy: PrivacyDeclaration,
    pub distribution: DistributionDeclaration,
}

impl FasciaManifest {
    pub fn validate(&self) -> Result<()> {
        if self.protocol != PROTOCOL_NAME {
            return Err(invalid(
                "fascia.protocol",
                format!("must be {PROTOCOL_NAME}"),
            ));
        }
        if self.schema != FASCIA_SCHEMA {
            return Err(invalid("fascia.schema", format!("must be {FASCIA_SCHEMA}")));
        }
        if self.fascia != APPLE_FASCIA_ID {
            return Err(invalid(
                "fascia.fascia",
                format!("must be {APPLE_FASCIA_ID}"),
            ));
        }
        let required = self
            .required_files
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if required.len() != self.required_files.len()
            || required.len() != REQUIRED_FASCIA_FILES.len()
            || REQUIRED_FASCIA_FILES
                .iter()
                .any(|path| !required.contains(path))
        {
            return Err(invalid(
                "fascia.required_files",
                "must include each normative Fascia file exactly once",
            ));
        }
        if self.installation_identity.algorithm != "p256"
            || self.installation_identity.scope != "app_installation"
            || !self.installation_identity.hardware_backed_when_available
        {
            return Err(invalid(
                "fascia.installation_identity",
                "must declare app-scoped P-256 hardware-backed-when-available identity",
            ));
        }
        if self.privacy.telemetry
            || self.privacy.tracking
            || self.privacy.account_required
            || self.privacy.silent_identity_linkage
        {
            return Err(invalid(
                "fascia.privacy",
                "telemetry, tracking, account walls, and silent linkage must default false",
            ));
        }
        let mut capabilities = BTreeSet::new();
        for declaration in &self.capabilities {
            validate_token("fascia.capability.purpose", &declaration.purpose, 1, 500)?;
            if !capabilities.insert(declaration.capability) {
                return Err(invalid(
                    "fascia.capabilities",
                    "must not repeat a capability",
                ));
            }
            match (declaration.capability, &declaration.entitlement) {
                (Capability::OtherAppleEntitlement, Some(value)) => {
                    validate_token("fascia.capability.entitlement", value, 1, 255)?;
                }
                (Capability::OtherAppleEntitlement, None) => {
                    return Err(invalid(
                        "fascia.capability.entitlement",
                        "is required for other_apple_entitlement",
                    ))
                }
                (_, Some(_)) => {
                    return Err(invalid(
                        "fascia.capability.entitlement",
                        "is allowed only for other_apple_entitlement",
                    ))
                }
                (_, None) => {}
            }
        }
        if !capabilities.contains(&Capability::LocalStorage) {
            return Err(invalid("fascia.capabilities", "must declare local_storage"));
        }
        if !self.network.is_empty() && !capabilities.contains(&Capability::NetworkAccess) {
            return Err(invalid(
                "fascia.capabilities",
                "must declare network_access when endpoints are present",
            ));
        }
        if self.network.is_empty() && capabilities.contains(&Capability::NetworkAccess) {
            return Err(invalid(
                "fascia.network",
                "must declare every network endpoint when network_access is present",
            ));
        }
        let mut storage = BTreeSet::new();
        for declaration in &self.storage {
            validate_token("fascia.storage.purpose", &declaration.purpose, 1, 500)?;
            if !storage.insert(declaration.kind) {
                return Err(invalid("fascia.storage", "must not repeat a storage kind"));
            }
        }
        if storage.is_empty() {
            return Err(invalid(
                "fascia.storage",
                "must declare at least one local storage mechanism",
            ));
        }
        if storage.contains(&StorageKind::PrivateCloudkit)
            != capabilities.contains(&Capability::PrivateCloudkitSync)
        {
            return Err(invalid(
                "fascia.storage",
                "private CloudKit storage and capability declarations must agree",
            ));
        }
        let mut endpoints = BTreeSet::new();
        for endpoint in &self.network {
            validate_token("fascia.network.endpoint", &endpoint.endpoint, 1, 2048)?;
            validate_token("fascia.network.purpose", &endpoint.purpose, 1, 500)?;
            if !endpoints.insert(&endpoint.endpoint) {
                return Err(invalid("fascia.network", "must not repeat an endpoint"));
            }
            if endpoint.data_categories.is_empty() {
                return Err(invalid(
                    "fascia.network.data_categories",
                    "must not be empty",
                ));
            }
            let mut categories = BTreeSet::new();
            for category in &endpoint.data_categories {
                validate_token("fascia.network.data_categories", category, 1, 100)?;
                if !categories.insert(category) {
                    return Err(invalid(
                        "fascia.network.data_categories",
                        "must not contain duplicates",
                    ));
                }
            }
        }
        validate_bundle_id(
            "fascia.distribution.bundle_id",
            &self.distribution.bundle_id,
        )?;
        if self.distribution.bundle_version == 0 || self.distribution.surfaces.is_empty() {
            return Err(invalid(
                "fascia.distribution",
                "bundle_version and supported surfaces must be present",
            ));
        }
        let surfaces = self
            .distribution
            .surfaces
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if surfaces.len() != self.distribution.surfaces.len() {
            return Err(invalid(
                "fascia.distribution.surfaces",
                "must not contain duplicates",
            ));
        }
        match (self.distribution.state, self.distribution.app_store_id) {
            (DistributionState::AppStore, Some(id)) if id > 0 && id <= MAX_SAFE_JSON_INTEGER => {}
            (DistributionState::AppStore, _) => {
                return Err(invalid(
                    "fascia.distribution.app_store_id",
                    "is required for App Store state",
                ))
            }
            (_, None) => {}
            (_, Some(_)) => {
                return Err(invalid(
                    "fascia.distribution.app_store_id",
                    "is allowed only for App Store state",
                ))
            }
        }
        Ok(())
    }

    /// Commits this concrete per-Shot manifest only.
    ///
    /// This is not the record's `fascia_sha256`; that field uses
    /// `fascia_tree::hash_fascia_tree` over the pinned reusable reference.
    pub fn commitment(&self) -> Result<Bytes32> {
        self.validate()?;
        canonical::sha256_commitment(self)
    }
}
