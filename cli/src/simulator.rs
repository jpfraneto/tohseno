use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tempfile::TempDir;
use tohseno_engine::{Event, EventBus, Ledger};
use tokio::process::Command;

#[derive(Clone, Debug)]
pub struct SimulatorSession {
    pub device_id: String,
}

pub async fn launch(
    ledger: &Ledger,
    events: &EventBus,
    app_name: &str,
    shot_number: u32,
) -> Result<SimulatorSession, SimulatorError> {
    let app = ledger.load_app(app_name)?;
    let shot = ledger.shot(app_name, shot_number)?;
    if !ledger
        .list_shots(app_name)?
        .iter()
        .any(|candidate| candidate.number == shot_number)
    {
        return Err(SimulatorError::IncompleteShot(shot_number));
    }

    let device = choose_device().await?;
    boot(&device).await?;
    let _ = std::process::Command::new("open")
        .args(["-g", "-a", "Simulator", "--args", "-CurrentDeviceUDID"])
        .arg(&device.udid)
        .spawn();

    events.emit(Event::status(format!(
        "opening evolution {shot_number} of {app_name} in Simulator…"
    )));
    let build_directory = tempfile::tempdir()?;
    let app_bundle = build(
        &shot.source_path(),
        app_name,
        shot_number,
        &device.udid,
        &build_directory,
    )
    .await?;
    checked(
        "xcrun",
        [
            "simctl".into(),
            "install".into(),
            device.udid.clone().into(),
            app_bundle.into_os_string(),
        ],
    )
    .await?;
    checked(
        "xcrun",
        [
            "simctl".into(),
            "launch".into(),
            "--terminate-running-process".into(),
            device.udid.clone().into(),
            app.bundle_id.into(),
        ],
    )
    .await?;
    events.emit(Event::status(format!(
        "evolution {shot_number} of {app_name} is running in Simulator."
    )));
    Ok(SimulatorSession {
        device_id: device.udid,
    })
}

pub async fn screenshot(session: &SimulatorSession) -> Result<Vec<u8>, SimulatorError> {
    let output = Command::new("xcrun")
        .args([
            "simctl",
            "io",
            &session.device_id,
            "screenshot",
            "--type=png",
            "--mask=ignored",
            "-",
        ])
        .output()
        .await?;
    if !output.status.success() {
        return Err(SimulatorError::Command(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    Ok(output.stdout)
}

async fn build(
    source: &Path,
    app_name: &str,
    shot_number: u32,
    device_id: &str,
    build_directory: &TempDir,
) -> Result<PathBuf, SimulatorError> {
    let project = std::fs::read_dir(source)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "xcodeproj")
        })
        .ok_or(SimulatorError::ProjectMissing)?;
    let output = Command::new("xcodebuild")
        .current_dir(source)
        .arg("-project")
        .arg(project)
        .args([
            "-scheme",
            app_name,
            "-configuration",
            "Debug",
            "-sdk",
            "iphonesimulator",
            "-destination",
            &format!("id={device_id}"),
            "-derivedDataPath",
        ])
        .arg(build_directory.path())
        .args([
            "CODE_SIGNING_ALLOWED=NO",
            &format!("CURRENT_PROJECT_VERSION={shot_number}"),
            "build",
        ])
        .output()
        .await?;
    if !output.status.success() {
        let log = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let tail = log
            .chars()
            .rev()
            .take(12_000)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        return Err(SimulatorError::Build(tail));
    }
    let bundle = build_directory
        .path()
        .join("Build")
        .join("Products")
        .join("Debug-iphonesimulator")
        .join(format!("{app_name}.app"));
    bundle
        .is_dir()
        .then_some(bundle)
        .ok_or(SimulatorError::ArtifactMissing)
}

async fn choose_device() -> Result<SimulatorDevice, SimulatorError> {
    let output = checked(
        "xcrun",
        [
            "simctl".into(),
            "list".into(),
            "devices".into(),
            "available".into(),
            "-j".into(),
        ],
    )
    .await?;
    let listing: DeviceListing = serde_json::from_slice(&output.stdout)?;
    let mut i_phones = listing
        .devices
        .into_iter()
        .rev()
        .flat_map(|(_, devices)| devices)
        .filter(|device| device.device_type_identifier.contains("iPhone"))
        .collect::<Vec<_>>();
    i_phones
        .iter()
        .find(|device| device.state == "Booted")
        .cloned()
        .or_else(|| i_phones.drain(..).next())
        .ok_or(SimulatorError::DeviceMissing)
}

async fn boot(device: &SimulatorDevice) -> Result<(), SimulatorError> {
    if device.state != "Booted" {
        checked(
            "xcrun",
            ["simctl".into(), "boot".into(), device.udid.clone().into()],
        )
        .await?;
    }
    checked(
        "xcrun",
        [
            "simctl".into(),
            "bootstatus".into(),
            device.udid.clone().into(),
            "-b".into(),
        ],
    )
    .await?;
    Ok(())
}

async fn checked<const N: usize>(
    program: &str,
    arguments: [std::ffi::OsString; N],
) -> Result<std::process::Output, SimulatorError> {
    let output = Command::new(program)
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(SimulatorError::Command(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimulatorDevice {
    udid: String,
    state: String,
    device_type_identifier: String,
}

#[derive(Debug, Deserialize)]
struct DeviceListing {
    devices: BTreeMap<String, Vec<SimulatorDevice>>,
}

#[derive(Debug)]
pub enum SimulatorError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Ledger(tohseno_engine::LedgerError),
    Command(String),
    Build(String),
    DeviceMissing,
    ProjectMissing,
    ArtifactMissing,
    IncompleteShot(u32),
}

impl std::fmt::Display for SimulatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::Ledger(error) => write!(f, "{error}"),
            Self::Command(error) => write!(f, "{error}"),
            Self::Build(error) => write!(f, "Simulator build failed: {error}"),
            Self::DeviceMissing => write!(f, "no available iPhone Simulator was found"),
            Self::ProjectMissing => write!(f, "the shot has no Xcode project"),
            Self::ArtifactMissing => write!(f, "the Simulator app was not produced"),
            Self::IncompleteShot(number) => write!(f, "shot {number} is incomplete"),
        }
    }
}

impl std::error::Error for SimulatorError {}

impl From<std::io::Error> for SimulatorError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for SimulatorError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<tohseno_engine::LedgerError> for SimulatorError {
    fn from(value: tohseno_engine::LedgerError) -> Self {
        Self::Ledger(value)
    }
}
