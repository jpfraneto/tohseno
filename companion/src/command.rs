//! Signed phone-to-Mac commands. Admission remains the Mac's responsibility.

use crate::canonical;
use crate::capability::CapabilityAction;
use crate::crypto::{base64url, decode_array, sha256};
use crate::identity::CompanionIdentity;
use crate::reference::MAX_REFERENCE_BLOB_BYTES;
use crate::{
    parse_timestamp, require, validate_identifier, validate_text, Result, MAX_CLOCK_SKEW_SECONDS,
    MAX_TEXT_BYTES,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const COMPANION_COMMAND_SCHEMA: &str = "tohseno.companion-command/1";
pub const COMMAND_SIGNATURE_DOMAIN: &[u8] = b"tohseno.companion.command-signature.v1";
pub const MAX_COMMAND_AGE_SECONDS: i64 = 30 * 24 * 60 * 60;
pub const MAX_REFERENCES: usize = 8;
pub const MAX_REFERENCE_BYTES: u64 = MAX_REFERENCE_BLOB_BYTES as u64;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceDescriptor {
    /// Opaque identifier for bytes delivered in an encrypted blob envelope.
    pub blob_id: String,
    pub origin_name: String,
    pub media_type: String,
    pub byte_length: u64,
    pub sha256: String,
}

impl ReferenceDescriptor {
    fn validate(&self) -> Result<()> {
        validate_identifier("reference blob ID", &self.blob_id)?;
        validate_text("reference origin name", &self.origin_name, 512)?;
        require(
            !self.origin_name.contains('/') && !self.origin_name.contains('\\'),
            "reference origin name must not contain a path",
        )?;
        require(
            matches!(self.media_type.as_str(), "image/png" | "image/jpeg"),
            "reference media type must be image/png or image/jpeg",
        )?;
        require(
            (1..=MAX_REFERENCE_BYTES).contains(&self.byte_length),
            "reference byte length is invalid",
        )?;
        decode_array::<32>("reference SHA-256", &self.sha256)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command_kind")]
pub enum CommandPayload {
    /// Request a complete authoritative snapshot after a retained event range
    /// can no longer be reconciled. The signed command wrapper supplies the
    /// idempotency key; this payload intentionally carries no relay cursor,
    /// because relay routing state is not workspace authority.
    #[serde(rename = "workspace.snapshot.request")]
    WorkspaceSnapshotRequest,
    #[serde(rename = "feedback.submit")]
    FeedbackSubmit {
        shot_id: String,
        expression_id: String,
        version_id: String,
        version_ordinal: u64,
        body: String,
    },
    #[serde(rename = "marketing.submit")]
    MarketingSubmit {
        note_id: String,
        shot_id: String,
        body: String,
    },
    #[serde(rename = "shot.evolve.request")]
    ShotEvolveRequest {
        shot_id: String,
        base_expression_id: String,
        base_version_id: String,
        base_version_ordinal: u64,
        intention: String,
        selected_feedback_action_commitments: Vec<String>,
        references: Vec<ReferenceDescriptor>,
    },
    /// Private evolution of an owner-adopted source project. The project and
    /// source-state identities are not public Shot lineage identifiers.
    #[serde(rename = "project.evolve.request")]
    ProjectEvolveRequest {
        project_id: String,
        base_source_state: String,
        intention: String,
        references: Vec<ReferenceDescriptor>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        follow_up_to: Option<String>,
    },
    #[serde(rename = "shot.create.request")]
    ShotCreateRequest {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suggested_name: Option<String>,
        intention: String,
        references: Vec<ReferenceDescriptor>,
    },
}

impl CommandPayload {
    pub fn required_capability(&self) -> CapabilityAction {
        match self {
            Self::WorkspaceSnapshotRequest => CapabilityAction::WorkspaceRead,
            Self::FeedbackSubmit { .. } => CapabilityAction::FeedbackWrite,
            Self::MarketingSubmit { .. } => CapabilityAction::MarketingWrite,
            Self::ShotEvolveRequest { .. } => CapabilityAction::ShotEvolve,
            Self::ProjectEvolveRequest { .. } => CapabilityAction::ShotEvolve,
            Self::ShotCreateRequest { .. } => CapabilityAction::ShotCreate,
        }
    }

    fn validate(&self) -> Result<()> {
        match self {
            Self::WorkspaceSnapshotRequest => Ok(()),
            Self::FeedbackSubmit {
                shot_id,
                expression_id,
                version_id,
                version_ordinal,
                body,
            } => {
                validate_identifier("feedback Shot ID", shot_id)?;
                validate_identifier("feedback Expression ID", expression_id)?;
                validate_identifier("feedback Version ID", version_id)?;
                require(
                    *version_ordinal > 0,
                    "feedback Version ordinal must be positive",
                )?;
                validate_text("feedback body", body, 256 * 1024)
            }
            Self::MarketingSubmit {
                note_id,
                shot_id,
                body,
            } => {
                validate_identifier("marketing note ID", note_id)?;
                validate_identifier("marketing Shot ID", shot_id)?;
                validate_text("marketing-note body", body, 256 * 1024)
            }
            Self::ShotEvolveRequest {
                shot_id,
                base_expression_id,
                base_version_id,
                base_version_ordinal,
                intention,
                selected_feedback_action_commitments,
                references,
            } => {
                validate_identifier("evolution Shot ID", shot_id)?;
                validate_identifier("base Expression ID", base_expression_id)?;
                validate_identifier("base Version ID", base_version_id)?;
                require(
                    *base_version_ordinal > 0,
                    "base Version ordinal must be positive",
                )?;
                validate_text("evolution intention", intention, MAX_TEXT_BYTES)?;
                require(
                    selected_feedback_action_commitments.len() <= 256,
                    "too many selected feedback actions",
                )?;
                for commitment in selected_feedback_action_commitments {
                    decode_array::<32>("feedback action commitment", commitment)?;
                }
                validate_references(references)
            }
            Self::ProjectEvolveRequest {
                project_id,
                base_source_state,
                intention,
                references,
                follow_up_to,
            } => {
                validate_identifier("project ID", project_id)?;
                validate_identifier("project source state", base_source_state)?;
                validate_text("project evolution intention", intention, MAX_TEXT_BYTES)?;
                if let Some(value) = follow_up_to {
                    validate_identifier("follow-up evolution ID", value)?;
                }
                validate_references(references)
            }
            Self::ShotCreateRequest {
                suggested_name,
                intention,
                references,
            } => {
                if let Some(name) = suggested_name {
                    validate_text("suggested Shot name", name, 256)?;
                }
                validate_text("creation intention", intention, MAX_TEXT_BYTES)?;
                validate_references(references)
            }
        }
    }
}

fn validate_references(references: &[ReferenceDescriptor]) -> Result<()> {
    require(
        references.len() <= MAX_REFERENCES,
        "a companion command may contain at most eight references",
    )?;
    for reference in references {
        reference.validate()?;
    }
    let mut blob_ids: Vec<_> = references.iter().map(|item| &item.blob_id).collect();
    blob_ids.sort();
    blob_ids.dedup();
    require(
        blob_ids.len() == references.len(),
        "reference blob IDs must be unique",
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandBody {
    pub schema: String,
    pub command_id: String,
    pub workspace_id: String,
    pub capability_id: String,
    pub author_device_id: String,
    pub created_at: String,
    pub payload: CommandPayload,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionCommand {
    #[serde(flatten)]
    pub body: CommandBody,
    pub signature: String,
}

impl CompanionCommand {
    pub fn sign(identity: &CompanionIdentity, mut body: CommandBody) -> Result<Self> {
        body.schema = COMPANION_COMMAND_SCHEMA.into();
        body.author_device_id = identity.device_id().into();
        body.validate_shape()?;
        let bytes = canonical::to_vec(&body)?;
        Ok(Self {
            body,
            signature: base64url(&identity.sign(COMMAND_SIGNATURE_DOMAIN, &bytes)),
        })
    }

    pub fn verify(
        &self,
        expected_signing_public_key: &[u8; 32],
        expected_device_id: &str,
        now: OffsetDateTime,
    ) -> Result<()> {
        self.body.validate_shape()?;
        require(
            self.body.author_device_id == expected_device_id,
            "command author is not the expected paired device",
        )?;
        let created_at = parse_timestamp(&self.body.created_at)?;
        require(
            now >= created_at - time::Duration::seconds(MAX_CLOCK_SKEW_SECONDS),
            "command was created too far in the future",
        )?;
        require(
            now <= created_at + time::Duration::seconds(MAX_COMMAND_AGE_SECONDS),
            "command is older than the offline-admission limit",
        )?;
        let signature = decode_array::<64>("companion command signature", &self.signature)?;
        CompanionIdentity::verify(
            expected_signing_public_key,
            COMMAND_SIGNATURE_DOMAIN,
            &canonical::to_vec(&self.body)?,
            &signature,
        )
    }

    pub fn payload_digest(&self) -> Result<[u8; 32]> {
        Ok(sha256(&canonical::to_vec(&self.body)?))
    }
}

impl CommandBody {
    fn validate_shape(&self) -> Result<()> {
        require(
            self.schema == COMPANION_COMMAND_SCHEMA,
            "unsupported companion command schema",
        )?;
        validate_identifier("command ID", &self.command_id)?;
        validate_identifier("workspace ID", &self.workspace_id)?;
        validate_identifier("capability ID", &self.capability_id)?;
        validate_identifier("author device ID", &self.author_device_id)?;
        parse_timestamp(&self.created_at)?;
        self.payload.validate()
    }
}

/// Append-only private Shot note derived from a verified companion command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketingNoteRecord {
    pub schema: String,
    pub note_id: String,
    pub command_id: String,
    pub shot_id: String,
    pub body: String,
    pub created_at: String,
    pub author_device_id: String,
    pub companion_command_digest: String,
    pub companion_signature: String,
}

impl MarketingNoteRecord {
    pub fn from_verified_command(
        command: &CompanionCommand,
        expected_signing_public_key: &[u8; 32],
        expected_device_id: &str,
        now: OffsetDateTime,
    ) -> Result<Self> {
        command.verify(expected_signing_public_key, expected_device_id, now)?;
        let CommandPayload::MarketingSubmit {
            note_id,
            shot_id,
            body,
        } = &command.body.payload
        else {
            return Err(crate::CompanionError::Invalid(
                "only marketing.submit can become a marketing note".into(),
            ));
        };
        let record = Self {
            schema: "tohseno.marketing-note/1".into(),
            note_id: note_id.clone(),
            command_id: command.body.command_id.clone(),
            shot_id: shot_id.clone(),
            body: body.clone(),
            created_at: command.body.created_at.clone(),
            author_device_id: command.body.author_device_id.clone(),
            companion_command_digest: base64url(&command.payload_digest()?),
            companion_signature: command.signature.clone(),
        };
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == "tohseno.marketing-note/1",
            "unsupported private marketing-note schema",
        )?;
        validate_identifier("marketing note ID", &self.note_id)?;
        validate_identifier("marketing command ID", &self.command_id)?;
        validate_identifier("marketing Shot ID", &self.shot_id)?;
        validate_text("marketing-note body", &self.body, 256 * 1024)?;
        parse_timestamp(&self.created_at)?;
        validate_identifier("marketing author device ID", &self.author_device_id)?;
        decode_array::<32>(
            "marketing companion command digest",
            &self.companion_command_digest,
        )?;
        decode_array::<64>("marketing companion signature", &self.companion_signature)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptState {
    Received,
    Accepted,
    Completed,
    Rejected,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandReceipt {
    pub schema: String,
    pub command_id: String,
    pub state: ReceiptState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection_code: Option<String>,
}

impl CommandReceipt {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == "tohseno.companion-command-receipt/1",
            "unsupported command receipt schema",
        )?;
        validate_identifier("command ID", &self.command_id)?;
        for (label, value) in [
            ("receipt Shot ID", &self.shot_id),
            ("receipt execution ID", &self.execution_id),
            ("receipt result ID", &self.result_id),
            ("receipt rejection code", &self.rejection_code),
        ] {
            if let Some(value) = value {
                validate_identifier(label, value)?;
            }
        }
        require(
            matches!(self.state, ReceiptState::Rejected) || self.rejection_code.is_none(),
            "only a rejected receipt may contain a rejection code",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_command(identity: &CompanionIdentity) -> CompanionCommand {
        CompanionCommand::sign(
            identity,
            CommandBody {
                schema: String::new(),
                command_id: "command_fixture".into(),
                workspace_id: "workspace_fixture".into(),
                capability_id: "capability_fixture".into(),
                author_device_id: String::new(),
                created_at: "2026-08-15T12:00:00Z".into(),
                payload: CommandPayload::FeedbackSubmit {
                    shot_id: "shot_fixture".into(),
                    expression_id: "expression_fixture".into(),
                    version_id: "version_fixture".into(),
                    version_ordinal: 3,
                    body: "The exact reviewed version needs a clearer button.".into(),
                },
            },
        )
        .unwrap()
    }

    #[test]
    fn signed_exact_version_feedback_verifies() {
        let (_, identity) = CompanionIdentity::from_entropy([40_u8; 16]).unwrap();
        let command = fixture_command(&identity);
        command
            .verify(
                &identity.signing_public_key(),
                identity.device_id(),
                crate::parse_timestamp("2026-08-15T12:01:00Z").unwrap(),
            )
            .unwrap();
        assert_eq!(
            command.body.payload.required_capability(),
            CapabilityAction::FeedbackWrite
        );
    }

    #[test]
    fn snapshot_fallback_has_an_exact_empty_payload_and_requires_read_access() {
        let (_, identity) = CompanionIdentity::from_entropy([45_u8; 16]).unwrap();
        let command = CompanionCommand::sign(
            &identity,
            CommandBody {
                schema: String::new(),
                command_id: "command_snapshot_fallback".into(),
                workspace_id: "workspace_fixture".into(),
                capability_id: "capability_fixture".into(),
                author_device_id: String::new(),
                created_at: "2026-08-15T12:00:00Z".into(),
                payload: CommandPayload::WorkspaceSnapshotRequest,
            },
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&command.body.payload).unwrap(),
            serde_json::json!({"command_kind": "workspace.snapshot.request"})
        );
        assert_eq!(
            command.body.payload.required_capability(),
            CapabilityAction::WorkspaceRead
        );
    }

    #[test]
    fn tamper_wrong_device_and_stale_command_are_rejected() {
        let (_, identity) = CompanionIdentity::from_entropy([41_u8; 16]).unwrap();
        let mut command = fixture_command(&identity);
        if let CommandPayload::FeedbackSubmit { body, .. } = &mut command.body.payload {
            body.push('!');
        }
        assert!(command
            .verify(
                &identity.signing_public_key(),
                identity.device_id(),
                crate::parse_timestamp("2026-08-15T12:01:00Z").unwrap(),
            )
            .is_err());
        let command = fixture_command(&identity);
        assert!(command
            .verify(
                &identity.signing_public_key(),
                "device_attacker",
                crate::parse_timestamp("2026-08-15T12:01:00Z").unwrap(),
            )
            .is_err());
        assert!(command
            .verify(
                &identity.signing_public_key(),
                identity.device_id(),
                crate::parse_timestamp("2026-10-01T12:01:00Z").unwrap(),
            )
            .is_err());
    }

    #[test]
    fn more_than_eight_references_are_rejected() {
        let (_, identity) = CompanionIdentity::from_entropy([42_u8; 16]).unwrap();
        let reference = ReferenceDescriptor {
            blob_id: "blob_fixture".into(),
            origin_name: "reference.png".into(),
            media_type: "image/png".into(),
            byte_length: 10,
            sha256: base64url(&[1_u8; 32]),
        };
        let result = CompanionCommand::sign(
            &identity,
            CommandBody {
                schema: String::new(),
                command_id: "command_too_many".into(),
                workspace_id: "workspace_fixture".into(),
                capability_id: "capability_fixture".into(),
                author_device_id: String::new(),
                created_at: "2026-08-15T12:00:00Z".into(),
                payload: CommandPayload::ShotCreateRequest {
                    suggested_name: None,
                    intention: "Make the app.".into(),
                    references: vec![reference; 9],
                },
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn malformed_signature_encoding_is_rejected() {
        let (_, identity) = CompanionIdentity::from_entropy([43_u8; 16]).unwrap();
        let mut command = fixture_command(&identity);
        command.signature = "not+base64url".into();
        assert!(matches!(
            command.verify(
                &identity.signing_public_key(),
                identity.device_id(),
                crate::parse_timestamp("2026-08-15T12:01:00Z").unwrap(),
            ),
            Err(crate::CompanionError::Invalid(_))
        ));
    }

    #[test]
    fn marketing_note_preserves_verified_private_provenance() {
        let (_, identity) = CompanionIdentity::from_entropy([44_u8; 16]).unwrap();
        let command = CompanionCommand::sign(
            &identity,
            CommandBody {
                schema: String::new(),
                command_id: "command_marketing".into(),
                workspace_id: "workspace_fixture".into(),
                capability_id: "capability_fixture".into(),
                author_device_id: String::new(),
                created_at: "2026-08-15T12:00:00Z".into(),
                payload: CommandPayload::MarketingSubmit {
                    note_id: "note_fixture".into(),
                    shot_id: "shot_fixture".into(),
                    body: "Private launch idea.".into(),
                },
            },
        )
        .unwrap();
        let note = MarketingNoteRecord::from_verified_command(
            &command,
            &identity.signing_public_key(),
            identity.device_id(),
            crate::parse_timestamp("2026-08-15T12:01:00Z").unwrap(),
        )
        .unwrap();
        assert_eq!(note.schema, "tohseno.marketing-note/1");
        assert_eq!(note.companion_signature, command.signature);
        assert_eq!(
            note.companion_command_digest,
            base64url(&command.payload_digest().unwrap())
        );
    }
}
