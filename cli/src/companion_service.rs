//! Private companion authorization and translation into the shared application service.
//!
//! Nothing in this module is public Shot lineage. The persisted records are
//! private device/capability provenance used by the Local Workspace Service.

use futures_util::StreamExt;
use rand_core::{OsRng, RngCore};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime, UtcOffset};
use tohseno_application::{
    ApplicationError, CommandOrigin, CommandState, WorkspaceSnapshot as LocalWorkspaceSnapshot,
};
use tohseno_application::{
    CreateShotCommand, EvolveShotCommand, ReferenceInput, ShotApplicationService,
    SubmitFeedbackCommand, SubmitMarketingNoteCommand,
};
use tohseno_companion::capability::{
    CapabilityAction, CapabilityGrant, CapabilityGrantBody, CapabilityRegistry,
    CAPABILITY_GRANT_SCHEMA,
};
use tohseno_companion::command::{
    CommandPayload, CommandReceipt, CompanionCommand, ReceiptState, ReferenceDescriptor,
};
use tohseno_companion::crypto::{base64url, decode_array};
use tohseno_companion::envelope::{open_envelope, seal_envelope, EnvelopeMetadata, OpaqueEnvelope};
use tohseno_companion::event::{WorkspaceEvent, WorkspaceEventPayload, COMPANION_EVENT_SCHEMA};
use tohseno_companion::icon::IconBlob;
use tohseno_companion::journal::ReplayWindow;
use tohseno_companion::pairing::{
    EncryptedPairingResponse, PairingAcceptance, PairingInvitation, PairingProof,
    PairingResponseBody, PairingSessionState, PairingSessionStore, RelayAllowlist,
    PAIRING_ACCEPTANCE_SCHEMA, PAIRING_INVITATION_LIFETIME_SECONDS, PAIRING_RESPONSE_BODY_SCHEMA,
};
use tohseno_companion::reference::{
    ChunkAdmission, PhoneToMacPayload, ReferenceBlob, ReferenceBlobAssembler, ReferenceBlobChunk,
    MAX_REFERENCE_BLOB_BYTES, MAX_REFERENCE_CHUNK_BYTES,
};
use tohseno_companion::relay_client::{
    capability_verifier, EnvelopeAccepted, MailboxAck, MailboxAcknowledged, MailboxCreate,
    MailboxCreated, MailboxPage, MailboxResetRequired, MailboxRevoked, PairingSessionCreate,
    PairingSessionCreated, RelayHealth,
};
use tohseno_companion::snapshot::{
    DeviceCapabilityState, ExecutionStatus, ExecutionSummary, ShotKind, ShotSummary,
    WorkspaceSnapshot, WORKSPACE_SNAPSHOT_SCHEMA,
};
use tohseno_protocol::digest::{Bytes32, ExpressionId, ShotId, VersionId};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use crate::workspace_identity::WorkspaceIdentity;

const DEVICE_SCHEMA: &str = "tohseno.companion-device-record/1";
const PAIRING_COMPLETION_SCHEMA: &str = "tohseno.studio-pairing-completion/1";
const MAX_DEVICE_RECORD_BYTES: u64 = 256 * 1024;
const MAX_REFERENCE_BLOBS_PER_DEVICE: usize = 64;
const MAX_REFERENCE_CHUNKS_PER_BLOB: usize =
    MAX_REFERENCE_BLOB_BYTES.div_ceil(MAX_REFERENCE_CHUNK_BYTES);
const MAX_REFERENCE_ENVELOPES_PER_BLOB: usize = 256;
const MAX_REFERENCE_INDEX_BYTES: u64 = 4 * 1024;
const MAX_REFERENCE_CHUNK_RECORD_BYTES: u64 =
    (MAX_REFERENCE_CHUNK_BYTES as u64 * 4 / 3) + 64 * 1024;
const MAX_COMPLETED_REFERENCE_RECORD_BYTES: u64 =
    (MAX_REFERENCE_BLOB_BYTES as u64 * 4 / 3) + 64 * 1024;
const MAX_REFERENCE_ENVELOPE_LINK_BYTES: u64 = 1024;
// One relay page is deliberately requested one envelope at a time. A maximum
// 16 MiB ciphertext expands under base64url and outer canonical JSON, so keep
// the response bounded while still accepting every valid envelope.
const MAX_RELAY_JSON_BYTES: usize = 24 * 1024 * 1024;
const MAX_RELAY_PAIRING_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_ADMITTED_ENVELOPE_RECORD_BYTES: u64 = 64 * 1024;
const MAX_ADMITTED_ENVELOPE_RECORDS: usize = 200_000;
const MAX_PROCESSED_COMMAND_RECORD_BYTES: u64 = 64 * 1024;
const MAX_PROCESSED_COMMAND_RECORDS: usize = 100_000;
const MAX_RELAY_PAGES_PER_RECONCILIATION: usize = 32;
const WORKSPACE_PROJECTION_SCHEMA: &str = "tohseno.companion-workspace-projection/1";

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceRecord {
    pub schema: String,
    pub device_id: String,
    pub display_name: String,
    pub signing_public_key: String,
    pub agreement_public_key: String,
    pub capability: CapabilityGrant,
    pub phone_mailbox_id: String,
    pub phone_mailbox_write_capability: String,
    pub phone_mailbox_revoke_capability: String,
    pub studio_mailbox_id: String,
    pub studio_mailbox_write_capability: String,
    pub studio_mailbox_read_capability: String,
    pub studio_mailbox_ack_capability: String,
    pub studio_mailbox_revoke_capability: String,
    pub studio_mailbox_cursor: u64,
    pub paired_at: String,
    pub last_seen: String,
    pub revocation_epoch: u64,
    pub revoked: bool,
    pub relay_revocation_complete: bool,
}

impl std::fmt::Debug for DeviceRecord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeviceRecord")
            .field("schema", &self.schema)
            .field("device_id", &abbreviate(&self.device_id))
            .field("display_name", &"[REDACTED]")
            .field("relay_credentials", &"[REDACTED]")
            .field("revocation_epoch", &self.revocation_epoch)
            .field("revoked", &self.revoked)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DeviceSummary {
    pub device_id: String,
    pub device_id_abbreviation: String,
    pub display_name: String,
    pub capabilities: Vec<CapabilityAction>,
    pub paired_at: String,
    pub last_seen: String,
    pub sync_state: &'static str,
    pub revoked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PairingSessionView {
    pub schema: &'static str,
    pub session_id: String,
    pub state: &'static str,
    pub expires_at: String,
    pub pairing_uri: String,
    pub qr_svg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PairingCompletion {
    pub schema: &'static str,
    pub capability_envelope: OpaqueEnvelope,
    pub snapshot_envelope: OpaqueEnvelope,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RelayReconciliationSummary {
    pub devices_checked: usize,
    pub envelopes_processed: usize,
    pub command_receipts_published: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmittedEnvelopeRecord {
    schema: String,
    envelope_id: String,
    envelope_digest: String,
    result: ProcessedEnvelope,
}

/// A compact transport receipt keyed by the companion command idempotency key.
///
/// The application command journal remains authoritative for the semantic
/// action and retains exact private inputs. This record lets a newly sealed
/// retry return the same companion receipt after its temporary reference-inbox
/// copies have been reclaimed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessedCommandRecord {
    schema: String,
    command_id: String,
    command_digest: String,
    origin_device_id: String,
    reference_blob_ids: Vec<String>,
    receipt: CommandReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceEnvelopeLink {
    schema: String,
    envelope_id: String,
    envelope_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProcessedEnvelope {
    Command(CommandReceipt),
    ReferenceChunk(ReferenceChunkReceipt),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceChunkReceipt {
    pub schema: String,
    pub blob_id: String,
    pub chunk_index: u64,
    pub state: ReferenceChunkState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceChunkState {
    Stored,
    Duplicate,
    Complete,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceBlobIndex {
    schema: String,
    device_id: String,
    descriptor: ReferenceDescriptor,
    chunk_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishedWorkspaceProjection {
    schema: String,
    workspace_id: String,
    service_version: String,
    shots: Vec<ShotSummary>,
    active_executions: Vec<ExecutionSummary>,
    device_capability_state: DeviceCapabilityState,
    next_cursor: u64,
}

#[derive(Clone)]
struct RuntimePairing {
    invitation: PairingInvitation,
    relay_read_capability: String,
    relay_cancel_capability: String,
    pending_response: Option<PairingResponseBody>,
    device_name: Option<String>,
}

#[derive(Clone)]
struct RelayConfiguration {
    origin: String,
    client: reqwest::Client,
}

pub struct CompanionCoordinator {
    service_root: PathBuf,
    workspace: Arc<WorkspaceIdentity>,
    application: ShotApplicationService,
    sessions: Mutex<PairingSessionStore>,
    session_views: Mutex<BTreeMap<String, RuntimePairing>>,
    replay: Mutex<ReplayWindow>,
    outbox_counters: Mutex<()>,
    inbox_publications: Mutex<()>,
    pairing_operations: AsyncMutex<()>,
    relay: Option<RelayConfiguration>,
}

impl CompanionCoordinator {
    pub fn open(
        service_root: PathBuf,
        workspace: Arc<WorkspaceIdentity>,
        application: ShotApplicationService,
    ) -> Result<Self, BoxError> {
        ensure_private_directory(&service_root.join("devices"))?;
        ensure_private_directory(&service_root.join("inbox/envelopes"))?;
        ensure_private_directory(&service_root.join("inbox/commands"))?;
        ensure_private_directory(&service_root.join("inbox/blobs"))?;
        ensure_private_directory(&service_root.join("outbox"))?;
        let relay = RelayConfiguration::from_environment()?;
        Ok(Self {
            service_root,
            workspace,
            application,
            sessions: Mutex::new(PairingSessionStore::default()),
            session_views: Mutex::new(BTreeMap::new()),
            replay: Mutex::new(ReplayWindow::new(65_536)?),
            outbox_counters: Mutex::new(()),
            inbox_publications: Mutex::new(()),
            pairing_operations: AsyncMutex::new(()),
            relay,
        })
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace.record.workspace_id
    }

    pub async fn relay_health(&self) -> Result<Option<RelayHealth>, BoxError> {
        match &self.relay {
            Some(relay) => relay.health().await.map(Some),
            None => Ok(None),
        }
    }

    pub async fn create_pairing_session(&self) -> Result<PairingSessionView, BoxError> {
        let _operation = self.pairing_operations.lock().await;
        self.create_pairing_session_serialized().await
    }

    async fn create_pairing_session_serialized(&self) -> Result<PairingSessionView, BoxError> {
        let relay = self
            .relay
            .as_ref()
            .ok_or("Companion Relay is not configured; set TOHSENO_COMPANION_RELAY_ORIGIN")?;
        let issued = OffsetDateTime::now_utc().replace_nanosecond(0)?;
        let expires = issued + Duration::seconds(PAIRING_INVITATION_LIFETIME_SECONDS);
        let issued_at = timestamp(issued)?;
        let expires_at = timestamp(expires)?;
        let relay_read_capability = random_relay_capability();
        let relay_cancel_capability = random_relay_capability();
        let created = relay
            .create_pairing_session(
                &expires_at,
                &relay_read_capability,
                &relay_cancel_capability,
            )
            .await?;
        let allowlist = RelayAllowlist::official();
        let invitation = self
            .sessions
            .lock()
            .map_err(|_| "pairing-session lock failed")?
            .register_relay_session(
                created.session_id.clone(),
                &self.workspace.record.workspace_id,
                &self.workspace.record.studio_device_id,
                "official-v1",
                &issued_at,
                &expires_at,
                &*self.workspace.identity,
                &allowlist,
            )?;
        let uri = invitation.to_uri()?;
        let qr_svg = qrcode::QrCode::new(uri.as_bytes())?
            .render::<qrcode::render::svg::Color>()
            .quiet_zone(true)
            .min_dimensions(300, 300)
            .build();
        let session_id = invitation.body.session_id.clone();
        self.session_views
            .lock()
            .map_err(|_| "pairing-session view lock failed")?
            .insert(
                session_id.clone(),
                RuntimePairing {
                    invitation,
                    relay_read_capability,
                    relay_cancel_capability,
                    pending_response: None,
                    device_name: None,
                },
            );
        Ok(PairingSessionView {
            schema: "tohseno.studio-pairing-session/1",
            session_id,
            state: "waiting",
            expires_at,
            pairing_uri: uri,
            qr_svg,
            device_name: None,
        })
    }

    pub async fn pairing_session(&self, session_id: &str) -> Result<PairingSessionView, BoxError> {
        let _operation = self.pairing_operations.lock().await;
        self.pairing_session_serialized(session_id).await
    }

    async fn pairing_session_serialized(
        &self,
        session_id: &str,
    ) -> Result<PairingSessionView, BoxError> {
        let runtime = self
            .session_views
            .lock()
            .map_err(|_| "pairing-session view lock failed")?
            .get(session_id)
            .cloned()
            .ok_or("pairing session does not exist")?;
        let initial_state = self
            .sessions
            .lock()
            .map_err(|_| "pairing-session lock failed")?
            .state(session_id)
            .ok_or("pairing session does not exist")?;
        let now_value = OffsetDateTime::now_utc().replace_nanosecond(0)?;
        let mut response_to_finish = None;
        if initial_state == PairingSessionState::Active
            && now_value <= tohseno_companion::parse_timestamp(&runtime.invitation.body.expires_at)?
        {
            let relay = self
                .relay
                .as_ref()
                .ok_or("Companion Relay is not configured")?;
            if let Some(bytes) = relay
                .pairing_response(session_id, &runtime.relay_read_capability)
                .await?
            {
                let response: EncryptedPairingResponse =
                    tohseno_companion::canonical::from_slice(&bytes)?;
                let response_body = self
                    .sessions
                    .lock()
                    .map_err(|_| "pairing-session lock failed")?
                    .consume_encrypted(
                        session_id,
                        &response,
                        &self.workspace.identity.signing_public_key(),
                        &RelayAllowlist::official(),
                        now_value,
                    )?;
                self.session_views
                    .lock()
                    .map_err(|_| "pairing-session view lock failed")?
                    .get_mut(session_id)
                    .ok_or("pairing session does not exist")?
                    .pending_response = Some(response_body.clone());
                response_to_finish = Some(response_body);
            }
        }
        if response_to_finish.is_none()
            && initial_state == PairingSessionState::Consumed
            && runtime.device_name.is_none()
        {
            response_to_finish = runtime.pending_response.clone();
        }
        if let Some(response_body) = response_to_finish {
            let _ = self.finish_pairing(session_id, response_body).await?;
        }
        let runtime = self
            .session_views
            .lock()
            .map_err(|_| "pairing-session view lock failed")?
            .get(session_id)
            .cloned()
            .ok_or("pairing session does not exist")?;
        let state = self
            .sessions
            .lock()
            .map_err(|_| "pairing-session lock failed")?
            .state(session_id)
            .ok_or("pairing session does not exist")?;
        let state = match state {
            PairingSessionState::Active => {
                if OffsetDateTime::now_utc()
                    > tohseno_companion::parse_timestamp(&runtime.invitation.body.expires_at)?
                {
                    "expired"
                } else {
                    "waiting"
                }
            }
            PairingSessionState::Consumed => "paired",
            PairingSessionState::Cancelled => "cancelled",
        };
        let uri = runtime.invitation.to_uri()?;
        let qr_svg = qrcode::QrCode::new(uri.as_bytes())?
            .render::<qrcode::render::svg::Color>()
            .quiet_zone(true)
            .min_dimensions(300, 300)
            .build();
        Ok(PairingSessionView {
            schema: "tohseno.studio-pairing-session/1",
            session_id: session_id.into(),
            state,
            expires_at: runtime.invitation.body.expires_at.clone(),
            pairing_uri: uri,
            qr_svg,
            device_name: runtime.device_name,
        })
    }

    /// Reconciles every pending one-use session independently of Studio.
    ///
    /// One async operation lock covers relay reads, session consumption,
    /// cancellation, and direct proof completion. This prevents a Studio GET
    /// and the detached service loop from consuming the same response or
    /// allocating two capability/mailbox sets for one phone.
    pub async fn reconcile_pairing_sessions(&self) -> Result<usize, BoxError> {
        let _operation = self.pairing_operations.lock().await;
        let candidates = {
            let views = self
                .session_views
                .lock()
                .map_err(|_| "pairing-session view lock failed")?;
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| "pairing-session lock failed")?;
            views
                .iter()
                .filter(|(session_id, runtime)| {
                    runtime.device_name.is_none()
                        && matches!(
                            sessions.state(session_id),
                            Some(PairingSessionState::Active | PairingSessionState::Consumed)
                        )
                })
                .map(|(session_id, _)| session_id.clone())
                .collect::<Vec<_>>()
        };
        let mut completed = 0_usize;
        let mut first_error = None;
        for session_id in candidates {
            let before = self
                .session_views
                .lock()
                .map_err(|_| "pairing-session view lock failed")?
                .get(&session_id)
                .and_then(|runtime| runtime.device_name.clone());
            match self.pairing_session_serialized(&session_id).await {
                Ok(view)
                    if before.is_none() && view.state == "paired" && view.device_name.is_some() =>
                {
                    completed += 1;
                }
                Ok(_) => {}
                Err(error) if first_error.is_none() => first_error = Some(error),
                Err(_) => {}
            }
        }
        if completed > 0 {
            Ok(completed)
        } else if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(0)
        }
    }

    pub async fn cancel_pairing_session(&self, session_id: &str) -> Result<(), BoxError> {
        let _operation = self.pairing_operations.lock().await;
        self.cancel_pairing_session_serialized(session_id).await
    }

    async fn cancel_pairing_session_serialized(&self, session_id: &str) -> Result<(), BoxError> {
        let runtime = self
            .session_views
            .lock()
            .map_err(|_| "pairing-session view lock failed")?
            .get(session_id)
            .cloned()
            .ok_or("pairing session does not exist")?;
        self.sessions
            .lock()
            .map_err(|_| "pairing-session lock failed")?
            .cancel(session_id)?;
        if let Some(relay) = &self.relay {
            relay
                .cancel_pairing_session(session_id, &runtime.relay_cancel_capability)
                .await?;
        }
        Ok(())
    }

    pub async fn complete_pairing(
        &self,
        session_id: &str,
        proof: PairingProof,
    ) -> Result<PairingCompletion, BoxError> {
        let _operation = self.pairing_operations.lock().await;
        self.complete_pairing_serialized(session_id, proof).await
    }

    async fn complete_pairing_serialized(
        &self,
        session_id: &str,
        proof: PairingProof,
    ) -> Result<PairingCompletion, BoxError> {
        let relay = self
            .relay
            .as_ref()
            .ok_or("Companion Relay is not configured")?;
        let response_mailbox = relay.create_mailbox().await?;
        let now_value = OffsetDateTime::now_utc().replace_nanosecond(0)?;
        let consumed = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| "pairing-session lock failed")?;
            sessions.consume(
                session_id,
                &proof,
                &self.workspace.identity.signing_public_key(),
                &RelayAllowlist::official(),
                now_value,
            )
        };
        let proof_body = match consumed {
            Ok(body) => body,
            Err(error) => {
                let _ = relay
                    .revoke_mailbox(
                        &response_mailbox.created.mailbox_id,
                        &response_mailbox.revoke_capability,
                    )
                    .await;
                return Err(error.into());
            }
        };
        let response = PairingResponseBody {
            schema: PAIRING_RESPONSE_BODY_SCHEMA.into(),
            proof: PairingProof {
                body: proof_body,
                key_confirmation: proof.key_confirmation,
                signature: proof.signature,
            },
            response_mailbox_id: response_mailbox.created.mailbox_id,
            response_mailbox_write_capability: response_mailbox.write_capability,
            response_mailbox_revoke_capability: response_mailbox.revoke_capability,
        };
        self.session_views
            .lock()
            .map_err(|_| "pairing-session view lock failed")?
            .get_mut(session_id)
            .ok_or("pairing session does not exist")?
            .pending_response = Some(response.clone());
        self.finish_pairing(session_id, response).await
    }

    async fn finish_pairing(
        &self,
        session_id: &str,
        response: PairingResponseBody,
    ) -> Result<PairingCompletion, BoxError> {
        let relay = self
            .relay
            .as_ref()
            .ok_or("Companion Relay is not configured")?;
        response.validate()?;
        let now_value = OffsetDateTime::now_utc().replace_nanosecond(0)?;
        let proof_body = &response.proof.body;
        let signing_key = decode_array::<32>(
            "companion signing public key",
            &proof_body.companion_signing_public_key,
        )?;
        let agreement_key = decode_array::<32>(
            "companion agreement public key",
            &proof_body.companion_agreement_public_key,
        )?;
        let issued_at = timestamp(now_value)?;
        let existing = self.load_device(&proof_body.companion_device_id)?;
        let is_retry = existing.as_ref().is_some_and(|record| {
            !record.revoked
                && record.phone_mailbox_id == response.response_mailbox_id
                && record.phone_mailbox_write_capability
                    == response.response_mailbox_write_capability
                && record.phone_mailbox_revoke_capability
                    == response.response_mailbox_revoke_capability
                && record.signing_public_key == base64url(&signing_key)
                && record.agreement_public_key == base64url(&agreement_key)
        });
        let record = if is_retry {
            existing.ok_or("paired-device retry state disappeared")?
        } else {
            let prior_epoch = existing
                .as_ref()
                .map(|record| record.revocation_epoch)
                .unwrap_or(0);
            let capability = CapabilityGrant::sign(
                CapabilityGrantBody {
                    schema: CAPABILITY_GRANT_SCHEMA.into(),
                    capability_id: format!("capability_{}", compact_uuid()),
                    workspace_id: self.workspace.record.workspace_id.clone(),
                    device_id: proof_body.companion_device_id.clone(),
                    allowed_actions: all_capabilities(),
                    issued_at: issued_at.clone(),
                    expires_at: None,
                    revocation_epoch: prior_epoch,
                    studio_signing_public_key: self
                        .workspace
                        .identity
                        .signing_public_key_base64url(),
                },
                &*self.workspace.identity,
            )?;
            let studio_mailbox = relay.create_mailbox().await?;
            let record = DeviceRecord {
                schema: DEVICE_SCHEMA.into(),
                device_id: proof_body.companion_device_id.clone(),
                display_name: proof_body.companion_display_name.clone(),
                signing_public_key: base64url(&signing_key),
                agreement_public_key: base64url(&agreement_key),
                capability,
                phone_mailbox_id: response.response_mailbox_id.clone(),
                phone_mailbox_write_capability: response.response_mailbox_write_capability.clone(),
                phone_mailbox_revoke_capability: response
                    .response_mailbox_revoke_capability
                    .clone(),
                studio_mailbox_id: studio_mailbox.created.mailbox_id,
                studio_mailbox_write_capability: studio_mailbox.write_capability,
                studio_mailbox_read_capability: studio_mailbox.read_capability,
                studio_mailbox_ack_capability: studio_mailbox.ack_capability,
                studio_mailbox_revoke_capability: studio_mailbox.revoke_capability,
                studio_mailbox_cursor: 0,
                paired_at: issued_at.clone(),
                last_seen: issued_at.clone(),
                revocation_epoch: prior_epoch,
                revoked: false,
                relay_revocation_complete: false,
            };
            self.store_device(&record)?;
            record
        };
        let acceptance = PairingAcceptance {
            schema: PAIRING_ACCEPTANCE_SCHEMA.into(),
            capability_grant: record.capability.clone(),
            studio_agreement_public_key: self.workspace.identity.agreement_public_key_base64url(),
            command_mailbox_id: record.studio_mailbox_id.clone(),
            command_mailbox_write_capability: record.studio_mailbox_write_capability.clone(),
        };
        acceptance.validate(
            &self.workspace.identity.signing_public_key(),
            &self.workspace.record.studio_device_id,
            &record.device_id,
            &self.workspace.record.workspace_id,
            now_value,
        )?;
        let completion_bytes = tohseno_companion::canonical::to_vec(&acceptance)?;
        let capability_envelope = self.load_or_create_pairing_acceptance_envelope(
            &record,
            session_id,
            &agreement_key,
            &completion_bytes,
        )?;
        relay
            .upload_envelope(
                &record.phone_mailbox_id,
                &record.phone_mailbox_write_capability,
                &capability_envelope,
            )
            .await?;
        let (_, snapshot_envelope) = self
            .publish_workspace_snapshot(&record, &agreement_key)
            .await?;
        if let Some(runtime) = self
            .session_views
            .lock()
            .map_err(|_| "pairing-session view lock failed")?
            .get_mut(session_id)
        {
            runtime.device_name = Some(record.display_name.clone());
        }
        Ok(PairingCompletion {
            schema: PAIRING_COMPLETION_SCHEMA,
            capability_envelope,
            snapshot_envelope,
        })
    }

    pub fn devices(&self) -> Result<Vec<DeviceSummary>, BoxError> {
        let mut summaries = self
            .load_devices()?
            .into_iter()
            .map(|record| DeviceSummary {
                device_id_abbreviation: abbreviate(&record.device_id),
                device_id: record.device_id,
                display_name: record.display_name,
                capabilities: record.capability.body.allowed_actions,
                paired_at: record.paired_at,
                last_seen: record.last_seen,
                sync_state: if record.revoked { "revoked" } else { "ready" },
                revoked: record.revoked,
            })
            .collect::<Vec<_>>();
        summaries.sort_by(|left, right| left.paired_at.cmp(&right.paired_at));
        Ok(summaries)
    }

    /// Reconcile every active phone-to-Mac payload mailbox once. The Local
    /// Workspace Service invokes this independently of Studio, so reference
    /// chunks and commands continue after every Terminal and browser window
    /// has closed.
    pub async fn reconcile_relay_once(&self) -> Result<RelayReconciliationSummary, BoxError> {
        let Some(relay) = &self.relay else {
            return Ok(RelayReconciliationSummary {
                devices_checked: 0,
                envelopes_processed: 0,
                command_receipts_published: 0,
            });
        };
        let devices = self
            .load_devices()?
            .into_iter()
            .filter(|record| !record.revoked)
            .collect::<Vec<_>>();
        let mut summary = RelayReconciliationSummary {
            devices_checked: devices.len(),
            envelopes_processed: 0,
            command_receipts_published: 0,
        };
        for mut record in devices {
            if record.studio_mailbox_cursor > 0 {
                relay
                    .acknowledge_mailbox(
                        &record.studio_mailbox_id,
                        &record.studio_mailbox_ack_capability,
                        record.studio_mailbox_cursor,
                    )
                    .await?;
            }
            for _ in 0..MAX_RELAY_PAGES_PER_RECONCILIATION {
                let page = match relay
                    .mailbox_page(
                        &record.studio_mailbox_id,
                        &record.studio_mailbox_read_capability,
                        record.studio_mailbox_cursor,
                    )
                    .await?
                {
                    RelayMailboxFetch::Page(page) => page,
                    RelayMailboxFetch::Reset(reset) => {
                        // The phone remains authoritative for its durable
                        // outbox until a Mac-signed command receipt. Advance
                        // only past the relay-confirmed expired prefix (never
                        // to the head), then let the phone retry/reseal its
                        // exact idempotent payloads. Persist before ACK so a
                        // crash can cause only an idempotent acknowledgement.
                        record.studio_mailbox_cursor =
                            cursor_after_mailbox_reset(record.studio_mailbox_cursor, &reset)?;
                        record.last_seen = now()?;
                        self.store_device(&record)?;
                        relay
                            .acknowledge_mailbox(
                                &record.studio_mailbox_id,
                                &record.studio_mailbox_ack_capability,
                                record.studio_mailbox_cursor,
                            )
                            .await?;
                        continue;
                    }
                };
                page.validate_routing(&record.studio_mailbox_id, record.studio_mailbox_cursor)?;
                let has_more = page.has_more;
                if page.envelopes.is_empty() {
                    break;
                }
                for item in page.envelopes {
                    let processed = self.process_envelope(&item.envelope).await?;
                    if let ProcessedEnvelope::Command(receipt) = processed {
                        self.publish_command_receipt(&record, receipt).await?;
                        summary.command_receipts_published += 1;
                    }
                    // Publish the durable local cursor before the relay ACK.
                    // A crash can therefore cause at most an idempotent ACK,
                    // never a retained-range reset that loses an admitted
                    // payload between these two stores.
                    record.studio_mailbox_cursor = item.cursor;
                    record.last_seen = now()?;
                    self.store_device(&record)?;
                    relay
                        .acknowledge_mailbox(
                            &record.studio_mailbox_id,
                            &record.studio_mailbox_ack_capability,
                            item.cursor,
                        )
                        .await?;
                    summary.envelopes_processed += 1;
                }
                if !has_more {
                    break;
                }
            }
        }
        Ok(summary)
    }

    pub async fn revoke(&self, device_id: &str) -> Result<DeviceSummary, BoxError> {
        let mut record = self
            .load_device(device_id)?
            .ok_or("paired device does not exist")?;
        if !record.revoked {
            record.revoked = true;
            record.revocation_epoch = record
                .revocation_epoch
                .checked_add(1)
                .ok_or("device revocation epoch overflowed")?;
            record.last_seen = now()?;
            self.store_device(&record)?;
        }
        if !record.relay_revocation_complete {
            let relay = self
                .relay
                .as_ref()
                .ok_or("Companion Relay is not configured")?;
            relay
                .revoke_mailbox(
                    &record.phone_mailbox_id,
                    &record.phone_mailbox_revoke_capability,
                )
                .await?;
            relay
                .revoke_mailbox(
                    &record.studio_mailbox_id,
                    &record.studio_mailbox_revoke_capability,
                )
                .await?;
            record.relay_revocation_complete = true;
            self.store_device(&record)?;
        }
        Ok(DeviceSummary {
            device_id_abbreviation: abbreviate(&record.device_id),
            device_id: record.device_id,
            display_name: record.display_name,
            capabilities: record.capability.body.allowed_actions,
            paired_at: record.paired_at,
            last_seen: record.last_seen,
            sync_state: "revoked",
            revoked: true,
        })
    }

    pub async fn process_envelope(
        &self,
        envelope: &OpaqueEnvelope,
    ) -> Result<ProcessedEnvelope, BoxError> {
        envelope.validate_relay_shape()?;
        if let Some(result) = self.admitted_envelope_result(envelope)? {
            return Ok(result);
        }
        let record = self
            .load_device(&envelope.header.sender_device_id)?
            .ok_or("envelope sender is not paired")?;
        let signing_key =
            decode_array::<32>("device signing public key", &record.signing_public_key)?;
        let (plaintext, replay_candidate) = {
            let replay = self.replay.lock().map_err(|_| "replay lock failed")?;
            let mut candidate = replay.clone();
            drop(replay);
            let plaintext = open_envelope(
                envelope,
                &signing_key,
                &record.device_id,
                &*self.workspace.identity,
                OffsetDateTime::now_utc(),
                &mut candidate,
            )?;
            (plaintext, candidate)
        };
        let payload = PhoneToMacPayload::from_canonical_slice(&plaintext)?;
        let result = match payload {
            PhoneToMacPayload::Command(command) => {
                let result = ProcessedEnvelope::Command(self.process_command(command).await?);
                self.record_admitted_envelope(envelope, &result)?;
                result
            }
            PhoneToMacPayload::ReferenceBlobChunk(chunk) => {
                if record.revoked {
                    return Err("revoked devices cannot upload reference chunks".into());
                }
                let _publication = self
                    .inbox_publications
                    .lock()
                    .map_err(|_| "companion inbox publication lock failed")?;
                let receipt =
                    persist_reference_chunk(&self.service_root, &record.device_id, chunk)?;
                track_reference_envelope(
                    &self.service_root,
                    &record.device_id,
                    &receipt.blob_id,
                    envelope,
                )?;
                let result = ProcessedEnvelope::ReferenceChunk(receipt);
                record_admitted_envelope_at(&self.service_root, envelope, &result)?;
                result
            }
        };
        *self.replay.lock().map_err(|_| "replay lock failed")? = replay_candidate;
        let mut updated = record;
        updated.last_seen = now()?;
        self.store_device(&updated)?;
        Ok(result)
    }

    pub async fn process_command(
        &self,
        command: CompanionCommand,
    ) -> Result<CommandReceipt, BoxError> {
        let Some(record) = self.load_device(&command.body.author_device_id)? else {
            return Ok(rejection(&command.body.command_id, "device_not_paired"));
        };
        if record.revoked {
            return Ok(rejection(&command.body.command_id, "device_revoked"));
        }
        let signing_key =
            decode_array::<32>("device signing public key", &record.signing_public_key)?;
        let mut registry = CapabilityRegistry::new(self.workspace.record.workspace_id.clone())?;
        registry.restore_device_epoch(&record.device_id, record.revocation_epoch)?;
        if registry
            .authorize_command(
                &record.capability,
                &command,
                &signing_key,
                &self.workspace.identity.signing_public_key(),
                OffsetDateTime::now_utc(),
            )
            .is_err()
        {
            return Ok(rejection(&command.body.command_id, "capability_rejected"));
        }
        if let Some(existing) = processed_command_result_at(&self.service_root, &command)? {
            self.cleanup_processed_command_references(&existing)?;
            return Ok(existing.receipt);
        }
        {
            let _publication = self
                .inbox_publications
                .lock()
                .map_err(|_| "companion inbox publication lock failed")?;
            let path = processed_command_path(&self.service_root, &command.body.command_id)?;
            ensure_bounded_record_store_capacity(
                path.parent().ok_or("processed command has no parent")?,
                MAX_PROCESSED_COMMAND_RECORDS,
                &path,
                "processed-command store",
            )?;
        }

        let result = self.process_command_uncached(command.clone()).await?;
        let persisted = {
            let _publication = self
                .inbox_publications
                .lock()
                .map_err(|_| "companion inbox publication lock failed")?;
            record_processed_command_at(&self.service_root, &command, &result)?
        };
        self.cleanup_processed_command_references(&persisted)?;
        Ok(result)
    }

    async fn process_command_uncached(
        &self,
        command: CompanionCommand,
    ) -> Result<CommandReceipt, BoxError> {
        let Some(record) = self.load_device(&command.body.author_device_id)? else {
            return Ok(rejection(&command.body.command_id, "device_not_paired"));
        };
        if record.revoked {
            return Ok(rejection(&command.body.command_id, "device_revoked"));
        }
        let signing_key =
            decode_array::<32>("device signing public key", &record.signing_public_key)?;
        let mut registry = CapabilityRegistry::new(self.workspace.record.workspace_id.clone())?;
        registry.restore_device_epoch(&record.device_id, record.revocation_epoch)?;
        if registry
            .authorize_command(
                &record.capability,
                &command,
                &signing_key,
                &self.workspace.identity.signing_public_key(),
                OffsetDateTime::now_utc(),
            )
            .is_err()
        {
            return Ok(rejection(&command.body.command_id, "capability_rejected"));
        }
        let command_id = command.body.command_id.clone();
        let signature = Some(command.signature.clone());
        let submitted_at = Some(command.body.created_at.clone());
        let result = match command.body.payload {
            CommandPayload::WorkspaceSnapshotRequest => {
                let (snapshot_version, _) = self
                    .publish_workspace_snapshot(
                        &record,
                        &decode_array::<32>(
                            "device agreement public key",
                            &record.agreement_public_key,
                        )?,
                    )
                    .await?;
                CommandReceipt {
                    schema: "tohseno.companion-command-receipt/1".into(),
                    command_id,
                    state: ReceiptState::Completed,
                    shot_id: None,
                    execution_id: None,
                    result_id: Some(format!("snapshot_{snapshot_version}")),
                    rejection_code: None,
                }
            }
            CommandPayload::FeedbackSubmit {
                shot_id,
                expression_id,
                version_id,
                version_ordinal,
                body,
            } => {
                let Some((name, parsed_shot)) = self.resolve_shot(&shot_id)? else {
                    return Ok(rejection(&command_id, "unknown_shot"));
                };
                let parsed_expression = match parse_expression_id(&expression_id) {
                    Ok(value) => value,
                    Err(_) => return Ok(rejection(&command_id, "invalid_lineage_identity")),
                };
                let parsed_version = match parse_version_id(&version_id) {
                    Ok(value) => value,
                    Err(_) => return Ok(rejection(&command_id, "invalid_lineage_identity")),
                };
                // Feedback remains attached to the exact Version the owner
                // experienced. It need not be the newest accepted Version;
                // unlike evolution, there is no silent rebase as long as the
                // named immutable Version still exists in this Expression.
                let reviewed = match self
                    .application
                    .engine()
                    .accepted_version_base(&name, version_ordinal)
                {
                    Ok(reviewed) => reviewed,
                    Err(_) => return Ok(rejection(&command_id, "unknown_reviewed_version")),
                };
                if reviewed.expression_id != parsed_expression
                    || reviewed.version_id != parsed_version
                    || reviewed.version_ordinal != version_ordinal
                {
                    return Ok(rejection(&command_id, "unknown_reviewed_version"));
                }
                let receipt = match self
                    .application
                    .submit_feedback(SubmitFeedbackCommand {
                        command_id: command_id.clone(),
                        origin: CommandOrigin::Companion,
                        origin_device_id: Some(record.device_id.clone()),
                        name,
                        shot_id: parsed_shot,
                        expression_id: parsed_expression,
                        version_id: parsed_version,
                        version_ordinal,
                        body,
                        companion_signature: signature,
                        submitted_at,
                    })
                    .await
                {
                    Ok(receipt) => receipt,
                    Err(error) => return self.application_failure(&command_id, error),
                };
                CommandReceipt {
                    schema: "tohseno.companion-command-receipt/1".into(),
                    command_id,
                    state: ReceiptState::Completed,
                    shot_id: Some(receipt.shot_id.to_string()),
                    execution_id: None,
                    result_id: Some(receipt.action_commitment.to_string()),
                    rejection_code: None,
                }
            }
            CommandPayload::MarketingSubmit {
                note_id: _,
                shot_id,
                body,
            } => {
                let Some((name, parsed_shot)) = self.resolve_shot(&shot_id)? else {
                    return Ok(rejection(&command_id, "unknown_shot"));
                };
                let receipt = match self
                    .application
                    .submit_marketing_note(SubmitMarketingNoteCommand {
                        command_id: command_id.clone(),
                        origin: CommandOrigin::Companion,
                        origin_device_id: Some(record.device_id.clone()),
                        name,
                        shot_id: parsed_shot,
                        body,
                        companion_signature: signature,
                        submitted_at,
                    })
                    .await
                {
                    Ok(receipt) => receipt,
                    Err(error) => return self.application_failure(&command_id, error),
                };
                CommandReceipt {
                    schema: "tohseno.companion-command-receipt/1".into(),
                    command_id,
                    state: ReceiptState::Completed,
                    shot_id: Some(receipt.shot_id.to_string()),
                    execution_id: None,
                    result_id: Some(receipt.note_id),
                    rejection_code: None,
                }
            }
            CommandPayload::ShotEvolveRequest {
                shot_id,
                base_expression_id,
                base_version_id,
                base_version_ordinal,
                intention,
                selected_feedback_action_commitments,
                references,
            } => {
                let Some((name, parsed_shot)) = self.resolve_shot(&shot_id)? else {
                    return Ok(rejection(&command_id, "unknown_shot"));
                };
                let parsed_expression = match parse_expression_id(&base_expression_id) {
                    Ok(value) => value,
                    Err(_) => return Ok(rejection(&command_id, "invalid_lineage_identity")),
                };
                let parsed_version = match parse_version_id(&base_version_id) {
                    Ok(value) => value,
                    Err(_) => return Ok(rejection(&command_id, "invalid_lineage_identity")),
                };
                let current = match self.application.engine().current_accepted_base(&name) {
                    Ok(current) => current,
                    Err(_) => return Ok(rejection(&command_id, "stale_base_version")),
                };
                if current.expression_id != parsed_expression
                    || current.version_id != parsed_version
                    || current.version_ordinal != base_version_ordinal
                {
                    return Ok(rejection(&command_id, "stale_base_version"));
                }
                let reference_inputs = match self.reference_inputs(&record, references) {
                    Ok(Some(inputs)) => inputs,
                    Ok(None) => return Ok(rejection(&command_id, "reference_blob_missing")),
                    Err(_) => return Ok(rejection(&command_id, "reference_blob_invalid")),
                };
                let feedback = selected_feedback_action_commitments
                    .iter()
                    .map(|value| {
                        decode_array::<32>("feedback action commitment", value).map(Bytes32::new)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let receipt = match self
                    .application
                    .evolve_shot(EvolveShotCommand {
                        command_id: command_id.clone(),
                        origin: CommandOrigin::Companion,
                        origin_device_id: Some(record.device_id.clone()),
                        name,
                        base_expression_id: parsed_expression,
                        base_version_id: parsed_version,
                        base_version_ordinal,
                        intention,
                        selected_feedback_actions: feedback,
                        references: reference_inputs,
                        submitted_at,
                    })
                    .await
                {
                    Ok(receipt) => receipt,
                    Err(error) => return self.application_failure(&command_id, error),
                };
                CommandReceipt {
                    schema: "tohseno.companion-command-receipt/1".into(),
                    command_id,
                    state: ReceiptState::Accepted,
                    shot_id: Some(parsed_shot.to_string()),
                    execution_id: Some(receipt.execution_id),
                    result_id: None,
                    rejection_code: None,
                }
            }
            CommandPayload::ShotCreateRequest {
                suggested_name,
                intention,
                references,
            } => {
                let name = suggested_name
                    .map(|name| name.to_ascii_lowercase())
                    .unwrap_or_else(|| {
                        format!("shot-{}", command_id.chars().take(8).collect::<String>())
                    });
                if tohseno_engine::ledger::validate_app_name(&name).is_err() {
                    return Ok(rejection(&command_id, "invalid_shot_name"));
                }
                let reference_inputs = match self.reference_inputs(&record, references) {
                    Ok(Some(inputs)) => inputs,
                    Ok(None) => return Ok(rejection(&command_id, "reference_blob_missing")),
                    Err(_) => return Ok(rejection(&command_id, "reference_blob_invalid")),
                };
                let receipt = match self
                    .application
                    .create_shot(CreateShotCommand {
                        command_id: command_id.clone(),
                        origin: CommandOrigin::Companion,
                        origin_device_id: Some(record.device_id.clone()),
                        name,
                        intention,
                        references: reference_inputs,
                        submitted_at,
                    })
                    .await
                {
                    Ok(receipt) => receipt,
                    Err(error) => return self.application_failure(&command_id, error),
                };
                CommandReceipt {
                    schema: "tohseno.companion-command-receipt/1".into(),
                    command_id,
                    state: ReceiptState::Accepted,
                    shot_id: Some(receipt.shot_id.to_string()),
                    execution_id: Some(receipt.execution_id),
                    result_id: None,
                    rejection_code: None,
                }
            }
        };
        result.validate()?;
        Ok(result)
    }

    fn application_failure(
        &self,
        command_id: &str,
        error: ApplicationError,
    ) -> Result<CommandReceipt, BoxError> {
        // The application service owns durable command truth. Translate its
        // privacy-safe terminal state into a companion acknowledgement so a
        // permanently rejected/failed command cannot poison mailbox cursor
        // reconciliation forever.
        if matches!(
            &error,
            ApplicationError::Journal(tohseno_application::JournalError::Conflict(_))
        ) {
            return Ok(rejection(command_id, "command_id_conflict"));
        }
        if let Ok((_, status)) = self.application.journal().load(command_id) {
            return Ok(match status.state {
                CommandState::Rejected => rejection(command_id, "command_rejected"),
                CommandState::Failed | CommandState::Cancelled => failure(command_id),
                _ => return Err(error.into()),
            });
        }
        if matches!(&error, ApplicationError::Invalid(_)) {
            return Ok(rejection(command_id, "invalid_command"));
        }
        Err(error.into())
    }

    async fn converted_workspace_for(
        &self,
        record: &DeviceRecord,
    ) -> Result<ConvertedWorkspace, BoxError> {
        let local = self.application.workspace_snapshot().await?;
        convert_snapshot(local, record)
    }

    /// Publish a complete authoritative snapshot as an ordinary encrypted
    /// workspace event. A fresh event cursor also becomes the snapshot version
    /// and establishes the next incremental cursor after a retention reset.
    async fn publish_workspace_snapshot(
        &self,
        record: &DeviceRecord,
        agreement_key: &[u8; 32],
    ) -> Result<(u64, OpaqueEnvelope), BoxError> {
        let event_cursor = self.next_event_cursor(&record.device_id)?;
        let next_cursor = event_cursor
            .checked_add(1)
            .ok_or("companion event cursor overflowed")?;
        let converted = self.converted_workspace_for(record).await?;
        let mut snapshot = converted.snapshot;
        snapshot.snapshot_version = event_cursor;
        snapshot.next_cursor = next_cursor;
        snapshot.validate()?;
        let published_snapshot = snapshot.clone();
        let event = WorkspaceEvent {
            schema: COMPANION_EVENT_SCHEMA.into(),
            event_id: format!("event_{}", compact_uuid()),
            workspace_id: self.workspace.record.workspace_id.clone(),
            cursor: event_cursor,
            emitted_at: now()?,
            payload: WorkspaceEventPayload::WorkspaceSnapshot {
                snapshot: Box::new(snapshot),
            },
        };
        event.validate()?;
        let bytes = tohseno_companion::canonical::to_vec(&event)?;
        let envelope = self.seal_to_device(record, agreement_key, &bytes)?;
        let relay = self
            .relay
            .as_ref()
            .ok_or("Companion Relay is not configured")?;
        relay
            .upload_envelope(
                &record.phone_mailbox_id,
                &record.phone_mailbox_write_capability,
                &envelope,
            )
            .await?;
        let mut projection = published_snapshot;
        for icon_blob in converted.icon_blobs {
            let cursor = self
                .publish_workspace_event(
                    record,
                    agreement_key,
                    WorkspaceEventPayload::IconBlob {
                        blob: Box::new(icon_blob),
                    },
                )
                .await?;
            projection.next_cursor = cursor
                .checked_add(1)
                .ok_or("companion event cursor overflowed")?;
        }
        self.store_workspace_projection(record, &projection)?;
        Ok((event_cursor, envelope))
    }

    /// Project authoritative workspace changes to every active device. This
    /// runs independently of Studio and the submitting CLI, so detached
    /// execution progress remains synchronized after those clients exit.
    pub async fn publish_workspace_changes(&self) -> Result<usize, BoxError> {
        if self.relay.is_none() {
            return Ok(0);
        }
        let devices = self
            .load_devices()?
            .into_iter()
            .filter(|record| !record.revoked)
            .collect::<Vec<_>>();
        let mut published = 0_usize;
        for record in devices {
            let converted = self.converted_workspace_for(&record).await?;
            let current = converted.snapshot;
            let Some(previous) = self.load_workspace_projection(&record)? else {
                let agreement_key = decode_array::<32>(
                    "device agreement public key",
                    &record.agreement_public_key,
                )?;
                self.publish_workspace_snapshot(&record, &agreement_key)
                    .await?;
                published += 1;
                continue;
            };
            let payloads = workspace_change_payloads(&previous, &current)?;
            if payloads.is_empty() {
                continue;
            }
            let agreement_key =
                decode_array::<32>("device agreement public key", &record.agreement_public_key)?;
            let mut next_cursor = previous.next_cursor;
            for payload in payloads {
                let icon_blob_id = match &payload {
                    WorkspaceEventPayload::ShotUpsert { shot } => {
                        shot.icon.as_ref().map(|icon| icon.blob_id.clone())
                    }
                    _ => None,
                };
                let cursor = self
                    .publish_workspace_event(&record, &agreement_key, payload)
                    .await?;
                next_cursor = cursor
                    .checked_add(1)
                    .ok_or("companion event cursor overflowed")?;
                published += 1;
                if let Some(blob_id) = icon_blob_id {
                    let blob = converted
                        .icon_blobs
                        .iter()
                        .find(|blob| blob.blob_id == blob_id)
                        .ok_or("Shot icon descriptor has no private blob")?
                        .clone();
                    let cursor = self
                        .publish_workspace_event(
                            &record,
                            &agreement_key,
                            WorkspaceEventPayload::IconBlob {
                                blob: Box::new(blob),
                            },
                        )
                        .await?;
                    next_cursor = cursor
                        .checked_add(1)
                        .ok_or("companion event cursor overflowed")?;
                    published += 1;
                }
            }
            let mut projection = current;
            projection.next_cursor = next_cursor;
            self.store_workspace_projection(&record, &projection)?;
        }
        Ok(published)
    }

    async fn publish_workspace_event(
        &self,
        record: &DeviceRecord,
        agreement_key: &[u8; 32],
        payload: WorkspaceEventPayload,
    ) -> Result<u64, BoxError> {
        let payload_bytes = tohseno_companion::canonical::to_vec(&payload)?;
        let payload_digest = tohseno_protocol::digest::sha256(&payload_bytes).to_string();
        let cursor = self.next_event_cursor(&record.device_id)?;
        let event = WorkspaceEvent {
            schema: COMPANION_EVENT_SCHEMA.into(),
            event_id: format!(
                "event_{}",
                payload_digest
                    .trim_start_matches("0x")
                    .chars()
                    .take(32)
                    .collect::<String>()
            ),
            workspace_id: self.workspace.record.workspace_id.clone(),
            cursor,
            emitted_at: now()?,
            payload,
        };
        event.validate()?;
        let envelope = self.seal_to_device(
            record,
            agreement_key,
            &tohseno_companion::canonical::to_vec(&event)?,
        )?;
        let relay = self
            .relay
            .as_ref()
            .ok_or("Companion Relay is not configured")?;
        relay
            .upload_envelope(
                &record.phone_mailbox_id,
                &record.phone_mailbox_write_capability,
                &envelope,
            )
            .await?;
        Ok(cursor)
    }

    fn workspace_projection_path(&self, record: &DeviceRecord) -> Result<PathBuf, BoxError> {
        let device_directory = self.service_root.join("outbox").join(&record.device_id);
        if self
            .device_path(&record.device_id)?
            .file_stem()
            .and_then(|value| value.to_str())
            != Some(record.device_id.as_str())
        {
            return Err("invalid workspace-projection device ID".into());
        }
        ensure_private_directory(&device_directory)?;
        Ok(device_directory.join("workspace-projection.json"))
    }

    fn load_workspace_projection(
        &self,
        record: &DeviceRecord,
    ) -> Result<Option<PublishedWorkspaceProjection>, BoxError> {
        let path = self.workspace_projection_path(record)?;
        let bytes = match fs::symlink_metadata(&path) {
            Ok(_) => read_bounded(&path, MAX_RELAY_JSON_BYTES as u64)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let projection: PublishedWorkspaceProjection =
            tohseno_companion::canonical::from_slice(&bytes)?;
        validate_workspace_projection(&projection, record, &self.workspace.record.workspace_id)?;
        Ok(Some(projection))
    }

    fn store_workspace_projection(
        &self,
        record: &DeviceRecord,
        snapshot: &WorkspaceSnapshot,
    ) -> Result<(), BoxError> {
        snapshot.validate()?;
        let projection = PublishedWorkspaceProjection {
            schema: WORKSPACE_PROJECTION_SCHEMA.into(),
            workspace_id: snapshot.workspace_id.clone(),
            service_version: snapshot.service_version.clone(),
            shots: snapshot.shots.clone(),
            active_executions: snapshot.active_executions.clone(),
            device_capability_state: snapshot.device_capability_state.clone(),
            next_cursor: snapshot.next_cursor,
        };
        validate_workspace_projection(&projection, record, &self.workspace.record.workspace_id)?;
        write_replace(
            &self.workspace_projection_path(record)?,
            &tohseno_companion::canonical::to_vec(&projection)?,
            0o600,
        )
    }

    async fn publish_command_receipt(
        &self,
        record: &DeviceRecord,
        receipt: CommandReceipt,
    ) -> Result<OpaqueEnvelope, BoxError> {
        receipt.validate()?;
        let event_cursor = self.next_event_cursor(&record.device_id)?;
        let payload = if receipt.state == ReceiptState::Rejected {
            WorkspaceEventPayload::CommandRejected { receipt }
        } else {
            WorkspaceEventPayload::CommandAcknowledged { receipt }
        };
        let event = WorkspaceEvent {
            schema: COMPANION_EVENT_SCHEMA.into(),
            event_id: format!("event_{}", compact_uuid()),
            workspace_id: self.workspace.record.workspace_id.clone(),
            cursor: event_cursor,
            emitted_at: now()?,
            payload,
        };
        event.validate()?;
        let agreement_key =
            decode_array::<32>("device agreement public key", &record.agreement_public_key)?;
        let envelope = self.seal_to_device(
            record,
            &agreement_key,
            &tohseno_companion::canonical::to_vec(&event)?,
        )?;
        let relay = self
            .relay
            .as_ref()
            .ok_or("Companion Relay is not configured")?;
        relay
            .upload_envelope(
                &record.phone_mailbox_id,
                &record.phone_mailbox_write_capability,
                &envelope,
            )
            .await?;
        Ok(envelope)
    }

    fn resolve_shot(&self, value: &str) -> Result<Option<(String, ShotId)>, BoxError> {
        let parsed = match parse_shot_id(value) {
            Ok(value) => value,
            Err(_) => return Ok(None),
        };
        for app in self.application.engine().ledger().list_apps()? {
            if app.shot_id == Some(parsed) {
                return Ok(Some((app.name, parsed)));
            }
        }
        Ok(None)
    }

    fn reference_inputs(
        &self,
        record: &DeviceRecord,
        descriptors: Vec<ReferenceDescriptor>,
    ) -> Result<Option<Vec<ReferenceInput>>, BoxError> {
        resolve_reference_inputs(&self.service_root, &record.device_id, descriptors)
    }

    fn cleanup_processed_command_references(
        &self,
        record: &ProcessedCommandRecord,
    ) -> Result<(), BoxError> {
        let _publication = self
            .inbox_publications
            .lock()
            .map_err(|_| "companion inbox publication lock failed")?;
        for blob_id in &record.reference_blob_ids {
            remove_consumed_reference_blob(&self.service_root, &record.origin_device_id, blob_id)?;
        }
        Ok(())
    }

    fn seal_to_device(
        &self,
        record: &DeviceRecord,
        agreement_key: &[u8; 32],
        bytes: &[u8],
    ) -> Result<OpaqueEnvelope, BoxError> {
        let created = OffsetDateTime::now_utc().replace_nanosecond(0)?;
        Ok(seal_envelope(
            &*self.workspace.identity,
            agreement_key,
            EnvelopeMetadata {
                envelope_id: Uuid::new_v4().to_string(),
                mailbox_id: record.phone_mailbox_id.clone(),
                recipient_device_id: record.device_id.clone(),
                sender_sequence: self.next_sender_sequence()?,
                created_at: timestamp(created)?,
                expires_at: timestamp(created + Duration::days(7))?,
            },
            bytes,
        )?)
    }

    fn load_or_create_pairing_acceptance_envelope(
        &self,
        record: &DeviceRecord,
        session_id: &str,
        agreement_key: &[u8; 32],
        bytes: &[u8],
    ) -> Result<OpaqueEnvelope, BoxError> {
        validate_relay_path_id(session_id)?;
        let directory = self.service_root.join("outbox").join(&record.device_id);
        ensure_private_directory(&directory)?;
        let path = directory.join(format!("pairing-{session_id}-acceptance.json"));
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                let envelope: OpaqueEnvelope = tohseno_companion::canonical::from_slice(
                    &read_bounded(&path, MAX_ADMITTED_ENVELOPE_RECORD_BYTES)?,
                )?;
                envelope.validate_relay_shape()?;
                if envelope.header.mailbox_id != record.phone_mailbox_id
                    || envelope.header.recipient_device_id != record.device_id
                {
                    return Err("persisted pairing acceptance envelope is misrouted".into());
                }
                Ok(envelope)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let envelope = self.seal_to_device(record, agreement_key, bytes)?;
                write_new(
                    &path,
                    &tohseno_companion::canonical::to_vec(&envelope)?,
                    0o600,
                )?;
                Ok(envelope)
            }
            Err(error) => Err(error.into()),
        }
    }

    fn next_sender_sequence(&self) -> Result<u64, BoxError> {
        let _guard = self
            .outbox_counters
            .lock()
            .map_err(|_| "outbox counter lock failed")?;
        let path = self.service_root.join("outbox/sender-sequence");
        let current = match fs::symlink_metadata(&path) {
            Ok(_) => String::from_utf8(read_bounded(&path, 64)?)?.parse::<u64>()?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error.into()),
        };
        let next = current.checked_add(1).ok_or("sender sequence overflowed")?;
        write_replace(&path, next.to_string().as_bytes(), 0o600)?;
        Ok(next)
    }

    fn next_event_cursor(&self, device_id: &str) -> Result<u64, BoxError> {
        let _guard = self
            .outbox_counters
            .lock()
            .map_err(|_| "outbox counter lock failed")?;
        let device_directory = self.service_root.join("outbox").join(device_id);
        if self
            .device_path(device_id)?
            .file_stem()
            .and_then(|value| value.to_str())
            != Some(device_id)
        {
            return Err("invalid event-cursor device ID".into());
        }
        ensure_private_directory(&device_directory)?;
        let path = device_directory.join("event-cursor");
        let current = match fs::symlink_metadata(&path) {
            Ok(_) => String::from_utf8(read_bounded(&path, 64)?)?.parse::<u64>()?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error.into()),
        };
        let next = current.checked_add(1).ok_or("event cursor overflowed")?;
        write_replace(&path, next.to_string().as_bytes(), 0o600)?;
        Ok(next)
    }

    fn admitted_envelope_result(
        &self,
        envelope: &OpaqueEnvelope,
    ) -> Result<Option<ProcessedEnvelope>, BoxError> {
        admitted_envelope_result_at(&self.service_root, envelope)
    }

    fn record_admitted_envelope(
        &self,
        envelope: &OpaqueEnvelope,
        result: &ProcessedEnvelope,
    ) -> Result<(), BoxError> {
        let _publication = self
            .inbox_publications
            .lock()
            .map_err(|_| "companion inbox publication lock failed")?;
        record_admitted_envelope_at(&self.service_root, envelope, result)
    }

    fn device_path(&self, device_id: &str) -> Result<PathBuf, BoxError> {
        if device_id.is_empty()
            || device_id.len() > 128
            || !device_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err("invalid device ID".into());
        }
        Ok(self
            .service_root
            .join("devices")
            .join(format!("{device_id}.json")))
    }

    fn load_device(&self, device_id: &str) -> Result<Option<DeviceRecord>, BoxError> {
        let path = self.device_path(device_id)?;
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                let value: DeviceRecord = tohseno_protocol::canonical::from_slice(&read_bounded(
                    &path,
                    MAX_DEVICE_RECORD_BYTES,
                )?)?;
                validate_device_record(&value, &self.workspace)?;
                Ok(Some(value))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn load_devices(&self) -> Result<Vec<DeviceRecord>, BoxError> {
        let mut values = Vec::new();
        for entry in fs::read_dir(self.service_root.join("devices"))?.take(10_000) {
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("device store contains an unsafe entry".into());
            }
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let value: DeviceRecord = tohseno_protocol::canonical::from_slice(&read_bounded(
                &entry.path(),
                MAX_DEVICE_RECORD_BYTES,
            )?)?;
            validate_device_record(&value, &self.workspace)?;
            values.push(value);
        }
        Ok(values)
    }

    fn store_device(&self, value: &DeviceRecord) -> Result<(), BoxError> {
        validate_device_record(value, &self.workspace)?;
        let path = self.device_path(&value.device_id)?;
        let bytes = tohseno_protocol::canonical::to_vec(value)?;
        write_replace(&path, &bytes, 0o600)
    }
}

struct RelayMailboxProvision {
    created: MailboxCreated,
    write_capability: String,
    read_capability: String,
    ack_capability: String,
    revoke_capability: String,
}

enum RelayMailboxFetch {
    Page(MailboxPage),
    Reset(MailboxResetRequired),
}

fn cursor_after_mailbox_reset(current: u64, reset: &MailboxResetRequired) -> Result<u64, BoxError> {
    reset.validate()?;
    if reset.reset_before_cursor <= current {
        return Err("Companion Relay returned a non-advancing mailbox reset".into());
    }
    Ok(reset.reset_before_cursor)
}

impl RelayConfiguration {
    fn from_environment() -> Result<Option<Self>, BoxError> {
        let Some(origin) = std::env::var("TOHSENO_COMPANION_RELAY_ORIGIN")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(None);
        };
        let parsed = reqwest::Url::parse(&origin)?;
        if parsed.username() != ""
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !matches!(parsed.path(), "" | "/")
        {
            return Err(
                "Companion Relay origin must not contain credentials, path, or query".into(),
            );
        }
        let loopback_http = parsed.scheme() == "http"
            && matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
        if parsed.scheme() != "https" && !loopback_http {
            return Err("Companion Relay requires HTTPS except on an exact loopback host".into());
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(15))
            .user_agent(concat!(
                "tohseno-workspace-service/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;
        Ok(Some(Self {
            origin: origin.trim_end_matches('/').into(),
            client,
        }))
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.origin)
    }

    async fn create_pairing_session(
        &self,
        expires_at: &str,
        read_capability: &str,
        cancel_capability: &str,
    ) -> Result<PairingSessionCreated, BoxError> {
        let request = PairingSessionCreate {
            schema: "tohseno.companion-pairing-session-create/1".into(),
            expires_at: expires_at.into(),
            read_verifier: capability_verifier(read_capability)?,
            cancel_verifier: capability_verifier(cancel_capability)?,
        };
        request.validate()?;
        let response = self
            .client
            .post(self.endpoint("/v1/companion/pairing-sessions"))
            .json(&request)
            .send()
            .await?;
        require_status(
            &response,
            &[reqwest::StatusCode::CREATED],
            "create pairing session",
        )?;
        let created: PairingSessionCreated = read_json_bounded(response).await?;
        created.validate()?;
        if created.expires_at != expires_at {
            return Err("Companion Relay changed the pairing expiry".into());
        }
        Ok(created)
    }

    async fn pairing_response(
        &self,
        session_id: &str,
        read_capability: &str,
    ) -> Result<Option<Vec<u8>>, BoxError> {
        validate_relay_path_id(session_id)?;
        let response = self
            .client
            .get(self.endpoint(&format!("/v1/companion/pairing-sessions/{session_id}")))
            .bearer_auth(read_capability)
            .send()
            .await?;
        match response.status() {
            reqwest::StatusCode::NO_CONTENT => Ok(None),
            reqwest::StatusCode::OK => Ok(Some(
                read_response_bounded(response, MAX_RELAY_PAIRING_RESPONSE_BYTES).await?,
            )),
            _ => Err("Companion Relay rejected pairing reconciliation".into()),
        }
    }

    async fn cancel_pairing_session(
        &self,
        session_id: &str,
        cancel_capability: &str,
    ) -> Result<(), BoxError> {
        validate_relay_path_id(session_id)?;
        let response = self
            .client
            .delete(self.endpoint(&format!("/v1/companion/pairing-sessions/{session_id}")))
            .bearer_auth(cancel_capability)
            .send()
            .await?;
        require_status(
            &response,
            &[reqwest::StatusCode::NO_CONTENT],
            "cancel pairing session",
        )
    }

    async fn create_mailbox(&self) -> Result<RelayMailboxProvision, BoxError> {
        let write_capability = random_relay_capability();
        let read_capability = random_relay_capability();
        let ack_capability = random_relay_capability();
        let revoke_capability = random_relay_capability();
        let push_capability = random_relay_capability();
        let request = MailboxCreate {
            schema: "tohseno.companion-mailbox-create/1".into(),
            write_verifier: capability_verifier(&write_capability)?,
            read_verifier: capability_verifier(&read_capability)?,
            ack_verifier: capability_verifier(&ack_capability)?,
            revoke_verifier: capability_verifier(&revoke_capability)?,
            push_verifier: capability_verifier(&push_capability)?,
        };
        request.validate()?;
        let response = self
            .client
            .post(self.endpoint("/v1/companion/mailboxes"))
            .json(&request)
            .send()
            .await?;
        require_status(&response, &[reqwest::StatusCode::CREATED], "create mailbox")?;
        let created: MailboxCreated = read_json_bounded(response).await?;
        created.validate()?;
        Ok(RelayMailboxProvision {
            created,
            write_capability,
            read_capability,
            ack_capability,
            revoke_capability,
        })
    }

    async fn upload_envelope(
        &self,
        mailbox_id: &str,
        write_capability: &str,
        envelope: &OpaqueEnvelope,
    ) -> Result<EnvelopeAccepted, BoxError> {
        validate_relay_path_id(mailbox_id)?;
        envelope.validate_relay_shape()?;
        if envelope.header.mailbox_id != mailbox_id {
            return Err("refusing to upload an envelope to a different mailbox".into());
        }
        let response = self
            .client
            .post(self.endpoint(&format!("/v1/companion/mailboxes/{mailbox_id}/envelopes")))
            .bearer_auth(write_capability)
            .json(envelope)
            .send()
            .await?;
        require_status(
            &response,
            &[reqwest::StatusCode::OK, reqwest::StatusCode::CREATED],
            "upload envelope",
        )?;
        let accepted: EnvelopeAccepted = read_json_bounded(response).await?;
        accepted.validate()?;
        Ok(accepted)
    }

    async fn mailbox_page(
        &self,
        mailbox_id: &str,
        read_capability: &str,
        cursor: u64,
    ) -> Result<RelayMailboxFetch, BoxError> {
        validate_relay_path_id(mailbox_id)?;
        let response = self
            .client
            .get(self.endpoint(&format!("/v1/companion/mailboxes/{mailbox_id}/envelopes")))
            .bearer_auth(read_capability)
            .query(&[("cursor", cursor), ("limit", 1_u64)])
            .send()
            .await?;
        match response.status() {
            reqwest::StatusCode::OK => {
                let page: MailboxPage = read_json_bounded(response).await?;
                page.validate_routing(mailbox_id, cursor)?;
                Ok(RelayMailboxFetch::Page(page))
            }
            reqwest::StatusCode::CONFLICT => {
                let reset: MailboxResetRequired = read_json_bounded(response).await?;
                reset.validate()?;
                Ok(RelayMailboxFetch::Reset(reset))
            }
            _ => Err("Companion Relay rejected mailbox reconciliation".into()),
        }
    }

    async fn acknowledge_mailbox(
        &self,
        mailbox_id: &str,
        acknowledgement_capability: &str,
        cursor: u64,
    ) -> Result<(), BoxError> {
        validate_relay_path_id(mailbox_id)?;
        let request = MailboxAck {
            schema: "tohseno.companion-mailbox-ack/1".into(),
            cursor,
        };
        request.validate()?;
        let response = self
            .client
            .post(self.endpoint(&format!("/v1/companion/mailboxes/{mailbox_id}/ack")))
            .bearer_auth(acknowledgement_capability)
            .json(&request)
            .send()
            .await?;
        require_status(&response, &[reqwest::StatusCode::OK], "acknowledge mailbox")?;
        let acknowledged: MailboxAcknowledged = read_json_bounded(response).await?;
        acknowledged.validate()?;
        if acknowledged.acknowledged_cursor != cursor {
            return Err("Companion Relay acknowledged a different cursor".into());
        }
        Ok(())
    }

    async fn revoke_mailbox(
        &self,
        mailbox_id: &str,
        revoke_capability: &str,
    ) -> Result<MailboxRevoked, BoxError> {
        validate_relay_path_id(mailbox_id)?;
        let response = self
            .client
            .delete(self.endpoint(&format!("/v1/companion/mailboxes/{mailbox_id}")))
            .bearer_auth(revoke_capability)
            .send()
            .await?;
        require_status(&response, &[reqwest::StatusCode::OK], "revoke mailbox")?;
        let revoked: MailboxRevoked = read_json_bounded(response).await?;
        revoked.validate()?;
        Ok(revoked)
    }

    async fn health(&self) -> Result<RelayHealth, BoxError> {
        let response = self.client.get(self.endpoint("/healthz")).send().await?;
        require_status(&response, &[reqwest::StatusCode::OK], "check relay health")?;
        let health: RelayHealth = read_json_bounded(response).await?;
        health.validate()?;
        Ok(health)
    }
}

async fn read_json_bounded<T: DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, BoxError> {
    let bytes = read_response_bounded(response, MAX_RELAY_JSON_BYTES).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

async fn read_response_bounded(
    response: reqwest::Response,
    maximum: usize,
) -> Result<Vec<u8>, BoxError> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err("Companion Relay response exceeds its byte bound".into());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len().saturating_add(chunk.len()) > maximum {
            return Err("Companion Relay response exceeds its byte bound".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn require_status(
    response: &reqwest::Response,
    expected: &[reqwest::StatusCode],
    operation: &str,
) -> Result<(), BoxError> {
    if expected.contains(&response.status()) {
        Ok(())
    } else {
        Err(format!("Companion Relay could not {operation}").into())
    }
}

fn validate_relay_path_id(value: &str) -> Result<(), BoxError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("invalid Companion Relay path identifier".into());
    }
    Ok(())
}

fn random_relay_capability() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    base64url(&bytes)
}

fn validate_device_record(
    value: &DeviceRecord,
    workspace: &WorkspaceIdentity,
) -> Result<(), BoxError> {
    if value.schema != DEVICE_SCHEMA
        || value.device_id != value.capability.body.device_id
        || value.capability.body.workspace_id != workspace.record.workspace_id
        || value.revocation_epoch < value.capability.body.revocation_epoch
        || (value.relay_revocation_complete && !value.revoked)
    {
        return Err("paired device record is invalid".into());
    }
    decode_array::<32>("device signing public key", &value.signing_public_key)?;
    decode_array::<32>("device agreement public key", &value.agreement_public_key)?;
    validate_relay_path_id(&value.phone_mailbox_id)?;
    validate_relay_path_id(&value.studio_mailbox_id)?;
    for capability in [
        &value.phone_mailbox_write_capability,
        &value.phone_mailbox_revoke_capability,
        &value.studio_mailbox_write_capability,
        &value.studio_mailbox_read_capability,
        &value.studio_mailbox_ack_capability,
        &value.studio_mailbox_revoke_capability,
    ] {
        tohseno_companion::relay_client::validate_bearer_capability(capability)?;
    }
    value.capability.verify(
        &workspace.identity.signing_public_key(),
        OffsetDateTime::now_utc(),
    )?;
    tohseno_companion::parse_timestamp(&value.paired_at)?;
    tohseno_companion::parse_timestamp(&value.last_seen)?;
    Ok(())
}

fn convert_snapshot(
    local: LocalWorkspaceSnapshot,
    record: &DeviceRecord,
) -> Result<ConvertedWorkspace, BoxError> {
    let mut shots = Vec::with_capacity(local.shots.len());
    let mut icon_blobs = BTreeMap::new();
    for shot in local.shots {
        let kind = match shot.kind {
            tohseno_application::ShotKind::FactoryShot => ShotKind::FactoryShot,
            tohseno_application::ShotKind::RecordingOnly => ShotKind::RecordingOnly,
        };
        let execution = shot.execution.map(convert_execution).transpose()?;
        let icon_blob = validated_icon_blob(shot.icon)?;
        let icon = icon_blob.descriptor()?;
        let icon_revision = icon.revision;
        match icon_blobs.entry(icon_blob.blob_id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(icon_blob);
            }
            std::collections::btree_map::Entry::Occupied(entry) if entry.get() != &icon_blob => {
                return Err("icon blob identifier collision".into());
            }
            std::collections::btree_map::Entry::Occupied(_) => {}
        }
        let has_accepted_version = shot.expression_id.is_some() && shot.latest_version_id.is_some();
        let supported_companion_actions = if kind == ShotKind::FactoryShot {
            let mut actions = vec![CapabilityAction::WorkspaceRead];
            if has_accepted_version {
                actions.push(CapabilityAction::FeedbackWrite);
            }
            actions.push(CapabilityAction::MarketingWrite);
            if has_accepted_version {
                actions.push(CapabilityAction::ShotEvolve);
            }
            actions
        } else {
            vec![CapabilityAction::WorkspaceRead]
        };
        shots.push(ShotSummary {
            shot_id: shot.shot_id,
            display_name: shot.display_name,
            bundle_identifier: shot.bundle_identifier,
            kind,
            icon: Some(icon),
            icon_revision,
            expression_id: shot.expression_id.map(|value| value.to_string()),
            latest_version_id: shot.latest_version_id.map(|value| value.to_string()),
            latest_version_ordinal: shot.latest_version_ordinal,
            latest_version_created_at: shot.latest_version_created_at,
            execution,
            archived: shot.archived,
            retired: shot.retired,
            sort_index: i64::try_from(shot.sort_index).unwrap_or(i64::MAX),
            supported_companion_actions,
        });
    }
    let active_executions = local
        .active_executions
        .into_iter()
        .map(convert_execution)
        .collect::<Result<Vec<_>, _>>()?;
    let snapshot = WorkspaceSnapshot {
        schema: WORKSPACE_SNAPSHOT_SCHEMA.into(),
        workspace_id: local.workspace_id,
        snapshot_version: local.snapshot_version,
        generated_at: local.generated_at,
        service_version: env!("CARGO_PKG_VERSION").into(),
        shots,
        active_executions,
        device_capability_state: DeviceCapabilityState {
            device_id: record.device_id.clone(),
            capability_id: record.capability.body.capability_id.clone(),
            revocation_epoch: record.revocation_epoch,
            allowed_actions: record.capability.body.allowed_actions.clone(),
            revoked: record.revoked,
        },
        next_cursor: local.next_cursor.max(1),
    };
    snapshot.validate()?;
    Ok(ConvertedWorkspace {
        snapshot,
        icon_blobs: icon_blobs.into_values().collect(),
    })
}

struct ConvertedWorkspace {
    snapshot: WorkspaceSnapshot,
    icon_blobs: Vec<IconBlob>,
}

fn validated_icon_blob(icon: tohseno_application::IconDescriptor) -> Result<IconBlob, BoxError> {
    let revision = revision_number(&icon.revision);
    match IconBlob::new(
        icon.blob_id,
        revision,
        icon.media_type,
        icon.placeholder,
        &icon.private_bytes,
    ) {
        Ok(blob) => Ok(blob),
        Err(_) => {
            // A malformed or overlarge user-controlled asset is never sent.
            // Every recipient gets the same deterministic branded PNG instead.
            let bytes = include_bytes!("../../brand/logos/tohseno-app-icon-1024.png");
            let digest = tohseno_protocol::digest::sha256(bytes).to_string();
            Ok(IconBlob::new(
                digest.clone(),
                revision_number(&digest),
                "image/png",
                true,
                bytes,
            )?)
        }
    }
}

fn validate_workspace_projection(
    projection: &PublishedWorkspaceProjection,
    record: &DeviceRecord,
    workspace_id: &str,
) -> Result<(), BoxError> {
    if projection.schema != WORKSPACE_PROJECTION_SCHEMA
        || projection.workspace_id != workspace_id
        || projection.service_version != env!("CARGO_PKG_VERSION")
        || projection.device_capability_state.device_id != record.device_id
        || projection.next_cursor == 0
        || projection.shots.len() > tohseno_companion::snapshot::MAX_SHOTS_PER_SNAPSHOT
        || projection.active_executions.len()
            > tohseno_companion::snapshot::MAX_EXECUTIONS_PER_SNAPSHOT
    {
        return Err("persisted workspace projection is invalid".into());
    }
    projection.device_capability_state.validate()?;
    let mut shot_ids = std::collections::BTreeSet::new();
    for shot in &projection.shots {
        shot.validate()?;
        if !shot_ids.insert(shot.shot_id.as_str()) {
            return Err("persisted workspace projection contains duplicate Shots".into());
        }
    }
    let mut execution_ids = std::collections::BTreeSet::new();
    for execution in &projection.active_executions {
        execution.validate()?;
        if matches!(
            execution.state,
            ExecutionStatus::Accepted | ExecutionStatus::Failed
        ) || !execution_ids.insert(execution.execution_id.as_str())
        {
            return Err("persisted workspace projection contains invalid active executions".into());
        }
    }
    Ok(())
}

fn workspace_change_payloads(
    previous: &PublishedWorkspaceProjection,
    current: &WorkspaceSnapshot,
) -> Result<Vec<WorkspaceEventPayload>, BoxError> {
    current.validate()?;
    if previous.workspace_id != current.workspace_id
        || previous.device_capability_state.device_id != current.device_capability_state.device_id
    {
        return Err("workspace projection identity changed unexpectedly".into());
    }
    let previous_shots = previous
        .shots
        .iter()
        .map(|shot| (shot.shot_id.as_str(), shot))
        .collect::<BTreeMap<_, _>>();
    let current_shots = current
        .shots
        .iter()
        .map(|shot| (shot.shot_id.as_str(), shot))
        .collect::<BTreeMap<_, _>>();
    let mut payloads = Vec::new();
    for shot in &current.shots {
        let prior = previous_shots.get(shot.shot_id.as_str()).copied();
        if prior != Some(shot) {
            payloads.push(WorkspaceEventPayload::ShotUpsert {
                shot: Box::new(shot.clone()),
            });
        }
        if prior.is_some_and(|prior| !prior.archived) && shot.archived {
            payloads.push(WorkspaceEventPayload::ShotArchive {
                shot_id: shot.shot_id.clone(),
            });
        }
        if prior.and_then(|prior| prior.latest_version_id.as_deref())
            != shot.latest_version_id.as_deref()
        {
            if let (
                Some(expression_id),
                Some(version_id),
                Some(version_ordinal),
                Some(accepted_at),
            ) = (
                shot.expression_id.clone(),
                shot.latest_version_id.clone(),
                shot.latest_version_ordinal,
                shot.latest_version_created_at.clone(),
            ) {
                payloads.push(WorkspaceEventPayload::VersionAccepted {
                    shot_id: shot.shot_id.clone(),
                    expression_id,
                    version_id,
                    version_ordinal,
                    accepted_at,
                });
            }
        }
    }
    for shot in &previous.shots {
        if !current_shots.contains_key(shot.shot_id.as_str()) {
            payloads.push(WorkspaceEventPayload::ShotRemove {
                shot_id: shot.shot_id.clone(),
            });
        }
    }

    let previous_executions = previous
        .active_executions
        .iter()
        .map(|execution| (execution.execution_id.as_str(), execution))
        .collect::<BTreeMap<_, _>>();
    let current_executions = current
        .active_executions
        .iter()
        .map(|execution| (execution.execution_id.as_str(), execution))
        .collect::<BTreeMap<_, _>>();
    for execution in &current.active_executions {
        if previous_executions
            .get(execution.execution_id.as_str())
            .is_some_and(|previous| *previous == execution)
        {
            continue;
        }
        let payload = match execution.state {
            ExecutionStatus::Queued => WorkspaceEventPayload::ExecutionQueued {
                execution: execution.clone(),
            },
            ExecutionStatus::Planning
            | ExecutionStatus::Conception
            | ExecutionStatus::Materializing => WorkspaceEventPayload::ExecutionStarted {
                execution: execution.clone(),
            },
            ExecutionStatus::WaitingForDevice => WorkspaceEventPayload::ExecutionWaitingForDevice {
                execution: execution.clone(),
            },
            ExecutionStatus::Accepted => WorkspaceEventPayload::ExecutionCompleted {
                execution: execution.clone(),
            },
            ExecutionStatus::Failed => WorkspaceEventPayload::ExecutionFailed {
                execution: execution.clone(),
            },
            ExecutionStatus::Building
            | ExecutionStatus::Testing
            | ExecutionStatus::Verifying
            | ExecutionStatus::Repairing
            | ExecutionStatus::Installing
            | ExecutionStatus::Launching => WorkspaceEventPayload::ExecutionUpdated {
                execution: execution.clone(),
            },
        };
        payloads.push(payload);
    }
    for execution in &previous.active_executions {
        if current_executions.contains_key(execution.execution_id.as_str()) {
            continue;
        }
        let current_shot = current_shots.get(execution.shot_id.as_str()).copied();
        // A Shot upsert carrying the newly accepted Version can be published
        // one reconciliation before the execution leaves the active list.
        // Prefer the authoritative terminal execution retained on the Shot;
        // comparing Version IDs alone would misreport that second transition
        // as a failure.
        let accepted = current_shot
            .and_then(|shot| shot.execution.as_ref())
            .filter(|terminal| terminal.execution_id == execution.execution_id)
            .and_then(|terminal| match terminal.state {
                ExecutionStatus::Accepted => Some(true),
                ExecutionStatus::Failed => Some(false),
                _ => None,
            })
            .unwrap_or_else(|| {
                current_shot.and_then(|shot| shot.latest_version_id.as_deref())
                    != previous_shots
                        .get(execution.shot_id.as_str())
                        .and_then(|shot| shot.latest_version_id.as_deref())
            });
        let mut terminal = execution.clone();
        terminal.updated_at = current.generated_at.clone();
        terminal.state = if accepted {
            ExecutionStatus::Accepted
        } else {
            ExecutionStatus::Failed
        };
        terminal.failure_code = (!accepted).then(|| "execution_failed".into());
        payloads.push(if accepted {
            WorkspaceEventPayload::ExecutionCompleted {
                execution: terminal,
            }
        } else {
            WorkspaceEventPayload::ExecutionFailed {
                execution: terminal,
            }
        });
    }
    Ok(payloads)
}

fn convert_execution(
    value: tohseno_application::ExecutionSummary,
) -> Result<ExecutionSummary, BoxError> {
    let state = match value.state.as_str() {
        "queued" => ExecutionStatus::Queued,
        "planning" => ExecutionStatus::Planning,
        "conception" => ExecutionStatus::Conception,
        "materializing" => ExecutionStatus::Materializing,
        "building" => ExecutionStatus::Building,
        "testing" => ExecutionStatus::Testing,
        "verifying" => ExecutionStatus::Verifying,
        "repairing" => ExecutionStatus::Repairing,
        "waiting_for_device" => ExecutionStatus::WaitingForDevice,
        "installing" => ExecutionStatus::Installing,
        "launching" => ExecutionStatus::Launching,
        "accepted" => ExecutionStatus::Accepted,
        _ => ExecutionStatus::Failed,
    };
    Ok(ExecutionSummary {
        execution_id: value.execution_id,
        shot_id: value.shot_id,
        state,
        // Engine execution journals intentionally preserve sub-second local
        // timing. Companion wire timestamps use the stricter exact-second UTC
        // form, so normalize only at this private transport boundary.
        updated_at: timestamp(OffsetDateTime::parse(&value.updated_at, &Rfc3339)?)?,
        failure_code: (state == ExecutionStatus::Failed).then(|| "execution_failed".into()),
    })
}

fn parse_shot_id(value: &str) -> Result<ShotId, BoxError> {
    Ok(ShotId::from_bytes(
        Bytes32::from_hex("Shot ID", value)?.into_bytes(),
    ))
}

fn parse_expression_id(value: &str) -> Result<ExpressionId, BoxError> {
    Ok(ExpressionId::from_bytes(
        Bytes32::from_hex("Expression ID", value)?.into_bytes(),
    ))
}

fn parse_version_id(value: &str) -> Result<VersionId, BoxError> {
    Ok(VersionId::from_bytes(
        Bytes32::from_hex("Version ID", value)?.into_bytes(),
    ))
}

fn rejection(command_id: &str, code: &str) -> CommandReceipt {
    CommandReceipt {
        schema: "tohseno.companion-command-receipt/1".into(),
        command_id: command_id.into(),
        state: ReceiptState::Rejected,
        shot_id: None,
        execution_id: None,
        result_id: None,
        rejection_code: Some(code.into()),
    }
}

fn failure(command_id: &str) -> CommandReceipt {
    CommandReceipt {
        schema: "tohseno.companion-command-receipt/1".into(),
        command_id: command_id.into(),
        state: ReceiptState::Failed,
        shot_id: None,
        execution_id: None,
        result_id: None,
        rejection_code: None,
    }
}

fn all_capabilities() -> Vec<CapabilityAction> {
    vec![
        CapabilityAction::WorkspaceRead,
        CapabilityAction::ExecutionRead,
        CapabilityAction::FeedbackWrite,
        CapabilityAction::MarketingWrite,
        CapabilityAction::ShotCreate,
        CapabilityAction::ShotEvolve,
    ]
}

fn revision_number(value: &str) -> u64 {
    let digest = value.strip_prefix("0x").unwrap_or(value).as_bytes();
    const MODULUS: u64 = (1_u64 << 53) - 1;
    digest
        .iter()
        .take(16)
        .fold(0_u64, |current, byte| {
            (current * 257 + u64::from(*byte)) % MODULUS
        })
        .max(1)
}

fn abbreviate(value: &str) -> String {
    let first = value.chars().take(12).collect::<String>();
    format!("{first}…")
}

fn compact_uuid() -> String {
    Uuid::new_v4().simple().to_string()
}

fn timestamp(value: OffsetDateTime) -> Result<String, time::error::Format> {
    value
        .to_offset(UtcOffset::UTC)
        .replace_nanosecond(0)
        .expect("zero nanoseconds are valid")
        .format(&Rfc3339)
}

fn now() -> Result<String, time::error::Format> {
    timestamp(
        OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("valid"),
    )
}

fn admitted_envelope_path(
    service_root: &Path,
    envelope: &OpaqueEnvelope,
) -> Result<PathBuf, BoxError> {
    admitted_envelope_path_by_id(service_root, &envelope.header.envelope_id)
}

fn admitted_envelope_path_by_id(
    service_root: &Path,
    envelope_id: &str,
) -> Result<PathBuf, BoxError> {
    validate_reference_blob_id(envelope_id)?;
    Ok(service_root
        .join("inbox/envelopes")
        .join(format!("{envelope_id}.json")))
}

fn admitted_envelope_digest(envelope: &OpaqueEnvelope) -> Result<String, BoxError> {
    Ok(
        tohseno_protocol::digest::sha256(&tohseno_companion::canonical::to_vec(envelope)?)
            .to_string(),
    )
}

fn admitted_envelope_result_at(
    service_root: &Path,
    envelope: &OpaqueEnvelope,
) -> Result<Option<ProcessedEnvelope>, BoxError> {
    let path = admitted_envelope_path(service_root, envelope)?;
    let bytes = match fs::symlink_metadata(&path) {
        Ok(_) => read_bounded(&path, MAX_ADMITTED_ENVELOPE_RECORD_BYTES)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let record: AdmittedEnvelopeRecord = tohseno_companion::canonical::from_slice(&bytes)?;
    if record.schema != "tohseno.companion-admitted-envelope/1"
        || record.envelope_id != envelope.header.envelope_id
        || record.envelope_digest != admitted_envelope_digest(envelope)?
    {
        return Err("companion envelope ID was reused with different bytes".into());
    }
    validate_processed_envelope(&record.result)?;
    Ok(Some(record.result))
}

fn record_admitted_envelope_at(
    service_root: &Path,
    envelope: &OpaqueEnvelope,
    result: &ProcessedEnvelope,
) -> Result<(), BoxError> {
    validate_processed_envelope(result)?;
    let path = admitted_envelope_path(service_root, envelope)?;
    ensure_private_directory(path.parent().ok_or("admitted envelope has no parent")?)?;
    let record = AdmittedEnvelopeRecord {
        schema: "tohseno.companion-admitted-envelope/1".into(),
        envelope_id: envelope.header.envelope_id.clone(),
        envelope_digest: admitted_envelope_digest(envelope)?,
        result: result.clone(),
    };
    let bytes = tohseno_companion::canonical::to_vec(&record)?;
    ensure_bounded_record_store_capacity(
        path.parent().ok_or("admitted envelope has no parent")?,
        MAX_ADMITTED_ENVELOPE_RECORDS,
        &path,
        "admitted-envelope store",
    )?;
    match write_new_atomic(&path, &bytes, 0o600) {
        Ok(()) => Ok(()),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|value| value.kind() == std::io::ErrorKind::AlreadyExists) =>
        {
            let existing = admitted_envelope_result_at(service_root, envelope)?
                .ok_or("admitted envelope record disappeared")?;
            if existing == *result {
                Ok(())
            } else {
                Err("companion envelope has a different durable result".into())
            }
        }
        Err(error) => Err(error),
    }
}

fn processed_command_path(service_root: &Path, command_id: &str) -> Result<PathBuf, BoxError> {
    validate_reference_blob_id(command_id)?;
    Ok(service_root
        .join("inbox/commands")
        .join(format!("{command_id}.json")))
}

fn command_reference_blob_ids(command: &CompanionCommand) -> Vec<String> {
    match &command.body.payload {
        CommandPayload::ShotEvolveRequest { references, .. }
        | CommandPayload::ShotCreateRequest { references, .. } => references
            .iter()
            .map(|reference| reference.blob_id.clone())
            .collect(),
        _ => Vec::new(),
    }
}

fn processed_command_digest(command: &CompanionCommand) -> Result<String, BoxError> {
    Ok(base64url(&command.payload_digest()?))
}

fn validate_processed_command_record(record: &ProcessedCommandRecord) -> Result<(), BoxError> {
    if record.schema != "tohseno.companion-processed-command/1"
        || record.receipt.command_id != record.command_id
        || record.reference_blob_ids.len() > 8
    {
        return Err("invalid processed companion command record".into());
    }
    validate_reference_blob_id(&record.command_id)?;
    validate_reference_device_id(&record.origin_device_id)?;
    decode_array::<32>("processed command digest", &record.command_digest)?;
    record.receipt.validate()?;
    let mut unique = record.reference_blob_ids.clone();
    for blob_id in &unique {
        validate_reference_blob_id(blob_id)?;
    }
    unique.sort();
    unique.dedup();
    if unique.len() != record.reference_blob_ids.len() {
        return Err("processed companion command references are not unique".into());
    }
    Ok(())
}

fn processed_command_result_at(
    service_root: &Path,
    command: &CompanionCommand,
) -> Result<Option<ProcessedCommandRecord>, BoxError> {
    let path = processed_command_path(service_root, &command.body.command_id)?;
    let bytes = match fs::symlink_metadata(&path) {
        Ok(_) => read_bounded(&path, MAX_PROCESSED_COMMAND_RECORD_BYTES)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let record: ProcessedCommandRecord = tohseno_companion::canonical::from_slice(&bytes)?;
    validate_processed_command_record(&record)?;
    if record.command_id != command.body.command_id
        || record.command_digest != processed_command_digest(command)?
        || record.origin_device_id != command.body.author_device_id
        || record.reference_blob_ids != command_reference_blob_ids(command)
    {
        return Err("companion command ID was reused with different signed bytes".into());
    }
    Ok(Some(record))
}

fn record_processed_command_at(
    service_root: &Path,
    command: &CompanionCommand,
    receipt: &CommandReceipt,
) -> Result<ProcessedCommandRecord, BoxError> {
    let record = ProcessedCommandRecord {
        schema: "tohseno.companion-processed-command/1".into(),
        command_id: command.body.command_id.clone(),
        command_digest: processed_command_digest(command)?,
        origin_device_id: command.body.author_device_id.clone(),
        reference_blob_ids: command_reference_blob_ids(command),
        receipt: receipt.clone(),
    };
    validate_processed_command_record(&record)?;
    let path = processed_command_path(service_root, &record.command_id)?;
    ensure_private_directory(path.parent().ok_or("processed command has no parent")?)?;
    ensure_bounded_record_store_capacity(
        path.parent().ok_or("processed command has no parent")?,
        MAX_PROCESSED_COMMAND_RECORDS,
        &path,
        "processed-command store",
    )?;
    let bytes = tohseno_companion::canonical::to_vec(&record)?;
    match write_new_atomic(&path, &bytes, 0o600) {
        Ok(()) => Ok(record),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|value| value.kind() == std::io::ErrorKind::AlreadyExists) =>
        {
            let existing = processed_command_result_at(service_root, command)?
                .ok_or("processed command record disappeared")?;
            if existing == record {
                Ok(existing)
            } else {
                Err("companion command has a different durable receipt".into())
            }
        }
        Err(error) => Err(error),
    }
}

fn ensure_bounded_record_store_capacity(
    root: &Path,
    maximum: usize,
    candidate: &Path,
    label: &'static str,
) -> Result<(), BoxError> {
    require_private_directory(root)?;
    let candidate_exists = match fs::symlink_metadata(candidate) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(format!("{label} candidate is unsafe").into());
        }
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    let mut count = 0_usize;
    for entry in fs::read_dir(root)?.take(maximum + 1) {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| format!("{label} contains a non-UTF-8 entry"))?;
        let identifier = name
            .strip_suffix(".json")
            .ok_or_else(|| format!("{label} contains an unexpected entry"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!("{label} contains an unsafe entry").into());
        }
        validate_reference_blob_id(identifier)?;
        count += 1;
    }
    if count > maximum || (!candidate_exists && count >= maximum) {
        return Err(format!("{label} reached its bounded record limit").into());
    }
    Ok(())
}

fn validate_processed_envelope(value: &ProcessedEnvelope) -> Result<(), BoxError> {
    match value {
        ProcessedEnvelope::Command(receipt) => receipt.validate().map_err(Into::into),
        ProcessedEnvelope::ReferenceChunk(receipt) => {
            if receipt.schema != "tohseno.companion-reference-chunk-receipt/1"
                || receipt.chunk_index >= MAX_REFERENCE_CHUNKS_PER_BLOB as u64
            {
                return Err("invalid durable reference chunk result".into());
            }
            validate_reference_blob_id(&receipt.blob_id)
        }
    }
}

fn validate_reference_device_id(value: &str) -> Result<(), BoxError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("invalid reference-inbox device ID".into());
    }
    Ok(())
}

fn validate_reference_blob_id(value: &str) -> Result<(), BoxError> {
    if value.is_empty()
        || value.len() > 128
        || matches!(value, "." | ".." | ".publication-staging")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err("invalid reference-inbox blob ID".into());
    }
    Ok(())
}

fn require_private_directory(path: &Path) -> Result<(), BoxError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("private reference-inbox directory is unsafe".into());
    }
    Ok(())
}

fn reference_blob_directory(
    service_root: &Path,
    device_id: &str,
    blob_id: &str,
) -> Result<PathBuf, BoxError> {
    validate_reference_device_id(device_id)?;
    validate_reference_blob_id(blob_id)?;
    Ok(service_root
        .join("inbox/blobs")
        .join(device_id)
        .join(blob_id))
}

fn ensure_reference_blob_directory(
    service_root: &Path,
    device_id: &str,
    blob_id: &str,
) -> Result<PathBuf, BoxError> {
    let blobs_root = service_root.join("inbox/blobs");
    ensure_private_directory(&blobs_root)?;
    let device_root = blobs_root.join(device_id);
    validate_reference_device_id(device_id)?;
    ensure_private_directory(&device_root)?;
    let blob_root = reference_blob_directory(service_root, device_id, blob_id)?;
    let blob_exists = match fs::symlink_metadata(&blob_root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("private reference blob directory is unsafe".into());
        }
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };

    let mut count = 0_usize;
    for entry in fs::read_dir(&device_root)?.take(MAX_REFERENCE_BLOBS_PER_DEVICE + 1) {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("reference inbox contains an unsafe entry".into());
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "reference inbox contains a non-UTF-8 blob ID")?;
        if name == ".publication-staging" {
            continue;
        }
        validate_reference_blob_id(&name)?;
        count += 1;
    }
    if count > MAX_REFERENCE_BLOBS_PER_DEVICE
        || (!blob_exists && count >= MAX_REFERENCE_BLOBS_PER_DEVICE)
    {
        return Err("reference inbox reached its per-device blob limit".into());
    }
    if !blob_exists {
        ensure_private_directory(&blob_root)?;
    }
    Ok(blob_root)
}

fn persist_reference_chunk(
    service_root: &Path,
    device_id: &str,
    chunk: ReferenceBlobChunk,
) -> Result<ReferenceChunkReceipt, BoxError> {
    chunk.validate()?;
    let descriptor = chunk.descriptor()?;
    let blob_root = ensure_reference_blob_directory(service_root, device_id, &chunk.blob_id)?;
    let chunks_root = blob_root.join("chunks");
    ensure_private_directory(&chunks_root)?;

    let index = ReferenceBlobIndex {
        schema: "tohseno.companion-reference-blob-index/1".into(),
        device_id: device_id.into(),
        descriptor,
        chunk_count: chunk.chunk_count,
    };
    let index_bytes = tohseno_companion::canonical::to_vec(&index)?;
    store_or_compare_immutable(
        &blob_root.join("index.json"),
        &index_bytes,
        MAX_REFERENCE_INDEX_BYTES,
        "reference blob metadata was reused with different values",
    )?;

    let chunk_path = chunks_root.join(format!("{:020}.json", chunk.chunk_index));
    let chunk_bytes = tohseno_companion::canonical::to_vec(&chunk)?;
    let chunk_was_new = store_or_compare_immutable(
        &chunk_path,
        &chunk_bytes,
        MAX_REFERENCE_CHUNK_RECORD_BYTES,
        "reference chunk index was reused with different bytes",
    )?;

    let mut stored_chunks = Vec::new();
    for entry in fs::read_dir(&chunks_root)?.take(MAX_REFERENCE_CHUNKS_PER_BLOB + 1) {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("reference chunk store contains an unsafe entry".into());
        }
        let value: ReferenceBlobChunk = tohseno_companion::canonical::from_slice(&read_bounded(
            &path,
            MAX_REFERENCE_CHUNK_RECORD_BYTES,
        )?)?;
        value.validate()?;
        let expected_name = format!("{:020}.json", value.chunk_index);
        if path.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str())
            || value.blob_id != chunk.blob_id
        {
            return Err("reference chunk store contains a misindexed entry".into());
        }
        stored_chunks.push(value);
    }
    if stored_chunks.len() > MAX_REFERENCE_CHUNKS_PER_BLOB
        || stored_chunks.len() > chunk.chunk_count as usize
    {
        return Err("reference chunk store exceeds its bounded count".into());
    }
    stored_chunks.sort_by_key(|value| value.chunk_index);

    let mut assembler = ReferenceBlobAssembler::default();
    let mut complete = None;
    for value in stored_chunks {
        if let ChunkAdmission::Complete(blob) = assembler.admit(value)? {
            complete = Some(blob);
        }
    }
    let state = if let Some(blob) = complete {
        let completed_bytes = tohseno_companion::canonical::to_vec(&blob)?;
        store_or_compare_immutable(
            &blob_root.join("completed.json"),
            &completed_bytes,
            MAX_COMPLETED_REFERENCE_RECORD_BYTES,
            "completed reference blob was reused with different bytes",
        )?;
        ReferenceChunkState::Complete
    } else if chunk_was_new {
        ReferenceChunkState::Stored
    } else {
        ReferenceChunkState::Duplicate
    };
    let receipt = ReferenceChunkReceipt {
        schema: "tohseno.companion-reference-chunk-receipt/1".into(),
        blob_id: chunk.blob_id,
        chunk_index: chunk.chunk_index,
        state,
    };
    validate_processed_envelope(&ProcessedEnvelope::ReferenceChunk(receipt.clone()))?;
    Ok(receipt)
}

fn track_reference_envelope(
    service_root: &Path,
    device_id: &str,
    blob_id: &str,
    envelope: &OpaqueEnvelope,
) -> Result<(), BoxError> {
    let blob_root = reference_blob_directory(service_root, device_id, blob_id)?;
    require_private_directory(&service_root.join("inbox/blobs"))?;
    require_private_directory(&service_root.join("inbox/blobs").join(device_id))?;
    require_private_directory(&blob_root)?;
    let links_root = blob_root.join("envelopes");
    ensure_private_directory(&links_root)?;
    let path = links_root.join(format!("{}.json", envelope.header.envelope_id));
    let link = ReferenceEnvelopeLink {
        schema: "tohseno.companion-reference-envelope-link/1".into(),
        envelope_id: envelope.header.envelope_id.clone(),
        envelope_digest: admitted_envelope_digest(envelope)?,
    };
    validate_reference_envelope_link(&link)?;
    ensure_bounded_record_store_capacity(
        &links_root,
        MAX_REFERENCE_ENVELOPES_PER_BLOB,
        &path,
        "reference-envelope link store",
    )?;
    let bytes = tohseno_companion::canonical::to_vec(&link)?;
    store_or_compare_immutable(
        &path,
        &bytes,
        MAX_REFERENCE_ENVELOPE_LINK_BYTES,
        "reference envelope link was reused with different bytes",
    )?;
    Ok(())
}

fn validate_reference_envelope_link(link: &ReferenceEnvelopeLink) -> Result<(), BoxError> {
    if link.schema != "tohseno.companion-reference-envelope-link/1" {
        return Err("invalid reference-envelope link schema".into());
    }
    validate_reference_blob_id(&link.envelope_id)?;
    Bytes32::from_hex("reference envelope digest", &link.envelope_digest)?;
    Ok(())
}

fn admitted_envelope_record_by_id(
    service_root: &Path,
    envelope_id: &str,
) -> Result<Option<AdmittedEnvelopeRecord>, BoxError> {
    let path = admitted_envelope_path_by_id(service_root, envelope_id)?;
    let bytes = match fs::symlink_metadata(&path) {
        Ok(_) => read_bounded(&path, MAX_ADMITTED_ENVELOPE_RECORD_BYTES)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let record: AdmittedEnvelopeRecord = tohseno_companion::canonical::from_slice(&bytes)?;
    if record.schema != "tohseno.companion-admitted-envelope/1" || record.envelope_id != envelope_id
    {
        return Err("invalid admitted companion envelope record".into());
    }
    Bytes32::from_hex("admitted envelope digest", &record.envelope_digest)?;
    validate_processed_envelope(&record.result)?;
    Ok(Some(record))
}

/// Reclaims only the exact private reference-inbox tree owned by one admitted
/// command. All entries are validated before the first unlink, and deletion is
/// leaf-first without following links. Missing paths are accepted so a crash at
/// any deletion boundary can be retried from the durable command receipt.
fn remove_consumed_reference_blob(
    service_root: &Path,
    device_id: &str,
    blob_id: &str,
) -> Result<(), BoxError> {
    let blobs_root = service_root.join("inbox/blobs");
    let device_root = blobs_root.join(device_id);
    let blob_root = reference_blob_directory(service_root, device_id, blob_id)?;
    for directory in [&blobs_root, &device_root] {
        match fs::symlink_metadata(directory) {
            Ok(_) => require_private_directory(directory)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }
    match fs::symlink_metadata(&blob_root) {
        Ok(_) => require_private_directory(&blob_root)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    }

    let index_path = blob_root.join("index.json");
    let completed_path = blob_root.join("completed.json");
    let chunks_root = blob_root.join("chunks");
    let links_root = blob_root.join("envelopes");
    let staging_root = blob_root.join(".publication-staging");
    let mut entry_count = 0_usize;
    for entry in fs::read_dir(&blob_root)?.take(6) {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "reference blob contains a non-UTF-8 entry")?;
        let valid = match name.as_str() {
            "index.json" | "completed.json" => metadata.is_file(),
            "chunks" | "envelopes" | ".publication-staging" => metadata.is_dir(),
            _ => false,
        };
        if metadata.file_type().is_symlink() || !valid {
            return Err("reference blob contains an unsafe or unexpected entry".into());
        }
        entry_count += 1;
    }
    if entry_count > 5 {
        return Err("reference blob contains too many entries".into());
    }

    if fs::symlink_metadata(&index_path).is_ok() {
        let index: ReferenceBlobIndex = tohseno_companion::canonical::from_slice(&read_bounded(
            &index_path,
            MAX_REFERENCE_INDEX_BYTES,
        )?)?;
        if index.schema != "tohseno.companion-reference-blob-index/1"
            || index.device_id != device_id
            || index.descriptor.blob_id != blob_id
            || index.chunk_count == 0
            || index.chunk_count as usize > MAX_REFERENCE_CHUNKS_PER_BLOB
        {
            return Err("reference blob index is invalid during cleanup".into());
        }
    }
    if fs::symlink_metadata(&completed_path).is_ok() {
        let completed: ReferenceBlob = tohseno_companion::canonical::from_slice(&read_bounded(
            &completed_path,
            MAX_COMPLETED_REFERENCE_RECORD_BYTES,
        )?)?;
        completed.validate()?;
        if completed.blob_id != blob_id {
            return Err("completed reference blob is misindexed during cleanup".into());
        }
    }

    let mut chunk_files = Vec::new();
    collect_owned_chunk_files(&chunks_root, blob_id, &mut chunk_files)?;
    let mut staging_files = Vec::new();
    collect_owned_staging_files(&staging_root, &mut staging_files)?;
    let links = collect_reference_envelope_links(service_root, &links_root, blob_id)?;

    // The link is retained until its admitted record has been removed. A
    // restart can therefore finish a partially completed cleanup exactly.
    let envelopes_root = service_root.join("inbox/envelopes");
    for (link_path, link) in &links {
        remove_regular_file_if_present(&admitted_envelope_path_by_id(
            service_root,
            &link.envelope_id,
        )?)?;
        remove_regular_file_if_present(link_path)?;
    }
    for path in chunk_files.iter().chain(staging_files.iter()) {
        remove_regular_file_if_present(path)?;
    }
    remove_regular_file_if_present(&completed_path)?;
    remove_regular_file_if_present(&index_path)?;
    remove_empty_directory_if_present(&links_root)?;
    remove_empty_directory_if_present(&chunks_root)?;
    remove_empty_directory_if_present(&staging_root)?;
    remove_empty_directory_if_present(&blob_root)?;
    sync_directory_if_present(&envelopes_root)?;
    File::open(&device_root)?.sync_all()?;
    Ok(())
}

fn collect_owned_chunk_files(
    chunks_root: &Path,
    blob_id: &str,
    output: &mut Vec<PathBuf>,
) -> Result<(), BoxError> {
    match fs::symlink_metadata(chunks_root) {
        Ok(_) => require_private_directory(chunks_root)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    for entry in fs::read_dir(chunks_root)?.take(MAX_REFERENCE_CHUNKS_PER_BLOB + 1) {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("reference chunk cleanup found an unsafe entry".into());
        }
        let chunk: ReferenceBlobChunk = tohseno_companion::canonical::from_slice(&read_bounded(
            &path,
            MAX_REFERENCE_CHUNK_RECORD_BYTES,
        )?)?;
        chunk.validate()?;
        let expected = format!("{:020}.json", chunk.chunk_index);
        if chunk.blob_id != blob_id
            || path.file_name().and_then(|name| name.to_str()) != Some(expected.as_str())
        {
            return Err("reference chunk cleanup found a misindexed entry".into());
        }
        output.push(path);
    }
    if output.len() > MAX_REFERENCE_CHUNKS_PER_BLOB {
        return Err("reference chunk cleanup exceeded its bounded count".into());
    }
    Ok(())
}

fn collect_owned_staging_files(root: &Path, output: &mut Vec<PathBuf>) -> Result<(), BoxError> {
    match fs::symlink_metadata(root) {
        Ok(_) => require_private_directory(root)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    for entry in fs::read_dir(root)?.take(65) {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "reference publication staging contains non-UTF-8 data")?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || name.len() > 300
            || !name.ends_with(".tmp")
        {
            return Err("reference publication staging contains an unsafe entry".into());
        }
        output.push(path);
    }
    if output.len() > 64 {
        return Err("reference publication staging exceeded its bounded count".into());
    }
    Ok(())
}

fn collect_reference_envelope_links(
    service_root: &Path,
    links_root: &Path,
    blob_id: &str,
) -> Result<Vec<(PathBuf, ReferenceEnvelopeLink)>, BoxError> {
    match fs::symlink_metadata(links_root) {
        Ok(_) => require_private_directory(links_root)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    }
    let mut links = Vec::new();
    for entry in fs::read_dir(links_root)?.take(MAX_REFERENCE_ENVELOPES_PER_BLOB + 1) {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("reference-envelope cleanup found an unsafe entry".into());
        }
        let link: ReferenceEnvelopeLink = tohseno_companion::canonical::from_slice(&read_bounded(
            &path,
            MAX_REFERENCE_ENVELOPE_LINK_BYTES,
        )?)?;
        validate_reference_envelope_link(&link)?;
        if path.file_name().and_then(|name| name.to_str())
            != Some(format!("{}.json", link.envelope_id).as_str())
        {
            return Err("reference-envelope cleanup found a misindexed link".into());
        }
        if let Some(admitted) = admitted_envelope_record_by_id(service_root, &link.envelope_id)? {
            if admitted.envelope_digest != link.envelope_digest
                || !matches!(
                    admitted.result,
                    ProcessedEnvelope::ReferenceChunk(ref receipt) if receipt.blob_id == blob_id
                )
            {
                return Err("reference-envelope link does not match its admitted record".into());
            }
        }
        links.push((path, link));
    }
    if links.len() > MAX_REFERENCE_ENVELOPES_PER_BLOB {
        return Err("reference-envelope cleanup exceeded its bounded count".into());
    }
    Ok(links)
}

fn remove_regular_file_if_present(path: &Path) -> Result<(), BoxError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("refusing to remove an unsafe private-store entry".into())
        }
        Ok(_) => {
            fs::remove_file(path)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn remove_empty_directory_if_present(path: &Path) -> Result<(), BoxError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err("refusing to remove an unsafe private-store directory".into())
        }
        Ok(_) => {
            fs::remove_dir(path)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn sync_directory_if_present(path: &Path) -> Result<(), BoxError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err("private-store sync target is unsafe".into())
        }
        Ok(_) => {
            File::open(path)?.sync_all()?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn load_completed_reference_blob(
    service_root: &Path,
    device_id: &str,
    blob_id: &str,
) -> Result<Option<ReferenceBlob>, BoxError> {
    let blob_root = reference_blob_directory(service_root, device_id, blob_id)?;
    let blobs_root = service_root.join("inbox/blobs");
    let device_root = blobs_root.join(device_id);
    for directory in [&blobs_root, &device_root, &blob_root] {
        match fs::symlink_metadata(directory) {
            Ok(_) => require_private_directory(directory)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        }
    }
    let path = blob_root.join("completed.json");
    let bytes = match fs::symlink_metadata(&path) {
        Ok(_) => read_bounded(&path, MAX_COMPLETED_REFERENCE_RECORD_BYTES)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let blob: ReferenceBlob = tohseno_companion::canonical::from_slice(&bytes)?;
    blob.validate()?;
    if blob.blob_id != blob_id {
        return Err("completed reference blob is stored under a different identifier".into());
    }
    Ok(Some(blob))
}

fn resolve_reference_inputs(
    service_root: &Path,
    device_id: &str,
    descriptors: Vec<ReferenceDescriptor>,
) -> Result<Option<Vec<ReferenceInput>>, BoxError> {
    let mut values = Vec::with_capacity(descriptors.len());
    for descriptor in descriptors {
        let Some(blob) =
            load_completed_reference_blob(service_root, device_id, &descriptor.blob_id)?
        else {
            return Ok(None);
        };
        blob.matches_descriptor(&descriptor)?;
        let bytes = blob.decoded_bytes()?;
        values.push(ReferenceInput {
            display_filename: descriptor.origin_name,
            media_type: descriptor.media_type,
            origin: format!("companion:{}", descriptor.blob_id),
            bytes,
        });
    }
    Ok(Some(values))
}

fn store_or_compare_immutable(
    path: &Path,
    bytes: &[u8],
    maximum: u64,
    conflict: &'static str,
) -> Result<bool, BoxError> {
    if bytes.len() as u64 > maximum {
        return Err("private reference record exceeds its encoded bound".into());
    }
    match write_new_atomic(path, bytes, 0o600) {
        Ok(()) => Ok(true),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|value| value.kind() == std::io::ErrorKind::AlreadyExists) =>
        {
            if read_bounded(path, maximum)? == bytes {
                Ok(false)
            } else {
                Err(conflict.into())
            }
        }
        Err(error) => Err(error),
    }
}

fn ensure_private_directory(path: &Path) -> Result<(), BoxError> {
    if let Some(parent) = path.parent() {
        if parent != path {
            match fs::symlink_metadata(parent) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                    return Err("private store parent is unsafe".into());
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir_all(parent)?;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("private store directory is unsafe".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir_all(path)?,
        Err(error) => return Err(error.into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, BoxError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err("private record is not a bounded regular file".into());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() != metadata.len() {
        return Err("private record changed while opening".into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err("private record exceeds its bound".into());
    }
    Ok(bytes)
}

fn write_new(path: &Path, bytes: &[u8], mode: u32) -> Result<(), BoxError> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(mode)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    File::open(path.parent().ok_or("private record has no parent")?)?.sync_all()?;
    Ok(())
}

fn write_new_atomic(path: &Path, bytes: &[u8], mode: u32) -> Result<(), BoxError> {
    let parent = path.parent().ok_or("private record has no parent")?;
    ensure_private_directory(parent)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err("private record target is unsafe".into());
        }
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "private record already exists",
            )
            .into());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let staging = parent
        .parent()
        .unwrap_or(parent)
        .join(".publication-staging");
    ensure_private_directory(&staging)?;
    let mut staged_count = 0_usize;
    for entry in fs::read_dir(&staging)?.take(65) {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "publication staging contains a non-UTF-8 entry")?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || name.len() > 300
            || !name.ends_with(".tmp")
        {
            return Err("publication staging contains an unsafe entry".into());
        }
        staged_count += 1;
    }
    if staged_count >= 64 {
        return Err("publication staging reached its bounded record limit".into());
    }
    let temporary = staging.join(format!(
        "{}.{}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("record"),
        compact_uuid()
    ));
    write_new(&temporary, bytes, mode)?;
    let publication = fs::hard_link(&temporary, path);
    let cleanup = fs::remove_file(&temporary);
    if let Err(error) = publication {
        cleanup?;
        File::open(&staging)?.sync_all()?;
        return Err(error.into());
    }
    cleanup?;
    File::open(&staging)?.sync_all()?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn write_replace(path: &Path, bytes: &[u8], mode: u32) -> Result<(), BoxError> {
    let parent = path.parent().ok_or("private record has no parent")?;
    ensure_private_directory(parent)?;
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("private record target is unsafe".into());
        }
    }
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("record"),
        compact_uuid()
    ));
    write_new(&temporary, bytes, mode)?;
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PNG_HEADER: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0,
        0, 1, 0, 0, 0, 1,
    ];

    fn reference_blob(blob_id: &str, origin_name: &str, size: usize) -> ReferenceBlob {
        let mut bytes = vec![0_u8; size.max(TEST_PNG_HEADER.len())];
        bytes[..TEST_PNG_HEADER.len()].copy_from_slice(TEST_PNG_HEADER);
        ReferenceBlob::new(blob_id, origin_name, "image/png", &bytes).unwrap()
    }

    fn command_with_reference(
        command_id: &str,
        device_id: &str,
        blob: &ReferenceBlob,
    ) -> CompanionCommand {
        let mut command = tohseno_companion::vectors::deterministic_vectors()
            .unwrap()
            .command
            .command;
        command.body.command_id = command_id.into();
        command.body.author_device_id = device_id.into();
        command.body.payload = CommandPayload::ShotCreateRequest {
            suggested_name: Some("reference-fixture".into()),
            intention: "Use the exact admitted reference.".into(),
            references: vec![blob.descriptor().unwrap()],
        };
        command
    }

    fn capability_state() -> DeviceCapabilityState {
        DeviceCapabilityState {
            device_id: "device_fixture".into(),
            capability_id: "capability_fixture".into(),
            revocation_epoch: 0,
            allowed_actions: vec![CapabilityAction::WorkspaceRead],
            revoked: false,
        }
    }

    #[test]
    fn phone_mailbox_reset_advances_only_to_retained_predecessor() {
        let reset = MailboxResetRequired {
            schema: "tohseno.companion-mailbox-reset-required/1".into(),
            reset_required: true,
            reset_before_cursor: 4,
            head_cursor: 9,
        };
        assert_eq!(cursor_after_mailbox_reset(2, &reset).unwrap(), 4);
        assert!(cursor_after_mailbox_reset(4, &reset).is_err());

        let invalid = MailboxResetRequired {
            reset_before_cursor: 10,
            ..reset
        };
        assert!(cursor_after_mailbox_reset(2, &invalid).is_err());
    }

    fn shot(execution: Option<ExecutionSummary>) -> ShotSummary {
        ShotSummary {
            shot_id: "shot_fixture".into(),
            display_name: "fixture".into(),
            bundle_identifier: Some("org.tohseno.genesis.fixture".into()),
            kind: ShotKind::FactoryShot,
            icon: None,
            icon_revision: 1,
            expression_id: None,
            latest_version_id: None,
            latest_version_ordinal: None,
            latest_version_created_at: None,
            execution,
            archived: false,
            retired: false,
            sort_index: 0,
            supported_companion_actions: vec![CapabilityAction::WorkspaceRead],
        }
    }

    fn execution(state: ExecutionStatus) -> ExecutionSummary {
        ExecutionSummary {
            execution_id: "execution_fixture".into(),
            shot_id: "shot_fixture".into(),
            state,
            updated_at: "2026-08-16T00:00:00Z".into(),
            failure_code: None,
        }
    }

    fn projection(
        shots: Vec<ShotSummary>,
        active_executions: Vec<ExecutionSummary>,
    ) -> PublishedWorkspaceProjection {
        PublishedWorkspaceProjection {
            schema: WORKSPACE_PROJECTION_SCHEMA.into(),
            workspace_id: "workspace_fixture".into(),
            service_version: env!("CARGO_PKG_VERSION").into(),
            shots,
            active_executions,
            device_capability_state: capability_state(),
            next_cursor: 2,
        }
    }

    fn snapshot(
        shots: Vec<ShotSummary>,
        active_executions: Vec<ExecutionSummary>,
    ) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            schema: WORKSPACE_SNAPSHOT_SCHEMA.into(),
            workspace_id: "workspace_fixture".into(),
            snapshot_version: 1,
            generated_at: "2026-08-16T00:01:00Z".into(),
            service_version: env!("CARGO_PKG_VERSION").into(),
            shots,
            active_executions,
            device_capability_state: capability_state(),
            next_cursor: 2,
        }
    }

    #[test]
    fn capabilities_are_explicit_and_sorted() {
        let values = all_capabilities();
        assert_eq!(values.len(), 6);
        assert!(values.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn protocol_hex_ids_are_valid_companion_opaque_identifiers() {
        assert!(tohseno_protocol::digest::Bytes32::from_hex(
            "fixture",
            "0x1111111111111111111111111111111111111111111111111111111111111111"
        )
        .is_ok());
    }

    #[test]
    fn execution_projection_normalizes_local_subseconds_to_companion_seconds() {
        let projected = convert_execution(tohseno_application::ExecutionSummary {
            execution_id: "execution_fixture".into(),
            shot_id: "shot_fixture".into(),
            state: "accepted".into(),
            version_ordinal: 1,
            updated_at: "2026-08-16T01:02:03.987654Z".into(),
        })
        .unwrap();
        assert_eq!(projected.updated_at, "2026-08-16T01:02:03Z");
        projected.validate().unwrap();
    }

    #[test]
    fn chunk_admissions_are_not_command_receipts() {
        let command = rejection("command_fixture", "fixture_rejection");
        assert_eq!(
            tohseno_companion::canonical::to_vec(&ProcessedEnvelope::Command(command.clone()))
                .unwrap(),
            tohseno_companion::canonical::to_vec(&command).unwrap()
        );

        let chunk = ProcessedEnvelope::ReferenceChunk(ReferenceChunkReceipt {
            schema: "tohseno.companion-reference-chunk-receipt/1".into(),
            blob_id: "blob_fixture".into(),
            chunk_index: 0,
            state: ReferenceChunkState::Stored,
        });
        let value = serde_json::to_value(chunk).unwrap();
        assert_eq!(
            value["schema"],
            "tohseno.companion-reference-chunk-receipt/1"
        );
        assert!(value.get("command_id").is_none());
        assert!(value.get("rejection_code").is_none());
    }

    #[test]
    fn admitted_envelopes_are_idempotent_for_commands_and_chunks() {
        let root = tempfile::tempdir().unwrap();
        let vectors = tohseno_companion::vectors::deterministic_vectors().unwrap();
        let command_envelope = vectors.envelope.envelope;
        let command_result =
            ProcessedEnvelope::Command(rejection("command_fixture", "fixture_rejection"));
        record_admitted_envelope_at(root.path(), &command_envelope, &command_result).unwrap();
        record_admitted_envelope_at(root.path(), &command_envelope, &command_result).unwrap();
        assert_eq!(
            admitted_envelope_result_at(root.path(), &command_envelope)
                .unwrap()
                .unwrap(),
            command_result
        );

        let mut chunk_envelope = vectors.relay.direct_envelope;
        chunk_envelope.header.envelope_id = "envelope_chunk_fixture".into();
        let chunk_result = ProcessedEnvelope::ReferenceChunk(ReferenceChunkReceipt {
            schema: "tohseno.companion-reference-chunk-receipt/1".into(),
            blob_id: "blob_fixture".into(),
            chunk_index: 0,
            state: ReferenceChunkState::Stored,
        });
        record_admitted_envelope_at(root.path(), &chunk_envelope, &chunk_result).unwrap();
        record_admitted_envelope_at(root.path(), &chunk_envelope, &chunk_result).unwrap();
        assert_eq!(
            admitted_envelope_result_at(root.path(), &chunk_envelope)
                .unwrap()
                .unwrap(),
            chunk_result
        );

        let conflicting = ProcessedEnvelope::ReferenceChunk(ReferenceChunkReceipt {
            schema: "tohseno.companion-reference-chunk-receipt/1".into(),
            blob_id: "blob_fixture".into(),
            chunk_index: 0,
            state: ReferenceChunkState::Complete,
        });
        assert!(record_admitted_envelope_at(root.path(), &chunk_envelope, &conflicting).is_err());
    }

    #[test]
    fn workspace_projection_emits_shot_and_execution_updates() {
        let queued = execution(ExecutionStatus::Queued);
        let previous = projection(Vec::new(), Vec::new());
        let current = snapshot(vec![shot(Some(queued.clone()))], vec![queued]);
        let payloads = workspace_change_payloads(&previous, &current).unwrap();
        assert!(payloads
            .iter()
            .any(|payload| matches!(payload, WorkspaceEventPayload::ShotUpsert { .. })));
        assert!(payloads
            .iter()
            .any(|payload| matches!(payload, WorkspaceEventPayload::ExecutionQueued { .. })));
    }

    #[test]
    fn persisted_icon_projection_is_stable_across_reconciliation() {
        let revision =
            revision_number("0xc4c26ab4c33958d4b35c2de82b8543dbcaecd64f60794a12fcc5c441c98332df");
        assert!(revision <= (1_u64 << 53) - 1);
        let mut current_shot = shot(None);
        current_shot.icon_revision = revision;
        current_shot.icon = Some(tohseno_companion::snapshot::IconDescriptor {
            blob_id: "icon_fixture".into(),
            revision,
            media_type: "image/png".into(),
            byte_length: 24,
            width: 1,
            height: 1,
            placeholder: true,
        });
        let persisted = projection(vec![current_shot.clone()], Vec::new());
        let bytes = tohseno_companion::canonical::to_vec(&persisted).unwrap();
        let restored: PublishedWorkspaceProjection =
            tohseno_companion::canonical::from_slice(&bytes).unwrap();

        let payloads =
            workspace_change_payloads(&restored, &snapshot(vec![current_shot], Vec::new()))
                .unwrap();
        assert!(payloads.is_empty());
    }

    #[test]
    fn disappearing_execution_becomes_accepted_only_with_a_new_version() {
        let running = execution(ExecutionStatus::Verifying);
        let previous = projection(vec![shot(Some(running.clone()))], vec![running]);
        let mut accepted = shot(None);
        accepted.expression_id = Some("expression_fixture".into());
        accepted.latest_version_id = Some("version_fixture".into());
        accepted.latest_version_ordinal = Some(1);
        accepted.latest_version_created_at = Some("2026-08-16T00:01:00Z".into());
        accepted.supported_companion_actions = vec![
            CapabilityAction::WorkspaceRead,
            CapabilityAction::FeedbackWrite,
            CapabilityAction::ShotEvolve,
        ];
        let payloads =
            workspace_change_payloads(&previous, &snapshot(vec![accepted], Vec::new())).unwrap();
        assert!(payloads
            .iter()
            .any(|payload| matches!(payload, WorkspaceEventPayload::ExecutionCompleted { .. })));
        assert!(!payloads
            .iter()
            .any(|payload| matches!(payload, WorkspaceEventPayload::ExecutionFailed { .. })));
    }

    #[test]
    fn terminal_shot_state_wins_after_version_upsert_precedes_active_removal() {
        let running = execution(ExecutionStatus::Verifying);
        let mut prior_shot = shot(Some(running.clone()));
        prior_shot.expression_id = Some("expression_fixture".into());
        prior_shot.latest_version_id = Some("version_two".into());
        prior_shot.latest_version_ordinal = Some(2);
        prior_shot.latest_version_created_at = Some("2026-08-16T00:01:00Z".into());
        let previous = projection(vec![prior_shot.clone()], vec![running]);

        let mut accepted_execution = execution(ExecutionStatus::Accepted);
        accepted_execution.updated_at = "2026-08-16T00:02:00Z".into();
        prior_shot.execution = Some(accepted_execution);
        let payloads =
            workspace_change_payloads(&previous, &snapshot(vec![prior_shot], Vec::new())).unwrap();
        assert!(payloads
            .iter()
            .any(|payload| matches!(payload, WorkspaceEventPayload::ExecutionCompleted { .. })));
        assert!(!payloads
            .iter()
            .any(|payload| matches!(payload, WorkspaceEventPayload::ExecutionFailed { .. })));
    }

    #[test]
    fn reference_chunks_rebuild_after_crash_and_out_of_order_delivery() {
        let root = tempfile::tempdir().unwrap();
        let blob = reference_blob(
            "blob_crash_fixture",
            "crash.png",
            MAX_REFERENCE_CHUNK_BYTES + TEST_PNG_HEADER.len(),
        );
        let chunks = blob.chunks().unwrap();
        assert_eq!(chunks.len(), 2);

        let second =
            persist_reference_chunk(root.path(), "device_fixture", chunks[1].clone()).unwrap();
        assert_eq!(second.state, ReferenceChunkState::Stored);
        assert!(
            load_completed_reference_blob(root.path(), "device_fixture", "blob_crash_fixture")
                .unwrap()
                .is_none()
        );

        // The helper has no in-memory assembly state. Re-entering it models a
        // service restart and reconstructs exclusively from durable chunks.
        let first =
            persist_reference_chunk(root.path(), "device_fixture", chunks[0].clone()).unwrap();
        assert_eq!(first.state, ReferenceChunkState::Complete);
        let restored =
            load_completed_reference_blob(root.path(), "device_fixture", "blob_crash_fixture")
                .unwrap()
                .unwrap();
        assert_eq!(restored, blob);
        assert_eq!(
            restored.decoded_bytes().unwrap().len(),
            MAX_REFERENCE_CHUNK_BYTES + 24
        );
    }

    #[test]
    fn reference_chunk_replay_is_idempotent_before_completion() {
        let root = tempfile::tempdir().unwrap();
        let blob = reference_blob(
            "blob_replay_fixture",
            "replay.png",
            MAX_REFERENCE_CHUNK_BYTES + TEST_PNG_HEADER.len(),
        );
        let chunk = blob.chunks().unwrap().remove(0);
        let first = persist_reference_chunk(root.path(), "device_fixture", chunk.clone()).unwrap();
        let duplicate = persist_reference_chunk(root.path(), "device_fixture", chunk).unwrap();
        assert_eq!(first.state, ReferenceChunkState::Stored);
        assert_eq!(duplicate.state, ReferenceChunkState::Duplicate);

        let chunk_files = fs::read_dir(
            root.path()
                .join("inbox/blobs/device_fixture/blob_replay_fixture/chunks"),
        )
        .unwrap()
        .count();
        assert_eq!(chunk_files, 1);
    }

    #[test]
    fn consumed_reference_cleanup_keeps_a_durable_command_receipt() {
        let root = tempfile::tempdir().unwrap();
        let device_id = "device_cleanup_fixture";
        let blob = reference_blob("blob_cleanup_fixture", "cleanup.png", TEST_PNG_HEADER.len());
        let chunk = blob.chunks().unwrap().remove(0);
        let receipt = persist_reference_chunk(root.path(), device_id, chunk).unwrap();
        assert_eq!(receipt.state, ReferenceChunkState::Complete);

        let mut envelope = tohseno_companion::vectors::deterministic_vectors()
            .unwrap()
            .envelope
            .envelope;
        envelope.header.envelope_id = "envelope_cleanup_fixture".into();
        track_reference_envelope(root.path(), device_id, "blob_cleanup_fixture", &envelope)
            .unwrap();
        let chunk_result = ProcessedEnvelope::ReferenceChunk(receipt);
        record_admitted_envelope_at(root.path(), &envelope, &chunk_result).unwrap();

        let command = command_with_reference("command_cleanup_fixture", device_id, &blob);
        let command_receipt = rejection("command_cleanup_fixture", "fixture_rejection");
        let durable = record_processed_command_at(root.path(), &command, &command_receipt).unwrap();
        assert!(root
            .path()
            .join("inbox/blobs/device_cleanup_fixture/blob_cleanup_fixture")
            .is_dir());

        remove_consumed_reference_blob(root.path(), device_id, "blob_cleanup_fixture").unwrap();
        assert!(!root
            .path()
            .join("inbox/blobs/device_cleanup_fixture/blob_cleanup_fixture")
            .exists());
        assert!(!root
            .path()
            .join("inbox/envelopes/envelope_cleanup_fixture.json")
            .exists());
        assert_eq!(
            processed_command_result_at(root.path(), &command)
                .unwrap()
                .unwrap(),
            durable
        );

        // This is the recovery path after a crash at any prior unlink.
        remove_consumed_reference_blob(root.path(), device_id, "blob_cleanup_fixture").unwrap();
    }

    #[test]
    fn reference_cleanup_releases_the_per_device_capacity() {
        let root = tempfile::tempdir().unwrap();
        for index in 0..MAX_REFERENCE_BLOBS_PER_DEVICE {
            let blob = reference_blob(
                &format!("blob_capacity_{index:03}"),
                &format!("capacity-{index:03}.png"),
                TEST_PNG_HEADER.len(),
            );
            persist_reference_chunk(
                root.path(),
                "device_capacity_fixture",
                blob.chunks().unwrap().remove(0),
            )
            .unwrap();
        }
        let overflow = reference_blob(
            "blob_capacity_overflow",
            "overflow.png",
            TEST_PNG_HEADER.len(),
        );
        assert!(persist_reference_chunk(
            root.path(),
            "device_capacity_fixture",
            overflow.chunks().unwrap().remove(0),
        )
        .is_err());

        remove_consumed_reference_blob(root.path(), "device_capacity_fixture", "blob_capacity_000")
            .unwrap();
        let replacement = reference_blob(
            "blob_capacity_replacement",
            "replacement.png",
            TEST_PNG_HEADER.len(),
        );
        persist_reference_chunk(
            root.path(),
            "device_capacity_fixture",
            replacement.chunks().unwrap().remove(0),
        )
        .unwrap();
    }

    #[test]
    fn processed_command_id_reuse_with_different_bytes_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let blob = reference_blob(
            "blob_command_conflict",
            "conflict.png",
            TEST_PNG_HEADER.len(),
        );
        let command = command_with_reference("command_conflict", "device_fixture", &blob);
        record_processed_command_at(
            root.path(),
            &command,
            &rejection("command_conflict", "fixture_rejection"),
        )
        .unwrap();
        let mut changed = command;
        let CommandPayload::ShotCreateRequest { intention, .. } = &mut changed.body.payload else {
            panic!("fixture command is not a creation request");
        };
        *intention = "Different signed bytes under the same command ID.".into();
        assert!(processed_command_result_at(root.path(), &changed).is_err());
    }

    #[test]
    fn bounded_record_store_rejects_capacity_and_unsafe_entries() {
        let root = tempfile::tempdir().unwrap();
        let store = root.path().join("records");
        ensure_private_directory(&store).unwrap();
        let first = store.join("first.json");
        ensure_bounded_record_store_capacity(&store, 1, &first, "fixture store").unwrap();
        fs::write(&first, b"{}").unwrap();
        ensure_bounded_record_store_capacity(&store, 1, &first, "fixture store").unwrap();
        assert!(ensure_bounded_record_store_capacity(
            &store,
            1,
            &store.join("second.json"),
            "fixture store"
        )
        .is_err());
        assert!(validate_reference_blob_id(".publication-staging").is_err());
    }

    #[test]
    fn reference_metadata_reuse_and_tampered_index_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let first = reference_blob("blob_tamper_fixture", "first.png", TEST_PNG_HEADER.len())
            .chunks()
            .unwrap()
            .remove(0);
        persist_reference_chunk(root.path(), "device_fixture", first).unwrap();

        let conflicting = reference_blob(
            "blob_tamper_fixture",
            "different.png",
            TEST_PNG_HEADER.len(),
        )
        .chunks()
        .unwrap()
        .remove(0);
        let error = persist_reference_chunk(root.path(), "device_fixture", conflicting)
            .unwrap_err()
            .to_string();
        assert!(error.contains("metadata was reused"));

        let index = root
            .path()
            .join("inbox/blobs/device_fixture/blob_tamper_fixture/index.json");
        fs::write(&index, b"{}").unwrap();
        let retry = reference_blob("blob_tamper_fixture", "first.png", TEST_PNG_HEADER.len())
            .chunks()
            .unwrap()
            .remove(0);
        assert!(persist_reference_chunk(root.path(), "device_fixture", retry).is_err());
    }

    #[test]
    fn command_resolution_requires_a_complete_matching_reference() {
        let root = tempfile::tempdir().unwrap();
        let blob = reference_blob("blob_missing_fixture", "missing.png", TEST_PNG_HEADER.len());
        let descriptor = blob.descriptor().unwrap();
        assert!(
            resolve_reference_inputs(root.path(), "device_fixture", vec![descriptor.clone()])
                .unwrap()
                .is_none()
        );

        persist_reference_chunk(
            root.path(),
            "device_fixture",
            blob.chunks().unwrap().remove(0),
        )
        .unwrap();
        let resolved =
            resolve_reference_inputs(root.path(), "device_fixture", vec![descriptor.clone()])
                .unwrap()
                .unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].bytes, blob.decoded_bytes().unwrap());

        let mut wrong = descriptor;
        wrong.origin_name = "other.png".into();
        assert!(resolve_reference_inputs(root.path(), "device_fixture", vec![wrong]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn reference_inbox_rejects_symlinked_completed_records() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let blob_root =
            ensure_reference_blob_directory(root.path(), "device_fixture", "blob_symlink_fixture")
                .unwrap();
        let outside = root.path().join("outside.json");
        fs::write(&outside, b"{}").unwrap();
        symlink(&outside, blob_root.join("completed.json")).unwrap();
        assert!(load_completed_reference_blob(
            root.path(),
            "device_fixture",
            "blob_symlink_fixture"
        )
        .is_err());
        assert!(remove_consumed_reference_blob(
            root.path(),
            "device_fixture",
            "blob_symlink_fixture"
        )
        .is_err());
        assert!(outside.exists());
    }
}
