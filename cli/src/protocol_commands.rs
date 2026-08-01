use crate::ProtocolCommand;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tohseno_engine::contract_generation::{
    resolve_current_contract_generation, ContractGenerationState, ResolvedContractGeneration,
    CURRENT_GENERATION_REPOSITORY_PATH,
};
use tohseno_engine::page;
use tohseno_engine::protocol_lifecycle;
use tohseno_engine::verifier::{
    self, LineageVerificationReport, ShotVerificationReport, VerificationCheck, VerificationStatus,
};
use tohseno_engine::{Event, EventBus, Evolution, Ledger, ShotBodyVerification, ShotLayout};
use tohseno_protocol::fascia::FasciaManifest;
use tohseno_protocol::record::ShotRecord;
use tohseno_protocol::signature::SignatureSidecar;

const VECTORS: &str = include_str!("../../protocol/test-vectors/protocol-v1.json");

pub fn protocol_command(
    command: ProtocolCommand,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        ProtocolCommand::Info => {
            let info = protocol_info()?;
            if json {
                print_json(&info)?;
            } else {
                bus.emit(Event::result(format!(
                    "TOHSENO {} · stable local product.",
                    info.product_version
                )));
                bus.emit(Event::status(format!(
                    "contract generation {} is definition-only and inactive · no public authority.",
                    info.contract_generation.generation
                )));
            }
        }
        ProtocolCommand::Vectors => {
            if json {
                let vectors: serde_json::Value = serde_json::from_str(VECTORS)?;
                print_json(&vectors)?;
            } else {
                let value: serde_json::Value = serde_json::from_str(VECTORS)?;
                bus.emit(Event::result(
                    "the frozen cross-language protocol vectors are valid JSON.",
                ));
                bus.emit(Event::status(format!(
                    "{} · candidate 0.7.0",
                    value
                        .get("schema")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("protocol vector schema unavailable")
                )));
            }
        }
        ProtocolCommand::VerifyRecord { path } => {
            let (record, signature) = read_record_pair(&path)?;
            record.verify_signature(&signature)?;
            let result = RecordVerification {
                conformant: true,
                record_path: path.display().to_string(),
                shot_id: record.shot_id.to_string(),
                builder_id: record.builder_id.to_string(),
                sequence: record.sequence,
                commitment: record.commitment()?.to_string(),
                signer_key_id: tohseno_protocol::identity::device_key_id(&signature.public_key)
                    .to_string(),
            };
            if json {
                print_json(&result)?;
            } else {
                bus.emit(Event::result("record and low-s P-256 signature are valid."));
                bus.emit(Event::status(format!(
                    "Shot {} · Evolution {}.",
                    result.shot_id, result.sequence
                )));
            }
        }
    }
    Ok(())
}

pub fn inspect_target(
    target: &str,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let shot_body = resolve_shot_body(target)?;
    let shot = match resolve_shot(target) {
        Ok(shot) => shot,
        Err(error) => {
            if let Some(body) = shot_body {
                let view = ShotBodyInspection::from_report(target, body);
                if json {
                    print_json(&view)?;
                } else {
                    bus.emit(Event::result(format!(
                        "Shot {} has a verified protocol {} body.",
                        view.report.shot_id, view.report.protocol_version
                    )));
                    bus.emit(Event::status(format!(
                        "{} signed action(s) · head {}.",
                        view.report.lineage_sequence, view.report.lineage_head
                    )));
                }
                return Ok(());
            }
            if report_unfinished_app(target, json, bus)? {
                return Err(
                    "no complete Shot yet; the attempt above is unfinished and unsigned".into(),
                );
            }
            return Err(error);
        }
    };
    if report_unfinished_app(target, json, bus)? {
        return Err("no complete Shot yet; the attempt above is unfinished and unsigned".into());
    }
    let fascia_reference = protocol_lifecycle::reference_fascia_root()?;
    let verification = verifier::verify_shot_directory(&shot.path, &fascia_reference);
    if !verification.conformant {
        return Err("Shot failed deterministic offline verification".into());
    }
    let record: ShotRecord =
        tohseno_protocol::canonical::from_slice(&fs::read(shot.path.join("TOHSENO/shot.json"))?)?;
    record.validate()?;
    let signature: SignatureSidecar = tohseno_protocol::canonical::from_slice(&fs::read(
        shot.path.join("TOHSENO/signature.json"),
    )?)?;
    record.verify_signature(&signature)?;
    let fascia: FasciaManifest =
        tohseno_protocol::canonical::from_slice(&fs::read(shot.path.join("TOHSENO/fascia.json"))?)?;
    fascia.validate()?;
    let conformance: tohseno_protocol::conformance::ConformanceReport =
        tohseno_protocol::canonical::from_slice(&fs::read(
            shot.path.join("TOHSENO/conformance.json"),
        )?)?;
    conformance.validate()?;
    let view = Inspection {
        app_name: shot.app_name,
        shot_directory: shot.path.display().to_string(),
        shot_id: record.shot_id.to_string(),
        builder_id: record.builder_id.to_string(),
        sequence: record.sequence,
        previous: record.previous.map(|value| value.to_string()),
        commitment: record.commitment()?.to_string(),
        source_tree_sha256: record.source_tree_sha256.to_string(),
        fascia_sha256: record.fascia_sha256.to_string(),
        signer_key_id: tohseno_protocol::identity::device_key_id(&signature.public_key).to_string(),
        conformant: conformance.conformant,
        public_state: "private",
        shot_body,
    };
    if json {
        print_json(&view)?;
    } else {
        bus.emit(Event::result(format!(
            "evolution {} is locally verified and private.",
            view.sequence
        )));
        bus.emit(Event::status(format!(
            "{} · {}",
            view.shot_id, view.builder_id
        )));
    }
    Ok(())
}

pub fn verify_target(
    target: &str,
    public: bool,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    if public {
        ensure_public_verification_available()?;
    }
    let fascia_reference = protocol_lifecycle::reference_fascia_root()?;
    if report_unfinished_app(target, json, bus)? {
        return Err("no complete Shot yet; the attempt above is unfinished and unsigned".into());
    }
    let local = resolve_verification(target, &fascia_reference)?;
    let conformant = local.conformant();
    let result = VerificationOutput {
        schema: "tohseno.cli-verification/1",
        conformant,
        local,
    };
    if json {
        print_json(&result)?;
    } else {
        render_verification(&result, bus);
    }
    if !result.conformant {
        return Err("offline Shot verification failed".into());
    }
    Ok(())
}

pub fn build_page(
    app_name: &str,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = Ledger::discover()?;
    let report = page::build(&ledger, app_name)?;
    if json {
        print_json(&report)?;
    } else {
        bus.emit(Event::result(format!(
            "private static Shot page built at {}.",
            report.output_path.display()
        )));
        bus.emit(Event::status(
            "generation did not publish source, registry state, or a public URL.",
        ));
    }
    Ok(())
}

pub fn network_status(json: bool, bus: &EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let status = inactive_network_status()?;
    if json {
        print_json(&status)?;
    } else {
        bus.emit(Event::result(format!(
            "contract generation {} is committed but inactive.",
            status.contract_generation.generation
        )));
        bus.emit(Event::status(status.reason));
    }
    Ok(())
}

fn protocol_info() -> Result<ProtocolInfo, Box<dyn std::error::Error>> {
    let generation = resolve_current_contract_generation()?;
    Ok(ProtocolInfo {
        schema: "tohseno.protocol-info/2",
        protocol: "tohseno",
        product_version: env!("CARGO_PKG_VERSION"),
        release_status: "stable",
        shot_schema: tohseno_protocol::record::SHOT_SCHEMA,
        compatibility_shot_schema: tohseno_protocol::record::SHOT_SCHEMA,
        lineage_protocol_version: tohseno_protocol::lineage::LINEAGE_PROTOCOL_VERSION,
        lineage_action_schema: tohseno_protocol::lineage::LINEAGE_ACTION_SCHEMA,
        lineage_schema_version: tohseno_protocol::lineage::LINEAGE_SCHEMA_VERSION,
        signature_schema: SignatureSidecar::SCHEMA,
        fascia: tohseno_protocol::record::APPLE_FASCIA_ID,
        app_metadata_schemas: [
            tohseno_protocol::app_metadata::APP_METADATA_SCHEMA,
            tohseno_protocol::app_metadata::APP_METADATA_V2_SCHEMA,
        ],
        supported_token_association_chains: [tohseno_protocol::identity::ROBINHOOD_CHAIN_ID, 8_453],
        contract_generation: ContractGenerationSummary::from_resolved(&generation),
        active_generation: active_generation_label(&generation),
        public_authority_available: generation.allows_public_signing(),
    })
}

/// The generation label surfaces only when the resolved trust root grants
/// public authority; a committed or deployed-inactive definition stays null.
fn active_generation_label(generation: &ResolvedContractGeneration) -> Option<String> {
    generation
        .allows_public_signing()
        .then(|| generation.definition.generation.clone())
}

/// Why public workflows are unavailable right now, in either state: inactive
/// builds lack authority, and active builds still lack the registry
/// RPC/receipt workflow, which is separate implementation work.
fn public_workflow_reason(generation: &ResolvedContractGeneration) -> &'static str {
    if generation.allows_public_signing() {
        "the generation is active, but the registry verification workflow is not implemented in this build"
    } else {
        generation.inactive_reason()
    }
}

fn inactive_network_status() -> Result<NetworkStatus, Box<dyn std::error::Error>> {
    let generation = resolve_current_contract_generation()?;
    Ok(NetworkStatus {
        schema: "tohseno.network-status/2",
        protocol: "tohseno",
        product_version: env!("CARGO_PKG_VERSION"),
        contract_generation: ContractGenerationSummary::from_resolved(&generation),
        active_generation: active_generation_label(&generation),
        ready: false,
        rpc_checked: false,
        public_authority_available: generation.allows_public_signing(),
        reason: public_workflow_reason(&generation),
    })
}

fn ensure_public_verification_available() -> Result<(), Box<dyn std::error::Error>> {
    let generation = resolve_current_contract_generation()?;
    match generation.state {
        ContractGenerationState::Inactive => Err(format!(
            "public verification unavailable: {}; no RPC was contacted",
            generation.inactive_reason()
        )
        .into()),
        // Activation authorizes public verification, but the registry
        // RPC/receipt workflow is separate implementation work; until it
        // exists an activated build must still refuse rather than pass a
        // local-only check off as a public one.
        ContractGenerationState::Active => Err(
            "public verification unavailable: the registry verification workflow is not implemented in this build; no RPC was contacted"
                .into(),
        ),
    }
}

pub fn registry_show(
    app_name: &str,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let (shot, record) = verified_local_head(app_name)?;
    let generation = resolve_current_contract_generation()?;
    let view = RegistryView {
        schema: "tohseno.registry-view/2",
        app_name: app_name.to_owned(),
        shot_id: record.shot_id.to_string(),
        local_head: record.commitment()?.to_string(),
        local_sequence: record.sequence,
        local_state: "private",
        local_verified: true,
        active_generation: active_generation_label(&generation),
        public_checked: false,
        public_authority_available: generation.allows_public_signing(),
        reason: public_workflow_reason(&generation),
        evidence_path: shot.path.join("TOHSENO/shot.json").display().to_string(),
    };
    if json {
        print_json(&view)?;
    } else {
        bus.emit(Event::result(format!(
            "shot {} is locally verified and private.",
            record.sequence
        )));
        bus.emit(Event::status(if generation.allows_public_signing() {
            "public witness not checked: the registry workflow is not implemented in this build."
        } else {
            "public witness not checked: no contract generation is active."
        }));
    }
    Ok(())
}

fn verified_local_head(
    app_name: &str,
) -> Result<(Evolution, ShotRecord), Box<dyn std::error::Error>> {
    tohseno_engine::ledger::validate_app_name(app_name)?;
    let fascia_reference = protocol_lifecycle::reference_fascia_root()?;
    let local = resolve_verification(app_name, &fascia_reference)?;
    if !local.conformant() {
        return Err("local Shot lineage is not conformant".into());
    }
    let ledger = Ledger::discover()?;
    let shot = ledger
        .latest_evolution(app_name)?
        .ok_or("app has no complete Shot")?;
    let record: ShotRecord =
        tohseno_protocol::canonical::from_slice(&fs::read(shot.path.join("TOHSENO/shot.json"))?)?;
    record.validate()?;
    Ok((shot, record))
}

#[derive(serde::Serialize)]
struct UnfinishedAttempt {
    schema: &'static str,
    app_name: String,
    attempt_directory: String,
    stages: Vec<UnfinishedStage>,
}

#[derive(serde::Serialize)]
struct UnfinishedStage {
    id: &'static str,
    status: &'static str,
}

/// Speaks honestly about an app whose newest attempt never finalized, instead
/// of refusing with "no complete Shot". Returns true when it reported one.
fn report_unfinished_app(
    target: &str,
    json: bool,
    bus: &EventBus,
) -> Result<bool, Box<dyn std::error::Error>> {
    if fs::symlink_metadata(target).is_ok()
        || tohseno_engine::ledger::validate_app_name(target).is_err()
    {
        return Ok(false);
    }
    let ledger = Ledger::discover()?;
    if ledger.load_app(target).is_err() || !ledger.list_evolutions(target)?.is_empty() {
        return Ok(false);
    }
    let app_dir = ledger.root().join(target).join(".tohseno");
    let mut attempts: Vec<PathBuf> = Vec::new();
    for parent in [app_dir.join("evolutions"), app_dir.join("incomplete")] {
        let Ok(entries) = fs::read_dir(parent) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                attempts.push(entry.path());
            }
        }
    }
    attempts.sort();
    let Some(attempt) = attempts.last() else {
        return Ok(false);
    };
    let has_project = fs::read_dir(attempt.join("src"))
        .map(|entries| {
            entries.flatten().any(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "xcodeproj")
            })
        })
        .unwrap_or(false);
    let has_artifact = fs::read_dir(attempt.join("artifact"))
        .map(|entries| {
            entries.flatten().any(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "app")
            })
        })
        .unwrap_or(false);
    let stage = |done: bool| if done { "pass" } else { "pending" };
    let stages = vec![
        UnfinishedStage {
            id: "intention.recorded",
            status: stage(attempt.join("prompt.md").is_file()),
        },
        UnfinishedStage {
            id: "world.generated",
            status: stage(has_project),
        },
        UnfinishedStage {
            id: "world.memory",
            status: stage(attempt.join("src/MEMORY.md").is_file()),
        },
        UnfinishedStage {
            id: "artifact.materialized",
            status: stage(has_artifact),
        },
        UnfinishedStage {
            id: "record.prepared",
            status: stage(attempt.join("TOHSENO/shot.json").is_file()),
        },
        UnfinishedStage {
            id: "record.signed",
            status: stage(attempt.join("TOHSENO/signature.json").is_file()),
        },
        UnfinishedStage {
            id: "conformance.receipt",
            status: stage(attempt.join("TOHSENO/conformance.json").is_file()),
        },
        UnfinishedStage {
            id: "evolution.finalized",
            status: stage(attempt.join(".complete").is_file()),
        },
    ];
    let view = UnfinishedAttempt {
        schema: "tohseno.cli-unfinished-attempt/1",
        app_name: target.into(),
        attempt_directory: attempt.display().to_string(),
        stages,
    };
    if json {
        print_json(&view)?;
    } else {
        bus.emit(Event::status(format!(
            "{target} · newest unfinished attempt"
        )));
        for stage in &view.stages {
            let marker = if stage.status == "pass" { "✓" } else { "–" };
            bus.emit(Event::status(format!("{marker} {}", stage.id)));
        }
        bus.emit(Event::status(format!(
            "next: `tohseno create {target}` — this attempt archives automatically."
        )));
    }
    Ok(true)
}

fn resolve_verification(
    target: &str,
    fascia_reference: &Path,
) -> Result<LocalVerification, Box<dyn std::error::Error>> {
    let shot_body = resolve_shot_body(target)?;
    let candidate = PathBuf::from(target);
    if fs::symlink_metadata(&candidate).is_ok() {
        return match resolve_shot(target) {
            Ok(shot) => {
                let expression = verifier::verify_shot_directory(&shot.path, fascia_reference);
                Ok(match shot_body {
                    Some(shot_body) => LocalVerification::EvolutionAndShotBody {
                        expression,
                        shot_body,
                    },
                    None => LocalVerification::Evolution(expression),
                })
            }
            Err(error) => shot_body.map(LocalVerification::ShotBody).ok_or(error),
        };
    }

    tohseno_engine::ledger::validate_app_name(target)?;
    let ledger = Ledger::discover()?;
    let shots = ledger.list_evolutions(target)?;
    if shots.is_empty() {
        return shot_body
            .map(LocalVerification::ShotBody)
            .ok_or_else(|| "app has no complete Shot".into());
    }
    let protocol_start = shots.iter().position(|shot| {
        fs::symlink_metadata(shot.path.join("TOHSENO"))
            .map(|metadata| metadata.is_dir() || metadata.file_type().is_symlink())
            .unwrap_or(false)
    });
    let roots = match protocol_start {
        Some(index) => shots[index..]
            .iter()
            .map(|shot| shot.path.clone())
            .collect::<Vec<_>>(),
        None => vec![shots.last().ok_or("app has no complete Shot")?.path.clone()],
    };
    let expression = verifier::verify_lineage_directories(&roots, fascia_reference);
    Ok(match shot_body {
        Some(shot_body) => LocalVerification::LineageAndShotBody {
            expression,
            shot_body,
        },
        None => LocalVerification::Lineage(expression),
    })
}

fn resolve_shot_body(
    target: &str,
) -> Result<Option<ShotBodyVerification>, Box<dyn std::error::Error>> {
    let candidate = PathBuf::from(target);
    if let Ok(metadata) = fs::symlink_metadata(&candidate) {
        if metadata.file_type().is_symlink() {
            return Err("Shot target must not be a symbolic link".into());
        }
        let start = if metadata.is_file() {
            candidate
                .parent()
                .ok_or("Shot target has no parent directory")?
                .to_path_buf()
        } else {
            candidate
        };
        let root = std::iter::successors(Some(start.as_path()), |path| path.parent())
            .take(5)
            .find(|path| {
                path.join(".tohseno/lineage.jsonl").is_file()
                    || path.join(".tohseno/legacy-v1.json").is_file()
            });
        return root
            .map(|root| ShotLayout::at(root).verify_shot_body(None))
            .transpose()
            .map_err(Into::into);
    }

    if tohseno_engine::ledger::validate_app_name(target).is_err() {
        return Ok(None);
    }
    let ledger = Ledger::discover()?;
    let app = match ledger.load_app(target) {
        Ok(app) => app,
        Err(tohseno_engine::LedgerError::AppMissing(_)) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let root = ledger.working_tree(target);
    if !root.join(".tohseno/lineage.jsonl").is_file()
        && !root.join(".tohseno/legacy-v1.json").is_file()
    {
        return Ok(None);
    }
    Ok(Some(
        ShotLayout::at(root).verify_shot_body(app.expression_id)?,
    ))
}

fn render_verification(report: &VerificationOutput, bus: &EventBus) {
    bus.emit(Event::status("TOHSENO SHOT"));
    for check in report.local.checks() {
        let marker = match check.status {
            VerificationStatus::Pass => "✓",
            VerificationStatus::Fail => "×",
            VerificationStatus::NotChecked => "–",
        };
        let detail = if check.status == VerificationStatus::Fail {
            format!(" — {}", check.observed)
        } else {
            String::new()
        };
        bus.emit(Event::status(format!("{marker} {}{detail}", check.id)));
    }
    if let Some(body) = report.local.shot_body() {
        bus.emit(Event::status(format!(
            "✓ lineage.v{} · {} signed action(s)",
            body.protocol_version, body.lineage_sequence
        )));
        bus.emit(Event::status(format!(
            "{} intention.exact-bytes",
            if body.intention_bytes_verified {
                "✓"
            } else {
                "–"
            }
        )));
        bus.emit(Event::status(format!(
            "{} genome.accepted",
            if body.genome_revision.is_some() {
                "✓"
            } else {
                "–"
            }
        )));
        bus.emit(Event::status(format!(
            "{} expression.embedded-identity",
            if body.selected_version_id.is_none() || body.embedded_metadata_verified {
                "✓"
            } else {
                "×"
            }
        )));
    }
    if report.conformant {
        bus.emit(Event::result("CONFORMANT · locally verified."));
    } else {
        bus.emit(Event::status("NONCONFORMANT"));
    }
}

fn resolve_shot(target: &str) -> Result<Evolution, Box<dyn std::error::Error>> {
    let path = PathBuf::from(target);
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if metadata.file_type().is_symlink() {
            return Err("Shot target must not be a symbolic link".into());
        }
        let path = if metadata.is_file() {
            path.parent()
                .ok_or("record path has no parent")?
                .to_path_buf()
        } else {
            path
        };
        let shot_path = if path.ends_with("TOHSENO") {
            path.parent()
                .ok_or("TOHSENO path has no parent")?
                .to_path_buf()
        } else {
            path
        };
        if !shot_path.join("TOHSENO/shot.json").is_file() {
            return Err("path does not identify a completed Shot directory".into());
        }
        let number = shot_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("shot directory name is invalid")?
            .parse::<u32>()?;
        let app_name = shot_path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .ok_or("shot path does not have ledger anatomy")?
            .to_owned();
        return Ok(Evolution {
            app_name,
            number,
            path: shot_path,
        });
    }
    tohseno_engine::ledger::validate_app_name(target)?;
    let ledger = Ledger::discover()?;
    ledger
        .latest_evolution(target)?
        .ok_or_else(|| "app has no complete Shot".into())
}

fn read_record_pair(
    path: &Path,
) -> Result<(ShotRecord, SignatureSidecar), Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("record path must be a regular non-symlinked file".into());
    }
    let record: ShotRecord = tohseno_protocol::canonical::from_slice(&fs::read(path)?)?;
    record.validate()?;
    let signature_path = path
        .parent()
        .ok_or("record has no parent directory")?
        .join("signature.json");
    let signature_metadata = fs::symlink_metadata(&signature_path)?;
    if signature_metadata.file_type().is_symlink() || !signature_metadata.is_file() {
        return Err("signature path must be a regular non-symlinked file".into());
    }
    let signature: SignatureSidecar =
        tohseno_protocol::canonical::from_slice(&fs::read(signature_path)?)?;
    Ok((record, signature))
}

fn print_json(value: &impl Serialize) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[derive(Serialize)]
struct ProtocolInfo {
    schema: &'static str,
    protocol: &'static str,
    product_version: &'static str,
    release_status: &'static str,
    /// Frozen v1 field retained for existing machine consumers.
    shot_schema: &'static str,
    compatibility_shot_schema: &'static str,
    lineage_protocol_version: &'static str,
    lineage_action_schema: &'static str,
    lineage_schema_version: u32,
    signature_schema: &'static str,
    fascia: &'static str,
    app_metadata_schemas: [&'static str; 2],
    supported_token_association_chains: [u64; 2],
    contract_generation: ContractGenerationSummary,
    active_generation: Option<String>,
    public_authority_available: bool,
}

#[derive(Serialize)]
struct ContractGenerationSummary {
    generation: String,
    protocol_major: u64,
    definition_path: &'static str,
    definition_digest: String,
    status: &'static str,
    chain_id: u64,
    conditional_create2: ConditionalCreate2Coordinates,
}

impl ContractGenerationSummary {
    fn from_resolved(resolved: &ResolvedContractGeneration) -> Self {
        Self {
            generation: resolved.definition.generation.clone(),
            protocol_major: resolved.definition.protocol_major,
            definition_path: CURRENT_GENERATION_REPOSITORY_PATH,
            definition_digest: resolved.definition_digest.to_string(),
            status: if resolved.allows_public_signing() {
                "active"
            } else {
                "deployed_inactive_untrusted"
            },
            chain_id: resolved.definition.chain.chain_id,
            conditional_create2: ConditionalCreate2Coordinates {
                condition:
                    "only if the exact declared init code is deployed by the declared CREATE2 deployer and later authorized by a signed activation",
                deployer: resolved.definition.create2.deployer.to_string(),
                builder_account_factory: resolved
                    .definition
                    .create2
                    .builder_account_factory
                    .predicted_address
                    .to_string(),
                shot_registry: resolved
                    .definition
                    .create2
                    .shot_registry
                    .predicted_address
                    .to_string(),
            },
        }
    }
}

#[derive(Serialize)]
struct ConditionalCreate2Coordinates {
    condition: &'static str,
    deployer: String,
    builder_account_factory: String,
    shot_registry: String,
}

#[derive(Serialize)]
struct NetworkStatus {
    schema: &'static str,
    protocol: &'static str,
    product_version: &'static str,
    contract_generation: ContractGenerationSummary,
    active_generation: Option<String>,
    ready: bool,
    rpc_checked: bool,
    public_authority_available: bool,
    reason: &'static str,
}

#[derive(Serialize)]
struct RecordVerification {
    conformant: bool,
    record_path: String,
    shot_id: String,
    builder_id: String,
    sequence: u32,
    commitment: String,
    signer_key_id: String,
}

#[derive(Serialize)]
#[serde(tag = "scope", content = "report", rename_all = "snake_case")]
enum LocalVerification {
    Evolution(ShotVerificationReport),
    Lineage(LineageVerificationReport),
    ShotBody(ShotBodyVerification),
    EvolutionAndShotBody {
        expression: ShotVerificationReport,
        shot_body: ShotBodyVerification,
    },
    LineageAndShotBody {
        expression: LineageVerificationReport,
        shot_body: ShotBodyVerification,
    },
}

impl LocalVerification {
    fn conformant(&self) -> bool {
        match self {
            Self::Evolution(report) => report.conformant,
            Self::Lineage(report) => report.conformant,
            Self::ShotBody(_) => true,
            Self::EvolutionAndShotBody { expression, .. } => expression.conformant,
            Self::LineageAndShotBody { expression, .. } => expression.conformant,
        }
    }

    fn checks(&self) -> Vec<&VerificationCheck> {
        match self {
            Self::Evolution(report) => report.checks.iter().collect(),
            Self::Lineage(report) => report
                .shots
                .iter()
                .flat_map(|shot| shot.checks.iter())
                .chain(std::iter::once(&report.lineage))
                .collect(),
            Self::ShotBody(_) => Vec::new(),
            Self::EvolutionAndShotBody { expression, .. } => expression.checks.iter().collect(),
            Self::LineageAndShotBody { expression, .. } => expression
                .shots
                .iter()
                .flat_map(|shot| shot.checks.iter())
                .chain(std::iter::once(&expression.lineage))
                .collect(),
        }
    }

    fn shot_body(&self) -> Option<&ShotBodyVerification> {
        match self {
            Self::ShotBody(report) => Some(report),
            Self::EvolutionAndShotBody { shot_body, .. }
            | Self::LineageAndShotBody { shot_body, .. } => Some(shot_body),
            Self::Evolution(_) | Self::Lineage(_) => None,
        }
    }
}

#[derive(Serialize)]
struct VerificationOutput {
    schema: &'static str,
    conformant: bool,
    local: LocalVerification,
}

#[derive(Serialize)]
struct RegistryView {
    schema: &'static str,
    app_name: String,
    shot_id: String,
    local_head: String,
    local_sequence: u32,
    local_state: &'static str,
    local_verified: bool,
    active_generation: Option<String>,
    public_checked: bool,
    public_authority_available: bool,
    reason: &'static str,
    evidence_path: String,
}

#[derive(Serialize)]
struct Inspection {
    app_name: String,
    shot_directory: String,
    shot_id: String,
    builder_id: String,
    sequence: u32,
    previous: Option<String>,
    commitment: String,
    source_tree_sha256: String,
    fascia_sha256: String,
    signer_key_id: String,
    conformant: bool,
    public_state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    shot_body: Option<ShotBodyVerification>,
}

#[derive(Serialize)]
struct ShotBodyInspection {
    schema: &'static str,
    target: String,
    local_state: &'static str,
    ownership_acquired: bool,
    source_materialized: bool,
    report: ShotBodyVerification,
}

impl ShotBodyInspection {
    fn from_report(target: &str, report: ShotBodyVerification) -> Self {
        Self {
            schema: "tohseno.cli-shot-body-inspection/1",
            target: target.into(),
            local_state: "verified_records",
            ownership_acquired: false,
            source_materialized: report.embedded_metadata_verified,
            report,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_info_reports_stable_product_and_deployed_inactive_generation() {
        let info = serde_json::to_value(protocol_info().unwrap()).unwrap();
        assert_eq!(info["schema"], "tohseno.protocol-info/2");
        assert_eq!(info["product_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(info["release_status"], "stable");
        assert_eq!(info["contract_generation"]["generation"], "0.8.0");
        assert_eq!(
            info["contract_generation"]["status"],
            "deployed_inactive_untrusted"
        );
        assert!(info["contract_generation"]["definition_digest"]
            .as_str()
            .is_some_and(|value| value.starts_with("0x") && value.len() == 66));
        assert!(info["active_generation"].is_null());
        assert_eq!(info["public_authority_available"], false);
        assert!(info.get("deployment").is_none());
        assert!(info.get("candidate_version").is_none());
        assert!(info["contract_generation"]["conditional_create2"]
            .get("shot_relations")
            .is_none());
    }

    #[test]
    fn network_status_is_offline_and_inactive() {
        let status = serde_json::to_value(inactive_network_status().unwrap()).unwrap();
        assert_eq!(status["schema"], "tohseno.network-status/2");
        assert_eq!(status["rpc_checked"], false);
        assert_eq!(status["ready"], false);
        assert!(status["active_generation"].is_null());
        assert_eq!(status["contract_generation"]["generation"], "0.8.0");
        assert!(status["reason"]
            .as_str()
            .unwrap()
            .contains("no trusted release-authority"));
    }

    #[test]
    fn public_verification_fails_closed_without_rpc() {
        let error = ensure_public_verification_available()
            .unwrap_err()
            .to_string();
        assert!(error.starts_with("public verification unavailable:"));
        assert!(error.contains("no RPC was contacted"));
        assert!(error.contains("signed chain activation"));
    }
}
