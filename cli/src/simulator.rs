use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tempfile::TempDir;
use tohseno_engine::{Event, EventBus};
use tokio::process::Command;

/// Build one verified workshop source tree locally, then install and launch
/// that local build. No publisher binary or ownership state is imported.
pub async fn launch_workshop(
    source: &Path,
    events: &EventBus,
    app_name: &str,
    bundle_id: &str,
    version_ordinal: u64,
) -> Result<(), SimulatorError> {
    let shot_number = u32::try_from(version_ordinal)
        .map_err(|_| SimulatorError::InvalidVersion(version_ordinal))?;
    let device = choose_device().await?;
    boot(&device).await?;
    let _ = std::process::Command::new("open")
        .args(["-g", "-a", "Simulator", "--args", "-CurrentDeviceUDID"])
        .arg(&device.udid)
        .spawn();
    events.emit(Event::status(format!(
        "building verified workshop version {version_ordinal} of {app_name} locally…"
    )));
    let build_directory = tempfile::tempdir()?;
    let app_bundle = build(
        source,
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
            bundle_id.into(),
        ],
    )
    .await?;
    events.emit(Event::result(format!(
        "workshop version {version_ordinal} of {app_name} is running in Simulator."
    )));
    Ok(())
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
    Command(String),
    Build(String),
    DeviceMissing,
    ProjectMissing,
    ArtifactMissing,
    InvalidVersion(u64),
}

impl std::fmt::Display for SimulatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::Command(error) => write!(f, "{error}"),
            Self::Build(error) => write!(f, "Simulator build failed: {error}"),
            Self::DeviceMissing => write!(f, "no available iPhone Simulator was found"),
            Self::ProjectMissing => write!(f, "the shot has no Xcode project"),
            Self::ArtifactMissing => write!(f, "the Simulator app was not produced"),
            Self::InvalidVersion(number) => {
                write!(
                    f,
                    "workshop version {number} does not fit Apple build numbering"
                )
            }
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
