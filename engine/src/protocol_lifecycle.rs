//! Engine integration for signed Shot Evolutions.
//!
//! This module owns orchestration and evidence paths. Exact byte laws remain
//! in `tohseno-protocol`.

use crate::builder_identity::{
    resolve_initial_device_builder_id, BuilderIdentity, BuilderIdentityError,
    BuilderIdentityManager,
};
use crate::gates::build;
use crate::ledger::{AppRecord, Evolution, Ledger, LedgerError};
use crate::shot_layout::ShotLayout;
use crate::verifier;
use bip39::{Language, Mnemonic};
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tohseno_protocol::app_metadata::{AppMetadata, AppMetadataV2, EmbeddedAppMetadata};
use tohseno_protocol::conformance::{
    CheckStatus, ConformanceCheck, ConformanceReport, CONFORMANCE_SCHEMA,
};
use tohseno_protocol::digest::Bytes32;
use tohseno_protocol::fascia::{
    AppleSurface, Capability, CapabilityDeclaration, DistributionDeclaration, DistributionState,
    FasciaManifest, InstallationIdentityDeclaration, PrivacyDeclaration, StorageDeclaration,
    StorageKind, FASCIA_SCHEMA, REQUIRED_FASCIA_FILES,
};
use tohseno_protocol::fascia_tree::{hash_fascia_tree, PINNED_APPLE_FASCIA_SHA256};
use tohseno_protocol::genesis::{genesis_image, genesis_input_sha256};
use tohseno_protocol::record::{
    CanonicalTimestamp, FactoryDescriptor, ShotOrigin, ShotRecord, APPLE_FASCIA_ID, PROTOCOL_NAME,
    SHOT_SCHEMA,
};
use tohseno_protocol::signature::SignatureSidecar;
use tohseno_protocol::tree_hash::hash_source_tree;

const FACTORY_IMPLEMENTATION: &str = "jpfraneto/tohseno";
const CANDIDATE_VERSION: &str = "0.7.0";

#[derive(Clone, Debug)]
pub struct PreparedEvolution {
    pub record: ShotRecord,
    pub fascia: FasciaManifest,
    pub commitment: Bytes32,
    provenance: AppMetadata,
    provenance_v2: Option<AppMetadataV2>,
}

#[derive(Clone, Debug)]
pub struct CompletedEvolution {
    pub record: ShotRecord,
    pub signature: SignatureSidecar,
    pub conformance: ConformanceReport,
    pub commitment: Bytes32,
    pub app_metadata_v2: Option<AppMetadataV2>,
}

pub fn prepare_evolution(
    ledger: &Ledger,
    shot: &Evolution,
    app: &AppRecord,
    builder: &BuilderIdentity,
    expected_genesis_input_sha256: Bytes32,
    origin: Option<ShotOrigin>,
) -> Result<PreparedEvolution, ProtocolLifecycleError> {
    builder.validate()?;
    let shot_id = app.shot_id.ok_or_else(|| {
        ProtocolLifecycleError::InvalidState("app has no permanent ShotID".into())
    })?;
    let builder_id = app
        .builder_id
        .ok_or_else(|| ProtocolLifecycleError::InvalidState("app has no bound BuilderID".into()))?;
    if builder_id != builder.builder_id {
        return Err(ProtocolLifecycleError::InvalidState(
            "app BuilderID differs from this local builder".into(),
        ));
    }
    if shot.number == 0 || app.bundle_id.is_empty() {
        return Err(ProtocolLifecycleError::InvalidState(
            "shot sequence or bundle identifier is invalid".into(),
        ));
    }

    let genesis_input_sha256 = capture_input_commitment(shot)?;
    if genesis_input_sha256 != expected_genesis_input_sha256 {
        return Err(ProtocolLifecycleError::InvalidState(
            "recorded prompt or input images changed while the harness was running".into(),
        ));
    }
    let fascia = inspect_fascia(&shot.source_path(), &app.bundle_id, shot.number)?;
    fascia
        .validate()
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
    write_json(ledger, shot, "src/TOHSENO/fascia.json", &fascia)?;
    let fascia_sha256 = reference_fascia_commitment()?;
    let source_tree_sha256 = hash_source_tree(&shot.source_path())
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?
        .digest;
    let previous = previous_commitment(ledger, shot, origin.as_ref())?;
    let created_at = canonical_now()?;
    let version = if env!("TOHSENO_SOURCE_DIRTY") == "1" {
        format!("{CANDIDATE_VERSION}+working-tree")
    } else {
        CANDIDATE_VERSION.into()
    };
    let record = ShotRecord {
        protocol: PROTOCOL_NAME.into(),
        schema: SHOT_SCHEMA.into(),
        shot_id,
        // v1's `slug` is also the Xcode product name. Keep it stable when
        // the enclosing Shot folder's mutable display name changes.
        slug: app.target_name().into(),
        builder_id,
        sequence: shot.number,
        previous,
        fascia: APPLE_FASCIA_ID.into(),
        bundle_id: app.bundle_id.clone(),
        bundle_version: shot.number,
        genesis_input_sha256,
        source_tree_sha256,
        fascia_sha256,
        factory: FactoryDescriptor {
            implementation: FACTORY_IMPLEMENTATION.into(),
            version,
            source_commit: env!("TOHSENO_SOURCE_COMMIT").into(),
        },
        created_at,
        origin,
    };
    let commitment = record
        .commitment()
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;

    write_json(ledger, shot, "TOHSENO/fascia.json", &fascia)?;
    write_json(ledger, shot, "TOHSENO/shot.json", &record)?;
    let provenance = AppMetadata::for_record(&record, commitment, &fascia)
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
    write_json(
        ledger,
        shot,
        "src/TOHSENO/embedded-provenance.json",
        &provenance,
    )?;

    Ok(PreparedEvolution {
        record,
        fascia,
        commitment,
        provenance,
        provenance_v2: None,
    })
}

/// Replace the frozen v1 transport metadata with the v2 expression/version
/// identity that the accepted materialization will use. The v1 record remains
/// signed compatibility evidence; the one bundled resource is schema-disjoint.
#[allow(clippy::too_many_arguments)]
pub fn bind_v2_app_metadata(
    ledger: &Ledger,
    shot: &Evolution,
    prepared: &mut PreparedEvolution,
    expression_id: tohseno_protocol::digest::ExpressionId,
    version_id: tohseno_protocol::digest::VersionId,
    version_ordinal: u64,
    genome_revision: u64,
    genome_digest: Bytes32,
    lineage_sequence: u64,
    lineage_head: Bytes32,
    build_digest: Option<Bytes32>,
) -> Result<AppMetadataV2, ProtocolLifecycleError> {
    let metadata = AppMetadataV2::from_v1(
        &prepared.provenance,
        expression_id,
        version_id,
        version_ordinal,
        genome_revision,
        genome_digest,
        lineage_sequence,
        lineage_head,
        build_digest,
    )
    .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
    write_json(
        ledger,
        shot,
        "src/TOHSENO/embedded-provenance.json",
        &metadata,
    )?;
    prepared.provenance_v2 = Some(metadata.clone());
    Ok(metadata)
}

pub fn complete_evolution(
    ledger: &Ledger,
    shot: &Evolution,
    builder: &BuilderIdentity,
    prepared: PreparedEvolution,
) -> Result<CompletedEvolution, ProtocolLifecycleError> {
    let manager = BuilderIdentityManager::for_ledger(ledger);
    if manager.load()? != *builder {
        return Err(ProtocolLifecycleError::InvalidState(
            "builder identity state changed while the generated project was building".into(),
        ));
    }
    let app = ledger.load_app(&shot.app_name)?;
    if app.shot_id != Some(prepared.record.shot_id)
        || app.builder_id != Some(prepared.record.builder_id)
        || app.bundle_id != prepared.record.bundle_id
        || app.target_name() != prepared.record.slug
    {
        return Err(ProtocolLifecycleError::InvalidState(
            "app identity state changed while the generated project was building".into(),
        ));
    }
    if capture_input_commitment(shot)? != prepared.record.genesis_input_sha256 {
        return Err(ProtocolLifecycleError::InvalidState(
            "recorded prompt or input images changed during the signed build".into(),
        ));
    }
    if previous_commitment(ledger, shot, prepared.record.origin.as_ref())?
        != prepared.record.previous
    {
        return Err(ProtocolLifecycleError::InvalidState(
            "the parent Evolution changed during the signed build".into(),
        ));
    }
    verify_exact_json_file(shot, "TOHSENO/shot.json", &prepared.record, "Shot record")?;
    verify_exact_json_file(shot, "TOHSENO/fascia.json", &prepared.fascia, "Shot Fascia")?;
    verify_exact_json_file(
        shot,
        "src/TOHSENO/fascia.json",
        &prepared.fascia,
        "source Fascia",
    )?;
    if let Some(provenance) = &prepared.provenance_v2 {
        verify_exact_json_file(
            shot,
            "src/TOHSENO/embedded-provenance.json",
            provenance,
            "embedded v2 provenance",
        )?;
    } else {
        verify_exact_json_file(
            shot,
            "src/TOHSENO/embedded-provenance.json",
            &prepared.provenance,
            "embedded v1 provenance",
        )?;
    }

    let mut checks = local_checks(shot, &prepared)?;
    if checks.iter().any(|check| check.status != CheckStatus::Pass) {
        let report = report_for(&prepared.record, checks);
        write_json(ledger, shot, "TOHSENO/conformance.json", &report)?;
        return Err(ProtocolLifecycleError::ConformanceFailed(failed_check_ids(
            &report,
        )));
    }

    let signature = manager.sign_record_digest(builder, prepared.commitment)?;
    prepared
        .record
        .verify_signature(&signature)
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
    if signature.public_key != builder.device.public_key {
        return Err(ProtocolLifecycleError::InvalidState(
            "record signer is not the active local DeviceKey".into(),
        ));
    }
    checks.push(pass(
        "record.signature",
        "valid low-s P-256 signature by the active local DeviceKey",
        if builder.test_only {
            "signature verified and DeviceKey matched (TEST ONLY local authority; never publishable)"
        } else {
            "signature verified and DeviceKey matched"
        },
        Some("TOHSENO/signature.json"),
    ));
    let generation =
        resolve_initial_device_builder_id(prepared.record.builder_id, &signature.public_key)?;
    checks.push(pass(
        "record.device_authority",
        "signing DeviceKey deterministically controls the claimed BuilderID",
        &format!(
            "initial DeviceKey, pinned generation {}, salt, and CREATE2 address matched",
            generation.version()
        ),
        Some("TOHSENO/signature.json"),
    ));
    let report = report_for(&prepared.record, checks);
    report
        .validate()
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
    write_json(ledger, shot, "TOHSENO/signature.json", &signature)?;
    write_json(ledger, shot, "TOHSENO/conformance.json", &report)?;
    let preflight = verifier::verify_unsealed_shot_directory(&shot.path, &reference_fascia_root()?);
    if !preflight.conformant {
        return Err(ProtocolLifecycleError::ConformanceFailed(
            preflight
                .checks
                .iter()
                .filter(|check| check.status != verifier::VerificationStatus::Pass)
                .map(|check| check.id.clone())
                .collect(),
        ));
    }

    Ok(CompletedEvolution {
        record: prepared.record,
        signature,
        conformance: report,
        commitment: prepared.commitment,
        app_metadata_v2: prepared.provenance_v2,
    })
}

/// Seals the user's raw prompt and copied input-image bytes before an external
/// harness receives access to the Shot workspace.
pub fn capture_input_commitment(shot: &Evolution) -> Result<Bytes32, ProtocolLifecycleError> {
    let prompt = fs::read(shot.prompt_path())?;
    let mut images = Vec::new();
    for entry in fs::read_dir(shot.images_path())? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() || !file_type.is_file() {
            return Err(ProtocolLifecycleError::InvalidState(format!(
                "input image is not a regular file: {}",
                entry.path().display()
            )));
        }
        let filename = entry.file_name().into_string().map_err(|_| {
            ProtocolLifecycleError::InvalidState("input image filename is not valid UTF-8".into())
        })?;
        images.push(
            genesis_image(filename, &fs::read(entry.path())?)
                .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?,
        );
    }
    genesis_input_sha256(&prompt, &images)
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))
}

fn previous_commitment(
    ledger: &Ledger,
    shot: &Evolution,
    origin: Option<&ShotOrigin>,
) -> Result<Option<Bytes32>, ProtocolLifecycleError> {
    if let Some(ShotOrigin::LegacyAdoption {
        legacy_latest_shot,
        legacy_source_sha256,
    }) = origin
    {
        if legacy_latest_shot.saturating_add(1) != shot.number {
            return Err(ProtocolLifecycleError::InvalidState(
                "legacy adoption does not begin at the next filesystem Shot".into(),
            ));
        }
        let legacy = ledger.shot(&shot.app_name, *legacy_latest_shot)?;
        let observed = hash_source_tree(&legacy.source_path())
            .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?
            .digest;
        if observed != *legacy_source_sha256 {
            return Err(ProtocolLifecycleError::InvalidState(
                "legacy source changed while its adoption was being generated".into(),
            ));
        }
        return Ok(None);
    }
    if shot.number == 1 {
        return Ok(None);
    }
    let previous = ledger.shot(&shot.app_name, shot.number - 1)?;
    verify_completed_evolution(&previous)?;
    let record = read_record(&previous.path.join("TOHSENO/shot.json"))?;
    if record.sequence + 1 != shot.number {
        return Err(ProtocolLifecycleError::InvalidState(
            "previous Evolution sequence is not contiguous".into(),
        ));
    }
    record
        .commitment()
        .map(Some)
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))
}

/// Recomputes all available local evidence for one finalized Evolution.
///
/// This is called before an old source tree is used as evolution context and
/// again after the harness returns, so a process with workspace access cannot
/// silently rewrite the append-only parent.
pub fn verify_completed_evolution(shot: &Evolution) -> Result<(), ProtocolLifecycleError> {
    let report = verifier::verify_shot_directory(&shot.path, &reference_fascia_root()?);
    if !report.conformant {
        let failures = report
            .checks
            .iter()
            .filter(|check| check.status == verifier::VerificationStatus::Fail)
            .map(|check| check.id.clone())
            .collect::<Vec<_>>();
        return Err(ProtocolLifecycleError::ConformanceFailed(failures));
    }

    let metadata = verifier::load_embedded_app_metadata(
        &shot.source_path(),
        "TOHSENO/embedded-provenance.json",
    )
    .map_err(|error| {
        ProtocolLifecycleError::InvalidState(format!(
            "completed Evolution embedded identity changed after conformance verification: {error}"
        ))
    })?;
    if let EmbeddedAppMetadata::V2(metadata) = metadata {
        let layout = completed_evolution_shot_layout(shot)?;
        layout
            .verify_accepted_apple_metadata(&metadata)
            .map_err(|error| {
                ProtocolLifecycleError::InvalidState(format!(
                    "completed Evolution v2 identity is not authenticated by canonical lineage: {error}"
                ))
            })?;
    }
    Ok(())
}

fn completed_evolution_shot_layout(shot: &Evolution) -> Result<ShotLayout, ProtocolLifecycleError> {
    let expected_sequence = format!("{:04}", shot.number);
    if shot.number == 0
        || shot.path.file_name().and_then(|value| value.to_str())
            != Some(expected_sequence.as_str())
    {
        return Err(ProtocolLifecycleError::InvalidState(
            "completed Evolution path does not identify its canonical sequence".into(),
        ));
    }
    let evolutions = shot.path.parent().ok_or_else(|| {
        ProtocolLifecycleError::InvalidState(
            "completed Evolution path has no evolutions directory".into(),
        )
    })?;
    if evolutions.file_name().and_then(|value| value.to_str()) != Some("evolutions") {
        return Err(ProtocolLifecycleError::InvalidState(
            "completed Evolution is outside a Shot's .tohseno/evolutions ledger".into(),
        ));
    }
    let metadata = evolutions.parent().ok_or_else(|| {
        ProtocolLifecycleError::InvalidState(
            "completed Evolution path has no .tohseno directory".into(),
        )
    })?;
    if metadata.file_name().and_then(|value| value.to_str()) != Some(".tohseno") {
        return Err(ProtocolLifecycleError::InvalidState(
            "completed Evolution is outside a Shot's .tohseno/evolutions ledger".into(),
        ));
    }
    let root = metadata.parent().ok_or_else(|| {
        ProtocolLifecycleError::InvalidState(
            "completed Evolution path has no containing Shot body".into(),
        )
    })?;
    Ok(ShotLayout::at(root))
}

pub(crate) fn inspect_fascia(
    source: &Path,
    bundle_id: &str,
    sequence: u32,
) -> Result<FasciaManifest, ProtocolLifecycleError> {
    build::validate_fascia_source_inventory(source).map_err(|error| {
        ProtocolLifecycleError::InvalidState(format!(
            "generated Apple Fascia inventory is not closed: {error}"
        ))
    })?;
    let scan = SourceScan::inspect(source)?;
    if scan.third_party_dependency {
        return Err(ProtocolLifecycleError::InvalidState(
            "third-party runtime dependencies are unsupported by this candidate".into(),
        ));
    }
    if scan.network {
        return Err(ProtocolLifecycleError::InvalidState(
            "network-capable generated source is unsupported until exact endpoint declarations can be proven".into(),
        ));
    }
    if scan.cloud {
        return Err(ProtocolLifecycleError::InvalidState(
            "cloud storage or synchronization is unsupported by this local-first candidate".into(),
        ));
    }
    if scan.tracking_or_accounts {
        return Err(ProtocolLifecycleError::InvalidState(
            "tracking, analytics, telemetry, or account-gating source is forbidden".into(),
        ));
    }
    if scan.forbidden_secret_marker {
        return Err(ProtocolLifecycleError::InvalidState(
            "generated source contains Builder recovery or valid BIP-39 mnemonic material".into(),
        ));
    }
    if scan.explicit_entitlements {
        return Err(ProtocolLifecycleError::InvalidState(
            "explicit application entitlements are unsupported by this finite candidate surface"
                .into(),
        ));
    }
    if scan.unsupported_storage {
        return Err(ProtocolLifecycleError::InvalidState(
            "generated source uses a storage runtime that this Fascia cannot declare exactly"
                .into(),
        ));
    }
    if !scan.apple_capabilities.is_empty() {
        return Err(ProtocolLifecycleError::InvalidState(format!(
            "protected Apple capabilities are unsupported by this candidate: {:?}",
            scan.apple_capabilities
        )));
    }

    let capabilities = vec![CapabilityDeclaration {
        capability: Capability::LocalStorage,
        purpose: "Local-first application state and Apple Fascia metadata".into(),
        entitlement: None,
    }];

    let mut surfaces = vec![AppleSurface::Iphone];
    if scan.ipad {
        surfaces.push(AppleSurface::Ipad);
    }
    let mut storage = vec![
        StorageDeclaration {
            kind: StorageKind::Files,
            purpose: "Atomic local-first app documents and domain state".into(),
        },
        StorageDeclaration {
            kind: StorageKind::UserDefaults,
            purpose: "Small non-sensitive preferences".into(),
        },
        StorageDeclaration {
            kind: StorageKind::Keychain,
            purpose: "App-specific InstallationKey reference".into(),
        },
        StorageDeclaration {
            kind: StorageKind::SecureEnclave,
            purpose: "App-specific InstallationKey when hardware permits".into(),
        },
    ];
    if scan.swift_data {
        storage.push(StorageDeclaration {
            kind: StorageKind::SwiftData,
            purpose: "Local structured application state".into(),
        });
    }
    Ok(FasciaManifest {
        protocol: PROTOCOL_NAME.into(),
        schema: FASCIA_SCHEMA.into(),
        fascia: APPLE_FASCIA_ID.into(),
        required_files: REQUIRED_FASCIA_FILES
            .iter()
            .map(|path| (*path).to_owned())
            .collect(),
        installation_identity: InstallationIdentityDeclaration {
            algorithm: "p256".into(),
            scope: "app_installation".into(),
            hardware_backed_when_available: true,
        },
        capabilities,
        storage,
        network: Vec::new(),
        privacy: PrivacyDeclaration {
            telemetry: false,
            tracking: false,
            account_required: false,
            silent_identity_linkage: false,
        },
        distribution: DistributionDeclaration {
            bundle_id: bundle_id.into(),
            bundle_version: sequence,
            surfaces,
            state: DistributionState::Local,
            app_store_id: None,
        },
    })
}

fn local_checks(
    shot: &Evolution,
    prepared: &PreparedEvolution,
) -> Result<Vec<ConformanceCheck>, ProtocolLifecycleError> {
    let mut checks = Vec::new();
    checks.push(result_check(
        "record.schema",
        "valid closed tohseno.shot/1 record",
        prepared
            .record
            .validate()
            .map(|_| "record validated".into()),
        "TOHSENO/shot.json",
    ));
    checks.push(result_check(
        "fascia.manifest",
        "valid closed tohseno.fascia/1 manifest",
        prepared
            .fascia
            .validate()
            .map(|_| "manifest validated".into()),
        "TOHSENO/fascia.json",
    ));

    let observed_tree = hash_source_tree(&shot.source_path())
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
    checks.push(comparison(
        "source.commitment",
        prepared.record.source_tree_sha256.to_string(),
        observed_tree.digest.to_string(),
        "src",
    ));
    checks.push(comparison(
        "fascia.commitment",
        prepared.record.fascia_sha256.to_string(),
        reference_fascia_commitment()?.to_string(),
        "fascia/apple",
    ));
    checks.push(result_check(
        "apple.anatomy",
        "complete Xcode project with target-member Fascia and provenance",
        build::validate_complete_source(&shot.source_path())
            .map(|_| "required source anatomy found".into()),
        "src",
    ));
    checks.push(result_check(
        "fascia.source_declarations",
        "Fascia declarations exactly reproduce the conservative source policy scan",
        inspect_fascia(
            &shot.source_path(),
            &prepared.record.bundle_id,
            prepared.record.sequence,
        )
        .and_then(|observed| {
            if observed == prepared.fascia {
                Ok("source policy and concrete Fascia matched".into())
            } else {
                Err(ProtocolLifecycleError::InvalidState(
                    "source policy scan produced a different Fascia".into(),
                ))
            }
        }),
        "src",
    ));

    for path in REQUIRED_FASCIA_FILES {
        // These two files are created only after deterministic checks and
        // DeviceKey signing succeed. The verifier checks them on the completed
        // immutable Shot.
        if matches!(*path, "TOHSENO/signature.json" | "TOHSENO/conformance.json") {
            continue;
        }
        let absolute = if *path == "TOHSENO/embedded-provenance.json" {
            shot.source_path().join(path)
        } else if path.starts_with("TOHSENO/") {
            shot.path.join(path)
        } else {
            shot.source_path().join(path)
        };
        checks.push(if absolute.is_file() {
            pass(
                &format!("fascia.file.{}", check_token(path)),
                "required regular file",
                "present",
                Some(path),
            )
        } else {
            fail(
                &format!("fascia.file.{}", check_token(path)),
                "required regular file",
                "missing",
                Some(path),
            )
        });
    }

    let scan = SourceScan::inspect(&shot.source_path())?;
    checks.push(if scan.third_party_dependency {
        fail(
            "apple.dependencies",
            "no undeclared third-party runtime package",
            "XCRemoteSwiftPackageReference found",
            Some("src"),
        )
    } else {
        pass(
            "apple.dependencies",
            "no undeclared third-party runtime package",
            "none found",
            Some("src"),
        )
    });
    checks.push(if scan.forbidden_secret_marker {
        fail(
            "privacy.boundary",
            "no Builder recovery or mnemonic material in app source",
            "forbidden recovery marker found",
            Some("src"),
        )
    } else {
        pass(
            "privacy.boundary",
            "no Builder recovery or mnemonic material in app source",
            "no forbidden marker found",
            Some("src"),
        )
    });
    checks.push(result_check(
        "privacy.input_boundary",
        "raw private prompt and input-image bytes are absent from source and retained artifact",
        verifier::verify_private_input_boundary(&shot.path),
        "src",
    ));
    checks.push(pass(
        "storage.local_first",
        "local storage declared and no cloud default",
        "files, preferences, and app-specific Keychain declared",
        Some("TOHSENO/fascia.json"),
    ));

    let project_text = xcode_project_text(&shot.source_path())?;
    checks.push(if project_text.contains(&prepared.record.bundle_id) {
        pass(
            "apple.bundle_id",
            &prepared.record.bundle_id,
            &prepared.record.bundle_id,
            Some("src"),
        )
    } else {
        fail(
            "apple.bundle_id",
            &prepared.record.bundle_id,
            "bundle identifier absent from project source",
            Some("src"),
        )
    });
    checks.push(if project_version_matches(&project_text, shot.number) {
        pass(
            "apple.bundle_version",
            &shot.number.to_string(),
            &shot.number.to_string(),
            Some("src"),
        )
    } else {
        fail(
            "apple.bundle_version",
            &shot.number.to_string(),
            "CURRENT_PROJECT_VERSION did not match",
            Some("src"),
        )
    });

    let artifact = shot
        .artifact_path()
        .join(format!("{}.app", prepared.record.slug));
    checks.push(result_check(
        "artifact.runtime_dependencies",
        "no embedded frameworks, plug-ins, extensions, dynamic libraries, or executable service bundles",
        verifier::verify_artifact_runtime_boundary(&artifact),
        "artifact",
    ));
    let artifact_bundle = plist_value(&artifact.join("Info.plist"), "CFBundleIdentifier");
    checks.push(comparison_result(
        "artifact.bundle_id",
        &prepared.record.bundle_id,
        artifact_bundle,
        "artifact",
    ));
    let artifact_version = plist_value(&artifact.join("Info.plist"), "CFBundleVersion");
    checks.push(comparison_result(
        "artifact.bundle_version",
        &shot.number.to_string(),
        artifact_version,
        "artifact",
    ));
    checks.push(result_check(
        "artifact.provenance",
        "one embedded public provenance resource exactly matching source",
        artifact_resource_matches(
            &shot.source_path().join("TOHSENO/embedded-provenance.json"),
            &artifact,
            "embedded-provenance.json",
        ),
        "artifact",
    ));
    checks.push(result_check(
        "artifact.fascia",
        "one embedded concrete Fascia resource exactly matching source",
        artifact_resource_matches(
            &shot.source_path().join("TOHSENO/fascia.json"),
            &artifact,
            "fascia.json",
        ),
        "artifact",
    ));
    Ok(checks)
}

fn report_for(record: &ShotRecord, checks: Vec<ConformanceCheck>) -> ConformanceReport {
    let conformant = checks.iter().all(|check| check.status == CheckStatus::Pass);
    ConformanceReport {
        schema: CONFORMANCE_SCHEMA.into(),
        shot_id: record.shot_id,
        sequence: record.sequence,
        conformant,
        checks,
    }
}

fn failed_check_ids(report: &ConformanceReport) -> Vec<String> {
    report
        .checks
        .iter()
        .filter(|check| check.status != CheckStatus::Pass)
        .map(|check| check.id.clone())
        .collect()
}

fn pass(id: &str, expected: &str, observed: &str, evidence: Option<&str>) -> ConformanceCheck {
    check(id, CheckStatus::Pass, expected, observed, evidence)
}

fn fail(id: &str, expected: &str, observed: &str, evidence: Option<&str>) -> ConformanceCheck {
    check(id, CheckStatus::Fail, expected, observed, evidence)
}

fn check(
    id: &str,
    status: CheckStatus,
    expected: &str,
    observed: &str,
    evidence: Option<&str>,
) -> ConformanceCheck {
    ConformanceCheck {
        id: id.into(),
        status,
        expected: expected.into(),
        observed: observed.into(),
        evidence_path: evidence.map(str::to_owned),
    }
}

fn comparison(id: &str, expected: String, observed: String, evidence: &str) -> ConformanceCheck {
    if expected == observed {
        pass(id, &expected, &observed, Some(evidence))
    } else {
        fail(id, &expected, &observed, Some(evidence))
    }
}

fn comparison_result(
    id: &str,
    expected: &str,
    observed: Result<String, ProtocolLifecycleError>,
    evidence: &str,
) -> ConformanceCheck {
    match observed {
        Ok(observed) if observed == expected => pass(id, expected, &observed, Some(evidence)),
        Ok(observed) => fail(id, expected, &observed, Some(evidence)),
        Err(error) => fail(id, expected, &error.to_string(), Some(evidence)),
    }
}

fn result_check<E: std::fmt::Display>(
    id: &str,
    expected: &str,
    result: Result<String, E>,
    evidence: &str,
) -> ConformanceCheck {
    match result {
        Ok(observed) => pass(id, expected, &observed, Some(evidence)),
        Err(error) => fail(id, expected, &error.to_string(), Some(evidence)),
    }
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

fn write_json<T: Serialize>(
    ledger: &Ledger,
    shot: &Evolution,
    relative: &str,
    value: &T,
) -> Result<(), ProtocolLifecycleError> {
    let bytes = json_bytes(value)?;
    ledger.write_evolution_file(shot, relative, &bytes)?;
    Ok(())
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, ProtocolLifecycleError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn verify_exact_json_file<T: Serialize>(
    shot: &Evolution,
    relative: &str,
    expected: &T,
    label: &str,
) -> Result<(), ProtocolLifecycleError> {
    let observed = fs::read(shot.path.join(relative))?;
    if observed != json_bytes(expected)? {
        return Err(ProtocolLifecycleError::InvalidState(format!(
            "{label} changed during the signed build"
        )));
    }
    Ok(())
}

fn read_record(path: &Path) -> Result<ShotRecord, ProtocolLifecycleError> {
    let record = tohseno_protocol::canonical::from_slice::<ShotRecord>(&fs::read(path)?)
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
    record
        .validate()
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
    Ok(record)
}

fn canonical_now() -> Result<CanonicalTimestamp, ProtocolLifecycleError> {
    let value = OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .map_err(|error| ProtocolLifecycleError::InvalidState(error.to_string()))?
        .format(&Rfc3339)
        .map_err(|error| ProtocolLifecycleError::InvalidState(error.to_string()))?;
    CanonicalTimestamp::parse(value)
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))
}

fn reference_fascia_commitment() -> Result<Bytes32, ProtocolLifecycleError> {
    hash_fascia_tree(&reference_fascia_root()?)
        .map(|commitment| commitment.digest)
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))
}

/// Locates the exact reusable Apple Fascia tree pinned by this candidate.
///
/// The offline verifier accepts this path explicitly; exposing the candidate
/// locator here keeps CLI and engine lifecycle verification on one source of
/// truth without making it part of the pure protocol crate.
pub fn reference_fascia_root() -> Result<std::path::PathBuf, ProtocolLifecycleError> {
    if let Some(configured) = std::env::var_os("TOHSENO_FASCIA_ROOT") {
        if !cfg!(debug_assertions) {
            return Err(ProtocolLifecycleError::InvalidState(
                "TOHSENO_FASCIA_ROOT is a development-only locator and is disabled in release builds"
                    .into(),
            ));
        }
        let root = std::path::PathBuf::from(configured);
        if !root.is_absolute() || !root.is_dir() {
            return Err(ProtocolLifecycleError::InvalidState(
                "TOHSENO_FASCIA_ROOT must be an absolute Fascia directory".into(),
            ));
        }
        return validate_pinned_fascia_root(root);
    }
    let executable = std::env::current_exe()?;
    if let Some(prefix) = executable.parent().and_then(Path::parent) {
        // Release archives carry `fascia/apple` beside `bin`; conventional
        // system installs may place the same pinned bytes under `share`.
        for installed in [
            prefix.join("fascia/apple"),
            prefix.join("genesis/fascia/apple"),
            prefix.join("share/genesis/fascia/apple"),
        ] {
            if installed.is_dir() {
                return validate_pinned_fascia_root(installed);
            }
        }
    }
    if cfg!(debug_assertions) {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fascia/apple");
        if source.is_dir() {
            return validate_pinned_fascia_root(source);
        }
    }
    Err(ProtocolLifecycleError::InvalidState(
        "the pinned Apple Fascia reference tree is missing".into(),
    ))
}

fn validate_pinned_fascia_root(
    root: std::path::PathBuf,
) -> Result<std::path::PathBuf, ProtocolLifecycleError> {
    let observed = hash_fascia_tree(&root)
        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?
        .digest;
    if observed.to_string() != PINNED_APPLE_FASCIA_SHA256 {
        return Err(ProtocolLifecycleError::InvalidState(format!(
            "Apple Fascia root {} has commitment {observed}, but this candidate pins {PINNED_APPLE_FASCIA_SHA256}; a development locator cannot replace candidate law",
            root.display()
        )));
    }
    Ok(root)
}

fn find_unique_regular_file(
    directory: &Path,
    filename: &str,
) -> Result<Option<std::path::PathBuf>, ProtocolLifecycleError> {
    let mut found = None;
    find_unique_regular_file_inner(directory, filename, &mut found)?;
    Ok(found)
}

fn find_unique_regular_file_inner(
    directory: &Path,
    filename: &str,
    found: &mut Option<std::path::PathBuf>,
) -> Result<(), ProtocolLifecycleError> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(ProtocolLifecycleError::InvalidState(format!(
                "refusing symlink in artifact: {}",
                entry.path().display()
            )));
        }
        if file_type.is_dir() {
            find_unique_regular_file_inner(&entry.path(), filename, found)?;
        } else if file_type.is_file()
            && entry.file_name() == filename
            && found.replace(entry.path()).is_some()
        {
            return Err(ProtocolLifecycleError::InvalidState(format!(
                "artifact contains more than one {filename}"
            )));
        }
    }
    Ok(())
}

fn artifact_resource_matches(
    source: &Path,
    artifact: &Path,
    filename: &str,
) -> Result<String, ProtocolLifecycleError> {
    const MAX_RESOURCE_BYTES: u64 = 4 * 1024 * 1024;

    let source_metadata = fs::symlink_metadata(source)?;
    if source_metadata.file_type().is_symlink()
        || !source_metadata.is_file()
        || source_metadata.len() > MAX_RESOURCE_BYTES
    {
        return Err(ProtocolLifecycleError::InvalidState(format!(
            "source {filename} is not a bounded regular file"
        )));
    }
    let embedded = find_unique_regular_file(artifact, filename)?.ok_or_else(|| {
        ProtocolLifecycleError::InvalidState(format!("artifact has no {filename}"))
    })?;
    let embedded_metadata = fs::symlink_metadata(&embedded)?;
    if embedded_metadata.file_type().is_symlink()
        || !embedded_metadata.is_file()
        || embedded_metadata.len() > MAX_RESOURCE_BYTES
    {
        return Err(ProtocolLifecycleError::InvalidState(format!(
            "embedded {filename} is not a bounded regular file"
        )));
    }
    if fs::read(source)? != fs::read(embedded)? {
        return Err(ProtocolLifecycleError::InvalidState(format!(
            "embedded {filename} differs from source"
        )));
    }
    Ok(format!("{filename} matched byte-for-byte"))
}

fn xcode_project_text(source: &Path) -> Result<String, ProtocolLifecycleError> {
    let project = fs::read_dir(source)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "xcodeproj")
        })
        .ok_or_else(|| ProtocolLifecycleError::InvalidState("Xcode project missing".into()))?;
    Ok(fs::read_to_string(project.join("project.pbxproj"))?)
}

fn project_version_matches(project: &str, sequence: u32) -> bool {
    project.lines().any(|line| {
        line.trim()
            .strip_prefix("CURRENT_PROJECT_VERSION = ")
            .and_then(|value| value.strip_suffix(';'))
            .is_some_and(|value| value.trim() == sequence.to_string())
    })
}

fn plist_value(path: &Path, key: &str) -> Result<String, ProtocolLifecycleError> {
    if !path.is_file() {
        return Err(ProtocolLifecycleError::InvalidState(format!(
            "{} is missing",
            path.display()
        )));
    }
    let output = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", &format!("Print :{key}")])
        .arg(path)
        .output()?;
    if !output.status.success() {
        return Err(ProtocolLifecycleError::InvalidState(format!(
            "artifact Info.plist has no {key}"
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[derive(Default)]
struct SourceScan {
    network: bool,
    cloud: bool,
    tracking_or_accounts: bool,
    explicit_entitlements: bool,
    unsupported_storage: bool,
    swift_data: bool,
    ipad: bool,
    third_party_dependency: bool,
    forbidden_secret_marker: bool,
    apple_capabilities: BTreeSet<Capability>,
}

impl SourceScan {
    fn inspect(root: &Path) -> Result<Self, ProtocolLifecycleError> {
        let mut scan = Self::default();
        scan.visit(root)?;
        Ok(scan)
    }

    fn visit(&mut self, directory: &Path) -> Result<(), ProtocolLifecycleError> {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                return Err(ProtocolLifecycleError::InvalidState(format!(
                    "refusing symlink in generated source: {}",
                    entry.path().display()
                )));
            }
            if file_type.is_dir() {
                if entry.path().extension().is_some_and(|extension| {
                    matches!(extension.to_str(), Some("framework" | "xcframework"))
                }) {
                    self.third_party_dependency = true;
                }
                self.visit(&entry.path())?;
                continue;
            }
            if !file_type.is_file() {
                return Err(ProtocolLifecycleError::InvalidState(format!(
                    "unsupported source entry: {}",
                    entry.path().display()
                )));
            }
            let bytes = fs::read(entry.path())?;
            if entry.path().extension().is_some_and(|extension| {
                matches!(
                    extension.to_str(),
                    Some("a" | "dylib" | "so" | "framework" | "xcframework")
                )
            }) || entry.file_name() == "Package.swift"
                || is_mach_o(&bytes)
            {
                self.third_party_dependency = true;
            }
            let Ok(text) = std::str::from_utf8(&bytes) else {
                continue;
            };
            self.network |= [
                "URLSession",
                "NSURLSession",
                "NWConnection",
                "import Network",
                "WebSocket",
                "WKWebView",
                "SFSafariViewController",
                "CFStream",
                "socket(",
                "connect(",
                "curl ",
                "http://",
                "https://",
                "Network.framework",
            ]
            .iter()
            .any(|marker| text.contains(marker));
            self.cloud |= [
                "import CloudKit",
                "CKContainer",
                "NSUbiquitous",
                "iCloud",
                "com.apple.developer.icloud",
                "Firebase",
                "Supabase",
            ]
            .iter()
            .any(|marker| text.contains(marker));
            self.tracking_or_accounts |= [
                "AppTrackingTransparency",
                "ATTrackingManager",
                "Analytics",
                "Telemetry",
                "advertisingIdentifier",
                "AuthenticationServices",
                "SignInWithApple",
            ]
            .iter()
            .any(|marker| text.contains(marker));
            self.explicit_entitlements |= entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "entitlements")
                && text.contains("<key>");
            self.swift_data |= text.contains("import SwiftData") || text.contains("@Model");
            self.unsupported_storage |= [
                "import CoreData",
                "NSPersistentContainer",
                "SQLite",
                "RealmSwift",
            ]
            .iter()
            .any(|marker| text.contains(marker));
            self.ipad |= text.contains("TARGETED_DEVICE_FAMILY = \"1,2\"")
                || text.contains("TARGETED_DEVICE_FAMILY = 1,2");
            self.third_party_dependency |= [
                "XCRemoteSwiftPackageReference",
                "XCLocalSwiftPackageReference",
                "packageProductDependencies",
                "CocoaPods",
                "Carthage",
            ]
            .iter()
            .any(|marker| text.contains(marker));
            self.forbidden_secret_marker |= [
                "recovery.json.enc",
                "BIP39",
                "BIP-39 mnemonic",
                "builder_recovery_mnemonic",
                "TOHSENO_RECOVERY_WORDS",
            ]
            .iter()
            .any(|marker| text.contains(marker));
            self.forbidden_secret_marker |= contains_valid_bip39_mnemonic(text);
            for (marker, capability) in [
                ("NSCameraUsageDescription", Capability::Camera),
                ("NSMicrophoneUsageDescription", Capability::Microphone),
                ("NSLocation", Capability::Location),
                ("NSContactsUsageDescription", Capability::Contacts),
                ("NSHealth", Capability::Health),
                ("NSBluetooth", Capability::Bluetooth),
                ("UNUserNotificationCenter", Capability::Notifications),
                ("import StoreKit", Capability::Storekit),
            ] {
                if text.contains(marker) {
                    self.apple_capabilities.insert(capability);
                }
            }
        }
        Ok(())
    }
}

fn is_mach_o(bytes: &[u8]) -> bool {
    let Some(magic) = bytes.get(..4) else {
        return false;
    };
    matches!(
        magic,
        [0xfe, 0xed, 0xfa, 0xce]
            | [0xce, 0xfa, 0xed, 0xfe]
            | [0xfe, 0xed, 0xfa, 0xcf]
            | [0xcf, 0xfa, 0xed, 0xfe]
            | [0xca, 0xfe, 0xba, 0xbe]
            | [0xbe, 0xba, 0xfe, 0xca]
    )
}

fn contains_valid_bip39_mnemonic(text: &str) -> bool {
    let words = text
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    [12_usize, 15, 18, 21, 24].into_iter().any(|length| {
        words.windows(length).any(|window| {
            let phrase = window.join(" ");
            Mnemonic::parse_in(Language::English, &phrase).is_ok()
        })
    })
}

#[derive(Debug)]
pub enum ProtocolLifecycleError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Ledger(LedgerError),
    Identity(BuilderIdentityError),
    Protocol(String),
    InvalidState(String),
    ConformanceFailed(Vec<String>),
}

impl std::fmt::Display for ProtocolLifecycleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Ledger(error) => write!(formatter, "{error}"),
            Self::Identity(error) => write!(formatter, "{error}"),
            Self::Protocol(error) => write!(formatter, "{error}"),
            Self::InvalidState(error) => write!(formatter, "{error}"),
            Self::ConformanceFailed(checks) => {
                write!(formatter, "Shot conformance failed: {}", checks.join(", "))
            }
        }
    }
}

impl std::error::Error for ProtocolLifecycleError {}

impl From<std::io::Error> for ProtocolLifecycleError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for ProtocolLifecycleError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<LedgerError> for ProtocolLifecycleError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

impl From<BuilderIdentityError> for ProtocolLifecycleError {
    fn from(value: BuilderIdentityError) -> Self {
        Self::Identity(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tohseno_protocol::digest::{Address20, ShotId};
    use tohseno_protocol::identity::BuilderId;

    #[test]
    fn project_version_requires_exact_integer_setting() {
        assert!(project_version_matches("CURRENT_PROJECT_VERSION = 7;", 7));
        assert!(!project_version_matches("CURRENT_PROJECT_VERSION = 70;", 7));
    }

    #[test]
    fn input_commitment_is_filename_order_independent() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(directory.path());
        ledger.create_app("press", "com.example.press").unwrap();
        let shot = ledger.reserve_evolution("press", None).unwrap();
        fs::write(shot.prompt_path(), b"Make it.\n").unwrap();
        fs::write(shot.images_path().join("z.png"), b"z").unwrap();
        fs::write(shot.images_path().join("a.png"), b"a").unwrap();
        let observed = capture_input_commitment(&shot).unwrap();
        let expected = genesis_input_sha256(
            b"Make it.\n",
            &[
                genesis_image("a.png", b"a").unwrap(),
                genesis_image("z.png", b"z").unwrap(),
            ],
        )
        .unwrap();
        assert_eq!(observed, expected);
    }

    #[test]
    fn input_commitment_changes_when_harness_visible_input_changes() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(directory.path());
        ledger.create_app("press", "com.example.press").unwrap();
        let shot = ledger.reserve_evolution("press", None).unwrap();
        fs::write(shot.prompt_path(), b"Make it.\n").unwrap();
        let sealed = capture_input_commitment(&shot).unwrap();
        fs::write(shot.prompt_path(), b"Make something else.\n").unwrap();
        assert_ne!(capture_input_commitment(&shot).unwrap(), sealed);
    }

    #[test]
    fn artifact_resources_must_be_unique_and_byte_identical() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.json");
        let artifact = directory.path().join("Press.app");
        fs::create_dir(&artifact).unwrap();
        fs::write(&source, b"{\"public\":true}\n").unwrap();
        fs::write(artifact.join("fascia.json"), b"{\"public\":true}\n").unwrap();
        artifact_resource_matches(&source, &artifact, "fascia.json").unwrap();

        let nested = artifact.join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("fascia.json"), b"{\"public\":true}\n").unwrap();
        assert!(artifact_resource_matches(&source, &artifact, "fascia.json").is_err());
    }

    #[test]
    fn candidate_fascia_pin_accepts_only_the_committed_law() {
        let reference = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fascia/apple");
        assert_eq!(
            validate_pinned_fascia_root(reference.clone()).unwrap(),
            reference
        );

        let alternate = tempfile::tempdir().unwrap();
        fs::write(alternate.path().join("FASCIA.md"), "alternate law\n").unwrap();
        assert!(validate_pinned_fascia_root(alternate.path().to_owned()).is_err());
    }

    #[test]
    fn report_truth_is_the_conjunction_of_checks() {
        let record = ShotRecord {
            protocol: PROTOCOL_NAME.into(),
            schema: SHOT_SCHEMA.into(),
            shot_id: ShotId::from_bytes([1; 32]),
            slug: "press".into(),
            builder_id: BuilderId::new(Address20::from_bytes([2; 20])),
            sequence: 1,
            previous: None,
            fascia: APPLE_FASCIA_ID.into(),
            bundle_id: "com.example.press".into(),
            bundle_version: 1,
            genesis_input_sha256: Bytes32::new([3; 32]),
            source_tree_sha256: Bytes32::new([4; 32]),
            fascia_sha256: Bytes32::new([5; 32]),
            factory: FactoryDescriptor {
                implementation: "example/factory".into(),
                version: CANDIDATE_VERSION.into(),
                source_commit: "a".repeat(40),
            },
            created_at: CanonicalTimestamp::parse("2026-07-28T00:00:00Z").unwrap(),
            origin: None,
        };
        let passing = report_for(&record, vec![pass("one", "expected", "observed", None)]);
        assert!(passing.conformant);
        let failing = report_for(&record, vec![fail("one", "expected", "wrong", None)]);
        assert!(!failing.conformant);
    }

    #[test]
    fn engine_metadata_bytes_are_the_swift_fixture() {
        let record = ShotRecord {
            protocol: PROTOCOL_NAME.into(),
            schema: SHOT_SCHEMA.into(),
            shot_id: ShotId::from_bytes([1; 32]),
            slug: "fixture".into(),
            builder_id: BuilderId::new(Address20::from_bytes([0x11; 20])),
            sequence: 1,
            previous: None,
            fascia: APPLE_FASCIA_ID.into(),
            bundle_id: "example.app".into(),
            bundle_version: 1,
            genesis_input_sha256: Bytes32::new([2; 32]),
            source_tree_sha256: Bytes32::new([4; 32]),
            fascia_sha256: Bytes32::new([5; 32]),
            factory: FactoryDescriptor {
                implementation: "example/factory".into(),
                version: CANDIDATE_VERSION.into(),
                source_commit: "a".repeat(40),
            },
            created_at: CanonicalTimestamp::parse("2026-07-28T00:00:00Z").unwrap(),
            origin: None,
        };
        let fascia = FasciaManifest {
            protocol: PROTOCOL_NAME.into(),
            schema: FASCIA_SCHEMA.into(),
            fascia: APPLE_FASCIA_ID.into(),
            required_files: REQUIRED_FASCIA_FILES
                .iter()
                .map(|path| (*path).to_owned())
                .collect(),
            installation_identity: InstallationIdentityDeclaration {
                algorithm: "p256".into(),
                scope: "app_installation".into(),
                hardware_backed_when_available: true,
            },
            capabilities: vec![CapabilityDeclaration {
                capability: Capability::LocalStorage,
                purpose: "Save the user's documents".into(),
                entitlement: None,
            }],
            storage: vec![StorageDeclaration {
                kind: StorageKind::Files,
                purpose: "Save the user's documents".into(),
            }],
            network: Vec::new(),
            privacy: PrivacyDeclaration {
                telemetry: false,
                tracking: false,
                account_required: false,
                silent_identity_linkage: false,
            },
            distribution: DistributionDeclaration {
                bundle_id: record.bundle_id.clone(),
                bundle_version: record.bundle_version,
                surfaces: vec![AppleSurface::Iphone],
                state: DistributionState::Local,
                app_store_id: None,
            },
        };
        let commitment = record.commitment().unwrap();
        let metadata = AppMetadata::for_record(&record, commitment, &fascia).unwrap();
        let fixture = include_bytes!("../../protocol/test-vectors/app-metadata-v1.json");
        assert_eq!(
            String::from_utf8(json_bytes(&metadata).unwrap()).unwrap(),
            String::from_utf8(fixture.to_vec()).unwrap()
        );
    }
}
