//! Filesystem body of one persistent Shot.
//!
//! Canonical ontology values come from `tohseno-protocol`; this module only
//! owns their local paths, safe persistence, human working surfaces, and
//! private attachment storage. It deliberately does not define fallback
//! protocol records.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use tohseno_protocol::app_metadata::AppMetadataV2;
use tohseno_protocol::digest::{sha256, Bytes32, ExpressionId, ShotId, VersionId};
use tohseno_protocol::lineage::AdaptedV1Lineage;
use tohseno_protocol::ontology::{
    ArtifactAvailability, ArtifactDescriptor, AvailabilityStatus, ARTIFACT_AVAILABILITY_SCHEMA,
};
use tohseno_protocol::tree_hash::SourceTreeCommitment;
use tohseno_protocol::{
    reduce_lineage, Feedback, LineagePayload, SignedLineageAction, VersionRecord,
};

pub const INTENTION_DOCUMENT: &str = "INTENTION.md";
pub const GENOME_DOCUMENT: &str = "GENOME.md";
pub const EVOLUTIONARY_INTENT_DOCUMENT: &str = "EVOLUTIONARY_INTENT.md";
pub const FEEDBACK_DIRECTORY: &str = "feedback";
pub const VERSIONS_DIRECTORY: &str = "versions";
pub const LINEAGE_FILE: &str = "lineage.jsonl";
pub const PORTABLE_MANIFEST_FILE: &str = "shot-bundle.json";

const SHOT_README: &str = r#"# TOHSENO Shot

This directory is a local body of one persistent Shot. The Shot is not this folder, repository, app, or any token; those are expressions or associations that may change while its signed identity continues.

- `INTENTION.md` preserves the human's exact original material.
- `GENOME.md` is the deterministic human view of the currently accepted operational constraints.
- `EVOLUTIONARY_INTENT.md` is the private working surface for the next proposed change.
- `.tohseno/lineage.jsonl` is the signed append-only history; `.tohseno/shot.json` is only its rebuildable local head.
- `versions/` records immutable accepted expression states.
- `feedback/versions/` binds private experience to an exact ExpressionID and VersionID.
- Application source is one expression of this Shot and embeds its protocol identity when materialized.

Intention, Genome, evolutionary intent, feedback, Version projections, signed lineage, derived `.tohseno/` protocol views, and `.tohseno/private/` working material are private by default and excluded from ordinary source publication. Use TOHSENO's explicit verified export flow when sharing reviewed records.
"#;

const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
const PRIVATE_FILE_MODE: u32 = 0o600;
const PUBLIC_DIRECTORY_MODE: u32 = 0o755;
const PUBLIC_FILE_MODE: u32 = 0o644;
const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;
const MAX_LINEAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_ATTACHMENT_BYTES: usize = 64 * 1024 * 1024;
const MAX_PUBLIC_ACTION_BYTES: usize = 256 * 1024;
const MAX_ATTACHMENTS: usize = 16;
const MAX_PORTABLE_BYTES: u64 = 256 * 1024 * 1024;
const FEEDBACK_INDEX_SCHEMA_V1: &str = "tohseno.local-feedback-index/1";
const FEEDBACK_INDEX_SCHEMA_V2: &str = "tohseno.local-feedback-index/2";
const PENDING_EVOLUTION_SELECTION_SCHEMA_V1: &str = "tohseno.pending-evolution-selection/1";
const PENDING_EVOLUTION_SELECTION_SCHEMA_V2: &str = "tohseno.pending-evolution-selection/2";
pub const MAX_PRIVATE_REFERENCES: usize = 8;

const PRIVATE_IGNORE_BLOCK: &str = r#"# BEGIN TOHSENO PRIVATE MATERIAL
INTENTION.md
GENOME.md
EVOLUTIONARY_INTENT.md
feedback/
versions/
.tohseno/intent.md
.tohseno/pending-intent.md
.tohseno/TASK.md
.tohseno/references/
.tohseno/EVOLUTION_INTENT.md
.tohseno/executions/
.tohseno/lineage.jsonl
.tohseno/shot.json
.tohseno/intention.json
.tohseno/genome.json
.tohseno/expression.json
.tohseno/ownership.json
.tohseno/capabilities.lock
.tohseno/verification.json
.tohseno/protocol-version
.tohseno/import.json
.tohseno/feedback/
.tohseno/private/
.tohseno/incomplete/
.tohseno/evolutions/*/prompt.md
.tohseno/evolutions/*/images/
.tohseno/evolutions/*/TASK.md
.tohseno/evolutions/*/build.log
.tohseno/evolutions/*/harness.log
.tohseno/evolutions/*/artifact/
.tohseno/evolutions/*/previous-src/
# END TOHSENO PRIVATE MATERIAL
"#;

const EVOLUTIONARY_INTENT_TEMPLATE: &str = r#"# Evolutionary Intent

Current version:

Feedback and references:

Desired changes:

Invariants to preserve:

Proposed genome mutations:

"#;

/// These visible paths describe the Shot around an expression. They are not
/// application source and must not enter a v1 source snapshot.
pub fn is_shot_level_path(normalized: &str) -> bool {
    let first = normalized.split('/').next().unwrap_or_default();
    matches!(
        first,
        "README.md"
            | INTENTION_DOCUMENT
            | GENOME_DOCUMENT
            | EVOLUTIONARY_INTENT_DOCUMENT
            | FEEDBACK_DIRECTORY
            | VERSIONS_DIRECTORY
    ) || normalized == ".gitignore"
}

/// Hash a living software expression without treating Shot-level working
/// surfaces as application source.
///
/// The strict v1 sealed-source hash is deliberately unchanged. This adapter
/// uses the protocol's existing lenient walk, removes only engine-owned Shot
/// body paths, then recomputes the same domain-separated tree commitment.
pub fn hash_expression_working_tree(root: &Path) -> tohseno_protocol::Result<SourceTreeCommitment> {
    let mut commitment = tohseno_protocol::tree_hash::hash_working_tree(root)?;
    commitment
        .entries
        .retain(|entry| !is_shot_level_path(&entry.path));
    commitment.digest = tohseno_protocol::tree_hash::hash_entries(&commitment.entries)?;
    Ok(commitment)
}

/// Describe one bounded local feedback attachment without publishing it.
///
/// The same no-follow read used by storage establishes the digest and length,
/// so a caller cannot accidentally sign path metadata instead of exact bytes.
pub fn describe_feedback_attachment(
    source: &Path,
) -> Result<ArtifactAvailability, ShotLayoutError> {
    let attachment = read_private_attachment(source)?;
    let name = source
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| ShotLayoutError::UnsafePath(source.into()))?
        .to_owned();
    let availability = ArtifactAvailability {
        schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
        artifact: ArtifactDescriptor {
            digest: attachment.digest,
            media_type: media_type_for_extension(attachment.extension.as_deref()).into(),
            byte_length: attachment.byte_length,
            name: Some(name),
        },
        status: AvailabilityStatus::IntentionallyPrivate,
        locations: Vec::new(),
    };
    availability.validate()?;
    Ok(availability)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShotLayout {
    root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredFeedback {
    pub feedback_id: Bytes32,
    /// Commitment of the signed Feedback lineage action. This—not the
    /// payload-only `feedback_id`—is what EvolutionaryIntent references.
    pub action_commitment: Bytes32,
    pub directory: PathBuf,
    pub attachments: Vec<PathBuf>,
}

/// One exact private reference staged under its content digest.
///
/// The signed protocol record carries only `availability`; `path` is a local
/// private resolution and never becomes a public location claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredReference {
    pub availability: ArtifactAvailability,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedIntentReference {
    pub label: String,
    pub relative_path: String,
    pub availability: ArtifactAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedIntentPackage {
    pub intention_digest: Bytes32,
    pub document_digest: Bytes32,
    pub document_relative_path: String,
    pub references: Vec<PreparedIntentReference>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedMaterialization {
    pub version: VersionRecord,
    pub version_path: PathBuf,
    pub feedback_directory: PathBuf,
    pub lineage_head: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingEvolutionSelection {
    schema: String,
    prompt_digest: Bytes32,
    feedback_actions: Vec<Bytes32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    references: Vec<ArtifactAvailability>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedExpressionHead {
    pub expression_id: ExpressionId,
    pub current_version: Option<VersionId>,
    pub accepted_version_count: u64,
}

/// Rebuildable local identity/head cache. Canonical authority remains the
/// signed lineage; this object is deliberately small and contains no raw
/// private intention or feedback.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedShotSnapshot {
    pub schema: String,
    pub protocol_version: String,
    pub shot_id: ShotId,
    pub origin_action: Bytes32,
    pub controller: tohseno_protocol::identity::BuilderId,
    pub current_genome_revision: Option<u64>,
    pub current_genome_digest: Option<Bytes32>,
    pub expressions: Vec<DerivedExpressionHead>,
    pub lineage_sequence: u64,
    pub lineage_head: Bytes32,
    pub public_action_count: u64,
    pub private_action_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotBodyVerification {
    pub shot_id: ShotId,
    pub controller: tohseno_protocol::identity::BuilderId,
    pub protocol_version: String,
    pub lineage_sequence: u64,
    pub lineage_head: Bytes32,
    pub legacy_v1_adapter: bool,
    pub intention_bytes_verified: bool,
    pub genome_revision: Option<u64>,
    pub genome_digest: Option<Bytes32>,
    pub selected_expression_id: Option<ExpressionId>,
    pub selected_version_id: Option<VersionId>,
    pub embedded_metadata_verified: bool,
    pub missing_attachment_digests: Vec<Bytes32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortableVisibility {
    Public,
    IncludePrivate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableShotManifest {
    pub schema: String,
    pub protocol_version: String,
    pub shot_id: ShotId,
    pub controller: tohseno_protocol::identity::BuilderId,
    pub lineage_head: Bytes32,
    pub action_count: u64,
    pub visibility: PortableVisibility,
    pub intention_bytes: AvailabilityStatus,
    pub feedback_bytes: AvailabilityStatus,
    pub materialization_ready: bool,
    pub omitted_artifacts: Vec<String>,
    /// Attachment descriptors carried by canonical feedback whose exact bytes
    /// are not present in this bundle.
    #[serde(default)]
    pub missing_attachment_digests: Vec<Bytes32>,
    pub files: Vec<PortableFile>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableFile {
    pub path: String,
    pub digest: Bytes32,
    pub byte_length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedShot {
    pub layout: ShotLayout,
    pub manifest: PortableShotManifest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum FeedbackPresence {
    Absent,
    Present,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FeedbackVersionIndex {
    schema: String,
    expression_id: ExpressionId,
    version_id: VersionId,
    status: FeedbackPresence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct LegacyVersionProjection {
    schema: &'static str,
    version_id: VersionId,
    expression_id: ExpressionId,
    record_commitment: Bytes32,
    genome_status: AvailabilityStatus,
}

struct PrivateAttachment {
    source: PathBuf,
    bytes: Vec<u8>,
    digest: Bytes32,
    byte_length: u64,
    extension: Option<String>,
}

/// Canonical human rendering of the accepted machine-readable Genome.
///
/// Owners review this surface, while `.tohseno/genome.json` and the signed
/// acceptance action remain the machine facts. Verification regenerates these
/// exact bytes, so either side drifting is a hard error.
pub fn render_genome_document(
    genome: &tohseno_protocol::Genome,
) -> Result<String, ShotLayoutError> {
    genome.validate()?;
    let digest = genome.digest()?;
    let mut output = format!(
        "# Genome\n\nRevision: {}\nDigest: {}\n\n## Purpose\n\n{}\n",
        genome.revision, digest, genome.purpose
    );
    for (title, values) in [
        ("Intended human or community", &genome.intended_for),
        ("Essential experience", &genome.essential_experience),
        ("What must remain true", &genome.behavioral_invariants),
        ("Interaction laws", &genome.interaction_laws),
        ("Aesthetic principles", &genome.aesthetic_principles),
        ("Privacy principles", &genome.privacy_principles),
        ("Ownership principles", &genome.ownership_principles),
        ("Platform commitments", &genome.platform_commitments),
        ("Boundaries", &genome.boundaries),
        ("Non-goals", &genome.non_goals),
        ("Required capabilities", &genome.required_capabilities),
        ("What must never happen", &genome.forbidden_transformations),
        ("Acceptance principles", &genome.acceptance_principles),
        ("What may change freely", &genome.freely_changeable),
    ] {
        output.push_str("\n## ");
        output.push_str(title);
        output.push_str("\n\n");
        if values.is_empty() {
            output.push_str("- None declared.\n");
        } else {
            for value in values {
                output.push_str("- ");
                output.push_str(value);
                output.push('\n');
            }
        }
    }
    Ok(output)
}

impl PortableShotManifest {
    const SCHEMA: &'static str = "tohseno.shot-bundle/1";

    fn validate(&self) -> Result<(), ShotLayoutError> {
        let omitted_are_exact = self.omitted_artifacts.iter().map(String::as_str).eq([
            "expression_source",
            "retained_build_artifact",
            "private_working_memory",
            "owner_private_keys",
        ]);
        if self.schema != Self::SCHEMA
            || self.protocol_version != tohseno_protocol::lineage::LINEAGE_PROTOCOL_VERSION
            || self.shot_id.is_zero()
            || self.lineage_head == Bytes32::ZERO
            || self.action_count == 0
            || self.materialization_ready
            || !omitted_are_exact
        {
            return Err(ShotLayoutError::Invalid(
                "portable Shot manifest has invalid or dishonest fixed facts".into(),
            ));
        }
        self.controller.validate().map_err(ShotLayoutError::from)?;
        if self.files.is_empty() || self.files.len() > 1026 {
            return Err(ShotLayoutError::Invalid(
                "portable inventory must contain 1..=1026 files".into(),
            ));
        }
        let mut previous: Option<&str> = None;
        let mut lineage_files = 0_usize;
        let mut total_bytes = 0_u64;
        for file in &self.files {
            validate_portable_file(file)?;
            total_bytes = total_bytes
                .checked_add(file.byte_length)
                .ok_or_else(|| ShotLayoutError::Limit("portable byte count overflowed".into()))?;
            if previous.is_some_and(|value| value >= file.path.as_str()) {
                return Err(ShotLayoutError::Invalid(
                    "portable inventory paths must be unique and byte-sorted".into(),
                ));
            }
            previous = Some(&file.path);
            if file.path == LINEAGE_FILE {
                lineage_files += 1;
            }
        }
        if total_bytes > MAX_PORTABLE_BYTES {
            return Err(ShotLayoutError::Limit(
                "portable inventory exceeds 256 MiB".into(),
            ));
        }
        let mut previous_missing = None;
        for digest in &self.missing_attachment_digests {
            if *digest == Bytes32::ZERO
                || previous_missing.is_some_and(|previous| previous >= *digest)
            {
                return Err(ShotLayoutError::Invalid(
                    "missing attachment digests must be nonzero, unique, and sorted".into(),
                ));
            }
            previous_missing = Some(*digest);
        }
        if lineage_files != 1 {
            return Err(ShotLayoutError::Invalid(
                "portable inventory must contain exactly one lineage.jsonl".into(),
            ));
        }
        let carries_intention = self
            .files
            .iter()
            .any(|file| file.path == INTENTION_DOCUMENT);
        let carries_feedback = self
            .files
            .iter()
            .any(|file| file.path.starts_with("feedback/"));
        if carries_intention != (self.intention_bytes != AvailabilityStatus::Absent)
            || carries_feedback != (self.feedback_bytes != AvailabilityStatus::Absent)
        {
            return Err(ShotLayoutError::Invalid(
                "portable inventory disagrees with byte availability".into(),
            ));
        }
        match self.visibility {
            PortableVisibility::Public
                if self.feedback_bytes != AvailabilityStatus::Absent
                    || !matches!(
                        self.intention_bytes,
                        AvailabilityStatus::Absent
                            | AvailabilityStatus::PubliclyAvailable
                            | AvailabilityStatus::Replicated
                    ) =>
            {
                Err(ShotLayoutError::Invalid(
                    "public bundle claims private carried bytes".into(),
                ))
            }
            PortableVisibility::IncludePrivate
                if !matches!(
                    self.intention_bytes,
                    AvailabilityStatus::Absent
                        | AvailabilityStatus::IntentionallyPrivate
                        | AvailabilityStatus::LocallyAvailable
                        | AvailabilityStatus::PubliclyAvailable
                        | AvailabilityStatus::Replicated
                        | AvailabilityStatus::CryptographicallyVerified
                ) || !matches!(
                    self.feedback_bytes,
                    AvailabilityStatus::Absent | AvailabilityStatus::IntentionallyPrivate
                ) =>
            {
                Err(ShotLayoutError::Invalid(
                    "private bundle byte availability is inconsistent".into(),
                ))
            }
            _ => Ok(()),
        }
    }
}

impl ShotLayout {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn metadata_root(&self) -> PathBuf {
        self.root.join(".tohseno")
    }

    pub fn lineage_path(&self) -> PathBuf {
        self.metadata_root().join(LINEAGE_FILE)
    }

    pub fn initialize_directories(&self) -> Result<(), ShotLayoutError> {
        require_real_directory(&self.root)?;
        let metadata = self.metadata_root();
        ensure_directory(&metadata, true)?;
        for relative in [
            "private",
            "private/planning",
            "private/agent-runs",
            "feedback",
            "feedback/versions",
            "references",
        ] {
            ensure_directory(&metadata.join(relative), true)?;
        }
        ensure_directory(&self.root.join(FEEDBACK_DIRECTORY), true)?;
        ensure_directory(&self.root.join(FEEDBACK_DIRECTORY).join("versions"), true)?;
        ensure_directory(&self.root.join(VERSIONS_DIRECTORY), false)?;
        self.ensure_private_ignore_rules()?;
        let readme = self.root.join("README.md");
        match fs::symlink_metadata(&readme) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {}
            Ok(_) => return Err(ShotLayoutError::UnsafePath(readme)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                write_new_file(&readme, SHOT_README.as_bytes(), false)?;
            }
            Err(error) => return Err(error.into()),
        }
        if !self.root.join(EVOLUTIONARY_INTENT_DOCUMENT).exists() {
            write_new_file(
                &self.root.join(EVOLUTIONARY_INTENT_DOCUMENT),
                EVOLUTIONARY_INTENT_TEMPLATE.as_bytes(),
                true,
            )?;
        }
        ensure_file(&self.lineage_path(), b"", true)?;
        Ok(())
    }

    /// Preserve the human's exact UTF-8 material. Existing bytes must match;
    /// migration and retries never silently rewrite the source intention.
    pub fn preserve_exact_intention(&self, source: &[u8]) -> Result<Bytes32, ShotLayoutError> {
        std::str::from_utf8(source)
            .map_err(|_| ShotLayoutError::Invalid("intention must be valid UTF-8".into()))?;
        self.initialize_directories()?;
        let path = self.root.join(INTENTION_DOCUMENT);
        ensure_exact_file(&path, source, true)?;
        Ok(sha256(source))
    }

    /// Copy explicitly supplied reference bytes into private,
    /// content-addressed storage and return their exact protocol descriptors.
    ///
    /// All sources are validated before any object is written. Names must be
    /// safe portable components; names that collide under ASCII
    /// case-insensitive comparison and repeated content are rejected.
    pub fn stage_private_references(
        &self,
        sources: &[PathBuf],
    ) -> Result<Vec<StoredReference>, ShotLayoutError> {
        if sources.len() > MAX_PRIVATE_REFERENCES {
            return Err(ShotLayoutError::Limit(format!(
                "this Apple factory accepts at most {MAX_PRIVATE_REFERENCES} references"
            )));
        }
        let mut prepared = Vec::with_capacity(sources.len());
        let mut names = BTreeSet::new();
        let mut digests = BTreeSet::new();
        for source in sources {
            let name = source
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| ShotLayoutError::UnsafePath(source.clone()))?
                .to_owned();
            validate_reference_name(&name)?;
            let collision_key = name.to_ascii_lowercase();
            if !names.insert(collision_key) {
                return Err(ShotLayoutError::Invalid(
                    "private reference names collide on Apple filesystems".into(),
                ));
            }
            let attachment = read_private_attachment(source)?;
            if !digests.insert(attachment.digest) {
                return Err(ShotLayoutError::Invalid(
                    "private references must not repeat content".into(),
                ));
            }
            let availability = private_reference_availability(&attachment, name)?;
            prepared.push((attachment, availability));
        }
        prepared.sort_by_key(|(_, availability)| availability.artifact.digest);

        self.initialize_directories()?;
        let destination = self.metadata_root().join("references");
        let mut stored = Vec::with_capacity(prepared.len());
        for (attachment, availability) in prepared {
            let path = destination.join(attachment.digest.to_string().trim_start_matches("0x"));
            ensure_exact_file(&path, &attachment.bytes, true).map_err(|error| match error {
                ShotLayoutError::ImmutableConflict(_) => ShotLayoutError::Invalid(format!(
                    "content-addressed private reference conflicts with {}",
                    attachment.source.display()
                )),
                other => other,
            })?;
            stored.push(StoredReference { availability, path });
        }
        Ok(stored)
    }

    /// Build the human-facing, deterministic intention package consumed by a
    /// native harness. Original attachment names never enter the package:
    /// input order alone assigns `image_1` through `image_8`.
    pub fn prepare_intent_package(
        &self,
        intention: &[u8],
        sources: &[PathBuf],
    ) -> Result<(PreparedIntentPackage, Vec<StoredReference>), ShotLayoutError> {
        let intention_text = std::str::from_utf8(intention)
            .map_err(|_| ShotLayoutError::Invalid("intention must be valid UTF-8".into()))?;
        if intention_text.trim().is_empty() {
            return Err(ShotLayoutError::Invalid(
                "prepared intention must not be empty".into(),
            ));
        }
        if sources.len() > MAX_PRIVATE_REFERENCES {
            return Err(ShotLayoutError::Limit(format!(
                "a prepared Shot accepts at most {MAX_PRIVATE_REFERENCES} reference images"
            )));
        }
        let mut prepared = Vec::with_capacity(sources.len());
        let mut digests = BTreeSet::new();
        for (index, source) in sources.iter().enumerate() {
            let extension = source
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase)
                .filter(|extension| {
                    ["png", "jpg", "jpeg", "heic", "webp"].contains(&extension.as_str())
                })
                .ok_or_else(|| {
                    ShotLayoutError::Invalid(format!(
                        "{} is not a supported PNG, JPEG, HEIC, or WebP image",
                        source.display()
                    ))
                })?;
            let attachment = read_private_attachment(source)?;
            validate_image_bytes(&extension, &attachment.bytes).map_err(|reason| {
                ShotLayoutError::Invalid(format!("{}: {reason}", source.display()))
            })?;
            if !digests.insert(attachment.digest) {
                return Err(ShotLayoutError::Invalid(
                    "reference images must not repeat content".into(),
                ));
            }
            let label = format!("image_{}", index + 1);
            let filename = format!("{label}.{extension}");
            let availability = private_reference_availability(&attachment, filename.clone())?;
            prepared.push((attachment, label, filename, availability));
        }

        self.initialize_directories()?;
        let references_root = self.metadata_root().join("references");
        remove_prepared_reference_aliases(&references_root)?;
        let mut stored = Vec::with_capacity(prepared.len());
        let mut references = Vec::with_capacity(prepared.len());
        for (attachment, label, filename, availability) in prepared {
            let object_path =
                references_root.join(attachment.digest.to_string().trim_start_matches("0x"));
            ensure_exact_file(&object_path, &attachment.bytes, true).map_err(
                |error| match error {
                    ShotLayoutError::ImmutableConflict(_) => ShotLayoutError::Invalid(format!(
                        "content-addressed private reference conflicts with {}",
                        attachment.source.display()
                    )),
                    other => other,
                },
            )?;
            let alias_path = references_root.join(&filename);
            write_replace_file(&alias_path, &attachment.bytes, true)?;
            stored.push(StoredReference {
                availability: availability.clone(),
                path: object_path,
            });
            references.push(PreparedIntentReference {
                label,
                relative_path: format!(".tohseno/references/{filename}"),
                availability,
            });
        }

        let mut document = String::from("# TOHSENO Evolution Intent\n\n## Intention\n\n");
        document.push_str(intention_text.trim());
        document.push_str("\n\n## Reference images\n\n");
        if references.is_empty() {
            document.push_str("No reference images were supplied.\n");
        } else {
            for reference in &references {
                document.push_str(&format!(
                    "- {}: `{}`\n",
                    reference.label, reference.relative_path
                ));
            }
            document.push_str(
                "\nInspect these images as part of the implementation context. Treat them as references for the intention, not as files to modify unless the intention explicitly requires it.\n",
            );
        }
        let document_path = self.metadata_root().join("EVOLUTION_INTENT.md");
        write_replace_file(&document_path, document.as_bytes(), true)?;
        let package = PreparedIntentPackage {
            intention_digest: sha256(intention),
            document_digest: sha256(document.as_bytes()),
            document_relative_path: ".tohseno/EVOLUTION_INTENT.md".into(),
            references,
        };
        let mut encoded = tohseno_protocol::canonical::to_vec(&package)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        encoded.push(b'\n');
        write_replace_file(
            &self
                .metadata_root()
                .join("private/planning/prepared-intent.json"),
            &encoded,
            true,
        )?;
        Ok((package, stored))
    }

    pub fn prepared_intent_package(&self) -> Result<PreparedIntentPackage, ShotLayoutError> {
        let package = read_canonical_json::<PreparedIntentPackage>(
            &self
                .metadata_root()
                .join("private/planning/prepared-intent.json"),
        )?;
        let document = read_regular_limited(
            &self.metadata_root().join("EVOLUTION_INTENT.md"),
            MAX_JSON_BYTES,
        )?;
        if sha256(&document) != package.document_digest {
            return Err(ShotLayoutError::Invalid(
                "prepared intention document changed after preparation".into(),
            ));
        }
        if package.references.len() > MAX_PRIVATE_REFERENCES {
            return Err(ShotLayoutError::Invalid(
                "prepared intention exceeds eight references".into(),
            ));
        }
        for (index, reference) in package.references.iter().enumerate() {
            if reference.label != format!("image_{}", index + 1)
                || !reference
                    .relative_path
                    .starts_with(".tohseno/references/image_")
            {
                return Err(ShotLayoutError::Invalid(
                    "prepared image labels are not deterministic".into(),
                ));
            }
            let path = self.root.join(&reference.relative_path);
            let bytes = read_regular_limited(&path, MAX_ATTACHMENT_BYTES)?;
            if sha256(&bytes) != reference.availability.artifact.digest
                || u64::try_from(bytes.len()).ok()
                    != Some(reference.availability.artifact.byte_length)
            {
                return Err(ShotLayoutError::Invalid(format!(
                    "{} no longer matches its prepared digest",
                    reference.label
                )));
            }
        }
        Ok(package)
    }

    /// Read one staged private reference only when its exact descriptor still
    /// matches the bounded, non-symlink object at the digest-derived path.
    pub fn read_private_reference(
        &self,
        availability: &ArtifactAvailability,
    ) -> Result<Vec<u8>, ShotLayoutError> {
        validate_private_reference_availability(availability)?;
        let path = self.metadata_root().join("references").join(
            availability
                .artifact
                .digest
                .to_string()
                .trim_start_matches("0x"),
        );
        let bytes = read_regular_limited(&path, MAX_ATTACHMENT_BYTES)?;
        if sha256(&bytes) != availability.artifact.digest
            || u64::try_from(bytes.len()).ok() != Some(availability.artifact.byte_length)
        {
            return Err(ShotLayoutError::Invalid(
                "private reference bytes differ from their exact descriptor".into(),
            ));
        }
        Ok(bytes)
    }

    /// Preserve an exact private planning artifact under the ignored working
    /// memory boundary. It is never a substitute for a signed lineage action.
    pub fn preserve_private_planning_file(
        &self,
        filename: &str,
        bytes: &[u8],
    ) -> Result<Bytes32, ShotLayoutError> {
        validate_metadata_filename(filename)?;
        if bytes.len() > MAX_JSON_BYTES {
            return Err(ShotLayoutError::Limit(
                "private planning artifact exceeds 4 MiB".into(),
            ));
        }
        self.initialize_directories()?;
        let path = self
            .metadata_root()
            .join("private")
            .join("planning")
            .join(filename);
        ensure_exact_file(&path, bytes, true)?;
        Ok(sha256(bytes))
    }

    fn pending_evolution_selection_path(&self) -> PathBuf {
        self.metadata_root()
            .join("private")
            .join("planning")
            .join("pending-evolution-selection.json")
    }

    fn pending_evolution_prompt_path(&self) -> PathBuf {
        self.metadata_root().join("pending-intent.md")
    }

    pub fn stage_evolution_prompt(&self, prompt: &[u8]) -> Result<(), ShotLayoutError> {
        validate_evolution_prompt(prompt)?;
        self.initialize_directories()?;
        write_replace_file(&self.pending_evolution_prompt_path(), prompt, true)
    }

    pub fn pending_evolution_prompt(&self) -> Result<Option<String>, ShotLayoutError> {
        let path = self.pending_evolution_prompt_path();
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                Err(ShotLayoutError::UnsafePath(path))
            }
            Ok(_) => String::from_utf8(read_regular_limited(&path, MAX_JSON_BYTES)?)
                .map(Some)
                .map_err(|_| {
                    ShotLayoutError::Invalid("pending evolutionary instruction is not UTF-8".into())
                }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn clear_evolution_prompt(&self, prompt: &[u8]) -> Result<(), ShotLayoutError> {
        let path = self.pending_evolution_prompt_path();
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                Err(ShotLayoutError::UnsafePath(path))
            }
            Ok(_) => {
                let observed = read_regular_limited(&path, MAX_JSON_BYTES)?;
                if observed != prompt {
                    return Err(ShotLayoutError::Invalid(
                        "refusing to clear a different pending evolutionary instruction".into(),
                    ));
                }
                fs::remove_file(path)?;
                Ok(())
            }
        }
    }

    /// Stage one exact evolutionary instruction together with its selected
    /// signed Feedback actions and explicitly supplied private references.
    ///
    /// Content objects may be safely left behind by a failed attempt; the
    /// prompt-bound selection is replaced only after every source validates
    /// and every content-addressed write succeeds.
    pub fn stage_evolution_inputs(
        &self,
        prompt: &[u8],
        feedback_actions: &[Bytes32],
        reference_sources: &[PathBuf],
    ) -> Result<Vec<StoredReference>, ShotLayoutError> {
        validate_evolution_prompt(prompt)?;
        validate_feedback_actions(feedback_actions)?;
        let (_, references) = self.prepare_intent_package(prompt, reference_sources)?;
        let mut selected = references
            .iter()
            .map(|reference| reference.availability.clone())
            .collect::<Vec<_>>();
        selected.sort_by_key(|reference| reference.artifact.digest);
        self.write_pending_evolution_selection(prompt, feedback_actions, &selected)?;
        self.stage_evolution_prompt(prompt)?;
        Ok(references)
    }

    /// Compatibility surface for callers that select only Feedback.
    /// Payload digests remain invalid: the reducer consumes signed action
    /// commitments.
    pub fn stage_evolution_feedback_selection(
        &self,
        prompt: &[u8],
        feedback_actions: &[Bytes32],
    ) -> Result<(), ShotLayoutError> {
        validate_feedback_actions(feedback_actions)?;
        self.write_pending_evolution_selection(prompt, feedback_actions, &[])
    }

    fn write_pending_evolution_selection(
        &self,
        prompt: &[u8],
        feedback_actions: &[Bytes32],
        references: &[ArtifactAvailability],
    ) -> Result<(), ShotLayoutError> {
        self.initialize_directories()?;
        let path = self.pending_evolution_selection_path();
        if feedback_actions.is_empty() && references.is_empty() {
            return remove_regular_file_if_present(&path);
        }
        let pending = PendingEvolutionSelection {
            schema: PENDING_EVOLUTION_SELECTION_SCHEMA_V2.into(),
            prompt_digest: sha256(prompt),
            feedback_actions: feedback_actions.to_vec(),
            references: references.to_vec(),
        };
        validate_pending_evolution_selection(&pending, prompt)?;
        let mut encoded = tohseno_protocol::canonical::to_vec(&pending)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        encoded.push(b'\n');
        write_replace_file(&path, &encoded, true)
    }

    fn pending_evolution_selection(
        &self,
        prompt: &[u8],
    ) -> Result<Option<PendingEvolutionSelection>, ShotLayoutError> {
        let path = self.pending_evolution_selection_path();
        let pending = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(ShotLayoutError::UnsafePath(path))
            }
            Ok(_) => read_canonical_json::<PendingEvolutionSelection>(&path)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        validate_pending_evolution_selection(&pending, prompt)?;
        Ok(Some(pending))
    }

    /// Resolve staged Feedback selections only for the exact prompt bytes
    /// reviewed with them. A failed materialization leaves this intact.
    pub fn pending_evolution_feedback_selection(
        &self,
        prompt: &[u8],
    ) -> Result<Vec<Bytes32>, ShotLayoutError> {
        Ok(self.pending_evolution_inputs(prompt)?.0)
    }

    /// Resolve and reauthenticate every exact private reference selected for
    /// this prompt before it can enter a signed EvolutionaryIntent.
    pub fn pending_evolution_references(
        &self,
        prompt: &[u8],
    ) -> Result<Vec<ArtifactAvailability>, ShotLayoutError> {
        Ok(self.pending_evolution_inputs(prompt)?.1)
    }

    /// Resolve Feedback and private artifacts from one exact read of their
    /// shared prompt-bound selection, then reauthenticate all local bytes.
    pub fn pending_evolution_inputs(
        &self,
        prompt: &[u8],
    ) -> Result<(Vec<Bytes32>, Vec<ArtifactAvailability>), ShotLayoutError> {
        let Some(pending) = self.pending_evolution_selection(prompt)? else {
            return Ok((Vec::new(), Vec::new()));
        };
        let references = pending.references;
        for reference in &references {
            self.read_private_reference(reference)?;
        }
        Ok((pending.feedback_actions, references))
    }

    pub fn clear_evolution_feedback_selection(&self, prompt: &[u8]) -> Result<(), ShotLayoutError> {
        let path = self.pending_evolution_selection_path();
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                Err(ShotLayoutError::UnsafePath(path))
            }
            Ok(_) => {
                let pending = read_canonical_json::<PendingEvolutionSelection>(&path)?;
                if pending.prompt_digest != sha256(prompt)
                    || !matches!(
                        pending.schema.as_str(),
                        PENDING_EVOLUTION_SELECTION_SCHEMA_V1
                            | PENDING_EVOLUTION_SELECTION_SCHEMA_V2
                    )
                {
                    return Err(ShotLayoutError::Invalid(
                        "refusing to clear an input selection for a different instruction".into(),
                    ));
                }
                fs::remove_file(path)?;
                Ok(())
            }
        }
    }

    pub fn write_pending_evolution_document(&self, document: &str) -> Result<(), ShotLayoutError> {
        self.initialize_directories()?;
        write_replace_file(
            &self.root.join(EVOLUTIONARY_INTENT_DOCUMENT),
            document.as_bytes(),
            true,
        )
    }

    pub fn write_human_genome(&self, rendered: &str) -> Result<Bytes32, ShotLayoutError> {
        if rendered.is_empty() || !rendered.ends_with('\n') {
            return Err(ShotLayoutError::Invalid(
                "the deterministic genome rendering must end with one newline".into(),
            ));
        }
        self.initialize_directories()?;
        write_replace_file(&self.root.join(GENOME_DOCUMENT), rendered.as_bytes(), false)?;
        Ok(sha256(rendered.as_bytes()))
    }

    pub fn verify_human_genome(&self, rendered: &str) -> Result<Bytes32, ShotLayoutError> {
        let path = self.root.join(GENOME_DOCUMENT);
        let observed = read_regular_limited(&path, MAX_JSON_BYTES)?;
        if observed != rendered.as_bytes() {
            return Err(ShotLayoutError::GenomeDrift);
        }
        Ok(sha256(&observed))
    }

    /// Refresh the deterministic human and machine views of an already
    /// accepted Genome. This does not accept or mutate a Genome.
    pub fn write_accepted_genome(
        &self,
        genome: &tohseno_protocol::Genome,
    ) -> Result<(), ShotLayoutError> {
        let lineage = self.read_lineage()?;
        let state = reduce_lineage(&lineage)?;
        let accepted = state.accepted_genome.as_ref().ok_or_else(|| {
            ShotLayoutError::Invalid("cannot write a Genome that is not accepted".into())
        })?;
        if &accepted.genome != genome {
            return Err(ShotLayoutError::Invalid(
                "requested Genome is not the current signed accepted revision".into(),
            ));
        }
        self.write_metadata_json("genome.json", genome, false)?;
        self.write_human_genome(&render_genome_document(genome)?)?;
        Ok(())
    }

    /// Deterministically verify the complete local v2 Shot body, or a frozen
    /// v1 compatibility projection when no v2 lineage exists.
    ///
    /// `expression_id` selects one expression when a Shot has several.
    /// Omitting it selects the sole expression, if there is exactly one.
    pub fn verify_shot_body(
        &self,
        expression_id: Option<ExpressionId>,
    ) -> Result<ShotBodyVerification, ShotLayoutError> {
        let lineage = self.read_lineage()?;
        if lineage.is_empty() {
            return self.verify_legacy_v1_body();
        }
        let state = reduce_lineage(&lineage)?;
        let snapshot =
            read_canonical_json::<DerivedShotSnapshot>(&self.metadata_root().join("shot.json"))?;
        let expected_snapshot = derived_shot_snapshot(&lineage, &state)?;
        if snapshot != expected_snapshot {
            return Err(ShotLayoutError::Invalid(
                "derived shot.json does not reproduce the canonical lineage head".into(),
            ));
        }

        let intention_bytes_verified =
            self.verify_local_intention_bytes(state.intention.as_ref())?;
        let (genome_revision, genome_digest) = match &state.accepted_genome {
            Some(accepted) => {
                let machine = read_canonical_json::<tohseno_protocol::Genome>(
                    &self.metadata_root().join("genome.json"),
                )?;
                if machine != accepted.genome {
                    return Err(ShotLayoutError::Invalid(
                        "derived genome.json is not the accepted signed Genome".into(),
                    ));
                }
                let rendered = render_genome_document(&machine)?;
                self.verify_human_genome(&rendered)?;
                (Some(machine.revision), Some(machine.digest()?))
            }
            None => {
                if self.root.join(GENOME_DOCUMENT).exists()
                    || self.metadata_root().join("genome.json").exists()
                {
                    return Err(ShotLayoutError::Invalid(
                        "Genome views exist before a signed Genome acceptance".into(),
                    ));
                }
                (None, None)
            }
        };

        let selected_expression_id = match expression_id {
            Some(value) => {
                if !state.expressions.contains_key(&value) {
                    return Err(ShotLayoutError::Invalid(
                        "selected ExpressionID is not in the canonical lineage".into(),
                    ));
                }
                Some(value)
            }
            None if state.expressions.len() == 1 => state.expressions.keys().next().copied(),
            None => None,
        };
        let mut selected_version_id = None;
        let mut embedded_metadata_verified = false;
        if let Some(selected) = selected_expression_id {
            let expression = state
                .expression(selected)
                .expect("selected expression was checked above");
            let expression_view = read_canonical_json::<tohseno_protocol::Expression>(
                &self.metadata_root().join("expression.json"),
            )?;
            if expression_view != expression.expression {
                return Err(ShotLayoutError::Invalid(
                    "derived expression.json is not the selected canonical Expression".into(),
                ));
            }
            let capability_view = read_canonical_json::<Vec<tohseno_protocol::Organ>>(
                &self.metadata_root().join("capabilities.lock"),
            )?;
            let expected_capabilities = expression.organs.values().cloned().collect::<Vec<_>>();
            if capability_view != expected_capabilities {
                return Err(ShotLayoutError::Invalid(
                    "capabilities.lock does not reproduce the selected canonical Organ graph"
                        .into(),
                ));
            }
            if let Some(current_version_id) = expression.current_version {
                let version = expression
                    .versions
                    .iter()
                    .find(|candidate| candidate.version_id == current_version_id)
                    .ok_or_else(|| {
                        ShotLayoutError::Invalid(
                            "expression current version is absent from reduced versions".into(),
                        )
                    })?;
                let ordinal = u32::try_from(version.ordinal).map_err(|_| {
                    ShotLayoutError::Limit(
                        "selected version ordinal does not fit local layout".into(),
                    )
                })?;
                let local_version =
                    self.read_local_version_record(selected, ordinal, state.expressions.len())?;
                if &local_version != version {
                    return Err(ShotLayoutError::Invalid(
                        "visible Version record is not the selected accepted Version".into(),
                    ));
                }
                selected_version_id = Some(version.version_id);
                if let Some(path) = embedded_metadata_path(&self.root)? {
                    let metadata = read_embedded_metadata_v2(&path)?;
                    self.verify_apple_materialization_binding(&metadata, version)?;
                    embedded_metadata_verified = true;
                } else if has_xcode_project(&self.root)? {
                    return Err(ShotLayoutError::Invalid(
                        "Apple expression source is present without embedded AppMetadataV2".into(),
                    ));
                }
            }
        }

        let feedback_root = self.root.join(FEEDBACK_DIRECTORY);
        let missing_attachment_digests = if directory_has_regular_files(&feedback_root)? {
            validate_feedback_storage(&feedback_root, &state)?
        } else {
            Vec::new()
        };
        Ok(ShotBodyVerification {
            shot_id: state.shot_id,
            controller: state.controller,
            protocol_version: tohseno_protocol::lineage::LINEAGE_PROTOCOL_VERSION.into(),
            lineage_sequence: state.sequence,
            lineage_head: state.head,
            legacy_v1_adapter: false,
            intention_bytes_verified,
            genome_revision,
            genome_digest,
            selected_expression_id,
            selected_version_id,
            embedded_metadata_verified,
            missing_attachment_digests,
        })
    }

    fn verify_legacy_v1_body(&self) -> Result<ShotBodyVerification, ShotLayoutError> {
        let path = self.metadata_root().join("legacy-v1.json");
        let adapted = read_canonical_json::<AdaptedV1Lineage>(&path)?;
        let entries = adapted
            .entries
            .iter()
            .map(|entry| (&entry.record, &entry.signature))
            .collect::<Vec<_>>();
        if tohseno_protocol::adapt_v1_lineage(&entries)? != adapted {
            return Err(ShotLayoutError::Invalid(
                "legacy-v1.json does not reproduce its frozen signed records".into(),
            ));
        }
        let selected_version_id = adapted.entries.last().map(|entry| entry.version_id);
        Ok(ShotBodyVerification {
            shot_id: adapted.shot_id,
            controller: adapted.controller,
            protocol_version: "1-adapter".into(),
            lineage_sequence: u64::try_from(adapted.entries.len())
                .map_err(|_| ShotLayoutError::Limit("legacy lineage length overflowed".into()))?,
            lineage_head: adapted.head,
            legacy_v1_adapter: true,
            intention_bytes_verified: false,
            genome_revision: None,
            genome_digest: None,
            selected_expression_id: Some(adapted.expression_id),
            selected_version_id,
            embedded_metadata_verified: false,
            missing_attachment_digests: Vec::new(),
        })
    }

    fn verify_local_intention_bytes(
        &self,
        intention: Option<&tohseno_protocol::IntentionRecord>,
    ) -> Result<bool, ShotLayoutError> {
        let Some(intention) = intention else {
            if self.root.join(INTENTION_DOCUMENT).exists() {
                return Err(ShotLayoutError::Invalid(
                    "INTENTION.md exists before the canonical intention action".into(),
                ));
            }
            return Ok(false);
        };
        let path = self.root.join(INTENTION_DOCUMENT);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                Err(ShotLayoutError::UnsafePath(path))
            }
            Ok(_) => {
                let bytes = read_regular_limited(&path, MAX_JSON_BYTES)?;
                if !intention.materials.iter().any(|material| {
                    material.artifact.artifact.digest == sha256(&bytes)
                        && material.artifact.artifact.byte_length
                            == u64::try_from(bytes.len()).unwrap_or(u64::MAX)
                }) {
                    return Err(ShotLayoutError::Invalid(
                        "INTENTION.md does not match any canonical original material".into(),
                    ));
                }
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if intention.materials.iter().any(|material| {
                    matches!(
                        material.artifact.status,
                        AvailabilityStatus::LocallyAvailable
                            | AvailabilityStatus::PubliclyAvailable
                            | AvailabilityStatus::Replicated
                            | AvailabilityStatus::CryptographicallyVerified
                    )
                }) {
                    return Err(ShotLayoutError::Invalid(
                        "canonical intention claims available bytes but INTENTION.md is absent"
                            .into(),
                    ));
                }
                Ok(false)
            }
            Err(error) => Err(error.into()),
        }
    }

    fn read_local_version_record(
        &self,
        expression_id: ExpressionId,
        ordinal: u32,
        expression_count: usize,
    ) -> Result<VersionRecord, ShotLayoutError> {
        let scoped = self
            .root
            .join(VERSIONS_DIRECTORY)
            .join(expression_component(expression_id))
            .join(format!("{ordinal:04}"))
            .join("version.json");
        match fs::symlink_metadata(&scoped) {
            Ok(_) => read_canonical_json(&scoped),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // Adapter for the first single-expression v2 layout.
                if expression_count != 1 {
                    return Err(ShotLayoutError::Invalid(
                        "unscoped Version storage is ambiguous for multiple expressions".into(),
                    ));
                }
                read_canonical_json(
                    &self
                        .root
                        .join(VERSIONS_DIRECTORY)
                        .join(format!("{ordinal:04}"))
                        .join("version.json"),
                )
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn write_metadata_json(
        &self,
        filename: &str,
        value: &impl Serialize,
        private: bool,
    ) -> Result<Bytes32, ShotLayoutError> {
        validate_metadata_filename(filename)?;
        self.initialize_directories()?;
        let bytes = tohseno_protocol::canonical::to_vec(value)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        if bytes.len() > MAX_JSON_BYTES {
            return Err(ShotLayoutError::Limit("JSON exceeds 4 MiB".into()));
        }
        let digest = sha256(&bytes);
        let mut terminated = bytes;
        terminated.push(b'\n');
        write_replace_file(&self.metadata_root().join(filename), &terminated, private)?;
        Ok(digest)
    }

    fn write_version_json(
        &self,
        sequence: u32,
        filename: &str,
        value: &impl Serialize,
    ) -> Result<PathBuf, ShotLayoutError> {
        if sequence == 0 {
            return Err(ShotLayoutError::Invalid(
                "version sequence must be positive".into(),
            ));
        }
        validate_metadata_filename(filename)?;
        self.initialize_directories()?;
        let directory = self
            .root
            .join(VERSIONS_DIRECTORY)
            .join(format!("{sequence:04}"));
        ensure_directory(&directory, false)?;
        let bytes = tohseno_protocol::canonical::to_vec(value)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        if bytes.len() > MAX_JSON_BYTES {
            return Err(ShotLayoutError::Limit("version JSON exceeds 4 MiB".into()));
        }
        let mut terminated = bytes;
        terminated.push(b'\n');
        let path = directory.join(filename);
        ensure_exact_file(&path, &terminated, false)?;
        Ok(path)
    }

    /// Persist the immutable canonical record for one accepted version.
    pub fn write_version_record(
        &self,
        shot_id: ShotId,
        record: &VersionRecord,
    ) -> Result<PathBuf, ShotLayoutError> {
        record.validate(shot_id)?;
        let sequence = u32::try_from(record.ordinal).map_err(|_| {
            ShotLayoutError::Limit("version ordinal does not fit the local directory format".into())
        })?;
        self.initialize_directories()?;
        let expression_root = self
            .root
            .join(VERSIONS_DIRECTORY)
            .join(expression_component(record.expression_id));
        ensure_directory(&expression_root, false)?;
        let directory = expression_root.join(format!("{sequence:04}"));
        ensure_directory(&directory, false)?;
        let bytes = tohseno_protocol::canonical::to_vec(record)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        if bytes.len() > MAX_JSON_BYTES {
            return Err(ShotLayoutError::Limit("version JSON exceeds 4 MiB".into()));
        }
        let mut terminated = bytes;
        terminated.push(b'\n');
        let path = directory.join("version.json");
        ensure_exact_file(&path, &terminated, false)?;
        Ok(path)
    }

    /// Verify the non-self-referential boundary shared by embedded Apple
    /// metadata and the canonical Version record.
    ///
    /// `lineage_head` is the last authorized materialization-input action,
    /// never the later verification or Version action. That exact head is
    /// also `VersionRecord.provenance.input_action`, so metadata can be
    /// embedded before the artifact exists without circular commitments.
    pub fn verify_apple_materialization_binding(
        &self,
        metadata: &AppMetadataV2,
        version: &VersionRecord,
    ) -> Result<(), ShotLayoutError> {
        metadata.validate()?;
        version.validate(metadata.shot_id)?;
        if version.expression_id != metadata.expression_id
            || version.version_id != metadata.version_id
            || version.ordinal != metadata.version_ordinal
            || version.genome_revision != metadata.genome_revision
            || version.genome_digest != metadata.genome_digest
            || version.source_digest != metadata.source_tree_sha256
            || version.provenance.input_action != metadata.lineage_head
            || version.build_digest != metadata.build_digest
            || u64::from(metadata.bundle_version) != version.ordinal
        {
            return Err(ShotLayoutError::Invalid(
                "embedded Apple identity and canonical Version facts differ".into(),
            ));
        }

        let lineage = self.read_lineage()?;
        let mut input_index = None;
        for (index, action) in lineage.iter().enumerate() {
            if action.commitment()? == metadata.lineage_head {
                input_index = Some(index);
                break;
            }
        }
        let input_index = input_index.ok_or_else(|| {
            ShotLayoutError::Invalid("embedded metadata lineage head is unavailable locally".into())
        })?;
        let input = &lineage[input_index];
        if input.action.sequence != metadata.lineage_sequence {
            return Err(ShotLayoutError::Invalid(
                "embedded lineage sequence does not identify its lineage head".into(),
            ));
        }
        let state = reduce_lineage(&lineage[..=input_index])?;
        let accepted_genome = state.accepted_genome.as_ref().ok_or_else(|| {
            ShotLayoutError::Invalid("materialization requires an accepted Shot genome".into())
        })?;
        if state.shot_id != metadata.shot_id
            || state.controller != metadata.builder_id
            || accepted_genome.genome.revision != metadata.genome_revision
            || accepted_genome.genome.digest()? != metadata.genome_digest
            || !state.expressions.contains_key(&metadata.expression_id)
        {
            return Err(ShotLayoutError::Invalid(
                "embedded metadata is not authorized by its materialization-input lineage".into(),
            ));
        }
        Ok(())
    }

    /// Authenticate embedded v2 metadata from any completed historical
    /// Evolution against the exact accepted Version in this Shot's canonical
    /// lineage.
    ///
    /// Unlike `verify_shot_body`, this deliberately does not require the
    /// embedded Version to be the expression's current head: refresh and
    /// parent-context verification must remain valid after later Versions are
    /// accepted. The matched Version still has to be present in the
    /// authority-reduced signed lineage, and its materialization-input head is
    /// checked by `verify_apple_materialization_binding`.
    pub fn verify_accepted_apple_metadata(
        &self,
        metadata: &AppMetadataV2,
    ) -> Result<(), ShotLayoutError> {
        metadata.validate()?;
        let lineage = self.read_lineage()?;
        let state = reduce_lineage(&lineage)?;
        if state.shot_id != metadata.shot_id {
            return Err(ShotLayoutError::Invalid(
                "embedded Apple identity belongs to a different Shot".into(),
            ));
        }
        let expression = state.expression(metadata.expression_id).ok_or_else(|| {
            ShotLayoutError::Invalid(
                "embedded Apple ExpressionID is absent from the canonical lineage".into(),
            )
        })?;
        let mut matches = expression
            .versions
            .iter()
            .filter(|version| version.version_id == metadata.version_id);
        let version = matches.next().ok_or_else(|| {
            ShotLayoutError::Invalid(
                "embedded Apple VersionID is absent from the canonical lineage".into(),
            )
        })?;
        if matches.next().is_some() {
            return Err(ShotLayoutError::Invalid(
                "embedded Apple VersionID is ambiguous in the canonical lineage".into(),
            ));
        }
        self.verify_apple_materialization_binding(metadata, version)
    }

    /// Write the visible immutable version view only after the signed lineage
    /// has accepted the exact Version record through a passing verification.
    pub fn write_accepted_apple_version(
        &self,
        metadata: &AppMetadataV2,
        version: &VersionRecord,
    ) -> Result<PathBuf, ShotLayoutError> {
        self.verify_apple_materialization_binding(metadata, version)?;
        let lineage = self.read_lineage()?;
        let state = reduce_lineage(&lineage)?;
        let expression = state.expression(version.expression_id).ok_or_else(|| {
            ShotLayoutError::Invalid("accepted version expression is missing from lineage".into())
        })?;
        if expression.current_version != Some(version.version_id)
            || !expression
                .versions
                .iter()
                .any(|candidate| candidate == version)
        {
            return Err(ShotLayoutError::Invalid(
                "version has not been accepted by the canonical lineage".into(),
            ));
        }
        self.write_version_record(metadata.shot_id, version)
    }

    /// Commit one successful materialization as a single lineage transaction.
    ///
    /// Callers invoke this only after source compilation, retained artifact
    /// materialization, acceptance gates, and embedded-metadata verification
    /// have all succeeded. The protocol reducer independently enforces that
    /// failed verification cannot produce a Version and that a non-initial
    /// Version is followed by an Evolution connected to its accepted intent.
    pub fn record_accepted_materialization(
        &self,
        metadata: &AppMetadataV2,
        verification_action: &SignedLineageAction,
        version_action: &SignedLineageAction,
        evolution_action: Option<&SignedLineageAction>,
    ) -> Result<AcceptedMaterialization, ShotLayoutError> {
        let LineagePayload::VerificationResult(verification) = &verification_action.action.payload
        else {
            return Err(ShotLayoutError::Invalid(
                "first acceptance action must carry VerificationResult".into(),
            ));
        };
        let LineagePayload::Version(version) = &version_action.action.payload else {
            return Err(ShotLayoutError::Invalid(
                "second acceptance action must carry Version".into(),
            ));
        };
        if !verification.passed
            || version.verification_action != verification_action.commitment()?
            || verification.candidate_version_id != version.version_id
        {
            return Err(ShotLayoutError::Invalid(
                "accepted Version is not bound to this passing verification".into(),
            ));
        }
        match (version.ordinal, evolution_action) {
            (1, None) => {}
            (1, Some(_)) => {
                return Err(ShotLayoutError::Invalid(
                    "the first Version is origin, not an Evolution from an earlier Version".into(),
                ))
            }
            (_, Some(action)) => {
                let LineagePayload::Evolution(evolution) = &action.action.payload else {
                    return Err(ShotLayoutError::Invalid(
                        "third acceptance action must carry Evolution".into(),
                    ));
                };
                if evolution.expression_id != version.expression_id
                    || evolution.to_version_id != version.version_id
                {
                    return Err(ShotLayoutError::Invalid(
                        "Evolution does not end at the accepted Version".into(),
                    ));
                }
            }
            (_, None) => {
                return Err(ShotLayoutError::Invalid(
                    "a later Version requires its explicit Evolution action".into(),
                ))
            }
        }

        // This check happens against the pre-acceptance lineage. It proves
        // the embedded head is the authorized input boundary, not a circular
        // reference to any action in the batch below.
        self.verify_apple_materialization_binding(metadata, version)?;
        let mut batch = vec![verification_action.clone(), version_action.clone()];
        if let Some(evolution) = evolution_action {
            batch.push(evolution.clone());
        }
        let commitments = self.append_lineage_batch(&batch)?;
        let version_path = self.write_accepted_apple_version(metadata, version)?;
        let feedback_directory = self.initialize_feedback_for(metadata.shot_id, version)?;
        self.reset_evolutionary_intent()?;
        Ok(AcceptedMaterialization {
            version: version.clone(),
            version_path,
            feedback_directory,
            lineage_head: *commitments
                .last()
                .expect("acceptance batch contains at least two actions"),
        })
    }

    /// Persist the protocol-owned, read-only compatibility projection of the
    /// frozen v1 chain. Repeated migrations are byte-identical and never
    /// rewrite or re-sign historical records.
    pub fn write_v1_migration(
        &self,
        adapted: &AdaptedV1Lineage,
    ) -> Result<PathBuf, ShotLayoutError> {
        let entries = adapted
            .entries
            .iter()
            .map(|entry| (&entry.record, &entry.signature))
            .collect::<Vec<_>>();
        let verified = tohseno_protocol::adapt_v1_lineage(&entries)?;
        if &verified != adapted {
            return Err(ShotLayoutError::Invalid(
                "v1 migration projection differs from the verified frozen records".into(),
            ));
        }
        self.initialize_directories()?;
        let bytes = tohseno_protocol::canonical::to_vec(adapted)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        if bytes.len() > MAX_LINEAGE_BYTES {
            return Err(ShotLayoutError::Limit(
                "v1 compatibility projection exceeds 64 MiB".into(),
            ));
        }
        let mut terminated = bytes;
        terminated.push(b'\n');
        let path = self.metadata_root().join("legacy-v1.json");
        ensure_exact_file(&path, &terminated, false)?;
        for entry in &adapted.entries {
            let sequence = entry.record.sequence;
            self.write_version_json(sequence, "legacy-v1-record.json", &entry.record)?;
            self.write_version_json(sequence, "legacy-v1-signature.json", &entry.signature)?;
            let version = LegacyVersionProjection {
                schema: "tohseno.local-legacy-version/1",
                version_id: entry.version_id,
                expression_id: adapted.expression_id,
                record_commitment: entry.record_commitment,
                genome_status: adapted.genome_availability,
            };
            self.write_version_json(sequence, "legacy-projection.json", &version)?;
        }
        Ok(path)
    }

    /// Read and fully verify the local canonical lineage.
    ///
    /// A local Shot repository carries a complete prefix from its commitment;
    /// partial network segments belong in node storage, not this file.
    pub fn read_lineage(&self) -> Result<Vec<SignedLineageAction>, ShotLayoutError> {
        self.initialize_directories()?;
        let bytes = read_regular_limited(&self.lineage_path(), MAX_LINEAGE_BYTES)?;
        decode_lineage(&bytes)
    }

    /// Append one verified canonical signed protocol action.
    ///
    /// Repeating the current action is idempotent. A conflicting sequence,
    /// non-adjacent predecessor, invalid signature, or invalid ownership
    /// transition is rejected by reducing the candidate complete lineage
    /// before any bytes are appended.
    pub fn append_lineage(&self, action: &SignedLineageAction) -> Result<Bytes32, ShotLayoutError> {
        Ok(self
            .append_lineage_batch(std::slice::from_ref(action))?
            .into_iter()
            .next()
            .expect("one action yields one commitment"))
    }

    /// Persist one already-public signed action as exact canonical bytes for
    /// explicit node ingestion.
    ///
    /// The ignored private outbox prevents source publication from becoming
    /// implicit network publication. The file itself is a public protocol
    /// record: no private action can cross this boundary.
    pub fn write_public_action_outbox(
        &self,
        action: &SignedLineageAction,
    ) -> Result<PathBuf, ShotLayoutError> {
        action.verify()?;
        if action.action.availability != AvailabilityStatus::PubliclyAvailable {
            return Err(ShotLayoutError::Invalid(
                "only a publicly_available signed action may enter the public outbox".into(),
            ));
        }
        let commitment = action.commitment()?;
        let bytes = tohseno_protocol::canonical::to_vec(action)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        if bytes.len() > MAX_PUBLIC_ACTION_BYTES {
            return Err(ShotLayoutError::Limit(
                "public action exceeds the 256 KiB node-ingestion limit".into(),
            ));
        }
        self.initialize_directories()?;
        let root = self
            .metadata_root()
            .join("private")
            .join("public-action-outbox");
        ensure_directory(&root, true)?;
        let path = root.join(format!(
            "{}.json",
            commitment.to_string().trim_start_matches("0x")
        ));
        ensure_exact_file(&path, &bytes, true)?;
        Ok(path)
    }

    /// Atomically append a fully validated action batch.
    ///
    /// The complete candidate lineage is authority-reduced before the
    /// durable replacement. This is the commit boundary used for
    /// VerificationResult → Version → Evolution, so a process failure or
    /// invalid later action cannot expose a partial accepted transition.
    pub fn append_lineage_batch(
        &self,
        actions: &[SignedLineageAction],
    ) -> Result<Vec<Bytes32>, ShotLayoutError> {
        if actions.is_empty() {
            return Err(ShotLayoutError::Invalid(
                "cannot append an empty lineage action batch".into(),
            ));
        }
        let mut current = self.read_lineage()?;
        let mut commitments = Vec::with_capacity(actions.len());
        let mut present = 0_usize;
        for action in actions {
            action.verify()?;
            let commitment = action.commitment()?;
            commitments.push(commitment);
            let index = usize::try_from(action.action.sequence.saturating_sub(1))
                .map_err(|_| ShotLayoutError::Limit("lineage sequence overflowed".into()))?;
            if let Some(existing) = current.get(index) {
                if existing != action {
                    return Err(ShotLayoutError::ImmutableConflict(self.lineage_path()));
                }
                present += 1;
            }
        }
        if present == actions.len() {
            let state = reduce_lineage(&current)?;
            self.write_derived_views(&current, &state)?;
            return Ok(commitments);
        }
        if present != 0 {
            return Err(ShotLayoutError::Invalid(
                "lineage batch is only partially present".into(),
            ));
        }

        current.extend_from_slice(actions);
        let state = reduce_lineage(&current)?;
        let encoded = encode_lineage(&current)?;
        write_replace_file(&self.lineage_path(), &encoded, true)?;
        self.write_derived_views(&current, &state)?;
        Ok(commitments)
    }

    fn write_derived_views(
        &self,
        actions: &[SignedLineageAction],
        state: &tohseno_protocol::lineage::ShotState,
    ) -> Result<(), ShotLayoutError> {
        let snapshot = derived_shot_snapshot(actions, state)?;
        self.write_metadata_json("shot.json", &snapshot, false)?;
        if let Some(accepted) = &state.accepted_genome {
            self.write_metadata_json("genome.json", &accepted.genome, false)?;
            self.write_human_genome(&render_genome_document(&accepted.genome)?)?;
        }
        if state.expressions.len() == 1 {
            let expression = state
                .expressions
                .values()
                .next()
                .expect("one expression is present");
            self.write_metadata_json("expression.json", &expression.expression, false)?;
            self.write_metadata_json(
                "capabilities.lock",
                &expression.organs.values().cloned().collect::<Vec<_>>(),
                false,
            )?;
        }
        Ok(())
    }

    fn initialize_feedback_version(
        &self,
        expression_id: ExpressionId,
        sequence: u32,
    ) -> Result<PathBuf, ShotLayoutError> {
        self.initialize_directories()?;
        let expression_root = self
            .root
            .join(FEEDBACK_DIRECTORY)
            .join("versions")
            .join(expression_component(expression_id));
        ensure_directory(&expression_root, true)?;
        let path = expression_root.join(format!("{sequence:04}"));
        ensure_directory(&path, true)?;
        Ok(path)
    }

    /// Initialize honest private feedback storage for one accepted version.
    pub fn initialize_feedback_for(
        &self,
        shot_id: ShotId,
        version: &VersionRecord,
    ) -> Result<PathBuf, ShotLayoutError> {
        version.validate(shot_id)?;
        let sequence = u32::try_from(version.ordinal).map_err(|_| {
            ShotLayoutError::Limit("version ordinal does not fit the feedback layout".into())
        })?;
        let path = self.initialize_feedback_version(version.expression_id, sequence)?;
        let index = FeedbackVersionIndex {
            schema: FEEDBACK_INDEX_SCHEMA_V2.into(),
            expression_id: version.expression_id,
            version_id: version.version_id,
            status: FeedbackPresence::Absent,
        };
        let encoded = tohseno_protocol::canonical::to_vec(&index)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        let mut terminated = encoded;
        terminated.push(b'\n');
        ensure_exact_file(&path.join("index.json"), &terminated, true)?;
        Ok(path)
    }

    /// Store validated feedback bound to one exact accepted version and copy
    /// explicitly supplied attachments into private content-addressed paths.
    ///
    /// Every supplied attachment must have a matching digest and byte length
    /// in the canonical Feedback record. Remote or unavailable attachment
    /// descriptors may remain without local bytes.
    pub fn store_feedback(
        &self,
        shot_id: ShotId,
        version: &VersionRecord,
        record: &Feedback,
        action_commitment: Bytes32,
        attachments: &[PathBuf],
    ) -> Result<StoredFeedback, ShotLayoutError> {
        version.validate(shot_id)?;
        record.validate()?;
        if action_commitment == Bytes32::ZERO {
            return Err(ShotLayoutError::Invalid(
                "feedback storage requires its signed action commitment".into(),
            ));
        }
        if record.expression_id != version.expression_id
            || record.version_id != version.version_id
            || record.build_identity != version.build_identity
        {
            return Err(ShotLayoutError::Invalid(
                "feedback does not identify the exact accepted expression version and build".into(),
            ));
        }
        if attachments.len() > MAX_ATTACHMENTS {
            return Err(ShotLayoutError::Limit(format!(
                "feedback accepts at most {MAX_ATTACHMENTS} attachments"
            )));
        }
        let version_directory = self.initialize_feedback_for(shot_id, version)?;
        let feedback_id = tohseno_protocol::canonical::sha256_commitment(record)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        let directory = version_directory.join(feedback_id.to_string().trim_start_matches("0x"));
        ensure_directory(&directory, true)?;
        let attachment_root = directory.join("attachments");
        ensure_directory(&attachment_root, true)?;
        let mut copied = Vec::new();
        let mut observed = BTreeSet::new();
        for source in attachments {
            let attachment = read_private_attachment(source)?;
            if !observed.insert(attachment.digest) {
                return Err(ShotLayoutError::Invalid(
                    "feedback attachments must not repeat content".into(),
                ));
            }
            let declared = record.attachments.iter().any(|candidate| {
                candidate.artifact.digest == attachment.digest
                    && candidate.artifact.byte_length == attachment.byte_length
                    && !matches!(
                        candidate.status,
                        AvailabilityStatus::Absent | AvailabilityStatus::Unknown
                    )
            });
            if !declared {
                return Err(ShotLayoutError::Invalid(format!(
                    "feedback attachment {} is not declared by digest and length",
                    source.display()
                )));
            }
            copied.push(store_private_attachment(&attachment, &attachment_root)?);
        }
        let encoded = tohseno_protocol::canonical::to_vec(record)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        if encoded.len() > MAX_JSON_BYTES {
            return Err(ShotLayoutError::Limit(
                "feedback metadata exceeds 4 MiB".into(),
            ));
        }
        let mut terminated = encoded;
        terminated.push(b'\n');
        ensure_exact_file(&directory.join("feedback.json"), &terminated, true)?;
        let index = FeedbackVersionIndex {
            schema: FEEDBACK_INDEX_SCHEMA_V2.into(),
            expression_id: version.expression_id,
            version_id: version.version_id,
            status: FeedbackPresence::Present,
        };
        let index_bytes = tohseno_protocol::canonical::to_vec(&index)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        let mut terminated_index = index_bytes;
        terminated_index.push(b'\n');
        write_replace_file(
            &version_directory.join("index.json"),
            &terminated_index,
            true,
        )?;
        Ok(StoredFeedback {
            feedback_id,
            action_commitment,
            directory,
            attachments: copied,
        })
    }

    /// Append a signed exact-version feedback action and persist its private
    /// local body. The reducer rejects unknown or floating versions before
    /// any lineage bytes are changed.
    pub fn record_feedback_action(
        &self,
        shot_id: ShotId,
        version: &VersionRecord,
        record: &Feedback,
        action: &SignedLineageAction,
        attachments: &[PathBuf],
    ) -> Result<StoredFeedback, ShotLayoutError> {
        let LineagePayload::Feedback(payload) = &action.action.payload else {
            return Err(ShotLayoutError::Invalid(
                "feedback action carries the wrong protocol payload".into(),
            ));
        };
        if action.action.shot_id != shot_id
            || action.action.availability != AvailabilityStatus::IntentionallyPrivate
            || payload != record
        {
            return Err(ShotLayoutError::Invalid(
                "feedback action does not exactly bind the private feedback record".into(),
            ));
        }
        // Validate all local storage facts before committing the action.
        validate_feedback_inputs(shot_id, version, record, attachments)?;
        let action_commitment = action.commitment()?;
        self.append_lineage(action)?;
        self.store_feedback(shot_id, version, record, action_commitment, attachments)
    }

    /// Export a verified portable Shot record bundle.
    ///
    /// The bundle is intentionally not a source-code archive. It carries the
    /// signed identity and lineage needed for inspection or following, and
    /// states that source/build artifacts remain unavailable. Private bytes
    /// are included only through the explicit `IncludePrivate` mode.
    pub fn export_bundle(
        &self,
        destination: &Path,
        visibility: PortableVisibility,
    ) -> Result<PortableShotManifest, ShotLayoutError> {
        let lineage = self.read_lineage()?;
        if lineage.is_empty() {
            return Err(ShotLayoutError::Invalid(
                "a portable bundle requires a canonical Shot lineage".into(),
            ));
        }
        if visibility == PortableVisibility::Public
            && lineage
                .iter()
                .any(|action| action.action.availability != AvailabilityStatus::PubliclyAvailable)
        {
            return Err(ShotLayoutError::Invalid(
                "public export cannot leak or omit intentionally private lineage actions".into(),
            ));
        }
        let state = reduce_lineage(&lineage)?;
        let intention_path = self.root.join(INTENTION_DOCUMENT);
        let intention = match fs::symlink_metadata(&intention_path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(ShotLayoutError::UnsafePath(intention_path))
            }
            Ok(_) => {
                let bytes = read_regular_limited(&intention_path, MAX_JSON_BYTES)?;
                let declared = state.intention.as_ref().and_then(|record| {
                    record.materials.iter().find_map(|material| {
                        (material.artifact.artifact.digest == sha256(&bytes)
                            && material.artifact.artifact.byte_length
                                == u64::try_from(bytes.len()).unwrap_or(u64::MAX))
                        .then_some(material.artifact.status)
                    })
                });
                let Some(status) = declared else {
                    return Err(ShotLayoutError::Invalid(
                        "INTENTION.md does not match the canonical intention action".into(),
                    ));
                };
                Some((bytes, status))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let intention = match (visibility, intention) {
            (
                PortableVisibility::Public,
                Some((bytes, status @ AvailabilityStatus::PubliclyAvailable)),
            )
            | (
                PortableVisibility::Public,
                Some((bytes, status @ AvailabilityStatus::Replicated)),
            )
            | (PortableVisibility::IncludePrivate, Some((bytes, status))) => Some((bytes, status)),
            (PortableVisibility::Public, Some(_)) | (_, None) => None,
        };
        if visibility == PortableVisibility::Public
            && lineage.iter().any(|action| {
                matches!(
                    &action.action.payload,
                    LineagePayload::Intention(record)
                        if record.materials.iter().any(|material| {
                            material.inline_text.is_some()
                                && !matches!(
                                    material.artifact.status,
                                    AvailabilityStatus::PubliclyAvailable
                                        | AvailabilityStatus::Replicated
                                )
                        })
                )
            })
        {
            return Err(ShotLayoutError::Invalid(
                "public export would expose private intention bytes embedded in lineage".into(),
            ));
        }
        let feedback_source = self.root.join(FEEDBACK_DIRECTORY);
        let include_feedback = visibility == PortableVisibility::IncludePrivate
            && directory_has_regular_files(&feedback_source)?;
        let missing_attachment_digests = if include_feedback {
            validate_feedback_storage(&feedback_source, &state)?
        } else {
            canonical_declared_attachment_digests(&state)
        };
        let private = visibility == PortableVisibility::IncludePrivate;
        let staging = create_staging_sibling(destination)?;
        let result = (|| {
            let lineage_bytes = encode_lineage(&lineage)?;
            write_new_file(&staging.join(LINEAGE_FILE), &lineage_bytes, private)?;
            if let Some((bytes, _)) = &intention {
                write_new_file(&staging.join(INTENTION_DOCUMENT), bytes, private)?;
            }
            if include_feedback {
                copy_bounded_tree(&feedback_source, &staging.join(FEEDBACK_DIRECTORY), true)?;
            }

            // Inventory and semantic checks are made over the staged bytes,
            // not over paths that will be reread after the manifest is made.
            let files = collect_bundle_payload_inventory(&staging)?;
            let observed_missing = if include_feedback {
                validate_feedback_storage(&staging.join(FEEDBACK_DIRECTORY), &state)?
            } else {
                Vec::new()
            };
            if observed_missing != missing_attachment_digests {
                return Err(ShotLayoutError::Invalid(
                    "staged feedback availability changed during export".into(),
                ));
            }
            let manifest = PortableShotManifest {
                schema: PortableShotManifest::SCHEMA.into(),
                protocol_version: tohseno_protocol::lineage::LINEAGE_PROTOCOL_VERSION.into(),
                shot_id: state.shot_id,
                controller: state.controller,
                lineage_head: state.head,
                action_count: u64::try_from(lineage.len()).map_err(|_| {
                    ShotLayoutError::Limit("lineage action count overflowed".into())
                })?,
                visibility,
                intention_bytes: intention
                    .as_ref()
                    .map(|(_, status)| *status)
                    .unwrap_or(AvailabilityStatus::Absent),
                feedback_bytes: if include_feedback {
                    AvailabilityStatus::IntentionallyPrivate
                } else {
                    AvailabilityStatus::Absent
                },
                materialization_ready: false,
                omitted_artifacts: vec![
                    "expression_source".into(),
                    "retained_build_artifact".into(),
                    "private_working_memory".into(),
                    "owner_private_keys".into(),
                ],
                missing_attachment_digests: observed_missing,
                files,
            };
            manifest.validate()?;
            let mut encoded = tohseno_protocol::canonical::to_vec(&manifest)
                .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
            encoded.push(b'\n');
            // The manifest is written last inside the private staging
            // directory. Its presence marks a complete bundle.
            write_new_file(&staging.join(PORTABLE_MANIFEST_FILE), &encoded, private)?;
            validate_bundle_inventory(&staging)?;
            let rehashed = collect_bundle_payload_inventory(&staging)?;
            if rehashed != manifest.files {
                return Err(ShotLayoutError::Invalid(
                    "staged portable inventory changed before publication".into(),
                ));
            }
            publish_staged_directory(&staging, destination, private)?;
            Ok(manifest)
        })();
        if result.is_err() {
            cleanup_staging_directory(&staging);
        }
        result
    }

    /// Import and verify a portable Shot record without claiming ownership,
    /// cloning source, or materializing an expression.
    pub fn import_bundle(
        bundle: &Path,
        destination: &Path,
    ) -> Result<ImportedShot, ShotLayoutError> {
        require_real_directory(bundle)?;
        validate_bundle_inventory(bundle)?;
        let manifest_bytes =
            read_regular_limited(&bundle.join(PORTABLE_MANIFEST_FILE), MAX_JSON_BYTES)?;
        let manifest = serde_json::from_slice::<PortableShotManifest>(&manifest_bytes)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        let mut canonical_manifest = tohseno_protocol::canonical::to_vec(&manifest)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        canonical_manifest.push(b'\n');
        if manifest_bytes != canonical_manifest {
            return Err(ShotLayoutError::Invalid(
                "portable manifest is not canonical JSON".into(),
            ));
        }
        manifest.validate()?;
        let mut observed_files = Vec::new();
        let lineage_observed = read_regular_limited(&bundle.join(LINEAGE_FILE), MAX_LINEAGE_BYTES)?;
        observed_files.push(portable_file(LINEAGE_FILE, &lineage_observed)?);
        if bundle.join(INTENTION_DOCUMENT).exists() {
            let bytes = read_regular_limited(&bundle.join(INTENTION_DOCUMENT), MAX_JSON_BYTES)?;
            observed_files.push(portable_file(INTENTION_DOCUMENT, &bytes)?);
        }
        if bundle.join(FEEDBACK_DIRECTORY).exists() {
            collect_portable_inventory(
                &bundle.join(FEEDBACK_DIRECTORY),
                &bundle.join(FEEDBACK_DIRECTORY),
                FEEDBACK_DIRECTORY,
                &mut observed_files,
            )?;
        }
        observed_files.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
        if observed_files != manifest.files {
            return Err(ShotLayoutError::Invalid(
                "portable file inventory digest or length mismatch".into(),
            ));
        }
        let lineage_bytes = lineage_observed;
        let lineage = decode_lineage(&lineage_bytes)?;
        let state = reduce_lineage(&lineage)?;
        if state.shot_id != manifest.shot_id
            || state.controller != manifest.controller
            || state.head != manifest.lineage_head
            || u64::try_from(lineage.len()).ok() != Some(manifest.action_count)
        {
            return Err(ShotLayoutError::Invalid(
                "portable manifest does not match its verified lineage".into(),
            ));
        }
        if manifest.visibility == PortableVisibility::Public
            && lineage
                .iter()
                .any(|action| action.action.availability != AvailabilityStatus::PubliclyAvailable)
        {
            return Err(ShotLayoutError::Invalid(
                "public bundle contains a private lineage action".into(),
            ));
        }

        let intention = match fs::symlink_metadata(bundle.join(INTENTION_DOCUMENT)) {
            Ok(_) => Some(read_regular_limited(
                &bundle.join(INTENTION_DOCUMENT),
                MAX_JSON_BYTES,
            )?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if intention.is_some() != (manifest.intention_bytes != AvailabilityStatus::Absent) {
            return Err(ShotLayoutError::Invalid(
                "portable intention availability does not match carried bytes".into(),
            ));
        }
        if let Some(bytes) = &intention {
            let status = state.intention.as_ref().and_then(|record| {
                record.materials.iter().find_map(|material| {
                    (material.artifact.artifact.digest == sha256(bytes)
                        && material.artifact.artifact.byte_length
                            == u64::try_from(bytes.len()).unwrap_or(u64::MAX))
                    .then_some(material.artifact.status)
                })
            });
            if status != Some(manifest.intention_bytes) {
                return Err(ShotLayoutError::Invalid(
                    "portable intention bytes or availability do not match signed lineage".into(),
                ));
            }
        }

        let feedback_in_bundle = bundle.join(FEEDBACK_DIRECTORY).exists();
        if feedback_in_bundle != (manifest.feedback_bytes != AvailabilityStatus::Absent) {
            return Err(ShotLayoutError::Invalid(
                "portable feedback availability does not match carried bytes".into(),
            ));
        }
        if feedback_in_bundle {
            let missing = validate_feedback_storage(&bundle.join(FEEDBACK_DIRECTORY), &state)?;
            if missing != manifest.missing_attachment_digests {
                return Err(ShotLayoutError::Invalid(
                    "portable feedback omits attachment bytes not named by its manifest".into(),
                ));
            }
        } else if canonical_declared_attachment_digests(&state)
            != manifest.missing_attachment_digests
        {
            return Err(ShotLayoutError::Invalid(
                "portable manifest does not enumerate omitted canonical feedback attachments"
                    .into(),
            ));
        }

        let staging = create_staging_sibling(destination)?;
        let result = (|| {
            let staged_layout = ShotLayout::at(&staging);
            staged_layout.initialize_directories()?;
            staged_layout.append_lineage_batch(&lineage)?;
            if let Some(bytes) = &intention {
                staged_layout.preserve_exact_intention(bytes)?;
            }
            // Versions are derived record projections, not carried source.
            // Reproduce them from the verified lineage so an imported Shot
            // remains inspectable without claiming it can be materialized.
            for expression in state.expressions.values() {
                for version in &expression.versions {
                    staged_layout.write_version_record(state.shot_id, version)?;
                    if !feedback_in_bundle {
                        staged_layout.initialize_feedback_for(state.shot_id, version)?;
                    }
                }
            }
            if feedback_in_bundle {
                copy_bounded_tree(
                    &bundle.join(FEEDBACK_DIRECTORY),
                    &staging.join(FEEDBACK_DIRECTORY),
                    true,
                )?;
            }

            let copied_inventory = collect_imported_payload_inventory(
                &staged_layout,
                intention.is_some(),
                feedback_in_bundle,
            )?;
            if copied_inventory != manifest.files {
                return Err(ShotLayoutError::Invalid(
                    "imported bytes do not match the closed portable inventory".into(),
                ));
            }
            let copied_missing = if feedback_in_bundle {
                validate_feedback_storage(&staging.join(FEEDBACK_DIRECTORY), &state)?
            } else {
                Vec::new()
            };
            if copied_missing != manifest.missing_attachment_digests {
                return Err(ShotLayoutError::Invalid(
                    "imported feedback attachment availability differs from its manifest".into(),
                ));
            }
            staged_layout.write_metadata_json("import.json", &manifest, true)?;
            // Rehash once more after the final derived receipt and before the
            // atomic directory publication.
            if collect_imported_payload_inventory(
                &staged_layout,
                intention.is_some(),
                feedback_in_bundle,
            )? != manifest.files
            {
                return Err(ShotLayoutError::Invalid(
                    "imported payload changed before publication".into(),
                ));
            }
            publish_staged_directory(&staging, destination, true)?;
            Ok(ImportedShot {
                layout: ShotLayout::at(destination),
                manifest,
            })
        })();
        if result.is_err() {
            cleanup_staging_directory(&staging);
        }
        result
    }

    pub fn reset_evolutionary_intent(&self) -> Result<(), ShotLayoutError> {
        self.initialize_directories()?;
        write_replace_file(
            &self.root.join(EVOLUTIONARY_INTENT_DOCUMENT),
            EVOLUTIONARY_INTENT_TEMPLATE.as_bytes(),
            true,
        )
    }

    fn ensure_private_ignore_rules(&self) -> Result<(), ShotLayoutError> {
        const BEGIN: &str = "# BEGIN TOHSENO PRIVATE MATERIAL";
        const END: &str = "# END TOHSENO PRIVATE MATERIAL";
        let path = self.root.join(".gitignore");
        let existing = match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(ShotLayoutError::UnsafePath(path));
                }
                String::from_utf8(read_regular_limited(&path, MAX_JSON_BYTES)?)
                    .map_err(|_| ShotLayoutError::Invalid(".gitignore is not UTF-8".into()))?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        let begins = existing.match_indices(BEGIN).collect::<Vec<_>>();
        let ends = existing.match_indices(END).collect::<Vec<_>>();
        if !begins.is_empty() || !ends.is_empty() {
            if begins.len() != 1 || ends.len() != 1 || begins[0].0 >= ends[0].0 {
                return Err(ShotLayoutError::Invalid(
                    "the TOHSENO .gitignore block is incomplete or repeated".into(),
                ));
            }
            let start = begins[0].0;
            let mut end = ends[0].0 + END.len();
            if existing.as_bytes().get(end) == Some(&b'\n') {
                end += 1;
            }
            let mut updated = existing.clone();
            updated.replace_range(start..end, PRIVATE_IGNORE_BLOCK);
            if updated == existing {
                return Ok(());
            }
            return write_replace_file(&path, updated.as_bytes(), false);
        }
        let mut updated = existing;
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push('\n');
        }
        if !updated.is_empty() {
            updated.push('\n');
        }
        updated.push_str(PRIVATE_IGNORE_BLOCK);
        write_replace_file(&path, updated.as_bytes(), false)
    }
}

fn derived_shot_snapshot(
    actions: &[SignedLineageAction],
    state: &tohseno_protocol::lineage::ShotState,
) -> Result<DerivedShotSnapshot, ShotLayoutError> {
    let origin_action = actions
        .first()
        .ok_or_else(|| ShotLayoutError::Invalid("cannot derive an empty Shot".into()))?
        .commitment()?;
    let (current_genome_revision, current_genome_digest) = match &state.accepted_genome {
        Some(accepted) => (
            Some(accepted.genome.revision),
            Some(accepted.genome.digest()?),
        ),
        None => (None, None),
    };
    let expressions = state
        .expressions
        .iter()
        .map(|(expression_id, expression)| {
            Ok(DerivedExpressionHead {
                expression_id: *expression_id,
                current_version: expression.current_version,
                accepted_version_count: u64::try_from(expression.versions.len()).map_err(|_| {
                    ShotLayoutError::Limit("accepted version count overflowed".into())
                })?,
            })
        })
        .collect::<Result<Vec<_>, ShotLayoutError>>()?;
    let public_action_count = u64::try_from(
        actions
            .iter()
            .filter(|action| action.action.availability == AvailabilityStatus::PubliclyAvailable)
            .count(),
    )
    .map_err(|_| ShotLayoutError::Limit("public action count overflowed".into()))?;
    let private_action_count = u64::try_from(
        actions
            .iter()
            .filter(|action| action.action.availability == AvailabilityStatus::IntentionallyPrivate)
            .count(),
    )
    .map_err(|_| ShotLayoutError::Limit("private action count overflowed".into()))?;
    Ok(DerivedShotSnapshot {
        schema: "tohseno.local-shot-snapshot/2".into(),
        protocol_version: tohseno_protocol::lineage::LINEAGE_PROTOCOL_VERSION.into(),
        shot_id: state.shot_id,
        origin_action,
        controller: state.controller,
        current_genome_revision,
        current_genome_digest,
        expressions,
        lineage_sequence: state.sequence,
        lineage_head: state.head,
        public_action_count,
        private_action_count,
    })
}

fn embedded_metadata_path(root: &Path) -> Result<Option<PathBuf>, ShotLayoutError> {
    let candidates = [
        root.join("TOHSENO/embedded-provenance.json"),
        root.join("src/TOHSENO/embedded-provenance.json"),
    ];
    let mut found = None;
    for candidate in candidates {
        match fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(ShotLayoutError::UnsafePath(candidate))
            }
            Ok(_) if found.is_some() => {
                return Err(ShotLayoutError::Invalid(
                    "multiple embedded-provenance.json candidates are ambiguous".into(),
                ))
            }
            Ok(_) => found = Some(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(found)
}

fn read_embedded_metadata_v2(path: &Path) -> Result<AppMetadataV2, ShotLayoutError> {
    let bytes = read_regular_limited(path, MAX_JSON_BYTES)?;
    let decoded =
        tohseno_protocol::app_metadata::EmbeddedAppMetadata::decode_transport_json(&bytes)?;
    let metadata = match decoded {
        tohseno_protocol::app_metadata::EmbeddedAppMetadata::V2(metadata) => metadata,
        tohseno_protocol::app_metadata::EmbeddedAppMetadata::V1(_) => {
            return Err(ShotLayoutError::Invalid(
                "a v2 accepted Version requires embedded AppMetadataV2, not legacy v1 metadata"
                    .into(),
            ))
        }
    };
    Ok(metadata)
}

fn has_xcode_project(root: &Path) -> Result<bool, ShotLayoutError> {
    require_real_directory(root)?;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_string_lossy().ends_with(".xcodeproj") {
            let kind = entry.file_type()?;
            if kind.is_symlink() || !kind.is_dir() {
                return Err(ShotLayoutError::UnsafePath(entry.path()));
            }
            return Ok(true);
        }
    }
    Ok(false)
}

fn validate_metadata_filename(filename: &str) -> Result<(), ShotLayoutError> {
    let path = Path::new(filename);
    if filename.is_empty()
        || filename.len() > 128
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
        || !filename
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ShotLayoutError::Invalid(
            "metadata filename is not one safe component".into(),
        ));
    }
    Ok(())
}

fn validate_portable_file(file: &PortableFile) -> Result<(), ShotLayoutError> {
    let allowed_root = file.path == LINEAGE_FILE
        || file.path == INTENTION_DOCUMENT
        || file.path.starts_with("feedback/");
    if !allowed_root
        || file.path.is_empty()
        || file.path.len() > 4096
        || file.path.starts_with('/')
        || file.path.contains('\\')
        || file.path.chars().any(char::is_control)
        || file.path.split('/').any(|component| {
            component.is_empty()
                || component == "."
                || component == ".."
                || component.len() > 255
                || !component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
        || file.digest == Bytes32::ZERO
        || file.byte_length > tohseno_protocol::builder::MAX_SAFE_JSON_INTEGER
    {
        return Err(ShotLayoutError::Invalid(format!(
            "portable inventory path or digest is invalid: {}",
            file.path
        )));
    }
    Ok(())
}

fn encode_lineage(actions: &[SignedLineageAction]) -> Result<Vec<u8>, ShotLayoutError> {
    let mut output = Vec::new();
    for action in actions {
        let encoded = tohseno_protocol::canonical::to_vec(action)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        if encoded.len() > MAX_JSON_BYTES || encoded.contains(&b'\n') {
            return Err(ShotLayoutError::Limit(
                "lineage action is oversized or not one JSON line".into(),
            ));
        }
        let projected = output
            .len()
            .checked_add(encoded.len())
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| ShotLayoutError::Limit("lineage size overflowed".into()))?;
        if projected > MAX_LINEAGE_BYTES {
            return Err(ShotLayoutError::Limit(
                "lineage.jsonl exceeds 64 MiB".into(),
            ));
        }
        output.extend_from_slice(&encoded);
        output.push(b'\n');
    }
    Ok(output)
}

fn decode_lineage(bytes: &[u8]) -> Result<Vec<SignedLineageAction>, ShotLayoutError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if !bytes.ends_with(b"\n") {
        return Err(ShotLayoutError::Invalid(
            "lineage.jsonl must end at a complete line".into(),
        ));
    }
    let lines = bytes.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    let mut actions = Vec::with_capacity(lines.len().saturating_sub(1));
    for (index, line) in lines.iter().enumerate() {
        if line.is_empty() {
            if index + 1 == lines.len() {
                continue;
            }
            return Err(ShotLayoutError::Invalid(
                "lineage.jsonl contains an empty action".into(),
            ));
        }
        if line.len() > MAX_JSON_BYTES {
            return Err(ShotLayoutError::Limit(format!(
                "lineage action {} exceeds 4 MiB",
                index + 1
            )));
        }
        let action = serde_json::from_slice::<SignedLineageAction>(line)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        let canonical = tohseno_protocol::canonical::to_vec(&action)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        if canonical.as_slice() != *line {
            return Err(ShotLayoutError::Invalid(format!(
                "lineage action {} is not canonical JSON",
                index + 1
            )));
        }
        actions.push(action);
    }
    if !actions.is_empty() {
        reduce_lineage(&actions)?;
    }
    Ok(actions)
}

fn validate_evolution_prompt(prompt: &[u8]) -> Result<(), ShotLayoutError> {
    if prompt.is_empty() || prompt.len() > MAX_JSON_BYTES {
        return Err(ShotLayoutError::Limit(
            "pending evolutionary instruction must contain 1 byte..=4 MiB".into(),
        ));
    }
    std::str::from_utf8(prompt).map_err(|_| {
        ShotLayoutError::Invalid("pending evolutionary instruction must be UTF-8".into())
    })?;
    Ok(())
}

fn validate_feedback_actions(feedback_actions: &[Bytes32]) -> Result<(), ShotLayoutError> {
    if feedback_actions.len() > 256
        || feedback_actions.contains(&Bytes32::ZERO)
        || feedback_actions.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(ShotLayoutError::Invalid(
            "selected Feedback actions must be nonzero, unique, sorted commitments".into(),
        ));
    }
    Ok(())
}

fn validate_pending_evolution_selection(
    pending: &PendingEvolutionSelection,
    prompt: &[u8],
) -> Result<(), ShotLayoutError> {
    let schema_valid = match pending.schema.as_str() {
        PENDING_EVOLUTION_SELECTION_SCHEMA_V1 => pending.references.is_empty(),
        PENDING_EVOLUTION_SELECTION_SCHEMA_V2 => true,
        _ => false,
    };
    if !schema_valid
        || pending.prompt_digest != sha256(prompt)
        || (pending.feedback_actions.is_empty() && pending.references.is_empty())
    {
        return Err(ShotLayoutError::Invalid(
            "pending input selection does not bind this exact evolutionary instruction".into(),
        ));
    }
    validate_feedback_actions(&pending.feedback_actions)?;
    if pending.references.len() > MAX_PRIVATE_REFERENCES {
        return Err(ShotLayoutError::Limit(format!(
            "pending evolution exceeds {MAX_PRIVATE_REFERENCES} private references"
        )));
    }
    let mut names = BTreeSet::new();
    let mut previous_digest = None;
    for reference in &pending.references {
        validate_private_reference_availability(reference)?;
        if previous_digest.is_some_and(|previous| previous >= reference.artifact.digest) {
            return Err(ShotLayoutError::Invalid(
                "pending private references must be unique and digest-sorted".into(),
            ));
        }
        previous_digest = Some(reference.artifact.digest);
        let name = reference
            .artifact
            .name
            .as_deref()
            .expect("private reference validation requires a name");
        if !names.insert(name.to_ascii_lowercase()) {
            return Err(ShotLayoutError::Invalid(
                "pending private reference names collide on Apple filesystems".into(),
            ));
        }
    }
    Ok(())
}

fn validate_reference_name(name: &str) -> Result<(), ShotLayoutError> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 255
        || !name.is_ascii()
        || name.starts_with('.')
        || name.ends_with('.')
        || name.ends_with(' ')
        || bytes
            .iter()
            .any(|byte| byte.is_ascii_control() || matches!(*byte, b'/' | b'\\' | b':'))
        || !matches!(
            Path::new(name).components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        )
    {
        return Err(ShotLayoutError::Invalid(
            "private reference name is not one safe portable component".into(),
        ));
    }
    Ok(())
}

fn remove_prepared_reference_aliases(directory: &Path) -> Result<(), ShotLayoutError> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some((stem, extension)) = name.rsplit_once('.') else {
            continue;
        };
        let Some(ordinal) = stem.strip_prefix("image_") else {
            continue;
        };
        if ordinal.parse::<usize>().is_ok()
            && ["png", "jpg", "jpeg", "heic", "webp"]
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        {
            if file_type.is_symlink() || !file_type.is_file() {
                return Err(ShotLayoutError::UnsafePath(entry.path()));
            }
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn validate_image_bytes(extension: &str, bytes: &[u8]) -> Result<(), &'static str> {
    let valid = match extension {
        "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpg" | "jpeg" => bytes.starts_with(b"\xff\xd8\xff"),
        "webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        "heic" => {
            bytes.len() >= 12
                && &bytes[4..8] == b"ftyp"
                && matches!(
                    &bytes[8..12],
                    b"heic" | b"heix" | b"hevc" | b"hevx" | b"mif1" | b"msf1"
                )
        }
        _ => false,
    };
    valid
        .then_some(())
        .ok_or("file bytes do not match the declared supported image format")
}

fn media_type_for_extension(extension: Option<&str>) -> &'static str {
    match extension {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("heic") => "image/heic",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        Some("txt" | "md" | "markdown") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn private_reference_availability(
    attachment: &PrivateAttachment,
    name: String,
) -> Result<ArtifactAvailability, ShotLayoutError> {
    let availability = ArtifactAvailability {
        schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
        artifact: ArtifactDescriptor {
            digest: attachment.digest,
            media_type: media_type_for_extension(attachment.extension.as_deref()).into(),
            byte_length: attachment.byte_length,
            name: Some(name),
        },
        status: AvailabilityStatus::IntentionallyPrivate,
        locations: Vec::new(),
    };
    validate_private_reference_availability(&availability)?;
    Ok(availability)
}

fn validate_private_reference_availability(
    availability: &ArtifactAvailability,
) -> Result<(), ShotLayoutError> {
    availability.validate()?;
    if availability.status != AvailabilityStatus::IntentionallyPrivate
        || !availability.locations.is_empty()
    {
        return Err(ShotLayoutError::Invalid(
            "local reference availability must remain intentionally private without locations"
                .into(),
        ));
    }
    let name = availability.artifact.name.as_deref().ok_or_else(|| {
        ShotLayoutError::Invalid("private reference descriptor requires its original name".into())
    })?;
    validate_reference_name(name)?;
    let extension = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    if availability.artifact.media_type != media_type_for_extension(extension.as_deref()) {
        return Err(ShotLayoutError::Invalid(
            "private reference media type disagrees with its safe original name".into(),
        ));
    }
    if availability.artifact.byte_length > MAX_ATTACHMENT_BYTES as u64 {
        return Err(ShotLayoutError::Limit(
            "private reference exceeds 64 MiB".into(),
        ));
    }
    Ok(())
}

fn read_private_attachment(source: &Path) -> Result<PrivateAttachment, ShotLayoutError> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_ATTACHMENT_BYTES as u64
    {
        return Err(ShotLayoutError::Invalid(format!(
            "private attachment is not a bounded regular file: {}",
            source.display()
        )));
    }
    let bytes = read_regular_limited(source, MAX_ATTACHMENT_BYTES)?;
    let digest = sha256(&bytes);
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 16
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
        .map(str::to_ascii_lowercase);
    Ok(PrivateAttachment {
        source: source.into(),
        byte_length: u64::try_from(bytes.len())
            .map_err(|_| ShotLayoutError::Limit("attachment length overflowed".into()))?,
        bytes,
        digest,
        extension,
    })
}

fn store_private_attachment(
    attachment: &PrivateAttachment,
    destination: &Path,
) -> Result<PathBuf, ShotLayoutError> {
    let name = match &attachment.extension {
        Some(extension) => format!(
            "{}.{}",
            attachment.digest.to_string().trim_start_matches("0x"),
            extension
        ),
        None => attachment
            .digest
            .to_string()
            .trim_start_matches("0x")
            .into(),
    };
    let path = destination.join(name);
    ensure_exact_file(&path, &attachment.bytes, true).map_err(|error| match error {
        ShotLayoutError::ImmutableConflict(_) => ShotLayoutError::Invalid(format!(
            "content-addressed feedback attachment conflicts with {}",
            attachment.source.display()
        )),
        other => other,
    })?;
    Ok(path)
}

fn portable_file(path: &str, bytes: &[u8]) -> Result<PortableFile, ShotLayoutError> {
    let file = PortableFile {
        path: path.into(),
        digest: sha256(bytes),
        byte_length: u64::try_from(bytes.len())
            .map_err(|_| ShotLayoutError::Limit("portable file length overflowed".into()))?,
    };
    validate_portable_file(&file)?;
    Ok(file)
}

fn validate_feedback_inputs(
    shot_id: ShotId,
    version: &VersionRecord,
    record: &Feedback,
    attachments: &[PathBuf],
) -> Result<(), ShotLayoutError> {
    version.validate(shot_id)?;
    record.validate()?;
    if record.expression_id != version.expression_id
        || record.version_id != version.version_id
        || record.build_identity != version.build_identity
    {
        return Err(ShotLayoutError::Invalid(
            "feedback does not identify the exact accepted expression version and build".into(),
        ));
    }
    if attachments.len() > MAX_ATTACHMENTS {
        return Err(ShotLayoutError::Limit(format!(
            "feedback accepts at most {MAX_ATTACHMENTS} attachments"
        )));
    }
    let mut observed = BTreeSet::new();
    for source in attachments {
        let attachment = read_private_attachment(source)?;
        if !observed.insert(attachment.digest)
            || !record.attachments.iter().any(|candidate| {
                candidate.artifact.digest == attachment.digest
                    && candidate.artifact.byte_length == attachment.byte_length
                    && !matches!(
                        candidate.status,
                        AvailabilityStatus::Absent | AvailabilityStatus::Unknown
                    )
            })
        {
            return Err(ShotLayoutError::Invalid(format!(
                "feedback attachment {} is duplicate or not declared by digest and length",
                source.display()
            )));
        }
    }
    Ok(())
}

fn collect_portable_inventory(
    root: &Path,
    directory: &Path,
    prefix: &str,
    output: &mut Vec<PortableFile>,
) -> Result<(), ShotLayoutError> {
    require_real_directory(root)?;
    require_real_directory(directory)?;
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(ShotLayoutError::UnsafePath(entry.path()));
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| ShotLayoutError::UnsafePath(entry.path()))?
            .components()
            .map(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| ShotLayoutError::UnsafePath(entry.path()))
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("/");
        let portable_path = format!("{prefix}/{relative}");
        if file_type.is_dir() {
            collect_portable_inventory(root, &entry.path(), prefix, output)?;
        } else if file_type.is_file() {
            if output.len() >= 1026 {
                return Err(ShotLayoutError::Limit(
                    "portable inventory exceeds 1026 files".into(),
                ));
            }
            let metadata = entry.metadata()?;
            let maximum = usize::try_from(metadata.len())
                .map_err(|_| ShotLayoutError::Limit("portable file is too large".into()))?;
            if maximum > MAX_ATTACHMENT_BYTES {
                return Err(ShotLayoutError::Limit(format!(
                    "portable file exceeds 64 MiB: {portable_path}"
                )));
            }
            let bytes = read_regular_limited(&entry.path(), maximum)?;
            output.push(portable_file(&portable_path, &bytes)?);
        } else {
            return Err(ShotLayoutError::UnsafePath(entry.path()));
        }
    }
    Ok(())
}

fn collect_bundle_payload_inventory(bundle: &Path) -> Result<Vec<PortableFile>, ShotLayoutError> {
    require_real_directory(bundle)?;
    let lineage = read_regular_limited(&bundle.join(LINEAGE_FILE), MAX_LINEAGE_BYTES)?;
    let mut files = vec![portable_file(LINEAGE_FILE, &lineage)?];
    match fs::symlink_metadata(bundle.join(INTENTION_DOCUMENT)) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(ShotLayoutError::UnsafePath(bundle.join(INTENTION_DOCUMENT)))
        }
        Ok(_) => {
            let bytes = read_regular_limited(&bundle.join(INTENTION_DOCUMENT), MAX_JSON_BYTES)?;
            files.push(portable_file(INTENTION_DOCUMENT, &bytes)?);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    match fs::symlink_metadata(bundle.join(FEEDBACK_DIRECTORY)) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(ShotLayoutError::UnsafePath(bundle.join(FEEDBACK_DIRECTORY)))
        }
        Ok(_) => collect_portable_inventory(
            &bundle.join(FEEDBACK_DIRECTORY),
            &bundle.join(FEEDBACK_DIRECTORY),
            FEEDBACK_DIRECTORY,
            &mut files,
        )?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    files.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    Ok(files)
}

fn collect_imported_payload_inventory(
    layout: &ShotLayout,
    carries_intention: bool,
    carries_feedback: bool,
) -> Result<Vec<PortableFile>, ShotLayoutError> {
    let lineage = read_regular_limited(&layout.lineage_path(), MAX_LINEAGE_BYTES)?;
    let mut files = vec![portable_file(LINEAGE_FILE, &lineage)?];
    if carries_intention {
        let bytes = read_regular_limited(&layout.root.join(INTENTION_DOCUMENT), MAX_JSON_BYTES)?;
        files.push(portable_file(INTENTION_DOCUMENT, &bytes)?);
    }
    if carries_feedback {
        collect_portable_inventory(
            &layout.root.join(FEEDBACK_DIRECTORY),
            &layout.root.join(FEEDBACK_DIRECTORY),
            FEEDBACK_DIRECTORY,
            &mut files,
        )?;
    }
    files.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    Ok(files)
}

fn read_canonical_json<T>(path: &Path) -> Result<T, ShotLayoutError>
where
    T: serde::de::DeserializeOwned + Serialize,
{
    let bytes = read_regular_limited(path, MAX_JSON_BYTES)?;
    let value = serde_json::from_slice::<T>(&bytes)
        .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
    let mut canonical = tohseno_protocol::canonical::to_vec(&value)
        .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
    canonical.push(b'\n');
    if bytes != canonical {
        return Err(ShotLayoutError::Invalid(format!(
            "{} is not canonical JSON",
            path.display()
        )));
    }
    Ok(value)
}

fn canonical_declared_attachment_digests(
    state: &tohseno_protocol::lineage::ShotState,
) -> Vec<Bytes32> {
    state
        .feedback
        .values()
        .flat_map(|feedback| &feedback.attachments)
        .filter(|attachment| {
            !matches!(
                attachment.status,
                AvailabilityStatus::Absent | AvailabilityStatus::Unknown
            )
        })
        .map(|attachment| attachment.artifact.digest)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn validate_feedback_storage(
    feedback_root: &Path,
    state: &tohseno_protocol::lineage::ShotState,
) -> Result<Vec<Bytes32>, ShotLayoutError> {
    require_real_directory(feedback_root)?;
    let top = fs::read_dir(feedback_root)?.collect::<Result<Vec<_>, _>>()?;
    if top.len() != 1
        || top[0].file_name() != "versions"
        || top[0].file_type()?.is_symlink()
        || !top[0].file_type()?.is_dir()
    {
        return Err(ShotLayoutError::Invalid(
            "feedback storage must contain only versions/".into(),
        ));
    }
    let versions_root = feedback_root.join("versions");
    require_real_directory(&versions_root)?;
    let known_expressions = state
        .expressions
        .keys()
        .map(|expression_id| (expression_component(*expression_id), *expression_id))
        .collect::<BTreeMap<_, _>>();
    let mut seen_versions = BTreeSet::new();
    let mut missing_attachments = canonical_declared_attachment_digests(state)
        .into_iter()
        .collect::<BTreeSet<_>>();
    for version_entry in fs::read_dir(&versions_root)? {
        let version_entry = version_entry?;
        let version_kind = version_entry.file_type()?;
        if version_kind.is_symlink() || !version_kind.is_dir() {
            return Err(ShotLayoutError::UnsafePath(version_entry.path()));
        }
        let name = version_entry
            .file_name()
            .into_string()
            .map_err(|_| ShotLayoutError::Invalid("feedback version name is not UTF-8".into()))?;
        if let Some(expression_id) = known_expressions.get(&name).copied() {
            let mut ordinal_count = 0_usize;
            for ordinal_entry in fs::read_dir(version_entry.path())? {
                let ordinal_entry = ordinal_entry?;
                let kind = ordinal_entry.file_type()?;
                if kind.is_symlink() || !kind.is_dir() {
                    return Err(ShotLayoutError::UnsafePath(ordinal_entry.path()));
                }
                let ordinal_name = ordinal_entry.file_name().into_string().map_err(|_| {
                    ShotLayoutError::Invalid("feedback version ordinal is not UTF-8".into())
                })?;
                let ordinal = parse_version_ordinal(&ordinal_name)?;
                validate_feedback_version_directory(
                    &ordinal_entry.path(),
                    state,
                    Some(expression_id),
                    ordinal,
                    false,
                    &mut seen_versions,
                    &mut missing_attachments,
                )?;
                ordinal_count += 1;
            }
            if ordinal_count == 0 {
                return Err(ShotLayoutError::Invalid(format!(
                    "feedback expression directory is empty: {name}"
                )));
            }
            continue;
        }

        // Compatibility adapter for pre-v2 single-expression storage:
        // feedback/versions/0001. The index still resolves the exact
        // (ExpressionID, VersionID), so importing it never guesses by ordinal.
        let ordinal = parse_version_ordinal(&name)?;
        validate_feedback_version_directory(
            &version_entry.path(),
            state,
            None,
            ordinal,
            true,
            &mut seen_versions,
            &mut missing_attachments,
        )?;
    }
    Ok(missing_attachments.into_iter().collect())
}

#[allow(clippy::too_many_arguments)]
fn validate_feedback_version_directory(
    directory: &Path,
    state: &tohseno_protocol::lineage::ShotState,
    path_expression_id: Option<ExpressionId>,
    ordinal: u64,
    legacy_path: bool,
    seen_versions: &mut BTreeSet<(ExpressionId, VersionId)>,
    missing_attachments: &mut BTreeSet<Bytes32>,
) -> Result<(), ShotLayoutError> {
    let index = read_canonical_json::<FeedbackVersionIndex>(&directory.join("index.json"))?;
    let expected_schema = if legacy_path {
        FEEDBACK_INDEX_SCHEMA_V1
    } else {
        FEEDBACK_INDEX_SCHEMA_V2
    };
    if index.schema != expected_schema
        || path_expression_id.is_some_and(|value| value != index.expression_id)
    {
        return Err(ShotLayoutError::Invalid(
            "feedback index schema or expression path is inconsistent".into(),
        ));
    }
    let version = state
        .expressions
        .get(&index.expression_id)
        .and_then(|expression| {
            expression.versions.iter().find(|version| {
                version.ordinal == ordinal && version.version_id == index.version_id
            })
        })
        .ok_or_else(|| {
            ShotLayoutError::Invalid(format!(
                "feedback index names an unknown exact accepted version: {ordinal:04}"
            ))
        })?;
    if !seen_versions.insert((index.expression_id, index.version_id)) {
        return Err(ShotLayoutError::Invalid(
            "feedback storage repeats an exact expression version".into(),
        ));
    }

    let mut feedback_count = 0_usize;
    for item in fs::read_dir(directory)? {
        let item = item?;
        if item.file_name() == "index.json" {
            if item.file_type()?.is_symlink() || !item.file_type()?.is_file() {
                return Err(ShotLayoutError::UnsafePath(item.path()));
            }
            continue;
        }
        let kind = item.file_type()?;
        if kind.is_symlink() || !kind.is_dir() {
            return Err(ShotLayoutError::UnsafePath(item.path()));
        }
        feedback_count += 1;
        let feedback_name = item
            .file_name()
            .into_string()
            .map_err(|_| ShotLayoutError::Invalid("feedback identifier is not UTF-8".into()))?;
        if feedback_name.len() != 64
            || !feedback_name
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ShotLayoutError::Invalid(
                "feedback directory is not a content digest".into(),
            ));
        }
        let feedback_directory = item.path();
        let entries = fs::read_dir(&feedback_directory)?.collect::<Result<Vec<_>, _>>()?;
        if entries.len() != 2 {
            return Err(ShotLayoutError::Invalid(format!(
                "feedback entry must contain exactly feedback.json and attachments/: {feedback_name}"
            )));
        }
        for entry in &entries {
            let entry_name = entry.file_name();
            let kind = entry.file_type()?;
            let valid = match entry_name.to_str() {
                Some("feedback.json") => kind.is_file(),
                Some("attachments") => kind.is_dir(),
                _ => false,
            };
            if kind.is_symlink() || !valid {
                return Err(ShotLayoutError::UnsafePath(entry.path()));
            }
        }
        let feedback = read_canonical_json::<Feedback>(&feedback_directory.join("feedback.json"))?;
        feedback.validate()?;
        let digest = tohseno_protocol::canonical::sha256_commitment(&feedback)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        if digest.to_string().trim_start_matches("0x") != feedback_name
            || feedback.expression_id != version.expression_id
            || feedback.version_id != version.version_id
            || feedback.build_identity != version.build_identity
            || !state
                .feedback
                .values()
                .any(|canonical| canonical == &feedback)
        {
            return Err(ShotLayoutError::Invalid(format!(
                "private feedback is not bound to a canonical exact-version action: {feedback_name}"
            )));
        }

        let mut attachment_digests = BTreeSet::new();
        let attachment_root = feedback_directory.join("attachments");
        require_real_directory(&attachment_root)?;
        for attachment in fs::read_dir(&attachment_root)? {
            let attachment = attachment?;
            let kind = attachment.file_type()?;
            if kind.is_symlink() || !kind.is_file() {
                return Err(ShotLayoutError::UnsafePath(attachment.path()));
            }
            let metadata = attachment.metadata()?;
            let maximum = usize::try_from(metadata.len())
                .map_err(|_| ShotLayoutError::Limit("feedback attachment is too large".into()))?;
            if maximum > MAX_ATTACHMENT_BYTES {
                return Err(ShotLayoutError::Limit(
                    "feedback attachment exceeds 64 MiB".into(),
                ));
            }
            let bytes = read_regular_limited(&attachment.path(), maximum)?;
            let digest = sha256(&bytes);
            let name = attachment
                .file_name()
                .into_string()
                .map_err(|_| ShotLayoutError::Invalid("attachment name is not UTF-8".into()))?;
            let mut components = name.split('.');
            let stem = components.next();
            let extension = components.next();
            if components.next().is_some()
                || stem != Some(digest.to_string().trim_start_matches("0x"))
                || extension.is_some_and(|value| {
                    value.is_empty()
                        || value.len() > 16
                        || !value.bytes().all(|byte| byte.is_ascii_alphanumeric())
                        || value.bytes().any(|byte| byte.is_ascii_uppercase())
                })
                || !attachment_digests.insert(digest)
                || !feedback.attachments.iter().any(|declared| {
                    declared.artifact.digest == digest
                        && declared.artifact.byte_length == metadata.len()
                        && !matches!(
                            declared.status,
                            AvailabilityStatus::Absent | AvailabilityStatus::Unknown
                        )
                })
            {
                return Err(ShotLayoutError::Invalid(
                    "feedback attachment is not committed by its canonical record".into(),
                ));
            }
            missing_attachments.remove(&digest);
        }
    }
    let expected_status = if feedback_count == 0 {
        FeedbackPresence::Absent
    } else {
        FeedbackPresence::Present
    };
    if index.status != expected_status {
        return Err(ShotLayoutError::Invalid(
            "feedback index presence status is incorrect".into(),
        ));
    }
    Ok(())
}

fn parse_version_ordinal(name: &str) -> Result<u64, ShotLayoutError> {
    let ordinal = name.parse::<u64>().map_err(|_| {
        ShotLayoutError::Invalid(format!("feedback version directory is invalid: {name}"))
    })?;
    if ordinal == 0 || format!("{ordinal:04}") != name {
        return Err(ShotLayoutError::Invalid(format!(
            "feedback version directory is not canonical: {name}"
        )));
    }
    Ok(ordinal)
}

fn expression_component(expression_id: ExpressionId) -> String {
    expression_id
        .to_string()
        .trim_start_matches("0x")
        .to_owned()
}

fn create_new_directory(path: &Path, private: bool) -> Result<(), ShotLayoutError> {
    let parent = path
        .parent()
        .ok_or_else(|| ShotLayoutError::UnsafePath(path.into()))?;
    require_real_directory(parent)?;
    match fs::symlink_metadata(path) {
        Ok(_) => return Err(ShotLayoutError::ImmutableConflict(path.into())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut builder = fs::DirBuilder::new();
    set_directory_mode(&mut builder, private);
    builder.create(path)?;
    require_real_directory(path)?;
    set_permissions(path, private, true)
}

fn create_staging_sibling(destination: &Path) -> Result<PathBuf, ShotLayoutError> {
    let parent = destination
        .parent()
        .ok_or_else(|| ShotLayoutError::UnsafePath(destination.into()))?;
    require_real_directory(parent)?;
    if destination
        .file_name()
        .and_then(|value| value.to_str())
        .is_none()
    {
        return Err(ShotLayoutError::UnsafePath(destination.into()));
    }
    match fs::symlink_metadata(destination) {
        Ok(_) => return Err(ShotLayoutError::ImmutableConflict(destination.into())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    for ordinal in 1_u32.. {
        let staging = parent.join(format!(".tohseno-stage-{}-{ordinal}", std::process::id()));
        match create_new_directory(&staging, true) {
            Ok(()) => return Ok(staging),
            Err(ShotLayoutError::ImmutableConflict(_)) => continue,
            Err(error) => return Err(error),
        }
    }
    unreachable!()
}

fn publish_staged_directory(
    staging: &Path,
    destination: &Path,
    private: bool,
) -> Result<(), ShotLayoutError> {
    require_real_directory(staging)?;
    let staging_parent = staging
        .parent()
        .ok_or_else(|| ShotLayoutError::UnsafePath(staging.into()))?;
    let destination_parent = destination
        .parent()
        .ok_or_else(|| ShotLayoutError::UnsafePath(destination.into()))?;
    if staging_parent != destination_parent {
        return Err(ShotLayoutError::UnsafePath(destination.into()));
    }
    match fs::symlink_metadata(destination) {
        Ok(_) => return Err(ShotLayoutError::ImmutableConflict(destination.into())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    set_permissions(staging, private, true)?;
    fs::rename(staging, destination)?;
    require_real_directory(destination)?;
    File::open(destination_parent)?.sync_all()?;
    Ok(())
}

fn cleanup_staging_directory(staging: &Path) {
    let safe_name = staging
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.starts_with(".tohseno-stage-"));
    if !safe_name {
        return;
    }
    if fs::symlink_metadata(staging)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        let _ = fs::remove_dir_all(staging);
    }
}

fn directory_has_regular_files(path: &Path) -> Result<bool, ShotLayoutError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(ShotLayoutError::UnsafePath(path.into()))
        }
        Ok(_) => {}
    }
    for item in fs::read_dir(path)? {
        let item = item?;
        let file_type = item.file_type()?;
        if file_type.is_symlink() {
            return Err(ShotLayoutError::UnsafePath(item.path()));
        }
        if file_type.is_file() || (file_type.is_dir() && directory_has_regular_files(&item.path())?)
        {
            return Ok(true);
        }
        if !file_type.is_dir() {
            return Err(ShotLayoutError::UnsafePath(item.path()));
        }
    }
    Ok(false)
}

fn validate_bundle_inventory(bundle: &Path) -> Result<(), ShotLayoutError> {
    for item in fs::read_dir(bundle)? {
        let item = item?;
        let name = item.file_name().into_string().map_err(|_| {
            ShotLayoutError::Invalid("portable bundle contains a non-UTF-8 name".into())
        })?;
        let file_type = item.file_type()?;
        if file_type.is_symlink() {
            return Err(ShotLayoutError::UnsafePath(item.path()));
        }
        let valid = match name.as_str() {
            PORTABLE_MANIFEST_FILE | LINEAGE_FILE | INTENTION_DOCUMENT => file_type.is_file(),
            FEEDBACK_DIRECTORY => file_type.is_dir(),
            _ => false,
        };
        if !valid {
            return Err(ShotLayoutError::Invalid(format!(
                "portable bundle contains an unrecognized entry: {name}"
            )));
        }
    }
    for required in [PORTABLE_MANIFEST_FILE, LINEAGE_FILE] {
        let path = bundle.join(required);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ShotLayoutError::UnsafePath(path));
        }
    }
    Ok(())
}

fn copy_bounded_tree(
    source: &Path,
    destination: &Path,
    private: bool,
) -> Result<(), ShotLayoutError> {
    const MAX_FILES: usize = 1024;
    const MAX_BYTES: u64 = 256 * 1024 * 1024;

    fn copy_inner(
        source: &Path,
        destination: &Path,
        private: bool,
        files: &mut usize,
        bytes: &mut u64,
    ) -> Result<(), ShotLayoutError> {
        require_real_directory(source)?;
        ensure_directory(destination, private)?;
        let mut entries = fs::read_dir(source)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let name = entry.file_name();
            let name_text = name.to_str().ok_or_else(|| {
                ShotLayoutError::Invalid("portable feedback contains a non-UTF-8 name".into())
            })?;
            if name_text.is_empty()
                || name_text == "."
                || name_text == ".."
                || name_text.chars().any(char::is_control)
            {
                return Err(ShotLayoutError::UnsafePath(entry.path()));
            }
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Err(ShotLayoutError::UnsafePath(entry.path()));
            }
            let target = destination.join(&name);
            if kind.is_dir() {
                copy_inner(&entry.path(), &target, private, files, bytes)?;
            } else if kind.is_file() {
                *files = files
                    .checked_add(1)
                    .ok_or_else(|| ShotLayoutError::Limit("file count overflowed".into()))?;
                let metadata = entry.metadata()?;
                *bytes = bytes
                    .checked_add(metadata.len())
                    .ok_or_else(|| ShotLayoutError::Limit("byte count overflowed".into()))?;
                if *files > MAX_FILES || *bytes > MAX_BYTES {
                    return Err(ShotLayoutError::Limit(
                        "portable private feedback exceeds 1024 files or 256 MiB".into(),
                    ));
                }
                let maximum = usize::try_from(metadata.len()).map_err(|_| {
                    ShotLayoutError::Limit("feedback file is too large to copy".into())
                })?;
                let contents = read_regular_limited(&entry.path(), maximum)?;
                ensure_exact_file(&target, &contents, private)?;
            } else {
                return Err(ShotLayoutError::UnsafePath(entry.path()));
            }
        }
        Ok(())
    }

    let mut files = 0;
    let mut bytes = 0;
    copy_inner(source, destination, private, &mut files, &mut bytes)
}

fn ensure_directory(path: &Path, private: bool) -> Result<(), ShotLayoutError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(ShotLayoutError::UnsafePath(path.into()))
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            set_directory_mode(&mut builder, private);
            builder.create(path)?;
        }
        Err(error) => return Err(error.into()),
    }
    set_permissions(path, private, true)?;
    Ok(())
}

fn require_real_directory(path: &Path) -> Result<(), ShotLayoutError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ShotLayoutError::UnsafePath(path.into()));
    }
    Ok(())
}

fn ensure_file(path: &Path, bytes: &[u8], private: bool) -> Result<(), ShotLayoutError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(ShotLayoutError::UnsafePath(path.into()))
        }
        Ok(_) => {
            set_permissions(path, private, false)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            write_new_file(path, bytes, private)
        }
        Err(error) => Err(error.into()),
    }
}

fn ensure_exact_file(path: &Path, bytes: &[u8], private: bool) -> Result<(), ShotLayoutError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(ShotLayoutError::UnsafePath(path.into()))
        }
        Ok(_) => {
            let existing = read_regular_limited(path, bytes.len().max(MAX_JSON_BYTES))?;
            if existing != bytes {
                return Err(ShotLayoutError::ImmutableConflict(path.into()));
            }
            set_permissions(path, private, false)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            write_new_file(path, bytes, private)
        }
        Err(error) => Err(error.into()),
    }
}

fn remove_regular_file_if_present(path: &Path) -> Result<(), ShotLayoutError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(ShotLayoutError::UnsafePath(path.into()))
        }
        Ok(_) => {
            fs::remove_file(path)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn write_new_file(path: &Path, bytes: &[u8], private: bool) -> Result<(), ShotLayoutError> {
    let parent = path
        .parent()
        .ok_or_else(|| ShotLayoutError::UnsafePath(path.into()))?;
    require_real_directory(parent)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_file_options(&mut options, private);
    let mut file = options.open(path)?;
    set_open_file_permissions(&file, private)?;
    verify_regular_file(path, &file, private)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    verify_regular_file(path, &file, private)?;
    Ok(())
}

fn write_replace_file(path: &Path, bytes: &[u8], private: bool) -> Result<(), ShotLayoutError> {
    let parent = path
        .parent()
        .ok_or_else(|| ShotLayoutError::UnsafePath(path.into()))?;
    require_real_directory(parent)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(ShotLayoutError::UnsafePath(path.into()))
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    for ordinal in 1_u32.. {
        let temporary = parent.join(format!(
            ".{}.tohseno-tmp-{}-{ordinal}",
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("metadata"),
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        set_file_options(&mut options, private);
        match options.open(&temporary) {
            Ok(mut file) => {
                set_open_file_permissions(&file, private)?;
                verify_regular_file(&temporary, &file, private)?;
                file.write_all(bytes)?;
                file.sync_all()?;
                fs::rename(&temporary, path)?;
                set_permissions(path, private, false)?;
                File::open(parent)?.sync_all()?;
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    unreachable!()
}

fn read_regular_limited(path: &Path, limit: usize) -> Result<Vec<u8>, ShotLayoutError> {
    let path_metadata = fs::symlink_metadata(path)?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.is_file()
        || path_metadata.len() > limit as u64
    {
        return Err(ShotLayoutError::UnsafePath(path.into()));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(path)?;
    let open_metadata = file.metadata()?;
    if !same_file(&path_metadata, &open_metadata) {
        return Err(ShotLayoutError::UnsafePath(path.into()));
    }
    let mut output = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut output)?;
    if output.len() > limit {
        return Err(ShotLayoutError::Limit(format!(
            "{} exceeds its read limit",
            path.display()
        )));
    }
    let final_metadata = fs::symlink_metadata(path)?;
    if !same_file(&open_metadata, &final_metadata) {
        return Err(ShotLayoutError::UnsafePath(path.into()));
    }
    Ok(output)
}

fn verify_regular_file(path: &Path, file: &File, private: bool) -> Result<(), ShotLayoutError> {
    let open = file.metadata()?;
    let current = fs::symlink_metadata(path)?;
    if !open.is_file()
        || current.file_type().is_symlink()
        || !current.is_file()
        || !same_file(&open, &current)
    {
        return Err(ShotLayoutError::UnsafePath(path.into()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let expected = if private {
            PRIVATE_FILE_MODE
        } else {
            PUBLIC_FILE_MODE
        };
        if open.nlink() != 1
            || open.uid() != unsafe { libc::geteuid() }
            || open.permissions().mode() & 0o777 != expected
        {
            return Err(ShotLayoutError::UnsafePath(path.into()));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}

#[cfg(not(unix))]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.created().ok() == right.created().ok()
}

#[cfg(unix)]
fn set_file_options(options: &mut OpenOptions, private: bool) {
    use std::os::unix::fs::OpenOptionsExt;
    options
        .mode(if private {
            PRIVATE_FILE_MODE
        } else {
            PUBLIC_FILE_MODE
        })
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
}

#[cfg(unix)]
fn set_open_file_permissions(file: &File, private: bool) -> Result<(), ShotLayoutError> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(if private {
        PRIVATE_FILE_MODE
    } else {
        PUBLIC_FILE_MODE
    }))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_open_file_permissions(_file: &File, _private: bool) -> Result<(), ShotLayoutError> {
    Ok(())
}

#[cfg(not(unix))]
fn set_file_options(_options: &mut OpenOptions, _private: bool) {}

#[cfg(unix)]
fn set_directory_mode(builder: &mut fs::DirBuilder, private: bool) {
    use std::os::unix::fs::DirBuilderExt;
    builder.mode(if private {
        PRIVATE_DIRECTORY_MODE
    } else {
        PUBLIC_DIRECTORY_MODE
    });
}

#[cfg(not(unix))]
fn set_directory_mode(_builder: &mut fs::DirBuilder, _private: bool) {}

fn set_permissions(path: &Path, private: bool, directory: bool) -> Result<(), ShotLayoutError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = match (private, directory) {
            (true, true) => PRIVATE_DIRECTORY_MODE,
            (true, false) => PRIVATE_FILE_MODE,
            (false, true) => PUBLIC_DIRECTORY_MODE,
            (false, false) => PUBLIC_FILE_MODE,
        };
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = (path, private, directory);
    Ok(())
}

#[derive(Debug)]
pub enum ShotLayoutError {
    Io(std::io::Error),
    Encoding(String),
    Invalid(String),
    Limit(String),
    UnsafePath(PathBuf),
    ImmutableConflict(PathBuf),
    GenomeDrift,
}

impl std::fmt::Display for ShotLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Encoding(reason) => write!(formatter, "Shot metadata encoding failed: {reason}"),
            Self::Invalid(reason) => write!(formatter, "Shot layout is invalid: {reason}"),
            Self::Limit(reason) => write!(formatter, "Shot layout limit exceeded: {reason}"),
            Self::UnsafePath(path) => {
                write!(formatter, "Shot layout path is unsafe: {}", path.display())
            }
            Self::ImmutableConflict(path) => write!(
                formatter,
                "immutable Shot material already differs: {}",
                path.display()
            ),
            Self::GenomeDrift => {
                formatter.write_str("GENOME.md differs from its accepted machine-readable revision")
            }
        }
    }
}

impl std::error::Error for ShotLayoutError {}

impl From<std::io::Error> for ShotLayoutError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<tohseno_protocol::ProtocolError> for ShotLayoutError {
    fn from(value: tohseno_protocol::ProtocolError) -> Self {
        Self::Invalid(format!("protocol record failed validation: {value}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::hazmat::PrehashSigner;
    use p256::ecdsa::{Signature, SigningKey};
    use tohseno_protocol::app_metadata::{AppMetadataDistribution, AppMetadataV2};
    use tohseno_protocol::digest::Address20;
    use tohseno_protocol::fascia::{AppleSurface, DistributionState};
    use tohseno_protocol::identity::BuilderId;
    use tohseno_protocol::ontology::{
        ArtifactAvailability, ArtifactDescriptor, ChangeScope, CommitmentOrigin, DesiredChange,
        EvolutionaryIntent, GenomeAcceptance, GenomeProposal, MaterializationProvenance,
        OriginalMaterial, TokenAssociation, TokenAssociationOperation, VerificationGate,
        Visibility, ARTIFACT_AVAILABILITY_SCHEMA, EVOLUTIONARY_INTENT_SCHEMA, EVOLUTION_SCHEMA,
        EXPRESSION_SCHEMA, FEEDBACK_SCHEMA, GENOME_SCHEMA, SHOT_COMMITMENT_SCHEMA,
        TOKEN_ASSOCIATION_SCHEMA, VERIFICATION_RESULT_SCHEMA, VERSION_SCHEMA,
    };
    use tohseno_protocol::record::{
        CanonicalTimestamp, FactoryDescriptor, ShotRecord, APPLE_FASCIA_ID, PROTOCOL_NAME,
        SHOT_SCHEMA,
    };
    use tohseno_protocol::signature::{
        P256PublicKey, P256Signature, SignatureAlgorithm, SignatureSidecar,
    };
    use tohseno_protocol::{
        Expression, Genome as ShotGenome, IntentionRecord, LineageAction, LineagePayload,
        ShotCommitment, VerificationResult,
    };

    fn timestamp() -> CanonicalTimestamp {
        CanonicalTimestamp::parse("2026-07-29T00:00:00Z").unwrap()
    }

    fn signer(key: &SigningKey) -> P256PublicKey {
        let point = key.verifying_key().to_encoded_point(false);
        let mut x = [0_u8; 32];
        let mut y = [0_u8; 32];
        x.copy_from_slice(point.x().unwrap());
        y.copy_from_slice(point.y().unwrap());
        P256PublicKey {
            x: Bytes32::new(x),
            y: Bytes32::new(y),
        }
    }

    fn sign(action: LineageAction, key: &SigningKey) -> SignedLineageAction {
        let digest = action.signing_digest().unwrap();
        let signature: Signature = key.sign_prehash(digest.as_bytes()).unwrap();
        let signature = signature.normalize_s().unwrap_or(signature);
        let bytes = signature.to_bytes();
        let mut r = [0_u8; 32];
        let mut s = [0_u8; 32];
        r.copy_from_slice(&bytes[..32]);
        s.copy_from_slice(&bytes[32..]);
        action
            .attach_signature(
                signer(key),
                P256Signature {
                    r: Bytes32::new(r),
                    s: Bytes32::new(s),
                },
            )
            .unwrap()
    }

    fn sign_v1_record(record: &ShotRecord, key: &SigningKey) -> SignatureSidecar {
        let digest = record.commitment().unwrap();
        let signature: Signature = key.sign_prehash(digest.as_bytes()).unwrap();
        let signature = signature.normalize_s().unwrap_or(signature);
        let bytes = signature.to_bytes();
        let mut r = [0_u8; 32];
        let mut s = [0_u8; 32];
        r.copy_from_slice(&bytes[..32]);
        s.copy_from_slice(&bytes[32..]);
        SignatureSidecar {
            schema: SignatureSidecar::SCHEMA.into(),
            algorithm: SignatureAlgorithm::P256,
            digest,
            public_key: signer(key),
            signature: P256Signature {
                r: Bytes32::new(r),
                s: Bytes32::new(s),
            },
            low_s: true,
        }
    }

    fn signed_action(
        sequence: u64,
        previous: Option<Bytes32>,
        shot_id: ShotId,
        controller: BuilderId,
        payload: LineagePayload,
        key: &SigningKey,
    ) -> SignedLineageAction {
        signed_action_with_availability(
            sequence,
            previous,
            shot_id,
            controller,
            AvailabilityStatus::IntentionallyPrivate,
            payload,
            key,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn signed_action_with_availability(
        sequence: u64,
        previous: Option<Bytes32>,
        shot_id: ShotId,
        controller: BuilderId,
        availability: AvailabilityStatus,
        payload: LineagePayload,
        key: &SigningKey,
    ) -> SignedLineageAction {
        sign(
            LineageAction::new(
                sequence,
                previous,
                shot_id,
                controller,
                timestamp(),
                availability,
                payload,
            )
            .unwrap(),
            key,
        )
    }

    fn initial_lineage(
        layout: &ShotLayout,
    ) -> (
        SigningKey,
        BuilderId,
        ShotId,
        ExpressionId,
        ShotGenome,
        Bytes32,
        u64,
    ) {
        let key = SigningKey::from_bytes((&[1_u8; 32]).into()).unwrap();
        let public_key = signer(&key);
        let controller = BuilderId::new(Address20::from_bytes([0x22; 20]));
        let shot_id = ShotId::from_bytes([0x11; 32]);
        let expression_id = ExpressionId::from_bytes([0x33; 32]);
        let material = OriginalMaterial {
            artifact: ArtifactAvailability {
                schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
                artifact: ArtifactDescriptor {
                    digest: sha256(b"exact intention"),
                    media_type: "text/plain; charset=utf-8".into(),
                    byte_length: 15,
                    name: Some("INTENTION.md".into()),
                },
                status: AvailabilityStatus::IntentionallyPrivate,
                locations: Vec::new(),
            },
            inline_text: Some("exact intention".into()),
        };
        let intention = IntentionRecord::new(vec![material], timestamp());
        let intention_digest = intention.commitment().unwrap();
        let commitment = ShotCommitment {
            schema: SHOT_COMMITMENT_SCHEMA.into(),
            intention_commitment: intention_digest,
            initial_controller: controller,
            initial_controller_key: public_key,
            origin: CommitmentOrigin::Native,
            committed_at: timestamp(),
        };
        let first = signed_action(
            1,
            None,
            shot_id,
            controller,
            LineagePayload::Commitment(commitment),
            &key,
        );
        let first_head = first.commitment().unwrap();
        let second = signed_action(
            2,
            Some(first_head),
            shot_id,
            controller,
            LineagePayload::Intention(intention),
            &key,
        );
        let second_head = second.commitment().unwrap();
        let genome = ShotGenome {
            schema: GENOME_SCHEMA.into(),
            revision: 1,
            purpose: "Keep one calm private note alive.".into(),
            intended_for: vec!["One owner".into()],
            essential_experience: vec!["Immediate quiet writing".into()],
            behavioral_invariants: vec!["The note remains available offline".into()],
            interaction_laws: Vec::new(),
            aesthetic_principles: Vec::new(),
            privacy_principles: vec!["No note leaves the device".into()],
            ownership_principles: vec!["Only the owner accepts continuity changes".into()],
            platform_commitments: vec!["Native iPhone application".into()],
            boundaries: Vec::new(),
            non_goals: Vec::new(),
            required_capabilities: vec!["local_storage".into()],
            forbidden_transformations: vec!["Do not add tracking".into()],
            acceptance_principles: vec!["The Release build and privacy gates pass".into()],
            freely_changeable: vec!["Typography may evolve".into()],
        };
        let proposal = GenomeProposal::initial(genome.clone(), "Initial interpretation".into());
        let third = signed_action(
            3,
            Some(second_head),
            shot_id,
            controller,
            LineagePayload::GenomeProposal(proposal),
            &key,
        );
        let third_head = third.commitment().unwrap();
        let acceptance = GenomeAcceptance {
            schema: tohseno_protocol::ontology::GENOME_ACCEPTANCE_SCHEMA.into(),
            proposal_action: third_head,
            revision: 1,
            genome_digest: genome.digest().unwrap(),
            accepted_at: timestamp(),
        };
        let fourth = signed_action(
            4,
            Some(third_head),
            shot_id,
            controller,
            LineagePayload::GenomeAcceptance(acceptance),
            &key,
        );
        let fourth_head = fourth.commitment().unwrap();
        let expression = Expression {
            schema: EXPRESSION_SCHEMA.into(),
            expression_id,
            kind: "native_apple_application".into(),
            name: "LifecycleShot".into(),
            platforms: vec!["iphone".into()],
            genome_revision: 1,
            genome_digest: genome.digest().unwrap(),
            definition: ArtifactAvailability {
                schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
                artifact: ArtifactDescriptor {
                    digest: sha256(b"expression definition"),
                    media_type: "application/json".into(),
                    byte_length: 21,
                    name: Some("expression.json".into()),
                },
                status: AvailabilityStatus::LocallyAvailable,
                locations: Vec::new(),
            },
        };
        let fifth = signed_action(
            5,
            Some(fourth_head),
            shot_id,
            controller,
            LineagePayload::Expression(expression),
            &key,
        );
        let fifth_head = fifth.commitment().unwrap();
        layout
            .append_lineage_batch(&[first, second, third, fourth, fifth])
            .unwrap();
        (
            key,
            controller,
            shot_id,
            expression_id,
            genome,
            fifth_head,
            5,
        )
    }

    fn version(
        shot_id: ShotId,
        expression_id: ExpressionId,
        input_action: Bytes32,
    ) -> VersionRecord {
        let genome_digest = sha256(b"genome");
        let source_digest = sha256(b"source");
        let version_id = VersionId::derive(shot_id, expression_id, 1, genome_digest, source_digest);
        VersionRecord {
            schema: VERSION_SCHEMA.into(),
            version_id,
            expression_id,
            ordinal: 1,
            genome_revision: 1,
            genome_digest,
            source_digest,
            provenance: MaterializationProvenance {
                factory: "tohseno/apple-factory".into(),
                factory_version: "1.0.0-test".into(),
                factory_source_commit: Some("a".repeat(40)),
                template_digest: sha256(b"template"),
                input_action,
                deterministic: true,
            },
            capability_graph_digest: tohseno_protocol::capability_graph_digest(&[]).unwrap(),
            verification_action: sha256(b"verification action"),
            known_incompleteness: Vec::new(),
            build_identity: None,
            build_digest: None,
            accepted_at: timestamp(),
        }
    }

    #[test]
    fn exact_intention_is_private_immutable_and_git_ignored() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let layout = ShotLayout::at(&root);
        let input = b"I need a quiet place for one honest sentence.";
        assert_eq!(
            layout.preserve_exact_intention(input).unwrap(),
            sha256(input)
        );
        assert_eq!(fs::read(root.join(INTENTION_DOCUMENT)).unwrap(), input);
        let readme = fs::read_to_string(root.join("README.md")).unwrap();
        assert!(readme.contains("The Shot is not this folder"));
        assert!(readme.contains(".tohseno/lineage.jsonl"));
        layout.preserve_exact_intention(input).unwrap();
        assert!(layout.preserve_exact_intention(b"rewritten").is_err());
        let ignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(ignore.contains("INTENTION.md"));
        assert!(ignore.contains("GENOME.md"));
        assert!(ignore.contains("versions/"));
        assert!(ignore.contains(".tohseno/shot.json"));
        assert!(ignore.contains(".tohseno/genome.json"));
        assert!(ignore.contains(".tohseno/expression.json"));
        assert!(ignore.contains(".tohseno/capabilities.lock"));
        assert!(ignore.contains(".tohseno/private/"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(root.join(INTENTION_DOCUMENT))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        let adopted_root = temporary.path().join("adopted");
        fs::create_dir(&adopted_root).unwrap();
        fs::write(adopted_root.join("README.md"), b"# Existing project\n").unwrap();
        fs::write(
            adopted_root.join(".gitignore"),
            b"build/\n# BEGIN TOHSENO PRIVATE MATERIAL\nINTENTION.md\n# END TOHSENO PRIVATE MATERIAL\ncustom/\n",
        )
        .unwrap();
        ShotLayout::at(&adopted_root)
            .initialize_directories()
            .unwrap();
        assert_eq!(
            fs::read(adopted_root.join("README.md")).unwrap(),
            b"# Existing project\n"
        );
        let upgraded_ignore = fs::read_to_string(adopted_root.join(".gitignore")).unwrap();
        assert!(upgraded_ignore.contains("build/"));
        assert!(upgraded_ignore.contains("custom/"));
        assert!(upgraded_ignore.contains("GENOME.md"));
        assert!(upgraded_ignore.contains(".tohseno/capabilities.lock"));
    }

    #[test]
    fn genome_rendering_detects_drift() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let layout = ShotLayout::at(&root);
        let rendered = "# Genome\n\nMust remain local-first.\n";
        layout.write_human_genome(rendered).unwrap();
        layout.verify_human_genome(rendered).unwrap();
        fs::write(root.join(GENOME_DOCUMENT), "# changed\n").unwrap();
        assert!(matches!(
            layout.verify_human_genome(rendered),
            Err(ShotLayoutError::GenomeDrift)
        ));
    }

    #[test]
    fn lineage_and_version_writes_are_canonical_and_append_only() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let layout = ShotLayout::at(&root);
        let key = SigningKey::from_bytes((&[1_u8; 32]).into()).unwrap();
        let public_key = signer(&key);
        let controller = BuilderId::new(Address20::from_bytes([0x22; 20]));
        let shot_id = ShotId::from_bytes([0x11; 32]);
        let commitment = ShotCommitment {
            schema: tohseno_protocol::ontology::SHOT_COMMITMENT_SCHEMA.into(),
            intention_commitment: sha256(b"intention"),
            initial_controller: controller,
            initial_controller_key: public_key,
            origin: CommitmentOrigin::Native,
            committed_at: timestamp(),
        };
        let first = sign(
            LineageAction::new(
                1,
                None,
                shot_id,
                controller,
                timestamp(),
                AvailabilityStatus::IntentionallyPrivate,
                LineagePayload::Commitment(commitment),
            )
            .unwrap(),
            &key,
        );
        let digest = layout.append_lineage(&first).unwrap();
        assert_eq!(layout.append_lineage(&first).unwrap(), digest);
        assert_eq!(layout.read_lineage().unwrap(), [first]);
        assert_eq!(
            fs::read_to_string(layout.lineage_path())
                .unwrap()
                .lines()
                .count(),
            1
        );

        let expression_id = ExpressionId::from_bytes([0x33; 32]);
        let version = version(shot_id, expression_id, sha256(b"input action"));
        layout.write_version_record(shot_id, &version).unwrap();
        layout.write_version_record(shot_id, &version).unwrap();
        let mut conflicting = version;
        conflicting.known_incompleteness.push("different".into());
        assert!(layout.write_version_record(shot_id, &conflicting).is_err());
    }

    #[test]
    fn public_action_outbox_is_exact_idempotent_and_rejects_private_actions() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let layout = ShotLayout::at(&root);
        let (key, controller, shot_id, _, _, previous, sequence) = initial_lineage(&layout);
        let relation = TokenAssociation {
            schema: TOKEN_ASSOCIATION_SCHEMA.into(),
            operation: TokenAssociationOperation::Associate,
            chain_id: 8_453,
            token: Address20::from_bytes([0xa7; 20]),
            symbol: Some("ANKY".into()),
            anchor: None,
        };
        let public = signed_action_with_availability(
            sequence + 1,
            Some(previous),
            shot_id,
            controller,
            AvailabilityStatus::PubliclyAvailable,
            LineagePayload::TokenAssociation(relation.clone()),
            &key,
        );
        let path = layout.write_public_action_outbox(&public).unwrap();
        assert_eq!(layout.write_public_action_outbox(&public).unwrap(), path);
        assert_eq!(
            fs::read(&path).unwrap(),
            tohseno_protocol::canonical::to_vec(&public).unwrap()
        );
        assert!(path.starts_with(
            root.join(".tohseno")
                .join("private")
                .join("public-action-outbox")
        ));

        let private = signed_action(
            sequence + 1,
            Some(previous),
            shot_id,
            controller,
            LineagePayload::TokenAssociation(relation),
            &key,
        );
        assert!(layout.write_public_action_outbox(&private).is_err());
    }

    #[test]
    fn feedback_attachments_are_private_content_addressed_files() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let attachment = temporary.path().join("screen.PNG");
        fs::write(&attachment, b"image bytes").unwrap();
        let layout = ShotLayout::at(&root);
        let shot_id = ShotId::from_bytes([0x11; 32]);
        let version = version(
            shot_id,
            ExpressionId::from_bytes([0x33; 32]),
            sha256(b"input action"),
        );
        let descriptor = ArtifactDescriptor {
            digest: sha256(b"image bytes"),
            media_type: "image/png".into(),
            byte_length: 11,
            name: Some("screen.PNG".into()),
        };
        let record = Feedback {
            schema: FEEDBACK_SCHEMA.into(),
            expression_id: version.expression_id,
            version_id: version.version_id,
            build_identity: None,
            author: None,
            visibility: Visibility::Private,
            text: Some("The first screen felt calm.".into()),
            observations: Vec::new(),
            attachments: vec![ArtifactAvailability {
                schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
                artifact: descriptor,
                status: AvailabilityStatus::IntentionallyPrivate,
                locations: Vec::new(),
            }],
            observed_at: timestamp(),
        };
        let stored = layout
            .store_feedback(
                shot_id,
                &version,
                &record,
                sha256(b"feedback action"),
                &[attachment],
            )
            .unwrap();
        assert_eq!(stored.action_commitment, sha256(b"feedback action"));
        let entries = fs::read_dir(stored.directory.join("attachments"))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].file_name().to_string_lossy().ends_with(".png"));
        let index =
            fs::read_to_string(stored.directory.parent().unwrap().join("index.json")).unwrap();
        assert!(index.contains("\"status\":\"present\""));
    }

    #[test]
    fn pending_evolution_selection_is_exact_prompt_bound_and_retryable() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let layout = ShotLayout::at(&root);
        let prompt = b"Make the accepted continuity visible.";
        let mut actions = vec![sha256(b"feedback action 1"), sha256(b"feedback action 2")];
        actions.sort_unstable();

        layout.stage_evolution_prompt(prompt).unwrap();
        layout
            .stage_evolution_feedback_selection(prompt, &actions)
            .unwrap();
        assert_eq!(
            layout.pending_evolution_prompt().unwrap().as_deref(),
            Some(std::str::from_utf8(prompt).unwrap())
        );
        assert_eq!(
            layout.pending_evolution_feedback_selection(prompt).unwrap(),
            actions
        );
        assert!(layout
            .pending_evolution_feedback_selection(b"different")
            .is_err());
        // A failed attempt performs no clear and the exact selection remains.
        assert_eq!(
            layout.pending_evolution_feedback_selection(prompt).unwrap(),
            actions
        );

        layout.clear_evolution_feedback_selection(prompt).unwrap();
        layout.clear_evolution_prompt(prompt).unwrap();
        assert!(layout.pending_evolution_prompt().unwrap().is_none());
        assert!(layout
            .pending_evolution_feedback_selection(prompt)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn private_references_are_exact_content_addressed_source_materials() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let source = temporary.path().join("Visual Direction.PNG");
        fs::write(&source, b"exact private image bytes").unwrap();
        let layout = ShotLayout::at(&root);

        let stored = layout
            .stage_private_references(std::slice::from_ref(&source))
            .unwrap();
        assert_eq!(stored.len(), 1);
        let reference = &stored[0];
        assert_eq!(
            reference.path,
            root.join(".tohseno/references").join(
                reference
                    .availability
                    .artifact
                    .digest
                    .to_string()
                    .trim_start_matches("0x")
            )
        );
        assert_eq!(
            reference.availability.artifact.name.as_deref(),
            Some("Visual Direction.PNG")
        );
        assert_eq!(reference.availability.artifact.media_type, "image/png");
        assert_eq!(
            reference.availability.status,
            AvailabilityStatus::IntentionallyPrivate
        );
        assert!(reference.availability.locations.is_empty());
        assert_eq!(
            layout
                .read_private_reference(&reference.availability)
                .unwrap(),
            b"exact private image bytes"
        );
        assert_eq!(
            layout
                .stage_private_references(std::slice::from_ref(&source))
                .unwrap(),
            stored
        );
    }

    #[test]
    fn evolution_references_share_one_prompt_bound_retry_safe_selection() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let source = temporary.path().join("new-state.webp");
        fs::write(&source, b"RIFF\x10\x00\x00\x00WEBPnew visual state").unwrap();
        let layout = ShotLayout::at(&root);
        let prompt = b"Make the exact new visual state feel quieter.";
        let mut feedback = vec![sha256(b"feedback two"), sha256(b"feedback one")];
        feedback.sort_unstable();

        let first = layout
            .stage_evolution_inputs(prompt, &feedback, std::slice::from_ref(&source))
            .unwrap();
        let second = layout
            .stage_evolution_inputs(prompt, &feedback, std::slice::from_ref(&source))
            .unwrap();
        assert_eq!(first, second);
        let (selected_feedback, selected_references) =
            layout.pending_evolution_inputs(prompt).unwrap();
        assert_eq!(selected_feedback, feedback);
        assert_eq!(selected_references, vec![first[0].availability.clone()]);
        EvolutionaryIntent {
            schema: EVOLUTIONARY_INTENT_SCHEMA.into(),
            expression_id: ExpressionId::from_bytes([0x31; 32]),
            from_version_id: VersionId::from_bytes([0x32; 32]),
            preserved_invariants: vec!["Keep the experience private.".into()],
            desired_changes: vec![DesiredChange {
                scope: ChangeScope::Implementation,
                description: "Use the supplied exact visual state.".into(),
            }],
            feedback_actions: selected_feedback.clone(),
            references: selected_references.clone(),
            proposed_genome_action: None,
        }
        .validate()
        .unwrap();
        assert!(layout
            .pending_evolution_inputs(b"a different instruction")
            .is_err());

        let selection_path = layout.pending_evolution_selection_path();
        let mut forged = read_canonical_json::<PendingEvolutionSelection>(&selection_path).unwrap();
        forged.references[0].artifact.byte_length += 1;
        let mut forged_bytes = tohseno_protocol::canonical::to_vec(&forged).unwrap();
        forged_bytes.push(b'\n');
        write_replace_file(&selection_path, &forged_bytes, true).unwrap();
        assert!(layout.pending_evolution_inputs(prompt).is_err());
        layout
            .stage_evolution_inputs(prompt, &feedback, std::slice::from_ref(&source))
            .unwrap();

        fs::write(&first[0].path, b"tampered visual").unwrap();
        assert!(layout.pending_evolution_inputs(prompt).is_err());
        assert!(layout
            .stage_evolution_inputs(prompt, &feedback, std::slice::from_ref(&source))
            .is_err());
    }

    #[test]
    fn prepared_intent_package_labels_images_by_input_order_and_normalizes_names() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let first = temporary.path().join("unsafe name (final).PNG");
        let second = temporary.path().join("另一个.jpeg");
        fs::write(&first, b"\x89PNG\r\n\x1a\nreference one").unwrap();
        fs::write(&second, b"\xff\xd8\xffreference two").unwrap();
        let layout = ShotLayout::at(&root);
        let (package, stored) = layout
            .prepare_intent_package(
                b"Build the exact local experience.",
                &[first.clone(), second.clone()],
            )
            .unwrap();

        assert_eq!(stored.len(), 2);
        assert_eq!(package.references[0].label, "image_1");
        assert_eq!(
            package.references[0].relative_path,
            ".tohseno/references/image_1.png"
        );
        assert_eq!(package.references[1].label, "image_2");
        assert_eq!(
            package.references[1].relative_path,
            ".tohseno/references/image_2.jpeg"
        );
        assert_eq!(
            fs::read(root.join(".tohseno/references/image_1.png")).unwrap(),
            fs::read(first).unwrap()
        );
        assert_eq!(
            fs::read(root.join(".tohseno/references/image_2.jpeg")).unwrap(),
            fs::read(second).unwrap()
        );
        let document = fs::read_to_string(root.join(".tohseno/EVOLUTION_INTENT.md")).unwrap();
        assert!(document.contains("- image_1: `.tohseno/references/image_1.png`"));
        assert!(document.contains("- image_2: `.tohseno/references/image_2.jpeg`"));
        assert!(!document.contains("unsafe name"));
        assert_eq!(layout.prepared_intent_package().unwrap(), package);

        let (without_images, _) = layout
            .prepare_intent_package(b"A second exact intention.", &[])
            .unwrap();
        assert!(without_images.references.is_empty());
        assert!(!root.join(".tohseno/references/image_1.png").exists());
    }

    #[test]
    fn prepared_intent_package_rejects_every_unusable_attachment() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let fake = temporary.path().join("fake.png");
        fs::write(&fake, b"not a png").unwrap();
        let layout = ShotLayout::at(&root);
        assert!(layout
            .prepare_intent_package(b"Use the image.", std::slice::from_ref(&fake))
            .is_err());

        let mut nine = Vec::new();
        for index in 0..9 {
            let path = temporary.path().join(format!("{index}.png"));
            fs::write(&path, [b"\x89PNG\r\n\x1a\n".as_slice(), &[index]].concat()).unwrap();
            nine.push(path);
        }
        assert!(layout.prepare_intent_package(b"Too many.", &nine).is_err());
    }

    #[test]
    fn private_reference_staging_rejects_unsafe_oversize_and_colliding_inputs() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let layout = ShotLayout::at(&root);

        let duplicate_a = temporary.path().join("first.png");
        let duplicate_b = temporary.path().join("second.png");
        fs::write(&duplicate_a, b"same").unwrap();
        fs::write(&duplicate_b, b"same").unwrap();
        assert!(layout
            .stage_private_references(&[duplicate_a, duplicate_b])
            .is_err());

        let first_directory = temporary.path().join("first");
        let second_directory = temporary.path().join("second");
        fs::create_dir(&first_directory).unwrap();
        fs::create_dir(&second_directory).unwrap();
        let collision_a = first_directory.join("Scene.PNG");
        let collision_b = second_directory.join("scene.png");
        fs::write(&collision_a, b"first").unwrap();
        fs::write(&collision_b, b"second").unwrap();
        assert!(layout
            .stage_private_references(&[collision_a, collision_b])
            .is_err());

        let unsafe_name = temporary.path().join("unsafe\nname.png");
        fs::write(&unsafe_name, b"unsafe").unwrap();
        assert!(layout.stage_private_references(&[unsafe_name]).is_err());

        let oversized = temporary.path().join("oversized.png");
        File::create(&oversized)
            .unwrap()
            .set_len(MAX_ATTACHMENT_BYTES as u64 + 1)
            .unwrap();
        assert!(layout.stage_private_references(&[oversized]).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let target = temporary.path().join("target.png");
            let link = temporary.path().join("linked.png");
            fs::write(&target, b"target").unwrap();
            symlink(&target, &link).unwrap();
            assert!(layout.stage_private_references(&[link]).is_err());
        }
    }

    #[test]
    fn feedback_paths_are_expression_scoped_even_when_ordinals_match() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let layout = ShotLayout::at(&root);
        let shot_id = ShotId::from_bytes([0x41; 32]);
        let first = version(
            shot_id,
            ExpressionId::from_bytes([0x42; 32]),
            sha256(b"first-input"),
        );
        let second = version(
            shot_id,
            ExpressionId::from_bytes([0x43; 32]),
            sha256(b"second-input"),
        );
        let feedback_for = |version: &VersionRecord, text: &str| Feedback {
            schema: FEEDBACK_SCHEMA.into(),
            expression_id: version.expression_id,
            version_id: version.version_id,
            build_identity: version.build_identity.clone(),
            author: None,
            visibility: Visibility::Private,
            text: Some(text.into()),
            observations: Vec::new(),
            attachments: Vec::new(),
            observed_at: timestamp(),
        };
        let first_stored = layout
            .store_feedback(
                shot_id,
                &first,
                &feedback_for(&first, "First expression."),
                sha256(b"first feedback action"),
                &[],
            )
            .unwrap();
        let second_stored = layout
            .store_feedback(
                shot_id,
                &second,
                &feedback_for(&second, "Second expression."),
                sha256(b"second feedback action"),
                &[],
            )
            .unwrap();
        assert_ne!(
            first_stored.directory.parent(),
            second_stored.directory.parent()
        );
        assert!(first_stored
            .directory
            .to_string_lossy()
            .contains(&expression_component(first.expression_id)));
        assert!(second_stored
            .directory
            .to_string_lossy()
            .contains(&expression_component(second.expression_id)));
    }

    #[test]
    fn materialization_uses_input_head_and_failed_verification_is_never_canonical() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let layout = ShotLayout::at(&root);
        let (key, controller, shot_id, expression_id, genome, input_head, input_sequence) =
            initial_lineage(&layout);
        let mut version = version(shot_id, expression_id, input_head);
        version.genome_digest = genome.digest().unwrap();
        version.version_id = VersionId::derive(
            shot_id,
            expression_id,
            version.ordinal,
            version.genome_digest,
            version.source_digest,
        );

        let failed_verification = VerificationResult {
            schema: VERIFICATION_RESULT_SCHEMA.into(),
            expression_id,
            candidate_version_id: version.version_id,
            genome_revision: 1,
            genome_digest: genome.digest().unwrap(),
            source_digest: version.source_digest,
            capability_graph_digest: version.capability_graph_digest,
            gates: vec![VerificationGate {
                name: "release_build".into(),
                passed: false,
                deterministic: true,
                evidence: None,
            }],
            passed: false,
            known_incompleteness: Vec::new(),
            verified_at: timestamp(),
        };
        let failed_action = signed_action(
            6,
            Some(input_head),
            shot_id,
            controller,
            LineagePayload::VerificationResult(failed_verification),
            &key,
        );
        version.verification_action = failed_action.commitment().unwrap();
        let rejected_version = signed_action(
            7,
            Some(failed_action.commitment().unwrap()),
            shot_id,
            controller,
            LineagePayload::Version(version.clone()),
            &key,
        );
        let before = fs::read(layout.lineage_path()).unwrap();
        assert!(layout
            .append_lineage_batch(&[failed_action, rejected_version])
            .is_err());
        assert_eq!(fs::read(layout.lineage_path()).unwrap(), before);
        assert_eq!(layout.read_lineage().unwrap().len(), 5);
        assert!(!root.join("versions/0001").exists());

        let passing = VerificationResult {
            schema: VERIFICATION_RESULT_SCHEMA.into(),
            expression_id,
            candidate_version_id: version.version_id,
            genome_revision: 1,
            genome_digest: genome.digest().unwrap(),
            source_digest: version.source_digest,
            capability_graph_digest: version.capability_graph_digest,
            gates: vec![VerificationGate {
                name: "release_build".into(),
                passed: true,
                deterministic: true,
                evidence: None,
            }],
            passed: true,
            known_incompleteness: Vec::new(),
            verified_at: timestamp(),
        };
        let verification_action = signed_action(
            6,
            Some(input_head),
            shot_id,
            controller,
            LineagePayload::VerificationResult(passing),
            &key,
        );
        version.verification_action = verification_action.commitment().unwrap();
        let version_action = signed_action(
            7,
            Some(verification_action.commitment().unwrap()),
            shot_id,
            controller,
            LineagePayload::Version(version.clone()),
            &key,
        );
        let metadata = AppMetadataV2 {
            protocol_name: PROTOCOL_NAME.into(),
            protocol_version: tohseno_protocol::app_metadata::APP_METADATA_V2_PROTOCOL_VERSION
                .into(),
            schema: tohseno_protocol::app_metadata::APP_METADATA_V2_SCHEMA.into(),
            fascia: APPLE_FASCIA_ID.into(),
            shot_id,
            builder_id: controller,
            expression_id,
            version_id: version.version_id,
            version_ordinal: 1,
            genome_revision: 1,
            genome_digest: genome.digest().unwrap(),
            lineage_sequence: input_sequence,
            lineage_head: input_head,
            source_tree_sha256: version.source_digest,
            fascia_sha256: sha256(b"fascia"),
            build_digest: None,
            bundle_id: "com.tohseno.genesis.fixture.LifecycleShot".into(),
            bundle_version: 1,
            factory: FactoryDescriptor {
                implementation: "tohseno/apple-factory".into(),
                version: "1.0.0-test".into(),
                source_commit: "a".repeat(40),
            },
            distribution: AppMetadataDistribution {
                state: DistributionState::Local,
                supported_apple_surfaces: vec![AppleSurface::Iphone],
                app_store_id: None,
            },
            capabilities: Vec::new(),
            network: Vec::new(),
            registry: None,
            legacy_v1_evolution_commitment: None,
        };
        layout
            .verify_apple_materialization_binding(&metadata, &version)
            .unwrap();
        let accepted = layout
            .record_accepted_materialization(&metadata, &verification_action, &version_action, None)
            .unwrap();
        assert_eq!(accepted.version, version);
        assert_eq!(accepted.lineage_head, version_action.commitment().unwrap());
        assert!(accepted.version_path.is_file());
        assert!(accepted.feedback_directory.join("index.json").is_file());
        assert_eq!(layout.read_lineage().unwrap().len(), 7);
        let report = layout.verify_shot_body(Some(expression_id)).unwrap();
        assert_eq!(report.lineage_head, accepted.lineage_head);
        assert_eq!(report.selected_version_id, Some(version.version_id));
        assert_eq!(report.genome_digest, Some(genome.digest().unwrap()));
        assert!(!report.embedded_metadata_verified);
        layout.verify_accepted_apple_metadata(&metadata).unwrap();

        let mut circular = metadata.clone();
        circular.lineage_sequence = 6;
        circular.lineage_head = verification_action.commitment().unwrap();
        assert!(layout
            .verify_apple_materialization_binding(&circular, &version)
            .is_err());
        assert!(layout.verify_accepted_apple_metadata(&circular).is_err());

        let mut wrong_genome_revision = metadata.clone();
        wrong_genome_revision.genome_revision = 2;
        assert!(wrong_genome_revision.validate().is_ok());
        assert!(layout
            .verify_accepted_apple_metadata(&wrong_genome_revision)
            .is_err());

        let missing_attachment = sha256(b"declared private screenshot");
        let feedback = Feedback {
            schema: FEEDBACK_SCHEMA.into(),
            expression_id,
            version_id: version.version_id,
            build_identity: version.build_identity.clone(),
            author: None,
            visibility: Visibility::Private,
            text: Some("This exact version felt calm.".into()),
            observations: Vec::new(),
            attachments: vec![ArtifactAvailability {
                schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
                artifact: ArtifactDescriptor {
                    digest: missing_attachment,
                    media_type: "image/png".into(),
                    byte_length: 27,
                    name: Some("private-screen.png".into()),
                },
                status: AvailabilityStatus::IntentionallyPrivate,
                locations: Vec::new(),
            }],
            observed_at: timestamp(),
        };
        let feedback_action = signed_action(
            8,
            Some(accepted.lineage_head),
            shot_id,
            controller,
            LineagePayload::Feedback(feedback.clone()),
            &key,
        );
        let stored = layout
            .record_feedback_action(shot_id, &version, &feedback, &feedback_action, &[])
            .unwrap();
        assert_eq!(
            stored.action_commitment,
            feedback_action.commitment().unwrap()
        );
        assert!(stored.directory.join("feedback.json").is_file());
        assert_eq!(
            layout
                .verify_shot_body(Some(expression_id))
                .unwrap()
                .missing_attachment_digests,
            vec![missing_attachment]
        );

        let mut unavailable_version = version.clone();
        unavailable_version.ordinal = 2;
        unavailable_version.source_digest = sha256(b"unaccepted source");
        unavailable_version.version_id = VersionId::derive(
            shot_id,
            expression_id,
            unavailable_version.ordinal,
            unavailable_version.genome_digest,
            unavailable_version.source_digest,
        );
        let unavailable_feedback = Feedback {
            version_id: unavailable_version.version_id,
            attachments: Vec::new(),
            text: Some("Must not float to an unaccepted version.".into()),
            ..feedback
        };
        let rejected_action = signed_action(
            9,
            Some(feedback_action.commitment().unwrap()),
            shot_id,
            controller,
            LineagePayload::Feedback(unavailable_feedback.clone()),
            &key,
        );
        let before = fs::read(layout.lineage_path()).unwrap();
        assert!(layout
            .record_feedback_action(
                shot_id,
                &unavailable_version,
                &unavailable_feedback,
                &rejected_action,
                &[],
            )
            .is_err());
        assert_eq!(fs::read(layout.lineage_path()).unwrap(), before);

        let feedback_head = feedback_action.commitment().unwrap();
        let evolutionary_intent = EvolutionaryIntent {
            schema: EVOLUTIONARY_INTENT_SCHEMA.into(),
            expression_id,
            from_version_id: version.version_id,
            preserved_invariants: genome.behavioral_invariants.clone(),
            desired_changes: vec![DesiredChange {
                scope: ChangeScope::Implementation,
                description: "Apply the selected exact-version feedback without changing the accepted Genome."
                    .into(),
            }],
            feedback_actions: vec![feedback_head],
            references: Vec::new(),
            proposed_genome_action: None,
        };
        let intent_action = signed_action(
            9,
            Some(feedback_head),
            shot_id,
            controller,
            LineagePayload::EvolutionaryIntent(evolutionary_intent),
            &key,
        );
        let intent_head = intent_action.commitment().unwrap();
        layout.append_lineage_batch(&[intent_action]).unwrap();

        let mut second_version = version.clone();
        second_version.ordinal = 2;
        second_version.source_digest = sha256(b"source version 0002");
        second_version.version_id = VersionId::derive(
            shot_id,
            expression_id,
            second_version.ordinal,
            second_version.genome_digest,
            second_version.source_digest,
        );
        second_version.provenance.input_action = intent_head;
        let second_verification = VerificationResult {
            schema: VERIFICATION_RESULT_SCHEMA.into(),
            expression_id,
            candidate_version_id: second_version.version_id,
            genome_revision: second_version.genome_revision,
            genome_digest: second_version.genome_digest,
            source_digest: second_version.source_digest,
            capability_graph_digest: second_version.capability_graph_digest,
            gates: vec![VerificationGate {
                name: "release_build".into(),
                passed: true,
                deterministic: true,
                evidence: None,
            }],
            passed: true,
            known_incompleteness: Vec::new(),
            verified_at: timestamp(),
        };
        let second_verification_action = signed_action(
            10,
            Some(intent_head),
            shot_id,
            controller,
            LineagePayload::VerificationResult(second_verification),
            &key,
        );
        second_version.verification_action = second_verification_action.commitment().unwrap();
        let second_version_action = signed_action(
            11,
            Some(second_version.verification_action),
            shot_id,
            controller,
            LineagePayload::Version(second_version.clone()),
            &key,
        );
        let second_version_head = second_version_action.commitment().unwrap();
        let evolution_action = signed_action(
            12,
            Some(second_version_head),
            shot_id,
            controller,
            LineagePayload::Evolution(tohseno_protocol::Evolution {
                schema: EVOLUTION_SCHEMA.into(),
                evolutionary_intent_action: intent_head,
                expression_id,
                from_version_id: version.version_id,
                to_version_id: second_version.version_id,
                from_genome_digest: version.genome_digest,
                to_genome_digest: second_version.genome_digest,
                genome_acceptance_action: None,
                preserved_invariants: genome.behavioral_invariants.clone(),
                completed_at: timestamp(),
            }),
            &key,
        );
        let mut second_metadata = metadata;
        second_metadata.version_id = second_version.version_id;
        second_metadata.version_ordinal = 2;
        second_metadata.lineage_sequence = 9;
        second_metadata.lineage_head = intent_head;
        second_metadata.source_tree_sha256 = second_version.source_digest;
        second_metadata.bundle_version = 2;
        second_metadata.validate().unwrap();
        let second_accepted = layout
            .record_accepted_materialization(
                &second_metadata,
                &second_verification_action,
                &second_version_action,
                Some(&evolution_action),
            )
            .unwrap();
        assert_eq!(
            second_accepted.lineage_head,
            evolution_action.commitment().unwrap()
        );
        assert_eq!(layout.read_lineage().unwrap().len(), 12);
        assert!(root
            .join(VERSIONS_DIRECTORY)
            .join(expression_component(expression_id))
            .join("0001/version.json")
            .is_file());
        assert!(second_accepted.version_path.ends_with("0002/version.json"));
        assert!(second_accepted
            .feedback_directory
            .join("index.json")
            .is_file());
        let second_report = layout.verify_shot_body(Some(expression_id)).unwrap();
        assert_eq!(
            second_report.selected_version_id,
            Some(second_version.version_id)
        );
        assert_eq!(
            second_report.missing_attachment_digests,
            vec![missing_attachment]
        );

        let bundle = temporary.path().join("accepted-record-bundle");
        let manifest = layout
            .export_bundle(&bundle, PortableVisibility::IncludePrivate)
            .unwrap();
        assert!(!manifest.materialization_ready);
        let imported_root = temporary.path().join("accepted-record-import");
        let imported = ShotLayout::import_bundle(&bundle, &imported_root).unwrap();
        let imported_report = imported
            .layout
            .verify_shot_body(Some(expression_id))
            .unwrap();
        assert_eq!(
            imported_report.selected_version_id,
            Some(second_version.version_id)
        );
        assert!(!imported_report.embedded_metadata_verified);
        assert_eq!(
            imported_report.missing_attachment_digests,
            vec![missing_attachment]
        );

        fs::write(root.join(GENOME_DOCUMENT), b"# forged genome\n").unwrap();
        assert!(matches!(
            layout.verify_shot_body(Some(expression_id)),
            Err(ShotLayoutError::GenomeDrift)
        ));
    }

    #[test]
    fn v1_migration_is_verified_honest_and_idempotent() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let layout = ShotLayout::at(&root);
        let key = SigningKey::from_bytes((&[2_u8; 32]).into()).unwrap();
        let record = ShotRecord {
            protocol: PROTOCOL_NAME.into(),
            schema: SHOT_SCHEMA.into(),
            shot_id: ShotId::from_bytes([0x61; 32]),
            slug: "legacy-shot".into(),
            builder_id: BuilderId::new(Address20::from_bytes([0x62; 20])),
            sequence: 1,
            previous: None,
            fascia: APPLE_FASCIA_ID.into(),
            bundle_id: "com.tohseno.genesis.fixture.legacy-shot".into(),
            bundle_version: 1,
            genesis_input_sha256: sha256(b"legacy intention commitment"),
            source_tree_sha256: sha256(b"legacy source"),
            fascia_sha256: sha256(b"legacy fascia"),
            factory: FactoryDescriptor {
                implementation: "tohseno/legacy-apple-factory".into(),
                version: "0.7.0".into(),
                source_commit: "b".repeat(40),
            },
            created_at: timestamp(),
            origin: None,
        };
        let signature = sign_v1_record(&record, &key);
        let adapted = tohseno_protocol::adapt_v1_lineage(&[(&record, &signature)]).unwrap();
        let path = layout.write_v1_migration(&adapted).unwrap();
        let first = fs::read(&path).unwrap();
        layout.write_v1_migration(&adapted).unwrap();
        assert_eq!(fs::read(&path).unwrap(), first);
        assert_eq!(adapted.intention_availability, AvailabilityStatus::Unknown);
        assert_eq!(adapted.genome_availability, AvailabilityStatus::Unknown);
        assert!(fs::read_to_string(path)
            .unwrap()
            .contains("\"genome_availability\":\"unknown\""));
        let verified = layout.verify_shot_body(None).unwrap();
        assert!(verified.legacy_v1_adapter);
        assert_eq!(verified.shot_id, adapted.shot_id);
        assert_eq!(
            verified.selected_version_id,
            adapted.entries.last().map(|entry| entry.version_id)
        );
        assert_eq!(
            fs::read(root.join("versions/0001/legacy-v1-record.json")).unwrap(),
            {
                let mut encoded = tohseno_protocol::canonical::to_vec(&record).unwrap();
                encoded.push(b'\n');
                encoded
            }
        );

        let mut forged = adapted;
        forged.head = sha256(b"forged head");
        assert!(layout.write_v1_migration(&forged).is_err());
    }

    #[test]
    fn private_portable_bundle_round_trips_and_tampering_is_rejected() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let layout = ShotLayout::at(&root);
        layout.preserve_exact_intention(b"exact intention").unwrap();
        let (_, controller, shot_id, _, _, lineage_head, _) = initial_lineage(&layout);

        let public = temporary.path().join("public-bundle");
        assert!(layout
            .export_bundle(&public, PortableVisibility::Public)
            .is_err());
        assert!(!public.exists());

        let bundle = temporary.path().join("private-bundle");
        let manifest = layout
            .export_bundle(&bundle, PortableVisibility::IncludePrivate)
            .unwrap();
        assert_eq!(manifest.shot_id, shot_id);
        assert_eq!(manifest.controller, controller);
        assert_eq!(manifest.lineage_head, lineage_head);
        assert!(!manifest.materialization_ready);
        assert!(bundle.join(INTENTION_DOCUMENT).is_file());
        assert!(!bundle.join(".tohseno").exists());

        let imported_root = temporary.path().join("imported-shot");
        let imported = ShotLayout::import_bundle(&bundle, &imported_root).unwrap();
        assert_eq!(imported.manifest, manifest);
        assert_eq!(imported.layout.read_lineage().unwrap().len(), 5);
        assert_eq!(
            fs::read(imported_root.join(INTENTION_DOCUMENT)).unwrap(),
            b"exact intention"
        );
        assert!(!imported_root.join(".tohseno/app.toml").exists());
        assert!(!imported_root
            .join(".tohseno/private/agent-runs/key")
            .exists());

        let tampered = temporary.path().join("tampered-bundle");
        copy_bounded_tree(&bundle, &tampered, true).unwrap();
        let lineage_path = tampered.join(LINEAGE_FILE);
        let mut lineage = fs::read(&lineage_path).unwrap();
        lineage.insert(1, b' ');
        fs::write(&lineage_path, lineage).unwrap();
        assert!(ShotLayout::import_bundle(&tampered, &temporary.path().join("rejected")).is_err());
        assert!(!temporary.path().join("rejected").exists());
    }

    #[test]
    fn public_export_never_relabels_private_intention_bytes() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("shot");
        fs::create_dir(&root).unwrap();
        let layout = ShotLayout::at(&root);
        let exact = b"private intention bytes";
        layout.preserve_exact_intention(exact).unwrap();
        let key = SigningKey::from_bytes((&[9_u8; 32]).into()).unwrap();
        let controller = BuilderId::new(Address20::from_bytes([0x71; 20]));
        let shot_id = ShotId::from_bytes([0x72; 32]);
        let intention = IntentionRecord::new(
            vec![OriginalMaterial {
                artifact: ArtifactAvailability {
                    schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
                    artifact: ArtifactDescriptor {
                        digest: sha256(exact),
                        media_type: "text/plain; charset=utf-8".into(),
                        byte_length: u64::try_from(exact.len()).unwrap(),
                        name: Some(INTENTION_DOCUMENT.into()),
                    },
                    status: AvailabilityStatus::IntentionallyPrivate,
                    locations: Vec::new(),
                },
                // The public signed action carries only the digest. Exact
                // private bytes remain solely in the ignored local body.
                inline_text: None,
            }],
            timestamp(),
        );
        let commitment = ShotCommitment::new(
            intention.commitment().unwrap(),
            controller,
            signer(&key),
            timestamp(),
        );
        let first = signed_action_with_availability(
            1,
            None,
            shot_id,
            controller,
            AvailabilityStatus::PubliclyAvailable,
            LineagePayload::Commitment(commitment),
            &key,
        );
        let second = signed_action_with_availability(
            2,
            Some(first.commitment().unwrap()),
            shot_id,
            controller,
            AvailabilityStatus::PubliclyAvailable,
            LineagePayload::Intention(intention),
            &key,
        );
        layout.append_lineage_batch(&[first, second]).unwrap();

        let bundle = temporary.path().join("public");
        let manifest = layout
            .export_bundle(&bundle, PortableVisibility::Public)
            .unwrap();
        assert_eq!(manifest.intention_bytes, AvailabilityStatus::Absent);
        assert!(!bundle.join(INTENTION_DOCUMENT).exists());
        assert!(!manifest
            .files
            .iter()
            .any(|file| file.path == INTENTION_DOCUMENT));
        assert_eq!(fs::read(root.join(INTENTION_DOCUMENT)).unwrap(), exact);
    }

    #[test]
    fn shot_level_surfaces_are_distinct_from_expression_source() {
        for path in [
            "README.md",
            "INTENTION.md",
            "GENOME.md",
            "EVOLUTIONARY_INTENT.md",
            "feedback/versions/0001",
            "versions/0001/version.json",
            ".gitignore",
        ] {
            assert!(is_shot_level_path(path), "{path}");
        }
        assert!(!is_shot_level_path("App/App.swift"));
    }

    #[test]
    fn expression_working_hash_excludes_only_the_shot_body() {
        let working = tempfile::tempdir().unwrap();
        fs::write(working.path().join("App.swift"), b"struct App {}\n").unwrap();
        fs::write(working.path().join(INTENTION_DOCUMENT), b"private source").unwrap();
        fs::write(working.path().join(GENOME_DOCUMENT), b"# Genome\n").unwrap();
        fs::create_dir_all(working.path().join("feedback/versions/0001")).unwrap();
        fs::write(
            working.path().join("feedback/versions/0001/note.txt"),
            b"private feedback",
        )
        .unwrap();

        let clean = tempfile::tempdir().unwrap();
        fs::write(clean.path().join("App.swift"), b"struct App {}\n").unwrap();

        assert_eq!(
            hash_expression_working_tree(working.path()).unwrap().digest,
            tohseno_protocol::tree_hash::hash_source_tree(clean.path())
                .unwrap()
                .digest
        );
    }
}
