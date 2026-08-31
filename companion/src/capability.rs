//! Explicit, signed, revocable authority granted to one paired device.

use crate::canonical;
use crate::command::CompanionCommand;
use crate::crypto::{base64url, decode_array};
use crate::identity::{CompanionIdentity, TransportIdentity};
use crate::{
    require, validate_identifier, validate_window, CompanionError, Result, MAX_CLOCK_SKEW_SECONDS,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use time::OffsetDateTime;

pub const CAPABILITY_GRANT_SCHEMA: &str = "tohseno.companion-capability-grant/1";
pub const CAPABILITY_SIGNATURE_DOMAIN: &[u8] = b"tohseno.companion.capability-grant.v1";
const MAX_CAPABILITY_LIFETIME_SECONDS: i64 = 366 * 24 * 60 * 60;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum CapabilityAction {
    #[serde(rename = "workspace.read")]
    WorkspaceRead,
    #[serde(rename = "execution.read")]
    ExecutionRead,
    #[serde(rename = "feedback.write")]
    FeedbackWrite,
    #[serde(rename = "marketing.write")]
    MarketingWrite,
    #[serde(rename = "shot.create")]
    ShotCreate,
    #[serde(rename = "shot.evolve")]
    ShotEvolve,
    #[serde(rename = "publication.authorize")]
    PublicationAuthorize,
    #[serde(rename = "network.receive")]
    NetworkReceive,
    #[serde(rename = "preference.write")]
    PreferenceWrite,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityGrantBody {
    pub schema: String,
    pub capability_id: String,
    pub workspace_id: String,
    pub device_id: String,
    pub allowed_actions: Vec<CapabilityAction>,
    pub issued_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub revocation_epoch: u64,
    pub studio_signing_public_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityGrant {
    #[serde(flatten)]
    pub body: CapabilityGrantBody,
    pub signature: String,
}

impl CapabilityGrant {
    pub fn sign<I: TransportIdentity>(body: CapabilityGrantBody, studio: &I) -> Result<Self> {
        body.validate_shape()?;
        require(
            body.studio_signing_public_key == studio.signing_public_key_base64url(),
            "capability grant signing key does not match Studio",
        )?;
        let signature = studio.sign(CAPABILITY_SIGNATURE_DOMAIN, &canonical::to_vec(&body)?);
        Ok(Self {
            body,
            signature: base64url(&signature),
        })
    }

    pub fn verify(&self, trusted_studio_signing_key: &[u8; 32], now: OffsetDateTime) -> Result<()> {
        self.body.validate_shape()?;
        let embedded = decode_array::<32>(
            "capability Studio signing key",
            &self.body.studio_signing_public_key,
        )?;
        require(
            &embedded == trusted_studio_signing_key,
            "capability grant is not signed by the trusted Studio",
        )?;
        let issued = crate::parse_timestamp(&self.body.issued_at)?;
        if let Some(expires_at) = &self.body.expires_at {
            validate_window(
                &self.body.issued_at,
                expires_at,
                now,
                MAX_CAPABILITY_LIFETIME_SECONDS,
                MAX_CLOCK_SKEW_SECONDS,
            )?;
        } else {
            require(
                now >= issued - time::Duration::seconds(MAX_CLOCK_SKEW_SECONDS),
                "capability grant is not valid yet",
            )?;
        }
        let signature = decode_array::<64>("capability grant signature", &self.signature)?;
        CompanionIdentity::verify(
            trusted_studio_signing_key,
            CAPABILITY_SIGNATURE_DOMAIN,
            &canonical::to_vec(&self.body)?,
            &signature,
        )
    }
}

impl CapabilityGrantBody {
    fn validate_shape(&self) -> Result<()> {
        require(
            self.schema == CAPABILITY_GRANT_SCHEMA,
            "unsupported capability grant schema",
        )?;
        validate_identifier("capability ID", &self.capability_id)?;
        validate_identifier("workspace ID", &self.workspace_id)?;
        validate_identifier("device ID", &self.device_id)?;
        require(
            !self.allowed_actions.is_empty() && self.allowed_actions.len() <= 9,
            "capability grant must contain one to nine actions",
        )?;
        require(
            self.allowed_actions
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
            "capability actions must be unique and sorted",
        )?;
        crate::parse_timestamp(&self.issued_at)?;
        if let Some(expires_at) = &self.expires_at {
            crate::parse_timestamp(expires_at)?;
        }
        decode_array::<32>(
            "capability Studio signing key",
            &self.studio_signing_public_key,
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityRegistry {
    workspace_id: String,
    device_epochs: BTreeMap<String, u64>,
    revoked_capabilities: BTreeSet<String>,
}

impl CapabilityRegistry {
    pub fn new(workspace_id: impl Into<String>) -> Result<Self> {
        let workspace_id = workspace_id.into();
        validate_identifier("workspace ID", &workspace_id)?;
        Ok(Self {
            workspace_id,
            device_epochs: BTreeMap::new(),
            revoked_capabilities: BTreeSet::new(),
        })
    }

    pub fn current_epoch(&self, device_id: &str) -> u64 {
        self.device_epochs.get(device_id).copied().unwrap_or(0)
    }

    /// Restore the authoritative monotonic device epoch from durable local
    /// state before command admission.
    pub fn restore_device_epoch(&mut self, device_id: &str, epoch: u64) -> Result<()> {
        validate_identifier("device ID", device_id)?;
        require(
            epoch >= self.current_epoch(device_id),
            "revocation epoch cannot move backwards",
        )?;
        self.device_epochs.insert(device_id.into(), epoch);
        Ok(())
    }

    pub fn authorize(
        &self,
        grant: &CapabilityGrant,
        action: CapabilityAction,
        trusted_studio_signing_key: &[u8; 32],
        now: OffsetDateTime,
    ) -> Result<()> {
        grant.verify(trusted_studio_signing_key, now)?;
        require(
            grant.body.workspace_id == self.workspace_id,
            "capability grant belongs to a different workspace",
        )?;
        require(
            !self
                .revoked_capabilities
                .contains(&grant.body.capability_id),
            "capability grant was revoked",
        )?;
        require(
            grant.body.revocation_epoch == self.current_epoch(&grant.body.device_id),
            "capability grant has a stale revocation epoch",
        )?;
        require(
            grant.body.allowed_actions.binary_search(&action).is_ok(),
            "capability grant does not authorize this action",
        )
    }

    /// Verify both provenance layers and enforce revocation before a command
    /// may enter the durable application command journal.
    pub fn authorize_command(
        &self,
        grant: &CapabilityGrant,
        command: &CompanionCommand,
        companion_signing_public_key: &[u8; 32],
        trusted_studio_signing_key: &[u8; 32],
        now: OffsetDateTime,
    ) -> Result<()> {
        command.verify(companion_signing_public_key, &grant.body.device_id, now)?;
        require(
            command.body.workspace_id == self.workspace_id
                && command.body.workspace_id == grant.body.workspace_id,
            "companion command belongs to a different workspace",
        )?;
        require(
            command.body.capability_id == grant.body.capability_id,
            "companion command names a different capability grant",
        )?;
        self.authorize(
            grant,
            command.body.payload.required_capability(),
            trusted_studio_signing_key,
            now,
        )
    }

    pub fn revoke_device(&mut self, device_id: &str) -> Result<u64> {
        validate_identifier("device ID", device_id)?;
        let next = self
            .current_epoch(device_id)
            .checked_add(1)
            .ok_or_else(|| CompanionError::Invalid("revocation epoch overflowed".into()))?;
        self.device_epochs.insert(device_id.into(), next);
        Ok(next)
    }

    pub fn revoke_capability(&mut self, capability_id: &str) -> Result<()> {
        validate_identifier("capability ID", capability_id)?;
        self.revoked_capabilities.insert(capability_id.into());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandBody, CommandPayload, CompanionCommand};

    fn now() -> OffsetDateTime {
        crate::parse_timestamp("2026-08-15T12:01:00Z").unwrap()
    }

    fn grant(studio: &CompanionIdentity, device_id: &str, epoch: u64) -> CapabilityGrant {
        CapabilityGrant::sign(
            CapabilityGrantBody {
                schema: CAPABILITY_GRANT_SCHEMA.into(),
                capability_id: "cap_fixture".into(),
                workspace_id: "workspace_fixture".into(),
                device_id: device_id.into(),
                allowed_actions: vec![
                    CapabilityAction::WorkspaceRead,
                    CapabilityAction::FeedbackWrite,
                ],
                issued_at: "2026-08-15T12:00:00Z".into(),
                expires_at: Some("2026-08-15T13:00:00Z".into()),
                revocation_epoch: epoch,
                studio_signing_public_key: studio.signing_public_key_base64url(),
            },
            studio,
        )
        .unwrap()
    }

    #[test]
    fn action_must_be_explicitly_granted() {
        let (_, studio) = CompanionIdentity::from_entropy([20_u8; 16]).unwrap();
        let registry = CapabilityRegistry::new("workspace_fixture").unwrap();
        let grant = grant(&studio, "device_fixture", 0);
        registry
            .authorize(
                &grant,
                CapabilityAction::FeedbackWrite,
                &studio.signing_public_key(),
                now(),
            )
            .unwrap();
        assert!(registry
            .authorize(
                &grant,
                CapabilityAction::ShotEvolve,
                &studio.signing_public_key(),
                now(),
            )
            .is_err());
    }

    #[test]
    fn revocation_epoch_invalidates_every_old_grant_immediately() {
        let (_, studio) = CompanionIdentity::from_entropy([21_u8; 16]).unwrap();
        let mut registry = CapabilityRegistry::new("workspace_fixture").unwrap();
        let old = grant(&studio, "device_fixture", 0);
        registry.revoke_device("device_fixture").unwrap();
        assert!(registry
            .authorize(
                &old,
                CapabilityAction::WorkspaceRead,
                &studio.signing_public_key(),
                now(),
            )
            .is_err());
        let renewed = grant(&studio, "device_fixture", 1);
        registry
            .authorize(
                &renewed,
                CapabilityAction::WorkspaceRead,
                &studio.signing_public_key(),
                now(),
            )
            .unwrap();
    }

    #[test]
    fn signed_command_and_capability_are_checked_as_two_provenance_layers() {
        let (_, studio) = CompanionIdentity::from_entropy([22_u8; 16]).unwrap();
        let (_, phone) = CompanionIdentity::from_entropy([23_u8; 16]).unwrap();
        let grant = grant(&studio, phone.device_id(), 0);
        let command = CompanionCommand::sign(
            &phone,
            CommandBody {
                schema: String::new(),
                command_id: "command_fixture".into(),
                workspace_id: "workspace_fixture".into(),
                capability_id: "cap_fixture".into(),
                author_device_id: String::new(),
                created_at: "2026-08-15T12:00:00Z".into(),
                payload: CommandPayload::FeedbackSubmit {
                    shot_id: "shot_fixture".into(),
                    expression_id: "expression_fixture".into(),
                    version_id: "version_fixture".into(),
                    version_ordinal: 1,
                    body: "Exact-version feedback.".into(),
                },
            },
        )
        .unwrap();
        let mut registry = CapabilityRegistry::new("workspace_fixture").unwrap();
        registry
            .authorize_command(
                &grant,
                &command,
                &phone.signing_public_key(),
                &studio.signing_public_key(),
                now(),
            )
            .unwrap();
        registry.revoke_device(phone.device_id()).unwrap();
        assert!(registry
            .authorize_command(
                &grant,
                &command,
                &phone.signing_public_key(),
                &studio.signing_public_key(),
                now(),
            )
            .is_err());
    }

    #[test]
    fn capabilities_cannot_be_swapped_between_workspaces_or_commands() {
        let (_, studio) = CompanionIdentity::from_entropy([24_u8; 16]).unwrap();
        let (_, phone) = CompanionIdentity::from_entropy([25_u8; 16]).unwrap();
        let grant = grant(&studio, phone.device_id(), 0);
        let wrong_workspace = CapabilityRegistry::new("workspace_attacker").unwrap();
        assert!(wrong_workspace
            .authorize(
                &grant,
                CapabilityAction::FeedbackWrite,
                &studio.signing_public_key(),
                now(),
            )
            .is_err());

        let command = CompanionCommand::sign(
            &phone,
            CommandBody {
                schema: String::new(),
                command_id: "command_fixture".into(),
                workspace_id: "workspace_attacker".into(),
                capability_id: grant.body.capability_id.clone(),
                author_device_id: String::new(),
                created_at: "2026-08-15T12:00:00Z".into(),
                payload: CommandPayload::FeedbackSubmit {
                    shot_id: "shot_fixture".into(),
                    expression_id: "expression_fixture".into(),
                    version_id: "version_fixture".into(),
                    version_ordinal: 1,
                    body: "Cross-workspace substitution.".into(),
                },
            },
        )
        .unwrap();
        assert!(CapabilityRegistry::new("workspace_fixture")
            .unwrap()
            .authorize_command(
                &grant,
                &command,
                &phone.signing_public_key(),
                &studio.signing_public_key(),
                now(),
            )
            .is_err());
    }
}
