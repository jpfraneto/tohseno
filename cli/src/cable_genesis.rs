use serde::{Deserialize, Serialize};
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
    let view = |step, instruction, detail, primary_action, can_go_back, automatically_observed| {
        CableGenesisView {
            schema: "tohseno.cable-genesis-view/1",
            step,
            instruction,
            detail,
            primary_action,
            can_go_back,
            automatically_observed,
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
            None,
            true,
            true,
        );
    }
    if !observed.xcode_ready {
        if !record.pre_xcode_trust_guidance_seen {
            return view(GenesisStep::TrustMac, "Unlock your iPhone and tap Trust.", Some("Xcode is not installed yet, so TOHSENO will verify trust after Xcode is ready."), Some("continue"), true, false);
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
                None,
                true,
                true,
            )
        }
        Some(DeviceState::TrustRequired) => {
            return view(
                GenesisStep::TrustMac,
                "Unlock your iPhone and tap Trust.",
                None,
                None,
                true,
                true,
            )
        }
        Some(DeviceState::DeveloperModeRequired) => {
            return view(
                GenesisStep::DeveloperMode,
                "On your iPhone, open Settings → Privacy & Security → Developer Mode.",
                Some("Turn it on and let your iPhone restart."),
                None,
                true,
                true,
            )
        }
        Some(DeviceState::Ready(_)) => {}
    }
    if !observed.signing_ready {
        return view(
            GenesisStep::AddAppleAccount,
            "Open Xcode → Settings → Accounts and add your Apple Account.",
            None,
            Some("open_xcode_accounts"),
            true,
            true,
        );
    }
    match record.companion_install {
        CompanionInstallState::Idle | CompanionInstallState::Failed => {
            return view(
                GenesisStep::InstallCompanion,
                "Install TOHSENO on your iPhone.",
                record.last_error.as_ref().map(|_| {
                    "The previous installation stopped safely. Your existing work was unchanged."
                }),
                Some("install_companion"),
                true,
                false,
            );
        }
        CompanionInstallState::Building
        | CompanionInstallState::Installing
        | CompanionInstallState::Launching => {
            return view(
                GenesisStep::InstallCompanion,
                "Installing TOHSENO on your iPhone…",
                None,
                None,
                false,
                true,
            );
        }
        CompanionInstallState::WaitingForPairing | CompanionInstallState::Installed
            if !observed.paired =>
        {
            return view(GenesisStep::Pairing, "Write down the twelve words shown on your iPhone.", Some("Then confirm them on the iPhone. The private connection completes automatically."), None, false, true);
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
        .args([
            "devicectl",
            "device",
            "process",
            "launch",
            "--device",
            &device.identifier,
            "com.tohseno.companion",
            "--payload-url",
            invitation,
            "--terminate-existing",
            "--json-output",
        ])
        .arg(&json)
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
        .args([
            "devicectl",
            "device",
            "process",
            "launch",
            "--device",
            &device.identifier,
            "com.tohseno.companion",
            "--terminate-existing",
            "--json-output",
        ])
        .arg(&json)
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
        assert_eq!(project(&record, &state).step, GenesisStep::ConnectCable);
        let mut state = observed();
        state.device = Some(DeviceState::TrustRequired);
        assert_eq!(project(&record, &state).step, GenesisStep::TrustMac);
        let mut state = observed();
        state.device = Some(DeviceState::DeveloperModeRequired);
        assert_eq!(project(&record, &state).step, GenesisStep::DeveloperMode);
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
        assert_eq!(
            project(&record, &observed()).step,
            GenesisStep::InstallCompanion
        );
        record.companion_install = CompanionInstallState::WaitingForPairing;
        assert_eq!(project(&record, &observed()).step, GenesisStep::Pairing);
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
}
