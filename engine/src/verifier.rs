//! Bounded, offline verification of immutable Shot directories.
//!
//! This module deliberately accepts every filesystem root as an explicit
//! argument. It does not consult the network, environment, current directory,
//! Git, the Apple identity helper, or any mutable ledger state.

use crate::app_metadata_policy::validate_current_app_metadata_v2;
use crate::builder_identity::initial_device_builder_id_for_v1_factory;
#[cfg(test)]
use crate::builder_identity::{
    legacy_v07_initial_device_builder_id, LEGACY_V07_CANDIDATE_VERSION,
    LEGACY_V07_FACTORY_IMPLEMENTATION,
};
use crate::gates::build;
use crate::protocol_lifecycle::inspect_fascia;
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Number, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, Metadata};
use std::path::{Component, Path, PathBuf};
use tohseno_protocol::app_metadata::{AppMetadata, EmbeddedAppMetadata};
use tohseno_protocol::conformance::{
    CheckStatus as ReceiptStatus, ConformanceReport, CONFORMANCE_SCHEMA,
};
use tohseno_protocol::digest::Bytes32;
use tohseno_protocol::evolution::verify_lineage as verify_protocol_lineage;
use tohseno_protocol::fascia::{Capability, FasciaManifest, StorageKind, REQUIRED_FASCIA_FILES};
use tohseno_protocol::fascia_tree::{
    hash_fascia_tree, DEFAULT_FASCIA_EXCLUSIONS, PINNED_APPLE_FASCIA_SHA256,
};
#[cfg(test)]
use tohseno_protocol::record::FactoryDescriptor;
use tohseno_protocol::record::ShotRecord;
use tohseno_protocol::signature::SignatureSidecar;
use tohseno_protocol::tree_hash::{exclusion_reason, hash_source_tree};

pub const VERIFICATION_SCHEMA: &str = "tohseno.verification/1";
pub const LINEAGE_VERIFICATION_SCHEMA: &str = "tohseno.lineage-verification/1";

const MAX_JSON_BYTES: u64 = 1024 * 1024;
const MAX_PLIST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TREE_FILES: usize = 20_000;
const MAX_TREE_DEPTH: usize = 64;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_TREE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_RECEIPT_CHECKS: usize = 512;
const MAX_LINEAGE_LENGTH: usize = 4_096;
const MAX_REPORT_TEXT: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Pass,
    Fail,
    NotChecked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationCheck {
    pub id: String,
    pub status: VerificationStatus,
    pub expected: String,
    pub observed: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShotVerificationReport {
    pub schema: String,
    pub conformant: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evolution_commitment: Option<Bytes32>,
    pub checks: Vec<VerificationCheck>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageVerificationReport {
    pub schema: String,
    pub conformant: bool,
    pub shots: Vec<ShotVerificationReport>,
    pub lineage: VerificationCheck,
}

#[derive(Default)]
struct IndependentTruth {
    values: BTreeMap<String, bool>,
    artifact_present: bool,
}

/// Verify one completed Shot directory without requiring a build artifact.
///
/// `shot_root` is the immutable numbered Shot directory (the directory that
/// contains `src`, `TOHSENO`, and optionally `artifact`). `fascia_reference`
/// is the exact reusable Apple Fascia directory pinned by the candidate.
///
/// All expected failures are represented in the returned report. This
/// function does not panic for malformed or missing Shot data.
pub fn verify_shot_directory(shot_root: &Path, fascia_reference: &Path) -> ShotVerificationReport {
    verify_shot_directory_inner(shot_root, fascia_reference, true)
}

/// Runs the complete content and cryptographic verifier immediately before the
/// immutable completion marker is committed.
///
/// The only different observation is that `.complete` must still be absent.
pub(crate) fn verify_unsealed_shot_directory(
    shot_root: &Path,
    fascia_reference: &Path,
) -> ShotVerificationReport {
    verify_shot_directory_inner(shot_root, fascia_reference, false)
}

fn verify_shot_directory_inner(
    shot_root: &Path,
    fascia_reference: &Path,
    sealed: bool,
) -> ShotVerificationReport {
    let mut checks = Vec::new();
    let mut truth = IndependentTruth::default();

    let root_ok = check_directory_root(shot_root);
    checks.push(result_check(
        "layout.shot_root",
        "explicit non-symlink Shot directory",
        &root_ok,
        Some("."),
    ));
    let completion = if sealed {
        read_scoped_regular(shot_root, ".complete", 16).and_then(|bytes| {
            if bytes == b"complete\n" {
                Ok("immutable ledger completion marker verified".to_owned())
            } else {
                Err("completion marker must contain exact bytes \"complete\\n\"".into())
            }
        })
    } else {
        match fs::symlink_metadata(shot_root.join(".complete")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok("completion marker is correctly absent before commit".to_owned())
            }
            Ok(_) => Err("completion marker already exists before verifier preflight".into()),
            Err(error) => Err(format!("completion marker preflight failed: {error}")),
        }
    };
    checks.push(result_check(
        "layout.complete",
        if sealed {
            "exact immutable Shot completion marker"
        } else {
            "completion marker absent until all content checks pass"
        },
        &completion,
        Some(".complete"),
    ));

    let reference_ok = check_directory_root(fascia_reference);
    checks.push(result_check(
        "layout.fascia_reference",
        "explicit non-symlink pinned Fascia directory",
        &reference_ok,
        None,
    ));

    let record = load_validated_json::<ShotRecord, _>(shot_root, "TOHSENO/shot.json", |value| {
        value.validate().map_err(|error| error.to_string())
    });
    checks.push(result_check(
        "record.schema",
        "strict, closed, valid tohseno.shot/1 JSON",
        &record.as_ref().map(|_| "record validated".to_owned()),
        Some("TOHSENO/shot.json"),
    ));
    truth.values.insert("record.schema".into(), record.is_ok());
    let record = record.ok();

    let signature =
        load_validated_json::<SignatureSidecar, _>(shot_root, "TOHSENO/signature.json", |value| {
            value.validate().map_err(|error| error.to_string())
        });
    checks.push(result_check(
        "signature.schema",
        "strict, closed, valid tohseno.signature/1 JSON with low-s P-256 scalars",
        &signature
            .as_ref()
            .map(|_| "signature sidecar validated".to_owned()),
        Some("TOHSENO/signature.json"),
    ));
    let signature = signature.ok();

    let fascia =
        load_validated_json::<FasciaManifest, _>(shot_root, "TOHSENO/fascia.json", |value| {
            value.validate().map_err(|error| error.to_string())
        });
    checks.push(result_check(
        "fascia.manifest",
        "strict, closed, valid concrete tohseno.fascia/1 JSON",
        &fascia.as_ref().map(|_| "manifest validated".to_owned()),
        Some("TOHSENO/fascia.json"),
    ));
    truth
        .values
        .insert("fascia.manifest".into(), fascia.is_ok());
    let fascia = fascia.ok();

    let conformance = load_validated_json::<ConformanceReport, _>(
        shot_root,
        "TOHSENO/conformance.json",
        |value| {
            if value.checks.len() > MAX_RECEIPT_CHECKS {
                return Err(format!(
                    "receipt has {} checks; limit is {MAX_RECEIPT_CHECKS}",
                    value.checks.len()
                ));
            }
            value.validate().map_err(|error| error.to_string())
        },
    );
    checks.push(result_check(
        "conformance.schema",
        "strict, closed, internally consistent tohseno.conformance/1 JSON",
        &conformance
            .as_ref()
            .map(|_| "conformance receipt validated".to_owned()),
        Some("TOHSENO/conformance.json"),
    ));
    let conformance = conformance.ok();

    let commitment = match &record {
        Some(record) => record.commitment().map_err(|error| error.to_string()),
        None => Err("record was unavailable".into()),
    };
    checks.push(result_check(
        "record.commitment",
        "SHA-256 of RFC 8785 canonical Shot record bytes",
        &commitment
            .as_ref()
            .map(|digest| format!("recomputed {digest}")),
        Some("TOHSENO/shot.json"),
    ));
    let commitment = commitment.ok();

    let signature_valid = match (&record, &signature) {
        (Some(record), Some(signature)) => record
            .verify_signature(signature)
            .map(|_| "record digest and low-s P-256 signature verified".to_owned())
            .map_err(|error| error.to_string()),
        _ => Err("valid record and signature sidecar were unavailable".into()),
    };
    checks.push(result_check(
        "record.signature",
        "valid low-s P-256 signature over the recomputed record commitment",
        &signature_valid,
        Some("TOHSENO/signature.json"),
    ));
    truth
        .values
        .insert("record.signature".into(), signature_valid.is_ok());

    let device_authority = match (&record, &signature) {
        (Some(record), Some(signature)) => {
            initial_device_builder_id_for_v1_factory(&record.factory, &signature.public_key)
                .map_err(|error| error.to_string())
                .and_then(|predicted| {
                    if predicted == record.builder_id {
                        Ok(format!(
                            "signing DeviceKey reproduces BuilderID {} from the pinned factory",
                            record.builder_id
                        ))
                    } else {
                        Err(format!(
                            "signing DeviceKey controls {predicted}, not claimed BuilderID {}",
                            record.builder_id
                        ))
                    }
                })
        }
        _ => Err("valid record and signature sidecar were unavailable".into()),
    };
    checks.push(result_check(
        "record.device_authority",
        "record signer is the initial DeviceKey that deterministically controls the claimed BuilderID",
        &device_authority,
        Some("TOHSENO/signature.json"),
    ));
    truth
        .values
        .insert("record.device_authority".into(), device_authority.is_ok());

    let source_root = shot_root.join("src");
    let source_bound = audit_tree(&source_root, TreeKind::Source);
    checks.push(result_check(
        "limits.source_tree",
        "source tree within verifier file, byte, and depth limits",
        &source_bound,
        Some("src"),
    ));
    let source_tree = if source_bound.is_ok() {
        hash_source_tree(&source_root).map_err(|error| error.to_string())
    } else {
        Err("source tree exceeded verifier limits".into())
    };
    let source_match = match (&record, &source_tree) {
        (Some(record), Ok(tree)) if record.source_tree_sha256 == tree.digest => {
            Ok(format!("matched {}", tree.digest))
        }
        (Some(record), Ok(tree)) => Err(format!(
            "expected {}, observed {}",
            record.source_tree_sha256, tree.digest
        )),
        (_, Err(error)) => Err(error.clone()),
        (None, _) => Err("valid record was unavailable".into()),
    };
    checks.push(result_check(
        "source.commitment",
        "source tree digest equals shot.json source_tree_sha256",
        &source_match,
        Some("src"),
    ));
    truth
        .values
        .insert("source.commitment".into(), source_match.is_ok());
    let private_input_boundary = verify_private_input_boundary(shot_root);
    checks.push(result_check(
        "privacy.input_boundary",
        "raw private prompt and input-image bytes are absent from source and retained artifact",
        &private_input_boundary,
        Some("src"),
    ));
    truth.values.insert(
        "privacy.input_boundary".into(),
        private_input_boundary.is_ok(),
    );

    let fascia_bound = audit_tree(fascia_reference, TreeKind::Fascia);
    checks.push(result_check(
        "limits.fascia_tree",
        "reference Fascia tree within verifier file, byte, and depth limits",
        &fascia_bound,
        None,
    ));
    let fascia_tree = if fascia_bound.is_ok() {
        hash_fascia_tree(fascia_reference).map_err(|error| error.to_string())
    } else {
        Err("reference Fascia tree exceeded verifier limits".into())
    };
    let fascia_match = match (&record, &fascia_tree) {
        (Some(record), Ok(tree))
            if record.fascia_sha256 == tree.digest
                && tree.digest.to_string() == PINNED_APPLE_FASCIA_SHA256 =>
        {
            Ok(format!("matched {}", tree.digest))
        }
        (Some(record), Ok(tree)) => Err(format!(
            "record expected {}, candidate pins {}, observed {}",
            record.fascia_sha256, PINNED_APPLE_FASCIA_SHA256, tree.digest
        )),
        (_, Err(error)) => Err(error.clone()),
        (None, _) => Err("valid record was unavailable".into()),
    };
    checks.push(result_check(
        "fascia.commitment",
        "pinned reference Fascia digest equals shot.json fascia_sha256",
        &fascia_match,
        None,
    ));
    truth
        .values
        .insert("fascia.commitment".into(), fascia_match.is_ok());

    let fascia_binding: Result<String, String> = match (&record, &fascia) {
        (Some(record), Some(fascia))
            if fascia.fascia == record.fascia
                && fascia.distribution.bundle_id == record.bundle_id
                && fascia.distribution.bundle_version == record.bundle_version =>
        {
            Ok("Fascia, bundle identifier, and bundle version match the record".into())
        }
        (Some(record), Some(fascia)) => Err(format!(
            "record ({}, {}, {}) differs from manifest ({}, {}, {})",
            record.fascia,
            record.bundle_id,
            record.bundle_version,
            fascia.fascia,
            fascia.distribution.bundle_id,
            fascia.distribution.bundle_version
        )),
        _ => Err("valid record and Fascia manifest were unavailable".into()),
    };
    checks.push(result_check(
        "fascia.record_binding",
        "concrete Fascia public identity exactly matches shot.json",
        &fascia_binding,
        Some("TOHSENO/fascia.json"),
    ));

    let fascia_source_copy =
        load_validated_json::<FasciaManifest, _>(&source_root, "TOHSENO/fascia.json", |value| {
            value.validate().map_err(|error| error.to_string())
        });
    let fascia_copy_match: Result<String, String> = match (&fascia, fascia_source_copy) {
        (Some(expected), Ok(observed)) if expected == &observed => {
            Ok("bundled source manifest exactly matches the Shot manifest".into())
        }
        (Some(_), Ok(_)) => Err("bundled source manifest differs from Shot manifest".into()),
        (_, Err(error)) => Err(error),
        (None, _) => Err("valid Shot Fascia manifest was unavailable".into()),
    };
    checks.push(result_check(
        "fascia.source_copy",
        "src/TOHSENO/fascia.json equals TOHSENO/fascia.json",
        &fascia_copy_match,
        Some("src/TOHSENO/fascia.json"),
    ));
    let fascia_source_declarations: Result<String, String> = match (&record, &fascia) {
        (Some(record), Some(observed)) => {
            inspect_fascia(&source_root, &record.bundle_id, record.sequence)
                .map_err(|error| error.to_string())
                .and_then(|expected| {
                    if &expected == observed {
                        Ok("conservative source policy scan exactly matched the manifest".into())
                    } else {
                        Err("source policy scan produced different Fascia declarations".into())
                    }
                })
        }
        _ => Err("valid record and Fascia manifest were unavailable".into()),
    };
    checks.push(result_check(
        "fascia.source_declarations",
        "Fascia capabilities, storage, network, privacy, and surfaces exactly match the conservative source scan",
        &fascia_source_declarations,
        Some("src"),
    ));
    truth.values.insert(
        "fascia.source_declarations".into(),
        fascia_source_declarations.is_ok(),
    );

    let required_files = verify_required_files(shot_root, fascia_reference);
    checks.push(result_check(
        "fascia.required_files",
        "the generated Apple Fascia subtree is exactly the manifest-claimed regular-file inventory",
        &required_files
            .as_ref()
            .map(|count| format!("{count} required files verified")),
        Some("TOHSENO/fascia.json"),
    ));
    for path in REQUIRED_FASCIA_FILES {
        let id = format!("fascia.file.{}", check_token(path));
        truth.values.insert(
            id,
            required_file_exists(shot_root, fascia_reference, path).is_ok(),
        );
    }

    let anatomy = if source_bound.is_ok() {
        build::validate_complete_source(&source_root)
            .map(|_| "complete Xcode/Fascia source anatomy verified".to_owned())
            .map_err(|error| error.to_string())
    } else {
        Err("source tree exceeded verifier limits".into())
    };
    checks.push(result_check(
        "apple.anatomy",
        "complete Xcode project with target-member Fascia and provenance",
        &anatomy,
        Some("src"),
    ));
    truth.values.insert("apple.anatomy".into(), anatomy.is_ok());

    let source_facts = inspect_source_facts(&source_root, record.as_ref());
    checks.push(result_check(
        "apple.source_facts",
        "project bundle facts, dependency boundary, and private-material boundary",
        &source_facts
            .as_ref()
            .map(|facts| facts.summary(record.as_ref())),
        Some("src"),
    ));
    if let Ok(facts) = &source_facts {
        truth
            .values
            .insert("apple.dependencies".into(), !facts.third_party_dependency);
        truth
            .values
            .insert("privacy.boundary".into(), !facts.forbidden_secret_marker);
        if let Some(record) = &record {
            truth.values.insert(
                "apple.bundle_id".into(),
                facts.project_text.contains(&record.bundle_id),
            );
            truth.values.insert(
                "apple.bundle_version".into(),
                project_version_matches(&facts.project_text, record.sequence),
            );
        }
    }
    if let Some(fascia) = &fascia {
        let capabilities = fascia
            .capabilities
            .iter()
            .map(|value| value.capability)
            .collect::<BTreeSet<_>>();
        let storage = fascia
            .storage
            .iter()
            .map(|value| value.kind)
            .collect::<BTreeSet<_>>();
        truth.values.insert(
            "storage.local_first".into(),
            capabilities.contains(&Capability::LocalStorage)
                && !storage.contains(&StorageKind::PrivateCloudkit),
        );
    }

    let provenance = load_embedded_app_metadata(&source_root, "TOHSENO/embedded-provenance.json");
    checks.push(result_check(
        "provenance.schema",
        "strict, closed, valid supported embedded app-metadata JSON",
        &provenance
            .as_ref()
            .map(|_| "embedded provenance validated".to_owned()),
        Some("src/TOHSENO/embedded-provenance.json"),
    ));
    let provenance_binding = match (&record, &fascia, commitment, provenance.as_ref()) {
        (Some(record), Some(fascia), Some(commitment), Ok(provenance)) => {
            compare_provenance(provenance, record, fascia, commitment)
        }
        (_, _, _, Err(error)) => Err(error.clone()),
        _ => Err("valid record, commitment, and provenance were unavailable".into()),
    };
    checks.push(result_check(
        "provenance.binding",
        "embedded public facts exactly match the signed Shot record and commitment",
        &provenance_binding,
        Some("src/TOHSENO/embedded-provenance.json"),
    ));
    truth
        .values
        .insert("provenance.embedded".into(), provenance_binding.is_ok());

    let artifact = verify_artifact(shot_root, record.as_ref(), fascia.as_ref());
    truth.artifact_present = artifact.present;
    checks.extend(artifact.checks);
    for (id, value) in artifact.truth {
        truth.values.insert(id, value);
    }

    let receipt_binding: Result<String, String> = match (&record, &conformance) {
        (Some(record), Some(receipt))
            if receipt.schema == CONFORMANCE_SCHEMA
                && receipt.shot_id == record.shot_id
                && receipt.sequence == record.sequence
                && receipt.conformant
                && receipt
                    .checks
                    .iter()
                    .all(|check| check.status == ReceiptStatus::Pass) =>
        {
            Ok("receipt identity, sequence, and conformant state match".into())
        }
        (Some(record), Some(receipt)) => Err(format!(
            "record shot/sequence {}/{}; receipt shot/sequence {}/{}; conformant={}",
            record.shot_id, record.sequence, receipt.shot_id, receipt.sequence, receipt.conformant
        )),
        _ => Err("valid record and conformance receipt were unavailable".into()),
    };
    checks.push(result_check(
        "conformance.binding",
        "receipt identifies this Shot Evolution and contains only passing claims",
        &receipt_binding,
        Some("TOHSENO/conformance.json"),
    ));

    let receipt_evidence = match &conformance {
        Some(receipt) => {
            verify_receipt_evidence(shot_root, fascia_reference, receipt, truth.artifact_present)
        }
        None => Err("valid conformance receipt was unavailable".into()),
    };
    checks.push(result_check(
        "conformance.evidence",
        "every claimed evidence path is scoped, non-traversing, and present",
        &receipt_evidence,
        Some("TOHSENO/conformance.json"),
    ));

    let receipt_truth = match &conformance {
        Some(receipt) => verify_receipt_truth(receipt, &truth),
        None => Err("valid conformance receipt was unavailable".into()),
    };
    checks.push(result_check(
        "conformance.truth",
        "every receipt claim agrees with an independently recomputed check",
        &receipt_truth,
        Some("TOHSENO/conformance.json"),
    ));

    let conformant = checks
        .iter()
        .all(|check| check.status == VerificationStatus::Pass);
    ShotVerificationReport {
        schema: VERIFICATION_SCHEMA.into(),
        conformant,
        shot_id: record.as_ref().map(|value| value.shot_id.to_string()),
        sequence: record.as_ref().map(|value| value.sequence),
        evolution_commitment: commitment,
        checks,
    }
}

/// Verify an explicitly ordered lineage of completed Shot directories.
///
/// The pure protocol verifier decides whether the root is native sequence 1
/// or a declared legacy-adoption root. Per-Shot verification is still run for
/// every entry, so a valid record lineage cannot hide bad source evidence.
pub fn verify_lineage_directories(
    shot_roots: &[PathBuf],
    fascia_reference: &Path,
) -> LineageVerificationReport {
    if shot_roots.len() > MAX_LINEAGE_LENGTH {
        return LineageVerificationReport {
            schema: LINEAGE_VERIFICATION_SCHEMA.into(),
            conformant: false,
            shots: Vec::new(),
            lineage: fail(
                "lineage.chain",
                &format!("at most {MAX_LINEAGE_LENGTH} ordered Shot directories"),
                &format!("received {}", shot_roots.len()),
                None,
            ),
        };
    }

    let shots = shot_roots
        .iter()
        .map(|root| verify_shot_directory(root, fascia_reference))
        .collect::<Vec<_>>();
    let loaded = shot_roots
        .iter()
        .map(|root| load_record_and_signature(root))
        .collect::<Result<Vec<_>, _>>();
    let lineage = match loaded {
        Ok(loaded) => {
            let refs = loaded
                .iter()
                .map(|(record, signature)| (record, signature))
                .collect::<Vec<_>>();
            match verify_protocol_lineage(&refs) {
                Ok(verified) => pass(
                    "lineage.chain",
                    "pure-protocol contiguous signed lineage from native or adopted root",
                    &format!(
                        "{} Evolutions verified from sequence {}; legacy_latest_shot={}",
                        verified.evolutions.len(),
                        verified.root_sequence,
                        verified
                            .legacy_latest_shot
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "none".into())
                    ),
                    None,
                ),
                Err(error) => fail(
                    "lineage.chain",
                    "pure-protocol contiguous signed lineage from native or adopted root",
                    &error.to_string(),
                    None,
                ),
            }
        }
        Err(error) => fail(
            "lineage.chain",
            "strict valid shot.json and signature.json for every ordered directory",
            &error,
            None,
        ),
    };
    let conformant =
        lineage.status == VerificationStatus::Pass && shots.iter().all(|shot| shot.conformant);
    LineageVerificationReport {
        schema: LINEAGE_VERIFICATION_SCHEMA.into(),
        conformant,
        shots,
        lineage,
    }
}

fn load_record_and_signature(root: &Path) -> Result<(ShotRecord, SignatureSidecar), String> {
    let record = load_validated_json::<ShotRecord, _>(root, "TOHSENO/shot.json", |value| {
        value.validate().map_err(|error| error.to_string())
    })?;
    let signature =
        load_validated_json::<SignatureSidecar, _>(root, "TOHSENO/signature.json", |value| {
            value.validate().map_err(|error| error.to_string())
        })?;
    Ok((record, signature))
}

fn load_validated_json<T, F>(root: &Path, relative: &str, validate: F) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
    F: FnOnce(&T) -> Result<(), String>,
{
    let bytes = read_scoped_regular(root, relative, MAX_JSON_BYTES)?;
    let value = serde_json::from_slice::<UniqueJson>(&bytes)
        .map_err(|error| format!("{relative}: invalid strict JSON: {error}"))?;
    let decoded = serde_json::from_value::<T>(value.0)
        .map_err(|error| format!("{relative}: schema error: {error}"))?;
    validate(&decoded).map_err(|error| format!("{relative}: {error}"))?;
    Ok(decoded)
}

fn check_directory_root(path: &Path) -> Result<String, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", display_path(path)))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("{} is a symbolic link", display_path(path)));
    }
    if !metadata.is_dir() {
        return Err(format!("{} is not a directory", display_path(path)));
    }
    Ok("directory exists and is not a symlink".into())
}

fn read_scoped_regular(root: &Path, relative: &str, limit: u64) -> Result<Vec<u8>, String> {
    validate_relative_path(relative)?;
    check_directory_root(root)?;
    let path = checked_join(root, relative, false)?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| format!("{relative}: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("{relative}: symbolic links are forbidden"));
    }
    if !metadata.is_file() {
        return Err(format!("{relative}: expected a regular file"));
    }
    if metadata.len() > limit {
        return Err(format!(
            "{relative}: {} bytes exceeds the {limit}-byte limit",
            metadata.len()
        ));
    }
    let bytes = fs::read(&path).map_err(|error| format!("{relative}: {error}"))?;
    let final_metadata =
        fs::symlink_metadata(&path).map_err(|error| format!("{relative}: {error}"))?;
    if final_metadata.file_type().is_symlink()
        || !final_metadata.is_file()
        || !same_file(&metadata, &final_metadata)
        || bytes.len() as u64 != metadata.len()
    {
        return Err(format!("{relative}: file changed while it was read"));
    }
    Ok(bytes)
}

fn checked_join(root: &Path, relative: &str, allow_directory: bool) -> Result<PathBuf, String> {
    validate_relative_path(relative)?;
    let mut current = root.to_path_buf();
    let components = Path::new(relative).components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            return Err(format!("{relative}: path traversal is forbidden"));
        };
        current.push(component);
        let metadata =
            fs::symlink_metadata(&current).map_err(|error| format!("{relative}: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("{relative}: symbolic links are forbidden"));
        }
        let is_last = index + 1 == components.len();
        if !is_last && !metadata.is_dir() {
            return Err(format!("{relative}: path parent is not a directory"));
        }
        if is_last && allow_directory && !(metadata.is_dir() || metadata.is_file()) {
            return Err(format!("{relative}: unsupported filesystem entry"));
        }
    }
    Ok(current)
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
        || !Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!("{path:?}: expected a non-traversing relative path"));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum TreeKind {
    Source,
    Fascia,
    Artifact,
}

fn audit_tree(root: &Path, kind: TreeKind) -> Result<String, String> {
    check_directory_root(root)?;
    let mut stack = vec![(root.to_path_buf(), 0_usize)];
    let mut entries = 0_usize;
    let mut files = 0_usize;
    let mut bytes = 0_u64;

    while let Some((directory, depth)) = stack.pop() {
        if depth > MAX_TREE_DEPTH {
            return Err(format!(
                "tree depth {depth} exceeds the limit {MAX_TREE_DEPTH}"
            ));
        }
        let iterator = fs::read_dir(&directory)
            .map_err(|error| format!("{}: {error}", display_path(&directory)))?;
        for item in iterator {
            let item = item.map_err(|error| format!("{}: {error}", display_path(&directory)))?;
            entries = entries
                .checked_add(1)
                .ok_or_else(|| "tree entry count overflowed".to_owned())?;
            if entries > MAX_TREE_FILES {
                return Err(format!(
                    "tree has more than {MAX_TREE_FILES} filesystem entries"
                ));
            }
            let path = item.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("{}: {error}", display_path(&path)))?;
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "tree entry escaped its explicit root".to_owned())?;
            let normalized = normalized_relative(relative)?;
            if metadata.file_type().is_symlink() {
                return Err(format!("{normalized}: symbolic links are forbidden"));
            }
            if tree_excluded(&normalized, kind) {
                continue;
            }
            if metadata.is_dir() {
                stack.push((path, depth + 1));
            } else if metadata.is_file() {
                if metadata.len() > MAX_FILE_BYTES {
                    return Err(format!(
                        "{normalized}: {} bytes exceeds the per-file limit {MAX_FILE_BYTES}",
                        metadata.len()
                    ));
                }
                bytes = bytes
                    .checked_add(metadata.len())
                    .ok_or_else(|| "tree byte count overflowed".to_owned())?;
                if bytes > MAX_TREE_BYTES {
                    return Err(format!(
                        "tree content exceeds the {MAX_TREE_BYTES}-byte limit"
                    ));
                }
                files += 1;
            } else {
                return Err(format!(
                    "{normalized}: expected a regular file or directory"
                ));
            }
        }
    }
    Ok(format!(
        "{files} regular files and {bytes} bytes are within limits"
    ))
}

fn normalized_relative(path: &Path) -> Result<String, String> {
    let mut output = Vec::new();
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(format!(
                "{}: path traversal is forbidden",
                display_path(path)
            ));
        };
        let component = component
            .to_str()
            .ok_or_else(|| format!("{}: path is not UTF-8", display_path(path)))?;
        if component.is_empty() || matches!(component, "." | "..") {
            return Err(format!("{}: invalid path component", display_path(path)));
        }
        output.push(component);
    }
    let output = output.join("/");
    validate_relative_path(&output)?;
    Ok(output)
}

fn tree_excluded(path: &str, kind: TreeKind) -> bool {
    match kind {
        TreeKind::Source => exclusion_reason(path).is_some(),
        TreeKind::Fascia => DEFAULT_FASCIA_EXCLUSIONS.iter().any(|excluded| {
            path == *excluded
                || path
                    .strip_prefix(excluded)
                    .is_some_and(|rest| rest.starts_with('/'))
        }),
        TreeKind::Artifact => false,
    }
}

fn verify_required_files(shot_root: &Path, fascia_reference: &Path) -> Result<usize, String> {
    let mut failures = Vec::new();
    if let Err(error) = build::validate_fascia_source_inventory(&shot_root.join("src")) {
        failures.push(error.to_string());
    }
    for path in REQUIRED_FASCIA_FILES {
        if let Err(error) = required_file_exists(shot_root, fascia_reference, path) {
            failures.push(error);
        }
    }
    if failures.is_empty() {
        Ok(REQUIRED_FASCIA_FILES.len())
    } else {
        Err(failures.join("; "))
    }
}

fn required_file_exists(
    shot_root: &Path,
    _fascia_reference: &Path,
    relative: &str,
) -> Result<(), String> {
    let (root, relative) = if relative == "TOHSENO/embedded-provenance.json"
        || relative.starts_with("TohsenoFascia/")
    {
        (shot_root.join("src"), relative)
    } else {
        (shot_root.to_path_buf(), relative)
    };
    read_scoped_regular(&root, relative, MAX_FILE_BYTES).map(|_| ())
}

pub(crate) fn load_embedded_app_metadata(
    root: &Path,
    relative: &str,
) -> Result<EmbeddedAppMetadata, String> {
    let bytes = read_scoped_regular(root, relative, MAX_JSON_BYTES)?;
    EmbeddedAppMetadata::decode_transport_json(&bytes)
        .map_err(|error| format!("{relative}: {error}"))
}

fn compare_provenance(
    provenance: &EmbeddedAppMetadata,
    record: &ShotRecord,
    fascia: &FasciaManifest,
    commitment: Bytes32,
) -> Result<String, String> {
    if let EmbeddedAppMetadata::V2(metadata) = provenance {
        validate_current_app_metadata_v2(metadata)
            .map_err(|error| format!("embedded v2 provenance violates current policy: {error}"))?;
    }
    let expected = AppMetadata::for_record(record, commitment, fascia)
        .map_err(|error| format!("could not derive signed app metadata: {error}"))?;
    match provenance {
        EmbeddedAppMetadata::V1(provenance) if provenance == &expected => {
            Ok(format!("all v1 public facts matched commitment {commitment}"))
        }
        EmbeddedAppMetadata::V1(_) => Err(format!(
            "embedded v1 provenance does not exactly match commitment {commitment} and record public facts"
        )),
        EmbeddedAppMetadata::V2(provenance)
            if provenance.protocol_name == expected.protocol_name
                && provenance.fascia == expected.fascia
                && provenance.shot_id == expected.shot_id
                && provenance.builder_id == expected.builder_id
                && provenance.source_tree_sha256 == expected.source_tree_sha256
                && provenance.fascia_sha256 == expected.fascia_sha256
                && provenance.bundle_id == expected.bundle_id
                && provenance.bundle_version == expected.bundle_version
                && provenance.factory == expected.factory
                && provenance.distribution == expected.distribution
                && provenance.capabilities == expected.capabilities
                && provenance.network == expected.network
                && provenance.registry == expected.registry
                && provenance.legacy_v1_evolution_commitment == Some(commitment) =>
        {
            Ok(format!(
                "all shared v2 public facts and legacy v1 witness matched commitment {commitment}"
            ))
        }
        EmbeddedAppMetadata::V2(_) => Err(format!(
            "embedded v2 provenance does not match commitment {commitment}, its legacy v1 witness, and record public facts"
        )),
    }
}

struct SourceFacts {
    project_text: String,
    third_party_dependency: bool,
    forbidden_secret_marker: bool,
}

impl SourceFacts {
    fn summary(&self, record: Option<&ShotRecord>) -> String {
        let bundle = record
            .map(|record| self.project_text.contains(&record.bundle_id))
            .unwrap_or(false);
        let version = record
            .map(|record| project_version_matches(&self.project_text, record.sequence))
            .unwrap_or(false);
        format!(
            "bundle_id_match={bundle}, bundle_version_match={version}, third_party_dependency={}, forbidden_secret_marker={}",
            self.third_party_dependency, self.forbidden_secret_marker
        )
    }
}

pub(crate) fn verify_private_input_boundary(shot_root: &Path) -> Result<String, String> {
    let mut private_inputs = Vec::new();
    let prompt = read_scoped_regular(shot_root, "prompt.md", MAX_JSON_BYTES)?;
    if prompt.len() >= 16 {
        private_inputs.push(("prompt.md".to_owned(), prompt));
    }

    let images = checked_join(shot_root, "images", true)?;
    let image_metadata =
        fs::symlink_metadata(&images).map_err(|error| format!("images: {error}"))?;
    if image_metadata.file_type().is_symlink() || !image_metadata.is_dir() {
        return Err("images must be a non-symlink directory".into());
    }
    let mut entries = fs::read_dir(&images)
        .map_err(|error| format!("images: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("images: {error}"))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", display_path(&entry.path())))?;
        if file_type.is_symlink() || !file_type.is_file() {
            return Err(format!(
                "{}: input image must be a regular file",
                display_path(&entry.path())
            ));
        }
        let metadata = entry
            .metadata()
            .map_err(|error| format!("{}: {error}", display_path(&entry.path())))?;
        if metadata.len() > MAX_FILE_BYTES {
            return Err(format!(
                "{}: input image exceeds verifier limit",
                display_path(&entry.path())
            ));
        }
        let bytes = fs::read(entry.path())
            .map_err(|error| format!("{}: {error}", display_path(&entry.path())))?;
        if !bytes.is_empty() {
            private_inputs.push((entry.file_name().to_string_lossy().into_owned(), bytes));
        }
    }

    let mut scanned = 0_usize;
    for (root, kind) in [
        (shot_root.join("src"), TreeKind::Source),
        (shot_root.join("artifact"), TreeKind::Artifact),
    ] {
        match fs::symlink_metadata(&root) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("{}: {error}", display_path(&root))),
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(format!("{}: unsafe scan root", display_path(&root)))
            }
            Ok(_) => {}
        }
        audit_tree(&root, kind)?;
        scan_tree_for_private_inputs(&root, &root, &private_inputs, &mut scanned)?;
    }
    Ok(format!(
        "{} private input sequences absent across {scanned} files",
        private_inputs.len()
    ))
}

fn scan_tree_for_private_inputs(
    root: &Path,
    directory: &Path,
    private_inputs: &[(String, Vec<u8>)],
    scanned: &mut usize,
) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", display_path(directory)))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{}: {error}", display_path(directory)))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", display_path(&path)))?;
        if file_type.is_symlink() {
            return Err(format!(
                "{}: symbolic links are forbidden",
                display_path(&path)
            ));
        }
        if file_type.is_dir() {
            scan_tree_for_private_inputs(root, &path, private_inputs, scanned)?;
            continue;
        }
        if !file_type.is_file() {
            return Err(format!(
                "{}: unsupported filesystem entry",
                display_path(&path)
            ));
        }
        let metadata = entry
            .metadata()
            .map_err(|error| format!("{}: {error}", display_path(&path)))?;
        if metadata.len() > MAX_FILE_BYTES {
            return Err(format!(
                "{}: file exceeds verifier limit",
                display_path(&path)
            ));
        }
        let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", display_path(&path)))?;
        *scanned += 1;
        for (label, private) in private_inputs {
            if bytes
                .windows(private.len())
                .any(|window| window == private.as_slice())
            {
                let relative = path
                    .strip_prefix(root)
                    .map(display_path)
                    .unwrap_or_else(|_| display_path(&path));
                return Err(format!("{label} bytes appear in {relative}"));
            }
        }
    }
    Ok(())
}

fn inspect_source_facts(
    source_root: &Path,
    _record: Option<&ShotRecord>,
) -> Result<SourceFacts, String> {
    audit_tree(source_root, TreeKind::Source)?;
    let mut stack = vec![source_root.to_path_buf()];
    let mut projects = Vec::new();
    let mut third_party_dependency = false;
    let mut forbidden_secret_marker = false;

    while let Some(directory) = stack.pop() {
        let iterator = fs::read_dir(&directory)
            .map_err(|error| format!("{}: {error}", display_path(&directory)))?;
        for item in iterator {
            let item = item.map_err(|error| format!("{}: {error}", display_path(&directory)))?;
            let path = item.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("{}: {error}", display_path(&path)))?;
            let relative = normalized_relative(
                path.strip_prefix(source_root)
                    .map_err(|_| "source entry escaped its root".to_owned())?,
            )?;
            if metadata.file_type().is_symlink() {
                return Err(format!("{relative}: symbolic links are forbidden"));
            }
            if exclusion_reason(&relative).is_some() {
                continue;
            }
            if metadata.is_dir() {
                if path.extension().is_some_and(|value| value == "xcodeproj") {
                    projects.push(path.join("project.pbxproj"));
                }
                stack.push(path);
                continue;
            }
            if !metadata.is_file() {
                return Err(format!("{relative}: unsupported filesystem entry"));
            }
            if !is_inspectable_text_path(&path) {
                continue;
            }
            if metadata.len() > MAX_PLIST_BYTES {
                return Err(format!(
                    "{relative}: inspectable source exceeds the {MAX_PLIST_BYTES}-byte limit"
                ));
            }
            let bytes = fs::read(&path).map_err(|error| format!("{relative}: {error}"))?;
            let Ok(text) = std::str::from_utf8(&bytes) else {
                return Err(format!("{relative}: inspectable source is not UTF-8"));
            };
            third_party_dependency |= text.contains("XCRemoteSwiftPackageReference");
            forbidden_secret_marker |= [
                "recovery.json.enc",
                "BIP39",
                "BIP-39 mnemonic",
                "builder_recovery_mnemonic",
                "TOHSENO_RECOVERY_WORDS",
            ]
            .iter()
            .any(|marker| text.contains(marker));
        }
    }
    projects.sort();
    projects.dedup();
    if projects.len() != 1 {
        return Err(format!(
            "expected exactly one Xcode project, observed {}",
            projects.len()
        ));
    }
    let project = projects.remove(0);
    let relative = project
        .strip_prefix(source_root)
        .map_err(|_| "Xcode project escaped source root".to_owned())?;
    let project_text = String::from_utf8(read_scoped_regular(
        source_root,
        &normalized_relative(relative)?,
        MAX_PLIST_BYTES,
    )?)
    .map_err(|_| "project.pbxproj is not UTF-8".to_owned())?;
    Ok(SourceFacts {
        project_text,
        third_party_dependency,
        forbidden_secret_marker,
    })
}

fn is_inspectable_text_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name == "project.pbxproj" || name == "Package.swift")
        || path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension,
                    "swift" | "plist" | "entitlements" | "xcconfig" | "json"
                )
            })
}

fn project_version_matches(project: &str, sequence: u32) -> bool {
    project.lines().any(|line| {
        line.trim()
            .strip_prefix("CURRENT_PROJECT_VERSION = ")
            .and_then(|value| value.strip_suffix(';'))
            .is_some_and(|value| value.trim() == sequence.to_string())
    })
}

struct ArtifactVerification {
    present: bool,
    checks: Vec<VerificationCheck>,
    truth: BTreeMap<String, bool>,
}

fn verify_artifact(
    shot_root: &Path,
    record: Option<&ShotRecord>,
    fascia: Option<&FasciaManifest>,
) -> ArtifactVerification {
    let mut checks = Vec::new();
    let mut truth = BTreeMap::new();
    let artifact_root = shot_root.join("artifact");
    let root_metadata = match fs::symlink_metadata(&artifact_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            checks.push(not_checked(
                "artifact.info_plist",
                "matching bundle identifier and bundle version when an artifact is retained",
                "artifact directory is absent; source-only offline verification remains valid",
                Some("artifact"),
            ));
            return ArtifactVerification {
                present: false,
                checks,
                truth,
            };
        }
        Err(error) => {
            checks.push(fail(
                "artifact.info_plist",
                "optional artifact directory must be readable",
                &error.to_string(),
                Some("artifact"),
            ));
            return ArtifactVerification {
                present: true,
                checks,
                truth,
            };
        }
    };
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        checks.push(fail(
            "artifact.info_plist",
            "optional artifact root is a non-symlink directory",
            "artifact root is a symlink or unsupported entry",
            Some("artifact"),
        ));
        return ArtifactVerification {
            present: true,
            checks,
            truth,
        };
    }

    let Some(record) = record else {
        checks.push(not_checked(
            "artifact.info_plist",
            "matching bundle identifier and bundle version",
            "valid Shot record was unavailable",
            Some("artifact"),
        ));
        return ArtifactVerification {
            present: false,
            checks,
            truth,
        };
    };
    let app_relative = format!("{}.app", record.slug);
    let app_root = artifact_root.join(&app_relative);
    let app_metadata = match fs::symlink_metadata(&app_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let empty = fs::read_dir(&artifact_root)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false);
            let observed = if empty {
                "artifact directory is empty; source-only offline verification remains valid"
            } else {
                "expected application bundle is absent from a nonempty artifact directory"
            };
            checks.push(if empty {
                not_checked(
                    "artifact.info_plist",
                    "matching bundle identifier and bundle version when an artifact is retained",
                    observed,
                    Some("artifact"),
                )
            } else {
                fail(
                    "artifact.info_plist",
                    &format!("artifact/{app_relative} application bundle"),
                    observed,
                    Some("artifact"),
                )
            });
            return ArtifactVerification {
                present: !empty,
                checks,
                truth,
            };
        }
        Err(error) => {
            checks.push(fail(
                "artifact.info_plist",
                &format!("artifact/{app_relative} application bundle"),
                &error.to_string(),
                Some("artifact"),
            ));
            return ArtifactVerification {
                present: true,
                checks,
                truth,
            };
        }
    };
    if app_metadata.file_type().is_symlink() || !app_metadata.is_dir() {
        checks.push(fail(
            "artifact.info_plist",
            "non-symlink .app directory",
            "application bundle is a symlink or unsupported entry",
            Some("artifact"),
        ));
        return ArtifactVerification {
            present: true,
            checks,
            truth,
        };
    }
    let bound = audit_tree(&app_root, TreeKind::Artifact);
    if let Err(error) = bound {
        checks.push(fail(
            "artifact.limits",
            "artifact tree within verifier limits and free of symlinks",
            &error,
            Some("artifact"),
        ));
        return ArtifactVerification {
            present: true,
            checks,
            truth,
        };
    }
    let runtime_dependencies = verify_artifact_runtime_boundary(&app_root);
    checks.push(result_check(
        "artifact.runtime_dependencies",
        "no embedded frameworks, plug-ins, extensions, dynamic libraries, or executable service bundles",
        &runtime_dependencies,
        Some("artifact"),
    ));
    truth.insert(
        "artifact.runtime_dependencies".into(),
        runtime_dependencies.is_ok(),
    );

    let info = read_scoped_regular(&app_root, "Info.plist", MAX_PLIST_BYTES)
        .and_then(|bytes| parse_info_plist(&bytes));
    let info_matches = match info {
        Ok(info)
            if info.bundle_id == record.bundle_id
                && info.bundle_version == record.sequence.to_string() =>
        {
            Ok(format!(
                "bundle_id={}, bundle_version={}",
                info.bundle_id, info.bundle_version
            ))
        }
        Ok(info) => Err(format!(
            "expected {}/{}, observed {}/{}",
            record.bundle_id, record.sequence, info.bundle_id, info.bundle_version
        )),
        Err(error) => Err(error),
    };
    checks.push(result_check(
        "artifact.info_plist",
        "CFBundleIdentifier and CFBundleVersion match the signed record",
        &info_matches,
        Some("artifact"),
    ));
    truth.insert("artifact.bundle_id".into(), info_matches.is_ok());
    truth.insert("artifact.bundle_version".into(), info_matches.is_ok());

    let provenance = find_unique_named(&app_root, "embedded-provenance.json")
        .and_then(|path| {
            let path = path.ok_or_else(|| "embedded-provenance.json is missing".to_owned())?;
            let relative = normalized_relative(
                path.strip_prefix(&app_root)
                    .map_err(|_| "artifact provenance escaped app root".to_owned())?,
            )?;
            load_embedded_app_metadata(&app_root, &relative)
        })
        .and_then(|value| {
            let commitment = record.commitment().map_err(|error| error.to_string())?;
            let fascia =
                fascia.ok_or_else(|| "valid Shot Fascia manifest was unavailable".to_owned())?;
            compare_provenance(&value, record, fascia, commitment)
        });
    checks.push(result_check(
        "artifact.provenance",
        "one embedded provenance resource matching the signed record",
        &provenance,
        Some("artifact"),
    ));
    truth.insert("artifact.provenance".into(), provenance.is_ok());

    let artifact_fascia = find_unique_named(&app_root, "fascia.json").and_then(|path| {
        let path = path.ok_or_else(|| "fascia.json is missing".to_owned())?;
        let relative = normalized_relative(
            path.strip_prefix(&app_root)
                .map_err(|_| "artifact Fascia escaped app root".to_owned())?,
        )?;
        load_validated_json::<FasciaManifest, _>(&app_root, &relative, |value| {
            value.validate().map_err(|error| error.to_string())
        })
    });
    let fascia_match: Result<String, String> = match (fascia, artifact_fascia) {
        (Some(expected), Ok(observed)) if expected == &observed => {
            Ok("embedded concrete Fascia matches the Shot manifest".into())
        }
        (Some(_), Ok(_)) => Err("embedded Fascia differs from Shot manifest".into()),
        (_, Err(error)) => Err(error),
        (None, _) => Err("valid Shot Fascia manifest was unavailable".into()),
    };
    checks.push(result_check(
        "artifact.fascia",
        "one embedded concrete Fascia resource matching the Shot manifest",
        &fascia_match,
        Some("artifact"),
    ));
    truth.insert("artifact.fascia".into(), fascia_match.is_ok());

    ArtifactVerification {
        present: true,
        checks,
        truth,
    }
}

pub(crate) fn verify_artifact_runtime_boundary(app_root: &Path) -> Result<String, String> {
    let mut stack = vec![app_root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| format!("{}: {error}", display_path(&directory)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("{}: {error}", display_path(&directory)))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|error| format!("{}: {error}", display_path(&path)))?;
            if file_type.is_symlink() {
                return Err(format!(
                    "{}: symbolic links are forbidden",
                    display_path(&path)
                ));
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if file_type.is_dir() {
                if matches!(
                    name.as_ref(),
                    "Frameworks" | "PlugIns" | "XPCServices" | "AppClips" | "Watch"
                ) || path.extension().is_some_and(|extension| {
                    matches!(extension.to_str(), Some("framework" | "appex" | "xpc"))
                }) {
                    return Err(format!(
                        "{}: embedded executable/runtime container is unsupported",
                        display_path(&path)
                    ));
                }
                stack.push(path);
            } else if !file_type.is_file() {
                return Err(format!(
                    "{}: unsupported filesystem entry",
                    display_path(&path)
                ));
            } else if path
                .extension()
                .is_some_and(|extension| matches!(extension.to_str(), Some("dylib" | "so")))
            {
                return Err(format!(
                    "{}: embedded dynamic library is unsupported",
                    display_path(&path)
                ));
            }
        }
    }
    Ok("artifact contains no embedded runtime dependency containers".into())
}

fn find_unique_named(root: &Path, filename: &str) -> Result<Option<PathBuf>, String> {
    audit_tree(root, TreeKind::Artifact)?;
    let mut stack = vec![root.to_path_buf()];
    let mut found = None;
    while let Some(directory) = stack.pop() {
        for item in fs::read_dir(&directory)
            .map_err(|error| format!("{}: {error}", display_path(&directory)))?
        {
            let item = item.map_err(|error| format!("{}: {error}", display_path(&directory)))?;
            let metadata = item
                .file_type()
                .map_err(|error| format!("{}: {error}", display_path(&item.path())))?;
            if metadata.is_symlink() {
                return Err(format!(
                    "{}: symbolic links are forbidden",
                    display_path(&item.path())
                ));
            }
            if metadata.is_dir() {
                stack.push(item.path());
            } else if metadata.is_file() && item.file_name() == filename {
                if found.is_some() {
                    return Err(format!("artifact contains more than one {filename}"));
                }
                found = Some(item.path());
            }
        }
    }
    Ok(found)
}

fn verify_receipt_evidence(
    shot_root: &Path,
    fascia_reference: &Path,
    receipt: &ConformanceReport,
    artifact_present: bool,
) -> Result<String, String> {
    let mut verified = 0_usize;
    let mut deferred_artifact = 0_usize;
    for check in &receipt.checks {
        let Some(path) = &check.evidence_path else {
            return Err(format!("receipt check {} has no evidence_path", check.id));
        };
        validate_relative_path(path)
            .map_err(|error| format!("receipt check {}: {error}", check.id))?;
        if check.id.starts_with("artifact.") && !artifact_present {
            deferred_artifact += 1;
            continue;
        }
        evidence_exists(shot_root, fascia_reference, path)
            .map_err(|error| format!("receipt check {}: {error}", check.id))?;
        verified += 1;
    }
    Ok(format!(
        "{verified} evidence paths verified; {deferred_artifact} optional artifact paths deferred"
    ))
}

fn evidence_exists(shot_root: &Path, fascia_reference: &Path, path: &str) -> Result<(), String> {
    if path == "fascia/apple" {
        return check_directory_root(fascia_reference).map(|_| ());
    }
    let (root, relative) =
        if path == "TOHSENO/embedded-provenance.json" || path.starts_with("TohsenoFascia/") {
            (shot_root.join("src"), path)
        } else {
            (shot_root.to_path_buf(), path)
        };
    let resolved = checked_join(&root, relative, true)?;
    let metadata = fs::symlink_metadata(&resolved).map_err(|error| format!("{path}: {error}"))?;
    if metadata.file_type().is_symlink() || !(metadata.is_file() || metadata.is_dir()) {
        return Err(format!(
            "{path}: evidence must be a regular file or directory"
        ));
    }
    Ok(())
}

fn verify_receipt_truth(
    receipt: &ConformanceReport,
    truth: &IndependentTruth,
) -> Result<String, String> {
    let required = required_receipt_ids();
    let observed = receipt
        .checks
        .iter()
        .map(|check| check.id.as_str())
        .collect::<BTreeSet<_>>();
    let missing = required
        .iter()
        .filter(|id| !observed.contains(id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "receipt omits required independently checkable claims: {}",
            missing.join(", ")
        ));
    }

    let mut matched = 0_usize;
    let mut deferred_artifact = 0_usize;
    for check in &receipt.checks {
        if check.id.starts_with("artifact.") && !truth.artifact_present {
            deferred_artifact += 1;
            continue;
        }
        let Some(independent) = truth.values.get(&check.id) else {
            return Err(format!(
                "receipt contains unsupported claim id {}; its truth cannot be recomputed",
                check.id
            ));
        };
        let claimed = check.status == ReceiptStatus::Pass;
        if claimed != *independent {
            return Err(format!(
                "receipt claim {} says {:?}, independent result is {}",
                check.id, check.status, independent
            ));
        }
        if !independent {
            return Err(format!(
                "independent recomputation failed receipt claim {}",
                check.id
            ));
        }
        matched += 1;
    }
    Ok(format!(
        "{matched} receipt claims independently matched; {deferred_artifact} optional artifact claims deferred"
    ))
}

fn required_receipt_ids() -> BTreeSet<String> {
    let mut ids = [
        "record.schema",
        "fascia.manifest",
        "source.commitment",
        "fascia.commitment",
        "fascia.source_declarations",
        "apple.anatomy",
        "apple.dependencies",
        "privacy.boundary",
        "privacy.input_boundary",
        "storage.local_first",
        "apple.bundle_id",
        "apple.bundle_version",
        "record.signature",
        "record.device_authority",
        "artifact.runtime_dependencies",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    for path in REQUIRED_FASCIA_FILES {
        if !matches!(*path, "TOHSENO/signature.json" | "TOHSENO/conformance.json") {
            ids.insert(format!("fascia.file.{}", check_token(path)));
        }
    }
    ids
}

fn check_token(path: &str) -> String {
    path.bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() {
                byte.to_ascii_lowercase() as char
            } else {
                '-'
            }
        })
        .collect()
}

fn result_check<T, E>(
    id: &str,
    expected: &str,
    result: &Result<T, E>,
    evidence: Option<&str>,
) -> VerificationCheck
where
    T: fmt::Display,
    E: fmt::Display,
{
    match result {
        Ok(observed) => pass(id, expected, &observed.to_string(), evidence),
        Err(observed) => fail(id, expected, &observed.to_string(), evidence),
    }
}

fn pass(id: &str, expected: &str, observed: &str, evidence: Option<&str>) -> VerificationCheck {
    verification_check(id, VerificationStatus::Pass, expected, observed, evidence)
}

fn fail(id: &str, expected: &str, observed: &str, evidence: Option<&str>) -> VerificationCheck {
    verification_check(id, VerificationStatus::Fail, expected, observed, evidence)
}

fn not_checked(
    id: &str,
    expected: &str,
    observed: &str,
    evidence: Option<&str>,
) -> VerificationCheck {
    verification_check(
        id,
        VerificationStatus::NotChecked,
        expected,
        observed,
        evidence,
    )
}

fn verification_check(
    id: &str,
    status: VerificationStatus,
    expected: &str,
    observed: &str,
    evidence: Option<&str>,
) -> VerificationCheck {
    VerificationCheck {
        id: bounded_text(id),
        status,
        expected: bounded_text(expected),
        observed: bounded_text(observed),
        evidence_path: evidence.map(bounded_text),
    }
}

fn bounded_text(value: &str) -> String {
    let mut output = value.chars().take(MAX_REPORT_TEXT).collect::<String>();
    if output.len() < value.len() {
        output.push('…');
    }
    output
}

fn display_path(path: &Path) -> String {
    bounded_text(&path.display().to_string())
}

#[cfg(unix)]
fn same_file(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn same_file(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.file_type() == right.file_type()
}

/// A serde_json value visitor that rejects duplicate keys at every depth
/// before a protocol struct is decoded.
struct UniqueJson(Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

struct UniqueJsonVisitor;

impl<'de> Visitor<'de> for UniqueJsonVisitor {
    type Value = UniqueJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(Number::from(value))))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(Number::from(value))))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Number::from_f64(value)
            .map(Value::Number)
            .map(UniqueJson)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::String(value.into())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        UniqueJson::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(1024));
        while let Some(value) = sequence.next_element::<UniqueJson>()? {
            values.push(value.0);
        }
        Ok(UniqueJson(Value::Array(values)))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(<A::Error as serde::de::Error>::custom(format!(
                    "duplicate object key {key:?}"
                )));
            }
            let value = object.next_value::<UniqueJson>()?;
            values.insert(key, value.0);
        }
        Ok(UniqueJson(Value::Object(values)))
    }
}

struct PlistInfo {
    bundle_id: String,
    bundle_version: String,
}

fn parse_info_plist(bytes: &[u8]) -> Result<PlistInfo, String> {
    if bytes.starts_with(b"bplist00") {
        let plist = BinaryPlist::parse(bytes)?;
        return Ok(PlistInfo {
            bundle_id: plist.top_dictionary_value("CFBundleIdentifier")?,
            bundle_version: plist.top_dictionary_value("CFBundleVersion")?,
        });
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "Info.plist is neither XML nor bplist00".to_owned())?;
    Ok(PlistInfo {
        bundle_id: xml_plist_value(text, "CFBundleIdentifier")?,
        bundle_version: xml_plist_value(text, "CFBundleVersion")?,
    })
}

fn xml_plist_value(text: &str, key: &str) -> Result<String, String> {
    if text.contains("<!--") || text.contains("<![CDATA[") || text.contains("<!ENTITY") {
        return Err("Info.plist XML comments, CDATA, and custom entities are unsupported".into());
    }
    let mut cursor = 0_usize;
    let mut container_depth = 0_usize;
    let mut root_dictionary_seen = false;
    let mut found = Vec::new();
    while let Some(relative_start) = text[cursor..].find('<') {
        let start = cursor + relative_start;
        let relative_end = text[start..]
            .find('>')
            .ok_or_else(|| "Info.plist contains an unterminated XML tag".to_owned())?;
        let end = start + relative_end;
        let tag = text[start + 1..end].trim();
        cursor = end + 1;
        match tag {
            "dict" | "array" => {
                container_depth = container_depth
                    .checked_add(1)
                    .ok_or_else(|| "Info.plist nesting overflowed".to_owned())?;
                if tag == "dict" && container_depth == 1 {
                    if root_dictionary_seen {
                        return Err("Info.plist contains more than one root dictionary".into());
                    }
                    root_dictionary_seen = true;
                }
            }
            "/dict" | "/array" => {
                container_depth = container_depth
                    .checked_sub(1)
                    .ok_or_else(|| "Info.plist has an unmatched closing container".to_owned())?;
            }
            "key" if container_depth == 1 => {
                let closing = text[cursor..]
                    .find("</key>")
                    .ok_or_else(|| "Info.plist contains an unterminated key".to_owned())?;
                let raw_key = &text[cursor..cursor + closing];
                if raw_key.contains('<') {
                    return Err("Info.plist key contains nested markup".into());
                }
                cursor += closing + "</key>".len();
                if decode_xml_entities(raw_key)? == key {
                    found.push(xml_scalar_after(text, cursor, key)?);
                }
            }
            _ => {}
        }
    }
    if !root_dictionary_seen || container_depth != 0 {
        return Err("Info.plist has no balanced root dictionary".into());
    }
    if found.len() != 1 {
        return Err(format!(
            "Info.plist must contain exactly one {key}; observed {}",
            found.len()
        ));
    }
    Ok(found.remove(0))
}

fn xml_scalar_after(text: &str, cursor: usize, key: &str) -> Result<String, String> {
    let rest = text[cursor..].trim_start();
    for (opening, closing) in [("<string>", "</string>"), ("<integer>", "</integer>")] {
        if let Some(rest) = rest.strip_prefix(opening) {
            let end = rest
                .find(closing)
                .ok_or_else(|| format!("Info.plist {key} has no closing tag"))?;
            let value = decode_xml_entities(&rest[..end])?;
            if value.is_empty() || value.chars().any(char::is_control) {
                return Err(format!("Info.plist {key} is empty or contains controls"));
            }
            return Ok(value);
        }
    }
    Err(format!(
        "Info.plist {key} must be a string or integer value"
    ))
}

fn decode_xml_entities(value: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut rest = value;
    while let Some(index) = rest.find('&') {
        output.push_str(&rest[..index]);
        rest = &rest[index..];
        let end = rest
            .find(';')
            .ok_or_else(|| "Info.plist contains an unterminated XML entity".to_owned())?;
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            value if value.starts_with("#x") => {
                let code = u32::from_str_radix(&value[2..], 16)
                    .map_err(|_| "Info.plist contains an invalid XML entity".to_owned())?;
                char::from_u32(code)
                    .ok_or_else(|| "Info.plist contains an invalid XML scalar".to_owned())?
            }
            value if value.starts_with('#') => {
                let code = value[1..]
                    .parse::<u32>()
                    .map_err(|_| "Info.plist contains an invalid XML entity".to_owned())?;
                char::from_u32(code)
                    .ok_or_else(|| "Info.plist contains an invalid XML scalar".to_owned())?
            }
            _ => return Err(format!("Info.plist contains unsupported entity &{entity};")),
        };
        output.push(decoded);
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

struct BinaryPlist<'a> {
    bytes: &'a [u8],
    offsets: Vec<usize>,
    reference_size: usize,
    top_object: usize,
}

impl<'a> BinaryPlist<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() < 40 || !bytes.starts_with(b"bplist00") {
            return Err("invalid binary Info.plist header or trailer".into());
        }
        let trailer = &bytes[bytes.len() - 32..];
        let offset_size = usize::from(trailer[6]);
        let reference_size = usize::from(trailer[7]);
        if !(1..=8).contains(&offset_size) || !(1..=8).contains(&reference_size) {
            return Err("binary Info.plist has unsupported integer widths".into());
        }
        let object_count = usize_from_be(&trailer[8..16])?;
        let top_object = usize_from_be(&trailer[16..24])?;
        let offset_table = usize_from_be(&trailer[24..32])?;
        if object_count == 0
            || object_count > 100_000
            || top_object >= object_count
            || offset_table < 8
        {
            return Err("binary Info.plist trailer is out of range".into());
        }
        let table_bytes = object_count
            .checked_mul(offset_size)
            .ok_or_else(|| "binary Info.plist offset table overflowed".to_owned())?;
        let table_end = offset_table
            .checked_add(table_bytes)
            .ok_or_else(|| "binary Info.plist offset table overflowed".to_owned())?;
        if table_end > bytes.len() - 32 {
            return Err("binary Info.plist offset table is truncated".into());
        }
        let mut offsets = Vec::with_capacity(object_count);
        for index in 0..object_count {
            let start = offset_table + index * offset_size;
            let offset = usize_from_be(&bytes[start..start + offset_size])?;
            if !(8..offset_table).contains(&offset) {
                return Err("binary Info.plist object offset is out of range".into());
            }
            offsets.push(offset);
        }
        Ok(Self {
            bytes,
            offsets,
            reference_size,
            top_object,
        })
    }

    fn top_dictionary_value(&self, key: &str) -> Result<String, String> {
        let offset = self.object_offset(self.top_object)?;
        let marker = self.bytes[offset];
        if marker >> 4 != 0x0d {
            return Err("binary Info.plist top object is not a dictionary".into());
        }
        let (count, payload) = self.object_length(offset, marker & 0x0f)?;
        if count > 100_000 {
            return Err("binary Info.plist dictionary is too large".into());
        }
        let refs_bytes = count
            .checked_mul(self.reference_size)
            .ok_or_else(|| "binary Info.plist dictionary overflowed".to_owned())?;
        let values_start = payload
            .checked_add(refs_bytes)
            .ok_or_else(|| "binary Info.plist dictionary overflowed".to_owned())?;
        let end = values_start
            .checked_add(refs_bytes)
            .ok_or_else(|| "binary Info.plist dictionary overflowed".to_owned())?;
        if end > self.bytes.len() - 32 {
            return Err("binary Info.plist dictionary is truncated".into());
        }
        let mut found = None;
        for index in 0..count {
            let key_start = payload + index * self.reference_size;
            let value_start = values_start + index * self.reference_size;
            let key_ref = usize_from_be(&self.bytes[key_start..key_start + self.reference_size])?;
            let value_ref =
                usize_from_be(&self.bytes[value_start..value_start + self.reference_size])?;
            if self.object_string(key_ref)? == key {
                if found.is_some() {
                    return Err(format!("binary Info.plist contains duplicate key {key}"));
                }
                found = Some(self.object_scalar_string(value_ref)?);
            }
        }
        found.ok_or_else(|| format!("binary Info.plist has no {key}"))
    }

    fn object_offset(&self, reference: usize) -> Result<usize, String> {
        self.offsets
            .get(reference)
            .copied()
            .ok_or_else(|| "binary Info.plist object reference is out of range".to_owned())
    }

    fn object_string(&self, reference: usize) -> Result<String, String> {
        let offset = self.object_offset(reference)?;
        let marker = self.bytes[offset];
        let kind = marker >> 4;
        let (count, payload) = self.object_length(offset, marker & 0x0f)?;
        match kind {
            0x05 => {
                let end = payload
                    .checked_add(count)
                    .ok_or_else(|| "binary Info.plist string overflowed".to_owned())?;
                let value = self
                    .bytes
                    .get(payload..end)
                    .ok_or_else(|| "binary Info.plist string is truncated".to_owned())?;
                std::str::from_utf8(value)
                    .map(str::to_owned)
                    .map_err(|_| "binary Info.plist ASCII string is invalid".to_owned())
            }
            0x06 => {
                let byte_count = count
                    .checked_mul(2)
                    .ok_or_else(|| "binary Info.plist UTF-16 string overflowed".to_owned())?;
                let end = payload
                    .checked_add(byte_count)
                    .ok_or_else(|| "binary Info.plist UTF-16 string overflowed".to_owned())?;
                let value = self
                    .bytes
                    .get(payload..end)
                    .ok_or_else(|| "binary Info.plist UTF-16 string is truncated".to_owned())?;
                let units = value
                    .chunks_exact(2)
                    .map(|pair| u16::from_be_bytes([pair[0], pair[1]]));
                char::decode_utf16(units)
                    .collect::<Result<String, _>>()
                    .map_err(|_| "binary Info.plist UTF-16 string is invalid".to_owned())
            }
            _ => Err("binary Info.plist dictionary key is not a string".into()),
        }
    }

    fn object_scalar_string(&self, reference: usize) -> Result<String, String> {
        let offset = self.object_offset(reference)?;
        let marker = self.bytes[offset];
        match marker >> 4 {
            0x01 => {
                let width = 1_usize
                    .checked_shl(u32::from(marker & 0x0f))
                    .ok_or_else(|| "binary Info.plist integer width overflowed".to_owned())?;
                if width == 0 || width > 16 {
                    return Err("binary Info.plist integer width is unsupported".into());
                }
                let start = offset + 1;
                let end = start
                    .checked_add(width)
                    .ok_or_else(|| "binary Info.plist integer overflowed".to_owned())?;
                let bytes = self
                    .bytes
                    .get(start..end)
                    .ok_or_else(|| "binary Info.plist integer is truncated".to_owned())?;
                let mut value = 0_u128;
                for byte in bytes {
                    value = (value << 8) | u128::from(*byte);
                }
                Ok(value.to_string())
            }
            0x05 | 0x06 => self.object_string(reference),
            _ => Err("binary Info.plist value is not a string or integer".into()),
        }
    }

    fn object_length(&self, offset: usize, nibble: u8) -> Result<(usize, usize), String> {
        if nibble < 0x0f {
            return Ok((usize::from(nibble), offset + 1));
        }
        let marker_offset = offset
            .checked_add(1)
            .ok_or_else(|| "binary Info.plist length overflowed".to_owned())?;
        let marker = *self
            .bytes
            .get(marker_offset)
            .ok_or_else(|| "binary Info.plist extended length is truncated".to_owned())?;
        if marker >> 4 != 0x01 {
            return Err("binary Info.plist extended length is not an integer".into());
        }
        let width = 1_usize
            .checked_shl(u32::from(marker & 0x0f))
            .ok_or_else(|| "binary Info.plist length width overflowed".to_owned())?;
        if width == 0 || width > 8 {
            return Err("binary Info.plist length width is unsupported".into());
        }
        let start = marker_offset + 1;
        let end = start
            .checked_add(width)
            .ok_or_else(|| "binary Info.plist length overflowed".to_owned())?;
        let bytes = self
            .bytes
            .get(start..end)
            .ok_or_else(|| "binary Info.plist extended length is truncated".to_owned())?;
        Ok((usize_from_be(bytes)?, end))
    }
}

fn usize_from_be(bytes: &[u8]) -> Result<usize, String> {
    if bytes.is_empty() || bytes.len() > 8 {
        return Err("integer width is unsupported".into());
    }
    let mut value = 0_u64;
    for byte in bytes {
        value = (value << 8) | u64::from(*byte);
    }
    usize::try_from(value).map_err(|_| "integer does not fit this platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::hazmat::PrehashSigner;
    use p256::ecdsa::{Signature, SigningKey};
    use tempfile::TempDir;
    use tohseno_protocol::conformance::{ConformanceCheck, ConformanceReport};
    use tohseno_protocol::digest::ShotId;
    use tohseno_protocol::record::{
        CanonicalTimestamp, APPLE_FASCIA_ID, PROTOCOL_NAME, SHOT_SCHEMA,
    };
    use tohseno_protocol::signature::{P256PublicKey, P256Signature, SignatureAlgorithm};

    struct Fixture {
        _temporary: TempDir,
        shot: PathBuf,
        reference: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let shot = temporary.path().join("shot");
            let reference = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fascia/apple");
            fs::create_dir_all(shot.join("TOHSENO")).unwrap();
            fs::create_dir_all(shot.join("images")).unwrap();
            fs::create_dir_all(shot.join("src/TOHSENO")).unwrap();
            fs::create_dir_all(shot.join("src/TohsenoFascia")).unwrap();
            fs::create_dir_all(shot.join("src/Test.xcodeproj")).unwrap();
            fs::create_dir_all(shot.join("artifact")).unwrap();
            assert!(reference.is_dir());
            fs::write(shot.join(".complete"), "complete\n").unwrap();
            fs::write(
                shot.join("prompt.md"),
                "build a calm private fixture application",
            )
            .unwrap();
            for document in [
                "FASCIA.md",
                "IDENTITY.md",
                "STORAGE.md",
                "CONTINUITY.md",
                "PRIVACY.md",
                "PROVENANCE.md",
                "DISTRIBUTION.md",
            ] {
                fs::write(
                    shot.join("TOHSENO").join(document),
                    format!("# {document}\n"),
                )
                .unwrap();
            }
            for (source, bytes) in [
                (
                    "InstallationIdentity.swift",
                    include_bytes!("../../fascia/apple/swift/InstallationIdentity.swift")
                        .as_slice(),
                ),
                (
                    "ContinuityEnvelope.swift",
                    include_bytes!("../../fascia/apple/swift/ContinuityEnvelope.swift").as_slice(),
                ),
                (
                    "LocalPersistence.swift",
                    include_bytes!("../../fascia/apple/swift/LocalPersistence.swift").as_slice(),
                ),
                (
                    "Provenance.swift",
                    include_bytes!("../../fascia/apple/swift/Provenance.swift").as_slice(),
                ),
                (
                    "TohsenoMetadata.swift",
                    include_bytes!("../../fascia/apple/swift/TohsenoMetadata.swift").as_slice(),
                ),
            ] {
                fs::write(shot.join("src/TohsenoFascia").join(source), bytes).unwrap();
            }
            fs::write(
                shot.join("src/App.swift"),
                "func prepareIdentity() async throws {\n    try await InstallationIdentity.shared.prepare()\n}\n",
            )
            .unwrap();
            fs::write(
                shot.join("src/Test.xcodeproj/project.pbxproj"),
                r#"
PBXFileSystemSynchronizedRootGroup
TohsenoFascia
TOHSENO
PRODUCT_BUNDLE_IDENTIFIER = com.example.fixture;
CURRENT_PROJECT_VERSION = 1;
"#,
            )
            .unwrap();

            let manifest = inspect_fascia(&shot.join("src"), "com.example.fixture", 1).unwrap();
            write_json(&shot.join("TOHSENO/fascia.json"), &manifest);
            write_json(&shot.join("src/TOHSENO/fascia.json"), &manifest);
            fs::write(shot.join("src/TOHSENO/embedded-provenance.json"), "{}\n").unwrap();

            let source_tree = hash_source_tree(&shot.join("src")).unwrap();
            let fascia_tree = hash_fascia_tree(&reference).unwrap();
            let signing_key = SigningKey::from_bytes((&[1_u8; 32]).into()).unwrap();
            let encoded = signing_key.verifying_key().to_encoded_point(false);
            let mut x = [0_u8; 32];
            let mut y = [0_u8; 32];
            x.copy_from_slice(encoded.x().unwrap());
            y.copy_from_slice(encoded.y().unwrap());
            let public_key = P256PublicKey {
                x: Bytes32::new(x),
                y: Bytes32::new(y),
            };
            let record = ShotRecord {
                protocol: PROTOCOL_NAME.into(),
                schema: SHOT_SCHEMA.into(),
                shot_id: ShotId::from_bytes([0x11; 32]),
                slug: "fixture".into(),
                builder_id: legacy_v07_initial_device_builder_id(&public_key).unwrap(),
                sequence: 1,
                previous: None,
                fascia: APPLE_FASCIA_ID.into(),
                bundle_id: "com.example.fixture".into(),
                bundle_version: 1,
                genesis_input_sha256: Bytes32::new([0x33; 32]),
                source_tree_sha256: source_tree.digest,
                fascia_sha256: fascia_tree.digest,
                factory: FactoryDescriptor {
                    implementation: LEGACY_V07_FACTORY_IMPLEMENTATION.into(),
                    version: LEGACY_V07_CANDIDATE_VERSION.into(),
                    source_commit: "a".repeat(40),
                },
                created_at: CanonicalTimestamp::parse("2026-07-28T00:00:00Z").unwrap(),
                origin: None,
            };
            let commitment = record.commitment().unwrap();
            let signature: Signature = signing_key.sign_prehash(commitment.as_bytes()).unwrap();
            let signature = signature.normalize_s().unwrap_or(signature);
            let signature_bytes = signature.to_bytes();
            let mut r = [0_u8; 32];
            let mut s = [0_u8; 32];
            r.copy_from_slice(&signature_bytes[..32]);
            s.copy_from_slice(&signature_bytes[32..]);
            let sidecar = SignatureSidecar {
                schema: SignatureSidecar::SCHEMA.into(),
                algorithm: SignatureAlgorithm::P256,
                digest: commitment,
                public_key,
                signature: P256Signature {
                    r: Bytes32::new(r),
                    s: Bytes32::new(s),
                },
                low_s: true,
            };
            let provenance = AppMetadata::for_record(&record, commitment, &manifest).unwrap();
            write_json(&shot.join("TOHSENO/shot.json"), &record);
            write_json(&shot.join("TOHSENO/signature.json"), &sidecar);
            write_json(
                &shot.join("src/TOHSENO/embedded-provenance.json"),
                &provenance,
            );

            let checks = required_receipt_ids()
                .into_iter()
                .map(|id| ConformanceCheck {
                    evidence_path: Some(evidence_for_claim(&id)),
                    id,
                    status: ReceiptStatus::Pass,
                    expected: "independently true".into(),
                    observed: "independently true".into(),
                })
                .collect();
            let receipt = ConformanceReport {
                schema: CONFORMANCE_SCHEMA.into(),
                shot_id: record.shot_id,
                sequence: 1,
                conformant: true,
                checks,
            };
            write_json(&shot.join("TOHSENO/conformance.json"), &receipt);

            Self {
                _temporary: temporary,
                shot,
                reference,
            }
        }
    }

    fn evidence_for_claim(id: &str) -> String {
        match id {
            "record.schema" => "TOHSENO/shot.json".into(),
            "record.signature" => "TOHSENO/signature.json".into(),
            "fascia.manifest" => "TOHSENO/fascia.json".into(),
            "fascia.commitment" => "fascia/apple".into(),
            value if value.starts_with("artifact.") => "artifact".into(),
            value if value.starts_with("fascia.file.") => {
                let token = value.trim_start_matches("fascia.file.");
                REQUIRED_FASCIA_FILES
                    .iter()
                    .find(|path| check_token(path) == token)
                    .unwrap()
                    .to_string()
            }
            _ => "src".into(),
        }
    }

    fn write_json(path: &Path, value: &impl Serialize) {
        let mut bytes = serde_json::to_vec_pretty(value).unwrap();
        bytes.push(b'\n');
        fs::write(path, bytes).unwrap();
    }

    fn status(report: &ShotVerificationReport, id: &str) -> VerificationStatus {
        report
            .checks
            .iter()
            .find(|check| check.id == id)
            .unwrap()
            .status
    }

    fn retain_xml_artifact(fixture: &Fixture, bundle_id: &str, bundle_version: &str) {
        let app = fixture.shot.join("artifact/fixture.app");
        fs::create_dir_all(&app).unwrap();
        fs::write(
            app.join("Info.plist"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>{bundle_id}</string>
  <key>CFBundleVersion</key>
  <string>{bundle_version}</string>
</dict>
</plist>
"#
            ),
        )
        .unwrap();
        fs::copy(
            fixture.shot.join("src/TOHSENO/embedded-provenance.json"),
            app.join("embedded-provenance.json"),
        )
        .unwrap();
        fs::copy(
            fixture.shot.join("TOHSENO/fascia.json"),
            app.join("fascia.json"),
        )
        .unwrap();
    }

    #[test]
    fn source_only_fixture_is_inspectable_but_not_conformant() {
        let fixture = Fixture::new();
        let report = verify_shot_directory(&fixture.shot, &fixture.reference);
        assert!(!report.conformant);
        assert_eq!(
            status(&report, "artifact.info_plist"),
            VerificationStatus::NotChecked
        );
    }

    #[test]
    fn v1_builder_authority_rejects_unknown_and_next_factory_provenance() {
        for (implementation, version) in [
            ("unknown/factory", LEGACY_V07_CANDIDATE_VERSION),
            (LEGACY_V07_FACTORY_IMPLEMENTATION, "next"),
        ] {
            let fixture = Fixture::new();
            let path = fixture.shot.join("TOHSENO/shot.json");
            let mut record: ShotRecord = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            record.factory.implementation = implementation.into();
            record.factory.version = version.into();
            write_json(&path, &record);

            let report = verify_shot_directory(&fixture.shot, &fixture.reference);
            assert!(!report.conformant);
            assert_eq!(
                status(&report, "record.device_authority"),
                VerificationStatus::Fail
            );
            let authority = report
                .checks
                .iter()
                .find(|check| check.id == "record.device_authority")
                .unwrap();
            assert!(
                authority
                    .observed
                    .contains("unsupported v1 BuilderID provenance"),
                "{}",
                authority.observed
            );
        }
    }

    #[test]
    fn retained_artifact_info_plist_and_resources_are_verified() {
        let fixture = Fixture::new();
        retain_xml_artifact(&fixture, "com.example.fixture", "1");
        let report = verify_shot_directory(&fixture.shot, &fixture.reference);
        assert!(
            report.conformant,
            "{}",
            serde_json::to_string_pretty(&report).unwrap()
        );
        assert_eq!(
            status(&report, "artifact.info_plist"),
            VerificationStatus::Pass
        );
        assert_eq!(
            status(&report, "artifact.provenance"),
            VerificationStatus::Pass
        );
        assert_eq!(status(&report, "artifact.fascia"), VerificationStatus::Pass);
    }

    #[test]
    fn v2_embedded_metadata_preserves_and_verifies_the_v1_public_witness() {
        let fixture = Fixture::new();
        let record: ShotRecord =
            serde_json::from_slice(&fs::read(fixture.shot.join("TOHSENO/shot.json")).unwrap())
                .unwrap();
        let fascia: FasciaManifest =
            serde_json::from_slice(&fs::read(fixture.shot.join("TOHSENO/fascia.json")).unwrap())
                .unwrap();
        let commitment = record.commitment().unwrap();
        let v1 = AppMetadata::for_record(&record, commitment, &fascia).unwrap();
        let expression_id = tohseno_protocol::digest::ExpressionId::from_bytes([0x44; 32]);
        let genome_digest = Bytes32::new([0x55; 32]);
        let version_id = tohseno_protocol::digest::VersionId::derive(
            record.shot_id,
            expression_id,
            1,
            genome_digest,
            record.source_tree_sha256,
        );
        let v2 = tohseno_protocol::app_metadata::AppMetadataV2::from_v1(
            &v1,
            expression_id,
            version_id,
            1,
            1,
            genome_digest,
            7,
            Bytes32::new([0x66; 32]),
            None,
        )
        .unwrap();
        write_json(
            &fixture.shot.join("src/TOHSENO/embedded-provenance.json"),
            &v2,
        );
        retain_xml_artifact(&fixture, "com.example.fixture", "1");
        let report = verify_shot_directory(&fixture.shot, &fixture.reference);
        assert!(
            report.conformant,
            "{}",
            serde_json::to_string_pretty(&report).unwrap()
        );
        assert_eq!(
            status(&report, "provenance.binding"),
            VerificationStatus::Pass
        );
        assert_eq!(
            status(&report, "artifact.provenance"),
            VerificationStatus::Pass
        );

        let mut untrusted_registry_claim = v2.clone();
        untrusted_registry_claim.registry = Some(
            tohseno_protocol::app_metadata::AppMetadataRegistryReference {
                chain_id: 4_663,
                contract: tohseno_protocol::digest::Address20::from_bytes([0x66; 20]),
                transaction: Some(Bytes32::new([0x77; 32])),
            },
        );
        untrusted_registry_claim.validate().unwrap();
        write_json(
            &fixture.shot.join("src/TOHSENO/embedded-provenance.json"),
            &untrusted_registry_claim,
        );
        fs::copy(
            fixture.shot.join("src/TOHSENO/embedded-provenance.json"),
            fixture
                .shot
                .join("artifact/fixture.app/embedded-provenance.json"),
        )
        .unwrap();
        let report = verify_shot_directory(&fixture.shot, &fixture.reference);
        assert!(!report.conformant);
        assert_eq!(
            status(&report, "provenance.binding"),
            VerificationStatus::Fail
        );
        assert_eq!(
            status(&report, "artifact.provenance"),
            VerificationStatus::Fail
        );

        let mut forged = v2;
        forged.legacy_v1_evolution_commitment = Some(Bytes32::new([0x77; 32]));
        write_json(
            &fixture.shot.join("src/TOHSENO/embedded-provenance.json"),
            &forged,
        );
        fs::copy(
            fixture.shot.join("src/TOHSENO/embedded-provenance.json"),
            fixture
                .shot
                .join("artifact/fixture.app/embedded-provenance.json"),
        )
        .unwrap();
        let report = verify_shot_directory(&fixture.shot, &fixture.reference);
        assert!(!report.conformant);
        assert_eq!(
            status(&report, "provenance.binding"),
            VerificationStatus::Fail
        );
        assert_eq!(
            status(&report, "artifact.provenance"),
            VerificationStatus::Fail
        );
    }

    #[test]
    fn retained_artifact_bundle_mismatch_is_nonconformant() {
        let fixture = Fixture::new();
        retain_xml_artifact(&fixture, "com.example.wrong", "1");
        let report = verify_shot_directory(&fixture.shot, &fixture.reference);
        assert!(!report.conformant);
        assert_eq!(
            status(&report, "artifact.info_plist"),
            VerificationStatus::Fail
        );
    }

    #[test]
    fn wrong_source_tree_is_nonconformant() {
        let fixture = Fixture::new();
        fs::write(fixture.shot.join("src/App.swift"), "tampered\n").unwrap();
        let report = verify_shot_directory(&fixture.shot, &fixture.reference);
        assert!(!report.conformant);
        assert_eq!(
            status(&report, "source.commitment"),
            VerificationStatus::Fail
        );
    }

    #[test]
    fn unclaimed_fascia_source_is_nonconformant() {
        let fixture = Fixture::new();
        fs::write(
            fixture.shot.join("src/TohsenoFascia/UnclaimedPolicy.swift"),
            "let alternateLaw = true\n",
        )
        .unwrap();
        let report = verify_shot_directory(&fixture.shot, &fixture.reference);
        assert!(!report.conformant);
        assert_eq!(
            status(&report, "fascia.required_files"),
            VerificationStatus::Fail
        );
        assert_eq!(
            status(&report, "fascia.source_declarations"),
            VerificationStatus::Fail
        );
    }

    #[test]
    fn raw_private_prompt_bytes_in_source_are_rejected() {
        let fixture = Fixture::new();
        let prompt = fs::read(fixture.shot.join("prompt.md")).unwrap();
        fs::write(fixture.shot.join("src/PromptLeak.txt"), prompt).unwrap();
        let report = verify_shot_directory(&fixture.shot, &fixture.reference);
        assert!(!report.conformant);
        assert_eq!(
            status(&report, "privacy.input_boundary"),
            VerificationStatus::Fail
        );
    }

    #[test]
    fn record_tampering_breaks_the_sidecar_signature() {
        let fixture = Fixture::new();
        let path = fixture.shot.join("TOHSENO/shot.json");
        let mut record: ShotRecord = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        record.slug = "tampered".into();
        write_json(&path, &record);
        let report = verify_shot_directory(&fixture.shot, &fixture.reference);
        assert!(!report.conformant);
        assert_eq!(
            status(&report, "record.signature"),
            VerificationStatus::Fail
        );
    }

    #[test]
    fn sidecar_digest_tampering_is_rejected() {
        let fixture = Fixture::new();
        let path = fixture.shot.join("TOHSENO/signature.json");
        let mut sidecar: SignatureSidecar =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        sidecar.digest = Bytes32::new([0x99; 32]);
        write_json(&path, &sidecar);
        let report = verify_shot_directory(&fixture.shot, &fixture.reference);
        assert!(!report.conformant);
        assert_eq!(
            status(&report, "record.signature"),
            VerificationStatus::Fail
        );
    }

    #[test]
    fn cryptographically_valid_unproven_replacement_key_is_not_authority() {
        let fixture = Fixture::new();
        let record: ShotRecord =
            serde_json::from_slice(&fs::read(fixture.shot.join("TOHSENO/shot.json")).unwrap())
                .unwrap();
        let commitment = record.commitment().unwrap();
        let replacement = SigningKey::from_bytes((&[2_u8; 32]).into()).unwrap();
        let encoded = replacement.verifying_key().to_encoded_point(false);
        let mut x = [0_u8; 32];
        let mut y = [0_u8; 32];
        x.copy_from_slice(encoded.x().unwrap());
        y.copy_from_slice(encoded.y().unwrap());
        let signature: Signature = replacement.sign_prehash(commitment.as_bytes()).unwrap();
        let signature = signature.normalize_s().unwrap_or(signature);
        let signature_bytes = signature.to_bytes();
        let mut r = [0_u8; 32];
        let mut s = [0_u8; 32];
        r.copy_from_slice(&signature_bytes[..32]);
        s.copy_from_slice(&signature_bytes[32..]);
        let sidecar = SignatureSidecar {
            schema: SignatureSidecar::SCHEMA.into(),
            algorithm: SignatureAlgorithm::P256,
            digest: commitment,
            public_key: P256PublicKey {
                x: Bytes32::new(x),
                y: Bytes32::new(y),
            },
            signature: P256Signature {
                r: Bytes32::new(r),
                s: Bytes32::new(s),
            },
            low_s: true,
        };
        write_json(&fixture.shot.join("TOHSENO/signature.json"), &sidecar);

        let report = verify_shot_directory(&fixture.shot, &fixture.reference);
        assert_eq!(
            status(&report, "record.signature"),
            VerificationStatus::Pass,
            "the replacement signature itself must be valid for this test"
        );
        assert_eq!(
            status(&report, "record.device_authority"),
            VerificationStatus::Fail,
            "a valid signature without an authorization proof must fail closed"
        );
        assert!(!report.conformant);
    }

    #[test]
    fn receipt_path_traversal_is_rejected_without_reading_it() {
        let fixture = Fixture::new();
        let path = fixture.shot.join("TOHSENO/conformance.json");
        let mut receipt: ConformanceReport =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        receipt.checks[0].evidence_path = Some("../outside".into());
        write_json(&path, &receipt);
        let report = verify_shot_directory(&fixture.shot, &fixture.reference);
        assert!(!report.conformant);
        assert_eq!(
            status(&report, "conformance.schema"),
            VerificationStatus::Fail
        );
    }

    #[test]
    fn one_shot_directory_is_a_valid_native_lineage() {
        let fixture = Fixture::new();
        retain_xml_artifact(&fixture, "com.example.fixture", "1");
        let report = verify_lineage_directories(&[fixture.shot.clone()], &fixture.reference);
        assert!(report.conformant);
        assert_eq!(report.lineage.status, VerificationStatus::Pass);
    }
}
