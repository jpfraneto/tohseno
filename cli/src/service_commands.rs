use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub const SERVICE_LABEL: &str = "com.tohseno.workspace-service";
const VERIFICATION_LABEL_PREFIX: &str = "com.tohseno.workspace-service.verification.";
const PLIST_MARKER: &str = "TOHSENO_WORKSPACE_SERVICE_PLIST_V1";

pub trait Launchctl: Send + Sync {
    fn run(&self, arguments: &[String]) -> Result<Output, std::io::Error>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemLaunchctl;

impl Launchctl for SystemLaunchctl {
    fn run(&self, arguments: &[String]) -> Result<Output, std::io::Error> {
        Command::new(launchctl_program()?)
            .args(arguments)
            .stdin(Stdio::null())
            .output()
    }
}

fn launchctl_program() -> Result<PathBuf, std::io::Error> {
    #[cfg(debug_assertions)]
    if let Some(value) = std::env::var_os("TOHSENO_TEST_LAUNCHCTL") {
        return validated_test_launchctl(PathBuf::from(value));
    }
    Ok(PathBuf::from("/bin/launchctl"))
}

#[cfg(debug_assertions)]
fn validated_test_launchctl(path: PathBuf) -> Result<PathBuf, std::io::Error> {
    if !path.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "TOHSENO_TEST_LAUNCHCTL must be absolute",
        ));
    }
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "TOHSENO_TEST_LAUNCHCTL must be a regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "TOHSENO_TEST_LAUNCHCTL must be executable",
            ));
        }
    }
    Ok(path)
}

#[derive(Clone, Debug)]
pub struct ServicePaths {
    pub service_label: String,
    pub install_root: PathBuf,
    pub launcher: PathBuf,
    pub logs: PathBuf,
    pub service_state: PathBuf,
    pub launch_agent: PathBuf,
}

impl ServicePaths {
    pub fn discover() -> Result<Self, Box<dyn std::error::Error>> {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        let home = PathBuf::from(home);
        if !home.is_absolute() {
            return Err("HOME must be absolute".into());
        }
        let install_root = match std::env::var_os("TOHSENO_INSTALL_ROOT") {
            Some(root) => PathBuf::from(root),
            None => home.join(".tohseno"),
        };
        let launch_agents = match std::env::var_os("TOHSENO_LAUNCH_AGENTS_DIR") {
            Some(root) => PathBuf::from(root),
            None => home.join("Library/LaunchAgents"),
        };
        if !install_root.is_absolute() || !launch_agents.is_absolute() {
            return Err("service paths must be absolute".into());
        }
        let service_label = configured_service_label()?;
        Ok(Self {
            launcher: install_root.join("bin/tohseno"),
            logs: install_root.join("logs"),
            service_state: install_root.join("service"),
            launch_agent: launch_agents.join(format!("{service_label}.plist")),
            service_label,
            install_root,
        })
    }

    fn domain(&self) -> String {
        format!("gui/{}", unsafe { libc::getuid() })
    }

    fn service_target(&self) -> String {
        format!("{}/{}", self.domain(), self.service_label)
    }
}

fn configured_service_label() -> Result<String, Box<dyn std::error::Error>> {
    if std::env::var("TOHSENO_VERIFICATION_MODE").as_deref() != Ok("1") {
        return Ok(SERVICE_LABEL.into());
    }
    let label = std::env::var("TOHSENO_VERIFICATION_SERVICE_LABEL")
        .map_err(|_| "verification mode requires TOHSENO_VERIFICATION_SERVICE_LABEL")?;
    let suffix = label
        .strip_prefix(VERIFICATION_LABEL_PREFIX)
        .ok_or("verification service label has the wrong namespace")?;
    if suffix.is_empty()
        || label.len() > 128
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return Err("verification service label is invalid".into());
    }
    Ok(label)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ServiceAdminReceipt {
    pub schema: &'static str,
    pub operation: &'static str,
    pub installed: bool,
    pub launch_agent: String,
    pub service_label: String,
    pub state_preserved: bool,
}

pub fn install(
    paths: &ServicePaths,
    launchctl: &dyn Launchctl,
) -> Result<ServiceAdminReceipt, Box<dyn std::error::Error>> {
    require_safe_root(&paths.install_root)?;
    require_regular_launcher(&paths.launcher)?;
    ensure_private_directory(&paths.logs)?;
    ensure_private_directory(&paths.service_state)?;
    let parent = paths
        .launch_agent
        .parent()
        .ok_or("LaunchAgent path has no parent")?;
    ensure_real_directory(parent)?;
    let expected = plist(paths);
    match fs::symlink_metadata(&paths.launch_agent) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err("refusing an unsafe LaunchAgent path".into());
        }
        Ok(_) => {
            let existing = read_regular_bounded(&paths.launch_agent, 64 * 1024)?;
            if existing != expected.as_bytes() {
                return Err("refusing to overwrite an unrecognized LaunchAgent".into());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            write_new(&paths.launch_agent, expected.as_bytes(), 0o644)?;
        }
        Err(error) => return Err(error.into()),
    }
    // Bootstrap may start this RunAtLoad service before kickstart reaches it.
    // Accept either successful operation; the caller's health check remains
    // authoritative over whether the launched process actually became ready.
    let bootstrap = launchctl.run(&[
        "bootstrap".into(),
        paths.domain(),
        paths.launch_agent.display().to_string(),
    ])?;
    let kickstart = launchctl.run(&["kickstart".into(), "-k".into(), paths.service_target()])?;
    if !kickstart.status.success() && !bootstrap.status.success() {
        return Err("launchd could not start the Local Workspace Service".into());
    }
    Ok(receipt("install", true, paths))
}

pub fn start(
    paths: &ServicePaths,
    launchctl: &dyn Launchctl,
) -> Result<ServiceAdminReceipt, Box<dyn std::error::Error>> {
    require_recognized_agent(paths)?;
    let bootstrap = launchctl.run(&[
        "bootstrap".into(),
        paths.domain(),
        paths.launch_agent.display().to_string(),
    ])?;
    let kickstart = launchctl.run(&["kickstart".into(), "-k".into(), paths.service_target()])?;
    if !kickstart.status.success() && !bootstrap.status.success() {
        return Err("launchd could not start the Local Workspace Service".into());
    }
    Ok(receipt("start", true, paths))
}

pub fn stop(
    paths: &ServicePaths,
    launchctl: &dyn Launchctl,
) -> Result<ServiceAdminReceipt, Box<dyn std::error::Error>> {
    require_recognized_agent(paths)?;
    let output = launchctl.run(&[
        "bootout".into(),
        paths.domain(),
        paths.launch_agent.display().to_string(),
    ])?;
    // bootout is also the clean-stop marker. An already-unloaded job is
    // considered stopped as long as its owned plist was validated first.
    if !output.status.success() {
        let check = launchctl.run(&["print".into(), paths.service_target()])?;
        if check.status.success() {
            return Err("launchd could not stop the Local Workspace Service".into());
        }
    }
    Ok(receipt("stop", true, paths))
}

pub fn restart(
    paths: &ServicePaths,
    launchctl: &dyn Launchctl,
) -> Result<ServiceAdminReceipt, Box<dyn std::error::Error>> {
    let _ = stop(paths, launchctl)?;
    let _ = start(paths, launchctl)?;
    Ok(receipt("restart", true, paths))
}

pub fn uninstall(
    paths: &ServicePaths,
    launchctl: &dyn Launchctl,
) -> Result<ServiceAdminReceipt, Box<dyn std::error::Error>> {
    match fs::symlink_metadata(&paths.launch_agent) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err("refusing to remove an unsafe LaunchAgent".into());
        }
        Ok(_) => {
            require_recognized_agent(paths)?;
            let _ = stop(paths, launchctl);
            fs::remove_file(&paths.launch_agent)?;
            File::open(
                paths
                    .launch_agent
                    .parent()
                    .ok_or("LaunchAgent has no parent")?,
            )?
            .sync_all()?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(receipt("uninstall", false, paths))
}

pub fn launchd_loaded(
    paths: &ServicePaths,
    launchctl: &dyn Launchctl,
) -> Result<bool, Box<dyn std::error::Error>> {
    if !paths.launch_agent.is_file() {
        return Ok(false);
    }
    require_recognized_agent(paths)?;
    Ok(launchctl
        .run(&["print".into(), paths.service_target()])?
        .status
        .success())
}

pub fn bounded_logs(paths: &ServicePaths) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut lines = Vec::new();
    for name in ["workspace-service.log", "workspace-service.error.log"] {
        let path = paths.logs.join(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata)
                if metadata.file_type().is_symlink()
                    || !metadata.is_file()
                    || metadata.len() > 10 * 1024 * 1024 =>
            {
                return Err("service log is unsafe or unbounded".into());
            }
            Ok(_) => {
                let body = String::from_utf8(read_regular_bounded(&path, 10 * 1024 * 1024)?)
                    .map_err(|_| "service log is not UTF-8")?;
                lines.extend(body.lines().rev().take(100).map(str::to_owned));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    lines.reverse();
    Ok(lines)
}

fn receipt(operation: &'static str, installed: bool, paths: &ServicePaths) -> ServiceAdminReceipt {
    ServiceAdminReceipt {
        schema: "tohseno.service-admin-receipt/1",
        operation,
        installed,
        launch_agent: paths.launch_agent.display().to_string(),
        service_label: paths.service_label.clone(),
        state_preserved: true,
    }
}

fn plist(paths: &ServicePaths) -> String {
    let service_label = xml_escape(&paths.service_label);
    let launcher = xml_escape(&paths.launcher.display().to_string());
    let stdout = xml_escape(
        &paths
            .logs
            .join("workspace-service.log")
            .display()
            .to_string(),
    );
    let stderr = xml_escape(
        &paths
            .logs
            .join("workspace-service.error.log")
            .display()
            .to_string(),
    );
    let environment = launchd_verification_environment()
        .into_iter()
        .map(|(key, value)| {
            format!(
                "    <key>{}</key><string>{}</string>\n",
                xml_escape(&key),
                xml_escape(&value)
            )
        })
        .collect::<String>();
    let environment = if environment.is_empty() {
        String::new()
    } else {
        format!("  <key>EnvironmentVariables</key>\n  <dict>\n{environment}  </dict>\n")
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<!-- {PLIST_MARKER} -->
<plist version="1.0">
<dict>
  <key>Label</key><string>{service_label}</string>
  <key>ProgramArguments</key>
  <array><string>{launcher}</string><string>--json</string><string>service</string><string>run</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
{environment}  <key>ProcessType</key><string>Background</string>
  <key>StandardOutPath</key><string>{stdout}</string>
  <key>StandardErrorPath</key><string>{stderr}</string>
  <key>ThrottleInterval</key><integer>10</integer>
</dict>
</plist>
"#
    )
}

fn launchd_verification_environment() -> BTreeMap<String, String> {
    if std::env::var("TOHSENO_VERIFICATION_MODE").as_deref() != Ok("1") {
        return BTreeMap::new();
    }
    const KEYS: &[&str] = &[
        "HOME",
        "TOHSENO_COMPANION_RELAY_ORIGIN",
        "TOHSENO_DATA_ROOT",
        "TOHSENO_HOME",
        "TOHSENO_APPLE_IDENTITY_HELPER",
        "TOHSENO_IDENTITY_BACKEND",
        "TOHSENO_INSTALL_ROOT",
        "TOHSENO_LAUNCH_AGENTS_DIR",
        "TOHSENO_SERVICE_PORT",
        "TOHSENO_VERIFICATION_KEYCHAIN_SERVICE",
        "TOHSENO_VERIFICATION_KEYCHAIN_PATH",
        "TOHSENO_VERIFICATION_MODE",
        "TOHSENO_VERIFICATION_SERVICE_LABEL",
    ];
    KEYS.iter()
        .filter_map(|key| std::env::var(key).ok().map(|value| ((*key).into(), value)))
        .collect()
}

fn require_recognized_agent(paths: &ServicePaths) -> Result<(), Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(&paths.launch_agent)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("LaunchAgent is not a regular installer-owned file".into());
    }
    if read_regular_bounded(&paths.launch_agent, 64 * 1024)? != plist(paths).as_bytes() {
        return Err("LaunchAgent is not recognized as installer-owned".into());
    }
    Ok(())
}

fn read_regular_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let before = fs::symlink_metadata(path)?;
    if before.file_type().is_symlink() || !before.is_file() || before.len() > maximum {
        return Err("managed file is not a bounded regular file".into());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() != before.len() {
        return Err("managed file changed while opening".into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len())?);
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum || bytes.len() as u64 != opened.len() {
        return Err("managed file changed while reading".into());
    }
    Ok(bytes)
}

fn require_regular_launcher(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("stable TOHSENO launcher is not a regular installer-owned file".into());
    }
    Ok(())
}

fn require_safe_root(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !path.is_absolute() || path.parent().is_none() {
        return Err("installer root is unsafe".into());
    }
    ensure_real_directory(path)
}

fn ensure_real_directory(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{} is not a real directory", path.display()).into());
    }
    Ok(())
}

fn ensure_private_directory(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(format!("{} is not a real directory", path.display()).into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir_all(path)?,
        Err(error) => return Err(error.into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8], mode: u32) -> Result<(), Box<dyn std::error::Error>> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(mode)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    File::open(path.parent().ok_or("managed file has no parent")?)?.sync_all()?;
    Ok(())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeLaunchctl {
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl Launchctl for FakeLaunchctl {
        fn run(&self, arguments: &[String]) -> Result<Output, std::io::Error> {
            self.calls.lock().unwrap().push(arguments.to_vec());
            Ok(Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }

    struct KickstartRaceLaunchctl;

    impl Launchctl for KickstartRaceLaunchctl {
        fn run(&self, arguments: &[String]) -> Result<Output, std::io::Error> {
            let failed = arguments.first().map(String::as_str) == Some("kickstart");
            Ok(Output {
                status: std::process::ExitStatus::from_raw(if failed { 1 << 8 } else { 0 }),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }

    fn paths(root: &Path) -> ServicePaths {
        let install_root = root.join(".tohseno");
        fs::create_dir_all(install_root.join("bin")).unwrap();
        fs::write(install_root.join("bin/tohseno"), b"launcher").unwrap();
        let agents = root.join("Library/LaunchAgents");
        fs::create_dir_all(&agents).unwrap();
        ServicePaths {
            service_label: SERVICE_LABEL.into(),
            launcher: install_root.join("bin/tohseno"),
            logs: install_root.join("logs"),
            service_state: install_root.join("service"),
            launch_agent: agents.join(format!("{SERVICE_LABEL}.plist")),
            install_root,
        }
    }

    #[test]
    fn install_and_uninstall_use_injectable_launchctl_and_preserve_state() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let launchctl = FakeLaunchctl::default();
        install(&paths, &launchctl).unwrap();
        fs::write(paths.service_state.join("pairing-preserved"), b"yes").unwrap();
        assert_eq!(
            fs::read(&paths.launch_agent).unwrap(),
            plist(&paths).as_bytes()
        );
        uninstall(&paths, &launchctl).unwrap();
        assert!(!paths.launch_agent.exists());
        assert!(paths.service_state.join("pairing-preserved").is_file());
        let calls = launchctl.calls.lock().unwrap();
        assert!(calls
            .iter()
            .any(|call| call.first().map(String::as_str) == Some("bootstrap")));
        assert!(calls
            .iter()
            .any(|call| call.first().map(String::as_str) == Some("bootout")));
    }

    #[test]
    fn install_accepts_run_at_load_winning_the_kickstart_race() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        install(&paths, &KickstartRaceLaunchctl).unwrap();
        assert!(paths.launch_agent.is_file());
    }

    #[test]
    fn unrecognized_launch_agent_is_never_overwritten_or_removed() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        fs::write(&paths.launch_agent, b"unrelated plist").unwrap();
        let launchctl = FakeLaunchctl::default();
        assert!(install(&paths, &launchctl).is_err());
        assert!(uninstall(&paths, &launchctl).is_err());
        assert_eq!(fs::read(&paths.launch_agent).unwrap(), b"unrelated plist");
    }

    #[test]
    fn marker_bearing_but_modified_launch_agent_is_unrecognized() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let mut tampered = plist(&paths);
        tampered.push_str("<!-- injected -->\n");
        fs::write(&paths.launch_agent, tampered.as_bytes()).unwrap();
        let launchctl = FakeLaunchctl::default();
        assert!(install(&paths, &launchctl).is_err());
        assert!(uninstall(&paths, &launchctl).is_err());
        assert_eq!(fs::read(&paths.launch_agent).unwrap(), tampered.as_bytes());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_launch_agent_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        std::os::unix::fs::symlink(root.path(), &paths.launch_agent).unwrap();
        assert!(install(&paths, &FakeLaunchctl::default()).is_err());
    }

    #[cfg(all(debug_assertions, unix))]
    #[test]
    fn test_launchctl_override_requires_an_absolute_regular_executable() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("launchctl");
        fs::write(&executable, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            validated_test_launchctl(executable.clone()).unwrap(),
            executable
        );

        let relative = PathBuf::from("relative-launchctl");
        assert!(validated_test_launchctl(relative).is_err());

        let non_executable = root.path().join("not-executable");
        fs::write(&non_executable, b"fixture").unwrap();
        fs::set_permissions(&non_executable, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(validated_test_launchctl(non_executable).is_err());

        let symlink = root.path().join("symlink");
        std::os::unix::fs::symlink(&executable, &symlink).unwrap();
        assert!(validated_test_launchctl(symlink).is_err());
    }
}
