//! Bounded messages crossing the private Mac ↔ Companion publication boundary.
//!
//! These are application-layer records. They reuse the active protocol's
//! exact public action types without adding a protocol encoding.

use crate::catalog::{CatalogRelease, SignedCatalogRelease};
use crate::{NetworkError, Result};
use serde::{Deserialize, Serialize};
use tohseno_protocol::actions::{
    Eip712Domain, RegistryActionV2, SignedRegistryActionV2, SHOT_REGISTRY_DOMAIN,
    SHOT_REGISTRY_V2_EIP712_VERSION,
};
use tohseno_protocol::digest::Bytes32;
use tohseno_protocol::identity::{device_key_id, BuilderDeviceKey};
use tohseno_protocol::signature::P256PublicKey;

pub const BUILDER_DEVICE_ANNOUNCEMENT_SCHEMA: &str = "tohseno.builder-device-announcement/1";
pub const PUBLICATION_APPROVAL_REQUEST_SCHEMA: &str = "tohseno.publication-approval-request/1";
pub const PUBLICATION_APPROVAL_SCHEMA: &str = "tohseno.publication-approval/1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderDeviceAnnouncement {
    pub schema: String,
    pub key_id: Bytes32,
    pub public_key: P256PublicKey,
    pub security_level: String,
    pub test_only: bool,
}

impl BuilderDeviceAnnouncement {
    pub fn validate(&self, allow_test: bool) -> Result<()> {
        if self.schema != BUILDER_DEVICE_ANNOUNCEMENT_SCHEMA {
            return invalid("unsupported Builder DeviceKey announcement schema");
        }
        self.public_key.validate()?;
        if self.key_id != device_key_id(&self.public_key) {
            return invalid("Builder DeviceKey ID does not match its public coordinates");
        }
        if !matches!(
            self.security_level.as_str(),
            "secure_enclave" | "software_test"
        ) {
            return invalid("Builder DeviceKey security level is invalid");
        }
        if self.test_only != (self.security_level == "software_test") {
            return invalid("Builder DeviceKey test marker disagrees with its security level");
        }
        if self.test_only && !allow_test {
            return invalid("a test-only Builder DeviceKey cannot authorize production");
        }
        Ok(())
    }

    pub fn device_key(&self) -> Result<BuilderDeviceKey> {
        self.validate(cfg!(test))?;
        Ok(BuilderDeviceKey {
            key_id: self.key_id,
            public_key: self.public_key.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationApprovalRequest {
    pub schema: String,
    pub job_id: String,
    pub catalog_release: CatalogRelease,
    pub registry_domain: Eip712Domain,
    pub registry_action: RegistryActionV2,
    pub builder_device: BuilderDeviceAnnouncement,
    pub requested_route: String,
    pub issued_at: String,
    pub expires_at: String,
}

impl PublicationApprovalRequest {
    pub fn validate(&self, allow_test_key: bool) -> Result<()> {
        if self.schema != PUBLICATION_APPROVAL_REQUEST_SCHEMA {
            return invalid("unsupported publication approval request schema");
        }
        identifier("job_id", &self.job_id, 1, 128)?;
        self.catalog_release.validate()?;
        self.builder_device.validate(allow_test_key)?;
        self.registry_domain
            .validate_for_version(SHOT_REGISTRY_DOMAIN, SHOT_REGISTRY_V2_EIP712_VERSION)?;
        self.registry_action.validate()?;
        if self.registry_domain.chain_id != self.catalog_release.generation.chain_id
            || self.registry_domain.verifying_contract
                != self.catalog_release.generation.shot_registry
        {
            return invalid("Registry action domain differs from catalog generation");
        }
        let release_shot = self.catalog_release.shot_id;
        let release_head = self.catalog_release.public_checkpoint_digest;
        let release_sequence = self.catalog_release.checkpoint_sequence;
        match &self.registry_action {
            RegistryActionV2::RegisterShot {
                shot_id,
                controller,
                head,
                nonce,
                deadline,
                ..
            } => {
                if release_sequence != 1 || *nonce != 0 {
                    return invalid("first publication must register checkpoint 1 at nonce 0");
                }
                if *shot_id != release_shot
                    || *controller != self.catalog_release.builder_id.account()
                    || *head != release_head
                {
                    return invalid("RegisterShot action differs from the catalog release");
                }
                validate_deadline(*deadline, &self.issued_at, &self.expires_at)?;
            }
            RegistryActionV2::AppendCheckpoint {
                shot_id,
                new_head,
                checkpoint_sequence,
                nonce,
                deadline,
                ..
            } => {
                if *shot_id != release_shot
                    || *new_head != release_head
                    || *checkpoint_sequence != release_sequence
                    || nonce.checked_add(1) != Some(release_sequence)
                {
                    return invalid("AppendCheckpoint action differs from the catalog release");
                }
                validate_deadline(*deadline, &self.issued_at, &self.expires_at)?;
            }
            RegistryActionV2::TransferShot { .. } => {
                return invalid("publication approval cannot authorize Shot transfer");
            }
        }
        if self.requested_route.is_empty()
            || self.requested_route.len() > 160
            || !self.requested_route.starts_with('/')
            || self.requested_route.contains("..")
            || self.requested_route.chars().any(char::is_control)
        {
            return invalid("requested publication route is invalid");
        }
        let issued = parse_time("publication issued_at", &self.issued_at)?;
        let expires = parse_time("publication expires_at", &self.expires_at)?;
        if expires <= issued || expires - issued > time::Duration::hours(24) {
            return invalid("publication approval window must be positive and at most 24 hours");
        }
        Ok(())
    }

    pub fn catalog_digest(&self) -> Result<Bytes32> {
        self.catalog_release.digest()
    }

    pub fn registry_digest(&self) -> Result<Bytes32> {
        Ok(self.registry_action.digest(&self.registry_domain)?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationApproval {
    pub schema: String,
    pub job_id: String,
    pub catalog: SignedCatalogRelease,
    pub registry: SignedRegistryActionV2,
    pub approved_at: String,
}

impl PublicationApproval {
    pub fn verify_for(&self, request: &PublicationApprovalRequest) -> Result<()> {
        if self.schema != PUBLICATION_APPROVAL_SCHEMA || self.job_id != request.job_id {
            return invalid("publication approval does not identify the exact request");
        }
        request.validate(cfg!(test))?;
        self.catalog.verify()?;
        self.registry.verify()?;
        if self.catalog.release != request.catalog_release
            || self.registry.domain != request.registry_domain
            || self.registry.action != request.registry_action
            || self.catalog.signer != request.builder_device.public_key
            || self.registry.signer != request.builder_device.public_key
        {
            return invalid("publication approval signs a different structured request");
        }
        parse_time("publication approved_at", &self.approved_at)?;
        Ok(())
    }
}

fn identifier(name: &str, value: &str, minimum: usize, maximum: usize) -> Result<()> {
    if !(minimum..=maximum).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return invalid(format!("{name} is not a bounded identifier"));
    }
    Ok(())
}

fn validate_deadline(deadline: u64, issued_at: &str, expires_at: &str) -> Result<()> {
    let issued = parse_time("publication issued_at", issued_at)?;
    let expires = parse_time("publication expires_at", expires_at)?;
    if deadline != expires.unix_timestamp() as u64 || deadline <= issued.unix_timestamp() as u64 {
        return invalid("Registry action deadline differs from the approval window");
    }
    Ok(())
}

fn parse_time(name: &str, value: &str) -> Result<time::OffsetDateTime> {
    time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        .map_err(|_| NetworkError::Invalid(format!("{name} is invalid")))
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(NetworkError::Invalid(message.into()))
}
