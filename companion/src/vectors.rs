//! Deterministic cross-language vectors consumed by Rust and Apple clients.

use crate::canonical;
use crate::capability::{
    CapabilityAction, CapabilityGrant, CapabilityGrantBody, CAPABILITY_GRANT_SCHEMA,
};
use crate::command::{CommandBody, CommandPayload, CompanionCommand, COMPANION_COMMAND_SCHEMA};
use crate::crypto::base64url;
use crate::envelope::{seal_with_material, EnvelopeMetadata, OpaqueEnvelope};
use crate::icon::IconBlob;
use crate::identity::{derive_key_material, CompanionIdentity, WorkspaceServiceIdentity};
use crate::pairing::{
    EncryptedPairingResponse, PairingAcceptance, PairingInvitation, PairingProof,
    PairingResponseBody, PairingSessionStore, PAIRING_ACCEPTANCE_SCHEMA,
    PAIRING_RESPONSE_BODY_SCHEMA,
};
use crate::reference::{ReferenceBlob, ReferenceBlobChunk};
use crate::relay_client::{
    capability_verifier, CursorEnvelope, EnvelopeAccepted, MailboxAck, MailboxAcknowledged,
    MailboxCreate, MailboxCreated, MailboxPage, MailboxResetRequired, MailboxRevoked,
    PairingResponseAccepted, PairingSessionCreate, PairingSessionCreated, PushRegister,
    PushUnregister, RelayHealth,
};
use crate::Result;
use bip39::{Language, Mnemonic};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zeroize::Zeroizing;

pub const SHARED_VECTOR_SCHEMA: &str = "tohseno.companion-test-vectors/1";
const VECTOR_PAIRING_SESSION_ID: &str = "0123456789abcdef0123456789abcdef";
const VECTOR_MAILBOX_ID: &str = "abcdef0123456789abcdef0123456789";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bip39OfficialVector {
    pub entropy_hex: String,
    pub mnemonic: String,
    pub passphrase: String,
    pub seed_hex: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityVector {
    pub entropy_base64url: String,
    pub mnemonic: String,
    pub seed_base64url: String,
    pub signing_secret_key_base64url: String,
    pub signing_public_key_base64url: String,
    pub agreement_secret_key_base64url: String,
    pub agreement_public_key_base64url: String,
    pub storage_key_base64url: String,
    pub device_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceServiceIdentityVector {
    pub signing_secret_key_base64url: String,
    pub signing_public_key_base64url: String,
    pub agreement_secret_key_base64url: String,
    pub agreement_public_key_base64url: String,
    pub device_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingVector {
    pub studio_signing_public_key_base64url: String,
    pub studio_ephemeral_secret_key_base64url: String,
    pub invitation_body_canonical_base64url: String,
    pub invitation: PairingInvitation,
    pub invitation_uri: String,
    pub proof_body_canonical_base64url: String,
    pub proof: PairingProof,
    pub response_body_canonical_base64url: String,
    pub response_body: PairingResponseBody,
    pub encrypted_response_canonical_base64url: String,
    pub encrypted_response: EncryptedPairingResponse,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityVector {
    pub body_canonical_base64url: String,
    pub grant: CapabilityGrant,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingAcceptanceVector {
    pub canonical_base64url: String,
    pub acceptance: PairingAcceptance,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandVector {
    pub body_canonical_base64url: String,
    pub command: CompanionCommand,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeVector {
    pub plaintext_base64url: String,
    pub header_canonical_base64url: String,
    pub unsigned_canonical_base64url: String,
    pub envelope: OpaqueEnvelope,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IconBlobVector {
    pub canonical_base64url: String,
    pub blob: IconBlob,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceBlobVector {
    pub canonical_base64url: String,
    pub blob: ReferenceBlob,
    pub chunk_canonical_base64url: Vec<String>,
    pub chunks: Vec<ReferenceBlobChunk>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelayVector {
    pub pairing_create: PairingSessionCreate,
    pub pairing_created: PairingSessionCreated,
    pub pairing_response_accepted: PairingResponseAccepted,
    pub mailbox_create: MailboxCreate,
    pub mailbox_created: MailboxCreated,
    /// The upload body is the direct outer envelope, without a wrapper.
    pub direct_envelope: OpaqueEnvelope,
    pub envelope_accepted: EnvelopeAccepted,
    pub mailbox_page: MailboxPage,
    pub mailbox_reset_required: MailboxResetRequired,
    pub mailbox_ack: MailboxAck,
    pub mailbox_acknowledged: MailboxAcknowledged,
    pub mailbox_revoked: MailboxRevoked,
    pub push_register: PushRegister,
    pub push_unregister: PushUnregister,
    pub health: RelayHealth,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeVector {
    pub name: String,
    pub target: String,
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_pointer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<Value>,
    pub expected_rejection: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedVectors {
    pub schema: String,
    pub test_only: bool,
    pub bip39_official: Bip39OfficialVector,
    pub companion_identity: IdentityVector,
    pub workspace_service_identity: WorkspaceServiceIdentityVector,
    pub pairing: PairingVector,
    pub capability: CapabilityVector,
    pub pairing_acceptance: PairingAcceptanceVector,
    pub command: CommandVector,
    pub snapshot_request_command: CommandVector,
    pub icon_blob: IconBlobVector,
    pub reference_blob: ReferenceBlobVector,
    pub envelope: EnvelopeVector,
    pub relay: RelayVector,
    pub negative: Vec<NegativeVector>,
}

#[derive(Serialize)]
struct UnsignedEnvelope<'a> {
    #[serde(flatten)]
    header: &'a crate::envelope::EnvelopeHeader,
    ciphertext: &'a str,
}

/// Produce the source of truth for `test-vectors/companion-v1.json`.
pub fn deterministic_vectors() -> Result<SharedVectors> {
    let entropy = [0_u8; 16];
    let (phrase, companion) = CompanionIdentity::from_entropy(entropy)?;
    let mnemonic = Mnemonic::parse_in_normalized(Language::English, phrase.expose())
        .map_err(|_| crate::CompanionError::Invalid("fixture mnemonic failed to parse".into()))?;
    let seed = Zeroizing::new(mnemonic.to_seed_normalized(""));
    let material = derive_key_material(seed.as_ref())?;

    let official_seed = Zeroizing::new(mnemonic.to_seed_normalized("TREZOR"));
    let bip39_official = Bip39OfficialVector {
        entropy_hex: hex(&entropy),
        mnemonic: phrase.expose().into(),
        passphrase: "TREZOR".into(),
        seed_hex: hex(official_seed.as_ref()),
    };
    let companion_identity = IdentityVector {
        entropy_base64url: base64url(&entropy),
        mnemonic: phrase.expose().into(),
        seed_base64url: base64url(seed.as_ref()),
        signing_secret_key_base64url: base64url(material.signing_secret.as_ref()),
        signing_public_key_base64url: companion.signing_public_key_base64url(),
        agreement_secret_key_base64url: base64url(material.agreement_secret.as_ref()),
        agreement_public_key_base64url: companion.agreement_public_key_base64url(),
        storage_key_base64url: base64url(material.storage_key.as_ref()),
        device_id: companion.device_id().into(),
    };

    let studio_signing_secret = [1_u8; 32];
    let studio_agreement_secret = [2_u8; 32];
    let studio =
        WorkspaceServiceIdentity::from_secret_keys(studio_signing_secret, studio_agreement_secret)?;
    let workspace_service_identity = WorkspaceServiceIdentityVector {
        signing_secret_key_base64url: base64url(&studio_signing_secret),
        signing_public_key_base64url: studio.signing_public_key_base64url(),
        agreement_secret_key_base64url: base64url(&studio_agreement_secret),
        agreement_public_key_base64url: studio.agreement_public_key_base64url(),
        device_id: studio.device_id().into(),
    };
    let studio_ephemeral_secret = [9_u8; 32];
    let mut sessions = PairingSessionStore::default();
    let invitation = sessions.insert_with_secret(
        VECTOR_PAIRING_SESSION_ID.into(),
        "workspace_vector_001",
        studio.device_id(),
        "official-v1",
        "2026-08-15T12:00:00Z",
        "2026-08-15T12:02:00Z",
        &studio,
        studio_ephemeral_secret,
    )?;
    let proof = PairingProof::create(
        &invitation,
        &companion,
        "Vector iPhone",
        "2026-08-15T12:01:00Z",
    )?;
    let response_body = PairingResponseBody {
        schema: PAIRING_RESPONSE_BODY_SCHEMA.into(),
        proof: proof.clone(),
        response_mailbox_id: "fedcba9876543210fedcba9876543210".into(),
        response_mailbox_write_capability: base64url(&[12_u8; 32]),
        response_mailbox_revoke_capability: base64url(&[13_u8; 32]),
    };
    let encrypted_response = EncryptedPairingResponse::seal_with_material(
        &invitation,
        response_body.clone(),
        [10_u8; 32],
        [11_u8; 12],
    )?;
    let pairing = PairingVector {
        studio_signing_public_key_base64url: studio.signing_public_key_base64url(),
        studio_ephemeral_secret_key_base64url: base64url(&studio_ephemeral_secret),
        invitation_body_canonical_base64url: base64url(&canonical::to_vec(&invitation.body)?),
        invitation_uri: invitation.to_uri()?,
        invitation,
        proof_body_canonical_base64url: base64url(&canonical::to_vec(&proof.body)?),
        proof,
        response_body_canonical_base64url: base64url(&canonical::to_vec(&response_body)?),
        response_body,
        encrypted_response_canonical_base64url: base64url(&canonical::to_vec(&encrypted_response)?),
        encrypted_response,
    };

    let grant = CapabilityGrant::sign(
        CapabilityGrantBody {
            schema: CAPABILITY_GRANT_SCHEMA.into(),
            capability_id: "capability_vector_001".into(),
            workspace_id: "workspace_vector_001".into(),
            device_id: companion.device_id().into(),
            allowed_actions: vec![
                CapabilityAction::WorkspaceRead,
                CapabilityAction::ExecutionRead,
                CapabilityAction::FeedbackWrite,
                CapabilityAction::MarketingWrite,
                CapabilityAction::ShotCreate,
                CapabilityAction::ShotEvolve,
                CapabilityAction::PublicationAuthorize,
                CapabilityAction::NetworkReceive,
            ],
            issued_at: "2026-08-15T12:01:01Z".into(),
            expires_at: Some("2026-08-16T12:01:01Z".into()),
            revocation_epoch: 0,
            studio_signing_public_key: studio.signing_public_key_base64url(),
        },
        &studio,
    )?;
    let capability = CapabilityVector {
        body_canonical_base64url: base64url(&canonical::to_vec(&grant.body)?),
        grant,
    };
    let acceptance = PairingAcceptance {
        schema: PAIRING_ACCEPTANCE_SCHEMA.into(),
        capability_grant: capability.grant.clone(),
        studio_agreement_public_key: studio.agreement_public_key_base64url(),
        command_mailbox_id: "00112233445566778899aabbccddeeff".into(),
        command_mailbox_write_capability: base64url(&[14_u8; 32]),
    };
    let pairing_acceptance = PairingAcceptanceVector {
        canonical_base64url: base64url(&canonical::to_vec(&acceptance)?),
        acceptance,
    };

    let command = CompanionCommand::sign(
        &companion,
        CommandBody {
            schema: COMPANION_COMMAND_SCHEMA.into(),
            command_id: "command_vector_001".into(),
            workspace_id: "workspace_vector_001".into(),
            capability_id: "capability_vector_001".into(),
            author_device_id: companion.device_id().into(),
            created_at: "2026-08-15T12:01:02Z".into(),
            payload: CommandPayload::FeedbackSubmit {
                shot_id: "shot_vector_001".into(),
                expression_id: "expression_vector_001".into(),
                version_id: "version_vector_003".into(),
                version_ordinal: 3,
                body: "Make the accepted version's primary action clearer.".into(),
            },
        },
    )?;
    let command = CommandVector {
        body_canonical_base64url: base64url(&canonical::to_vec(&command.body)?),
        command,
    };
    let snapshot_request_command = CompanionCommand::sign(
        &companion,
        CommandBody {
            schema: COMPANION_COMMAND_SCHEMA.into(),
            command_id: "command_snapshot_request_001".into(),
            workspace_id: "workspace_vector_001".into(),
            capability_id: "capability_vector_001".into(),
            author_device_id: companion.device_id().into(),
            created_at: "2026-08-15T12:01:04Z".into(),
            payload: CommandPayload::WorkspaceSnapshotRequest,
        },
    )?;
    let snapshot_request_command = CommandVector {
        body_canonical_base64url: base64url(&canonical::to_vec(&snapshot_request_command.body)?),
        command: snapshot_request_command,
    };

    let icon_blob = IconBlob::new(
        "icon_vector_001",
        7,
        "image/png",
        false,
        &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52,
            0, 0, 0, 1, 0, 0, 0, 1, 8, 4, 0, 0, 0, 0xb5, 0x1c, 0x0c, 2, 0, 0, 0, 0x0b, 0x49, 0x44,
            0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0, 1, 5, 1, 1, 0x27, 0x18, 0xe3, 0x66,
            0, 0, 0, 0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ],
    )?;
    let icon_blob = IconBlobVector {
        canonical_base64url: base64url(&canonical::to_vec(&icon_blob)?),
        blob: icon_blob,
    };

    let reference_blob = ReferenceBlob::new(
        "reference_vector_001",
        "reference.png",
        "image/png",
        &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52,
            0, 0, 0, 1, 0, 0, 0, 1, 8, 4, 0, 0, 0, 0xb5, 0x1c, 0x0c, 2, 0, 0, 0, 0x0b, 0x49, 0x44,
            0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0, 1, 5, 1, 1, 0x27, 0x18, 0xe3, 0x66,
            0, 0, 0, 0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ],
    )?;
    let reference_chunks = reference_blob.chunks()?;
    let reference_blob = ReferenceBlobVector {
        canonical_base64url: base64url(&canonical::to_vec(&reference_blob)?),
        blob: reference_blob,
        chunk_canonical_base64url: reference_chunks
            .iter()
            .map(|chunk| canonical::to_vec(chunk).map(|bytes| base64url(&bytes)))
            .collect::<Result<Vec<_>>>()?,
        chunks: reference_chunks,
    };

    let plaintext = canonical::to_vec(&serde_json::json!({
        "fixture": "private companion bytes"
    }))?;
    let envelope = seal_with_material(
        &companion,
        &studio.agreement_public_key(),
        EnvelopeMetadata {
            envelope_id: "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee".into(),
            mailbox_id: VECTOR_MAILBOX_ID.into(),
            recipient_device_id: studio.device_id().into(),
            sender_sequence: 42,
            created_at: "2026-08-15T12:01:03Z".into(),
            expires_at: "2026-08-16T12:01:03Z".into(),
        },
        &plaintext,
        [31_u8; 32],
        [32_u8; 12],
    )?;
    let unsigned = UnsignedEnvelope {
        header: &envelope.header,
        ciphertext: &envelope.ciphertext,
    };
    let envelope = EnvelopeVector {
        plaintext_base64url: base64url(&plaintext),
        header_canonical_base64url: base64url(&canonical::to_vec(&envelope.header)?),
        unsigned_canonical_base64url: base64url(&canonical::to_vec(&unsigned)?),
        envelope,
    };

    let relay = relay_vectors(&envelope)?;

    let negative = negative_vectors(&pairing, &envelope, &icon_blob, &reference_blob);
    Ok(SharedVectors {
        schema: SHARED_VECTOR_SCHEMA.into(),
        test_only: true,
        bip39_official,
        companion_identity,
        workspace_service_identity,
        pairing,
        capability,
        pairing_acceptance,
        command,
        snapshot_request_command,
        icon_blob,
        reference_blob,
        envelope,
        relay,
        negative,
    })
}

fn relay_vectors(envelope: &EnvelopeVector) -> Result<RelayVector> {
    let verifier = |byte| capability_verifier(&base64url(&[byte; 32]));
    Ok(RelayVector {
        pairing_create: PairingSessionCreate {
            schema: "tohseno.companion-pairing-session-create/1".into(),
            expires_at: "2026-08-15T12:02:00Z".into(),
            read_verifier: verifier(50)?,
            cancel_verifier: verifier(51)?,
        },
        pairing_created: PairingSessionCreated {
            schema: "tohseno.companion-pairing-session-created/1".into(),
            session_id: VECTOR_PAIRING_SESSION_ID.into(),
            expires_at: "2026-08-15T12:02:00Z".into(),
        },
        pairing_response_accepted: PairingResponseAccepted {
            schema: "tohseno.companion-pairing-response-accepted/1".into(),
            accepted: true,
            duplicate: false,
        },
        mailbox_create: MailboxCreate {
            schema: "tohseno.companion-mailbox-create/1".into(),
            write_verifier: verifier(52)?,
            read_verifier: verifier(53)?,
            ack_verifier: verifier(54)?,
            revoke_verifier: verifier(55)?,
            push_verifier: verifier(56)?,
        },
        mailbox_created: MailboxCreated {
            schema: "tohseno.companion-mailbox-created/1".into(),
            mailbox_id: VECTOR_MAILBOX_ID.into(),
            created_at: "2026-08-15T12:00:00Z".into(),
        },
        direct_envelope: envelope.envelope.clone(),
        envelope_accepted: EnvelopeAccepted {
            schema: "tohseno.companion-envelope-accepted/1".into(),
            accepted: true,
            duplicate: false,
            cursor: 7,
        },
        mailbox_page: MailboxPage {
            schema: "tohseno.companion-mailbox-page/1".into(),
            envelopes: vec![CursorEnvelope {
                cursor: 7,
                envelope: envelope.envelope.clone(),
            }],
            next_cursor: 7,
            head_cursor: 7,
            has_more: false,
        },
        mailbox_reset_required: MailboxResetRequired {
            schema: "tohseno.companion-mailbox-reset-required/1".into(),
            reset_required: true,
            reset_before_cursor: 4,
            head_cursor: 7,
        },
        mailbox_ack: MailboxAck {
            schema: "tohseno.companion-mailbox-ack/1".into(),
            cursor: 7,
        },
        mailbox_acknowledged: MailboxAcknowledged {
            schema: "tohseno.companion-mailbox-acknowledged/1".into(),
            acknowledged_cursor: 7,
        },
        mailbox_revoked: MailboxRevoked {
            schema: "tohseno.companion-mailbox-revoked/1".into(),
            revoked: true,
            revocation_epoch: 1,
        },
        push_register: PushRegister {
            schema: "tohseno.companion-push-register/1".into(),
            mailbox_id: VECTOR_MAILBOX_ID.into(),
            device_id: "device_fixture_for_push".into(),
            apns_token: "test-only-apns-token".into(),
        },
        push_unregister: PushUnregister {
            schema: "tohseno.companion-push-unregister/1".into(),
            mailbox_id: VECTOR_MAILBOX_ID.into(),
        },
        health: RelayHealth {
            schema: "tohseno.companion-relay-health/1".into(),
            service_version: "0.9.0".into(),
            ready: true,
            push_enabled: false,
            maximum_envelope_bytes: 16 * 1024 * 1024 + 16,
            retention_seconds: 7 * 24 * 60 * 60,
        },
    })
}

fn negative_vectors(
    pairing: &PairingVector,
    envelope: &EnvelopeVector,
    icon_blob: &IconBlobVector,
    reference_blob: &ReferenceBlobVector,
) -> Vec<NegativeVector> {
    let proof_confirmation = corrupted(&pairing.proof.key_confirmation);
    let pairing_response_ciphertext = corrupted(&pairing.encrypted_response.ciphertext);
    let envelope_ciphertext = corrupted(&envelope.envelope.ciphertext);
    let icon_bytes = corrupted(&icon_blob.blob.bytes);
    let reference_bytes = corrupted(&reference_blob.blob.bytes);
    let reference_chunk_bytes = corrupted(&reference_blob.chunks[0].bytes);

    vec![
        NegativeVector {
            name: "invitation_signature_tamper".into(),
            target: "pairing.invitation".into(),
            operation: "replace_json_value".into(),
            json_pointer: Some("/workspace_id".into()),
            replacement: Some(Value::String("workspace_attacker".into())),
            expected_rejection: "signature".into(),
        },
        NegativeVector {
            name: "invitation_unallowlisted_relay".into(),
            target: "pairing.invitation".into(),
            operation: "replace_json_value".into(),
            json_pointer: Some("/relay_id".into()),
            replacement: Some(Value::String("attacker-relay".into())),
            expected_rejection: "relay_allowlist".into(),
        },
        NegativeVector {
            name: "pairing_key_confirmation_tamper".into(),
            target: "pairing.proof".into(),
            operation: "replace_json_value".into(),
            json_pointer: Some("/key_confirmation".into()),
            replacement: Some(Value::String(proof_confirmation)),
            expected_rejection: "key_confirmation".into(),
        },
        NegativeVector {
            name: "pairing_response_ciphertext_tamper".into(),
            target: "pairing.encrypted_response".into(),
            operation: "replace_json_value".into(),
            json_pointer: Some("/ciphertext".into()),
            replacement: Some(Value::String(pairing_response_ciphertext)),
            expected_rejection: "authentication".into(),
        },
        NegativeVector {
            name: "capability_signature_tamper".into(),
            target: "capability.grant".into(),
            operation: "replace_json_value".into(),
            json_pointer: Some("/revocation_epoch".into()),
            replacement: Some(Value::Number(1_u64.into())),
            expected_rejection: "signature".into(),
        },
        NegativeVector {
            name: "command_signature_tamper".into(),
            target: "command".into(),
            operation: "replace_json_value".into(),
            json_pointer: Some("/payload/body".into()),
            replacement: Some(Value::String("relay forged plaintext".into())),
            expected_rejection: "signature".into(),
        },
        NegativeVector {
            name: "envelope_ciphertext_tamper".into(),
            target: "envelope".into(),
            operation: "replace_json_value".into(),
            json_pointer: Some("/ciphertext".into()),
            replacement: Some(Value::String(envelope_ciphertext)),
            expected_rejection: "signature_or_authentication".into(),
        },
        NegativeVector {
            name: "envelope_replay".into(),
            target: "envelope".into(),
            operation: "redeliver_exact".into(),
            json_pointer: None,
            replacement: None,
            expected_rejection: "replay".into(),
        },
        NegativeVector {
            name: "icon_blob_bytes_tamper".into(),
            target: "icon_blob.blob".into(),
            operation: "replace_json_value".into(),
            json_pointer: Some("/bytes".into()),
            replacement: Some(Value::String(icon_bytes)),
            expected_rejection: "content_commitment".into(),
        },
        NegativeVector {
            name: "reference_blob_bytes_tamper".into(),
            target: "reference_blob.blob".into(),
            operation: "replace_json_value".into(),
            json_pointer: Some("/bytes".into()),
            replacement: Some(Value::String(reference_bytes)),
            expected_rejection: "content_commitment".into(),
        },
        NegativeVector {
            name: "reference_chunk_bytes_tamper".into(),
            target: "reference_blob.chunks/0".into(),
            operation: "replace_json_value".into(),
            json_pointer: Some("/bytes".into()),
            replacement: Some(Value::String(reference_chunk_bytes)),
            expected_rejection: "chunk_commitment".into(),
        },
    ]
}

fn corrupted(encoded: &str) -> String {
    let replacement = if encoded.starts_with('A') { 'B' } else { 'A' };
    let mut corrupted = encoded.to_owned();
    corrupted.replace_range(0..1, &replacement.to_string());
    corrupted
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::CapabilityRegistry;
    use crate::crypto::{decode_array, decode_base64url};
    use crate::envelope::open_envelope;
    use crate::journal::ReplayWindow;
    use crate::pairing::RelayAllowlist;
    use crate::reference::{ChunkAdmission, PhoneToMacPayload, ReferenceBlobAssembler};

    fn at(value: &str) -> time::OffsetDateTime {
        crate::parse_timestamp(value).unwrap()
    }

    fn verify_vectors(vectors: &SharedVectors) {
        assert_eq!(vectors.schema, SHARED_VECTOR_SCHEMA);
        assert!(vectors.test_only);
        let entropy = decode_array::<16>(
            "fixture entropy",
            &vectors.companion_identity.entropy_base64url,
        )
        .unwrap();
        let (phrase, companion) = CompanionIdentity::from_entropy(entropy).unwrap();
        assert_eq!(phrase.expose(), vectors.companion_identity.mnemonic);
        assert_eq!(
            companion.signing_public_key_base64url(),
            vectors.companion_identity.signing_public_key_base64url
        );

        vectors.icon_blob.blob.validate().unwrap();
        assert_eq!(
            base64url(&canonical::to_vec(&vectors.icon_blob.blob).unwrap()),
            vectors.icon_blob.canonical_base64url
        );
        vectors.reference_blob.blob.validate().unwrap();
        assert_eq!(
            base64url(&canonical::to_vec(&vectors.reference_blob.blob).unwrap()),
            vectors.reference_blob.canonical_base64url
        );
        assert_eq!(
            vectors.reference_blob.chunks.len(),
            vectors.reference_blob.chunk_canonical_base64url.len()
        );
        let mut assembler = ReferenceBlobAssembler::default();
        for (index, chunk) in vectors.reference_blob.chunks.iter().enumerate() {
            chunk.validate().unwrap();
            assert_eq!(
                base64url(&canonical::to_vec(chunk).unwrap()),
                vectors.reference_blob.chunk_canonical_base64url[index]
            );
            let parsed =
                PhoneToMacPayload::from_canonical_slice(&canonical::to_vec(chunk).unwrap())
                    .unwrap();
            assert_eq!(parsed, PhoneToMacPayload::ReferenceBlobChunk(chunk.clone()));
            let result = assembler.admit(chunk.clone()).unwrap();
            if index + 1 == vectors.reference_blob.chunks.len() {
                assert_eq!(
                    result,
                    ChunkAdmission::Complete(vectors.reference_blob.blob.clone())
                );
            }
        }
        assert_eq!(
            companion.agreement_public_key_base64url(),
            vectors.companion_identity.agreement_public_key_base64url
        );

        let mnemonic =
            Mnemonic::parse_in_normalized(Language::English, &vectors.bip39_official.mnemonic)
                .unwrap();
        assert_eq!(
            hex(&mnemonic.to_seed_normalized(&vectors.bip39_official.passphrase)),
            vectors.bip39_official.seed_hex
        );

        let studio = WorkspaceServiceIdentity::from_secret_keys([1_u8; 32], [2_u8; 32]).unwrap();
        vectors
            .pairing
            .invitation
            .verify(
                &studio.signing_public_key(),
                &RelayAllowlist::official(),
                at("2026-08-15T12:01:00Z"),
            )
            .unwrap();
        assert_eq!(
            PairingInvitation::from_uri(&vectors.pairing.invitation_uri).unwrap(),
            vectors.pairing.invitation
        );
        let mut sessions = PairingSessionStore::default();
        sessions
            .insert_with_secret(
                VECTOR_PAIRING_SESSION_ID.into(),
                "workspace_vector_001",
                studio.device_id(),
                "official-v1",
                "2026-08-15T12:00:00Z",
                "2026-08-15T12:02:00Z",
                &studio,
                decode_array::<32>(
                    "fixture ephemeral secret",
                    &vectors.pairing.studio_ephemeral_secret_key_base64url,
                )
                .unwrap(),
            )
            .unwrap();
        sessions
            .consume_encrypted(
                VECTOR_PAIRING_SESSION_ID,
                &vectors.pairing.encrypted_response,
                &studio.signing_public_key(),
                &RelayAllowlist::official(),
                at("2026-08-15T12:01:01Z"),
            )
            .unwrap();

        let registry = CapabilityRegistry::new("workspace_vector_001").unwrap();
        registry
            .authorize(
                &vectors.capability.grant,
                CapabilityAction::ShotEvolve,
                &studio.signing_public_key(),
                at("2026-08-15T12:02:00Z"),
            )
            .unwrap();
        vectors
            .pairing_acceptance
            .acceptance
            .validate(
                &studio.signing_public_key(),
                studio.device_id(),
                companion.device_id(),
                "workspace_vector_001",
                at("2026-08-15T12:02:00Z"),
            )
            .unwrap();
        vectors
            .command
            .command
            .verify(
                &companion.signing_public_key(),
                companion.device_id(),
                at("2026-08-15T12:02:00Z"),
            )
            .unwrap();
        assert_eq!(
            PhoneToMacPayload::from_canonical_slice(
                &canonical::to_vec(&vectors.command.command).unwrap(),
            )
            .unwrap(),
            PhoneToMacPayload::Command(Box::new(vectors.command.command.clone()))
        );
        vectors
            .snapshot_request_command
            .command
            .verify(
                &companion.signing_public_key(),
                companion.device_id(),
                at("2026-08-15T12:02:00Z"),
            )
            .unwrap();
        assert_eq!(
            vectors
                .snapshot_request_command
                .command
                .body
                .payload
                .required_capability(),
            CapabilityAction::WorkspaceRead
        );

        let plaintext = open_envelope(
            &vectors.envelope.envelope,
            &companion.signing_public_key(),
            companion.device_id(),
            &studio,
            at("2026-08-15T12:02:00Z"),
            &mut ReplayWindow::new(128).unwrap(),
        )
        .unwrap();
        assert_eq!(
            plaintext,
            decode_base64url(
                "fixture plaintext",
                &vectors.envelope.plaintext_base64url,
                1024 * 1024,
            )
            .unwrap()
        );

        vectors.relay.pairing_create.validate().unwrap();
        vectors.relay.pairing_created.validate().unwrap();
        vectors.relay.pairing_response_accepted.validate().unwrap();
        vectors.relay.mailbox_create.validate().unwrap();
        vectors.relay.mailbox_created.validate().unwrap();
        assert_eq!(vectors.relay.direct_envelope, vectors.envelope.envelope);
        vectors.relay.envelope_accepted.validate().unwrap();
        vectors
            .relay
            .mailbox_page
            .validate_routing(VECTOR_MAILBOX_ID, 6)
            .unwrap();
        vectors.relay.mailbox_reset_required.validate().unwrap();
        vectors.relay.mailbox_ack.validate().unwrap();
        vectors.relay.mailbox_acknowledged.validate().unwrap();
        vectors.relay.mailbox_revoked.validate().unwrap();
        vectors.relay.push_register.validate().unwrap();
        vectors.relay.push_unregister.validate().unwrap();
        vectors.relay.health.validate().unwrap();
    }

    #[test]
    fn generated_vectors_are_internally_conformant() {
        verify_vectors(&deterministic_vectors().unwrap());
    }

    #[test]
    fn checked_in_fixture_is_exactly_the_generated_fixture() {
        let fixture: SharedVectors =
            serde_json::from_str(include_str!("../test-vectors/companion-v1.json")).unwrap();
        assert_eq!(fixture, deterministic_vectors().unwrap());
        verify_vectors(&fixture);
    }

    #[test]
    fn every_checked_in_negative_fixture_is_rejected() {
        let vectors = deterministic_vectors().unwrap();
        let studio = WorkspaceServiceIdentity::from_secret_keys([1_u8; 32], [2_u8; 32]).unwrap();
        let (_, companion) = CompanionIdentity::from_entropy([0_u8; 16]).unwrap();
        let mut valid_replay = ReplayWindow::new(128).unwrap();
        open_envelope(
            &vectors.envelope.envelope,
            &companion.signing_public_key(),
            companion.device_id(),
            &studio,
            at("2026-08-15T12:02:00Z"),
            &mut valid_replay,
        )
        .unwrap();

        for negative in &vectors.negative {
            let rejected = match negative.name.as_str() {
                "invitation_signature_tamper" | "invitation_unallowlisted_relay" => {
                    let value: PairingInvitation = serde_json::from_value(apply_negative(
                        serde_json::to_value(&vectors.pairing.invitation).unwrap(),
                        negative,
                    ))
                    .unwrap();
                    value
                        .verify(
                            &studio.signing_public_key(),
                            &RelayAllowlist::official(),
                            at("2026-08-15T12:01:00Z"),
                        )
                        .is_err()
                }
                "pairing_key_confirmation_tamper" => {
                    let proof: PairingProof = serde_json::from_value(apply_negative(
                        serde_json::to_value(&vectors.pairing.proof).unwrap(),
                        negative,
                    ))
                    .unwrap();
                    let mut store = PairingSessionStore::default();
                    store
                        .insert_with_secret(
                            VECTOR_PAIRING_SESSION_ID.into(),
                            "workspace_vector_001",
                            studio.device_id(),
                            "official-v1",
                            "2026-08-15T12:00:00Z",
                            "2026-08-15T12:02:00Z",
                            &studio,
                            [9_u8; 32],
                        )
                        .unwrap();
                    store
                        .consume(
                            VECTOR_PAIRING_SESSION_ID,
                            &proof,
                            &studio.signing_public_key(),
                            &RelayAllowlist::official(),
                            at("2026-08-15T12:01:01Z"),
                        )
                        .is_err()
                }
                "pairing_response_ciphertext_tamper" => {
                    let response: EncryptedPairingResponse =
                        serde_json::from_value(apply_negative(
                            serde_json::to_value(&vectors.pairing.encrypted_response).unwrap(),
                            negative,
                        ))
                        .unwrap();
                    let mut store = PairingSessionStore::default();
                    store
                        .insert_with_secret(
                            VECTOR_PAIRING_SESSION_ID.into(),
                            "workspace_vector_001",
                            studio.device_id(),
                            "official-v1",
                            "2026-08-15T12:00:00Z",
                            "2026-08-15T12:02:00Z",
                            &studio,
                            [9_u8; 32],
                        )
                        .unwrap();
                    store
                        .consume_encrypted(
                            VECTOR_PAIRING_SESSION_ID,
                            &response,
                            &studio.signing_public_key(),
                            &RelayAllowlist::official(),
                            at("2026-08-15T12:01:01Z"),
                        )
                        .is_err()
                }
                "capability_signature_tamper" => {
                    let grant: CapabilityGrant = serde_json::from_value(apply_negative(
                        serde_json::to_value(&vectors.capability.grant).unwrap(),
                        negative,
                    ))
                    .unwrap();
                    grant
                        .verify(&studio.signing_public_key(), at("2026-08-15T12:02:00Z"))
                        .is_err()
                }
                "command_signature_tamper" => {
                    let command: CompanionCommand = serde_json::from_value(apply_negative(
                        serde_json::to_value(&vectors.command.command).unwrap(),
                        negative,
                    ))
                    .unwrap();
                    command
                        .verify(
                            &companion.signing_public_key(),
                            companion.device_id(),
                            at("2026-08-15T12:02:00Z"),
                        )
                        .is_err()
                }
                "envelope_ciphertext_tamper" => {
                    let envelope: OpaqueEnvelope = serde_json::from_value(apply_negative(
                        serde_json::to_value(&vectors.envelope.envelope).unwrap(),
                        negative,
                    ))
                    .unwrap();
                    open_envelope(
                        &envelope,
                        &companion.signing_public_key(),
                        companion.device_id(),
                        &studio,
                        at("2026-08-15T12:02:00Z"),
                        &mut ReplayWindow::new(128).unwrap(),
                    )
                    .is_err()
                }
                "envelope_replay" => {
                    let envelope = vectors.envelope.envelope.clone();
                    open_envelope(
                        &envelope,
                        &companion.signing_public_key(),
                        companion.device_id(),
                        &studio,
                        at("2026-08-15T12:02:00Z"),
                        &mut valid_replay,
                    )
                    .is_err()
                }
                "icon_blob_bytes_tamper" => {
                    let blob: IconBlob = serde_json::from_value(apply_negative(
                        serde_json::to_value(&vectors.icon_blob.blob).unwrap(),
                        negative,
                    ))
                    .unwrap();
                    blob.validate().is_err()
                }
                "reference_blob_bytes_tamper" => {
                    let blob: ReferenceBlob = serde_json::from_value(apply_negative(
                        serde_json::to_value(&vectors.reference_blob.blob).unwrap(),
                        negative,
                    ))
                    .unwrap();
                    blob.validate().is_err()
                }
                "reference_chunk_bytes_tamper" => {
                    let chunk: ReferenceBlobChunk = serde_json::from_value(apply_negative(
                        serde_json::to_value(&vectors.reference_blob.chunks[0]).unwrap(),
                        negative,
                    ))
                    .unwrap();
                    chunk.validate().is_err()
                }
                unknown => panic!("unhandled negative vector {unknown}"),
            };
            assert!(rejected, "negative vector {} was accepted", negative.name);
        }
    }

    fn apply_negative(mut base: Value, negative: &NegativeVector) -> Value {
        assert_eq!(negative.operation, "replace_json_value");
        let pointer = negative.json_pointer.as_deref().unwrap();
        *base.pointer_mut(pointer).unwrap() = negative.replacement.clone().unwrap();
        base
    }
}
