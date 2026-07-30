use serde::Serialize;
use std::path::{Path, PathBuf};
use tohseno_engine::{
    Engine, Event, EventBus, ImportedShot, PortableShotManifest, PortableVisibility, ShotLayout,
    StoredFeedback,
};
use tohseno_protocol::digest::{Address20, Bytes32};
use tohseno_protocol::ontology::{
    AvailabilityStatus, TokenAssociation, TokenAssociationOperation, TOKEN_ASSOCIATION_SCHEMA,
};

pub fn record_feedback(
    engine: &Engine,
    app_name: &str,
    version_ordinal: u64,
    text: &str,
    attachments: &[PathBuf],
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let stored =
        engine.record_feedback_with_attachments(app_name, version_ordinal, text, attachments)?;
    let output = FeedbackOutput::new(version_ordinal, &stored);
    if json {
        print_json(&output)?;
    } else {
        bus.emit(Event::result(format!(
            "private feedback {} is bound to version {version_ordinal:04}; signed action {}.",
            output.feedback_id, output.action_commitment
        )));
        bus.emit(Event::status(format!(
            "stored under {} · {} local attachment(s).",
            output.directory,
            output.attachments.len()
        )));
    }
    Ok(())
}

pub fn export_shot(
    engine: &Engine,
    app_name: &str,
    destination: &Path,
    include_private: bool,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let _lock = engine.ledger().lock_app(app_name)?;
    let app = engine.ledger().load_app(app_name)?;
    let destination = absolute(destination)?;
    let visibility = if include_private {
        PortableVisibility::IncludePrivate
    } else {
        PortableVisibility::Public
    };
    let layout = ShotLayout::at(engine.ledger().working_tree(&app.name));
    let manifest = layout.export_bundle(&destination, visibility)?;
    let manifest_digest = tohseno_protocol::canonical::sha256_commitment(&manifest)?;
    let output = ExportOutput {
        destination: destination.display().to_string(),
        manifest_digest,
        manifest,
    };
    if json {
        print_json(&output)?;
    } else {
        bus.emit(Event::result(format!(
            "portable Shot records exported to {}.",
            output.destination
        )));
        bus.emit(Event::status(format!(
            "manifest {} · this is not source, ownership transfer, or trusted materialization.",
            output.manifest_digest
        )));
    }
    Ok(())
}

pub fn import_shot(
    bundle: &Path,
    destination: &Path,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let bundle = absolute(bundle)?;
    let destination = absolute(destination)?;
    let imported = ShotLayout::import_bundle(&bundle, &destination)?;
    let output = ImportOutput::new(bundle, destination, imported);
    if json {
        print_json(&output)?;
    } else {
        bus.emit(Event::result(format!(
            "verified Shot records imported to {}.",
            output.destination
        )));
        bus.emit(Event::handoff(
            "The import follows verified lineage only. It did not acquire ownership, clone source, or materialize an expression.",
        ));
    }
    Ok(())
}

pub fn migrate_v1(
    engine: &Engine,
    app_name: &str,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let _lock = engine.ledger().lock_app(app_name)?;
    let adapted = engine.ledger().migrate_v1_identity(app_name)?;
    let layout = ShotLayout::at(engine.ledger().working_tree(app_name));
    let projection = layout.write_v1_migration(&adapted)?;
    let output = MigrationOutput {
        app_name: app_name.to_owned(),
        shot_id: adapted.shot_id.to_string(),
        expression_id: adapted.expression_id.to_string(),
        controller: adapted.controller.to_string(),
        versions: adapted.entries.len(),
        head: adapted.head.to_string(),
        intention_status: "unknown",
        genome_status: "unknown",
        projection: projection.display().to_string(),
        historical_records_rewritten: false,
    };
    if json {
        print_json(&output)?;
    } else {
        bus.emit(Event::result(format!(
            "{} frozen v1 version(s) were projected without rewriting history.",
            output.versions
        )));
        bus.emit(Event::status(format!(
            "Shot {} · expression {} · intention and Shot genome remain unknown.",
            output.shot_id, output.expression_id
        )));
    }
    Ok(())
}

pub fn migrate_legacy_v0_6(
    engine: &Engine,
    app_name: Option<&str>,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let migrated = engine.ledger().migrate_legacy_v0_6_apps(app_name)?;
    let mut projections = Vec::with_capacity(migrated.len());
    for name in &migrated {
        let adapted = engine.ledger().migrate_v1_identity(name)?;
        let layout = ShotLayout::at(engine.ledger().working_tree(name));
        let projection = layout.write_v1_migration(&adapted)?;
        projections.push(LegacyMigrationProjection {
            app_name: name.clone(),
            versions: adapted.entries.len(),
            shot_id: adapted.shot_id.to_string(),
            expression_id: adapted.expression_id.to_string(),
            projection: projection.display().to_string(),
        });
    }

    let output = LegacyMigrationOutput {
        source: engine
            .ledger()
            .machine_root()
            .join("apps")
            .display()
            .to_string(),
        destination: engine.ledger().root().display().to_string(),
        historical_source_deleted: false,
        apps: projections,
    };
    if json {
        print_json(&output)?;
    } else if output.apps.is_empty() {
        bus.emit(Event::status(
            "no preserved v0.6 apps were found; nothing changed.",
        ));
    } else {
        for app in &output.apps {
            bus.emit(Event::result(format!(
                "{} moved into the visible family with {} frozen version(s).",
                app.app_name, app.versions
            )));
        }
        bus.emit(Event::handoff(format!(
            "The original v0.6 ledger remains untouched at {}.",
            output.source
        )));
    }
    Ok(())
}

pub fn show_genome(
    engine: &Engine,
    app_name: &str,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let _lock = engine.ledger().lock_app(app_name)?;
    let app = engine.ledger().load_app(app_name)?;
    let layout = ShotLayout::at(engine.ledger().working_tree(app_name));
    layout.verify_shot_body(app.expression_id)?;
    let state = tohseno_protocol::reduce_lineage(&layout.read_lineage()?)?;
    let accepted = state
        .accepted_genome
        .ok_or("the Shot has no explicitly accepted Genome")?;
    if json {
        print_json(&accepted.genome)?;
    } else {
        bus.emit(Event::result(format!(
            "Genome revision {} is accepted and synchronized.",
            accepted.genome.revision
        )));
        bus.emit(Event::status(tohseno_engine::render_genome_document(
            &accepted.genome,
        )?));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn accept_genome(
    engine: &Engine,
    app_name: &str,
    genome: &tohseno_protocol::Genome,
    rationale: &str,
    mutations: &[String],
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let accepted = engine.accept_genome(app_name, genome, rationale, mutations)?;
    let output = GenomeAcceptanceOutput {
        app_name: app_name.into(),
        revision: accepted.genome.revision,
        genome_digest: accepted.genome.digest()?,
        proposal_action: accepted.proposal_action,
        acceptance_action: accepted.acceptance_action,
        lineage_head: accepted.lineage_head,
        explicit_mutations: mutations.to_vec(),
    };
    if json {
        print_json(&output)?;
    } else {
        bus.emit(Event::result(format!(
            "Genome revision {} explicitly accepted.",
            output.revision
        )));
        bus.emit(Event::status(format!(
            "digest {} · lineage head {}.",
            output.genome_digest, output.lineage_head
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn associate_token(
    engine: &Engine,
    app_name: &str,
    chain_id: u64,
    token_address: &str,
    symbol: Option<&str>,
    public: bool,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    record_token_relation(
        engine,
        app_name,
        TokenAssociation {
            schema: TOKEN_ASSOCIATION_SCHEMA.into(),
            operation: TokenAssociationOperation::Associate,
            chain_id,
            token: parse_address(token_address)?,
            symbol: symbol.map(str::to_owned),
            anchor: None,
        },
        public,
        json,
        bus,
    )
}

pub fn remove_token(
    engine: &Engine,
    app_name: &str,
    chain_id: u64,
    token_address: &str,
    public: bool,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    record_token_relation(
        engine,
        app_name,
        TokenAssociation {
            schema: TOKEN_ASSOCIATION_SCHEMA.into(),
            operation: TokenAssociationOperation::Remove,
            chain_id,
            token: parse_address(token_address)?,
            symbol: None,
            anchor: None,
        },
        public,
        json,
        bus,
    )
}

fn record_token_relation(
    engine: &Engine,
    app_name: &str,
    association: TokenAssociation,
    public: bool,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    if public {
        return Err(
            "public Token Association export is disabled: ordinary lineage actions may commit private predecessors; omit --public to record the relation privately"
                .into(),
        );
    }
    let availability = AvailabilityStatus::IntentionallyPrivate;
    let receipt = engine.record_token_association(app_name, association.clone(), availability)?;
    let output = TokenAssociationOutput {
        schema: "tohseno.cli-token-association/1",
        operation: match association.operation {
            TokenAssociationOperation::Associate => "associate",
            TokenAssociationOperation::Remove => "remove",
        },
        shot_id: receipt.action.action.shot_id.to_string(),
        chain_id: association.chain_id,
        token_address: association.token.to_string(),
        symbol: association.symbol,
        availability,
        action_commitment: receipt.action_commitment,
        lineage_head: receipt.lineage_head,
        outbox_path: receipt
            .outbox_path
            .as_ref()
            .map(|path| path.display().to_string()),
        shot_identity_changed: false,
        ownership_changed: false,
        chain_anchor_verified: false,
        relayed: false,
    };
    if json {
        print_json(&output)?;
    } else {
        bus.emit(Event::result(format!(
            "Token Association {} recorded for Shot {} without changing identity or ownership.",
            output.operation, output.shot_id
        )));
        if let Some(path) = &output.outbox_path {
            bus.emit(Event::handoff(format!(
                "The canonical public action is ready at {path}. Review it, then ingest it explicitly with `tohseno-node ingest {path}`."
            )));
        } else {
            bus.emit(Event::status(
                "The signed relation remains intentionally private and was not relayed or anchored.",
            ));
        }
    }
    Ok(())
}

fn parse_address(value: &str) -> Result<Address20, serde_json::Error> {
    serde_json::from_value(serde_json::Value::String(value.to_owned()))
}

fn absolute(path: &Path) -> Result<PathBuf, std::io::Error> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn print_json(value: &impl Serialize) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[derive(Serialize)]
struct FeedbackOutput {
    feedback_id: String,
    action_commitment: String,
    version_ordinal: u64,
    visibility: &'static str,
    directory: String,
    attachments: Vec<String>,
}

impl FeedbackOutput {
    fn new(version_ordinal: u64, stored: &StoredFeedback) -> Self {
        Self {
            feedback_id: stored.feedback_id.to_string(),
            action_commitment: stored.action_commitment.to_string(),
            version_ordinal,
            visibility: "intentionally_private",
            directory: stored.directory.display().to_string(),
            attachments: stored
                .attachments
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct ExportOutput {
    destination: String,
    manifest_digest: Bytes32,
    manifest: PortableShotManifest,
}

#[derive(Serialize)]
struct ImportOutput {
    source_bundle: String,
    destination: String,
    shot_id: String,
    controller: String,
    lineage_head: String,
    action_count: u64,
    visibility: PortableVisibility,
    ownership_acquired: bool,
    source_materialized: bool,
}

impl ImportOutput {
    fn new(source_bundle: PathBuf, destination: PathBuf, imported: ImportedShot) -> Self {
        Self {
            source_bundle: source_bundle.display().to_string(),
            destination: destination.display().to_string(),
            shot_id: imported.manifest.shot_id.to_string(),
            controller: imported.manifest.controller.to_string(),
            lineage_head: imported.manifest.lineage_head.to_string(),
            action_count: imported.manifest.action_count,
            visibility: imported.manifest.visibility,
            ownership_acquired: false,
            source_materialized: false,
        }
    }
}

#[derive(Serialize)]
struct MigrationOutput {
    app_name: String,
    shot_id: String,
    expression_id: String,
    controller: String,
    versions: usize,
    head: String,
    intention_status: &'static str,
    genome_status: &'static str,
    projection: String,
    historical_records_rewritten: bool,
}

#[derive(Debug, Serialize)]
struct LegacyMigrationOutput {
    source: String,
    destination: String,
    historical_source_deleted: bool,
    apps: Vec<LegacyMigrationProjection>,
}

#[derive(Debug, Serialize)]
struct LegacyMigrationProjection {
    app_name: String,
    versions: usize,
    shot_id: String,
    expression_id: String,
    projection: String,
}

#[derive(Serialize)]
struct GenomeAcceptanceOutput {
    app_name: String,
    revision: u64,
    genome_digest: Bytes32,
    proposal_action: Bytes32,
    acceptance_action: Bytes32,
    lineage_head: Bytes32,
    explicit_mutations: Vec<String>,
}

#[derive(Serialize)]
struct TokenAssociationOutput {
    schema: &'static str,
    operation: &'static str,
    shot_id: String,
    chain_id: u64,
    token_address: String,
    symbol: Option<String>,
    availability: AvailabilityStatus,
    action_commitment: Bytes32,
    lineage_head: Bytes32,
    outbox_path: Option<String>,
    shot_identity_changed: bool,
    ownership_changed: bool,
    chain_anchor_verified: bool,
    relayed: bool,
}
