//! Companion-independent readiness for installing generated iPhone apps.
//!
//! The old cable genesis record remains readable and paired devices remain
//! valid. This projection is deliberately separate: completing it never
//! installs, launches, pairs, or mutates the Companion.

use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use tohseno_engine::gates::{apple_signing, device, install, toolchain};
use uuid::Uuid;

const SCHEMA: &str = "tohseno.private-iphone-readiness/1";
const MAX_RECORD_BYTES: u64 = 64 * 1024;
const READINESS_BUNDLE_ID: &str = "org.tohseno.genesis.readiness.check";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationState {
    Pending,
    Building,
    Installing,
    Verified,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadinessRecord {
    schema: String,
    revision: u64,
    begun: bool,
    verification: VerificationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
}

impl Default for ReadinessRecord {
    fn default() -> Self {
        Self {
            schema: SCHEMA.into(),
            revision: 1,
            begun: false,
            verification: VerificationState::Pending,
            last_error: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReadinessStore {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl ReadinessStore {
    pub fn open(root: &Path) -> Result<Self, BoxError> {
        let store = Self {
            path: root.join("iphone-readiness-v1.json"),
            lock: Arc::new(Mutex::new(())),
        };
        if !store.path.exists() {
            store.write_unlocked(&ReadinessRecord::default())?;
        }
        store.load()?;
        Ok(store)
    }

    pub fn load(&self) -> Result<ReadinessRecord, BoxError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "readiness lock was poisoned")?;
        let metadata = fs::symlink_metadata(&self.path)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > MAX_RECORD_BYTES
        {
            return Err("iPhone readiness record is unsafe".into());
        }
        let mut file = OpenOptions::new().read(true).open(&self.path)?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(MAX_RECORD_BYTES + 1)
            .read_to_end(&mut bytes)?;
        let record: ReadinessRecord = serde_json::from_slice(&bytes)?;
        validate(&record)?;
        Ok(record)
    }

    pub fn begin(&self) -> Result<(), BoxError> {
        self.update(|record| record.begun = true).map(drop)
    }

    pub fn verification(
        &self,
        state: VerificationState,
        error: Option<&str>,
    ) -> Result<(), BoxError> {
        self.update(|record| {
            record.verification = state;
            record.last_error = error.map(|value| value.chars().take(500).collect());
        })
        .map(drop)
    }

    pub fn recover_interrupted(&self) -> Result<bool, BoxError> {
        let record = self.load()?;
        if !matches!(
            record.verification,
            VerificationState::Building | VerificationState::Installing
        ) {
            return Ok(false);
        }
        self.verification(
            VerificationState::Failed,
            Some("The previous readiness check was interrupted. No app source or history was changed."),
        )?;
        Ok(true)
    }

    fn update(
        &self,
        change: impl FnOnce(&mut ReadinessRecord),
    ) -> Result<ReadinessRecord, BoxError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "readiness lock was poisoned")?;
        let mut record = if self.path.exists() {
            let bytes = fs::read(&self.path)?;
            if bytes.len() as u64 > MAX_RECORD_BYTES {
                return Err("readiness record is oversized".into());
            }
            serde_json::from_slice(&bytes)?
        } else {
            ReadinessRecord::default()
        };
        change(&mut record);
        record.revision = record.revision.saturating_add(1);
        self.write_unlocked(&record)?;
        Ok(record)
    }

    fn write_unlocked(&self, record: &ReadinessRecord) -> Result<(), BoxError> {
        validate(record)?;
        let parent = self.path.parent().ok_or("readiness path has no parent")?;
        fs::create_dir_all(parent)?;
        let temporary = parent.join(format!(".iphone-readiness-{}.tmp", Uuid::new_v4().simple()));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&tohseno_protocol::canonical::to_vec(record)?)?;
        file.sync_all()?;
        fs::rename(&temporary, &self.path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ReadinessObservation {
    pub macos_supported: bool,
    pub xcode_ready: bool,
    pub components_ready: bool,
    pub device: Option<device::DeviceState>,
    pub signing_team: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadinessView {
    pub schema: &'static str,
    pub ready: bool,
    pub step: &'static str,
    pub headline: &'static str,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_action: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_label: Option<&'static str>,
}

pub fn observe() -> ReadinessObservation {
    let xcode_ready = toolchain::check() == toolchain::ToolchainState::Ready;
    let device = xcode_ready.then(device::check).and_then(Result::ok);
    let signing_team = match apple_signing::check() {
        apple_signing::AppleSigningState::Ready { team_id, .. } => Some(team_id),
        apple_signing::AppleSigningState::Missing => None,
    };
    ReadinessObservation {
        macos_supported: supported_macos(),
        xcode_ready,
        components_ready: xcode_ready && xcode_components_ready(),
        device,
        signing_team,
    }
}

pub fn project(record: &ReadinessRecord, observed: &ReadinessObservation) -> ReadinessView {
    let view = |ready, step, headline, detail: &str, action, label| ReadinessView {
        schema: "tohseno.iphone-readiness-view/1",
        ready,
        step,
        headline,
        detail: detail.into(),
        primary_action: action,
        primary_label: label,
    };
    if !record.begun {
        return view(false, "welcome", "One intention. One app. Yours.", "Tohseno turns one coherent intention into a native iPhone app you own, use, and evolve.", Some("begin"), Some("Set Up This Mac"));
    }
    if !observed.macos_supported {
        return view(false, "unsupported_macos", "This macOS version is not supported", "Tohseno for Mac requires macOS 14 or later. Your existing source and history are unchanged.", None, None);
    }
    if !observed.xcode_ready {
        return view(false, "install_xcode", "Install the full Xcode app", "Xcode is a large Apple download. Install it from the Mac App Store, then open it once so Apple can finish setup.", Some("open_app_store"), Some("Open Xcode in the App Store"));
    }
    if !observed.components_ready {
        return view(false, "finish_xcode", "Let Xcode finish its setup", "Open Xcode, accept Apple's license if asked, and allow required components to install. Tohseno advances only after Xcode reports that setup is complete.", Some("open_xcode"), Some("Open Xcode"));
    }
    match observed.device.as_ref() {
        None | Some(device::DeviceState::CableMissing) => return view(false, "connect_iphone", "Connect and unlock your iPhone", "Use a cable. Keep the phone unlocked while Tohseno checks Apple's device state.", Some("check"), Some("Check Again")),
        Some(device::DeviceState::TrustRequired) => return view(false, "trust_mac", "Trust this Mac on your iPhone", "Unlock the iPhone, tap Trust, and enter its passcode. Tohseno cannot claim this step for you.", Some("check"), Some("Check Again")),
        Some(device::DeviceState::DeveloperModeRequired) => return view(false, "developer_mode", "Turn on Developer Mode", "On iPhone open Settings → Privacy & Security → Developer Mode. Turn it on and let the phone restart, then reconnect and unlock it.", Some("check"), Some("Check Again")),
        Some(device::DeviceState::Ready(_)) => {}
    }
    if observed.signing_team.is_none() {
        return view(false, "apple_account", "Add your Apple Account in Xcode", "Tohseno never asks for your Apple credentials. In Xcode choose Settings → Accounts, add your account, and make sure a Personal Team or development team appears.", Some("open_xcode"), Some("Open Xcode"));
    }
    match record.verification {
        VerificationState::Pending | VerificationState::Failed => view(
            false,
            "verify_installation",
            "Verify this iPhone with a tiny readiness app",
            record.last_error.as_deref().unwrap_or("Tohseno will build, sign, install, open, and remove a deterministic test app. It does not touch your app source."),
            Some("verify_installation"),
            Some(if record.verification == VerificationState::Failed { "Try Again" } else { "Verify iPhone" }),
        ),
        VerificationState::Building => view(false, "building_readiness", "Building the readiness app…", "Xcode is compiling and signing the deterministic test app. Keep the iPhone connected and unlocked.", None, None),
        VerificationState::Installing => view(false, "installing_readiness", "Checking installation on your iPhone…", "The signed test app is being installed and opened. It will be removed after the check.", None, None),
        VerificationState::Verified => view(true, "ready", "This Mac can build for your iPhone", "Adopt an existing iOS project, connect the Companion, and request the next change from your phone. Creating a first app remains available as a secondary path.", None, None),
    }
}

pub fn verify_installation(
    project_root: &Path,
    service_root: &Path,
    target: &device::Device,
    team_id: &str,
    installing: impl FnOnce() -> Result<(), BoxError>,
) -> Result<(), BoxError> {
    let project = project_root.join("HelloWorld.xcodeproj");
    let source = project_root.join("HelloWorldApp.swift");
    for path in [&project, &source] {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !(metadata.is_dir() || metadata.is_file()) {
            return Err("the bundled readiness project is unsafe".into());
        }
    }
    if team_id.is_empty()
        || team_id.len() > 32
        || !team_id.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err("the selected Apple development team is invalid".into());
    }
    let derived = service_root.join("readiness-derived-data");
    match fs::symlink_metadata(&derived) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("the private readiness build directory is unsafe".into())
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&derived)?,
        Err(error) => return Err(error.into()),
    }
    let destination = target.udid.as_deref().unwrap_or(&target.identifier);
    let status = Command::new("xcodebuild")
        .args(["-project"])
        .arg(&project)
        .args([
            "-scheme",
            "HelloWorld",
            "-configuration",
            "Release",
            "-destination",
            &format!("id={destination}"),
            "-derivedDataPath",
        ])
        .arg(&derived)
        .args([
            &format!("DEVELOPMENT_TEAM={team_id}"),
            "CODE_SIGN_STYLE=Automatic",
            &format!("PRODUCT_BUNDLE_IDENTIFIER={READINESS_BUNDLE_ID}"),
            "INFOPLIST_KEY_CFBundleDisplayName=Tohseno Readiness",
            "-allowProvisioningUpdates",
            "build",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err("Xcode could not build and sign the readiness app".into());
    }
    let app = derived.join("Build/Products/Release-iphoneos/HelloWorld.app");
    let metadata = fs::symlink_metadata(&app)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Xcode did not produce the readiness app".into());
    }
    let signed = Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&app)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !signed.success() {
        return Err("the readiness app signature did not verify".into());
    }
    installing()?;
    install::install(target, &app, READINESS_BUNDLE_ID)?;
    install::launch(target, READINESS_BUNDLE_ID)?;
    install::retire(target, READINESS_BUNDLE_ID)?;
    Ok(())
}

fn supported_macos() -> bool {
    Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|value| value.trim().split('.').next()?.parse::<u64>().ok())
        .is_some_and(|major| major >= 14)
}

fn xcode_components_ready() -> bool {
    let quiet_success = |arguments: &[&str]| {
        Command::new("xcodebuild")
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    };
    quiet_success(&["-license", "check"]) && quiet_success(&["-checkFirstLaunchStatus"])
}

fn validate(record: &ReadinessRecord) -> Result<(), BoxError> {
    if record.schema != SCHEMA
        || record.revision == 0
        || record
            .last_error
            .as_ref()
            .is_some_and(|value| value.len() > 500)
    {
        return Err("iPhone readiness record is invalid".into());
    }
    Ok(())
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    fn ready_observation() -> ReadinessObservation {
        ReadinessObservation {
            macos_supported: true,
            xcode_ready: true,
            components_ready: true,
            device: Some(device::DeviceState::Ready(device::Device {
                identifier: "fixture".into(),
                udid: None,
                name: "iPhone".into(),
                product_type: None,
                marketing_name: None,
                os_version: None,
                os_build: None,
                physical: true,
                transport: "usb".into(),
            })),
            signing_team: Some("TEAM123".into()),
        }
    }

    #[test]
    fn every_gate_is_machine_observed_and_has_at_most_one_action() {
        let mut record = ReadinessRecord::default();
        assert_eq!(project(&record, &ready_observation()).step, "welcome");
        record.begun = true;
        let mut observed = ready_observation();
        observed.macos_supported = false;
        assert_eq!(project(&record, &observed).step, "unsupported_macos");
        observed = ready_observation();
        observed.xcode_ready = false;
        assert_eq!(project(&record, &observed).step, "install_xcode");
        observed = ready_observation();
        observed.components_ready = false;
        assert_eq!(project(&record, &observed).step, "finish_xcode");
        observed = ready_observation();
        observed.device = Some(device::DeviceState::CableMissing);
        assert_eq!(project(&record, &observed).step, "connect_iphone");
        observed.device = Some(device::DeviceState::TrustRequired);
        assert_eq!(project(&record, &observed).step, "trust_mac");
        observed.device = Some(device::DeviceState::DeveloperModeRequired);
        assert_eq!(project(&record, &observed).step, "developer_mode");
        observed = ready_observation();
        observed.signing_team = None;
        assert_eq!(project(&record, &observed).step, "apple_account");
        let verify = project(&record, &ready_observation());
        assert_eq!(verify.step, "verify_installation");
        assert_eq!(verify.primary_action, Some("verify_installation"));
        record.verification = VerificationState::Verified;
        assert!(project(&record, &ready_observation()).ready);
    }

    #[test]
    fn interrupted_verification_recovers_without_claiming_success() {
        let directory = tempfile::tempdir().unwrap();
        let store = ReadinessStore::open(directory.path()).unwrap();
        store
            .verification(VerificationState::Installing, None)
            .unwrap();
        assert!(store.recover_interrupted().unwrap());
        let record = store.load().unwrap();
        assert_eq!(record.verification, VerificationState::Failed);
        assert!(record.last_error.unwrap().contains("interrupted"));
    }
}
