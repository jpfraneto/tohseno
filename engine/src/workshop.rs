//! Source-first community testing transport.
//!
//! A workshop capsule is deliberately not protocol law and never becomes a
//! Shot merely because it is downloaded. It carries one already accepted
//! Apple Version's exact public source snapshot plus the canonical records
//! needed to authenticate that snapshot. Testers rebuild locally; the
//! publisher's retained binary and every private `.tohseno` surface stay out.

use crate::builder_identity::{
    initial_device_builder_id_for_v1_factory, BuilderIdentityError, BuilderIdentityManager,
};
use crate::gates::build;
use crate::ledger::{Evolution, Ledger};
use crate::protocol_lifecycle;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use time::OffsetDateTime;
use tohseno_protocol::app_metadata::AppMetadataV2;
use tohseno_protocol::canonical;
use tohseno_protocol::conformance::ConformanceReport;
use tohseno_protocol::digest::{sha256, Bytes32, ExpressionId, ShotId, VersionId};
use tohseno_protocol::identity::BuilderId;
use tohseno_protocol::record::{CanonicalTimestamp, ShotRecord};
use tohseno_protocol::signature::SignatureSidecar;
use tohseno_protocol::tree_hash::{hash_entries, hash_source_tree, SourceTreeEntry};

pub const WORKSHOP_CAPSULE_SCHEMA: &str = "tohseno.workshop-capsule/1";
pub const WORKSHOP_AUTHORIZATION_SCHEMA: &str = "tohseno.workshop-authorization/1";
pub const WORKSHOP_RECEIPT_SCHEMA: &str = "tohseno.local-workshop-receipt/1";
pub const WORKSHOP_FEEDBACK_SCHEMA: &str = "tohseno.workshop-feedback/1";
pub const WORKSHOP_CAPSULE_EXTENSION: &str = "tohseno-workshop";
pub const WORKSHOP_FEEDBACK_EXTENSION: &str = "tohseno-feedback";
pub const WORKSHOP_RECEIPT_FILE: &str = "workshop.json";

const MAX_CAPSULE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SOURCE_BYTES: u64 = 384 * 1024 * 1024;
const MAX_SOURCE_FILES: usize = 10_000;
const MAX_SOURCE_FILE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 4_096;
const MAX_FEEDBACK_BYTES: usize = 100_000;
const LICENSE_NAMES: &[&str] = &[
    "LICENSE",
    "LICENSE.md",
    "LICENSE.txt",
    "COPYING",
    "COPYING.md",
    "COPYING.txt",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkshopCapsule {
    schema: String,
    record: ShotRecord,
    signature: SignatureSidecar,
    conformance: ConformanceReport,
    metadata: AppMetadataV2,
    license: WorkshopLicense,
    files: Vec<WorkshopFile>,
    authorization: SignatureSidecar,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkshopAuthorizationStatement {
    schema: String,
    record_commitment: Bytes32,
    conformance_sha256: Bytes32,
    metadata_sha256: Bytes32,
    shot_id: ShotId,
    builder_id: BuilderId,
    expression_id: ExpressionId,
    version_id: VersionId,
    version_ordinal: u64,
    source_tree_sha256: Bytes32,
    license: WorkshopLicense,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkshopLicense {
    path: String,
    content_sha256: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkshopFile {
    path: String,
    content_sha256: Bytes32,
    byte_length: u64,
    contents_base64: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkshopShare {
    pub capsule: PathBuf,
    pub capsule_sha256: Bytes32,
    pub shot_id: ShotId,
    pub expression_id: ExpressionId,
    pub version_id: VersionId,
    pub version_ordinal: u64,
    pub source_tree_sha256: Bytes32,
    pub source_files: usize,
    pub source_bytes: u64,
    pub license_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkshopReceipt {
    pub schema: String,
    pub capsule_sha256: Bytes32,
    pub app_name: String,
    pub bundle_id: String,
    pub shot_id: ShotId,
    pub builder_id: BuilderId,
    pub expression_id: ExpressionId,
    pub version_id: VersionId,
    pub version_ordinal: u64,
    pub source_tree_sha256: Bytes32,
    pub license_path: String,
    pub source_path: String,
    pub ownership_acquired: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkshopMaterialization {
    pub root: PathBuf,
    pub source: PathBuf,
    pub receipt: WorkshopReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkshopFeedbackPacket {
    pub schema: String,
    pub shot_id: ShotId,
    pub expression_id: ExpressionId,
    pub version_id: VersionId,
    pub version_ordinal: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_display_name: Option<String>,
    pub text: String,
    pub observed_at: CanonicalTimestamp,
    pub author_authentication: String,
}

impl WorkshopFeedbackPacket {
    pub fn validate(&self) -> Result<(), WorkshopError> {
        if self.schema != WORKSHOP_FEEDBACK_SCHEMA {
            return Err(WorkshopError::Invalid(format!(
                "feedback schema must be {WORKSHOP_FEEDBACK_SCHEMA}"
            )));
        }
        if self.shot_id.is_zero()
            || self.expression_id.is_zero()
            || self.version_id.is_zero()
            || self.version_ordinal == 0
        {
            return Err(WorkshopError::Invalid(
                "feedback must bind one nonzero Shot, Expression, and Version".into(),
            ));
        }
        if self.text.is_empty()
            || self.text.len() > MAX_FEEDBACK_BYTES
            || self.text != self.text.trim()
            || self.text.chars().any(char::is_control)
        {
            return Err(WorkshopError::Invalid(
                "feedback text must be 1..=100000 trimmed characters without controls".into(),
            ));
        }
        if let Some(author) = &self.author_display_name {
            if author.is_empty()
                || author.len() > 255
                || author != author.trim()
                || author.chars().any(char::is_control)
            {
                return Err(WorkshopError::Invalid(
                    "feedback author must be 1..=255 trimmed characters without controls".into(),
                ));
            }
        }
        if self.author_authentication != "self_declared" {
            return Err(WorkshopError::Invalid(
                "this transport supports only explicit self_declared authorship".into(),
            ));
        }
        Ok(())
    }
}

pub fn share_workshop(
    ledger: &Ledger,
    app_name: &str,
    destination: &Path,
) -> Result<WorkshopShare, WorkshopError> {
    let app = ledger.load_app(app_name)?;
    let evolution = ledger
        .latest_evolution(app_name)?
        .ok_or_else(|| WorkshopError::Invalid(format!("{app_name} has no accepted Evolution")))?;
    protocol_lifecycle::verify_completed_evolution(&evolution)?;

    let record: ShotRecord = read_canonical(&evolution.path.join("TOHSENO/shot.json"))?;
    let signature: SignatureSidecar =
        read_canonical(&evolution.path.join("TOHSENO/signature.json"))?;
    let conformance: ConformanceReport =
        read_canonical(&evolution.path.join("TOHSENO/conformance.json"))?;
    let metadata: AppMetadataV2 = read_canonical(
        &evolution
            .source_path()
            .join("TOHSENO/embedded-provenance.json"),
    )?;

    record.verify_signature(&signature)?;
    conformance.validate()?;
    metadata.validate()?;
    validate_record_bindings(&record, &conformance, &metadata)?;
    if app.shot_id != Some(record.shot_id)
        || app.builder_id != Some(record.builder_id)
        || app.expression_id != Some(metadata.expression_id)
    {
        return Err(WorkshopError::Invalid(
            "accepted source does not match the local Shot identity".into(),
        ));
    }

    let tree = hash_source_tree(&evolution.source_path())?;
    if tree.digest != record.source_tree_sha256 {
        return Err(WorkshopError::Invalid(
            "accepted source tree no longer matches its signed record".into(),
        ));
    }
    if tree.entries.len() > MAX_SOURCE_FILES {
        return Err(WorkshopError::Limit(format!(
            "workshop source exceeds {MAX_SOURCE_FILES} files"
        )));
    }

    let license_entry = tree
        .entries
        .iter()
        .find(|entry| LICENSE_NAMES.contains(&entry.path.as_str()))
        .cloned()
        .ok_or_else(|| {
            WorkshopError::Invalid(
                "workshop sharing requires a reviewed top-level LICENSE or COPYING file in the accepted source"
                    .into(),
            )
        })?;

    let mut source_bytes = 0_u64;
    let mut files = Vec::with_capacity(tree.entries.len());
    for entry in &tree.entries {
        let bytes = read_source_file(&evolution, entry)?;
        if entry.path == license_entry.path {
            validate_license_bytes(&bytes)?;
        }
        let byte_length = u64::try_from(bytes.len())
            .map_err(|_| WorkshopError::Limit("source file length overflowed".into()))?;
        if byte_length > MAX_SOURCE_FILE_BYTES {
            return Err(WorkshopError::Limit(format!(
                "{} exceeds the per-file workshop limit",
                entry.path
            )));
        }
        source_bytes = source_bytes
            .checked_add(byte_length)
            .ok_or_else(|| WorkshopError::Limit("source length overflowed".into()))?;
        if source_bytes > MAX_SOURCE_BYTES {
            return Err(WorkshopError::Limit(
                "workshop source exceeds 384 MiB".into(),
            ));
        }
        files.push(WorkshopFile {
            path: entry.path.clone(),
            content_sha256: entry.content_sha256,
            byte_length,
            contents_base64: BASE64_STANDARD.encode(bytes),
        });
    }

    let manager = BuilderIdentityManager::for_ledger(ledger);
    let builder = manager.load()?;
    if builder.builder_id != record.builder_id || builder.device.public_key != signature.public_key
    {
        return Err(WorkshopError::Invalid(
            "the accepted Version is not controlled by this local Builder key".into(),
        ));
    }
    let license = WorkshopLicense {
        path: license_entry.path.clone(),
        content_sha256: license_entry.content_sha256,
    };
    let authorization_statement =
        workshop_authorization_statement(&record, &conformance, &metadata, &license)?;
    let authorization_digest = canonical::sha256_commitment(&authorization_statement)?;
    let authorization = manager.sign_record_digest(&builder, authorization_digest)?;

    let capsule = WorkshopCapsule {
        schema: WORKSHOP_CAPSULE_SCHEMA.into(),
        record: record.clone(),
        signature,
        conformance,
        metadata: metadata.clone(),
        license,
        files,
        authorization,
    };
    let mut encoded = canonical::to_vec(&capsule)?;
    encoded.push(b'\n');
    create_destination_parent(destination)?;
    write_new(destination, &encoded)?;

    Ok(WorkshopShare {
        capsule: destination.to_path_buf(),
        capsule_sha256: sha256(&encoded),
        shot_id: record.shot_id,
        expression_id: metadata.expression_id,
        version_id: metadata.version_id,
        version_ordinal: metadata.version_ordinal,
        source_tree_sha256: record.source_tree_sha256,
        source_files: tree.entries.len(),
        source_bytes,
        license_path: license_entry.path,
    })
}

pub fn materialize_workshop(
    capsule_path: &Path,
    destination: &Path,
) -> Result<WorkshopMaterialization, WorkshopError> {
    let capsule_bytes = read_bounded(capsule_path, MAX_CAPSULE_BYTES, "workshop capsule")?;
    let capsule: WorkshopCapsule = canonical::from_slice(&capsule_bytes)?;
    validate_capsule(&capsule)?;
    if destination.exists() {
        return Err(WorkshopError::Invalid(format!(
            "workshop destination already exists: {}",
            destination.display()
        )));
    }
    let parent = destination.parent().ok_or_else(|| {
        WorkshopError::Invalid("workshop destination must have a parent directory".into())
    })?;
    fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new()
        .prefix(".tohseno-workshop-")
        .tempdir_in(parent)?;
    let source = staging.path().join("src");
    fs::create_dir(&source)?;

    let mut observed_entries = Vec::with_capacity(capsule.files.len());
    let mut exact_paths = BTreeSet::new();
    let mut folded_paths = BTreeSet::new();
    let mut observed_source_bytes = 0_u64;
    for file in &capsule.files {
        validate_relative_path(&file.path)?;
        if !exact_paths.insert(file.path.clone())
            || !folded_paths.insert(file.path.to_ascii_lowercase())
        {
            return Err(WorkshopError::Invalid(format!(
                "workshop source repeats or Apple-collides at {}",
                file.path
            )));
        }
        let bytes = BASE64_STANDARD
            .decode(&file.contents_base64)
            .map_err(|_| WorkshopError::Invalid(format!("{} is not valid base64", file.path)))?;
        let observed_length = u64::try_from(bytes.len())
            .map_err(|_| WorkshopError::Limit("source file length overflowed".into()))?;
        observed_source_bytes = observed_source_bytes
            .checked_add(observed_length)
            .ok_or_else(|| WorkshopError::Limit("source length overflowed".into()))?;
        if observed_source_bytes > MAX_SOURCE_BYTES {
            return Err(WorkshopError::Limit(
                "workshop source exceeds 384 MiB".into(),
            ));
        }
        if observed_length != file.byte_length || sha256(&bytes) != file.content_sha256 {
            return Err(WorkshopError::Invalid(format!(
                "{} does not match its declared bytes",
                file.path
            )));
        }
        if file.path == capsule.license.path {
            validate_license_bytes(&bytes)?;
        }
        let path = source.join(Path::new(&file.path));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_new(&path, &bytes)?;
        observed_entries.push(SourceTreeEntry {
            path: file.path.clone(),
            content_sha256: file.content_sha256,
        });
    }

    let embedded = canonical::to_vec(&capsule.metadata)?;
    let embedded_path = source.join("TOHSENO/embedded-provenance.json");
    if let Some(parent) = embedded_path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_new(&embedded_path, &embedded)?;

    let observed_digest = hash_entries(&observed_entries)?;
    if observed_digest != capsule.record.source_tree_sha256
        || hash_source_tree(&source)?.digest != capsule.record.source_tree_sha256
    {
        return Err(WorkshopError::Invalid(
            "materialized workshop source does not match the signed source commitment".into(),
        ));
    }
    build::validate_complete_source(&source)?;

    let receipt = WorkshopReceipt {
        schema: WORKSHOP_RECEIPT_SCHEMA.into(),
        capsule_sha256: sha256(&capsule_bytes),
        app_name: capsule.record.slug.clone(),
        bundle_id: capsule.record.bundle_id.clone(),
        shot_id: capsule.record.shot_id,
        builder_id: capsule.record.builder_id,
        expression_id: capsule.metadata.expression_id,
        version_id: capsule.metadata.version_id,
        version_ordinal: capsule.metadata.version_ordinal,
        source_tree_sha256: capsule.record.source_tree_sha256,
        license_path: capsule.license.path.clone(),
        source_path: "src".into(),
        ownership_acquired: false,
    };
    let mut receipt_bytes = canonical::to_vec(&receipt)?;
    receipt_bytes.push(b'\n');
    write_new(&staging.path().join(WORKSHOP_RECEIPT_FILE), &receipt_bytes)?;

    #[allow(deprecated)]
    let staged_path = staging.into_path();
    fs::rename(&staged_path, destination).map_err(|error| {
        let _ = fs::remove_dir_all(&staged_path);
        WorkshopError::Io(error)
    })?;
    Ok(WorkshopMaterialization {
        root: destination.to_path_buf(),
        source: destination.join("src"),
        receipt,
    })
}

pub fn read_workshop_receipt(path: &Path) -> Result<WorkshopReceipt, WorkshopError> {
    let root = workshop_root(path)?;
    let receipt: WorkshopReceipt = read_canonical(&root.join(WORKSHOP_RECEIPT_FILE))?;
    if receipt.schema != WORKSHOP_RECEIPT_SCHEMA
        || receipt.ownership_acquired
        || receipt.source_path != "src"
        || receipt.shot_id.is_zero()
        || receipt.expression_id.is_zero()
        || receipt.version_id.is_zero()
        || receipt.version_ordinal == 0
    {
        return Err(WorkshopError::Invalid(
            "local workshop receipt is invalid".into(),
        ));
    }
    let source = root.join(&receipt.source_path);
    if hash_source_tree(&source)?.digest != receipt.source_tree_sha256 {
        return Err(WorkshopError::Invalid(
            "workshop source changed after materialization".into(),
        ));
    }
    Ok(receipt)
}

pub fn create_workshop_feedback(
    workshop: &Path,
    text: &str,
    author_display_name: Option<&str>,
    destination: &Path,
) -> Result<WorkshopFeedbackPacket, WorkshopError> {
    let receipt = read_workshop_receipt(workshop)?;
    let packet = WorkshopFeedbackPacket {
        schema: WORKSHOP_FEEDBACK_SCHEMA.into(),
        shot_id: receipt.shot_id,
        expression_id: receipt.expression_id,
        version_id: receipt.version_id,
        version_ordinal: receipt.version_ordinal,
        author_display_name: author_display_name.map(str::to_owned),
        text: text.to_owned(),
        observed_at: canonical_now()?,
        author_authentication: "self_declared".into(),
    };
    packet.validate()?;
    let mut encoded = canonical::to_vec(&packet)?;
    encoded.push(b'\n');
    create_destination_parent(destination)?;
    write_new(destination, &encoded)?;
    Ok(packet)
}

pub fn read_workshop_feedback(path: &Path) -> Result<WorkshopFeedbackPacket, WorkshopError> {
    let packet: WorkshopFeedbackPacket = read_canonical(path)?;
    packet.validate()?;
    Ok(packet)
}

fn validate_capsule(capsule: &WorkshopCapsule) -> Result<(), WorkshopError> {
    if capsule.schema != WORKSHOP_CAPSULE_SCHEMA {
        return Err(WorkshopError::Invalid(format!(
            "capsule schema must be {WORKSHOP_CAPSULE_SCHEMA}"
        )));
    }
    capsule.record.verify_signature(&capsule.signature)?;
    let authorized_builder = initial_device_builder_id_for_v1_factory(
        &capsule.record.factory,
        &capsule.signature.public_key,
    )
    .map_err(|error| WorkshopError::Invalid(error.to_string()))?;
    if authorized_builder != capsule.record.builder_id {
        return Err(WorkshopError::Invalid(
            "workshop signature is not authorized by the recorded Builder identity".into(),
        ));
    }
    if capsule.authorization.public_key != capsule.signature.public_key {
        return Err(WorkshopError::Invalid(
            "workshop authorization and accepted Version use different Builder keys".into(),
        ));
    }
    let authorization_statement = workshop_authorization_statement(
        &capsule.record,
        &capsule.conformance,
        &capsule.metadata,
        &capsule.license,
    )?;
    capsule.authorization.verify(&authorization_statement)?;
    capsule.conformance.validate()?;
    capsule.metadata.validate()?;
    validate_record_bindings(&capsule.record, &capsule.conformance, &capsule.metadata)?;
    if capsule.files.is_empty() || capsule.files.len() > MAX_SOURCE_FILES {
        return Err(WorkshopError::Limit(
            "workshop capsule has an invalid source file count".into(),
        ));
    }
    let mut declared_source_bytes = 0_u64;
    for file in &capsule.files {
        validate_relative_path(&file.path)?;
        if file.byte_length > MAX_SOURCE_FILE_BYTES {
            return Err(WorkshopError::Limit(format!(
                "{} exceeds the per-file workshop limit",
                file.path
            )));
        }
        declared_source_bytes = declared_source_bytes
            .checked_add(file.byte_length)
            .ok_or_else(|| WorkshopError::Limit("source length overflowed".into()))?;
        if declared_source_bytes > MAX_SOURCE_BYTES {
            return Err(WorkshopError::Limit(
                "workshop source exceeds 384 MiB".into(),
            ));
        }
    }
    let license = capsule
        .files
        .iter()
        .find(|file| file.path == capsule.license.path)
        .ok_or_else(|| WorkshopError::Invalid("workshop license file is missing".into()))?;
    if !LICENSE_NAMES.contains(&capsule.license.path.as_str())
        || license.content_sha256 != capsule.license.content_sha256
    {
        return Err(WorkshopError::Invalid(
            "workshop license declaration does not match its source file".into(),
        ));
    }
    Ok(())
}

fn workshop_authorization_statement(
    record: &ShotRecord,
    conformance: &ConformanceReport,
    metadata: &AppMetadataV2,
    license: &WorkshopLicense,
) -> Result<WorkshopAuthorizationStatement, WorkshopError> {
    Ok(WorkshopAuthorizationStatement {
        schema: WORKSHOP_AUTHORIZATION_SCHEMA.into(),
        record_commitment: record.commitment()?,
        conformance_sha256: canonical::sha256_commitment(conformance)?,
        metadata_sha256: canonical::sha256_commitment(metadata)?,
        shot_id: record.shot_id,
        builder_id: record.builder_id,
        expression_id: metadata.expression_id,
        version_id: metadata.version_id,
        version_ordinal: metadata.version_ordinal,
        source_tree_sha256: record.source_tree_sha256,
        license: license.clone(),
    })
}

fn validate_record_bindings(
    record: &ShotRecord,
    conformance: &ConformanceReport,
    metadata: &AppMetadataV2,
) -> Result<(), WorkshopError> {
    if !conformance.conformant
        || conformance.shot_id != record.shot_id
        || conformance.sequence != record.sequence
        || metadata.shot_id != record.shot_id
        || metadata.builder_id != record.builder_id
        || metadata.source_tree_sha256 != record.source_tree_sha256
        || metadata.fascia_sha256 != record.fascia_sha256
        || metadata.bundle_id != record.bundle_id
        || metadata.bundle_version != record.bundle_version
        || metadata.version_ordinal != u64::from(record.sequence)
    {
        return Err(WorkshopError::Invalid(
            "workshop records do not bind the same accepted Apple Version".into(),
        ));
    }
    Ok(())
}

fn read_source_file(
    evolution: &Evolution,
    entry: &SourceTreeEntry,
) -> Result<Vec<u8>, WorkshopError> {
    validate_relative_path(&entry.path)?;
    let path = evolution.source_path().join(Path::new(&entry.path));
    let bytes = read_bounded(&path, MAX_SOURCE_FILE_BYTES, "source file")?;
    if sha256(&bytes) != entry.content_sha256 {
        return Err(WorkshopError::Invalid(format!(
            "{} changed while the capsule was prepared",
            entry.path
        )));
    }
    Ok(bytes)
}

fn validate_relative_path(path: &str) -> Result<(), WorkshopError> {
    if path.is_empty() || path.len() > MAX_PATH_BYTES || path.starts_with('/') {
        return Err(WorkshopError::Invalid("unsafe workshop source path".into()));
    }
    let components = Path::new(path).components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(WorkshopError::Invalid(format!(
            "unsafe workshop source path: {path}"
        )));
    }
    Ok(())
}

fn validate_license_bytes(bytes: &[u8]) -> Result<(), WorkshopError> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        WorkshopError::Invalid("workshop license must be a nonempty UTF-8 text file".into())
    })?;
    if text.trim().is_empty() || text.chars().any(|character| character == '\0') {
        return Err(WorkshopError::Invalid(
            "workshop license must be a nonempty UTF-8 text file".into(),
        ));
    }
    Ok(())
}

fn workshop_root(path: &Path) -> Result<PathBuf, WorkshopError> {
    let direct = path.join(WORKSHOP_RECEIPT_FILE);
    if direct.is_file() {
        return Ok(path.to_path_buf());
    }
    if path.file_name().is_some_and(|name| name == "src") {
        if let Some(parent) = path.parent() {
            if parent.join(WORKSHOP_RECEIPT_FILE).is_file() {
                return Ok(parent.to_path_buf());
            }
        }
    }
    Err(WorkshopError::Invalid(format!(
        "{} is not a materialized TOHSENO workshop",
        path.display()
    )))
}

fn read_canonical<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, WorkshopError> {
    let bytes = read_bounded(path, MAX_CAPSULE_BYTES, "canonical record")?;
    canonical::from_slice(&bytes).map_err(WorkshopError::from)
}

fn read_bounded(path: &Path, maximum: u64, label: &str) -> Result<Vec<u8>, WorkshopError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err(WorkshopError::Limit(format!(
            "{label} must be one bounded regular file"
        )));
    }
    Ok(fs::read(path)?)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), WorkshopError> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn create_destination_parent(path: &Path) -> Result<(), WorkshopError> {
    let parent = path
        .parent()
        .ok_or_else(|| WorkshopError::Invalid("output path must have a parent directory".into()))?;
    fs::create_dir_all(parent)?;
    Ok(())
}

fn canonical_now() -> Result<CanonicalTimestamp, WorkshopError> {
    let now = OffsetDateTime::now_utc();
    CanonicalTimestamp::parse(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    ))
    .map_err(WorkshopError::from)
}

#[derive(Debug)]
pub enum WorkshopError {
    Io(std::io::Error),
    Protocol(tohseno_protocol::ProtocolError),
    Ledger(crate::ledger::LedgerError),
    Lifecycle(protocol_lifecycle::ProtocolLifecycleError),
    Build(build::BuildError),
    BuilderIdentity(BuilderIdentityError),
    Invalid(String),
    Limit(String),
}

impl std::fmt::Display for WorkshopError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Protocol(error) => write!(formatter, "{error}"),
            Self::Ledger(error) => write!(formatter, "{error}"),
            Self::Lifecycle(error) => write!(formatter, "{error}"),
            Self::Build(error) => write!(formatter, "{error}"),
            Self::BuilderIdentity(error) => write!(formatter, "{error}"),
            Self::Invalid(reason) | Self::Limit(reason) => write!(formatter, "{reason}"),
        }
    }
}

impl std::error::Error for WorkshopError {}

impl From<std::io::Error> for WorkshopError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<tohseno_protocol::ProtocolError> for WorkshopError {
    fn from(value: tohseno_protocol::ProtocolError) -> Self {
        Self::Protocol(value)
    }
}

impl From<crate::ledger::LedgerError> for WorkshopError {
    fn from(value: crate::ledger::LedgerError) -> Self {
        Self::Ledger(value)
    }
}

impl From<protocol_lifecycle::ProtocolLifecycleError> for WorkshopError {
    fn from(value: protocol_lifecycle::ProtocolLifecycleError) -> Self {
        Self::Lifecycle(value)
    }
}

impl From<build::BuildError> for WorkshopError {
    fn from(value: build::BuildError) -> Self {
        Self::Build(value)
    }
}

impl From<BuilderIdentityError> for WorkshopError {
    fn from(value: BuilderIdentityError) -> Self {
        Self::BuilderIdentity(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::hazmat::PrehashSigner;
    use p256::ecdsa::{Signature, SigningKey};
    use std::process::Command;
    use tohseno_protocol::app_metadata::{AppMetadata, AppMetadataV2};
    use tohseno_protocol::conformance::{CheckStatus, ConformanceCheck};
    use tohseno_protocol::digest::VersionId;
    use tohseno_protocol::fascia::FasciaManifest;
    use tohseno_protocol::fascia_tree::hash_fascia_tree;
    use tohseno_protocol::record::{
        FactoryDescriptor, APPLE_FASCIA_ID, PROTOCOL_NAME, SHOT_SCHEMA,
    };
    use tohseno_protocol::signature::{P256PublicKey, P256Signature, SignatureAlgorithm};

    #[test]
    fn source_capsule_materializes_without_private_ledger_or_publisher_binary() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source");
        fs::create_dir(&source).unwrap();
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/apple-expression/materialize.sh");
        let result = Command::new(fixture)
            .args([
                source.as_os_str(),
                "fixture".as_ref(),
                "com.example.fixture".as_ref(),
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        fs::write(source.join("LICENSE"), "Apache License 2.0 test fixture\n").unwrap();
        let fascia =
            crate::protocol_lifecycle::inspect_fascia(&source, "com.example.fixture", 1).unwrap();
        fs::write(
            source.join("TOHSENO/fascia.json"),
            canonical::to_vec(&fascia).unwrap(),
        )
        .unwrap();
        let tree = hash_source_tree(&source).unwrap();
        let capsule = fixture_capsule(&source, &tree.entries, tree.digest, &fascia);
        fs::write(
            source.join("TOHSENO/embedded-provenance.json"),
            canonical::to_vec(&capsule.metadata).unwrap(),
        )
        .unwrap();
        let capsule_path = temporary.path().join("fixture.tohseno-workshop");
        fs::write(&capsule_path, canonical::to_vec(&capsule).unwrap()).unwrap();
        let destination = temporary.path().join("guest");

        let materialized = materialize_workshop(&capsule_path, &destination).unwrap();
        assert_eq!(materialized.source, destination.join("src"));
        assert!(materialized.source.join("fixture.xcodeproj").is_dir());
        assert!(materialized.source.join("LICENSE").is_file());
        assert!(!destination.join(".tohseno").exists());
        assert!(!destination.join("artifact").exists());
        assert!(!destination.join("prompt.md").exists());
        assert!(!materialized.receipt.ownership_acquired);
        read_workshop_receipt(&destination).unwrap();

        let mut relayed = capsule.clone();
        relayed.metadata.expression_id = ExpressionId::from_bytes([0x77; 32]);
        relayed.metadata.version_id = VersionId::derive(
            relayed.metadata.shot_id,
            relayed.metadata.expression_id,
            relayed.metadata.version_ordinal,
            relayed.metadata.genome_digest,
            relayed.metadata.source_tree_sha256,
        );
        let relayed_path = temporary.path().join("relayed.tohseno-workshop");
        fs::write(&relayed_path, canonical::to_vec(&relayed).unwrap()).unwrap();
        assert!(materialize_workshop(&relayed_path, &temporary.path().join("relayed")).is_err());

        fs::write(
            materialized.source.join("TemplateApp.swift"),
            "// changed after receipt\n",
        )
        .unwrap();
        assert!(read_workshop_receipt(&destination).is_err());
    }

    #[test]
    fn workshop_feedback_is_exact_version_bound_and_self_declared() {
        let packet = WorkshopFeedbackPacket {
            schema: WORKSHOP_FEEDBACK_SCHEMA.into(),
            shot_id: ShotId::from_bytes([1; 32]),
            expression_id: ExpressionId::from_bytes([2; 32]),
            version_id: VersionId::from_bytes([3; 32]),
            version_ordinal: 4,
            author_display_name: Some("Maya".into()),
            text: "The save gesture was clear.".into(),
            observed_at: CanonicalTimestamp::parse("2026-08-04T12:00:00Z").unwrap(),
            author_authentication: "self_declared".into(),
        };
        packet.validate().unwrap();

        let mut forged = packet.clone();
        forged.author_authentication = "verified".into();
        assert!(forged.validate().is_err());
        let mut blank = packet;
        blank.text = " ".into();
        assert!(blank.validate().is_err());
    }

    fn fixture_capsule(
        source: &Path,
        entries: &[SourceTreeEntry],
        source_digest: Bytes32,
        fascia: &FasciaManifest,
    ) -> WorkshopCapsule {
        let signing = SigningKey::from_bytes((&[7_u8; 32]).into()).unwrap();
        let encoded = signing.verifying_key().to_encoded_point(false);
        let mut x = [0_u8; 32];
        let mut y = [0_u8; 32];
        x.copy_from_slice(encoded.x().unwrap());
        y.copy_from_slice(encoded.y().unwrap());
        let public_key = P256PublicKey {
            x: Bytes32::new(x),
            y: Bytes32::new(y),
        };
        let fascia_digest =
            hash_fascia_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../fascia/apple"))
                .unwrap()
                .digest;
        let record = ShotRecord {
            protocol: PROTOCOL_NAME.into(),
            schema: SHOT_SCHEMA.into(),
            shot_id: ShotId::from_bytes([0x11; 32]),
            slug: "fixture".into(),
            builder_id: crate::builder_identity::legacy_v07_initial_device_builder_id(&public_key)
                .unwrap(),
            sequence: 1,
            previous: None,
            fascia: APPLE_FASCIA_ID.into(),
            bundle_id: "com.example.fixture".into(),
            bundle_version: 1,
            genesis_input_sha256: Bytes32::new([0x22; 32]),
            source_tree_sha256: source_digest,
            fascia_sha256: fascia_digest,
            factory: FactoryDescriptor {
                implementation: crate::builder_identity::LEGACY_V07_FACTORY_IMPLEMENTATION.into(),
                version: "0.7.0".into(),
                source_commit: "a".repeat(40),
            },
            created_at: CanonicalTimestamp::parse("2026-08-04T12:00:00Z").unwrap(),
            origin: None,
        };
        let commitment = record.commitment().unwrap();
        let sidecar = sign_sidecar(&signing, public_key.clone(), commitment);
        let v1 = AppMetadata::for_record(&record, commitment, fascia).unwrap();
        let expression_id = ExpressionId::from_bytes([0x33; 32]);
        let genome_digest = Bytes32::new([0x44; 32]);
        let version_id = VersionId::derive(
            record.shot_id,
            expression_id,
            1,
            genome_digest,
            source_digest,
        );
        let metadata = AppMetadataV2::from_v1(
            &v1,
            expression_id,
            version_id,
            1,
            1,
            genome_digest,
            1,
            Bytes32::new([0x55; 32]),
            None,
        )
        .unwrap();
        let conformance = ConformanceReport {
            schema: tohseno_protocol::conformance::CONFORMANCE_SCHEMA.into(),
            shot_id: record.shot_id,
            sequence: 1,
            conformant: true,
            checks: vec![ConformanceCheck {
                id: "source.commitment".into(),
                status: CheckStatus::Pass,
                expected: source_digest.to_string(),
                observed: source_digest.to_string(),
                evidence_path: Some("src".into()),
            }],
        };
        let files = entries
            .iter()
            .map(|entry| {
                let bytes = fs::read(source.join(&entry.path)).unwrap();
                WorkshopFile {
                    path: entry.path.clone(),
                    content_sha256: entry.content_sha256,
                    byte_length: bytes.len() as u64,
                    contents_base64: BASE64_STANDARD.encode(bytes),
                }
            })
            .collect();
        let license_entry = entries
            .iter()
            .find(|entry| entry.path == "LICENSE")
            .unwrap();
        let license = WorkshopLicense {
            path: license_entry.path.clone(),
            content_sha256: license_entry.content_sha256,
        };
        let authorization_statement =
            workshop_authorization_statement(&record, &conformance, &metadata, &license).unwrap();
        let authorization_digest = canonical::sha256_commitment(&authorization_statement).unwrap();
        let authorization = sign_sidecar(&signing, public_key, authorization_digest);
        WorkshopCapsule {
            schema: WORKSHOP_CAPSULE_SCHEMA.into(),
            record,
            signature: sidecar,
            conformance,
            metadata,
            license,
            files,
            authorization,
        }
    }

    fn sign_sidecar(
        signing: &SigningKey,
        public_key: P256PublicKey,
        digest: Bytes32,
    ) -> SignatureSidecar {
        let signature: Signature = signing.sign_prehash(digest.as_bytes()).unwrap();
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
            public_key,
            signature: P256Signature {
                r: Bytes32::new(r),
                s: Bytes32::new(s),
            },
            low_s: true,
        }
    }
}
