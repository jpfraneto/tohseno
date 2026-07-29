//! Guarded preparation of relayer-reproducible public actions.
//!
//! This module is pure apart from explicit local artifact persistence. Chain
//! reads remain in `public_network`; transaction mutation remains in
//! `public_submission`.

use crate::builder_identity::{
    builder_account_creation_bytecode, BuilderIdentity, BuilderIdentityManager,
};
use crate::public_network::{
    embedded_deployment_plan, BuilderAccountCodeState, DeploymentPlan, PublicCheckStatus,
    PublicPreparationRead, RegistryPreparationState,
};
use serde::{Deserialize, Serialize};
use sha3::{Digest as _, Keccak256};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tohseno_protocol::actions::{
    Eip712Domain, PublicAction, PublicState, SignedPublicAction, EIP712_VERSION,
    PUBLIC_ACTION_SCHEMA, SHOT_REGISTRY_DOMAIN, SHOT_RELATIONS_DOMAIN,
};
use tohseno_protocol::canonical;
use tohseno_protocol::digest::{Address20, Bytes32};
use tohseno_protocol::identity::{
    device_key_id, predict_builder_account, BuilderId, ROBINHOOD_CHAIN_ID,
};
use tohseno_protocol::record::ShotRecord;
use tohseno_protocol::signature::{
    decode_compact, encode_compact, DetachedP256Signature, P256PublicKey, SignatureSidecar,
};

pub const SIGNED_PUBLIC_ACTION_PACKAGE_SCHEMA: &str = "tohseno.signed-public-action-package/1";
pub const BUILDER_DEPLOYMENT_REQUEST_SCHEMA: &str = "tohseno.builder-account-deployment-request/1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicAuthority {
    pub builder_id: BuilderId,
    pub account: Address20,
    pub signer: P256PublicKey,
    pub signer_key_id: Bytes32,
    pub account_salt: Bytes32,
}

pub fn bind_public_authority(
    identity: &BuilderIdentity,
    record: &ShotRecord,
    record_signature: &SignatureSidecar,
) -> Result<PublicAuthority, PublicActionError> {
    identity
        .validate()
        .map_err(|error| PublicActionError::Authority(error.to_string()))?;
    record
        .validate()
        .map_err(|error| PublicActionError::Record(error.to_string()))?;
    record
        .verify_signature(record_signature)
        .map_err(|error| PublicActionError::Record(error.to_string()))?;
    if identity.test_only
        || identity.key_backend != "secure_enclave"
        || identity.security_level != "secure_enclave"
    {
        return Err(PublicActionError::Authority(
            "public actions require a non-test Secure Enclave DeviceKey".into(),
        ));
    }
    if identity.builder_id != record.builder_id
        || identity.account_address != record.builder_id.account()
        || identity.device.public_key != record_signature.public_key
    {
        return Err(PublicActionError::Authority(
            "BuilderID, controller, active DeviceKey, and local Shot signature do not bind".into(),
        ));
    }
    Ok(PublicAuthority {
        builder_id: identity.builder_id,
        account: identity.account_address,
        signer: identity.device.public_key.clone(),
        signer_key_id: device_key_id(&identity.device.public_key),
        account_salt: identity.account_salt,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicActionRequest {
    Publish,
    ClaimHandle { handle: String },
    AssociateAppcoin { chain_id: u64, token: Address20 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanOutcome {
    BuilderDeploymentRequired(Box<BuilderAccountDeploymentRequest>),
    AlreadyCurrent(AlreadyCurrentPublicState),
    Action(Box<PlannedPublicAction>),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlreadyCurrentPublicState {
    pub schema: String,
    pub shot_id: String,
    pub controller: Address20,
    pub head: Bytes32,
    pub sequence: u64,
    pub verified: bool,
    pub signed: bool,
    pub submitted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedPublicAction {
    pub domain: Eip712Domain,
    pub action: PublicAction,
    pub signer: P256PublicKey,
    pub digest: Bytes32,
    pub relay_argument: RelayArgument,
    pub expected_state: ExpectedPublicState,
    pub record_binding: PublicRecordBinding,
    pub read_evidence: PublicPreparationRead,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelayArgument {
    None,
    Handle { value: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExpectedPublicState {
    Evolution {
        controller: Address20,
        head: Bytes32,
        sequence: u64,
        registry_nonce: u64,
    },
    Handle {
        shot_id: String,
        handle_hash: Bytes32,
        relations_nonce: u64,
    },
    Appcoin {
        shot_id: String,
        chain_id: u64,
        token: Address20,
        relations_nonce: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicRecordBinding {
    pub shot_id: String,
    pub builder_id: String,
    pub evolution_commitment: Bytes32,
    pub sequence: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedPublicActionPackage {
    pub schema: String,
    pub chain_id: u64,
    pub target: Address20,
    pub calldata: String,
    pub compact_signature: String,
    pub signed_action: SignedPublicAction,
    pub relay_argument: RelayArgument,
    pub expected_state: ExpectedPublicState,
    pub record_binding: PublicRecordBinding,
    pub read_evidence: PublicPreparationRead,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderAccountDeploymentRequest {
    pub schema: String,
    pub chain_id: u64,
    pub target: Address20,
    pub calldata: String,
    pub expected_account: Address20,
    pub account_salt: Bytes32,
    pub initial_key_id: Bytes32,
    pub initial_public_key: P256PublicKey,
    pub signed: bool,
    pub submitted: bool,
    pub read_evidence: PublicPreparationRead,
}

pub fn plan_public_action(
    plan: &DeploymentPlan,
    record: &ShotRecord,
    authority: &PublicAuthority,
    read: &PublicPreparationRead,
    request: PublicActionRequest,
    deadline: u64,
    now_unix: u64,
) -> Result<PlanOutcome, PublicActionError> {
    plan.validate()
        .map_err(|error| PublicActionError::Network(error.to_string()))?;
    record
        .validate()
        .map_err(|error| PublicActionError::Record(error.to_string()))?;
    verify_closed_read_header(read)?;
    if record.builder_id != authority.builder_id
        || record.builder_id.account() != authority.account
        || deadline <= now_unix
    {
        return Err(PublicActionError::Guard(
            "record authority or future deadline guard failed".into(),
        ));
    }
    let builder = read
        .builder_account
        .as_ref()
        .ok_or_else(|| PublicActionError::Network("BuilderAccount read is missing".into()))?;
    if builder.account != authority.account {
        return Err(PublicActionError::Guard(
            "observed BuilderAccount differs from the loaded BuilderID".into(),
        ));
    }
    if builder.code_state == BuilderAccountCodeState::Missing {
        return Ok(PlanOutcome::BuilderDeploymentRequired(Box::new(
            builder_deployment_request(plan, authority, read.clone())?,
        )));
    }
    if builder.protocol_permission != Some(true) {
        return Err(PublicActionError::Authority(
            "the active DeviceKey lacks on-chain protocol permission".into(),
        ));
    }

    let registry = read
        .registry
        .as_ref()
        .ok_or_else(|| PublicActionError::Network("ShotRegistry read is missing".into()))?;
    let commitment = record
        .commitment()
        .map_err(|error| PublicActionError::Record(error.to_string()))?;
    let record_binding = PublicRecordBinding {
        shot_id: record.shot_id.to_string(),
        builder_id: record.builder_id.to_string(),
        evolution_commitment: commitment,
        sequence: record.sequence,
    };

    let (action, relay_argument, expected_state, domain_name, target) = match request {
        PublicActionRequest::Publish => {
            match plan_publish(record, authority, registry, commitment, deadline)? {
                PublishPlan::AlreadyCurrent => {
                    return Ok(PlanOutcome::AlreadyCurrent(AlreadyCurrentPublicState {
                        schema: "tohseno.already-current-public-state/1".into(),
                        shot_id: record.shot_id.to_string(),
                        controller: registry.controller,
                        head: registry.head,
                        sequence: registry.sequence,
                        verified: true,
                        signed: false,
                        submitted: false,
                    }))
                }
                PublishPlan::Action(action, expected) => (
                    *action,
                    RelayArgument::None,
                    expected,
                    SHOT_REGISTRY_DOMAIN,
                    plan.contracts.shot_registry.planned_address,
                ),
            }
        }
        PublicActionRequest::ClaimHandle { handle } => {
            validate_handle(&handle)?;
            require_exact_public_head(record, authority, registry, commitment)?;
            let relations = read.relations.as_ref().ok_or_else(|| {
                PublicActionError::Network("ShotRelations reads are missing".into())
            })?;
            let next_nonce = relations.nonce.checked_add(1).ok_or_else(|| {
                PublicActionError::Guard("ShotRelations nonce cannot advance".into())
            })?;
            let handle_hash = handle_hash(&handle)?;
            if relations.handle_by_shot != Bytes32::ZERO
                || relations.shot_by_requested_handle != Some(Bytes32::ZERO)
            {
                return Err(PublicActionError::Guard(
                    "the Shot or requested handle already has a live relation".into(),
                ));
            }
            (
                PublicAction::ClaimHandle {
                    shot_id: record.shot_id,
                    handle_hash,
                    nonce: relations.nonce,
                    deadline,
                },
                RelayArgument::Handle {
                    value: handle.clone(),
                },
                ExpectedPublicState::Handle {
                    shot_id: record.shot_id.to_string(),
                    handle_hash,
                    relations_nonce: next_nonce,
                },
                SHOT_RELATIONS_DOMAIN,
                plan.contracts.shot_relations.planned_address,
            )
        }
        PublicActionRequest::AssociateAppcoin { chain_id, token } => {
            require_exact_public_head(record, authority, registry, commitment)?;
            let relations = read.relations.as_ref().ok_or_else(|| {
                PublicActionError::Network("ShotRelations reads are missing".into())
            })?;
            if relations.appcoin_chain_id != 0 || !is_zero_address(relations.appcoin_token) {
                return Err(PublicActionError::Guard(
                    "associate-appcoin refuses to overwrite an existing relation".into(),
                ));
            }
            let next_nonce = relations.nonce.checked_add(1).ok_or_else(|| {
                PublicActionError::Guard("ShotRelations nonce cannot advance".into())
            })?;
            (
                PublicAction::AssociateAppcoin {
                    shot_id: record.shot_id,
                    chain_id,
                    token,
                    nonce: relations.nonce,
                    deadline,
                },
                RelayArgument::None,
                ExpectedPublicState::Appcoin {
                    shot_id: record.shot_id.to_string(),
                    chain_id,
                    token,
                    relations_nonce: next_nonce,
                },
                SHOT_RELATIONS_DOMAIN,
                plan.contracts.shot_relations.planned_address,
            )
        }
    };
    action
        .validate()
        .map_err(|error| PublicActionError::Guard(error.to_string()))?;
    let domain = Eip712Domain {
        name: domain_name.into(),
        version: EIP712_VERSION.into(),
        chain_id: ROBINHOOD_CHAIN_ID,
        verifying_contract: target,
    };
    let digest = action
        .digest(&domain)
        .map_err(|error| PublicActionError::Guard(error.to_string()))?;
    Ok(PlanOutcome::Action(Box::new(PlannedPublicAction {
        domain,
        action,
        signer: authority.signer.clone(),
        digest,
        relay_argument,
        expected_state,
        record_binding,
        read_evidence: read.clone(),
    })))
}

enum PublishPlan {
    AlreadyCurrent,
    Action(Box<PublicAction>, ExpectedPublicState),
}

fn plan_publish(
    record: &ShotRecord,
    authority: &PublicAuthority,
    registry: &RegistryPreparationState,
    commitment: Bytes32,
    deadline: u64,
) -> Result<PublishPlan, PublicActionError> {
    if is_zero_address(registry.controller) {
        if registry.head != Bytes32::ZERO
            || registry.sequence != 0
            || registry.shot_nonce != 0
            || record.previous.is_some()
        {
            return Err(PublicActionError::Guard(
                "an absent public Shot must be a real local lineage root with zero observed state"
                    .into(),
            ));
        }
        return Ok(PublishPlan::Action(
            Box::new(PublicAction::CreateShot {
                shot_id: record.shot_id,
                controller: authority.account,
                head: commitment,
                sequence: u64::from(record.sequence),
                public_state: PublicState::Published,
                content_commitment: Bytes32::ZERO,
                nonce: registry.create_nonce,
                deadline,
            }),
            ExpectedPublicState::Evolution {
                controller: authority.account,
                head: commitment,
                sequence: u64::from(record.sequence),
                registry_nonce: 1,
            },
        ));
    }
    require_controller(record, authority, registry)?;
    if registry.head == commitment && registry.sequence == u64::from(record.sequence) {
        return Ok(PublishPlan::AlreadyCurrent);
    }
    let previous = record.previous.ok_or_else(|| {
        PublicActionError::Guard("a public append requires a local previous commitment".into())
    })?;
    if previous != registry.head
        || u64::from(record.sequence) != registry.sequence.checked_add(1).unwrap_or(u64::MAX)
    {
        return Err(PublicActionError::Guard(
            "public head and sequence are not the exact local predecessor; skips are forbidden"
                .into(),
        ));
    }
    let next_nonce = registry
        .shot_nonce
        .checked_add(1)
        .ok_or_else(|| PublicActionError::Guard("ShotRegistry nonce cannot advance".into()))?;
    Ok(PublishPlan::Action(
        Box::new(PublicAction::AppendEvolution {
            shot_id: record.shot_id,
            previous_head: previous,
            new_head: commitment,
            sequence: u64::from(record.sequence),
            content_commitment: Bytes32::ZERO,
            nonce: registry.shot_nonce,
            deadline,
        }),
        ExpectedPublicState::Evolution {
            controller: authority.account,
            head: commitment,
            sequence: u64::from(record.sequence),
            registry_nonce: next_nonce,
        },
    ))
}

fn require_exact_public_head(
    record: &ShotRecord,
    authority: &PublicAuthority,
    registry: &RegistryPreparationState,
    commitment: Bytes32,
) -> Result<(), PublicActionError> {
    require_controller(record, authority, registry)?;
    if registry.head != commitment || registry.sequence != u64::from(record.sequence) {
        return Err(PublicActionError::Guard(
            "relations require the exact local head and sequence to be public first".into(),
        ));
    }
    Ok(())
}

fn require_controller(
    record: &ShotRecord,
    authority: &PublicAuthority,
    registry: &RegistryPreparationState,
) -> Result<(), PublicActionError> {
    if registry.controller != authority.account
        || registry.controller != record.builder_id.account()
    {
        return Err(PublicActionError::Authority(
            "on-chain controller differs from the bound local BuilderID".into(),
        ));
    }
    Ok(())
}

fn builder_deployment_request(
    plan: &DeploymentPlan,
    authority: &PublicAuthority,
    read_evidence: PublicPreparationRead,
) -> Result<BuilderAccountDeploymentRequest, PublicActionError> {
    let factory = plan.contracts.builder_account_factory.planned_address;
    Ok(BuilderAccountDeploymentRequest {
        schema: BUILDER_DEPLOYMENT_REQUEST_SCHEMA.into(),
        chain_id: ROBINHOOD_CHAIN_ID,
        target: factory,
        calldata: encode_hex(&encode_builder_account_deployment(
            authority.account_salt,
            &authority.signer,
        )),
        expected_account: authority.account,
        account_salt: authority.account_salt,
        initial_key_id: authority.signer_key_id,
        initial_public_key: authority.signer.clone(),
        signed: false,
        submitted: false,
        read_evidence,
    })
}

pub fn sign_planned_action(
    manager: &BuilderIdentityManager,
    identity: &BuilderIdentity,
    planned: PlannedPublicAction,
) -> Result<SignedPublicActionPackage, PublicActionError> {
    let authorization = manager
        .sign_digest(identity, planned.digest)
        .map_err(|error| PublicActionError::Authority(error.to_string()))?;
    finalize_planned_action(planned, authorization)
}

pub fn finalize_planned_action(
    planned: PlannedPublicAction,
    authorization: DetachedP256Signature,
) -> Result<SignedPublicActionPackage, PublicActionError> {
    if authorization.digest != planned.digest {
        return Err(PublicActionError::Authority(
            "hardware signer returned a different public-action digest".into(),
        ));
    }
    let signed_action = SignedPublicAction {
        schema: PUBLIC_ACTION_SCHEMA.into(),
        domain: planned.domain,
        action: planned.action,
        signer: planned.signer,
        authorization,
    };
    signed_action
        .verify()
        .map_err(|error| PublicActionError::Signature(error.to_string()))?;
    let compact = encode_compact(
        &signed_action.signer,
        &signed_action.authorization.signature,
    )
    .map_err(|error| PublicActionError::Signature(error.to_string()))?;
    let calldata =
        encode_action_calldata(&signed_action.action, &compact, &planned.relay_argument)?;
    let package = SignedPublicActionPackage {
        schema: SIGNED_PUBLIC_ACTION_PACKAGE_SCHEMA.into(),
        chain_id: ROBINHOOD_CHAIN_ID,
        target: signed_action.domain.verifying_contract,
        calldata: encode_hex(&calldata),
        compact_signature: encode_hex(&compact),
        signed_action,
        relay_argument: planned.relay_argument,
        expected_state: planned.expected_state,
        record_binding: planned.record_binding,
        read_evidence: planned.read_evidence,
    };
    package.verify()?;
    Ok(package)
}

impl SignedPublicActionPackage {
    pub fn verify(&self) -> Result<(), PublicActionError> {
        if self.schema != SIGNED_PUBLIC_ACTION_PACKAGE_SCHEMA
            || self.chain_id != ROBINHOOD_CHAIN_ID
            || self.target != self.signed_action.domain.verifying_contract
        {
            return Err(PublicActionError::Guard(
                "signed public-action package metadata or read evidence is invalid".into(),
            ));
        }
        self.signed_action
            .verify()
            .map_err(|error| PublicActionError::Signature(error.to_string()))?;
        verify_closed_read_header(&self.read_evidence)?;
        let plan = embedded_deployment_plan()
            .map_err(|error| PublicActionError::Network(error.to_string()))?;
        let expected_target = match &self.signed_action.action {
            PublicAction::CreateShot { .. } | PublicAction::AppendEvolution { .. } => {
                plan.contracts.shot_registry.planned_address
            }
            PublicAction::ClaimHandle { .. } | PublicAction::AssociateAppcoin { .. } => {
                plan.contracts.shot_relations.planned_address
            }
            _ => {
                return Err(PublicActionError::Guard(
                    "package contains an unsupported public action".into(),
                ))
            }
        };
        if self.target != expected_target {
            return Err(PublicActionError::Guard(
                "signed action target is not the pinned candidate contract".into(),
            ));
        }
        verify_package_read_bindings(self)?;
        let compact = decode_hex(&self.compact_signature)?;
        let (public_key, signature) = decode_compact(&compact)
            .map_err(|error| PublicActionError::Signature(error.to_string()))?;
        if public_key != self.signed_action.signer
            || signature != self.signed_action.authorization.signature
        {
            return Err(PublicActionError::Signature(
                "compact signature differs from SignedPublicAction".into(),
            ));
        }
        let calldata =
            encode_action_calldata(&self.signed_action.action, &compact, &self.relay_argument)?;
        if encode_hex(&calldata) != self.calldata {
            return Err(PublicActionError::Calldata(
                "stored calldata is not the deterministic action encoding".into(),
            ));
        }
        Ok(())
    }
}

pub fn verify_submitted_state(
    package: &SignedPublicActionPackage,
    read: &PublicPreparationRead,
    receipt_block: u64,
) -> Result<(), PublicActionError> {
    package.verify()?;
    verify_closed_read_header(read)?;
    if read.block_number.is_none_or(|block| block < receipt_block) {
        return Err(PublicActionError::Network(
            "post-submit verification is incomplete or predates the receipt".into(),
        ));
    }
    let builder = read.builder_account.as_ref().ok_or_else(|| {
        PublicActionError::Network("post-submit BuilderAccount read is missing".into())
    })?;
    let builder_id = BuilderId::parse(&package.record_binding.builder_id)
        .map_err(|error| PublicActionError::Record(error.to_string()))?;
    if builder.account != builder_id.account()
        || builder.code_state != BuilderAccountCodeState::Exact
        || builder.queried_key_id != device_key_id(&package.signed_action.signer)
        || builder.protocol_permission != Some(true)
    {
        return Err(PublicActionError::Authority(
            "post-submit BuilderAccount authority is not exact".into(),
        ));
    }
    let registry = read.registry.as_ref().ok_or_else(|| {
        PublicActionError::Network("post-submit ShotRegistry read is missing".into())
    })?;
    if registry.controller != builder_id.account()
        || registry.head != package.record_binding.evolution_commitment
        || registry.sequence != u64::from(package.record_binding.sequence)
    {
        return Err(PublicActionError::Guard(
            "post-submit registry no longer matches the exact local record".into(),
        ));
    }
    match (&package.signed_action.action, &package.expected_state) {
        (
            PublicAction::CreateShot { .. } | PublicAction::AppendEvolution { .. },
            ExpectedPublicState::Evolution {
                controller,
                head,
                sequence,
                registry_nonce,
            },
        ) => {
            if registry.controller != *controller
                || registry.head != *head
                || registry.sequence != *sequence
                || registry.shot_nonce != *registry_nonce
            {
                return Err(PublicActionError::Guard(
                    "receipt succeeded but the expected ShotRegistry state was not observed".into(),
                ));
            }
        }
        (
            PublicAction::ClaimHandle { shot_id, .. },
            ExpectedPublicState::Handle {
                handle_hash,
                relations_nonce,
                ..
            },
        ) => {
            let relations = read.relations.as_ref().ok_or_else(|| {
                PublicActionError::Network("post-submit ShotRelations read is missing".into())
            })?;
            if relations.nonce != *relations_nonce
                || relations.handle_by_shot != *handle_hash
                || relations.shot_by_requested_handle != Some(shot_id.bytes())
            {
                return Err(PublicActionError::Guard(
                    "receipt succeeded but the expected handle relation was not observed".into(),
                ));
            }
        }
        (
            PublicAction::AssociateAppcoin { .. },
            ExpectedPublicState::Appcoin {
                chain_id,
                token,
                relations_nonce,
                ..
            },
        ) => {
            let relations = read.relations.as_ref().ok_or_else(|| {
                PublicActionError::Network("post-submit ShotRelations read is missing".into())
            })?;
            if relations.nonce != *relations_nonce
                || relations.appcoin_chain_id != *chain_id
                || relations.appcoin_token != *token
            {
                return Err(PublicActionError::Guard(
                    "receipt succeeded but the expected appcoin relation was not observed".into(),
                ));
            }
        }
        _ => {
            return Err(PublicActionError::Guard(
                "package expected state does not match its action".into(),
            ))
        }
    }
    Ok(())
}

fn verify_package_read_bindings(
    package: &SignedPublicActionPackage,
) -> Result<(), PublicActionError> {
    verify_closed_read_header(&package.read_evidence)?;
    let read = &package.read_evidence;
    let builder_id = BuilderId::parse(&package.record_binding.builder_id)
        .map_err(|error| PublicActionError::Record(error.to_string()))?;
    let builder = read.builder_account.as_ref().ok_or_else(|| {
        PublicActionError::Network("package has no BuilderAccount observation".into())
    })?;
    if builder.account != builder_id.account()
        || builder.queried_key_id != device_key_id(&package.signed_action.signer)
        || builder.code_state != BuilderAccountCodeState::Exact
        || builder.protocol_permission != Some(true)
    {
        return Err(PublicActionError::Authority(
            "package signer is not bound to an exact authorized BuilderAccount".into(),
        ));
    }
    let registry = read
        .registry
        .as_ref()
        .ok_or_else(|| PublicActionError::Network("package has no registry observation".into()))?;
    let sequence = u64::from(package.record_binding.sequence);

    match (
        &package.signed_action.action,
        &package.relay_argument,
        &package.expected_state,
    ) {
        (
            PublicAction::CreateShot {
                shot_id,
                controller,
                head,
                sequence: action_sequence,
                public_state,
                content_commitment,
                nonce,
                ..
            },
            RelayArgument::None,
            ExpectedPublicState::Evolution {
                controller: expected_controller,
                head: expected_head,
                sequence: expected_sequence,
                registry_nonce,
            },
        ) => {
            if shot_id.to_string() != package.record_binding.shot_id
                || *controller != builder_id.account()
                || *head != package.record_binding.evolution_commitment
                || *action_sequence != sequence
                || *public_state != PublicState::Published
                || *content_commitment != Bytes32::ZERO
                || *nonce != registry.create_nonce
                || !is_zero_address(registry.controller)
                || registry.head != Bytes32::ZERO
                || registry.sequence != 0
                || registry.shot_nonce != 0
                || *expected_controller != *controller
                || *expected_head != *head
                || *expected_sequence != *action_sequence
                || *registry_nonce != 1
            {
                return Err(PublicActionError::Guard(
                    "CREATE_SHOT package does not exactly bind its pre-state and local record"
                        .into(),
                ));
            }
        }
        (
            PublicAction::AppendEvolution {
                shot_id,
                previous_head,
                new_head,
                sequence: action_sequence,
                content_commitment,
                nonce,
                ..
            },
            RelayArgument::None,
            ExpectedPublicState::Evolution {
                controller,
                head,
                sequence: expected_sequence,
                registry_nonce,
            },
        ) => {
            if shot_id.to_string() != package.record_binding.shot_id
                || registry.controller != builder_id.account()
                || *controller != builder_id.account()
                || *previous_head != registry.head
                || *new_head != package.record_binding.evolution_commitment
                || *head != *new_head
                || registry
                    .sequence
                    .checked_add(1)
                    .is_none_or(|next| next != *action_sequence)
                || *action_sequence != sequence
                || *expected_sequence != *action_sequence
                || *content_commitment != Bytes32::ZERO
                || *nonce != registry.shot_nonce
                || registry_nonce.checked_sub(1) != Some(*nonce)
            {
                return Err(PublicActionError::Guard(
                    "APPEND_EVOLUTION package skips or differs from its exact predecessor".into(),
                ));
            }
        }
        (
            PublicAction::ClaimHandle {
                shot_id,
                handle_hash: action_hash,
                nonce,
                ..
            },
            RelayArgument::Handle { value },
            ExpectedPublicState::Handle {
                shot_id: expected_shot,
                handle_hash: expected_hash,
                relations_nonce,
            },
        ) => {
            let relations = exact_relation_base(package, builder_id, registry)?;
            if shot_id.to_string() != package.record_binding.shot_id
                || expected_shot != &package.record_binding.shot_id
                || handle_hash(value)? != *action_hash
                || *expected_hash != *action_hash
                || *nonce != relations.nonce
                || relations_nonce.checked_sub(1) != Some(*nonce)
                || relations.handle_by_shot != Bytes32::ZERO
                || relations.shot_by_requested_handle != Some(Bytes32::ZERO)
            {
                return Err(PublicActionError::Guard(
                    "CLAIM_HANDLE package does not exactly bind its relation pre-state".into(),
                ));
            }
        }
        (
            PublicAction::AssociateAppcoin {
                shot_id,
                chain_id,
                token,
                nonce,
                ..
            },
            RelayArgument::None,
            ExpectedPublicState::Appcoin {
                shot_id: expected_shot,
                chain_id: expected_chain,
                token: expected_token,
                relations_nonce,
            },
        ) => {
            let relations = exact_relation_base(package, builder_id, registry)?;
            if shot_id.to_string() != package.record_binding.shot_id
                || expected_shot != &package.record_binding.shot_id
                || *expected_chain != *chain_id
                || *expected_token != *token
                || *nonce != relations.nonce
                || relations_nonce.checked_sub(1) != Some(*nonce)
                || relations.appcoin_chain_id != 0
                || !is_zero_address(relations.appcoin_token)
            {
                return Err(PublicActionError::Guard(
                    "ASSOCIATE_APPCOIN package would overwrite or differs from its exact pre-state"
                        .into(),
                ));
            }
        }
        _ => {
            return Err(PublicActionError::Guard(
                "action, relay argument, and expected state are not a closed supported package"
                    .into(),
            ))
        }
    }
    Ok(())
}

fn verify_closed_read_header(read: &PublicPreparationRead) -> Result<(), PublicActionError> {
    const REQUIRED_CHECKS: [&str; 8] = [
        "candidate.plan",
        "network.chain_id",
        "network.code.deployer",
        "network.code.builder_account_factory",
        "network.code.shot_registry",
        "network.code.shot_relations",
        "network.p256verify",
        "network.relations.registry",
    ];
    if read.schema != crate::public_network::PUBLIC_PREPARATION_SCHEMA
        || read.block_number.is_none()
        || !read.read_complete
        || read.error.is_some()
        || read.network.schema != crate::public_network::NETWORK_STATUS_SCHEMA
        || !read.network.ready
        || read.network.chain_id != Some(ROBINHOOD_CHAIN_ID)
        || read.network.checks.len() != REQUIRED_CHECKS.len()
        || REQUIRED_CHECKS.iter().any(|required| {
            read.network
                .checks
                .iter()
                .filter(|check| check.id == *required && check.status == PublicCheckStatus::Pass)
                .count()
                != 1
        })
    {
        return Err(PublicActionError::Network(
            "package does not retain one closed, block-pinned candidate verification".into(),
        ));
    }
    Ok(())
}

fn exact_relation_base<'a>(
    package: &'a SignedPublicActionPackage,
    builder_id: BuilderId,
    registry: &RegistryPreparationState,
) -> Result<&'a crate::public_network::RelationsPreparationState, PublicActionError> {
    if registry.controller != builder_id.account()
        || registry.head != package.record_binding.evolution_commitment
        || registry.sequence != u64::from(package.record_binding.sequence)
    {
        return Err(PublicActionError::Guard(
            "relation package is not based on the exact public local head".into(),
        ));
    }
    package
        .read_evidence
        .relations
        .as_ref()
        .ok_or_else(|| PublicActionError::Network("package has no relations observation".into()))
}

impl BuilderAccountDeploymentRequest {
    pub fn verify(&self) -> Result<(), PublicActionError> {
        if self.schema != BUILDER_DEPLOYMENT_REQUEST_SCHEMA
            || self.chain_id != ROBINHOOD_CHAIN_ID
            || self.signed
            || self.submitted
            || !self.read_evidence.read_complete
            || !self.read_evidence.network.ready
        {
            return Err(PublicActionError::Guard(
                "BuilderAccount deployment request metadata is invalid".into(),
            ));
        }
        verify_closed_read_header(&self.read_evidence)?;
        let expected = encode_hex(&encode_builder_account_deployment(
            self.account_salt,
            &self.initial_public_key,
        ));
        let plan = embedded_deployment_plan()
            .map_err(|error| PublicActionError::Network(error.to_string()))?;
        let predicted = predict_builder_account(
            self.target,
            self.account_salt,
            &self.initial_public_key,
            &builder_account_creation_bytecode()
                .map_err(|error| PublicActionError::Authority(error.to_string()))?,
        )
        .map_err(|error| PublicActionError::Authority(error.to_string()))?;
        let factory_ready = self.read_evidence.network.checks.iter().any(|check| {
            check.id == "network.code.builder_account_factory"
                && check.status == PublicCheckStatus::Pass
        });
        let missing = self
            .read_evidence
            .builder_account
            .as_ref()
            .is_some_and(|builder| {
                builder.account == self.expected_account
                    && builder.queried_key_id == self.initial_key_id
                    && builder.code_state == BuilderAccountCodeState::Missing
                    && builder.protocol_permission.is_none()
            });
        if self.target != plan.contracts.builder_account_factory.planned_address
            || !factory_ready
            || !missing
            || predicted.account() != self.expected_account
            || self.calldata != expected
            || device_key_id(&self.initial_public_key) != self.initial_key_id
        {
            return Err(PublicActionError::Calldata(
                "BuilderAccount deployment calldata or key binding is invalid".into(),
            ));
        }
        Ok(())
    }
}

pub fn persist_signed_package(
    shot_root: &Path,
    package: &SignedPublicActionPackage,
) -> Result<PathBuf, PublicActionError> {
    package.verify()?;
    let digest = package.signed_action.authorization.digest.to_string();
    let signature_id = Bytes32::new(
        Keccak256::digest(
            decode_hex(&package.compact_signature)
                .map_err(|error| PublicActionError::Persistence(error.to_string()))?,
        )
        .into(),
    );
    persist_public_json(
        shot_root,
        &format!(
            "{}-{}.json",
            digest.trim_start_matches("0x"),
            signature_id.to_string().trim_start_matches("0x")
        ),
        package,
    )
}

pub fn persist_builder_deployment_request(
    shot_root: &Path,
    request: &BuilderAccountDeploymentRequest,
) -> Result<PathBuf, PublicActionError> {
    request.verify()?;
    let block = request.read_evidence.block_number.ok_or_else(|| {
        PublicActionError::Persistence("deployment request is not block-pinned".into())
    })?;
    persist_public_json(
        shot_root,
        &format!("builder-account-deployment-{block}.json"),
        request,
    )
}

fn persist_public_json(
    shot_root: &Path,
    filename: &str,
    value: &impl Serialize,
) -> Result<PathBuf, PublicActionError> {
    let tohseno = shot_root.join("TOHSENO");
    let tohseno_metadata = fs::symlink_metadata(&tohseno)?;
    if tohseno_metadata.file_type().is_symlink() || !tohseno_metadata.is_dir() {
        return Err(PublicActionError::Persistence(
            "TOHSENO must be a regular directory".into(),
        ));
    }
    verify_owned_directory(&tohseno_metadata, "TOHSENO", false)?;
    let directory = tohseno.join("public-actions");
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(PublicActionError::Persistence(
                "public-actions must be a regular directory".into(),
            ))
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_private_directory(&directory)?;
        }
        Err(error) => return Err(error.into()),
    }
    let directory_metadata = fs::symlink_metadata(&directory)?;
    if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
        return Err(PublicActionError::Persistence(
            "public-actions must be a regular directory".into(),
        ));
    }
    verify_owned_directory(&directory_metadata, "public-actions", true)?;
    let path = directory.join(filename);
    let bytes = canonical::to_vec(value)
        .map_err(|error| PublicActionError::Persistence(error.to_string()))?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    match options.open(&path) {
        Ok(mut file) => {
            verify_new_artifact_file(&path, &file)?;
            recheck_parent(&tohseno, &tohseno_metadata, "TOHSENO")?;
            recheck_parent(&directory, &directory_metadata, "public-actions")?;
            file.write_all(&bytes)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            verify_new_artifact_file(&path, &file)?;
            recheck_parent(&tohseno, &tohseno_metadata, "TOHSENO")?;
            recheck_parent(&directory, &directory_metadata, "public-actions")?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = read_existing_artifact(&path)?;
            let existing = existing.strip_suffix(b"\n").unwrap_or(&existing);
            if existing != bytes {
                return Err(PublicActionError::Persistence(
                    "an action artifact with this digest already differs".into(),
                ));
            }
        }
        Err(error) => return Err(error.into()),
    }
    Ok(path)
}

fn create_private_directory(path: &Path) -> Result<(), PublicActionError> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    Ok(())
}

fn read_existing_artifact(path: &Path) -> Result<Vec<u8>, PublicActionError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    verify_new_artifact_file(path, &file)?;
    let mut output = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut output)?;
    if output.len() > 1024 * 1024 {
        return Err(PublicActionError::Persistence(
            "existing public-action artifact exceeds 1 MiB".into(),
        ));
    }
    Ok(output)
}

fn verify_new_artifact_file(path: &Path, file: &fs::File) -> Result<(), PublicActionError> {
    let open_metadata = file.metadata()?;
    let path_metadata = fs::symlink_metadata(path)?;
    if !open_metadata.is_file()
        || path_metadata.file_type().is_symlink()
        || !path_metadata.is_file()
        || !same_file(&open_metadata, &path_metadata)
    {
        return Err(PublicActionError::Persistence(
            "public-action artifact path changed or is not a regular file".into(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if open_metadata.nlink() != 1
            || open_metadata.uid() != unsafe { libc::geteuid() }
            || open_metadata.permissions().mode() & 0o777 != 0o600
        {
            return Err(PublicActionError::Persistence(
                "public-action artifact must be a single-link 0600 file".into(),
            ));
        }
    }
    Ok(())
}

fn verify_owned_directory(
    metadata: &fs::Metadata,
    label: &str,
    private: bool,
) -> Result<(), PublicActionError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let mode = metadata.permissions().mode() & 0o777;
        if metadata.uid() != unsafe { libc::geteuid() }
            || mode & 0o022 != 0
            || (private && mode & 0o077 != 0)
        {
            return Err(PublicActionError::Persistence(format!(
                "{label} must be owned by this user and not writable by other users"
            )));
        }
    }
    #[cfg(not(unix))]
    let _ = (metadata, label, private);
    Ok(())
}

fn recheck_parent(
    path: &Path,
    expected: &fs::Metadata,
    label: &str,
) -> Result<(), PublicActionError> {
    let observed = fs::symlink_metadata(path)?;
    if observed.file_type().is_symlink() || !observed.is_dir() || !same_file(expected, &observed) {
        return Err(PublicActionError::Persistence(format!(
            "{label} directory changed during artifact creation"
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.created().ok() == right.created().ok()
}

pub fn encode_builder_account_deployment(salt: Bytes32, public_key: &P256PublicKey) -> Vec<u8> {
    let mut output = selector("createAccount(bytes32,uint256,uint256)").to_vec();
    output.extend_from_slice(salt.as_bytes());
    output.extend_from_slice(public_key.x.as_bytes());
    output.extend_from_slice(public_key.y.as_bytes());
    output
}

pub fn encode_action_calldata(
    action: &PublicAction,
    compact_signature: &[u8],
    relay_argument: &RelayArgument,
) -> Result<Vec<u8>, PublicActionError> {
    if compact_signature.len() != 129 || compact_signature.first() != Some(&1) {
        return Err(PublicActionError::Calldata(
            "compact signature must be exact versioned 129-byte form".into(),
        ));
    }
    match (action, relay_argument) {
        (
            PublicAction::CreateShot {
                shot_id,
                controller,
                head,
                sequence,
                public_state,
                content_commitment,
                nonce,
                deadline,
            },
            RelayArgument::None,
        ) => Ok(encode_static_tuple_and_bytes(
            "createShot((bytes32,address,bytes32,uint64,uint8,bytes32,uint64,uint64),bytes)",
            &[
                shot_id.bytes(),
                address_word(*controller),
                *head,
                u64_word(*sequence),
                u64_word(*public_state as u64),
                *content_commitment,
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            compact_signature,
        )),
        (
            PublicAction::AppendEvolution {
                shot_id,
                previous_head,
                new_head,
                sequence,
                content_commitment,
                nonce,
                deadline,
            },
            RelayArgument::None,
        ) => Ok(encode_static_tuple_and_bytes(
            "appendEvolution((bytes32,bytes32,bytes32,uint64,bytes32,uint64,uint64),bytes)",
            &[
                shot_id.bytes(),
                *previous_head,
                *new_head,
                u64_word(*sequence),
                *content_commitment,
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            compact_signature,
        )),
        (
            PublicAction::ClaimHandle {
                shot_id,
                handle_hash,
                nonce,
                deadline,
            },
            RelayArgument::Handle { value },
        ) => {
            validate_handle(value)?;
            let words = [
                shot_id.bytes(),
                *handle_hash,
                u64_word(*nonce),
                u64_word(*deadline),
            ];
            Ok(encode_tuple_string_and_bytes(
                "claimHandle((bytes32,bytes32,uint64,uint64),string,bytes)",
                &words,
                value.as_bytes(),
                compact_signature,
            ))
        }
        (
            PublicAction::AssociateAppcoin {
                shot_id,
                chain_id,
                token,
                nonce,
                deadline,
            },
            RelayArgument::None,
        ) => Ok(encode_static_tuple_and_bytes(
            "associateAppcoin((bytes32,uint256,address,uint64,uint64),bytes)",
            &[
                shot_id.bytes(),
                u64_word(*chain_id),
                address_word(*token),
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            compact_signature,
        )),
        _ => Err(PublicActionError::Calldata(
            "action and relay argument are not a supported public preparation".into(),
        )),
    }
}

fn encode_static_tuple_and_bytes(signature: &str, words: &[Bytes32], dynamic: &[u8]) -> Vec<u8> {
    let mut output = selector(signature).to_vec();
    for word in words {
        output.extend_from_slice(word.as_bytes());
    }
    output.extend_from_slice(u64_word(((words.len() + 1) * 32) as u64).as_bytes());
    append_dynamic(&mut output, dynamic);
    output
}

fn encode_tuple_string_and_bytes(
    signature: &str,
    words: &[Bytes32],
    string: &[u8],
    bytes: &[u8],
) -> Vec<u8> {
    let head_bytes = (words.len() + 2) * 32;
    let string_bytes = 32 + padded_len(string.len());
    let mut output = selector(signature).to_vec();
    for word in words {
        output.extend_from_slice(word.as_bytes());
    }
    output.extend_from_slice(u64_word(head_bytes as u64).as_bytes());
    output.extend_from_slice(u64_word((head_bytes + string_bytes) as u64).as_bytes());
    append_dynamic(&mut output, string);
    append_dynamic(&mut output, bytes);
    output
}

fn append_dynamic(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(u64_word(value.len() as u64).as_bytes());
    output.extend_from_slice(value);
    output.resize(output.len() + (padded_len(value.len()) - value.len()), 0);
}

fn padded_len(length: usize) -> usize {
    length.div_ceil(32) * 32
}

fn selector(signature: &str) -> [u8; 4] {
    Keccak256::digest(signature.as_bytes())[..4]
        .try_into()
        .expect("four-byte selector")
}

fn address_word(value: Address20) -> Bytes32 {
    let mut output = [0_u8; 32];
    output[12..].copy_from_slice(value.as_bytes());
    Bytes32::new(output)
}

fn u64_word(value: u64) -> Bytes32 {
    let mut output = [0_u8; 32];
    output[24..].copy_from_slice(&value.to_be_bytes());
    Bytes32::new(output)
}

pub fn handle_hash(handle: &str) -> Result<Bytes32, PublicActionError> {
    validate_handle(handle)?;
    Ok(Bytes32::new(Keccak256::digest(handle.as_bytes()).into()))
}

fn validate_handle(handle: &str) -> Result<(), PublicActionError> {
    let valid = !handle.is_empty()
        && handle.len() <= 63
        && handle.is_ascii()
        && !handle.starts_with('-')
        && !handle.ends_with('-')
        && handle
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(PublicActionError::Guard(
            "handle must match the exact lowercase contract vocabulary".into(),
        ))
    }
}

fn is_zero_address(address: Address20) -> bool {
    address.as_bytes().iter().all(|byte| *byte == 0)
}

fn encode_hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(2 + bytes.len() * 2);
    output.push_str("0x");
    for byte in bytes {
        output.push(TABLE[(byte >> 4) as usize] as char);
        output.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_hex(value: &str) -> Result<Vec<u8>, PublicActionError> {
    let encoded = value.strip_prefix("0x").ok_or_else(|| {
        PublicActionError::Calldata("hex value must have a lowercase 0x prefix".into())
    })?;
    if encoded.len() % 2 != 0 {
        return Err(PublicActionError::Calldata(
            "hex value has odd length".into(),
        ));
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn hex_nibble(value: u8) -> Result<u8, PublicActionError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(PublicActionError::Calldata(
            "hex value must be lowercase".into(),
        )),
    }
}

#[derive(Debug)]
pub enum PublicActionError {
    Authority(String),
    Record(String),
    Network(String),
    Guard(String),
    Signature(String),
    Calldata(String),
    Persistence(String),
    Io(std::io::Error),
}

impl std::fmt::Display for PublicActionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Authority(message) => write!(formatter, "public authority refused: {message}"),
            Self::Record(message) => write!(formatter, "local Shot refused: {message}"),
            Self::Network(message) => write!(formatter, "public read refused: {message}"),
            Self::Guard(message) => write!(formatter, "public action refused: {message}"),
            Self::Signature(message) => write!(formatter, "public signature refused: {message}"),
            Self::Calldata(message) => write!(formatter, "public calldata refused: {message}"),
            Self::Persistence(message) => {
                write!(formatter, "public action persistence refused: {message}")
            }
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for PublicActionError {}

impl From<std::io::Error> for PublicActionError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::hazmat::PrehashSigner;
    use p256::ecdsa::{Signature, SigningKey};
    use tohseno_protocol::digest::ShotId;
    use tohseno_protocol::record::{
        CanonicalTimestamp, FactoryDescriptor, APPLE_FASCIA_ID, PROTOCOL_NAME, SHOT_SCHEMA,
    };
    use tohseno_protocol::signature::{P256Signature, SignatureAlgorithm};

    fn signer_for(key: &SigningKey) -> P256PublicKey {
        let point = key.verifying_key().to_encoded_point(false);
        let mut x = [0_u8; 32];
        x.copy_from_slice(point.x().unwrap());
        let mut y = [0_u8; 32];
        y.copy_from_slice(point.y().unwrap());
        P256PublicKey {
            x: Bytes32::new(x),
            y: Bytes32::new(y),
        }
    }

    fn planning_record() -> ShotRecord {
        ShotRecord {
            protocol: PROTOCOL_NAME.into(),
            schema: SHOT_SCHEMA.into(),
            shot_id: ShotId::from_bytes([0x11; 32]),
            slug: "public-fixture".into(),
            builder_id: BuilderId::new(Address20::from_bytes([0x22; 20])),
            sequence: 2,
            previous: Some(Bytes32::new([0x77; 32])),
            fascia: APPLE_FASCIA_ID.into(),
            bundle_id: "com.example.public-fixture".into(),
            bundle_version: 2,
            genesis_input_sha256: Bytes32::new([0x33; 32]),
            source_tree_sha256: Bytes32::new([0x44; 32]),
            fascia_sha256: Bytes32::new([0x55; 32]),
            factory: FactoryDescriptor {
                implementation: "test/factory".into(),
                version: "1.0.0-test".into(),
                source_commit: "a".repeat(40),
            },
            created_at: CanonicalTimestamp::parse("2026-07-28T00:00:00Z").unwrap(),
            origin: None,
        }
    }

    fn ready_network() -> crate::public_network::NetworkStatusReport {
        crate::public_network::NetworkStatusReport {
            schema: crate::public_network::NETWORK_STATUS_SCHEMA.into(),
            ready: true,
            chain_id: Some(ROBINHOOD_CHAIN_ID),
            checks: [
                "candidate.plan",
                "network.chain_id",
                "network.code.deployer",
                "network.code.builder_account_factory",
                "network.code.shot_registry",
                "network.code.shot_relations",
                "network.p256verify",
                "network.relations.registry",
            ]
            .into_iter()
            .map(|id| crate::public_network::PublicCheck {
                id: id.into(),
                status: PublicCheckStatus::Pass,
                expected: "pinned".into(),
                observed: "pinned".into(),
                evidence: "fixture".into(),
            })
            .collect(),
        }
    }

    fn planning_read(
        authority: &PublicAuthority,
        registry: RegistryPreparationState,
        relations: Option<crate::public_network::RelationsPreparationState>,
    ) -> PublicPreparationRead {
        PublicPreparationRead {
            schema: crate::public_network::PUBLIC_PREPARATION_SCHEMA.into(),
            block_number: Some(123),
            read_complete: true,
            network: ready_network(),
            builder_account: Some(crate::public_network::BuilderAccountObservation {
                account: authority.account,
                queried_key_id: authority.signer_key_id,
                code_state: BuilderAccountCodeState::Exact,
                runtime_size: 8_132,
                runtime_keccak256: Some(Bytes32::new([9; 32])),
                protocol_permission: Some(true),
            }),
            registry: Some(registry),
            relations,
            error: None,
        }
    }

    #[test]
    fn selectors_and_dynamic_offsets_are_exact() {
        assert_eq!(
            encode_hex(&selector("createAccount(bytes32,uint256,uint256)")),
            "0xba8e9fcb"
        );
        assert_eq!(
            encode_hex(&selector(
                "createShot((bytes32,address,bytes32,uint64,uint8,bytes32,uint64,uint64),bytes)"
            )),
            "0x29c0060d"
        );
        assert_eq!(
            encode_hex(&selector(
                "appendEvolution((bytes32,bytes32,bytes32,uint64,bytes32,uint64,uint64),bytes)"
            )),
            "0x8f7427e8"
        );
        assert_eq!(
            encode_hex(&selector(
                "claimHandle((bytes32,bytes32,uint64,uint64),string,bytes)"
            )),
            "0x3e82a990"
        );
        assert_eq!(
            encode_hex(&selector(
                "associateAppcoin((bytes32,uint256,address,uint64,uint64),bytes)"
            )),
            "0x8afde7bd"
        );

        let action = PublicAction::ClaimHandle {
            shot_id: tohseno_protocol::digest::ShotId::from_bytes([1; 32]),
            handle_hash: Bytes32::new([2; 32]),
            nonce: 3,
            deadline: 4,
        };
        let compact = [1_u8; 129];
        let encoded = encode_action_calldata(
            &action,
            &compact,
            &RelayArgument::Handle {
                value: "field-notebook".into(),
            },
        )
        .unwrap();
        assert_eq!(&encoded[..4], &[0x3e, 0x82, 0xa9, 0x90]);
        assert_eq!(decode_word_u64(&encoded[4 + 4 * 32..4 + 5 * 32]), 192);
        assert_eq!(decode_word_u64(&encoded[4 + 5 * 32..4 + 6 * 32]), 256);
        assert_eq!(decode_word_u64(&encoded[4 + 6 * 32..4 + 7 * 32]), 14);
        assert_eq!(decode_word_u64(&encoded[4 + 8 * 32..4 + 9 * 32]), 129);
    }

    #[test]
    fn planning_requires_exact_predecessor_and_refuses_appcoin_overwrite() {
        let key = SigningKey::from_bytes((&[1_u8; 32]).into()).unwrap();
        let signer = signer_for(&key);
        let record = planning_record();
        let authority = PublicAuthority {
            builder_id: record.builder_id,
            account: record.builder_id.account(),
            signer: signer.clone(),
            signer_key_id: device_key_id(&signer),
            account_salt: Bytes32::new([0x42; 32]),
        };
        let registry = RegistryPreparationState {
            controller: authority.account,
            head: record.previous.unwrap(),
            sequence: 1,
            shot_nonce: 4,
            create_nonce: 2,
        };
        let plan = embedded_deployment_plan().unwrap();
        let read = planning_read(&authority, registry.clone(), None);
        let outcome = plan_public_action(
            &plan,
            &record,
            &authority,
            &read,
            PublicActionRequest::Publish,
            2_000,
            1_000,
        )
        .unwrap();
        let PlanOutcome::Action(planned) = outcome else {
            panic!("expected append preparation")
        };
        assert!(matches!(
            planned.action,
            PublicAction::AppendEvolution {
                previous_head,
                sequence: 2,
                content_commitment: Bytes32::ZERO,
                nonce: 4,
                ..
            } if previous_head == record.previous.unwrap()
        ));

        let mut skipped = registry.clone();
        skipped.sequence = 0;
        assert!(plan_public_action(
            &plan,
            &record,
            &authority,
            &planning_read(&authority, skipped, None),
            PublicActionRequest::Publish,
            2_000,
            1_000,
        )
        .is_err());

        let public_registry = RegistryPreparationState {
            controller: authority.account,
            head: record.commitment().unwrap(),
            sequence: 2,
            shot_nonce: 5,
            create_nonce: 2,
        };
        let existing_appcoin = crate::public_network::RelationsPreparationState {
            nonce: 3,
            handle_by_shot: Bytes32::ZERO,
            shot_by_requested_handle: None,
            appcoin_chain_id: 1,
            appcoin_token: Address20::from_bytes([8; 20]),
        };
        assert!(plan_public_action(
            &plan,
            &record,
            &authority,
            &planning_read(&authority, public_registry, Some(existing_appcoin)),
            PublicActionRequest::AssociateAppcoin {
                chain_id: 4663,
                token: Address20::from_bytes([7; 20]),
            },
            2_000,
            1_000,
        )
        .is_err());
    }

    #[test]
    fn compact_signature_and_calldata_round_trip_in_closed_package() {
        let key = SigningKey::from_bytes((&[1_u8; 32]).into()).unwrap();
        let signer = signer_for(&key);
        let signer_key_id = device_key_id(&signer);
        let action = PublicAction::AssociateAppcoin {
            shot_id: tohseno_protocol::digest::ShotId::from_bytes([1; 32]),
            chain_id: 4663,
            token: Address20::from_bytes([2; 20]),
            nonce: 0,
            deadline: 2_000,
        };
        let domain = Eip712Domain {
            name: SHOT_RELATIONS_DOMAIN.into(),
            version: EIP712_VERSION.into(),
            chain_id: ROBINHOOD_CHAIN_ID,
            verifying_contract: embedded_deployment_plan()
                .unwrap()
                .contracts
                .shot_relations
                .planned_address,
        };
        let digest = action.digest(&domain).unwrap();
        let signature: Signature = key.sign_prehash(digest.as_bytes()).unwrap();
        let signature = signature.normalize_s().unwrap_or(signature);
        let raw = signature.to_bytes();
        let detached = DetachedP256Signature {
            algorithm: SignatureAlgorithm::P256,
            digest,
            signature: P256Signature {
                r: Bytes32::new(raw[..32].try_into().unwrap()),
                s: Bytes32::new(raw[32..].try_into().unwrap()),
            },
            low_s: true,
        };
        let network = ready_network();
        let planned = PlannedPublicAction {
            domain,
            action,
            signer,
            digest,
            relay_argument: RelayArgument::None,
            expected_state: ExpectedPublicState::Appcoin {
                shot_id: format!("0x{}", "01".repeat(32)),
                chain_id: 4663,
                token: Address20::from_bytes([2; 20]),
                relations_nonce: 1,
            },
            record_binding: PublicRecordBinding {
                shot_id: format!("0x{}", "01".repeat(32)),
                builder_id: format!("eip155:4663:0x{}", "02".repeat(20)),
                evolution_commitment: Bytes32::new([4; 32]),
                sequence: 1,
            },
            read_evidence: PublicPreparationRead {
                schema: crate::public_network::PUBLIC_PREPARATION_SCHEMA.into(),
                block_number: Some(123),
                read_complete: true,
                network,
                builder_account: Some(crate::public_network::BuilderAccountObservation {
                    account: Address20::from_bytes([2; 20]),
                    queried_key_id: signer_key_id,
                    code_state: BuilderAccountCodeState::Exact,
                    runtime_size: 8_132,
                    runtime_keccak256: Some(Bytes32::new([9; 32])),
                    protocol_permission: Some(true),
                }),
                registry: Some(RegistryPreparationState {
                    controller: Address20::from_bytes([2; 20]),
                    head: Bytes32::new([4; 32]),
                    sequence: 1,
                    shot_nonce: 1,
                    create_nonce: 1,
                }),
                relations: Some(crate::public_network::RelationsPreparationState {
                    nonce: 0,
                    handle_by_shot: Bytes32::ZERO,
                    shot_by_requested_handle: None,
                    appcoin_chain_id: 0,
                    appcoin_token: Address20::from_bytes([0; 20]),
                }),
                error: None,
            },
        };
        let package = finalize_planned_action(planned, detached).unwrap();
        package.verify().unwrap();
        let compact = decode_hex(&package.compact_signature).unwrap();
        assert_eq!(compact.len(), 129);
        assert_eq!(compact[0], 1);
        assert_eq!(
            &decode_hex(&package.calldata).unwrap()[..4],
            &[0x8a, 0xfd, 0xe7, 0xbd]
        );

        let mut post = package.read_evidence.clone();
        post.block_number = Some(124);
        let relations = post.relations.as_mut().unwrap();
        relations.nonce = 1;
        relations.appcoin_chain_id = 4663;
        relations.appcoin_token = Address20::from_bytes([2; 20]);
        verify_submitted_state(&package, &post, 124).unwrap();
        post.relations.as_mut().unwrap().appcoin_token = Address20::from_bytes([8; 20]);
        assert!(verify_submitted_state(&package, &post, 124).is_err());
    }

    #[test]
    fn artifact_persistence_is_private_create_new_and_symlink_safe() {
        let temporary = tempfile::tempdir().unwrap();
        fs::create_dir(temporary.path().join("TOHSENO")).unwrap();
        #[cfg(unix)]
        fs::set_permissions(
            temporary.path().join("TOHSENO"),
            <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
        )
        .unwrap();
        let value = serde_json::json!({"schema": "fixture/1", "value": 1});
        let path = persist_public_json(temporary.path(), "fixture.json", &value).unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            br#"{"schema":"fixture/1","value":1}
"#
        );
        persist_public_json(temporary.path(), "fixture.json", &value).unwrap();
        assert!(persist_public_json(
            temporary.path(),
            "fixture.json",
            &serde_json::json!({"schema": "fixture/1", "value": 2}),
        )
        .is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::{symlink, PermissionsExt};
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(temporary.path().join("TOHSENO/public-actions"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );

            let other = tempfile::tempdir().unwrap();
            fs::create_dir(other.path().join("TOHSENO")).unwrap();
            fs::set_permissions(
                other.path().join("TOHSENO"),
                <fs::Permissions as PermissionsExt>::from_mode(0o700),
            )
            .unwrap();
            symlink(
                temporary.path().join("TOHSENO/public-actions"),
                other.path().join("TOHSENO/public-actions"),
            )
            .unwrap();
            assert!(
                persist_public_json(other.path(), "refused.json", &value).is_err(),
                "a symlinked artifact parent must be refused"
            );
        }
    }

    fn decode_word_u64(word: &[u8]) -> u64 {
        assert_eq!(word.len(), 32);
        assert!(word[..24].iter().all(|byte| *byte == 0));
        u64::from_be_bytes(word[24..].try_into().unwrap())
    }
}
