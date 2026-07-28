use crate::config::{Config, ConfigError};
use crate::events::{Event, EventBus};
use crate::gates::device::{self, DeviceState};
use crate::gates::identity::{self, IdentityState};
use crate::gates::intent::{Intent, IntentError};
use crate::gates::toolchain::{self, ToolchainState};
use crate::gates::{build, install, sign};
use crate::genome::{Genome, GenomeError};
use crate::harness::{Harness, HarnessError, HarnessMode};
use crate::ledger::{sanitize_component, Ledger, LedgerError, Shot};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct ShotRequest {
    pub app_name: String,
    pub intent: Intent,
}

pub struct Engine {
    ledger: Ledger,
    events: EventBus,
    config: Config,
    genome: Genome,
}

impl Engine {
    pub fn discover(events: EventBus) -> Result<Self, EngineError> {
        let ledger = Ledger::discover()?;
        ledger.initialize()?;
        let config = Config::load_or_create(ledger.root())?;
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

    pub async fn create(&self, request: ShotRequest) -> Result<Shot, EngineError> {
        crate::ledger::validate_app_name(&request.app_name)?;
        self.check_slot_limit()?;
        let bundle_id = bundle_id(&request.app_name)?;
        self.ledger.create_app(&request.app_name, &bundle_id)?;
        let shot = self.ledger.reserve_shot(&request.app_name, None)?;
        self.run_shot(request, shot, bundle_id, None).await
    }

    pub async fn evolve(&self, request: ShotRequest) -> Result<Shot, EngineError> {
        crate::ledger::validate_app_name(&request.app_name)?;
        let app = self.ledger.load_app(&request.app_name)?;
        let previous = self
            .ledger
            .latest_shot(&request.app_name)?
            .ok_or_else(|| EngineError::NoCompleteShot(request.app_name.clone()))?;
        let shot = self
            .ledger
            .reserve_shot(&request.app_name, Some(previous.number))?;
        self.run_shot(request, shot, app.bundle_id, Some(previous.source_path()))
            .await
    }

    async fn run_shot(
        &self,
        request: ShotRequest,
        shot: Shot,
        bundle_id: String,
        previous_source: Option<PathBuf>,
    ) -> Result<Shot, EngineError> {
        self.events
            .emit(Event::status(format!("preparing shot {}…", shot.number)));
        let image_names = request
            .intent
            .write_to_shot(&self.ledger, &shot, &self.events)?;
        self.genome.compose(
            &self.ledger,
            &shot,
            &request.app_name,
            &bundle_id,
            &image_names,
            previous_source.as_deref(),
        )?;

        let harness_mode = self.wait_for_prerequisites().await?;
        let harness = Harness::new(self.config.harness.clone(), self.events.clone());
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
        build::validate_complete_source(&shot.source_path())?;

        let mut repair_pass = 0;
        loop {
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
                    build::validate_complete_source(&shot.source_path())?;
                }
                Err(_) => {
                    return Err(EngineError::RepairExhausted {
                        shot: shot.number,
                        passes: self.config.max_repair_passes,
                    });
                }
            }
        }

        let pipeline = DevicePipeline::new(self.events.clone());
        pipeline
            .build_install(
                shot.number,
                &request.app_name,
                &bundle_id,
                &shot.source_path(),
                &shot.artifact_path(),
            )
            .await?;
        self.ledger.finalize_shot(&shot)?;
        self.ledger.set_retired(&request.app_name, false)?;
        self.events.emit(Event::result(format!(
            "shot {} of {} is on your phone.",
            shot.number, request.app_name
        )));
        Ok(shot)
    }

    pub async fn refresh(&self, app_name: Option<&str>) -> Result<(), EngineError> {
        self.wait_for_apple_prerequisites().await?;
        let apps = if let Some(app_name) = app_name {
            vec![self.ledger.load_app(app_name)?]
        } else {
            self.ledger.list_apps()?
        };
        for app in apps.into_iter().filter(|app| app.latest_shot.is_some()) {
            let shot = self.ledger.latest_shot(&app.name)?.unwrap();
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
        self.wait_for_apple_prerequisites().await?;
        let app = self.ledger.load_app(app_name)?;
        let device = DevicePipeline::new(self.events.clone())
            .wait_for_device()
            .await?;
        self.events
            .emit(Event::status(format!("retiring {app_name}…")));
        install::retire(&device, &app.bundle_id).map_err(EngineError::Command)?;
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

    fn check_slot_limit(&self) -> Result<(), EngineError> {
        let active = self
            .ledger
            .list_apps()?
            .into_iter()
            .filter(|app| !app.retired && app.latest_shot.is_some())
            .collect::<Vec<_>>();
        if active.len() >= 3 {
            let candidate = &active[0].name;
            self.events.emit(Event::handoff(format!(
                "Run `tohseno retire {candidate}` to free one iPhone slot."
            )));
            return Err(EngineError::SlotLimit);
        }
        Ok(())
    }

    fn emit_upsell_once(&self, wall: &str, message: &str) -> Result<(), EngineError> {
        let directory = self.ledger.root().join("walls");
        fs::create_dir_all(&directory)?;
        let marker = directory.join(wall);
        if !marker.exists() {
            fs::write(marker, b"shown\n")?;
            self.events.emit(Event::status(message));
        }
        Ok(())
    }

    async fn wait_for_prerequisites(&self) -> Result<HarnessMode, EngineError> {
        self.wait_for_apple_prerequisites().await?;
        Harness::new(self.config.harness.clone(), self.events.clone())
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

        let mut identity_announced = false;
        loop {
            match identity::check() {
                IdentityState::Ready { .. } => return Ok(()),
                IdentityState::Missing => {
                    if !identity_announced {
                        self.events.emit(Event::handoff(
                            "Open Xcode → Settings → Accounts and sign in with your Apple ID.",
                        ));
                        identity_announced = true;
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
        install::install(&device, &app).map_err(EngineError::Command)?;
        install::launch(&device, bundle_id).map_err(EngineError::Command)?;
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
    let username = sanitize_component(String::from_utf8_lossy(&output.stdout).trim());
    let username = if username.is_empty() {
        "user".to_owned()
    } else {
        username
    };
    Ok(format!("com.tohseno.{username}.{app_name}"))
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
    Command(crate::gates::CommandError),
    NoCompleteShot(String),
    RepairExhausted { shot: u32, passes: u8 },
    SlotLimit,
    IdentityName,
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
            Self::Command(error) => write!(f, "{error}"),
            Self::NoCompleteShot(app) => write!(f, "{app} has no complete shot to evolve"),
            Self::RepairExhausted { shot, passes } => write!(
                f,
                "engine bug: shot {shot} still fails after {passes} repair passes"
            ),
            Self::SlotLimit => write!(f, "the free Apple ID app limit is full"),
            Self::IdentityName => write!(f, "could not determine the local username"),
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

impl From<EngineError> for std::io::Error {
    fn from(value: EngineError) -> Self {
        std::io::Error::other(value)
    }
}
