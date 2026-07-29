use crate::builder_identity::{BuilderIdentity, BuilderIdentityError, BuilderIdentityManager};
use crate::config::{Config, ConfigError, HarnessConfig};
use crate::events::{Event, EventBus};
use crate::gates::apple_signing::{self, AppleSigningState};
use crate::gates::device::{self, DeviceState};
use crate::gates::intent::{Intent, IntentError};
use crate::gates::toolchain::{self, ToolchainState};
use crate::gates::{build, install, sign};
use crate::genome::{Genome, GenomeError};
use crate::harness::{
    discover_harnesses, selected_harness, Harness, HarnessError, HarnessMode, HarnessOption,
};
use crate::ledger::{sanitize_component, AppRecord, Ledger, LedgerError, Shot};
use crate::protocol_lifecycle::{self, ProtocolLifecycleError};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tohseno_protocol::record::ShotOrigin;

#[derive(Clone, Debug)]
pub struct ShotRequest {
    pub app_name: String,
    pub intent: Intent,
    pub harness: Option<String>,
}

struct ShotExecution {
    request: ShotRequest,
    shot: Shot,
    app: AppRecord,
    previous_source: Option<PathBuf>,
    harness_config: HarnessConfig,
    builder: BuilderIdentity,
    origin: Option<ShotOrigin>,
}

pub struct Engine {
    ledger: Ledger,
    events: EventBus,
    config: Config,
    genome: Genome,
}

/// A conducted birth: the folder exists, the briefing is written, and the
/// builder's own agent takes it from here.
pub struct ConductedCreation {
    pub folder: PathBuf,
    pub task_path: PathBuf,
}

impl Engine {
    pub fn discover(events: EventBus) -> Result<Self, EngineError> {
        let ledger = Ledger::discover()?;
        ledger.initialize()?;
        let config = Config::load_or_create(ledger.machine_root())?;
        Ok(Self {
            ledger,
            events,
            config,
            genome: Genome,
        })
    }

    pub fn at(ledger: Ledger, events: EventBus, config: Config) -> Self {
        Self {
            ledger,
            events,
            config,
            genome: Genome,
        }
    }

    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    pub fn harnesses(&self) -> Vec<HarnessOption> {
        discover_harnesses(&self.config.harness)
    }

    pub async fn create(&self, request: ShotRequest) -> Result<Shot, EngineError> {
        crate::ledger::validate_app_name(&request.app_name)?;
        let _app_lock = self.ledger.lock_app(&request.app_name)?;
        let harness = self.harness_for_request(request.harness.as_deref())?;
        self.check_slot_limit()?;
        self.emit_upsell_once(
            "welcome",
            "first shot: Xcode + Apple ID now · iPhone later · free Apple IDs refresh weekly.",
        )?;
        self.events
            .emit(Event::status("preparing your TOHSENO identity…"));
        let builder = BuilderIdentityManager::for_ledger(&self.ledger).ensure()?;
        let proposed_bundle_id = bundle_id(&request.app_name)?;
        let mut app = match self.ledger.load_app(&request.app_name) {
            Ok(existing) if existing.latest_shot.is_none() => existing,
            Ok(_) => return Err(LedgerError::AppExists(request.app_name.clone()).into()),
            Err(LedgerError::AppMissing(_)) => self
                .ledger
                .create_app(&request.app_name, &proposed_bundle_id)?,
            Err(error) => return Err(error.into()),
        };
        if app.shot_id.is_none() && app.builder_id.is_none() {
            app = self.ledger.bind_protocol_identity(
                &request.app_name,
                tohseno_protocol::digest::ShotId::random(),
                builder.builder_id,
            )?;
        }
        if app.builder_id != Some(builder.builder_id) {
            return Err(EngineError::BuilderMismatch(request.app_name.clone()));
        }
        if self.working_tree_has_content(&request.app_name)? {
            return Err(EngineError::FolderInProgress(request.app_name.clone()));
        }
        let shot = self.ledger.reserve_shot(&request.app_name, None)?;
        self.run_shot(ShotExecution {
            request,
            shot,
            app,
            previous_source: None,
            harness_config: harness,
            builder,
            origin: None,
        })
        .await
    }

    pub async fn evolve(&self, request: ShotRequest) -> Result<Shot, EngineError> {
        crate::ledger::validate_app_name(&request.app_name)?;
        let _app_lock = self.ledger.lock_app(&request.app_name)?;
        let harness = self.harness_for_request(request.harness.as_deref())?;
        let app = self.ledger.load_app(&request.app_name)?;
        if app.shot_id.is_none() || app.builder_id.is_none() {
            return Err(EngineError::LegacyRequiresAdoption(
                request.app_name.clone(),
            ));
        }
        let builder = BuilderIdentityManager::for_ledger(&self.ledger).ensure()?;
        if app.builder_id != Some(builder.builder_id) {
            return Err(EngineError::BuilderMismatch(request.app_name.clone()));
        }
        let previous = self
            .ledger
            .latest_shot(&request.app_name)?
            .ok_or_else(|| EngineError::NoCompleteShot(request.app_name.clone()))?;
        protocol_lifecycle::verify_completed_evolution(&previous)?;
        if !self.working_tree_matches(&previous)? {
            return Err(EngineError::UnsealedChanges(request.app_name.clone()));
        }
        let shot = self
            .ledger
            .reserve_shot(&request.app_name, Some(previous.number))?;
        self.run_shot(ShotExecution {
            request,
            shot,
            app,
            previous_source: Some(previous.source_path()),
            harness_config: harness,
            builder,
            origin: None,
        })
        .await
    }

    pub async fn adopt(
        &self,
        app_name: &str,
        requested_harness: Option<&str>,
    ) -> Result<Shot, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let harness = self.harness_for_request(requested_harness)?;
        let mut app = self.ledger.load_app(app_name)?;
        if app.shot_id.is_some() || app.builder_id.is_some() {
            return Err(EngineError::AlreadyProtocol(app_name.into()));
        }
        let previous = self
            .ledger
            .latest_shot(app_name)?
            .ok_or_else(|| EngineError::NoCompleteShot(app_name.into()))?;
        let legacy_source_sha256 =
            tohseno_protocol::tree_hash::hash_source_tree(&previous.source_path())
                .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?
                .digest;
        let builder = BuilderIdentityManager::for_ledger(&self.ledger).ensure()?;
        app = self.ledger.bind_protocol_identity(
            app_name,
            tohseno_protocol::digest::ShotId::random(),
            builder.builder_id,
        )?;
        let shot = self.ledger.reserve_shot(app_name, Some(previous.number))?;
        let request = ShotRequest {
            app_name: app_name.into(),
            intent: Intent {
                prompt: "Adopt this existing unsigned app into the TOHSENO protocol without changing its purpose or private local behavior.".into(),
                images: Vec::new(),
            },
            harness: requested_harness.map(str::to_owned),
        };
        let origin = ShotOrigin::LegacyAdoption {
            legacy_latest_shot: previous.number,
            legacy_source_sha256,
        };
        self.run_shot(ShotExecution {
            request,
            shot,
            app,
            previous_source: Some(previous.source_path()),
            harness_config: harness,
            builder,
            origin: Some(origin),
        })
        .await
    }

    async fn run_shot(&self, execution: ShotExecution) -> Result<Shot, EngineError> {
        let ShotExecution {
            request,
            shot,
            app,
            previous_source,
            harness_config,
            builder,
            origin,
        } = execution;
        let bundle_id = app.bundle_id.clone();
        install::require_candidate_namespace(&bundle_id).map_err(EngineError::Install)?;
        let working_digest_at_start = self.working_digest(&request.app_name);
        self.events
            .emit(Event::status(format!("preparing shot {}…", shot.number)));
        let image_names = request
            .intent
            .write_to_shot(&self.ledger, &shot, &self.events)?;
        let genesis_input_sha256 = protocol_lifecycle::capture_input_commitment(&shot)?;
        self.genome.compose(
            &self.ledger,
            &shot,
            &request.app_name,
            &bundle_id,
            &image_names,
            previous_source.as_deref(),
        )?;

        let harness_mode = self.wait_for_prerequisites(&harness_config).await?;
        let harness = Harness::new(harness_config, self.events.clone());
        self.events.emit(Event::status(format!(
            "writing shot {} of {}…",
            shot.number, request.app_name
        )));
        harness
            .run(
                &self.ledger,
                &shot,
                harness_mode,
                "Read TASK.md first, then build the complete app in src/ and verify your work.",
            )
            .await?;

        let mut repair_pass = 0;
        loop {
            if let Err(error) = build::validate_complete_source(&shot.source_path()) {
                if repair_pass >= self.config.max_repair_passes {
                    return Err(EngineError::RepairExhausted {
                        shot: shot.number,
                        passes: self.config.max_repair_passes,
                    });
                }
                repair_pass += 1;
                self.events.emit(Event::status(format!(
                    "repairing shot {} · pass {} of {}…",
                    shot.number, repair_pass, self.config.max_repair_passes
                )));
                self.genome
                    .append_repair(&self.ledger, &shot, repair_pass, &error.to_string())?;
                harness
                    .run(
                        &self.ledger,
                        &shot,
                        harness_mode,
                        "Read TASK.md, restore the missing Apple Fascia anatomy, and leave the complete corrected project in src/.",
                    )
                    .await?;
                continue;
            }

            self.events
                .emit(Event::status(format!("building shot {}…", shot.number)));
            match build::compile(&self.ledger, &shot, &request.app_name)? {
                Ok(()) => break,
                Err(failure) if repair_pass < self.config.max_repair_passes => {
                    repair_pass += 1;
                    self.events.emit(Event::status(format!(
                        "repairing shot {} · pass {} of {}…",
                        shot.number, repair_pass, self.config.max_repair_passes
                    )));
                    self.genome
                        .append_repair(&self.ledger, &shot, repair_pass, &failure.output)?;
                    harness
                        .run(
                            &self.ledger,
                            &shot,
                            harness_mode,
                            "Read TASK.md, fix the latest build failure, and leave the complete corrected project in src/.",
                        )
                        .await?;
                }
                Err(_) => {
                    return Err(EngineError::RepairExhausted {
                        shot: shot.number,
                        passes: self.config.max_repair_passes,
                    });
                }
            }
        }

        self.complete_shot(
            &shot,
            &app,
            &builder,
            genesis_input_sha256,
            origin,
            &request.app_name,
            &bundle_id,
            working_digest_at_start,
        )
        .await
    }

    /// The shared birth of every Evolution: protocol record, Simulator
    /// artifact, signature, conformance, finalization, working-tree
    /// checkout, then a non-blocking phone offer. The Mac is enough; the
    /// phone is a destination resumed through `tohseno refresh`.
    #[allow(clippy::too_many_arguments)]
    async fn complete_shot(
        &self,
        shot: &Shot,
        app: &AppRecord,
        builder: &BuilderIdentity,
        genesis_input_sha256: tohseno_protocol::digest::Bytes32,
        origin: Option<ShotOrigin>,
        app_name: &str,
        bundle_id: &str,
        working_digest_at_start: Option<tohseno_protocol::digest::Bytes32>,
    ) -> Result<Shot, EngineError> {
        self.events
            .emit(Event::status(format!("committing shot {}…", shot.number)));
        let prepared = protocol_lifecycle::prepare_evolution(
            &self.ledger,
            shot,
            app,
            builder,
            genesis_input_sha256,
            origin,
        )?;

        self.events.emit(Event::status(format!(
            "materializing shot {}…",
            shot.number
        )));
        if let Err(failure) = build::materialize_artifact(&self.ledger, shot, app_name)? {
            return Err(EngineError::ArtifactUnbuildable(failure.output));
        }
        self.events
            .emit(Event::status(format!("verifying shot {}…", shot.number)));
        protocol_lifecycle::complete_evolution(&self.ledger, shot, builder, prepared)?;
        self.ledger.finalize_shot(shot)?;
        protocol_lifecycle::verify_completed_evolution(shot)?;
        self.ledger.set_retired(app_name, false)?;
        if self.working_digest(app_name) == working_digest_at_start {
            self.ledger.checkout_working_tree(shot)?;
        } else {
            self.events.emit(Event::status(
                "the folder changed while sealing; your newer edits are kept as unsealed changes.",
            ));
        }
        self.events.emit(Event::result(format!(
            "shot {} of {} is complete and verified on this Mac.",
            shot.number, app_name
        )));
        self.events.emit(Event::status(format!(
            "folder: {}",
            self.ledger.working_tree(app_name).display()
        )));

        match device::check() {
            Ok(DeviceState::Ready(_)) => {
                let artifact_directory = temporary_path("install");
                DevicePipeline::new(self.events.clone())
                    .build_install(
                        shot.number,
                        app_name,
                        bundle_id,
                        &shot.source_path(),
                        &artifact_directory,
                    )
                    .await?;
                self.events.emit(Event::result(format!(
                    "shot {} of {} is on your phone.",
                    shot.number, app_name
                )));
            }
            _ => {
                self.events.emit(Event::handoff(format!(
                    "Plug in your iPhone anytime, then run `tohseno refresh {app_name}`.",
                )));
            }
        }
        Ok(shot.clone())
    }

    /// Seals the working tree — however it got there — as the next Evolution.
    /// Editing is not a tohseno operation; sealing is.
    pub async fn seal(&self, app_name: &str, note: Option<&str>) -> Result<Shot, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        if app.shot_id.is_none() || app.builder_id.is_none() {
            return Err(EngineError::LegacyRequiresAdoption(app_name.into()));
        }
        let builder = BuilderIdentityManager::for_ledger(&self.ledger).ensure()?;
        if app.builder_id != Some(builder.builder_id) {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }
        let working = self.ledger.working_tree(app_name);
        let has_project = fs::read_dir(&working)
            .map(|entries| {
                entries.flatten().any(|entry| {
                    entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "xcodeproj")
                })
            })
            .unwrap_or(false);
        if !has_project {
            return Err(EngineError::NothingToSeal(app_name.into()));
        }
        let previous = self.ledger.latest_shot(app_name)?;
        if let Some(previous) = &previous {
            protocol_lifecycle::verify_completed_evolution(previous)?;
            if self.working_tree_matches(previous)? {
                self.events.emit(Event::result(format!(
                    "nothing new to seal — the folder matches shot {}.",
                    previous.number
                )));
                return Ok(previous.clone());
            }
        } else {
            self.check_slot_limit()?;
        }
        self.wait_for_apple_prerequisites().await?;
        let working_digest_at_start = self.working_digest(app_name);
        let shot = self
            .ledger
            .reserve_shot(app_name, previous.as_ref().map(|shot| shot.number))?;
        self.events
            .emit(Event::status(format!("preparing shot {}…", shot.number)));
        let briefing_intent = if shot.number == 1 {
            fs::read_to_string(self.ledger.briefing_dir(app_name).join("intent.md")).ok()
        } else {
            None
        };
        let prompt = note
            .map(str::to_owned)
            .or(briefing_intent)
            .unwrap_or_else(|| "sealed from the working tree.".into());
        self.ledger
            .write_shot_file(&shot, "prompt.md", prompt.as_bytes())?;
        if shot.number == 1 {
            // Conducted reference images belong to the genesis commitment.
            let briefing_images = self.ledger.briefing_dir(app_name).join("images");
            if let Ok(entries) = fs::read_dir(briefing_images) {
                for entry in entries.flatten() {
                    if entry
                        .file_type()
                        .map(|kind| kind.is_file())
                        .unwrap_or(false)
                    {
                        let _ = fs::copy(entry.path(), shot.images_path().join(entry.file_name()));
                    }
                }
            }
        }
        let genesis_input_sha256 = protocol_lifecycle::capture_input_commitment(&shot)?;
        self.genome
            .compose(&self.ledger, &shot, app_name, &app.bundle_id, &[], None)?;
        self.events.emit(Event::status(format!(
            "sealing the folder as shot {}…",
            shot.number
        )));
        self.ledger.snapshot_working_tree(&shot)?;
        // The engine-owned provenance placeholder never travels through a
        // snapshot; recreate it so the anatomy gate and prepare can run.
        let placeholder = shot.source_path().join("TOHSENO/embedded-provenance.json");
        if !placeholder.exists() {
            if let Some(parent) = placeholder.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&placeholder, b"{}")?;
        }
        if let Err(error) = build::validate_complete_source(&shot.source_path()) {
            return Err(EngineError::WorkingTreeIncomplete(error.to_string()));
        }
        self.events
            .emit(Event::status(format!("building shot {}…", shot.number)));
        if build::compile(&self.ledger, &shot, app_name)?.is_err() {
            return Err(EngineError::WorkingTreeUnbuildable {
                app: app_name.into(),
                shot: shot.number,
            });
        }
        let bundle_id = app.bundle_id.clone();
        install::require_candidate_namespace(&bundle_id).map_err(EngineError::Install)?;
        self.complete_shot(
            &shot,
            &app,
            &builder,
            genesis_input_sha256,
            None,
            app_name,
            &bundle_id,
            working_digest_at_start,
        )
        .await
    }

    /// Creates the app's visible folder and private briefing, then leaves
    /// the builder's own agent to work in it. `tohseno shot` finishes the
    /// birth.
    pub fn create_folder(&self, request: &ShotRequest) -> Result<ConductedCreation, EngineError> {
        crate::ledger::validate_app_name(&request.app_name)?;
        let _app_lock = self.ledger.lock_app(&request.app_name)?;
        self.check_slot_limit()?;
        self.emit_upsell_once(
            "welcome",
            "first shot: Xcode + Apple ID now · iPhone later · free Apple IDs refresh weekly.",
        )?;
        self.events
            .emit(Event::status("preparing your TOHSENO identity…"));
        let builder = BuilderIdentityManager::for_ledger(&self.ledger).ensure()?;
        let proposed_bundle_id = bundle_id(&request.app_name)?;
        let mut app = match self.ledger.load_app(&request.app_name) {
            Ok(existing) if existing.latest_shot.is_none() => existing,
            Ok(_) => return Err(LedgerError::AppExists(request.app_name.clone()).into()),
            Err(LedgerError::AppMissing(_)) => self
                .ledger
                .create_app(&request.app_name, &proposed_bundle_id)?,
            Err(error) => return Err(error.into()),
        };
        if app.shot_id.is_none() && app.builder_id.is_none() {
            app = self.ledger.bind_protocol_identity(
                &request.app_name,
                tohseno_protocol::digest::ShotId::random(),
                builder.builder_id,
            )?;
        }
        if app.builder_id != Some(builder.builder_id) {
            return Err(EngineError::BuilderMismatch(request.app_name.clone()));
        }
        let task_path = self.genome.compose_briefing(
            &self.ledger,
            &request.app_name,
            &app.bundle_id,
            &request.intent,
        )?;
        Ok(ConductedCreation {
            folder: self.ledger.working_tree(&request.app_name),
            task_path,
        })
    }

    /// The lenient digest of the working tree, or None when the folder is
    /// absent or unhashable.
    fn working_digest(&self, app_name: &str) -> Option<tohseno_protocol::digest::Bytes32> {
        let working = self.ledger.working_tree(app_name);
        if !working.is_dir() {
            return None;
        }
        tohseno_protocol::tree_hash::hash_working_tree(&working)
            .ok()
            .map(|commitment| commitment.digest)
    }

    /// Whether the folder holds anything a seal would include.
    fn working_tree_has_content(&self, app_name: &str) -> Result<bool, EngineError> {
        let working = self.ledger.working_tree(app_name);
        if !working.is_dir() {
            return Ok(false);
        }
        let commitment = tohseno_protocol::tree_hash::hash_working_tree(&working)
            .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
        Ok(!commitment.entries.is_empty())
    }

    fn working_tree_matches(&self, sealed: &Shot) -> Result<bool, EngineError> {
        let working = self.ledger.working_tree(&sealed.app_name);
        if !working.is_dir() {
            return Ok(true);
        }
        let working_hash = tohseno_protocol::tree_hash::hash_working_tree(&working)
            .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
        let sealed_hash = tohseno_protocol::tree_hash::hash_source_tree(&sealed.source_path())
            .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
        Ok(working_hash.digest == sealed_hash.digest)
    }

    pub async fn refresh(&self, app_name: Option<&str>) -> Result<(), EngineError> {
        self.wait_for_apple_prerequisites().await?;
        let apps = if let Some(app_name) = app_name {
            let app = self.ledger.load_app(app_name)?;
            if app.retired {
                self.check_slot_limit()?;
            }
            vec![app]
        } else {
            self.ledger
                .list_apps()?
                .into_iter()
                .filter(|app| !app.retired)
                .collect()
        };
        for app in apps.into_iter().filter(|app| app.latest_shot.is_some()) {
            let _app_lock = self.ledger.lock_app(&app.name)?;
            let app = self.ledger.load_app(&app.name)?;
            if app.latest_shot.is_none() {
                continue;
            }
            if app.retired {
                self.check_slot_limit()?;
            }
            let shot = self.ledger.latest_shot(&app.name)?.unwrap();
            protocol_lifecycle::verify_completed_evolution(&shot)?;
            let recorded_artifact = shot.artifact_path().join(format!("{}.app", app.name));
            if sign::days_until_expiry(&recorded_artifact).is_some_and(|days| days <= 0) {
                self.emit_upsell_once(
                    "expiry",
                    "A paid Apple Developer membership removes weekly expiry: developer.apple.com.",
                )?;
            }
            let artifact_directory = temporary_path("refresh");
            self.events.emit(Event::status(format!(
                "refreshing shot {} of {}…",
                shot.number, app.name
            )));
            DevicePipeline::new(self.events.clone())
                .build_install(
                    shot.number,
                    &app.name,
                    &app.bundle_id,
                    &shot.source_path(),
                    &artifact_directory,
                )
                .await?;
            self.ledger.set_retired(&app.name, false)?;
            self.events.emit(Event::result(format!(
                "shot {} of {} is refreshed on your phone.",
                shot.number, app.name
            )));
        }
        Ok(())
    }

    pub async fn retire(&self, app_name: &str) -> Result<(), EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        install::require_candidate_namespace(&app.bundle_id).map_err(EngineError::Install)?;
        self.wait_for_apple_prerequisites().await?;
        let device = DevicePipeline::new(self.events.clone())
            .wait_for_device()
            .await?;
        self.events
            .emit(Event::status(format!("retiring {app_name}…")));
        install::retire(&device, &app.bundle_id).map_err(EngineError::Install)?;
        self.ledger.set_retired(app_name, true)?;
        self.events.emit(Event::result(format!(
            "{app_name} is off your phone and remains in your ledger."
        )));
        Ok(())
    }

    pub fn doctor_once(&self) -> Result<bool, EngineError> {
        match toolchain::check() {
            ToolchainState::Ready => {
                self.events.emit(Event::status("Xcode is ready."));
                Ok(true)
            }
            ToolchainState::Missing => {
                let _ = toolchain::trigger_install();
                self.events.emit(Event::handoff(
                    "Install Xcode from the App Store, then open it once.",
                ));
                Ok(false)
            }
        }
    }

    /// Starts Apple's installer before the user begins describing the app so
    /// toolchain download time overlaps with the intent gate.
    pub fn prime_toolchain(&self) {
        if toolchain::check() == ToolchainState::Missing {
            let _ = toolchain::trigger_install();
            self.events
                .emit(Event::status("starting the Apple toolchain installation…"));
        }
    }

    fn check_slot_limit(&self) -> Result<(), EngineError> {
        let active = self
            .ledger
            .list_apps()?
            .into_iter()
            .filter(|app| !app.retired && app.latest_shot.is_some())
            .collect::<Vec<_>>();
        if active.len() >= 3 {
            let candidate = &active[0].name;
            self.emit_upsell_once(
                "slots",
                "A paid Apple Developer membership raises this limit: developer.apple.com.",
            )?;
            self.events.emit(Event::handoff(format!(
                "Run `tohseno retire {candidate}` to free one iPhone slot."
            )));
            return Err(EngineError::SlotLimit);
        }
        Ok(())
    }

    fn emit_upsell_once(&self, wall: &str, message: &str) -> Result<(), EngineError> {
        let directory = self.ledger.machine_root().join("walls");
        fs::create_dir_all(&directory)?;
        let marker = directory.join(wall);
        if !marker.exists() {
            fs::write(marker, b"shown\n")?;
            self.events.emit(Event::status(message));
        }
        Ok(())
    }

    fn harness_for_request(&self, requested: Option<&str>) -> Result<HarnessConfig, EngineError> {
        let Some(requested) = requested else {
            return Ok(self.config.harness.clone());
        };
        if Path::new(requested).is_absolute() {
            // A bring-your-own agent is a one-shot choice, never persisted
            // over the builder's configured default.
            return Ok(HarnessConfig {
                command: crate::harness::command_for_path(Path::new(requested)),
            });
        }
        let harness = selected_harness(requested)?;
        if harness.command != self.config.harness.command {
            let mut config = self.config.clone();
            config.harness = harness.clone();
            config.save(self.ledger.machine_root())?;
        }
        Ok(harness)
    }

    async fn wait_for_prerequisites(
        &self,
        harness_config: &HarnessConfig,
    ) -> Result<HarnessMode, EngineError> {
        self.wait_for_apple_prerequisites().await?;
        Harness::new(harness_config.clone(), self.events.clone())
            .wait_until_available()
            .await
            .map_err(EngineError::Harness)
    }

    async fn wait_for_apple_prerequisites(&self) -> Result<(), EngineError> {
        let mut toolchain_announced = false;
        loop {
            match toolchain::check() {
                ToolchainState::Ready => break,
                ToolchainState::Missing => {
                    if !toolchain_announced {
                        let _ = toolchain::trigger_install();
                        self.events.emit(Event::handoff(
                            "Install Xcode from the App Store, then open it once.",
                        ));
                        toolchain_announced = true;
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }

        let mut apple_signing_announced = false;
        loop {
            match apple_signing::check() {
                AppleSigningState::Ready { .. } => return Ok(()),
                AppleSigningState::Missing => {
                    if !apple_signing_announced {
                        self.events.emit(Event::handoff(
                            "Open Xcode → Settings → Accounts and sign in with your Apple ID.",
                        ));
                        apple_signing_announced = true;
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
}

/// Gates 6–8, reusable by create/evolve and refresh.
pub struct DevicePipeline {
    events: EventBus,
    poll_interval: Duration,
}

impl DevicePipeline {
    pub fn new(events: EventBus) -> Self {
        Self {
            events,
            poll_interval: Duration::from_secs(2),
        }
    }

    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    pub async fn run(
        &self,
        shot: &Shot,
        app_name: &str,
        bundle_id: &str,
        source: &Path,
    ) -> Result<(), EngineError> {
        self.build_install(
            shot.number,
            app_name,
            bundle_id,
            source,
            &shot.artifact_path(),
        )
        .await?;
        self.events.emit(Event::result(format!(
            "shot {} of {} is on your phone.",
            shot.number, app_name
        )));
        Ok(())
    }

    pub async fn build_install(
        &self,
        shot_number: u32,
        app_name: &str,
        bundle_id: &str,
        source: &Path,
        artifact_directory: &Path,
    ) -> Result<(), EngineError> {
        install::require_candidate_namespace(bundle_id).map_err(EngineError::Install)?;
        let device = self.wait_for_device().await?;
        self.events
            .emit(Event::status(format!("signing shot {shot_number}…")));
        let app = sign::build_signed(sign::SignRequest {
            source,
            artifact_directory,
            app_name,
            bundle_id,
            shot_number,
            device: &device,
        })
        .map_err(EngineError::Sign)?;
        self.events
            .emit(Event::status(format!("installing shot {shot_number}…")));
        install::install(&device, &app, bundle_id).map_err(EngineError::Install)?;
        install::launch(&device, bundle_id).map_err(EngineError::Install)?;
        Ok(())
    }

    pub async fn wait_for_device(&self) -> Result<device::Device, EngineError> {
        let mut last_handoff: Option<&'static str> = None;
        loop {
            let state = device::check().map_err(EngineError::Device)?;
            let (handoff, ready) = match state {
                DeviceState::Ready(device) => (None, Some(device)),
                DeviceState::CableMissing => {
                    (Some("Plug in your iPhone with a cable."), None)
                }
                DeviceState::TrustRequired => (Some("Tap Trust on your iPhone."), None),
                DeviceState::DeveloperModeRequired => (
                    Some("Enable Developer Mode: Settings → Privacy & Security → Developer Mode, then let your phone restart."),
                    None,
                ),
            };
            if let Some(device) = ready {
                self.events
                    .emit(Event::status(format!("found {} over USB.", device.name)));
                return Ok(device);
            }
            if handoff != last_handoff {
                self.events.emit(Event::handoff(handoff.unwrap()));
                last_handoff = handoff;
            }
            tokio::time::sleep(self.poll_interval).await;
        }
    }
}

fn bundle_id(app_name: &str) -> Result<String, EngineError> {
    let output = Command::new("whoami").output().map_err(EngineError::Io)?;
    if !output.status.success() {
        return Err(EngineError::IdentityName);
    }
    Ok(candidate_bundle_id(
        String::from_utf8_lossy(&output.stdout).trim(),
        app_name,
    ))
}

fn candidate_bundle_id(local_username: &str, app_name: &str) -> String {
    let username = sanitize_component(local_username);
    let username = if username.is_empty() {
        "user".to_owned()
    } else {
        username
    };
    format!("{}{username}.{app_name}", install::CANDIDATE_BUNDLE_PREFIX)
}

fn temporary_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("tohseno-{label}-{}-{nonce}", std::process::id()));
    let _ = fs::create_dir_all(&path);
    path
}

#[derive(Debug)]
pub enum EngineError {
    Io(std::io::Error),
    Config(ConfigError),
    Ledger(LedgerError),
    Intent(IntentError),
    Genome(GenomeError),
    Harness(HarnessError),
    Build(build::BuildError),
    Device(device::DeviceError),
    Sign(sign::SignError),
    Install(install::InstallError),
    NoCompleteShot(String),
    RepairExhausted { shot: u32, passes: u8 },
    ArtifactUnbuildable(String),
    UnsealedChanges(String),
    FolderInProgress(String),
    NothingToSeal(String),
    WorkingTreeIncomplete(String),
    WorkingTreeUnbuildable { app: String, shot: u32 },
    SlotLimit,
    IdentityName,
    BuilderIdentity(BuilderIdentityError),
    ProtocolLifecycle(ProtocolLifecycleError),
    LegacyRequiresAdoption(String),
    BuilderMismatch(String),
    AlreadyProtocol(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Config(error) => write!(f, "{error}"),
            Self::Ledger(error) => write!(f, "{error}"),
            Self::Intent(error) => write!(f, "{error}"),
            Self::Genome(error) => write!(f, "{error}"),
            Self::Harness(error) => write!(f, "{error}"),
            Self::Build(error) => write!(f, "{error}"),
            Self::Device(error) => write!(f, "{error}"),
            Self::Sign(error) => write!(f, "{error}"),
            Self::Install(error) => write!(f, "{error}"),
            Self::NoCompleteShot(app) => write!(f, "{app} has no complete shot to evolve"),
            Self::RepairExhausted { shot, passes } => write!(
                f,
                "engine bug: shot {shot} still fails after {passes} repair passes"
            ),
            Self::UnsealedChanges(app) => {
                write!(
                    f,
                    "{app} has unsealed changes — run `tohseno shot {app}` first"
                )
            }
            Self::FolderInProgress(app) => {
                write!(
                    f,
                    "the {app} folder already holds work — seal it with `tohseno shot {app}`"
                )
            }
            Self::NothingToSeal(app) => {
                write!(
                    f,
                    "no Xcode project in the {app} folder yet — build one, then `tohseno shot {app}`"
                )
            }
            Self::WorkingTreeIncomplete(detail) => {
                write!(f, "the folder is missing required anatomy: {detail}")
            }
            Self::WorkingTreeUnbuildable { app, shot } => {
                write!(
                    f,
                    "the folder does not build; see .tohseno/evolutions/{shot:04}/build.log, fix, then `tohseno shot {app}`"
                )
            }
            Self::ArtifactUnbuildable(output) => {
                let tail: String = output
                    .lines()
                    .filter(|line| line.contains("error"))
                    .take(8)
                    .collect::<Vec<_>>()
                    .join("\n");
                write!(
                    f,
                    "the iOS device build passed but the Simulator artifact failed:\n{tail}"
                )
            }
            Self::SlotLimit => write!(f, "the free Apple ID app limit is full"),
            Self::IdentityName => write!(f, "could not determine the local username"),
            Self::BuilderIdentity(error) => write!(f, "{error}"),
            Self::ProtocolLifecycle(error) => write!(f, "{error}"),
            Self::LegacyRequiresAdoption(app) => {
                write!(f, "{app} predates signed Shots; run `tohseno adopt {app}`")
            }
            Self::BuilderMismatch(app) => write!(
                f,
                "{app} belongs to a different BuilderID than the active local identity"
            ),
            Self::AlreadyProtocol(app) => {
                write!(f, "{app} already has a signed TOHSENO identity")
            }
        }
    }
}

impl std::error::Error for EngineError {}

impl From<std::io::Error> for EngineError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ConfigError> for EngineError {
    fn from(value: ConfigError) -> Self {
        Self::Config(value)
    }
}

impl From<LedgerError> for EngineError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

impl From<IntentError> for EngineError {
    fn from(value: IntentError) -> Self {
        Self::Intent(value)
    }
}

impl From<GenomeError> for EngineError {
    fn from(value: GenomeError) -> Self {
        Self::Genome(value)
    }
}

impl From<HarnessError> for EngineError {
    fn from(value: HarnessError) -> Self {
        Self::Harness(value)
    }
}

impl From<build::BuildError> for EngineError {
    fn from(value: build::BuildError) -> Self {
        Self::Build(value)
    }
}

impl From<BuilderIdentityError> for EngineError {
    fn from(value: BuilderIdentityError) -> Self {
        Self::BuilderIdentity(value)
    }
}

impl From<ProtocolLifecycleError> for EngineError {
    fn from(value: ProtocolLifecycleError) -> Self {
        Self::ProtocolLifecycle(value)
    }
}

impl From<EngineError> for std::io::Error {
    fn from(value: EngineError) -> Self {
        std::io::Error::other(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_bundle_identity_uses_the_candidate_device_namespace() {
        let bundle_id = candidate_bundle_id("Alice Example", "quiet-press");
        assert_eq!(bundle_id, "org.tohseno.genesis.alice-example.quiet-press");
        install::require_candidate_namespace(&bundle_id).unwrap();
    }

    #[test]
    fn candidate_namespace_preserves_a_deterministic_fallback_identity() {
        assert_eq!(
            candidate_bundle_id("---", "press"),
            "org.tohseno.genesis.user.press"
        );
    }
}
