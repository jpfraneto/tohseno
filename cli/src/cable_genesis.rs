use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use tohseno_engine::gates::device::Device;
use tohseno_engine::gates::device::DeviceState;
use uuid::Uuid;

const GENESIS_SCHEMA: &str = "tohseno.private-cable-genesis/1";
const MAXIMUM_RECORD_BYTES: u64 = 64 * 1024;
pub const COMPANION_BUILD_FAILURE: &str = "Tohseno could not build and sign the iPhone app. Open Xcode and check your Apple Account, then try again.";
pub const COMPANION_INSTALL_FAILURE: &str = "The app was built and signed, but your iPhone did not accept the installation. Keep it connected and unlocked, then try again.";
pub const COMPANION_PAIRING_FAILURE: &str =
    "Tohseno Companion is installed, but its private connection did not start. Try again.";
const LEGACY_COMPANION_PAIRING_FAILURE: &str =
    "The private Companion connection is not configured yet.";
pub const COMPANION_LAUNCH_FAILURE: &str = "The Companion was installed but did not launch.";
pub const COMPANION_INTERRUPTED_FAILURE: &str =
    "The previous iPhone installation was interrupted. Try again.";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionInstallState {
    Idle,
    Building,
    Installing,
    Launching,
    WaitingForPairing,
    Installed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CableGenesisRecord {
    pub schema: String,
    pub revision: u64,
    pub begun: bool,
    pub pre_xcode_trust_guidance_seen: bool,
    pub companion_install: CompanionInstallState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intended_device_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pairing_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl Default for CableGenesisRecord {
    fn default() -> Self {
        Self {
            schema: GENESIS_SCHEMA.into(),
            revision: 1,
            begun: false,
            pre_xcode_trust_guidance_seen: false,
            companion_install: CompanionInstallState::Idle,
            intended_device_digest: None,
            pairing_session_id: None,
            last_error: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenesisStep {
    PickUpIphone,
    ConnectCable,
    TrustMac,
    InstallXcode,
    DeveloperMode,
    AddAppleAccount,
    InstallCompanion,
    Pairing,
    FirstShot,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CableGenesisView {
    pub schema: &'static str,
    pub step: GenesisStep,
    pub instruction: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_action: Option<&'static str>,
    pub can_go_back: bool,
    pub automatically_observed: bool,
    pub companion_install_state: CompanionInstallState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_product_type: Option<String>,
}

#[derive(Clone, Debug)]
pub struct GenesisObservation {
    pub cable_visible: bool,
    pub xcode_ready: bool,
    pub device: Option<DeviceState>,
    pub signing_ready: bool,
    pub paired: bool,
}

#[derive(Clone, Debug)]
pub struct CableGenesisStore {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl CableGenesisStore {
    pub fn open(service_root: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let path = service_root.join("cable-genesis-v1.json");
        let store = Self {
            path,
            lock: Arc::new(Mutex::new(())),
        };
        if !store.path.exists() {
            store.write(&CableGenesisRecord::default())?;
        }
        store.load()?;
        Ok(store)
    }

    pub fn load(&self) -> Result<CableGenesisRecord, Box<dyn std::error::Error + Send + Sync>> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "cable genesis lock was poisoned")?;
        self.read_unlocked()
    }

    pub fn update(
        &self,
        change: impl FnOnce(&mut CableGenesisRecord),
    ) -> Result<CableGenesisRecord, Box<dyn std::error::Error + Send + Sync>> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "cable genesis lock was poisoned")?;
        let mut record = self.read_unlocked()?;
        change(&mut record);
        record.revision = record.revision.saturating_add(1);
        validate(&record)?;
        self.write_unlocked(&record)?;
        Ok(record)
    }

    pub fn begin(&self) -> Result<CableGenesisRecord, Box<dyn std::error::Error + Send + Sync>> {
        self.update(|record| record.begun = true)
    }

    pub fn acknowledge_unobservable_trust_guidance(
        &self,
    ) -> Result<CableGenesisRecord, Box<dyn std::error::Error + Send + Sync>> {
        self.update(|record| record.pre_xcode_trust_guidance_seen = true)
    }

    pub fn set_install_state(
        &self,
        state: CompanionInstallState,
        device_identifier: Option<&str>,
        pairing_session_id: Option<&str>,
        error: Option<&str>,
    ) -> Result<CableGenesisRecord, Box<dyn std::error::Error + Send + Sync>> {
        self.update(|record| {
            record.companion_install = state;
            if let Some(identifier) = device_identifier {
                record.intended_device_digest = Some(device_digest(identifier));
            }
            if let Some(session) = pairing_session_id {
                record.pairing_session_id = Some(session.into());
            }
            record.last_error = error.map(|value| value.chars().take(500).collect());
        })
    }

    /// An active installation or pairing session belongs to the service
    /// process that started it. If a new process opens the durable record, no
    /// such task can still be owned by this service, so expose a safe retry
    /// instead of replaying an endless progress screen.
    pub fn recover_interrupted_install(
        &self,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let record = self.load()?;
        if !matches!(
            record.companion_install,
            CompanionInstallState::Building
                | CompanionInstallState::Installing
                | CompanionInstallState::Launching
                | CompanionInstallState::WaitingForPairing
        ) {
            return Ok(false);
        }
        let error = if record.companion_install == CompanionInstallState::WaitingForPairing {
            COMPANION_PAIRING_FAILURE
        } else {
            COMPANION_INTERRUPTED_FAILURE
        };
        self.set_install_state(CompanionInstallState::Failed, None, None, Some(error))?;
        Ok(true)
    }

    fn read_unlocked(
        &self,
    ) -> Result<CableGenesisRecord, Box<dyn std::error::Error + Send + Sync>> {
        let metadata = fs::symlink_metadata(&self.path)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > MAXIMUM_RECORD_BYTES
        {
            return Err("cable genesis record is unsafe or oversized".into());
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let mut file = options.open(&self.path)?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(MAXIMUM_RECORD_BYTES + 1)
            .read_to_end(&mut bytes)?;
        let record: CableGenesisRecord = serde_json::from_slice(&bytes)?;
        validate(&record)?;
        Ok(record)
    }

    fn write(
        &self,
        record: &CableGenesisRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "cable genesis lock was poisoned")?;
        self.write_unlocked(record)
    }

    fn write_unlocked(
        &self,
        record: &CableGenesisRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        validate(record)?;
        let bytes = tohseno_protocol::canonical::to_vec(record)?;
        let parent = self
            .path
            .parent()
            .ok_or("cable genesis path has no parent")?;
        let temporary = parent.join(format!(".cable-genesis-{}.tmp", Uuid::new_v4().simple()));
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
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, &self.path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    }
}

pub fn project(record: &CableGenesisRecord, observed: &GenesisObservation) -> CableGenesisView {
    let connected_device = observed.device.as_ref().and_then(|state| match state {
        DeviceState::Ready(device) => Some(device),
        _ => None,
    });
    let view = |step, instruction, detail, primary_action, can_go_back, automatically_observed| {
        CableGenesisView {
            schema: "tohseno.cable-genesis-view/1",
            step,
            instruction,
            detail,
            primary_action,
            can_go_back,
            automatically_observed,
            companion_install_state: record.companion_install,
            device_name: connected_device.map(|device| device.name.clone()),
            device_product_type: connected_device.and_then(|device| {
                device
                    .marketing_name
                    .clone()
                    .or(device.product_type.clone())
            }),
        }
    };
    if !record.begun {
        return view(
            GenesisStep::PickUpIphone,
            "Pick up your iPhone.",
            None,
            Some("begin"),
            false,
            false,
        );
    }
    if !observed.cable_visible {
        return view(
            GenesisStep::ConnectCable,
            "Connect your iPhone to this Mac with a cable.",
            None,
            Some("check"),
            true,
            true,
        );
    }
    if !observed.xcode_ready {
        if !record.pre_xcode_trust_guidance_seen {
            return view(GenesisStep::TrustMac, "Unlock your iPhone and tap Trust.", Some("Xcode is not installed yet, so Tohseno will verify trust after Xcode is ready."), Some("continue"), true, false);
        }
        return view(
            GenesisStep::InstallXcode,
            "Install Xcode from the App Store, then open it once.",
            None,
            Some("open_app_store"),
            true,
            true,
        );
    }
    match observed.device.as_ref() {
        Some(DeviceState::CableMissing) | None => {
            return view(
                GenesisStep::ConnectCable,
                "Connect your iPhone to this Mac with a cable.",
                None,
                Some("check"),
                true,
                true,
            )
        }
        Some(DeviceState::TrustRequired) => {
            return view(
                GenesisStep::TrustMac,
                "Unlock your iPhone and tap Trust.",
                None,
                Some("check"),
                true,
                true,
            )
        }
        Some(DeviceState::DeveloperModeRequired) => {
            return view(
                GenesisStep::DeveloperMode,
                "On your iPhone, open Settings → Privacy & Security → Developer Mode.",
                Some("Turn it on and let your iPhone restart."),
                Some("check"),
                true,
                true,
            )
        }
        Some(DeviceState::Ready(_)) => {}
    }
    if !observed.signing_ready {
        return view(
            GenesisStep::AddAppleAccount,
            "Add your Apple Account in Xcode.",
            Some("Tohseno will open Xcode. In the Mac menu bar choose Xcode → Settings… → Accounts, click +, and sign in. A free Personal Team works. Return here when Xcode shows your account; Tohseno detects it automatically."),
            Some("open_xcode_accounts"),
            true,
            true,
        );
    }
    match record.companion_install {
        CompanionInstallState::Idle | CompanionInstallState::Failed => {
            let pairing_retry = matches!(
                record.last_error.as_deref(),
                Some(COMPANION_PAIRING_FAILURE | LEGACY_COMPANION_PAIRING_FAILURE)
            );
            let failure = match record.last_error.as_deref() {
                Some(COMPANION_BUILD_FAILURE) => Some(COMPANION_BUILD_FAILURE),
                Some(COMPANION_INSTALL_FAILURE) => Some(COMPANION_INSTALL_FAILURE),
                Some(COMPANION_PAIRING_FAILURE) => Some(COMPANION_PAIRING_FAILURE),
                Some(LEGACY_COMPANION_PAIRING_FAILURE) => Some(COMPANION_PAIRING_FAILURE),
                Some(COMPANION_LAUNCH_FAILURE) => Some(COMPANION_LAUNCH_FAILURE),
                Some(COMPANION_INTERRUPTED_FAILURE) => Some(COMPANION_INTERRUPTED_FAILURE),
                Some(_) => Some(
                    "The previous installation stopped safely. Your existing work was unchanged.",
                ),
                None => None,
            };
            return view(
                GenesisStep::InstallCompanion,
                if pairing_retry {
                    "Finish connecting Tohseno Companion on your iPhone."
                } else {
                    "Install Tohseno Companion on your iPhone."
                },
                failure,
                Some(if pairing_retry {
                    "retry_companion"
                } else {
                    "install_companion"
                }),
                true,
                false,
            );
        }
        CompanionInstallState::Building => {
            return view(
                GenesisStep::InstallCompanion,
                "Building Tohseno Companion for your iPhone…",
                Some("Xcode is compiling and signing the app. This can take a few minutes. Keep your iPhone connected and unlocked."),
                None,
                false,
                true,
            );
        }
        CompanionInstallState::Installing => {
            return view(
                GenesisStep::InstallCompanion,
                "Installing Tohseno Companion on your iPhone…",
                Some("The build finished. Tohseno is copying the Companion to your iPhone. Keep it connected and unlocked."),
                None,
                false,
                true,
            );
        }
        CompanionInstallState::Launching => {
            return view(
                GenesisStep::InstallCompanion,
                "Opening Tohseno Companion on your iPhone…",
                Some("The Companion is installed. Tohseno is launching it and starting your private connection."),
                None,
                false,
                true,
            );
        }
        CompanionInstallState::WaitingForPairing | CompanionInstallState::Installed
            if !observed.paired =>
        {
            return view(
                GenesisStep::Pairing,
                "Finish setup in Tohseno Companion on your iPhone.",
                Some("This screen continues automatically when your private connection is ready."),
                None,
                false,
                true,
            );
        }
        CompanionInstallState::WaitingForPairing | CompanionInstallState::Installed => {}
    }
    view(
        GenesisStep::FirstShot,
        "Take your first Shot.",
        None,
        Some("create_app"),
        false,
        false,
    )
}

pub fn device_digest(identifier: &str) -> String {
    tohseno_protocol::digest::sha256(format!("TOHSENO-CABLE-DEVICE-V1\0{identifier}").as_bytes())
        .to_string()
        .trim_start_matches("0x")
        .into()
}

fn validate(record: &CableGenesisRecord) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if record.schema != GENESIS_SCHEMA
        || record.revision == 0
        || record.intended_device_digest.as_ref().is_some_and(|value| {
            value.len() != 64
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
        || record.pairing_session_id.as_ref().is_some_and(|value| {
            value.is_empty()
                || value.len() > 160
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
        || record
            .last_error
            .as_ref()
            .is_some_and(|value| value.len() > 500)
    {
        return Err("cable genesis record is invalid".into());
    }
    Ok(())
}

pub fn build_and_install_companion(
    project: &Path,
    service_root: &Path,
    device: &Device,
    team_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    build_and_install_companion_with_progress(project, service_root, device, team_id, || Ok(()))
}

pub fn build_and_install_companion_with_progress(
    project: &Path,
    service_root: &Path,
    device: &Device,
    team_id: &str,
    installation_started: impl FnOnce() -> Result<(), Box<dyn std::error::Error + Send + Sync>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let project_metadata = fs::symlink_metadata(project)?;
    if project_metadata.file_type().is_symlink() || !project_metadata.is_dir() {
        return Err("the installed Companion Xcode project is unavailable".into());
    }
    if team_id.is_empty()
        || team_id.len() > 32
        || !team_id.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err("the selected Apple development team is invalid".into());
    }
    let derived = service_root.join("companion-derived-data");
    match fs::symlink_metadata(&derived) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("the private Companion build directory is unsafe".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&derived)?,
        Err(error) => return Err(error.into()),
    }
    let destination = device.udid.as_deref().unwrap_or(&device.identifier);
    let status = Command::new("xcodebuild")
        .args(["-project"])
        .arg(project)
        .args([
            "-scheme",
            "TohsenoCompanion",
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
            "PRODUCT_BUNDLE_IDENTIFIER=com.tohseno.companion",
            "-allowProvisioningUpdates",
            "build",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err("the Companion build or Apple signing step did not complete".into());
    }
    let app = derived.join("Build/Products/Release-iphoneos/TohsenoCompanion.app");
    let metadata = fs::symlink_metadata(&app)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("the signed Companion app was not produced".into());
    }
    let signed = Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&app)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !signed.success() {
        return Err("the signed Companion app did not pass local verification".into());
    }
    installation_started()?;
    let json = service_root.join(format!(
        ".companion-install-{}.json",
        Uuid::new_v4().simple()
    ));
    let installed = Command::new("xcrun")
        .args([
            "devicectl",
            "device",
            "install",
            "app",
            "--device",
            &device.identifier,
        ])
        .arg(&app)
        .args(["--json-output"])
        .arg(&json)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = fs::remove_file(&json);
    if !installed?.success() {
        return Err("the Companion could not be installed on the connected iPhone".into());
    }
    Ok(())
}

pub fn launch_companion_bootstrap(
    service_root: &Path,
    device: &Device,
    invitation: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !invitation.starts_with("tohseno://pair/v1/") || invitation.len() > 16 * 1024 {
        return Err("the one-use Companion invitation is invalid".into());
    }
    let json = service_root.join(format!(
        ".companion-launch-{}.json",
        Uuid::new_v4().simple()
    ));
    let launched = Command::new("xcrun")
        .args(companion_launch_arguments(
            &device.identifier,
            Some(invitation),
            false,
            &json,
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = fs::remove_file(&json);
    if !launched?.success() {
        return Err("the Companion was installed but could not be launched".into());
    }
    Ok(())
}

pub fn launch_companion(
    service_root: &Path,
    device: &Device,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let json = service_root.join(format!(
        ".companion-launch-{}.json",
        Uuid::new_v4().simple()
    ));
    let launched = Command::new("xcrun")
        .args(companion_launch_arguments(
            &device.identifier,
            None,
            true,
            &json,
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = fs::remove_file(&json);
    if !launched?.success() {
        return Err("the Companion was installed but could not be launched".into());
    }
    Ok(())
}

fn companion_launch_arguments(
    device_identifier: &str,
    invitation: Option<&str>,
    terminate_existing: bool,
    json_output: &Path,
) -> Vec<OsString> {
    let mut arguments = [
        "devicectl",
        "device",
        "process",
        "launch",
        "--device",
        device_identifier,
    ]
    .into_iter()
    .map(OsString::from)
    .collect::<Vec<_>>();
    if let Some(invitation) = invitation {
        arguments.push("--payload-url".into());
        arguments.push(invitation.into());
    }
    if terminate_existing {
        arguments.push("--terminate-existing".into());
    }
    arguments.push("--json-output".into());
    arguments.push(json_output.as_os_str().to_owned());
    // devicectl treats everything after this positional value as arguments to
    // the launched process, so every devicectl option must precede it.
    arguments.push("com.tohseno.companion".into());
    arguments
}

#[cfg(test)]
mod tests {
    use super::*;
    use tohseno_engine::gates::device::Device;

    fn ready() -> DeviceState {
        DeviceState::Ready(Device {
            identifier: "core".into(),
            udid: None,
            name: "iPhone".into(),
            product_type: None,
            marketing_name: None,
            os_version: None,
            os_build: None,
            physical: true,
            transport: "usb".into(),
        })
    }

    fn observed() -> GenesisObservation {
        GenesisObservation {
            cable_visible: true,
            xcode_ready: true,
            device: Some(ready()),
            signing_ready: true,
            paired: false,
        }
    }

    #[test]
    fn every_machine_observable_gate_has_one_instruction() {
        let mut record = CableGenesisRecord::default();
        assert_eq!(
            project(&record, &observed()).step,
            GenesisStep::PickUpIphone
        );
        record.begun = true;
        let mut state = observed();
        state.cable_visible = false;
        let connect = project(&record, &state);
        assert_eq!(connect.step, GenesisStep::ConnectCable);
        assert_eq!(connect.primary_action, Some("check"));
        assert!(connect.automatically_observed);
        let mut state = observed();
        state.device = Some(DeviceState::TrustRequired);
        let trust = project(&record, &state);
        assert_eq!(trust.step, GenesisStep::TrustMac);
        assert_eq!(trust.primary_action, Some("check"));
        let mut state = observed();
        state.device = Some(DeviceState::DeveloperModeRequired);
        let developer_mode = project(&record, &state);
        assert_eq!(developer_mode.step, GenesisStep::DeveloperMode);
        assert_eq!(developer_mode.primary_action, Some("check"));
        let mut state = observed();
        state.signing_ready = false;
        assert_eq!(project(&record, &state).step, GenesisStep::AddAppleAccount);
    }

    #[test]
    fn xcode_dependency_returns_to_deferred_device_verification() {
        let mut record = CableGenesisRecord {
            begun: true,
            ..CableGenesisRecord::default()
        };
        let mut state = observed();
        state.xcode_ready = false;
        assert_eq!(project(&record, &state).step, GenesisStep::TrustMac);
        record.pre_xcode_trust_guidance_seen = true;
        assert_eq!(project(&record, &state).step, GenesisStep::InstallXcode);
        state.xcode_ready = true;
        state.device = Some(DeviceState::DeveloperModeRequired);
        assert_eq!(project(&record, &state).step, GenesisStep::DeveloperMode);
    }

    #[test]
    fn installation_and_pairing_cannot_claim_completion_early() {
        let mut record = CableGenesisRecord {
            begun: true,
            companion_install: CompanionInstallState::Building,
            ..CableGenesisRecord::default()
        };
        let building = project(&record, &observed());
        assert_eq!(building.step, GenesisStep::InstallCompanion);
        assert_eq!(
            building.instruction,
            "Building Tohseno Companion for your iPhone…"
        );
        assert!(building.detail.unwrap().contains("few minutes"));
        record.companion_install = CompanionInstallState::Installing;
        let installing = project(&record, &observed());
        assert_eq!(
            installing.instruction,
            "Installing Tohseno Companion on your iPhone…"
        );
        assert!(installing.detail.unwrap().contains("build finished"));
        record.companion_install = CompanionInstallState::Launching;
        let launching = project(&record, &observed());
        assert_eq!(
            launching.instruction,
            "Opening Tohseno Companion on your iPhone…"
        );
        assert!(launching.detail.unwrap().contains("Companion is installed"));
        record.companion_install = CompanionInstallState::Failed;
        record.last_error = Some(COMPANION_INSTALL_FAILURE.into());
        assert_eq!(
            project(&record, &observed()).detail,
            Some(COMPANION_INSTALL_FAILURE)
        );
        record.last_error = Some(LEGACY_COMPANION_PAIRING_FAILURE.into());
        let retry = project(&record, &observed());
        assert_eq!(
            retry.instruction,
            "Finish connecting Tohseno Companion on your iPhone."
        );
        assert_eq!(retry.detail, Some(COMPANION_PAIRING_FAILURE));
        assert_eq!(retry.primary_action, Some("retry_companion"));
        record.companion_install = CompanionInstallState::WaitingForPairing;
        let pairing = project(&record, &observed());
        assert_eq!(pairing.step, GenesisStep::Pairing);
        assert_eq!(
            pairing.instruction,
            "Finish setup in Tohseno Companion on your iPhone."
        );
        assert_eq!(pairing.primary_action, None);
        let mut paired = observed();
        paired.paired = true;
        assert_eq!(project(&record, &paired).step, GenesisStep::FirstShot);
    }

    #[test]
    fn record_survives_restart_without_raw_device_identity() {
        let root = tempfile::tempdir().unwrap();
        let store = CableGenesisStore::open(root.path()).unwrap();
        store.begin().unwrap();
        store
            .set_install_state(
                CompanionInstallState::Building,
                Some("private-device-id"),
                Some("session_fixture"),
                None,
            )
            .unwrap();
        let reopened = CableGenesisStore::open(root.path()).unwrap();
        let record = reopened.load().unwrap();
        assert!(record.begun);
        assert_eq!(
            record.intended_device_digest.as_deref(),
            Some(device_digest("private-device-id").as_str())
        );
        assert!(!serde_json::to_string(&record)
            .unwrap()
            .contains("private-device-id"));
    }

    #[test]
    fn interrupted_install_becomes_a_retry_after_restart() {
        let root = tempfile::tempdir().unwrap();
        let store = CableGenesisStore::open(root.path()).unwrap();
        store.begin().unwrap();
        store
            .set_install_state(CompanionInstallState::Building, None, None, None)
            .unwrap();
        let reopened = CableGenesisStore::open(root.path()).unwrap();
        assert!(reopened.recover_interrupted_install().unwrap());
        let record = reopened.load().unwrap();
        assert_eq!(record.companion_install, CompanionInstallState::Failed);
        assert_eq!(
            record.last_error.as_deref(),
            Some(COMPANION_INTERRUPTED_FAILURE)
        );
        let view = project(&record, &observed());
        assert_eq!(view.primary_action, Some("install_companion"));
        assert!(!reopened.recover_interrupted_install().unwrap());

        reopened
            .set_install_state(
                CompanionInstallState::WaitingForPairing,
                None,
                Some("session_fixture"),
                None,
            )
            .unwrap();
        assert!(reopened.recover_interrupted_install().unwrap());
        let record = reopened.load().unwrap();
        assert_eq!(record.companion_install, CompanionInstallState::Failed);
        assert_eq!(
            record.last_error.as_deref(),
            Some(COMPANION_PAIRING_FAILURE)
        );
        let view = project(&record, &observed());
        assert_eq!(view.primary_action, Some("retry_companion"));
    }

    #[test]
    fn pairing_url_is_a_devicectl_option_not_an_app_argument() {
        let arguments = companion_launch_arguments(
            "private-device-id",
            Some("tohseno://pair/v1/fixture"),
            false,
            Path::new("/private/result.json"),
        );
        let arguments = arguments
            .iter()
            .map(|argument| argument.to_string_lossy())
            .collect::<Vec<_>>();
        let bundle = arguments
            .iter()
            .position(|argument| argument == "com.tohseno.companion")
            .unwrap();
        let payload = arguments
            .iter()
            .position(|argument| argument == "--payload-url")
            .unwrap();
        let output = arguments
            .iter()
            .position(|argument| argument == "--json-output")
            .unwrap();
        assert_eq!(bundle, arguments.len() - 1);
        assert!(payload < bundle);
        assert!(output < bundle);
        assert_eq!(arguments[payload + 1], "tohseno://pair/v1/fixture");
    }
}
