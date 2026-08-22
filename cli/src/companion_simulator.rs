//! Deterministic native companion simulator for local protocol and factory E2E tests.
//!
//! This client deliberately traverses the real content-blind Companion Relay.
//! It never calls the service's direct simulation-envelope or pairing-response
//! shortcuts. Recovery words remain in Keychain and every durable simulator
//! record is authenticated ciphertext under the identity-derived storage key.

use rand_core::{OsRng, RngCore};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration as StdDuration, Instant};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};
use tohseno_companion::capability::CapabilityGrant;
use tohseno_companion::command::{
    CommandBody, CommandPayload, CommandReceipt, CompanionCommand, ReceiptState,
    COMPANION_COMMAND_SCHEMA,
};
use tohseno_companion::crypto::{base64url, decode_array, decode_base64url, decrypt, encrypt};
use tohseno_companion::envelope::{open_envelope, seal_envelope, EnvelopeMetadata, OpaqueEnvelope};
use tohseno_companion::event::{WorkspaceEvent, WorkspaceEventPayload};
use tohseno_companion::identity::{CompanionIdentity, RecoveryPhrase};
use tohseno_companion::journal::ReplayWindow;
use tohseno_companion::pairing::{
    EncryptedPairingResponse, PairingAcceptance, PairingInvitation, PairingProof,
    PairingResponseBody, RelayAllowlist, PAIRING_RESPONSE_BODY_SCHEMA,
};
use tohseno_companion::relay_client::{
    capability_verifier, EnvelopeAccepted, MailboxAck, MailboxAcknowledged, MailboxCreate,
    MailboxCreated, MailboxPage, PairingResponseAccepted,
};
use tohseno_companion::snapshot::{ShotKind, ShotSummary, WorkspaceSnapshot};
use tohseno_protocol::digest::Bytes32;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::service_client::ServiceClient;
use crate::service_commands::ServicePaths;
use crate::workspace_identity::{KeychainSecretStore, SecretStore};

const STATE_SCHEMA: &str = "tohseno.companion-simulator-state/1";
const STATE_RECORD_SCHEMA: &str = "tohseno.companion-simulator-encrypted-state/1";
const EXERCISE_SCHEMA: &str = "tohseno.companion-simulation/1";
const STATE_AAD_DOMAIN: &[u8] = b"tohseno.companion.simulator-state.v1";
const MAX_STATE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_STATE_RECORD_BYTES: u64 = 24 * 1024 * 1024;
const MAX_RELAY_RESPONSE_BYTES: usize = 24 * 1024 * 1024;
const MAX_PAIRING_MAILBOX_PAGES: usize = 32;
const RECONCILIATION_TIMEOUT: StdDuration = StdDuration::from_secs(240);
const RECONCILIATION_BACKOFF: StdDuration = StdDuration::from_millis(1_500);

type BoxError = Box<dyn std::error::Error>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SimulatorState {
    schema: String,
    device_id: String,
    relay_origin: String,
    workspace_id: String,
    studio_device_id: String,
    studio_signing_public_key: String,
    studio_agreement_public_key: String,
    capability: CapabilityGrant,
    response_mailbox_id: String,
    response_mailbox_read_capability: String,
    response_mailbox_ack_capability: String,
    command_mailbox_id: String,
    command_mailbox_write_capability: String,
    response_cursor: u64,
    sender_sequence: u64,
    replay: ReplayWindow,
    snapshot: WorkspaceSnapshot,
    outbox: Vec<QueuedEnvelope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    completed_exercise: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct QueuedEnvelope {
    command_id: String,
    command_kind: String,
    envelope: OpaqueEnvelope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    relay_cursor: Option<u64>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct StateRecordHeader<'a> {
    schema: &'a str,
    device_id: &'a str,
    nonce: &'a str,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncryptedStateRecord {
    schema: String,
    device_id: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Clone)]
struct RelayClient {
    origin: String,
    http: reqwest::Client,
}

struct MailboxAccess {
    mailbox_id: String,
    write: String,
    read: String,
    ack: String,
    revoke: String,
}

#[derive(Default)]
struct ObservedEvents {
    receipts: BTreeMap<String, CommandReceipt>,
    receipt_counts: BTreeMap<String, usize>,
    completed_executions: BTreeSet<String>,
    failed_executions: BTreeSet<String>,
}

pub async fn run(arguments: Vec<String>) -> Result<Value, BoxError> {
    match arguments.as_slice() {
        [operation] if operation == "pair" => pair().await,
        [operation, device_id] if operation == "exercise" => exercise(device_id).await,
        _ => Err(
            "usage: tohseno companion simulate pair | tohseno companion simulate exercise <device-id>"
                .into(),
        ),
    }
}

async fn pair() -> Result<Value, BoxError> {
    let service = ServiceClient::ensure_running()
        .await
        .map_err(|error| error.to_string())?;
    let relay = RelayClient::from_environment()?;
    let session: Value = service
        .post("/api/v1/companion/pairing-sessions", &json!({}))
        .await
        .map_err(|error| error.to_string())?;
    let uri = required_string(&session, "pairing_uri")?;
    let invitation = PairingInvitation::from_uri(uri)?;
    let studio_signing_key = decode_array::<32>(
        "Studio signing public key",
        &invitation.body.studio_signing_public_key,
    )?;
    invitation.verify(
        &studio_signing_key,
        &RelayAllowlist::official(),
        OffsetDateTime::now_utc(),
    )?;
    if required_string(&session, "session_id")? != invitation.body.session_id {
        return Err("Studio pairing route returned a different session identifier".into());
    }

    let response_mailbox = relay.create_mailbox().await?;
    let (phrase, phone) = CompanionIdentity::generate()?;
    let proof = PairingProof::create(
        &invitation,
        &phone,
        "TOHSENO Simulator",
        &timestamp(OffsetDateTime::now_utc())?,
    )?;
    let response = EncryptedPairingResponse::seal(
        &invitation,
        PairingResponseBody {
            schema: PAIRING_RESPONSE_BODY_SCHEMA.into(),
            proof,
            response_mailbox_id: response_mailbox.mailbox_id.clone(),
            response_mailbox_write_capability: response_mailbox.write.clone(),
            response_mailbox_revoke_capability: response_mailbox.revoke.clone(),
        },
    )?;
    let response_bytes = tohseno_companion::canonical::to_vec(&response)?;
    let first_response = relay
        .submit_pairing_response(&invitation.body.session_id, &response_bytes)
        .await?;
    let duplicate_response = relay
        .submit_pairing_response(&invitation.body.session_id, &response_bytes)
        .await?;
    if first_response.duplicate
        || !duplicate_response.duplicate
        || first_response.accepted != duplicate_response.accepted
    {
        return Err("Companion Relay pairing-response idempotence failed".into());
    }

    let deadline = Instant::now() + StdDuration::from_secs(20);
    loop {
        let view: Value = service
            .get(&format!(
                "/api/v1/companion/pairing-sessions/{}",
                invitation.body.session_id
            ))
            .await
            .map_err(|error| error.to_string())?;
        match view.get("state").and_then(Value::as_str) {
            Some("paired") => break,
            Some("waiting") if Instant::now() < deadline => {
                tokio::time::sleep(StdDuration::from_millis(100)).await;
            }
            Some(state) => return Err(format!("pairing entered unexpected state {state}").into()),
            None => return Err("pairing state response is malformed".into()),
        }
    }

    let mut replay = ReplayWindow::new(65_536)?;
    let mut cursor = 0_u64;
    let mut acceptance = None;
    let mut snapshot = None;
    let mut caught_up = false;
    for _ in 0..MAX_PAIRING_MAILBOX_PAGES {
        let page = relay
            .mailbox_page(&response_mailbox.mailbox_id, &response_mailbox.read, cursor)
            .await?;
        page.validate_routing(&response_mailbox.mailbox_id, cursor)?;
        for item in page.envelopes {
            let plaintext = open_envelope(
                &item.envelope,
                &studio_signing_key,
                &invitation.body.studio_device_id,
                &phone,
                OffsetDateTime::now_utc(),
                &mut replay,
            )?;
            let shape: Value = tohseno_companion::canonical::from_slice(&plaintext)?;
            match shape.get("schema").and_then(Value::as_str) {
                Some("tohseno.companion-pairing-grant-package/1") => {
                    let value: PairingAcceptance =
                        tohseno_companion::canonical::from_slice(&plaintext)?;
                    value.validate(
                        &studio_signing_key,
                        &invitation.body.studio_device_id,
                        phone.device_id(),
                        &invitation.body.workspace_id,
                        OffsetDateTime::now_utc(),
                    )?;
                    acceptance = Some(value);
                }
                Some("tohseno.companion-event/1") => {
                    let event: WorkspaceEvent =
                        tohseno_companion::canonical::from_slice(&plaintext)?;
                    event.validate()?;
                    if event.workspace_id != invitation.body.workspace_id {
                        return Err("pairing event names a different workspace".into());
                    }
                    if let WorkspaceEventPayload::WorkspaceSnapshot { snapshot: value } =
                        event.payload
                    {
                        snapshot = Some(*value);
                    }
                }
                _ => return Err("pairing mailbox contained an unsupported plaintext schema".into()),
            }
            cursor = item.cursor;
        }
        if !page.has_more {
            caught_up = true;
            break;
        }
    }
    if !caught_up {
        return Err("pairing mailbox catch-up exceeded its page bound".into());
    }
    let acceptance = acceptance.ok_or("pairing mailbox omitted the capability grant")?;
    let snapshot = snapshot.ok_or("pairing mailbox omitted the workspace snapshot")?;
    snapshot.validate()?;

    let paths = ServicePaths::discover()?;
    let secrets = KeychainSecretStore;
    let secret_reference = format!("simulator-phrase:{}", phone.device_id());
    secrets
        .put(&secret_reference, phrase.expose().as_bytes())
        .map_err(std::io::Error::other)?;
    let state = SimulatorState {
        schema: STATE_SCHEMA.into(),
        device_id: phone.device_id().into(),
        relay_origin: relay.origin.clone(),
        workspace_id: snapshot.workspace_id.clone(),
        studio_device_id: invitation.body.studio_device_id,
        studio_signing_public_key: invitation.body.studio_signing_public_key,
        studio_agreement_public_key: acceptance.studio_agreement_public_key,
        capability: acceptance.capability_grant,
        response_mailbox_id: response_mailbox.mailbox_id,
        response_mailbox_read_capability: response_mailbox.read,
        response_mailbox_ack_capability: response_mailbox.ack,
        command_mailbox_id: acceptance.command_mailbox_id,
        command_mailbox_write_capability: acceptance.command_mailbox_write_capability,
        response_cursor: cursor,
        sender_sequence: 0,
        replay,
        snapshot,
        outbox: Vec::new(),
        completed_exercise: None,
    };
    if let Err(error) = store_state(&paths.service_state, &phone, &state) {
        let _ = secrets.delete(&secret_reference);
        return Err(error);
    }
    if cursor > 0 {
        relay
            .acknowledge(
                &state.response_mailbox_id,
                &state.response_mailbox_ack_capability,
                cursor,
            )
            .await?;
    }

    Ok(json!({
        "schema": EXERCISE_SCHEMA,
        "operation": "pair",
        "device_id": state.device_id,
        "capability_id": state.capability.body.capability_id,
        "workspace_id": state.workspace_id,
        "shot_count": state.snapshot.shots.len(),
        "identity_storage": "macos_keychain",
        "state_storage": "chacha20poly1305_encrypted",
        "state_root": paths.service_state,
        "invitation_verified": true,
        "real_relay_pairing": true,
        "pairing_response_duplicate": true,
        "mailbox_cursor": cursor,
        "mailbox_acknowledged": cursor > 0,
        "encrypted_snapshot_received": true,
    }))
}

async fn exercise(device_id: &str) -> Result<Value, BoxError> {
    validate_device_id(device_id)?;
    let service = ServiceClient::ensure_running()
        .await
        .map_err(|error| error.to_string())?;
    let paths = ServicePaths::discover()?;
    let (phone, mut state) = load_state(&paths.service_state, device_id)?;
    if let Some(result) = &state.completed_exercise {
        return Ok(result.clone());
    }
    let relay = RelayClient::new(&state.relay_origin)?;
    if relay.origin != RelayClient::from_environment()?.origin {
        return Err("simulator relay origin changed after pairing".into());
    }
    validate_state(&state, &phone)?;
    let base = eligible_factory_shot(&state.snapshot)?;

    let feedback = signed_command(
        &phone,
        &state,
        CommandPayload::FeedbackSubmit {
            shot_id: base.shot_id.clone(),
            expression_id: base
                .expression_id
                .clone()
                .ok_or("fixture Shot has no accepted Expression")?,
            version_id: base
                .latest_version_id
                .clone()
                .ok_or("fixture Shot has no accepted Version")?,
            version_ordinal: base
                .latest_version_ordinal
                .ok_or("fixture Shot has no accepted Version ordinal")?,
            body: "Simulator feedback bound to this exact accepted Version.".into(),
        },
    )?;
    let feedback_id = feedback.body.command_id.clone();
    enqueue(&mut state, &phone, "feedback.submit", feedback)?;
    store_state(&paths.service_state, &phone, &state)?;

    let feedback_envelope = state
        .outbox
        .iter()
        .find(|item| item.command_id == feedback_id)
        .ok_or("feedback disappeared from the encrypted outbox")?
        .envelope
        .clone();
    let first = relay
        .upload_envelope(
            &state.command_mailbox_id,
            &state.command_mailbox_write_capability,
            &feedback_envelope,
        )
        .await?;
    let duplicate = relay
        .upload_envelope(
            &state.command_mailbox_id,
            &state.command_mailbox_write_capability,
            &feedback_envelope,
        )
        .await?;
    if first.duplicate || !duplicate.duplicate || first.cursor != duplicate.cursor {
        return Err("duplicate feedback envelope did not retain one relay cursor".into());
    }
    mark_uploaded(&mut state, &feedback_id, first.cursor)?;
    store_state(&paths.service_state, &phone, &state)?;

    let mut observed = ObservedEvents::default();
    wait_for_receipts(
        &paths.service_state,
        &relay,
        &phone,
        &mut state,
        &mut observed,
        &BTreeSet::from([feedback_id.clone()]),
    )
    .await?;
    let feedback_receipt = observed
        .receipts
        .get(&feedback_id)
        .cloned()
        .ok_or("feedback command receipt is unavailable")?;
    if feedback_receipt.state != ReceiptState::Completed
        || feedback_receipt.shot_id.as_deref() != Some(base.shot_id.as_str())
    {
        return Err("feedback was not attached to the exact fixture Shot".into());
    }
    state.outbox.retain(|item| item.command_id != feedback_id);
    store_state(&paths.service_state, &phone, &state)?;

    let marketing = signed_command(
        &phone,
        &state,
        CommandPayload::MarketingSubmit {
            note_id: opaque_id("note"),
            shot_id: base.shot_id.clone(),
            body: "Simulator private marketing note; never relay plaintext.".into(),
        },
    )?;
    let marketing_id = marketing.body.command_id.clone();
    let feedback_action = feedback_receipt
        .result_id
        .as_deref()
        .map(feedback_commitment_base64url)
        .transpose()?
        .into_iter()
        .collect();
    let evolution = signed_command(
        &phone,
        &state,
        CommandPayload::ShotEvolveRequest {
            shot_id: base.shot_id.clone(),
            base_expression_id: base
                .expression_id
                .clone()
                .ok_or("fixture Shot has no accepted Expression")?,
            base_version_id: base
                .latest_version_id
                .clone()
                .ok_or("fixture Shot has no accepted Version")?,
            base_version_ordinal: base
                .latest_version_ordinal
                .ok_or("fixture Shot has no accepted Version ordinal")?,
            intention: concat!(
                "# Evolutionary Intention\n\n",
                "Keep the exact Shot and accepted Genome while making continuity into ",
                "Version 0002 visible.\n"
            )
            .into(),
            selected_feedback_action_commitments: feedback_action,
            references: Vec::new(),
        },
    )?;
    let evolution_id = evolution.body.command_id.clone();
    enqueue(&mut state, &phone, "marketing.submit", marketing)?;
    enqueue(&mut state, &phone, "shot.evolve.request", evolution)?;
    store_state(&paths.service_state, &phone, &state)?;

    // A new process would reconstruct exactly these values from Keychain and
    // authenticated state. Drop every plaintext structure before loading it.
    drop(state);
    drop(phone);
    let (phone, mut state) = load_state(&paths.service_state, device_id)?;
    if state.outbox.len() != 2 {
        return Err("encrypted offline outbox did not survive simulator relaunch".into());
    }
    for index in 0..state.outbox.len() {
        let envelope = state.outbox[index].envelope.clone();
        let accepted = relay
            .upload_envelope(
                &state.command_mailbox_id,
                &state.command_mailbox_write_capability,
                &envelope,
            )
            .await?;
        state.outbox[index].relay_cursor = Some(accepted.cursor);
        store_state(&paths.service_state, &phone, &state)?;
    }

    let offline_expected = BTreeSet::from([marketing_id.clone(), evolution_id.clone()]);
    wait_for_receipts(
        &paths.service_state,
        &relay,
        &phone,
        &mut state,
        &mut observed,
        &offline_expected,
    )
    .await?;
    let marketing_receipt =
        required_receipt(&observed, &marketing_id, ReceiptState::Completed)?.clone();
    let evolution_receipt =
        required_receipt(&observed, &evolution_id, ReceiptState::Accepted)?.clone();
    let evolution_execution_id = evolution_receipt
        .execution_id
        .clone()
        .ok_or("evolution receipt omitted its execution ID")?;
    let evolution_execution = BTreeSet::from([evolution_execution_id.clone()]);
    wait_for_executions(
        &paths.service_state,
        &relay,
        &phone,
        &mut state,
        &mut observed,
        &evolution_execution,
    )
    .await?;
    state
        .outbox
        .retain(|item| !offline_expected.contains(&item.command_id));
    store_state(&paths.service_state, &phone, &state)?;

    // Submit creation only after evolution has reached accepted Version 0002.
    // This still traverses the same encrypted relay/application boundary while
    // keeping the deterministic Apple fixture from running two Xcode/identity
    // acceptance transactions concurrently on one developer machine.
    let creation = signed_command(
        &phone,
        &state,
        CommandPayload::ShotCreateRequest {
            suggested_name: Some(format!(
                "mobilefixture{}",
                Uuid::new_v4()
                    .simple()
                    .to_string()
                    .chars()
                    .take(8)
                    .collect::<String>()
            )),
            intention: concat!(
                "# Coherent Intention\n\n",
                "Show a complete, quiet native iPhone expression with a clear visible identity.\n",
                "The primary screen must state that it is a TOHSENO expression and that the ",
                "Apple materialization gates passed.\n"
            )
            .into(),
            references: Vec::new(),
        },
    )?;
    let creation_id = creation.body.command_id.clone();
    enqueue(&mut state, &phone, "shot.create.request", creation)?;
    store_state(&paths.service_state, &phone, &state)?;
    let creation_index = state
        .outbox
        .iter()
        .position(|item| item.command_id == creation_id)
        .ok_or("creation disappeared from the encrypted outbox")?;
    let creation_envelope = state.outbox[creation_index].envelope.clone();
    let creation_accepted = relay
        .upload_envelope(
            &state.command_mailbox_id,
            &state.command_mailbox_write_capability,
            &creation_envelope,
        )
        .await?;
    state.outbox[creation_index].relay_cursor = Some(creation_accepted.cursor);
    store_state(&paths.service_state, &phone, &state)?;
    wait_for_receipts(
        &paths.service_state,
        &relay,
        &phone,
        &mut state,
        &mut observed,
        &BTreeSet::from([creation_id.clone()]),
    )
    .await?;
    let creation_receipt =
        required_receipt(&observed, &creation_id, ReceiptState::Accepted)?.clone();
    let creation_execution_id = creation_receipt
        .execution_id
        .clone()
        .ok_or("creation receipt omitted its execution ID")?;
    wait_for_executions(
        &paths.service_state,
        &relay,
        &phone,
        &mut state,
        &mut observed,
        &BTreeSet::from([creation_execution_id.clone()]),
    )
    .await?;
    let expected = BTreeSet::from([marketing_id, evolution_id, creation_id]);
    for command_id in expected.iter().chain(std::iter::once(&feedback_id)) {
        if observed.receipt_counts.get(command_id) != Some(&1) {
            return Err(format!("command {command_id} produced a duplicate receipt event").into());
        }
    }
    state
        .outbox
        .retain(|item| !expected.contains(&item.command_id));
    store_state(&paths.service_state, &phone, &state)?;
    let execution_ids = BTreeSet::from([evolution_execution_id, creation_execution_id]);

    let post_revoke = signed_command(
        &phone,
        &state,
        CommandPayload::MarketingSubmit {
            note_id: opaque_id("note"),
            shot_id: base.shot_id,
            body: "This command must be rejected after revocation.".into(),
        },
    )?;
    let post_revoke_envelope = seal_command(&mut state, &phone, post_revoke)?;
    let revoked: Value = service
        .delete(&format!("/api/v1/companion/devices/{device_id}"))
        .await
        .map_err(|error| error.to_string())?;
    if revoked.get("revoked").and_then(Value::as_bool) != Some(true) {
        return Err("Local Workspace Service did not confirm device revocation".into());
    }
    let post_revocation_rejected = relay
        .upload_envelope(
            &state.command_mailbox_id,
            &state.command_mailbox_write_capability,
            &post_revoke_envelope,
        )
        .await
        .is_err();
    if !post_revocation_rejected {
        return Err("revoked command mailbox accepted a new companion command".into());
    }
    let devices: Value = service
        .get("/api/v1/companion/devices")
        .await
        .map_err(|error| error.to_string())?;
    let device_revoked = devices
        .get("devices")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("device_id").and_then(Value::as_str) == Some(device_id))
        })
        .and_then(|item| item.get("revoked"))
        .and_then(Value::as_bool)
        == Some(true);
    if !device_revoked {
        return Err("paired-device registry did not retain revocation".into());
    }

    let result = json!({
        "schema": EXERCISE_SCHEMA,
        "operation": "exercise",
        "device_id": device_id,
        "real_relay_commands": true,
        "feedback_exact_version": true,
        "feedback_result_id": feedback_receipt.result_id,
        "duplicate_feedback_relay_cursor": first.cursor,
        "duplicate_delivery_exactly_once": true,
        "offline_outbox_encrypted": true,
        "offline_outbox_relaunch": true,
        "marketing_note_recorded": marketing_receipt.result_id.is_some(),
        "evolution_execution_id": evolution_receipt.execution_id,
        "creation_execution_id": creation_receipt.execution_id,
        "executions_accepted": execution_ids.len(),
        "receipts_acknowledged": 4,
        "mailbox_cursor": state.response_cursor,
        "device_revoked": true,
        "post_revocation_rejected": true,
    });
    state.completed_exercise = Some(result.clone());
    store_state(&paths.service_state, &phone, &state)?;
    Ok(result)
}

fn eligible_factory_shot(snapshot: &WorkspaceSnapshot) -> Result<ShotSummary, BoxError> {
    snapshot
        .shots
        .iter()
        .find(|shot| {
            shot.kind == ShotKind::FactoryShot
                && shot.expression_id.is_some()
                && shot.latest_version_id.is_some()
                && shot.latest_version_ordinal.is_some()
        })
        .cloned()
        .ok_or_else(|| "exercise requires one accepted factory Shot in the paired snapshot".into())
}

fn signed_command(
    phone: &CompanionIdentity,
    state: &SimulatorState,
    payload: CommandPayload,
) -> Result<CompanionCommand, BoxError> {
    Ok(CompanionCommand::sign(
        phone,
        CommandBody {
            schema: COMPANION_COMMAND_SCHEMA.into(),
            command_id: opaque_id("command"),
            workspace_id: state.workspace_id.clone(),
            capability_id: state.capability.body.capability_id.clone(),
            author_device_id: phone.device_id().into(),
            created_at: timestamp(OffsetDateTime::now_utc())?,
            payload,
        },
    )?)
}

fn enqueue(
    state: &mut SimulatorState,
    phone: &CompanionIdentity,
    command_kind: &str,
    command: CompanionCommand,
) -> Result<(), BoxError> {
    let command_id = command.body.command_id.clone();
    if state
        .outbox
        .iter()
        .any(|item| item.command_id == command_id)
    {
        return Err("companion outbox command ID collision".into());
    }
    let envelope = seal_command(state, phone, command)?;
    state.outbox.push(QueuedEnvelope {
        command_id,
        command_kind: command_kind.into(),
        envelope,
        relay_cursor: None,
    });
    Ok(())
}

fn seal_command(
    state: &mut SimulatorState,
    phone: &CompanionIdentity,
    command: CompanionCommand,
) -> Result<OpaqueEnvelope, BoxError> {
    state.sender_sequence = state
        .sender_sequence
        .checked_add(1)
        .ok_or("simulator sender sequence overflowed")?;
    let now = OffsetDateTime::now_utc().replace_nanosecond(0)?;
    Ok(seal_envelope(
        phone,
        &decode_array::<32>(
            "Studio agreement public key",
            &state.studio_agreement_public_key,
        )?,
        EnvelopeMetadata {
            envelope_id: Uuid::new_v4().to_string(),
            mailbox_id: state.command_mailbox_id.clone(),
            recipient_device_id: state.studio_device_id.clone(),
            sender_sequence: state.sender_sequence,
            created_at: timestamp(now)?,
            expires_at: timestamp(now + Duration::days(7))?,
        },
        &tohseno_companion::canonical::to_vec(&command)?,
    )?)
}

fn mark_uploaded(
    state: &mut SimulatorState,
    command_id: &str,
    cursor: u64,
) -> Result<(), BoxError> {
    let item = state
        .outbox
        .iter_mut()
        .find(|item| item.command_id == command_id)
        .ok_or("uploaded command is absent from the durable outbox")?;
    item.relay_cursor = Some(cursor);
    Ok(())
}

fn required_receipt<'a>(
    observed: &'a ObservedEvents,
    command_id: &str,
    expected_state: ReceiptState,
) -> Result<&'a CommandReceipt, BoxError> {
    let receipt = observed
        .receipts
        .get(command_id)
        .ok_or("command receipt is unavailable")?;
    if receipt.state != expected_state {
        return Err(format!("command {command_id} returned an unexpected state").into());
    }
    Ok(receipt)
}

fn feedback_commitment_base64url(value: &str) -> Result<String, BoxError> {
    let commitment = Bytes32::from_hex("feedback action commitment", value)?;
    Ok(base64url(commitment.as_bytes()))
}

async fn wait_for_receipts(
    service_root: &Path,
    relay: &RelayClient,
    phone: &CompanionIdentity,
    state: &mut SimulatorState,
    observed: &mut ObservedEvents,
    command_ids: &BTreeSet<String>,
) -> Result<(), BoxError> {
    let deadline = Instant::now() + RECONCILIATION_TIMEOUT;
    loop {
        if command_ids
            .iter()
            .all(|command_id| observed.receipts.contains_key(command_id))
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("timed out reconciling encrypted companion command receipts".into());
        }
        reconcile_phone_once(service_root, relay, phone, state, observed).await?;
        // The local service reconciles its command mailbox independently.
        // Keep the simulated phone below the relay's ordinary per-source
        // budget even though both processes share one loopback address.
        tokio::time::sleep(RECONCILIATION_BACKOFF).await;
    }
}

async fn wait_for_executions(
    service_root: &Path,
    relay: &RelayClient,
    phone: &CompanionIdentity,
    state: &mut SimulatorState,
    observed: &mut ObservedEvents,
    execution_ids: &BTreeSet<String>,
) -> Result<(), BoxError> {
    let deadline = Instant::now() + RECONCILIATION_TIMEOUT;
    loop {
        if execution_ids
            .iter()
            .any(|execution_id| observed.failed_executions.contains(execution_id))
        {
            return Err("a companion factory execution reported failure".into());
        }
        if execution_ids
            .iter()
            .all(|execution_id| observed.completed_executions.contains(execution_id))
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("timed out reconciling encrypted execution completion events".into());
        }
        reconcile_phone_once(service_root, relay, phone, state, observed).await?;
        tokio::time::sleep(RECONCILIATION_BACKOFF).await;
    }
}

async fn reconcile_phone_once(
    service_root: &Path,
    relay: &RelayClient,
    phone: &CompanionIdentity,
    state: &mut SimulatorState,
    observed: &mut ObservedEvents,
) -> Result<usize, BoxError> {
    let page = relay
        .mailbox_page(
            &state.response_mailbox_id,
            &state.response_mailbox_read_capability,
            state.response_cursor,
        )
        .await?;
    page.validate_routing(&state.response_mailbox_id, state.response_cursor)?;
    let count = page.envelopes.len();
    for item in page.envelopes {
        let plaintext = open_envelope(
            &item.envelope,
            &decode_array::<32>(
                "Studio signing public key",
                &state.studio_signing_public_key,
            )?,
            &state.studio_device_id,
            phone,
            OffsetDateTime::now_utc(),
            &mut state.replay,
        )?;
        let event: WorkspaceEvent = tohseno_companion::canonical::from_slice(&plaintext)?;
        event.validate()?;
        if event.workspace_id != state.workspace_id {
            return Err("companion event names a different workspace".into());
        }
        apply_event(state, observed, event.payload)?;
        state.response_cursor = item.cursor;
    }
    if count > 0 {
        // Persist the local cursor and replay window before acknowledging relay
        // retention. A crash then causes only an idempotent ACK retry.
        store_state(service_root, phone, state)?;
        relay
            .acknowledge(
                &state.response_mailbox_id,
                &state.response_mailbox_ack_capability,
                state.response_cursor,
            )
            .await?;
    }
    Ok(count)
}

fn apply_event(
    state: &mut SimulatorState,
    observed: &mut ObservedEvents,
    payload: WorkspaceEventPayload,
) -> Result<(), BoxError> {
    match payload {
        WorkspaceEventPayload::WorkspaceSnapshot { snapshot } => {
            state.snapshot = *snapshot;
        }
        WorkspaceEventPayload::ShotUpsert { shot } => {
            upsert_shot(&mut state.snapshot, *shot);
        }
        WorkspaceEventPayload::ShotArchive { shot_id } => {
            if let Some(shot) = state
                .snapshot
                .shots
                .iter_mut()
                .find(|shot| shot.shot_id == shot_id)
            {
                shot.archived = true;
            }
        }
        WorkspaceEventPayload::ShotRemove { shot_id } => {
            state.snapshot.shots.retain(|shot| shot.shot_id != shot_id);
        }
        WorkspaceEventPayload::VersionAccepted {
            shot_id,
            expression_id,
            version_id,
            version_ordinal,
            accepted_at,
        } => {
            if let Some(shot) = state
                .snapshot
                .shots
                .iter_mut()
                .find(|shot| shot.shot_id == shot_id)
            {
                shot.expression_id = Some(expression_id);
                shot.latest_version_id = Some(version_id);
                shot.latest_version_ordinal = Some(version_ordinal);
                shot.latest_version_created_at = Some(accepted_at);
            }
        }
        WorkspaceEventPayload::CommandAcknowledged { receipt }
        | WorkspaceEventPayload::CommandRejected { receipt } => {
            let count = observed
                .receipt_counts
                .entry(receipt.command_id.clone())
                .or_default();
            *count += 1;
            if let Some(existing) = observed
                .receipts
                .insert(receipt.command_id.clone(), receipt.clone())
            {
                if existing != receipt {
                    return Err("one command ID produced conflicting receipts".into());
                }
            }
        }
        WorkspaceEventPayload::ExecutionCompleted { execution } => {
            observed.completed_executions.insert(execution.execution_id);
        }
        WorkspaceEventPayload::ExecutionFailed { execution } => {
            observed.failed_executions.insert(execution.execution_id);
        }
        WorkspaceEventPayload::ExecutionQueued { .. }
        | WorkspaceEventPayload::ProductEntitlement { .. }
        | WorkspaceEventPayload::ExecutionStarted { .. }
        | WorkspaceEventPayload::ExecutionUpdated { .. }
        | WorkspaceEventPayload::ExecutionWaitingForDevice { .. }
        | WorkspaceEventPayload::IconBlob { .. }
        | WorkspaceEventPayload::DeviceRevoked { .. } => {}
    }
    state.snapshot.validate()?;
    Ok(())
}

fn upsert_shot(snapshot: &mut WorkspaceSnapshot, value: ShotSummary) {
    if let Some(existing) = snapshot
        .shots
        .iter_mut()
        .find(|shot| shot.shot_id == value.shot_id)
    {
        *existing = value;
    } else {
        snapshot.shots.push(value);
        snapshot.shots.sort_by(|left, right| {
            left.sort_index
                .cmp(&right.sort_index)
                .then_with(|| left.shot_id.cmp(&right.shot_id))
        });
    }
}

fn store_state(
    service_root: &Path,
    phone: &CompanionIdentity,
    state: &SimulatorState,
) -> Result<(), BoxError> {
    validate_state(state, phone)?;
    let plaintext = tohseno_companion::canonical::to_vec(state)?;
    if plaintext.len() as u64 > MAX_STATE_BYTES {
        return Err("encrypted simulator state exceeds its plaintext bound".into());
    }
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let nonce_encoded = base64url(&nonce);
    let header = StateRecordHeader {
        schema: STATE_RECORD_SCHEMA,
        device_id: &state.device_id,
        nonce: &nonce_encoded,
    };
    let aad = state_aad(&header)?;
    let storage_key = phone.storage_key();
    let ciphertext = encrypt(&storage_key, &nonce, &plaintext, &aad)?;
    let record = EncryptedStateRecord {
        schema: STATE_RECORD_SCHEMA.into(),
        device_id: state.device_id.clone(),
        nonce: nonce_encoded,
        ciphertext: base64url(&ciphertext),
    };
    let bytes = tohseno_companion::canonical::to_vec(&record)?;
    if bytes.len() as u64 > MAX_STATE_RECORD_BYTES {
        return Err("encrypted simulator state exceeds its record bound".into());
    }
    write_private_replace(&state_path(service_root, &state.device_id)?, &bytes)
}

fn load_state(
    service_root: &Path,
    device_id: &str,
) -> Result<(CompanionIdentity, SimulatorState), BoxError> {
    validate_device_id(device_id)?;
    let secrets = KeychainSecretStore;
    let words = Zeroizing::new(
        secrets
            .get(&format!("simulator-phrase:{device_id}"))
            .map_err(std::io::Error::other)?,
    );
    let words = Zeroizing::new(std::str::from_utf8(&words)?.to_owned());
    let phrase = RecoveryPhrase::parse(words.as_str().to_owned())?;
    let phone = CompanionIdentity::restore(&phrase)?;
    if phone.device_id() != device_id {
        return Err("simulator recovery phrase does not match its device ID".into());
    }
    let path = state_path(service_root, device_id)?;
    let record: EncryptedStateRecord =
        tohseno_companion::canonical::from_slice(&read_bounded(&path, MAX_STATE_RECORD_BYTES)?)?;
    if record.schema != STATE_RECORD_SCHEMA || record.device_id != device_id {
        return Err("encrypted simulator state header is invalid".into());
    }
    let nonce = decode_array::<12>("simulator state nonce", &record.nonce)?;
    let ciphertext = decode_base64url(
        "simulator state ciphertext",
        &record.ciphertext,
        MAX_STATE_BYTES as usize + 16,
    )?;
    let header = StateRecordHeader {
        schema: &record.schema,
        device_id: &record.device_id,
        nonce: &record.nonce,
    };
    let storage_key = phone.storage_key();
    let plaintext = decrypt(&storage_key, &nonce, &ciphertext, &state_aad(&header)?)?;
    if plaintext.len() as u64 > MAX_STATE_BYTES {
        return Err("decrypted simulator state exceeds its bound".into());
    }
    let state: SimulatorState = tohseno_companion::canonical::from_slice(&plaintext)?;
    validate_state(&state, &phone)?;
    Ok((phone, state))
}

fn validate_state(state: &SimulatorState, phone: &CompanionIdentity) -> Result<(), BoxError> {
    if state.schema != STATE_SCHEMA
        || state.device_id != phone.device_id()
        || state.capability.body.device_id != state.device_id
        || state.capability.body.workspace_id != state.workspace_id
        || state.snapshot.workspace_id != state.workspace_id
        || state.outbox.len() > 32
    {
        return Err("simulator state identity boundary is invalid".into());
    }
    validate_device_id(&state.device_id)?;
    state.snapshot.validate()?;
    state.capability.verify(
        &decode_array::<32>(
            "Studio signing public key",
            &state.studio_signing_public_key,
        )?,
        OffsetDateTime::now_utc(),
    )?;
    RelayClient::new(&state.relay_origin)?;
    let mut command_ids = BTreeSet::new();
    let mut sequences = BTreeSet::new();
    for item in &state.outbox {
        if !command_ids.insert(item.command_id.clone())
            || item.envelope.header.mailbox_id != state.command_mailbox_id
            || item.envelope.header.sender_device_id != state.device_id
            || item.envelope.header.recipient_device_id != state.studio_device_id
            || !sequences.insert(item.envelope.header.sender_sequence)
            || item.envelope.header.sender_sequence > state.sender_sequence
        {
            return Err("encrypted simulator outbox is inconsistent".into());
        }
        item.envelope.validate_relay_shape()?;
    }
    Ok(())
}

fn state_aad(header: &StateRecordHeader<'_>) -> Result<Vec<u8>, BoxError> {
    let header_bytes = tohseno_companion::canonical::to_vec(header)?;
    let mut aad = Vec::with_capacity(STATE_AAD_DOMAIN.len() + 1 + header_bytes.len());
    aad.extend_from_slice(STATE_AAD_DOMAIN);
    aad.push(0);
    aad.extend_from_slice(&header_bytes);
    Ok(aad)
}

fn state_path(service_root: &Path, device_id: &str) -> Result<PathBuf, BoxError> {
    validate_device_id(device_id)?;
    Ok(service_root
        .join("simulator")
        .join(format!("{device_id}.json")))
}

fn ensure_private_directory(path: &Path) -> Result<(), BoxError> {
    if let Some(parent) = path.parent() {
        match fs::symlink_metadata(parent) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err("simulator state parent is unsafe".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir_all(parent)?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("simulator state directory is unsafe".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(path)?,
        Err(error) => return Err(error.into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_private_replace(path: &Path, bytes: &[u8]) -> Result<(), BoxError> {
    let parent = path.parent().ok_or("simulator state has no parent")?;
    ensure_private_directory(parent)?;
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("simulator state target is unsafe".into());
        }
    }
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("state"),
        Uuid::new_v4().simple()
    ));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, BoxError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err("simulator state is not a bounded regular file".into());
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
        return Err("simulator state changed while opening".into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err("simulator state exceeds its bound".into());
    }
    Ok(bytes)
}

impl RelayClient {
    fn from_environment() -> Result<Self, BoxError> {
        let origin = std::env::var("TOHSENO_COMPANION_RELAY_ORIGIN")
            .map_err(|_| "companion simulator requires TOHSENO_COMPANION_RELAY_ORIGIN")?;
        Self::new(&origin)
    }

    fn new(origin: &str) -> Result<Self, BoxError> {
        let parsed = reqwest::Url::parse(origin)?;
        if parsed.scheme() != "http"
            || !matches!(parsed.host_str(), Some("127.0.0.1" | "::1" | "[::1]"))
            || parsed.username() != ""
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !matches!(parsed.path(), "" | "/")
        {
            return Err(
                "companion simulator accepts only an exact loopback HTTP relay origin".into(),
            );
        }
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(StdDuration::from_secs(2))
            .timeout(StdDuration::from_secs(20))
            .user_agent(concat!(
                "tohseno-companion-simulator/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;
        Ok(Self {
            origin: origin.trim_end_matches('/').into(),
            http,
        })
    }

    async fn create_mailbox(&self) -> Result<MailboxAccess, BoxError> {
        let capabilities = (0..5).map(|_| random_capability()).collect::<Vec<_>>();
        if capabilities.iter().collect::<BTreeSet<_>>().len() != capabilities.len() {
            return Err("relay capability generator repeated a value".into());
        }
        let request = MailboxCreate {
            schema: "tohseno.companion-mailbox-create/1".into(),
            write_verifier: capability_verifier(&capabilities[0])?,
            read_verifier: capability_verifier(&capabilities[1])?,
            ack_verifier: capability_verifier(&capabilities[2])?,
            revoke_verifier: capability_verifier(&capabilities[3])?,
            push_verifier: capability_verifier(&capabilities[4])?,
        };
        request.validate()?;
        let response = self
            .http
            .post(self.url("/v1/companion/mailboxes"))
            .json(&request)
            .send()
            .await?;
        let created: MailboxCreated =
            decode_json(response, &[reqwest::StatusCode::CREATED]).await?;
        created.validate()?;
        Ok(MailboxAccess {
            mailbox_id: created.mailbox_id,
            write: capabilities[0].clone(),
            read: capabilities[1].clone(),
            ack: capabilities[2].clone(),
            revoke: capabilities[3].clone(),
        })
    }

    async fn submit_pairing_response(
        &self,
        session_id: &str,
        bytes: &[u8],
    ) -> Result<PairingResponseAccepted, BoxError> {
        validate_relay_id(session_id)?;
        let response = self
            .http
            .post(self.url(&format!(
                "/v1/companion/pairing-sessions/{session_id}/respond"
            )))
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(bytes.to_vec())
            .send()
            .await?;
        let accepted: PairingResponseAccepted = decode_json(
            response,
            &[reqwest::StatusCode::OK, reqwest::StatusCode::CREATED],
        )
        .await?;
        accepted.validate()?;
        Ok(accepted)
    }

    async fn upload_envelope(
        &self,
        mailbox_id: &str,
        write_capability: &str,
        envelope: &OpaqueEnvelope,
    ) -> Result<EnvelopeAccepted, BoxError> {
        validate_relay_id(mailbox_id)?;
        envelope.validate_relay_shape()?;
        if envelope.header.mailbox_id != mailbox_id {
            return Err("refusing to upload an envelope to a different mailbox".into());
        }
        let response = self
            .http
            .post(self.url(&format!("/v1/companion/mailboxes/{mailbox_id}/envelopes")))
            .header(AUTHORIZATION, bearer(write_capability)?)
            .json(envelope)
            .send()
            .await?;
        let accepted: EnvelopeAccepted = decode_json(
            response,
            &[reqwest::StatusCode::OK, reqwest::StatusCode::CREATED],
        )
        .await?;
        accepted.validate()?;
        Ok(accepted)
    }

    async fn mailbox_page(
        &self,
        mailbox_id: &str,
        read_capability: &str,
        cursor: u64,
    ) -> Result<MailboxPage, BoxError> {
        validate_relay_id(mailbox_id)?;
        let response = self
            .http
            .get(self.url(&format!("/v1/companion/mailboxes/{mailbox_id}/envelopes")))
            .header(AUTHORIZATION, bearer(read_capability)?)
            .query(&[("cursor", cursor), ("limit", 32_u64)])
            .send()
            .await?;
        let page: MailboxPage = decode_json(response, &[reqwest::StatusCode::OK]).await?;
        page.validate_routing(mailbox_id, cursor)?;
        Ok(page)
    }

    async fn acknowledge(
        &self,
        mailbox_id: &str,
        ack_capability: &str,
        cursor: u64,
    ) -> Result<(), BoxError> {
        validate_relay_id(mailbox_id)?;
        let request = MailboxAck {
            schema: "tohseno.companion-mailbox-ack/1".into(),
            cursor,
        };
        request.validate()?;
        let response = self
            .http
            .post(self.url(&format!("/v1/companion/mailboxes/{mailbox_id}/ack")))
            .header(AUTHORIZATION, bearer(ack_capability)?)
            .json(&request)
            .send()
            .await?;
        let acknowledged: MailboxAcknowledged =
            decode_json(response, &[reqwest::StatusCode::OK]).await?;
        acknowledged.validate()?;
        if acknowledged.acknowledged_cursor != cursor {
            return Err("Companion Relay changed the acknowledgement cursor".into());
        }
        Ok(())
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.origin)
    }
}

async fn decode_json<T: DeserializeOwned + Serialize>(
    response: reqwest::Response,
    expected: &[reqwest::StatusCode],
) -> Result<T, BoxError> {
    if !expected.contains(&response.status()) {
        return Err(format!(
            "Companion Relay rejected a simulator operation with status {}",
            response.status().as_u16()
        )
        .into());
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RELAY_RESPONSE_BYTES as u64)
    {
        return Err("Companion Relay response exceeds its bound".into());
    }
    let bytes = response.bytes().await?;
    if bytes.len() > MAX_RELAY_RESPONSE_BYTES {
        return Err("Companion Relay response exceeds its bound".into());
    }
    // Relay response objects are transport DTOs rather than signed protocol
    // payloads.  The Bun service deliberately uses ordinary JSON encoding;
    // each DTO's `validate` method supplies the wire-contract checks after
    // decoding.  Canonical JSON is required only inside signed/encrypted
    // companion payloads.
    Ok(serde_json::from_slice(&bytes)?)
}

fn bearer(capability: &str) -> Result<reqwest::header::HeaderValue, BoxError> {
    tohseno_companion::relay_client::validate_bearer_capability(capability)?;
    Ok(reqwest::header::HeaderValue::from_str(&format!(
        "Bearer {capability}"
    ))?)
}

fn random_capability() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    base64url(&bytes)
}

fn validate_relay_id(value: &str) -> Result<(), BoxError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("invalid Companion Relay path identifier".into());
    }
    Ok(())
}

fn validate_device_id(value: &str) -> Result<(), BoxError> {
    if !value.starts_with("device_")
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("invalid simulator device ID".into());
    }
    Ok(())
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, BoxError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("response omitted {field}").into())
}

fn opaque_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn timestamp(value: OffsetDateTime) -> Result<String, BoxError> {
    Ok(value.replace_nanosecond(0)?.format(&Rfc3339)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tohseno_companion::capability::CapabilityGrant;
    use tohseno_companion::identity::WorkspaceServiceIdentity;
    use tohseno_companion::snapshot::DeviceCapabilityState;

    fn fixture_state() -> (CompanionIdentity, SimulatorState, String) {
        let vectors = tohseno_companion::vectors::deterministic_vectors().unwrap();
        let phrase = RecoveryPhrase::parse(vectors.companion_identity.mnemonic).unwrap();
        let phone = CompanionIdentity::restore(&phrase).unwrap();
        // Shared vectors deliberately carry fixed timestamps so every
        // implementation can compare exact bytes. This storage-only fixture
        // still exercises production validation, so re-sign the same grant
        // body for the current test instant instead of becoming date-bound.
        let studio = WorkspaceServiceIdentity::from_secret_keys(
            decode_array(
                "vector Studio signing secret key",
                &vectors
                    .workspace_service_identity
                    .signing_secret_key_base64url,
            )
            .unwrap(),
            decode_array(
                "vector Studio agreement secret key",
                &vectors
                    .workspace_service_identity
                    .agreement_secret_key_base64url,
            )
            .unwrap(),
        )
        .unwrap();
        let now = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
        let mut capability_body = vectors.capability.grant.body;
        capability_body.issued_at = timestamp(now - Duration::minutes(1)).unwrap();
        capability_body.expires_at = Some(timestamp(now + Duration::days(1)).unwrap());
        let capability = CapabilityGrant::sign(capability_body, &studio).unwrap();
        let private_mailbox_capability = random_capability();
        let mailbox_id = vectors.relay.mailbox_created.mailbox_id;
        let snapshot = WorkspaceSnapshot {
            schema: "tohseno.companion-workspace-snapshot/1".into(),
            workspace_id: capability.body.workspace_id.clone(),
            snapshot_version: 1,
            generated_at: "2026-08-16T00:00:00Z".into(),
            service_version: env!("CARGO_PKG_VERSION").into(),
            shots: Vec::new(),
            active_executions: Vec::new(),
            device_capability_state: DeviceCapabilityState {
                device_id: phone.device_id().into(),
                capability_id: capability.body.capability_id.clone(),
                revocation_epoch: capability.body.revocation_epoch,
                allowed_actions: capability.body.allowed_actions.clone(),
                revoked: false,
            },
            next_cursor: 1,
        };
        let state = SimulatorState {
            schema: STATE_SCHEMA.into(),
            device_id: phone.device_id().into(),
            relay_origin: "http://127.0.0.1:4242".into(),
            workspace_id: capability.body.workspace_id.clone(),
            studio_device_id: vectors.workspace_service_identity.device_id,
            studio_signing_public_key: vectors
                .workspace_service_identity
                .signing_public_key_base64url,
            studio_agreement_public_key: vectors
                .workspace_service_identity
                .agreement_public_key_base64url,
            capability,
            response_mailbox_id: mailbox_id.clone(),
            response_mailbox_read_capability: random_capability(),
            response_mailbox_ack_capability: random_capability(),
            command_mailbox_id: mailbox_id,
            command_mailbox_write_capability: private_mailbox_capability.clone(),
            response_cursor: 0,
            sender_sequence: 0,
            replay: ReplayWindow::new(65_536).unwrap(),
            snapshot,
            outbox: Vec::new(),
            completed_exercise: None,
        };
        (phone, state, private_mailbox_capability)
    }

    #[test]
    fn simulator_accepts_only_exact_loopback_relay_origins() {
        assert_eq!(
            RelayClient::new("http://127.0.0.1:4242/").unwrap().origin,
            "http://127.0.0.1:4242"
        );
        assert!(RelayClient::new("http://[::1]:4242").is_ok());
        for value in [
            "https://127.0.0.1:4242",
            "http://localhost:4242",
            "http://relay.example:4242",
            "http://user@127.0.0.1:4242",
            "http://127.0.0.1:4242/private",
            "http://127.0.0.1:4242/?mailbox=secret",
        ] {
            assert!(RelayClient::new(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn durable_simulator_state_encrypts_private_mailbox_material() {
        let root = tempfile::tempdir().unwrap();
        let (phone, state, private_capability) = fixture_state();
        store_state(root.path(), &phone, &state).unwrap();
        let path = state_path(root.path(), phone.device_id()).unwrap();
        let record_bytes = read_bounded(&path, MAX_STATE_RECORD_BYTES).unwrap();
        assert!(!record_bytes
            .windows(private_capability.len())
            .any(|window| window == private_capability.as_bytes()));
        assert!(!record_bytes
            .windows(state.workspace_id.len())
            .any(|window| window == state.workspace_id.as_bytes()));

        let record: EncryptedStateRecord =
            tohseno_companion::canonical::from_slice(&record_bytes).unwrap();
        let nonce = decode_array::<12>("state nonce", &record.nonce).unwrap();
        let ciphertext = decode_base64url(
            "state ciphertext",
            &record.ciphertext,
            MAX_STATE_BYTES as usize + 16,
        )
        .unwrap();
        let header = StateRecordHeader {
            schema: &record.schema,
            device_id: &record.device_id,
            nonce: &record.nonce,
        };
        let plaintext = decrypt(
            &phone.storage_key(),
            &nonce,
            &ciphertext,
            &state_aad(&header).unwrap(),
        )
        .unwrap();
        assert_eq!(
            plaintext,
            tohseno_companion::canonical::to_vec(&state).unwrap()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn durable_simulator_state_refuses_a_symlink_target() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let (phone, state, _) = fixture_state();
        let path = state_path(root.path(), phone.device_id()).unwrap();
        fs::create_dir(path.parent().unwrap()).unwrap();
        let target = root.path().join("outside.json");
        fs::write(&target, b"unchanged").unwrap();
        symlink(&target, &path).unwrap();
        assert!(store_state(root.path(), &phone, &state).is_err());
        assert_eq!(fs::read(target).unwrap(), b"unchanged");
    }
}
