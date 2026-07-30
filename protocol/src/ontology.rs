//! Neutral, versioned protocol objects for coherent intention lineage.
//!
//! These records describe a Shot independently from any one repository,
//! expression medium, node, chain, or token. They are payloads of signed
//! [`crate::lineage::LineageAction`] values; mutable local files are derived
//! views rather than a second source of truth.

use crate::builder::MAX_SAFE_JSON_INTEGER;
use crate::canonical;
use crate::digest::{Address20, Bytes32, ExpressionId, ShotId, VersionId};
use crate::identity::BuilderId;
use crate::record::CanonicalTimestamp;
use crate::signature::{P256PublicKey, SignatureSidecar};
use crate::text::{invalid, validate_token};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const INTENTION_SCHEMA: &str = "tohseno.intention/2";
pub const SHOT_COMMITMENT_SCHEMA: &str = "tohseno.shot-commitment/2";
pub const GENOME_SCHEMA: &str = "tohseno.genome/2";
pub const GENOME_PROPOSAL_SCHEMA: &str = "tohseno.genome-proposal/2";
pub const GENOME_ACCEPTANCE_SCHEMA: &str = "tohseno.genome-acceptance/2";
pub const EXPRESSION_SCHEMA: &str = "tohseno.expression/2";
pub const ORGAN_SCHEMA: &str = "tohseno.organ/2";
pub const VERSION_SCHEMA: &str = "tohseno.version/2";
pub const FEEDBACK_SCHEMA: &str = "tohseno.feedback/2";
pub const EVOLUTIONARY_INTENT_SCHEMA: &str = "tohseno.evolutionary-intent/2";
pub const EVOLUTION_SCHEMA: &str = "tohseno.evolution/2";
pub const OWNERSHIP_SCHEMA: &str = "tohseno.ownership/2";
pub const TOKEN_ASSOCIATION_SCHEMA: &str = "tohseno.token-association/2";
pub const VERIFICATION_RESULT_SCHEMA: &str = "tohseno.verification-result/2";
pub const ARTIFACT_AVAILABILITY_SCHEMA: &str = "tohseno.artifact-availability/2";
pub const PARENT_RELATION_SCHEMA: &str = "tohseno.parent-relation/2";

fn require_schema(field: &'static str, observed: &str, expected: &'static str) -> Result<()> {
    if observed != expected {
        return Err(invalid(field, format!("must be {expected}")));
    }
    Ok(())
}

fn require_nonzero(field: &'static str, digest: Bytes32) -> Result<()> {
    if digest == Bytes32::ZERO {
        return Err(invalid(field, "must not be zero"));
    }
    Ok(())
}

fn validate_tokens(
    field: &'static str,
    values: &[String],
    minimum: usize,
    maximum: usize,
    item_maximum: usize,
) -> Result<()> {
    if values.len() < minimum || values.len() > maximum {
        return Err(invalid(
            field,
            format!("must contain {minimum}..={maximum} entries"),
        ));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_token(field, value, 1, item_maximum)?;
        if !unique.insert(value) {
            return Err(invalid(field, "must not contain duplicates"));
        }
    }
    Ok(())
}

fn validate_safe_positive(field: &'static str, value: u64) -> Result<()> {
    if value == 0 || value > MAX_SAFE_JSON_INTEGER {
        return Err(invalid(
            field,
            format!("must be in 1..={MAX_SAFE_JSON_INTEGER}"),
        ));
    }
    Ok(())
}

/// What a node or local repository honestly knows about an artifact.
///
/// States are intentionally not ordered: for example, a private artifact is
/// not a weaker public artifact, and an on-chain anchor does not imply the
/// artifact bytes are available.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityStatus {
    Absent,
    Unknown,
    IntentionallyPrivate,
    LocallyAvailable,
    PubliclyAvailable,
    Replicated,
    CryptographicallyVerified,
    OnChainAnchored,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactLocationKind {
    LocalPath,
    Https,
    ContentAddress,
    Node,
    Chain,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactLocation {
    pub kind: ArtifactLocationKind,
    pub value: String,
}

impl ArtifactLocation {
    pub fn validate(&self) -> Result<()> {
        validate_token("artifact.location.value", &self.value, 1, 4096)?;
        if self.kind == ArtifactLocationKind::Https && !self.value.starts_with("https://") {
            return Err(invalid(
                "artifact.location.value",
                "an https location must begin with https://",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDescriptor {
    pub digest: Bytes32,
    pub media_type: String,
    pub byte_length: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ArtifactDescriptor {
    pub fn validate(&self) -> Result<()> {
        require_nonzero("artifact.digest", self.digest)?;
        validate_token("artifact.media_type", &self.media_type, 1, 255)?;
        if self.byte_length > MAX_SAFE_JSON_INTEGER {
            return Err(invalid(
                "artifact.byte_length",
                "must be a JSON-safe unsigned integer",
            ));
        }
        if let Some(name) = &self.name {
            validate_token("artifact.name", name, 1, 255)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactAvailability {
    pub schema: String,
    pub artifact: ArtifactDescriptor,
    pub status: AvailabilityStatus,
    pub locations: Vec<ArtifactLocation>,
}

impl ArtifactAvailability {
    pub fn new(artifact: ArtifactDescriptor, status: AvailabilityStatus) -> Self {
        Self {
            schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
            artifact,
            status,
            locations: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        require_schema(
            "artifact_availability.schema",
            &self.schema,
            ARTIFACT_AVAILABILITY_SCHEMA,
        )?;
        self.artifact.validate()?;
        if self.locations.len() > 32 {
            return Err(invalid(
                "artifact_availability.locations",
                "must contain at most 32 locations",
            ));
        }
        let mut unique = BTreeSet::new();
        for location in &self.locations {
            location.validate()?;
            if !unique.insert((location.kind, &location.value)) {
                return Err(invalid(
                    "artifact_availability.locations",
                    "must not contain duplicate locations",
                ));
            }
        }
        if matches!(
            self.status,
            AvailabilityStatus::Absent | AvailabilityStatus::Unknown
        ) && !self.locations.is_empty()
        {
            return Err(invalid(
                "artifact_availability.locations",
                "absent or unknown bytes cannot claim a location",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginalMaterial {
    pub artifact: ArtifactAvailability,
    /// Exact UTF-8 source, when the owner elects to place it in this record.
    /// Its UTF-8 bytes must match `artifact.artifact.digest` and byte length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_text: Option<String>,
}

impl OriginalMaterial {
    pub fn validate(&self) -> Result<()> {
        self.artifact.validate()?;
        if let Some(text) = &self.inline_text {
            let bytes = text.as_bytes();
            if crate::digest::sha256(bytes) != self.artifact.artifact.digest
                || u64::try_from(bytes.len()).ok() != Some(self.artifact.artifact.byte_length)
            {
                return Err(invalid(
                    "intention.material.inline_text",
                    "exact UTF-8 bytes must match the artifact digest and length",
                ));
            }
            if matches!(
                self.artifact.status,
                AvailabilityStatus::Absent | AvailabilityStatus::Unknown
            ) {
                return Err(invalid(
                    "intention.material.availability",
                    "inline bytes cannot be absent or unknown",
                ));
            }
        }
        Ok(())
    }
}

/// The preserved human declaration, not a cleaned-up product summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentionRecord {
    pub schema: String,
    pub materials: Vec<OriginalMaterial>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_note: Option<String>,
    pub captured_at: CanonicalTimestamp,
}

impl IntentionRecord {
    pub fn new(materials: Vec<OriginalMaterial>, captured_at: CanonicalTimestamp) -> Self {
        Self {
            schema: INTENTION_SCHEMA.into(),
            materials,
            owner_note: None,
            captured_at,
        }
    }

    pub fn validate(&self) -> Result<()> {
        require_schema("intention.schema", &self.schema, INTENTION_SCHEMA)?;
        if self.materials.is_empty() || self.materials.len() > 64 {
            return Err(invalid(
                "intention.materials",
                "must contain 1..=64 original materials",
            ));
        }
        let mut digests = BTreeSet::new();
        for material in &self.materials {
            material.validate()?;
            if !digests.insert(material.artifact.artifact.digest) {
                return Err(invalid(
                    "intention.materials",
                    "must not repeat an artifact digest",
                ));
            }
        }
        if let Some(note) = &self.owner_note {
            validate_token("intention.owner_note", note, 1, 4000)?;
        }
        Ok(())
    }

    pub fn commitment(&self) -> Result<Bytes32> {
        self.validate()?;
        canonical::sha256_commitment(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommitmentOrigin {
    Native,
    LegacyV1 {
        root_record_commitment: Bytes32,
        head_record_commitment: Bytes32,
        root_sequence: u32,
    },
    Descendant {
        parent_shot_id: ShotId,
        parent_head: Bytes32,
    },
}

/// The explicit attributable act that begins a Shot.
///
/// Pure reduction can prove that later actions use this declared key. Candidate
/// factory and node policy must additionally reproduce `initial_controller`
/// from the pinned BuilderAccount factory, salt, creation bytecode, and this
/// key before treating a new commitment as production-authorized.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotCommitment {
    pub schema: String,
    pub intention_commitment: Bytes32,
    pub initial_controller: BuilderId,
    pub initial_controller_key: P256PublicKey,
    pub origin: CommitmentOrigin,
    pub committed_at: CanonicalTimestamp,
}

impl ShotCommitment {
    pub fn new(
        intention_commitment: Bytes32,
        initial_controller: BuilderId,
        initial_controller_key: P256PublicKey,
        committed_at: CanonicalTimestamp,
    ) -> Self {
        Self {
            schema: SHOT_COMMITMENT_SCHEMA.into(),
            intention_commitment,
            initial_controller,
            initial_controller_key,
            origin: CommitmentOrigin::Native,
            committed_at,
        }
    }

    pub fn validate(&self) -> Result<()> {
        require_schema(
            "shot_commitment.schema",
            &self.schema,
            SHOT_COMMITMENT_SCHEMA,
        )?;
        require_nonzero(
            "shot_commitment.intention_commitment",
            self.intention_commitment,
        )?;
        self.initial_controller.validate()?;
        self.initial_controller_key.validate()?;
        match self.origin {
            CommitmentOrigin::Native => {}
            CommitmentOrigin::LegacyV1 {
                root_record_commitment,
                head_record_commitment,
                root_sequence,
            } => {
                require_nonzero(
                    "shot_commitment.origin.root_record_commitment",
                    root_record_commitment,
                )?;
                require_nonzero(
                    "shot_commitment.origin.head_record_commitment",
                    head_record_commitment,
                )?;
                if root_sequence == 0 {
                    return Err(invalid(
                        "shot_commitment.origin.root_sequence",
                        "must be positive",
                    ));
                }
            }
            CommitmentOrigin::Descendant {
                parent_shot_id,
                parent_head,
            } => {
                if parent_shot_id.is_zero() {
                    return Err(invalid(
                        "shot_commitment.origin.parent_shot_id",
                        "must not be zero",
                    ));
                }
                require_nonzero("shot_commitment.origin.parent_head", parent_head)?;
            }
        }
        Ok(())
    }
}

/// Current accepted operational interpretation of what must remain true.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Genome {
    pub schema: String,
    pub revision: u64,
    pub purpose: String,
    pub intended_for: Vec<String>,
    pub essential_experience: Vec<String>,
    pub behavioral_invariants: Vec<String>,
    pub interaction_laws: Vec<String>,
    pub aesthetic_principles: Vec<String>,
    pub privacy_principles: Vec<String>,
    pub ownership_principles: Vec<String>,
    pub platform_commitments: Vec<String>,
    pub boundaries: Vec<String>,
    pub non_goals: Vec<String>,
    pub required_capabilities: Vec<String>,
    pub forbidden_transformations: Vec<String>,
    pub acceptance_principles: Vec<String>,
    pub freely_changeable: Vec<String>,
}

impl Genome {
    pub fn validate(&self) -> Result<()> {
        require_schema("genome.schema", &self.schema, GENOME_SCHEMA)?;
        validate_safe_positive("genome.revision", self.revision)?;
        validate_token("genome.purpose", &self.purpose, 1, 4000)?;
        validate_tokens("genome.intended_for", &self.intended_for, 1, 64, 1000)?;
        validate_tokens(
            "genome.essential_experience",
            &self.essential_experience,
            1,
            64,
            2000,
        )?;
        validate_tokens(
            "genome.behavioral_invariants",
            &self.behavioral_invariants,
            1,
            128,
            2000,
        )?;
        validate_tokens(
            "genome.interaction_laws",
            &self.interaction_laws,
            0,
            128,
            2000,
        )?;
        validate_tokens(
            "genome.aesthetic_principles",
            &self.aesthetic_principles,
            0,
            128,
            2000,
        )?;
        validate_tokens(
            "genome.privacy_principles",
            &self.privacy_principles,
            1,
            128,
            2000,
        )?;
        validate_tokens(
            "genome.ownership_principles",
            &self.ownership_principles,
            1,
            128,
            2000,
        )?;
        validate_tokens(
            "genome.platform_commitments",
            &self.platform_commitments,
            0,
            64,
            1000,
        )?;
        validate_tokens("genome.boundaries", &self.boundaries, 0, 128, 2000)?;
        validate_tokens("genome.non_goals", &self.non_goals, 0, 128, 2000)?;
        validate_tokens(
            "genome.required_capabilities",
            &self.required_capabilities,
            0,
            128,
            500,
        )?;
        validate_tokens(
            "genome.forbidden_transformations",
            &self.forbidden_transformations,
            1,
            128,
            2000,
        )?;
        validate_tokens(
            "genome.acceptance_principles",
            &self.acceptance_principles,
            1,
            128,
            2000,
        )?;
        validate_tokens(
            "genome.freely_changeable",
            &self.freely_changeable,
            0,
            128,
            2000,
        )
    }

    pub fn digest(&self) -> Result<Bytes32> {
        self.validate()?;
        canonical::sha256_commitment(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenomeProposal {
    pub schema: String,
    pub base_revision: Option<u64>,
    pub base_genome_digest: Option<Bytes32>,
    pub proposed: Genome,
    pub rationale: String,
    pub mutation_summary: Vec<String>,
}

impl GenomeProposal {
    pub fn initial(proposed: Genome, rationale: String) -> Self {
        Self {
            schema: GENOME_PROPOSAL_SCHEMA.into(),
            base_revision: None,
            base_genome_digest: None,
            proposed,
            rationale,
            mutation_summary: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        require_schema(
            "genome_proposal.schema",
            &self.schema,
            GENOME_PROPOSAL_SCHEMA,
        )?;
        self.proposed.validate()?;
        validate_token("genome_proposal.rationale", &self.rationale, 1, 4000)?;
        match (self.base_revision, self.base_genome_digest) {
            (None, None) if self.proposed.revision == 1 && self.mutation_summary.is_empty() => {}
            (Some(base), Some(digest))
                if base > 0
                    && self.proposed.revision == base.saturating_add(1)
                    && digest != Bytes32::ZERO
                    && !self.mutation_summary.is_empty() =>
            {
                validate_tokens(
                    "genome_proposal.mutation_summary",
                    &self.mutation_summary,
                    1,
                    128,
                    2000,
                )?;
            }
            _ => {
                return Err(invalid(
                    "genome_proposal.base_revision",
                    "initial revision 1 has no base or mutations; later revisions require the exact base and an explicit mutation summary",
                ))
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenomeAcceptance {
    pub schema: String,
    pub proposal_action: Bytes32,
    pub revision: u64,
    pub genome_digest: Bytes32,
    pub accepted_at: CanonicalTimestamp,
}

impl GenomeAcceptance {
    pub fn validate(&self) -> Result<()> {
        require_schema(
            "genome_acceptance.schema",
            &self.schema,
            GENOME_ACCEPTANCE_SCHEMA,
        )?;
        require_nonzero("genome_acceptance.proposal_action", self.proposal_action)?;
        validate_safe_positive("genome_acceptance.revision", self.revision)?;
        require_nonzero("genome_acceptance.genome_digest", self.genome_digest)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expression {
    pub schema: String,
    pub expression_id: ExpressionId,
    pub kind: String,
    pub name: String,
    pub platforms: Vec<String>,
    pub genome_revision: u64,
    pub genome_digest: Bytes32,
    pub definition: ArtifactAvailability,
}

impl Expression {
    pub fn validate(&self) -> Result<()> {
        require_schema("expression.schema", &self.schema, EXPRESSION_SCHEMA)?;
        if self.expression_id.is_zero() {
            return Err(invalid("expression.expression_id", "must not be zero"));
        }
        validate_token("expression.kind", &self.kind, 1, 100)?;
        validate_token("expression.name", &self.name, 1, 255)?;
        validate_tokens("expression.platforms", &self.platforms, 1, 32, 100)?;
        validate_safe_positive("expression.genome_revision", self.genome_revision)?;
        require_nonzero("expression.genome_digest", self.genome_digest)?;
        self.definition.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Organ {
    pub schema: String,
    pub expression_id: ExpressionId,
    pub organ_id: String,
    pub provides: Vec<String>,
    pub owns_state: Vec<String>,
    pub permissions: Vec<String>,
    pub dependencies: Vec<String>,
    pub emits: Vec<String>,
    pub consumes: Vec<String>,
    pub satisfies_genome_constraints: Vec<String>,
    pub acceptance_tests: Vec<String>,
    pub platforms: Vec<String>,
}

impl Organ {
    /// Organ declarations are immutable per `(expression_id, organ_id)`.
    /// Capability graph changes are represented by new declarations with new
    /// IDs and by the exact capability lock committed in each Version.
    pub fn validate(&self) -> Result<()> {
        require_schema("organ.schema", &self.schema, ORGAN_SCHEMA)?;
        if self.expression_id.is_zero() {
            return Err(invalid("organ.expression_id", "must not be zero"));
        }
        validate_token("organ.organ_id", &self.organ_id, 1, 128)?;
        validate_tokens("organ.provides", &self.provides, 1, 128, 500)?;
        validate_tokens("organ.owns_state", &self.owns_state, 0, 128, 500)?;
        validate_tokens("organ.permissions", &self.permissions, 0, 128, 500)?;
        validate_tokens("organ.dependencies", &self.dependencies, 0, 128, 128)?;
        validate_tokens("organ.emits", &self.emits, 0, 128, 500)?;
        validate_tokens("organ.consumes", &self.consumes, 0, 128, 500)?;
        validate_tokens(
            "organ.satisfies_genome_constraints",
            &self.satisfies_genome_constraints,
            1,
            128,
            2000,
        )?;
        validate_tokens(
            "organ.acceptance_tests",
            &self.acceptance_tests,
            1,
            256,
            1000,
        )?;
        validate_tokens("organ.platforms", &self.platforms, 1, 32, 100)
    }
}

/// Canonical JSON bytes for one expression's exact declared Organ graph.
///
/// Organ actions are causally ordered, but their graph commitment is ordered by
/// immutable `organ_id` so independent implementations reproduce the same
/// digest from the same declarations.
pub fn canonical_capability_graph_bytes(organs: &[Organ]) -> Result<Vec<u8>> {
    let mut graph = organs.to_vec();
    for organ in &graph {
        organ.validate()?;
    }
    graph.sort_by(|left, right| left.organ_id.as_bytes().cmp(right.organ_id.as_bytes()));

    if let Some(first) = graph.first() {
        let expression_id = first.expression_id;
        let mut organ_ids = BTreeSet::new();
        for organ in &graph {
            if organ.expression_id != expression_id {
                return Err(invalid(
                    "capability_graph.expression_id",
                    "all organs must belong to one expression",
                ));
            }
            if !organ_ids.insert(organ.organ_id.as_str()) {
                return Err(invalid(
                    "capability_graph.organ_id",
                    "must not contain duplicate organ IDs",
                ));
            }
        }
        for organ in &graph {
            if organ
                .dependencies
                .iter()
                .any(|dependency| !organ_ids.contains(dependency.as_str()))
            {
                return Err(invalid(
                    "capability_graph.dependencies",
                    "every dependency must name an organ in the same graph",
                ));
            }
        }
    }

    canonical::to_vec(&graph)
}

/// SHA-256 commitment to [`canonical_capability_graph_bytes`].
pub fn capability_graph_digest(organs: &[Organ]) -> Result<Bytes32> {
    Ok(crate::digest::sha256(&canonical_capability_graph_bytes(
        organs,
    )?))
}

/// Stable verification-gate name for one declared Organ acceptance test.
///
/// The full test digest prevents a changed sentence from inheriting a prior
/// gate result while the readable Organ ID and ordinal keep reports inspectable.
pub fn organ_acceptance_gate_name(organ: &Organ, index: usize) -> Result<String> {
    organ.validate()?;
    let test = organ.acceptance_tests.get(index).ok_or_else(|| {
        invalid(
            "organ.acceptance_tests",
            "gate index must name a declared acceptance test",
        )
    })?;
    let ordinal = index.checked_add(1).ok_or_else(|| {
        invalid(
            "organ.acceptance_tests",
            "gate ordinal overflowed the implementation range",
        )
    })?;
    Ok(format!(
        "organ.{}.acceptance.{ordinal}.{}",
        organ.organ_id,
        crate::digest::sha256(test.as_bytes())
    ))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializationProvenance {
    pub factory: String,
    pub factory_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factory_source_commit: Option<String>,
    pub template_digest: Bytes32,
    pub input_action: Bytes32,
    pub deterministic: bool,
}

impl MaterializationProvenance {
    pub fn validate(&self) -> Result<()> {
        validate_token("version.provenance.factory", &self.factory, 1, 255)?;
        validate_token(
            "version.provenance.factory_version",
            &self.factory_version,
            1,
            64,
        )?;
        if let Some(commit) = &self.factory_source_commit {
            if commit.len() != 40
                || !commit
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(invalid(
                    "version.provenance.factory_source_commit",
                    "must be a 40-character lowercase Git object ID",
                ));
            }
        }
        require_nonzero("version.provenance.template_digest", self.template_digest)?;
        require_nonzero("version.provenance.input_action", self.input_action)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionRecord {
    pub schema: String,
    pub version_id: VersionId,
    pub expression_id: ExpressionId,
    pub ordinal: u64,
    pub genome_revision: u64,
    pub genome_digest: Bytes32,
    pub source_digest: Bytes32,
    pub provenance: MaterializationProvenance,
    pub capability_graph_digest: Bytes32,
    pub verification_action: Bytes32,
    pub known_incompleteness: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_digest: Option<Bytes32>,
    pub accepted_at: CanonicalTimestamp,
}

impl VersionRecord {
    pub fn expected_id(&self, shot_id: ShotId) -> VersionId {
        VersionId::derive(
            shot_id,
            self.expression_id,
            self.ordinal,
            self.genome_digest,
            self.source_digest,
        )
    }

    pub fn validate(&self, shot_id: ShotId) -> Result<()> {
        require_schema("version.schema", &self.schema, VERSION_SCHEMA)?;
        if self.expression_id.is_zero() || self.version_id.is_zero() {
            return Err(invalid(
                "version.identity",
                "expression_id and version_id must not be zero",
            ));
        }
        validate_safe_positive("version.ordinal", self.ordinal)?;
        validate_safe_positive("version.genome_revision", self.genome_revision)?;
        require_nonzero("version.genome_digest", self.genome_digest)?;
        require_nonzero("version.source_digest", self.source_digest)?;
        require_nonzero(
            "version.capability_graph_digest",
            self.capability_graph_digest,
        )?;
        require_nonzero("version.verification_action", self.verification_action)?;
        self.provenance.validate()?;
        validate_tokens(
            "version.known_incompleteness",
            &self.known_incompleteness,
            0,
            128,
            2000,
        )?;
        if let Some(identity) = &self.build_identity {
            validate_token("version.build_identity", identity, 1, 500)?;
        }
        if self.build_digest == Some(Bytes32::ZERO) {
            return Err(invalid("version.build_digest", "must not be zero"));
        }
        if self.version_id != self.expected_id(shot_id) {
            return Err(invalid(
                "version.version_id",
                "must use the protocol content-bound derivation",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Private,
    Public,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackAuthor {
    pub identity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

impl FeedbackAuthor {
    pub fn validate(&self) -> Result<()> {
        validate_token("feedback.author.identity", &self.identity, 1, 500)?;
        if let Some(name) = &self.display_name {
            validate_token("feedback.author.display_name", name, 1, 255)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructuredObservation {
    pub kind: String,
    pub subject: String,
    pub value: String,
}

impl StructuredObservation {
    pub fn validate(&self) -> Result<()> {
        validate_token("feedback.observation.kind", &self.kind, 1, 100)?;
        validate_token("feedback.observation.subject", &self.subject, 1, 500)?;
        validate_token("feedback.observation.value", &self.value, 1, 4000)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feedback {
    pub schema: String,
    pub expression_id: ExpressionId,
    pub version_id: VersionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<FeedbackAuthor>,
    pub visibility: Visibility,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub observations: Vec<StructuredObservation>,
    pub attachments: Vec<ArtifactAvailability>,
    pub observed_at: CanonicalTimestamp,
}

impl Feedback {
    pub fn validate(&self) -> Result<()> {
        require_schema("feedback.schema", &self.schema, FEEDBACK_SCHEMA)?;
        if self.expression_id.is_zero() || self.version_id.is_zero() {
            return Err(invalid(
                "feedback.identity",
                "expression_id and version_id must not be zero",
            ));
        }
        if let Some(build) = &self.build_identity {
            validate_token("feedback.build_identity", build, 1, 500)?;
        }
        if let Some(author) = &self.author {
            author.validate()?;
        }
        if let Some(text) = &self.text {
            validate_token("feedback.text", text, 1, 100_000)?;
        }
        if self.observations.len() > 256 || self.attachments.len() > 64 {
            return Err(invalid(
                "feedback",
                "must contain at most 256 observations and 64 attachments",
            ));
        }
        for observation in &self.observations {
            observation.validate()?;
        }
        for attachment in &self.attachments {
            attachment.validate()?;
        }
        if self.text.is_none() && self.observations.is_empty() && self.attachments.is_empty() {
            return Err(invalid(
                "feedback",
                "must contain text, a structured observation, or an attachment",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeScope {
    Implementation,
    Expression,
    Organ,
    Genome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesiredChange {
    pub scope: ChangeScope,
    pub description: String,
}

impl DesiredChange {
    pub fn validate(&self) -> Result<()> {
        validate_token(
            "evolutionary_intent.change.description",
            &self.description,
            1,
            4000,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvolutionaryIntent {
    pub schema: String,
    pub expression_id: ExpressionId,
    pub from_version_id: VersionId,
    pub preserved_invariants: Vec<String>,
    pub desired_changes: Vec<DesiredChange>,
    pub feedback_actions: Vec<Bytes32>,
    pub references: Vec<ArtifactAvailability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_genome_action: Option<Bytes32>,
}

impl EvolutionaryIntent {
    pub fn validate(&self) -> Result<()> {
        require_schema(
            "evolutionary_intent.schema",
            &self.schema,
            EVOLUTIONARY_INTENT_SCHEMA,
        )?;
        if self.expression_id.is_zero() || self.from_version_id.is_zero() {
            return Err(invalid(
                "evolutionary_intent.identity",
                "expression_id and from_version_id must not be zero",
            ));
        }
        validate_tokens(
            "evolutionary_intent.preserved_invariants",
            &self.preserved_invariants,
            1,
            128,
            2000,
        )?;
        if self.desired_changes.is_empty() || self.desired_changes.len() > 128 {
            return Err(invalid(
                "evolutionary_intent.desired_changes",
                "must contain 1..=128 changes",
            ));
        }
        for change in &self.desired_changes {
            change.validate()?;
        }
        if self.feedback_actions.len() > 256 || self.references.len() > 64 {
            return Err(invalid(
                "evolutionary_intent",
                "contains too many feedback references or artifacts",
            ));
        }
        if self.feedback_actions.contains(&Bytes32::ZERO)
            || self.proposed_genome_action == Some(Bytes32::ZERO)
        {
            return Err(invalid(
                "evolutionary_intent.action_reference",
                "must not be zero",
            ));
        }
        let mut reference_digests = BTreeSet::new();
        for reference in &self.references {
            reference.validate()?;
            if !reference_digests.insert(reference.artifact.digest) {
                return Err(invalid(
                    "evolutionary_intent.references",
                    "must not repeat artifact content",
                ));
            }
        }
        let proposes_genome_change = self
            .desired_changes
            .iter()
            .any(|change| change.scope == ChangeScope::Genome);
        if proposes_genome_change != self.proposed_genome_action.is_some() {
            return Err(invalid(
                "evolutionary_intent.proposed_genome_action",
                "must be present exactly when a genome-scoped change is proposed",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evolution {
    pub schema: String,
    pub evolutionary_intent_action: Bytes32,
    pub expression_id: ExpressionId,
    pub from_version_id: VersionId,
    pub to_version_id: VersionId,
    pub from_genome_digest: Bytes32,
    pub to_genome_digest: Bytes32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genome_acceptance_action: Option<Bytes32>,
    pub preserved_invariants: Vec<String>,
    pub completed_at: CanonicalTimestamp,
}

impl Evolution {
    pub fn validate(&self) -> Result<()> {
        require_schema("evolution.schema", &self.schema, EVOLUTION_SCHEMA)?;
        require_nonzero(
            "evolution.evolutionary_intent_action",
            self.evolutionary_intent_action,
        )?;
        if self.expression_id.is_zero()
            || self.from_version_id.is_zero()
            || self.to_version_id.is_zero()
            || self.from_version_id == self.to_version_id
        {
            return Err(invalid(
                "evolution.identity",
                "must connect two distinct versions of a nonzero expression",
            ));
        }
        require_nonzero("evolution.from_genome_digest", self.from_genome_digest)?;
        require_nonzero("evolution.to_genome_digest", self.to_genome_digest)?;
        validate_tokens(
            "evolution.preserved_invariants",
            &self.preserved_invariants,
            1,
            128,
            2000,
        )?;
        let mutated = self.from_genome_digest != self.to_genome_digest;
        if mutated != self.genome_acceptance_action.is_some()
            || self.genome_acceptance_action == Some(Bytes32::ZERO)
        {
            return Err(invalid(
                "evolution.genome_acceptance_action",
                "must be present and nonzero exactly when the accepted genome changes",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ownership {
    pub schema: String,
    pub previous_controller: BuilderId,
    pub new_controller: BuilderId,
    pub new_controller_key: P256PublicKey,
    pub reason: String,
    pub effective_at: CanonicalTimestamp,
}

impl Ownership {
    pub fn validate(&self) -> Result<()> {
        require_schema("ownership.schema", &self.schema, OWNERSHIP_SCHEMA)?;
        self.previous_controller.validate()?;
        self.new_controller.validate()?;
        if self.previous_controller == self.new_controller {
            return Err(invalid(
                "ownership.new_controller",
                "must differ from the previous controller",
            ));
        }
        self.new_controller_key.validate()?;
        validate_token("ownership.reason", &self.reason, 1, 2000)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenAssociationOperation {
    Associate,
    Remove,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainAnchor {
    /// Chain on which this witness transaction exists. It is intentionally
    /// independent from the associated token's chain.
    pub chain_id: u64,
    pub contract: Address20,
    pub transaction: Bytes32,
}

impl ChainAnchor {
    pub fn validate(&self) -> Result<()> {
        validate_safe_positive("chain_anchor.chain_id", self.chain_id)?;
        if self.contract.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(invalid("chain_anchor.contract", "must not be zero"));
        }
        require_nonzero("chain_anchor.transaction", self.transaction)
    }
}

/// Optional economic relationship. It never supplies Shot, expression, version,
/// or ownership identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenAssociation {
    pub schema: String,
    pub operation: TokenAssociationOperation,
    pub chain_id: u64,
    pub token: Address20,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<ChainAnchor>,
}

impl TokenAssociation {
    pub fn validate(&self) -> Result<()> {
        require_schema(
            "token_association.schema",
            &self.schema,
            TOKEN_ASSOCIATION_SCHEMA,
        )?;
        validate_safe_positive("token_association.chain_id", self.chain_id)?;
        if self.token.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(invalid("token_association.token", "must not be zero"));
        }
        if let Some(symbol) = &self.symbol {
            validate_token("token_association.symbol", symbol, 1, 32)?;
        }
        if let Some(anchor) = &self.anchor {
            anchor.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationGate {
    pub name: String,
    pub passed: bool,
    pub deterministic: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<ArtifactAvailability>,
}

impl VerificationGate {
    pub fn validate(&self) -> Result<()> {
        validate_token("verification.gate.name", &self.name, 1, 255)?;
        if let Some(evidence) = &self.evidence {
            evidence.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationResult {
    pub schema: String,
    pub expression_id: ExpressionId,
    pub candidate_version_id: VersionId,
    pub genome_revision: u64,
    pub genome_digest: Bytes32,
    pub source_digest: Bytes32,
    pub capability_graph_digest: Bytes32,
    pub gates: Vec<VerificationGate>,
    pub passed: bool,
    pub known_incompleteness: Vec<String>,
    pub verified_at: CanonicalTimestamp,
}

impl VerificationResult {
    pub fn validate(&self) -> Result<()> {
        require_schema(
            "verification_result.schema",
            &self.schema,
            VERIFICATION_RESULT_SCHEMA,
        )?;
        if self.expression_id.is_zero() || self.candidate_version_id.is_zero() {
            return Err(invalid(
                "verification_result.identity",
                "expression and candidate version IDs must not be zero",
            ));
        }
        validate_safe_positive("verification_result.genome_revision", self.genome_revision)?;
        require_nonzero("verification_result.genome_digest", self.genome_digest)?;
        require_nonzero("verification_result.source_digest", self.source_digest)?;
        require_nonzero(
            "verification_result.capability_graph_digest",
            self.capability_graph_digest,
        )?;
        if self.gates.is_empty() || self.gates.len() > 256 {
            return Err(invalid(
                "verification_result.gates",
                "must contain 1..=256 gates",
            ));
        }
        let mut names = BTreeSet::new();
        for gate in &self.gates {
            gate.validate()?;
            if !names.insert(&gate.name) {
                return Err(invalid(
                    "verification_result.gates",
                    "must not repeat a gate name",
                ));
            }
        }
        let observed = self.gates.iter().all(|gate| gate.passed);
        if self.passed != observed {
            return Err(invalid(
                "verification_result.passed",
                "must equal the conjunction of all gate results",
            ));
        }
        validate_tokens(
            "verification_result.known_incompleteness",
            &self.known_incompleteness,
            0,
            128,
            2000,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParentRelationKind {
    Fork,
    Descendant,
    InspiredBy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParentAuthorizationStatement {
    pub schema: String,
    pub parent_shot_id: ShotId,
    pub parent_head: Bytes32,
    pub child_shot_id: ShotId,
    pub relation: ParentRelationKind,
}

impl ParentAuthorizationStatement {
    pub const SCHEMA: &'static str = "tohseno.parent-authorization/2";

    pub fn validate(&self) -> Result<()> {
        require_schema("parent_authorization.schema", &self.schema, Self::SCHEMA)?;
        if self.parent_shot_id.is_zero()
            || self.child_shot_id.is_zero()
            || self.parent_shot_id == self.child_shot_id
        {
            return Err(invalid(
                "parent_authorization.identity",
                "parent and child must be distinct nonzero Shot IDs",
            ));
        }
        require_nonzero("parent_authorization.parent_head", self.parent_head)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParentRelation {
    pub schema: String,
    pub parent_shot_id: ShotId,
    pub parent_head: Bytes32,
    pub child_shot_id: ShotId,
    pub relation: ParentRelationKind,
    /// Proves only the statement bytes and signer. A verifier must separately
    /// establish that this key controlled the parent at `parent_head`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_authorization: Option<SignatureSidecar>,
}

impl ParentRelation {
    pub fn statement(&self) -> ParentAuthorizationStatement {
        ParentAuthorizationStatement {
            schema: ParentAuthorizationStatement::SCHEMA.into(),
            parent_shot_id: self.parent_shot_id,
            parent_head: self.parent_head,
            child_shot_id: self.child_shot_id,
            relation: self.relation,
        }
    }

    pub fn validate(&self) -> Result<()> {
        require_schema(
            "parent_relation.schema",
            &self.schema,
            PARENT_RELATION_SCHEMA,
        )?;
        let statement = self.statement();
        statement.validate()?;
        if let Some(signature) = &self.parent_authorization {
            signature.verify(&statement)?;
        }
        Ok(())
    }
}

/// An availability observation attached to a named role in the Shot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactAvailabilityRecord {
    pub target_role: String,
    pub availability: ArtifactAvailability,
    pub observed_at: CanonicalTimestamp,
}

// Explicit aliases make the "record" role discoverable without creating a
// second wire representation.
pub type ExpressionRecord = Expression;
pub type OrganDeclaration = Organ;
pub type FeedbackRecord = Feedback;
pub type EvolutionaryIntentRecord = EvolutionaryIntent;
pub type EvolutionRecord = Evolution;
pub type OwnershipRecord = Ownership;
pub type TokenAssociationRecord = TokenAssociation;
pub type VerificationResultRecord = VerificationResult;

impl ArtifactAvailabilityRecord {
    pub fn validate(&self) -> Result<()> {
        validate_token(
            "artifact_availability_record.target_role",
            &self.target_role,
            1,
            255,
        )?;
        self.availability.validate()
    }
}
