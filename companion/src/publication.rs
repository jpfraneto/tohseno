//! Private, bounded publication-review messages.
//!
//! The Mac sends canonical structured JSON, never an unexplained digest. The
//! Companion independently validates and digests those bytes before asking a
//! human to sign them with the separate Builder DeviceKey.

use crate::canonical;
use crate::{parse_timestamp, require, validate_identifier, validate_text, Result};
use serde::{Deserialize, Serialize};

pub const BUILDER_DEVICE_ANNOUNCEMENT_SCHEMA: &str = "tohseno.builder-device-announcement/1";
pub const PUBLICATION_APPROVAL_REQUEST_SCHEMA: &str = "tohseno.publication-approval-request/2";
pub const LEGACY_PUBLICATION_APPROVAL_REQUEST_SCHEMA: &str =
    "tohseno.publication-approval-request/1";
pub const PUBLICATION_SIGNATURE_SCHEMA: &str = "tohseno.builder-device-signature/1";
pub const ACTIVE_CHAIN_ID: u64 = 4663;
pub const ACTIVE_FACTORY: &str = "0xb1bd208cd2af98e701f43d06aaa889d3a594df65";
pub const ACTIVE_REGISTRY: &str = "0x3fe6508ba2660bc575080024f402c192a2e035a0";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderDeviceAnnouncement {
    pub schema: String,
    pub key_id: String,
    pub x: String,
    pub y: String,
    pub security_level: String,
    pub test_only: bool,
}

impl BuilderDeviceAnnouncement {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == BUILDER_DEVICE_ANNOUNCEMENT_SCHEMA,
            "unsupported Builder DeviceKey announcement schema",
        )?;
        hex32("Builder DeviceKey ID", &self.key_id)?;
        hex32("Builder DeviceKey x", &self.x)?;
        hex32("Builder DeviceKey y", &self.y)?;
        require(
            matches!(
                self.security_level.as_str(),
                "secure_enclave" | "software_test"
            ),
            "Builder DeviceKey security level is invalid",
        )?;
        require(
            self.test_only == (self.security_level == "software_test"),
            "Builder DeviceKey test marker disagrees with its security level",
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationApprovalRequest {
    pub schema: String,
    pub job_id: String,
    pub app_name: String,
    pub source_file_count: u64,
    pub source_byte_length: u64,
    pub install_allowed: bool,
    pub fork_allowed: bool,
    pub requested_route: String,
    pub chain_id: u64,
    pub builder_account_factory: String,
    pub shot_registry: String,
    pub builder_id: String,
    pub builder_device: BuilderDeviceAnnouncement,
    pub shot_id: String,
    pub checkpoint_sequence: u64,
    pub action_nonce: u64,
    pub action_deadline: u64,
    pub catalog_release_json: String,
    pub catalog_digest: String,
    pub registry_action_json: String,
    pub registry_digest: String,
    pub issued_at: String,
    pub expires_at: String,
    #[serde(default)]
    pub publication_kind: Option<String>,
    #[serde(default)]
    pub claim_edition: Option<ClaimEditionApprovalContext>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimEditionPolicySummary {
    pub kind: String,
    pub max_claims: u64,
    pub closes_at: u64,
}

impl ClaimEditionPolicySummary {
    pub fn validate(&self) -> Result<()> {
        require(
            self.max_claims <= 9_007_199_254_740_991 && self.closes_at <= 9_007_199_254_740_991,
            "Claim Edition bounds exceed the exact integer limit",
        )?;
        require(
            matches!(
                (self.kind.as_str(), self.max_claims, self.closes_at),
                ("open", 0, 0)
                    | ("limited", 1.., 0)
                    | ("timed", 0, 1..)
                    | ("limited_timed", 1.., 1..)
            ),
            "Claim Edition policy shape is invalid",
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimEditionApprovalContext {
    pub claims_contract: String,
    pub claims_activation_signing_digest: String,
    pub controller: String,
    pub edition_nonce: u64,
    pub action_deadline: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_policy: Option<ClaimEditionPolicySummary>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenClaimEditionApprovalAction {
    pub shot_registry: String,
    pub shot_id: String,
    pub max_claims: u64,
    pub closes_at: u64,
    pub controller: String,
    pub nonce: u64,
    pub deadline: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedClaimEdition {
    pub policy: ClaimEditionPolicySummary,
    pub action: OpenClaimEditionApprovalAction,
    pub digest: String,
    pub signature: BuilderDeviceSignature,
}

impl ApprovedClaimEdition {
    pub fn validate(&self) -> Result<()> {
        self.policy.validate()?;
        hex32("Claim Edition digest", &self.digest)?;
        self.signature.validate()?;
        require(
            self.signature.digest == self.digest
                && self.action.shot_registry == ACTIVE_REGISTRY
                && self.action.shot_id.len() == 66
                && self.action.controller.len() == 42
                && self.action.max_claims == self.policy.max_claims
                && self.action.closes_at == self.policy.closes_at
                && self.action.nonce <= 9_007_199_254_740_991
                && self.action.deadline > 0
                && self.action.deadline <= 9_007_199_254_740_991,
            "approved Claim Edition is invalid",
        )?;
        hex32("Claim Edition ShotID", &self.action.shot_id)
    }
}

impl ClaimEditionApprovalContext {
    fn validate(&self, request: &PublicationApprovalRequest) -> Result<()> {
        require(
            self.claims_contract.len() == 42
                && self.claims_contract.starts_with("0x")
                && is_hex(&self.claims_contract),
            "Claims contract address is invalid",
        )?;
        hex32(
            "Claims activation signing digest",
            &self.claims_activation_signing_digest,
        )?;
        require(
            self.controller == request.builder_id.rsplit(':').next().unwrap_or_default(),
            "Claim Edition controller differs from the exact BuilderAccount",
        )?;
        require(
            self.edition_nonce <= 9_007_199_254_740_991
                && self.action_deadline == request.action_deadline,
            "Claim Edition nonce or deadline is invalid",
        )?;
        if let Some(policy) = &self.requested_policy {
            policy.validate()?;
        }
        Ok(())
    }
}

impl PublicationApprovalRequest {
    pub fn validate(&self) -> Result<()> {
        let legacy = self.schema == LEGACY_PUBLICATION_APPROVAL_REQUEST_SCHEMA;
        require(
            legacy || self.schema == PUBLICATION_APPROVAL_REQUEST_SCHEMA,
            "unsupported publication approval request schema",
        )?;
        validate_identifier("publication job ID", &self.job_id)?;
        validate_text("publication app name", &self.app_name, 160)?;
        require(
            self.source_file_count > 0 && self.source_file_count <= 100_000,
            "publication source file count is invalid",
        )?;
        require(
            self.source_byte_length > 0 && self.source_byte_length <= 2 * 1024 * 1024 * 1024,
            "publication source size is invalid",
        )?;
        require(self.install_allowed, "publication must permit installation")?;
        require(
            !self.requested_route.is_empty()
                && self.requested_route.len() <= 160
                && self.requested_route.starts_with('/')
                && !self.requested_route.contains("..")
                && !self.requested_route.chars().any(char::is_control),
            "requested publication route is invalid",
        )?;
        require(
            self.chain_id == ACTIVE_CHAIN_ID
                && self.builder_account_factory == ACTIVE_FACTORY
                && self.shot_registry == ACTIVE_REGISTRY,
            "publication request does not use the active contract generation",
        )?;
        require(
            self.builder_id
                .strip_prefix("eip155:4663:")
                .is_some_and(|address| address.len() == 42 && is_hex(address)),
            "publication BuilderID is invalid",
        )?;
        self.builder_device.validate()?;
        hex32("publication ShotID", &self.shot_id)?;
        require(
            self.checkpoint_sequence > 0 && self.checkpoint_sequence <= 9_007_199_254_740_991,
            "publication checkpoint sequence is invalid",
        )?;
        require(
            self.action_nonce <= 9_007_199_254_740_991,
            "publication action nonce is invalid",
        )?;
        hex32("catalog digest", &self.catalog_digest)?;
        hex32("Registry digest", &self.registry_digest)?;
        canonical_json("catalog release", &self.catalog_release_json, 512 * 1024)?;
        canonical_json("Registry action", &self.registry_action_json, 64 * 1024)?;
        let issued = parse_timestamp(&self.issued_at)?;
        let expires = parse_timestamp(&self.expires_at)?;
        require(
            expires > issued && (expires - issued).whole_hours() <= 24,
            "publication approval lifetime is invalid",
        )?;
        require(
            self.action_deadline == expires.unix_timestamp() as u64,
            "Registry action deadline differs from approval expiry",
        )?;
        if legacy {
            require(
                self.publication_kind.is_none() && self.claim_edition.is_none(),
                "legacy publication approval cannot carry Claims fields",
            )?;
            return Ok(());
        }
        match (self.publication_kind.as_deref(), &self.claim_edition) {
            (Some("ship"), Some(context)) if self.checkpoint_sequence == 1 => {
                context.validate(self)
            }
            (Some("update"), None) if self.checkpoint_sequence >= 2 => Ok(()),
            _ => Err(crate::CompanionError::Invalid(
                "publication kind and immutable Claim Edition context disagree".into(),
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderDeviceSignature {
    pub schema: String,
    pub signer: BuilderDeviceAnnouncement,
    pub algorithm: String,
    pub digest: String,
    pub r: String,
    pub s: String,
    pub low_s: bool,
}

impl BuilderDeviceSignature {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == PUBLICATION_SIGNATURE_SCHEMA,
            "unsupported Builder DeviceKey signature schema",
        )?;
        self.signer.validate()?;
        require(
            self.algorithm == "p256",
            "publication signature must use P-256",
        )?;
        hex32("publication signature digest", &self.digest)?;
        hex32("publication signature r", &self.r)?;
        hex32("publication signature s", &self.s)?;
        require(self.low_s, "publication signature must be low-s")?;
        require(
            self.s.as_str() <= "0x7fffffff800000007fffffffffffffffde737d56d38bcf4279dce5617e3192a8",
            "publication signature s is not low-s",
        )
    }
}

fn canonical_json(label: &str, value: &str, maximum: usize) -> Result<()> {
    require(
        !value.is_empty() && value.len() <= maximum,
        format!("{label} exceeds its bound"),
    )?;
    let parsed: serde_json::Value = serde_json::from_str(value)?;
    require(
        matches!(parsed, serde_json::Value::Object(_)),
        format!("{label} must be a JSON object"),
    )?;
    require(
        canonical::to_vec(&parsed)? == value.as_bytes(),
        format!("{label} is not canonical JSON"),
    )
}

fn hex32(label: &str, value: &str) -> Result<()> {
    require(
        value.len() == 66 && is_hex(value),
        format!("{label} must be lowercase 0x-prefixed bytes32"),
    )
}

fn is_hex(value: &str) -> bool {
    value.strip_prefix("0x").is_some_and(|body| {
        body.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
