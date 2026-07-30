//! Signed append-only lineage actions and deterministic Shot-state reduction.
//!
//! Cryptographic segment verification is deliberately separate from authority
//! reduction. A node with only a middle segment can prove canonical bytes,
//! signatures, adjacency, and tamper resistance without pretending it knows
//! the missing ownership history.

use crate::builder::MAX_SAFE_JSON_INTEGER;
use crate::canonical;
use crate::digest::{Bytes32, ExpressionId, ShotId, VersionId};
use crate::evolution::verify_lineage;
use crate::identity::BuilderId;
use crate::ontology::{
    capability_graph_digest, organ_acceptance_gate_name, ArtifactAvailabilityRecord,
    AvailabilityStatus, Evolution, EvolutionaryIntent, Expression, Feedback, Genome,
    GenomeAcceptance, GenomeProposal, Organ, Ownership, ParentRelation, ShotCommitment,
    TokenAssociation, TokenAssociationOperation, VerificationResult, VersionRecord, Visibility,
};
use crate::record::{CanonicalTimestamp, ShotRecord};
use crate::signature::{P256PublicKey, P256Signature, SignatureAlgorithm, SignatureSidecar};
use crate::text::invalid;
use crate::{ProtocolError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const LINEAGE_PROTOCOL: &str = "tohseno";
pub const LINEAGE_PROTOCOL_VERSION: &str = "2";
pub const LINEAGE_ACTION_SCHEMA: &str = "tohseno.lineage-action/2";
pub const LINEAGE_SCHEMA_VERSION: u32 = 2;

/// Closed action payload set. The `type` discriminator is inside the payload,
/// and each variant's record is itself a closed, versioned canonical object.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LineagePayload {
    Commitment(ShotCommitment),
    Intention(crate::ontology::IntentionRecord),
    GenomeProposal(GenomeProposal),
    GenomeAcceptance(GenomeAcceptance),
    Expression(Expression),
    Organ(Organ),
    VerificationResult(VerificationResult),
    Version(VersionRecord),
    Feedback(Feedback),
    EvolutionaryIntent(EvolutionaryIntent),
    Evolution(Evolution),
    Ownership(Ownership),
    TokenAssociation(TokenAssociation),
    ArtifactAvailability(ArtifactAvailabilityRecord),
    ParentRelation(ParentRelation),
}

impl LineagePayload {
    pub fn validate(&self, shot_id: ShotId) -> Result<()> {
        match self {
            Self::Commitment(record) => record.validate(),
            Self::Intention(record) => record.validate(),
            Self::GenomeProposal(record) => record.validate(),
            Self::GenomeAcceptance(record) => record.validate(),
            Self::Expression(record) => record.validate(),
            Self::Organ(record) => record.validate(),
            Self::VerificationResult(record) => record.validate(),
            Self::Version(record) => record.validate(shot_id),
            Self::Feedback(record) => record.validate(),
            Self::EvolutionaryIntent(record) => record.validate(),
            Self::Evolution(record) => record.validate(),
            Self::Ownership(record) => record.validate(),
            Self::TokenAssociation(record) => record.validate(),
            Self::ArtifactAvailability(record) => record.validate(),
            Self::ParentRelation(record) => record.validate(),
        }
    }

    pub fn digest(&self) -> Result<Bytes32> {
        canonical::sha256_commitment(self)
    }

    pub const fn action_type(&self) -> &'static str {
        match self {
            Self::Commitment(_) => "commitment",
            Self::Intention(_) => "intention",
            Self::GenomeProposal(_) => "genome_proposal",
            Self::GenomeAcceptance(_) => "genome_acceptance",
            Self::Expression(_) => "expression",
            Self::Organ(_) => "organ",
            Self::VerificationResult(_) => "verification_result",
            Self::Version(_) => "version",
            Self::Feedback(_) => "feedback",
            Self::EvolutionaryIntent(_) => "evolutionary_intent",
            Self::Evolution(_) => "evolution",
            Self::Ownership(_) => "ownership",
            Self::TokenAssociation(_) => "token_association",
            Self::ArtifactAvailability(_) => "artifact_availability",
            Self::ParentRelation(_) => "parent_relation",
        }
    }
}

/// One immutable signed action in a Shot's canonical history.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineageAction {
    pub protocol: String,
    pub protocol_version: String,
    pub schema: String,
    pub schema_version: u32,
    pub sequence: u64,
    pub previous: Option<Bytes32>,
    pub shot_id: ShotId,
    pub actor: BuilderId,
    pub timestamp: CanonicalTimestamp,
    /// Publisher-declared handling at issuance. Changing observed artifact
    /// availability is a later `artifact_availability` action, never a rewrite.
    pub availability: AvailabilityStatus,
    pub payload: LineagePayload,
    pub payload_digest: Bytes32,
}

impl LineageAction {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sequence: u64,
        previous: Option<Bytes32>,
        shot_id: ShotId,
        actor: BuilderId,
        timestamp: CanonicalTimestamp,
        availability: AvailabilityStatus,
        payload: LineagePayload,
    ) -> Result<Self> {
        let payload_digest = payload.digest()?;
        let action = Self {
            protocol: LINEAGE_PROTOCOL.into(),
            protocol_version: LINEAGE_PROTOCOL_VERSION.into(),
            schema: LINEAGE_ACTION_SCHEMA.into(),
            schema_version: LINEAGE_SCHEMA_VERSION,
            sequence,
            previous,
            shot_id,
            actor,
            timestamp,
            availability,
            payload,
            payload_digest,
        };
        action.validate()?;
        Ok(action)
    }

    pub fn validate(&self) -> Result<()> {
        if self.protocol != LINEAGE_PROTOCOL {
            return Err(invalid(
                "lineage_action.protocol",
                format!("must be {LINEAGE_PROTOCOL}"),
            ));
        }
        if self.protocol_version != LINEAGE_PROTOCOL_VERSION {
            return Err(invalid(
                "lineage_action.protocol_version",
                format!("must be {LINEAGE_PROTOCOL_VERSION}"),
            ));
        }
        if self.schema != LINEAGE_ACTION_SCHEMA {
            return Err(invalid(
                "lineage_action.schema",
                format!("must be {LINEAGE_ACTION_SCHEMA}"),
            ));
        }
        if self.schema_version != LINEAGE_SCHEMA_VERSION {
            return Err(invalid(
                "lineage_action.schema_version",
                format!("must be {LINEAGE_SCHEMA_VERSION}"),
            ));
        }
        if self.sequence == 0 || self.sequence > MAX_SAFE_JSON_INTEGER {
            return Err(invalid(
                "lineage_action.sequence",
                "must be a positive JSON-safe integer",
            ));
        }
        if self.shot_id.is_zero() {
            return Err(invalid("lineage_action.shot_id", "must not be zero"));
        }
        self.actor.validate()?;
        if !matches!(
            self.availability,
            AvailabilityStatus::IntentionallyPrivate | AvailabilityStatus::PubliclyAvailable
        ) {
            return Err(invalid(
                "lineage_action.availability",
                "signed actions declare intentionally_private or publicly_available handling; mutable observations use artifact-availability actions",
            ));
        }
        self.payload.validate(self.shot_id)?;
        if let LineagePayload::Feedback(feedback) = &self.payload {
            let expected = match feedback.visibility {
                Visibility::Private => AvailabilityStatus::IntentionallyPrivate,
                Visibility::Public => AvailabilityStatus::PubliclyAvailable,
            };
            if self.availability != expected {
                return Err(invalid(
                    "lineage_action.availability",
                    "must exactly match feedback visibility: private feedback is intentionally_private and public feedback is publicly_available",
                ));
            }
        }
        self.verify_payload_digest()
    }

    pub fn verify_payload_digest(&self) -> Result<()> {
        if self.payload_digest != self.payload.digest()? {
            return Err(ProtocolError::DigestMismatch);
        }
        Ok(())
    }

    /// SHA-256 of RFC 8785 canonical action bytes. This is both the lineage
    /// link and the digest signed by the existing v1 P-256 sidecar format.
    pub fn commitment(&self) -> Result<Bytes32> {
        self.validate()?;
        canonical::sha256_commitment(self)
    }

    pub fn signing_digest(&self) -> Result<Bytes32> {
        self.commitment()
    }

    /// Attaches a signature produced by an external key boundary. The protocol
    /// crate never accepts or stores private key material.
    pub fn attach_signature(
        self,
        public_key: P256PublicKey,
        signature: P256Signature,
    ) -> Result<SignedLineageAction> {
        public_key.validate()?;
        signature.validate_low_s()?;
        let digest = self.signing_digest()?;
        let signed = SignedLineageAction {
            action: self,
            signature: SignatureSidecar {
                schema: SignatureSidecar::SCHEMA.into(),
                algorithm: SignatureAlgorithm::P256,
                digest,
                public_key,
                signature,
                low_s: true,
            },
        };
        signed.verify()?;
        Ok(signed)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedLineageAction {
    pub action: LineageAction,
    pub signature: SignatureSidecar,
}

impl SignedLineageAction {
    pub fn new(action: LineageAction, signature: SignatureSidecar) -> Result<Self> {
        let signed = Self { action, signature };
        signed.verify()?;
        Ok(signed)
    }

    /// Verifies shape, payload digest, action commitment, and P-256 signature.
    /// It intentionally does not claim the signer was the authorized owner;
    /// that requires `reduce_lineage` or `apply_lineage_actions`.
    pub fn verify(&self) -> Result<()> {
        self.action.validate()?;
        self.signature.verify(&self.action)
    }

    pub fn commitment(&self) -> Result<Bytes32> {
        self.verify()?;
        self.action.commitment()
    }
}

/// Trusted boundary immediately before a partial contiguous segment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageSegmentAnchor {
    pub shot_id: ShotId,
    pub sequence: u64,
    pub head: Bytes32,
    pub timestamp: CanonicalTimestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedLineageSegment {
    pub shot_id: ShotId,
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub head: Bytes32,
    pub complete_from_commitment: bool,
    /// False for an unanchored middle segment: cryptography and adjacency are
    /// verified, but missing ownership history prevents an authority claim.
    pub authority_context_available: bool,
}

/// Verifies a complete prefix or a partial contiguous segment without any
/// filesystem or network policy.
pub fn verify_lineage_segment(
    actions: &[SignedLineageAction],
    anchor: Option<&LineageSegmentAnchor>,
) -> Result<VerifiedLineageSegment> {
    let Some(first) = actions.first() else {
        return Err(ProtocolError::LineageAction {
            sequence: 0,
            reason: "segment is empty".into(),
        });
    };
    let shot_id = first.action.shot_id;
    let mut expected_sequence = match anchor {
        Some(value) => value
            .sequence
            .checked_add(1)
            .ok_or_else(|| action_error(value.sequence, "anchor sequence overflowed"))?,
        None => first.action.sequence,
    };
    let mut previous = anchor.map(|value| value.head);
    let mut prior_time = anchor.map(|value| value.timestamp.unix_timestamp());
    if anchor.is_some_and(|value| value.shot_id != shot_id) {
        return Err(action_error(
            first.action.sequence,
            "segment ShotID does not match its anchor",
        ));
    }

    for signed in actions {
        let action = &signed.action;
        let fail = |reason: &str| action_error(action.sequence, reason);
        signed
            .verify()
            .map_err(|error| fail(&format!("action verification failed: {error}")))?;
        if action.shot_id != shot_id {
            return Err(fail("ShotID changed within the segment"));
        }
        if action.sequence != expected_sequence {
            return Err(fail("sequence is not contiguous"));
        }
        if (anchor.is_some() || action.sequence > first.action.sequence)
            && action.previous != previous
        {
            return Err(fail("previous commitment does not match"));
        }
        if prior_time.is_some_and(|value| action.timestamp.unix_timestamp() < value) {
            return Err(fail("timestamp moved backwards"));
        }
        previous = Some(action.commitment()?);
        prior_time = Some(action.timestamp.unix_timestamp());
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or_else(|| fail("sequence overflowed"))?;
    }

    let complete_from_commitment = anchor.is_none()
        && first.action.sequence == 1
        && first.action.previous.is_none()
        && matches!(first.action.payload, LineagePayload::Commitment(_));
    Ok(VerifiedLineageSegment {
        shot_id,
        first_sequence: first.action.sequence,
        last_sequence: actions.last().expect("nonempty").action.sequence,
        head: previous.expect("a verified action supplied a commitment"),
        complete_from_commitment,
        authority_context_available: anchor.is_some() || complete_from_commitment,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedGenome {
    pub genome: Genome,
    pub proposal_action: Bytes32,
    pub acceptance_action: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpressionState {
    pub expression: Expression,
    pub organs: BTreeMap<String, Organ>,
    pub versions: Vec<VersionRecord>,
    pub current_version: Option<VersionId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionRecord<T> {
    pub action: Bytes32,
    pub record: T,
}

/// Deterministic state derived from one fully authorized contiguous prefix.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotState {
    pub shot_id: ShotId,
    pub commitment: ShotCommitment,
    pub intention: Option<crate::ontology::IntentionRecord>,
    pub controller: BuilderId,
    pub controller_key: P256PublicKey,
    pub accepted_genome: Option<AcceptedGenome>,
    pub expressions: BTreeMap<ExpressionId, ExpressionState>,
    pub genome_proposals: BTreeMap<Bytes32, GenomeProposal>,
    pub genome_acceptances: BTreeMap<Bytes32, GenomeAcceptance>,
    pub verification_results: BTreeMap<Bytes32, VerificationResult>,
    pub feedback: BTreeMap<Bytes32, Feedback>,
    pub evolutionary_intents: BTreeMap<Bytes32, EvolutionaryIntent>,
    pub evolutions: Vec<ActionRecord<Evolution>>,
    pub ownership_history: Vec<ActionRecord<Ownership>>,
    /// Current v1-compatible single association. Every replacement/removal is
    /// retained in `token_history`.
    pub token_association: Option<TokenAssociation>,
    pub token_history: Vec<ActionRecord<TokenAssociation>>,
    pub availability: BTreeMap<Bytes32, ArtifactAvailabilityRecord>,
    pub parent_relations: Vec<ActionRecord<ParentRelation>>,
    pub sequence: u64,
    pub head: Bytes32,
    pub last_timestamp: CanonicalTimestamp,
}

impl ShotState {
    pub fn expression(&self, expression_id: ExpressionId) -> Option<&ExpressionState> {
        self.expressions.get(&expression_id)
    }
}

/// Reduces a full lineage beginning at the signed commitment action.
pub fn reduce_lineage(actions: &[SignedLineageAction]) -> Result<ShotState> {
    verify_lineage_segment(actions, None)?;
    let Some(first) = actions.first() else {
        return Err(action_error(0, "lineage is empty"));
    };
    if first.action.sequence != 1 || first.action.previous.is_some() {
        return Err(action_error(
            first.action.sequence,
            "full lineage must begin at sequence 1 with no previous action",
        ));
    }
    let LineagePayload::Commitment(commitment) = &first.action.payload else {
        return Err(action_error(
            first.action.sequence,
            "the first action must be the Shot commitment",
        ));
    };
    if first.action.actor != commitment.initial_controller
        || first.signature.public_key != commitment.initial_controller_key
        || first.action.timestamp != commitment.committed_at
    {
        return Err(action_error(
            first.action.sequence,
            "commitment actor, signer, controller, and timestamp must agree",
        ));
    }
    let head = first.action.commitment()?;
    let mut state = ShotState {
        shot_id: first.action.shot_id,
        commitment: commitment.clone(),
        intention: None,
        controller: commitment.initial_controller,
        controller_key: commitment.initial_controller_key.clone(),
        accepted_genome: None,
        expressions: BTreeMap::new(),
        genome_proposals: BTreeMap::new(),
        genome_acceptances: BTreeMap::new(),
        verification_results: BTreeMap::new(),
        feedback: BTreeMap::new(),
        evolutionary_intents: BTreeMap::new(),
        evolutions: Vec::new(),
        ownership_history: Vec::new(),
        token_association: None,
        token_history: Vec::new(),
        availability: BTreeMap::new(),
        parent_relations: Vec::new(),
        sequence: 1,
        head,
        last_timestamp: first.action.timestamp.clone(),
    };
    apply_lineage_actions(&mut state, &actions[1..])?;
    Ok(state)
}

/// Continues reduction from a previously derived trusted state. This is the
/// authority-aware path for a node that holds only a new partial segment.
pub fn apply_lineage_actions(state: &mut ShotState, actions: &[SignedLineageAction]) -> Result<()> {
    if actions.is_empty() {
        return Ok(());
    }
    let anchor = LineageSegmentAnchor {
        shot_id: state.shot_id,
        sequence: state.sequence,
        head: state.head,
        timestamp: state.last_timestamp.clone(),
    };
    verify_lineage_segment(actions, Some(&anchor))?;
    let mut candidate = state.clone();
    for signed in actions {
        apply_one(&mut candidate, signed)?;
    }
    *state = candidate;
    Ok(())
}

fn apply_one(state: &mut ShotState, signed: &SignedLineageAction) -> Result<()> {
    let action = &signed.action;
    let action_commitment = action.commitment()?;
    let fail = |reason: &str| action_error(action.sequence, reason);
    if action.actor != state.controller || signed.signature.public_key != state.controller_key {
        return Err(fail(
            "action is not signed by the current Shot controller key",
        ));
    }

    match &action.payload {
        LineagePayload::Commitment(_) => {
            return Err(fail("a Shot may contain only one commitment action"))
        }
        LineagePayload::Intention(record) => {
            if state.intention.is_some() {
                return Err(fail("the original intention is immutable"));
            }
            if record.commitment()? != state.commitment.intention_commitment {
                return Err(fail(
                    "intention does not match the commitment made at Shot origin",
                ));
            }
            if record.captured_at.unix_timestamp() > action.timestamp.unix_timestamp() {
                return Err(fail("intention capture cannot occur after its action"));
            }
            state.intention = Some(record.clone());
        }
        LineagePayload::GenomeProposal(record) => {
            match (
                &state.accepted_genome,
                record.base_revision,
                record.base_genome_digest,
            ) {
                (None, None, None) => {}
                (Some(current), Some(revision), Some(digest))
                    if revision == current.genome.revision
                        && digest == current.genome.digest()? => {}
                _ => {
                    return Err(fail(
                        "genome proposal base does not match the currently accepted genome",
                    ))
                }
            }
            state
                .genome_proposals
                .insert(action_commitment, record.clone());
        }
        LineagePayload::GenomeAcceptance(record) => {
            if record.accepted_at != action.timestamp {
                return Err(fail(
                    "genome acceptance timestamp must equal action timestamp",
                ));
            }
            let proposal = state
                .genome_proposals
                .get(&record.proposal_action)
                .ok_or_else(|| fail("genome proposal action is unavailable"))?;
            if proposal.proposed.revision != record.revision
                || proposal.proposed.digest()? != record.genome_digest
            {
                return Err(fail(
                    "accepted revision or digest does not match the proposal",
                ));
            }
            let expected_revision = state
                .accepted_genome
                .as_ref()
                .map(|current| current.genome.revision.saturating_add(1))
                .unwrap_or(1);
            if record.revision != expected_revision {
                return Err(fail("accepted genome revision is not contiguous"));
            }
            state.accepted_genome = Some(AcceptedGenome {
                genome: proposal.proposed.clone(),
                proposal_action: record.proposal_action,
                acceptance_action: action_commitment,
            });
            state
                .genome_acceptances
                .insert(action_commitment, record.clone());
        }
        LineagePayload::Expression(record) => {
            let genome = state
                .accepted_genome
                .as_ref()
                .ok_or_else(|| fail("an expression requires an accepted genome"))?;
            if record.genome_revision != genome.genome.revision
                || record.genome_digest != genome.genome.digest()?
            {
                return Err(fail("expression does not bind the accepted genome"));
            }
            if state.expressions.contains_key(&record.expression_id) {
                return Err(fail("expression ID already exists"));
            }
            state.expressions.insert(
                record.expression_id,
                ExpressionState {
                    expression: record.clone(),
                    organs: BTreeMap::new(),
                    versions: Vec::new(),
                    current_version: None,
                },
            );
        }
        LineagePayload::Organ(record) => {
            let expression = state
                .expressions
                .get_mut(&record.expression_id)
                .ok_or_else(|| fail("organ names an unknown expression"))?;
            if expression.organs.contains_key(&record.organ_id) {
                return Err(fail("organ ID is already declared for this expression"));
            }
            for dependency in &record.dependencies {
                if dependency == &record.organ_id {
                    return Err(fail("organ cannot depend on itself"));
                }
                if !expression.organs.contains_key(dependency) {
                    return Err(fail("organ dependency has not been declared"));
                }
            }
            expression
                .organs
                .insert(record.organ_id.clone(), record.clone());
        }
        LineagePayload::VerificationResult(record) => {
            let genome = state
                .accepted_genome
                .as_ref()
                .ok_or_else(|| fail("verification requires an accepted genome"))?;
            if !state.expressions.contains_key(&record.expression_id) {
                return Err(fail("verification names an unknown expression"));
            }
            if record.genome_revision != genome.genome.revision
                || record.genome_digest != genome.genome.digest()?
            {
                return Err(fail("verification does not bind the accepted genome"));
            }
            let expression = state
                .expressions
                .get(&record.expression_id)
                .expect("checked above");
            let capability_graph = expression.organs.values().cloned().collect::<Vec<_>>();
            let expected_capability_graph_digest = capability_graph_digest(&capability_graph)?;
            if record.capability_graph_digest != expected_capability_graph_digest {
                return Err(fail(
                    "verification does not bind the exact current Organ graph",
                ));
            }
            let gate_names = record
                .gates
                .iter()
                .map(|gate| gate.name.as_str())
                .collect::<BTreeSet<_>>();
            for organ in &capability_graph {
                for index in 0..organ.acceptance_tests.len() {
                    let expected = organ_acceptance_gate_name(organ, index)?;
                    if !gate_names.contains(expected.as_str()) {
                        return Err(fail(
                            "verification omits a declared Organ acceptance test gate",
                        ));
                    }
                }
            }
            let ordinal = u64::try_from(expression.versions.len())
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| fail("expression version sequence overflowed"))?;
            let expected = VersionId::derive(
                state.shot_id,
                record.expression_id,
                ordinal,
                record.genome_digest,
                record.source_digest,
            );
            if record.candidate_version_id != expected {
                return Err(fail(
                    "verification candidate does not use the next content-bound VersionID",
                ));
            }
            state
                .verification_results
                .insert(action_commitment, record.clone());
        }
        LineagePayload::Version(record) => {
            let verification = state
                .verification_results
                .get(&record.verification_action)
                .ok_or_else(|| fail("accepted version references an unknown verification"))?;
            if !verification.passed {
                return Err(fail(
                    "failed verification cannot produce an accepted version",
                ));
            }
            if verification.expression_id != record.expression_id
                || verification.candidate_version_id != record.version_id
                || verification.genome_revision != record.genome_revision
                || verification.genome_digest != record.genome_digest
                || verification.source_digest != record.source_digest
                || verification.capability_graph_digest != record.capability_graph_digest
                || verification.known_incompleteness != record.known_incompleteness
            {
                return Err(fail(
                    "version facts do not exactly match the referenced verification",
                ));
            }
            let genome = state
                .accepted_genome
                .as_ref()
                .ok_or_else(|| fail("version requires an accepted genome"))?;
            if genome.genome.revision != record.genome_revision
                || genome.genome.digest()? != record.genome_digest
            {
                return Err(fail("version does not bind the accepted genome"));
            }
            if state
                .expressions
                .values()
                .flat_map(|value| &value.versions)
                .any(|value| value.version_id == record.version_id)
            {
                return Err(fail("VersionID already exists"));
            }
            let expression = state
                .expressions
                .get_mut(&record.expression_id)
                .ok_or_else(|| fail("version names an unknown expression"))?;
            let capability_graph = expression.organs.values().cloned().collect::<Vec<_>>();
            if record.capability_graph_digest != capability_graph_digest(&capability_graph)? {
                return Err(fail("version does not bind the exact current Organ graph"));
            }
            let expected_ordinal = u64::try_from(expression.versions.len())
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| fail("expression version sequence overflowed"))?;
            if record.ordinal != expected_ordinal {
                return Err(fail("expression version ordinal is not contiguous"));
            }
            expression.versions.push(record.clone());
            expression.current_version = Some(record.version_id);
        }
        LineagePayload::Feedback(record) => {
            let expression = state
                .expressions
                .get(&record.expression_id)
                .ok_or_else(|| fail("feedback names an unknown expression"))?;
            let version = expression
                .versions
                .iter()
                .find(|version| version.version_id == record.version_id)
                .ok_or_else(|| fail("feedback names an unknown exact version"))?;
            if record
                .build_identity
                .as_ref()
                .is_some_and(|value| version.build_identity.as_ref() != Some(value))
            {
                return Err(fail(
                    "feedback build identity does not match the exact version",
                ));
            }
            state.feedback.insert(action_commitment, record.clone());
        }
        LineagePayload::EvolutionaryIntent(record) => {
            let expression = state
                .expressions
                .get(&record.expression_id)
                .ok_or_else(|| fail("evolutionary intent names an unknown expression"))?;
            if expression.current_version != Some(record.from_version_id) {
                return Err(fail(
                    "evolutionary intent must begin from the current expression version",
                ));
            }
            for reference in &record.feedback_actions {
                let feedback = state
                    .feedback
                    .get(reference)
                    .ok_or_else(|| fail("evolutionary intent references unknown feedback"))?;
                if feedback.expression_id != record.expression_id
                    || feedback.version_id != record.from_version_id
                {
                    return Err(fail(
                        "selected feedback must bind the same exact expression version",
                    ));
                }
            }
            if let Some(proposal) = record.proposed_genome_action {
                if !state.genome_proposals.contains_key(&proposal) {
                    return Err(fail("proposed genome action is unavailable"));
                }
            }
            state
                .evolutionary_intents
                .insert(action_commitment, record.clone());
        }
        LineagePayload::Evolution(record) => {
            if record.completed_at != action.timestamp {
                return Err(fail(
                    "evolution completion timestamp must equal action timestamp",
                ));
            }
            let intent = state
                .evolutionary_intents
                .get(&record.evolutionary_intent_action)
                .ok_or_else(|| fail("evolution references unknown evolutionary intent"))?;
            if intent.expression_id != record.expression_id
                || intent.from_version_id != record.from_version_id
            {
                return Err(fail("evolution does not match its accepted intent"));
            }
            let expression = state
                .expressions
                .get(&record.expression_id)
                .ok_or_else(|| fail("evolution names an unknown expression"))?;
            let from_index = expression
                .versions
                .iter()
                .position(|version| version.version_id == record.from_version_id)
                .ok_or_else(|| fail("evolution source version is unknown"))?;
            let to_index = expression
                .versions
                .iter()
                .position(|version| version.version_id == record.to_version_id)
                .ok_or_else(|| fail("evolution target version is not accepted"))?;
            if to_index != from_index.saturating_add(1)
                || expression.current_version != Some(record.to_version_id)
            {
                return Err(fail(
                    "evolution must connect adjacent accepted versions and end at the current version",
                ));
            }
            let from = &expression.versions[from_index];
            let to = &expression.versions[to_index];
            if from.genome_digest != record.from_genome_digest
                || to.genome_digest != record.to_genome_digest
            {
                return Err(fail("evolution genome facts do not match its versions"));
            }
            let organ_graph_changed = from.capability_graph_digest != to.capability_graph_digest;
            let organ_change_declared = intent
                .desired_changes
                .iter()
                .any(|change| change.scope == crate::ontology::ChangeScope::Organ);
            if organ_graph_changed && !organ_change_declared {
                return Err(fail(
                    "Organ graph transition was not declared by its evolutionary intent",
                ));
            }
            if !organ_graph_changed && organ_change_declared {
                return Err(fail(
                    "Organ-scoped evolutionary intent cannot complete without an Organ graph transition",
                ));
            }
            if record.from_genome_digest != record.to_genome_digest {
                let proposal_action = intent.proposed_genome_action.ok_or_else(|| {
                    fail("genome mutation was not proposed by its evolutionary intent")
                })?;
                let acceptance_action = record.genome_acceptance_action.ok_or_else(|| {
                    fail("genome mutation lacks the exact accepted genome action")
                })?;
                let acceptance = state
                    .genome_acceptances
                    .get(&acceptance_action)
                    .ok_or_else(|| fail("genome acceptance action is unavailable"))?;
                if acceptance.proposal_action != proposal_action {
                    return Err(fail(
                        "genome acceptance does not accept the intent's exact proposal",
                    ));
                }
                if acceptance.genome_digest != record.to_genome_digest
                    || acceptance.revision != to.genome_revision
                {
                    return Err(fail(
                        "genome acceptance does not bind the evolution target version",
                    ));
                }
            } else if intent.proposed_genome_action.is_some()
                || intent
                    .desired_changes
                    .iter()
                    .any(|change| change.scope == crate::ontology::ChangeScope::Genome)
            {
                return Err(fail(
                    "genome-scoped evolutionary intent cannot complete without its genome mutation",
                ));
            } else if record.genome_acceptance_action.is_some() {
                return Err(fail(
                    "unchanged genome must not name a genome acceptance action",
                ));
            }
            state.evolutions.push(ActionRecord {
                action: action_commitment,
                record: record.clone(),
            });
        }
        LineagePayload::Ownership(record) => {
            if record.effective_at != action.timestamp {
                return Err(fail("ownership timestamp must equal action timestamp"));
            }
            if record.previous_controller != state.controller {
                return Err(fail("ownership action names the wrong current controller"));
            }
            state.ownership_history.push(ActionRecord {
                action: action_commitment,
                record: record.clone(),
            });
            state.controller = record.new_controller;
            state.controller_key = record.new_controller_key.clone();
        }
        LineagePayload::TokenAssociation(record) => {
            match record.operation {
                TokenAssociationOperation::Associate => {
                    // v1 ShotRelations semantics: a fresh authorized action
                    // replaces the one current association, including with an
                    // identical or conflicting value.
                    state.token_association = Some(record.clone());
                }
                TokenAssociationOperation::Remove => {
                    let Some(current) = &state.token_association else {
                        return Err(fail("cannot remove a missing token association"));
                    };
                    if current.chain_id != record.chain_id || current.token != record.token {
                        return Err(fail(
                            "token removal must exactly match the current association",
                        ));
                    }
                    state.token_association = None;
                }
            }
            state.token_history.push(ActionRecord {
                action: action_commitment,
                record: record.clone(),
            });
        }
        LineagePayload::ArtifactAvailability(record) => {
            state
                .availability
                .insert(record.availability.artifact.digest, record.clone());
        }
        LineagePayload::ParentRelation(record) => {
            if record.child_shot_id != state.shot_id {
                return Err(fail("parent relation child is not this Shot"));
            }
            if state
                .parent_relations
                .iter()
                .any(|existing| existing.record.parent_shot_id == record.parent_shot_id)
            {
                return Err(fail("parent relationship is already recorded"));
            }
            state.parent_relations.push(ActionRecord {
                action: action_commitment,
                record: record.clone(),
            });
        }
    }

    state.sequence = action.sequence;
    state.head = action_commitment;
    state.last_timestamp = action.timestamp.clone();
    Ok(())
}

fn action_error(sequence: u64, reason: impl Into<String>) -> ProtocolError {
    ProtocolError::LineageAction {
        sequence,
        reason: reason.into(),
    }
}

/// One exact v1 record and sidecar retained without rewriting or re-signing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyV1Entry {
    pub record: ShotRecord,
    pub signature: SignatureSidecar,
    pub record_commitment: Bytes32,
    pub version_id: VersionId,
}

/// Honest neutral projection of a frozen signed `tohseno.shot/1` chain.
///
/// `intention_commitment` is populated only when every historical record
/// carries the same v1 genesis digest. Genome remains explicitly unknown:
/// Fascia and source hashes are not retroactively called a Shot genome.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptedV1Lineage {
    pub shot_id: ShotId,
    pub controller: BuilderId,
    pub expression_id: ExpressionId,
    pub intention_commitment: Option<Bytes32>,
    pub intention_availability: AvailabilityStatus,
    pub genome_availability: AvailabilityStatus,
    pub entries: Vec<LegacyV1Entry>,
    pub head: Bytes32,
}

pub fn adapt_v1_lineage(entries: &[(&ShotRecord, &SignatureSidecar)]) -> Result<AdaptedV1Lineage> {
    let verified = verify_lineage(entries)?;
    let first = entries
        .first()
        .expect("verify_lineage rejects an empty lineage")
        .0;
    let expression_id = ExpressionId::for_legacy_v1(first.shot_id, &first.bundle_id);
    let intention_digests = entries
        .iter()
        .map(|(record, _)| record.genesis_input_sha256)
        .collect::<BTreeSet<_>>();
    let intention_commitment = if intention_digests.len() == 1 {
        intention_digests.first().copied()
    } else {
        None
    };
    let mut projected = Vec::with_capacity(entries.len());
    for (record, signature) in entries {
        let record_commitment = record.commitment()?;
        projected.push(LegacyV1Entry {
            record: (*record).clone(),
            signature: (*signature).clone(),
            record_commitment,
            version_id: VersionId::for_legacy_v1(
                record.shot_id,
                expression_id,
                record.sequence,
                record_commitment,
            ),
        });
    }
    let head = verified
        .head()
        .ok_or_else(|| action_error(0, "verified v1 lineage has no head"))?
        .commitment;
    Ok(AdaptedV1Lineage {
        shot_id: verified.shot_id,
        controller: verified.builder_id,
        expression_id,
        intention_commitment,
        intention_availability: AvailabilityStatus::Unknown,
        genome_availability: AvailabilityStatus::Unknown,
        entries: projected,
        head,
    })
}
