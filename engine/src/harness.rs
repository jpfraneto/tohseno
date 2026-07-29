use crate::config::HarnessConfig;
use crate::events::{Event, EventBus};
use crate::ledger::{Ledger, LedgerError, Shot};
use serde::Serialize;
use serde_json::Value;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Clone, Debug, Serialize)]
pub struct HarnessOption {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing)]
    pub command: String,
    pub installed: bool,
    pub selected: bool,
}

#[derive(Clone, Copy)]
struct KnownHarness {
    id: &'static str,
    label: &'static str,
    binaries: &'static [&'static str],
    home_paths: &'static [&'static str],
}

static KNOWN_HARNESSES: [KnownHarness; 5] = [
    KnownHarness {
        id: "hermes",
        label: "Hermes Agent",
        binaries: &["hermes"],
        home_paths: &[".local/bin/hermes", ".hermes/bin/hermes"],
    },
    KnownHarness {
        id: "codex",
        label: "Codex",
        binaries: &["codex"],
        home_paths: &[".local/bin/codex"],
    },
    KnownHarness {
        id: "claude",
        label: "Claude Code",
        binaries: &["claude"],
        home_paths: &[".local/bin/claude"],
    },
    KnownHarness {
        id: "grok",
        label: "Grok Build",
        binaries: &["grok", "grok-build", "grokbuild"],
        home_paths: &[".grok/bin/grok", ".local/bin/grok"],
    },
    KnownHarness {
        id: "opencode",
        label: "OpenCode",
        binaries: &["opencode"],
        home_paths: &[".opencode/bin/opencode", ".local/bin/opencode"],
    },
];

const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
const SAFE_SYSTEM_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// One fail-closed macOS Seatbelt boundary for a harness process tree.
///
/// The coding agent receives a fresh home and temporary directory, read-only
/// access to the staged Shot, write access only to `src/`, and read/execute
/// access to the selected harness plus Apple system developer tools. No host
/// environment variables, home-directory configuration, Keychain service, or
/// sibling ledger paths are admitted. Provider authentication therefore
/// deliberately remains unavailable until it can be supplied by a narrow
/// credential broker; host credentials must never be staged into this sandbox.
struct HarnessSandbox {
    _temporary: TempDir,
    executable: PathBuf,
    profile: PathBuf,
    home: PathBuf,
    temporary: PathBuf,
    tools: PathBuf,
    working_directory: PathBuf,
    canary: PathBuf,
}

impl HarnessSandbox {
    #[cfg(not(target_os = "macos"))]
    fn new(_program: &std::ffi::OsStr, _shot: Option<&Path>) -> Result<Self, HarnessError> {
        Err(HarnessError::IsolationUnavailable(
            "coding harnesses require the macOS Seatbelt boundary".into(),
        ))
    }

    #[cfg(target_os = "macos")]
    fn new(program: &std::ffi::OsStr, shot: Option<&Path>) -> Result<Self, HarnessError> {
        let sandbox_metadata = fs::symlink_metadata(SANDBOX_EXEC).map_err(|error| {
            HarnessError::IsolationUnavailable(format!(
                "macOS Seatbelt launcher is unavailable: {error}"
            ))
        })?;
        if sandbox_metadata.file_type().is_symlink() || !sandbox_metadata.is_file() {
            return Err(HarnessError::IsolationUnavailable(
                "macOS Seatbelt launcher is not a regular system file".into(),
            ));
        }

        let executable = resolve_executable(program)?;
        let temporary_guard = tempfile::Builder::new()
            .prefix("tohseno-harness-")
            .tempdir_in("/private/tmp")
            .map_err(HarnessError::Io)?;
        let temporary_root = fs::canonicalize(temporary_guard.path())?;
        let home = temporary_root.join("home");
        let temporary = temporary_root.join("tmp");
        let tools = temporary_root.join("tools");
        for directory in [
            &home,
            &temporary,
            &tools,
            &home.join(".cache"),
            &home.join(".config"),
            &home.join(".codex"),
            &home.join(".claude"),
        ] {
            fs::create_dir(directory)?;
        }

        let canonical_shot = match shot {
            Some(shot) => {
                let metadata = fs::symlink_metadata(shot)?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(HarnessError::IsolationUnavailable(
                        "Shot workspace is not a real directory".into(),
                    ));
                }
                let canonical = fs::canonicalize(shot)?;
                let source = canonical.join("src");
                let source_metadata = fs::symlink_metadata(&source)?;
                if source_metadata.file_type().is_symlink() || !source_metadata.is_dir() {
                    return Err(HarnessError::IsolationUnavailable(
                        "Shot output directory is not a real directory".into(),
                    ));
                }
                Some(canonical)
            }
            None => None,
        };
        let mut executable_reads = vec![ExecutableRead::Literal(executable.clone())];
        for root in executable_support_roots(&executable) {
            executable_reads.push(ExecutableRead::Subpath(root));
        }
        stage_shebang_interpreter(&executable, &tools, &mut executable_reads)?;
        let mut writable_roots = vec![home.clone(), temporary.clone(), tools.clone()];
        if let Some(shot) = &canonical_shot {
            writable_roots.push(shot.join("src"));
        }
        reject_executable_write_overlap(&executable_reads, &writable_roots)?;

        let working_directory = canonical_shot.clone().unwrap_or_else(|| home.clone());
        let canary = temporary_root.join("isolation-canary");
        fs::write(&canary, b"this must remain unreadable to the harness\n")?;
        let profile = temporary_root.join("harness.sb");
        fs::write(
            &profile,
            seatbelt_profile(
                &home,
                &temporary,
                &tools,
                canonical_shot.as_deref(),
                &executable_reads,
            )?,
        )?;

        let sandbox = Self {
            _temporary: temporary_guard,
            executable,
            profile,
            home,
            temporary,
            tools,
            working_directory,
            canary,
        };
        sandbox.verify_boundary()?;
        Ok(sandbox)
    }

    fn command(&self) -> Command {
        let mut command = Command::new(SANDBOX_EXEC);
        command
            .arg("-f")
            .arg(&self.profile)
            .arg(&self.executable)
            .current_dir(&self.working_directory)
            .env_clear()
            .envs(self.safe_environment())
            .stdin(Stdio::null());
        command
    }

    fn safe_environment(&self) -> Vec<(OsString, OsString)> {
        let path = format!("{}:{SAFE_SYSTEM_PATH}", self.tools.display());
        vec![
            ("HOME".into(), self.home.as_os_str().to_owned()),
            ("TMPDIR".into(), self.temporary.as_os_str().to_owned()),
            ("PATH".into(), path.into()),
            ("SHELL".into(), "/bin/sh".into()),
            ("USER".into(), "tohseno-harness".into()),
            ("LOGNAME".into(), "tohseno-harness".into()),
            ("LANG".into(), "en_US.UTF-8".into()),
            ("LC_ALL".into(), "en_US.UTF-8".into()),
            ("TERM".into(), "dumb".into()),
            ("NO_COLOR".into(), "1".into()),
            ("CI".into(), "1".into()),
            (
                "XDG_CONFIG_HOME".into(),
                self.home.join(".config").into_os_string(),
            ),
            (
                "XDG_CACHE_HOME".into(),
                self.home.join(".cache").into_os_string(),
            ),
            (
                "CODEX_HOME".into(),
                self.home.join(".codex").into_os_string(),
            ),
            (
                "CLAUDE_CONFIG_DIR".into(),
                self.home.join(".claude").into_os_string(),
            ),
            ("xcrun_nocache".into(), "1".into()),
            ("TOHSENO_HARNESS_ISOLATED".into(), "1".into()),
        ]
    }

    #[cfg(target_os = "macos")]
    fn verify_boundary(&self) -> Result<(), HarnessError> {
        let allowed = self
            .probe_command("/usr/bin/true")
            .output()
            .map_err(|error| {
                HarnessError::IsolationUnavailable(format!(
                    "could not start the macOS Seatbelt probe: {error}"
                ))
            })?;
        if !allowed.status.success() {
            return Err(HarnessError::IsolationUnavailable(format!(
                "macOS Seatbelt rejected the required system-tool probe: {}",
                String::from_utf8_lossy(&allowed.stderr).trim()
            )));
        }

        let denied = self
            .probe_command("/bin/cat")
            .arg(&self.canary)
            .output()
            .map_err(|error| {
                HarnessError::IsolationUnavailable(format!(
                    "could not verify the macOS Seatbelt deny rule: {error}"
                ))
            })?;
        if denied.status.success() {
            return Err(HarnessError::IsolationUnavailable(
                "macOS Seatbelt did not enforce the host-file deny rule".into(),
            ));
        }

        self.verify_keychain_denial()?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn verify_keychain_denial(&self) -> Result<(), HarnessError> {
        const PASSWORD: &str = "tohseno-isolation-probe";
        const ACCOUNT: &str = "tohseno-isolation-probe";
        const SERVICE: &str = "org.tohseno.isolation-probe";

        let keychain = self
            .canary
            .parent()
            .expect("sandbox canary has a parent")
            .join("isolation-probe.keychain-db");
        let preparation = [
            vec![
                OsString::from("create-keychain"),
                OsString::from("-p"),
                OsString::from(PASSWORD),
                keychain.as_os_str().to_owned(),
            ],
            vec![
                OsString::from("unlock-keychain"),
                OsString::from("-p"),
                OsString::from(PASSWORD),
                keychain.as_os_str().to_owned(),
            ],
            vec![
                OsString::from("add-generic-password"),
                OsString::from("-a"),
                OsString::from(ACCOUNT),
                OsString::from("-s"),
                OsString::from(SERVICE),
                OsString::from("-w"),
                OsString::from("sentinel"),
                OsString::from("-T"),
                OsString::from("/usr/bin/security"),
                keychain.as_os_str().to_owned(),
            ],
        ];
        for arguments in preparation {
            let output = self
                .host_security_command()
                .args(arguments)
                .output()
                .map_err(|error| {
                    HarnessError::IsolationUnavailable(format!(
                        "could not prepare the macOS Keychain isolation probe: {error}"
                    ))
                })?;
            if !output.status.success() {
                let _ = self
                    .host_security_command()
                    .arg("delete-keychain")
                    .arg(&keychain)
                    .status();
                return Err(HarnessError::IsolationUnavailable(format!(
                    "could not prepare the macOS Keychain isolation probe: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                )));
            }
        }

        let probe = self
            .probe_command("/usr/bin/security")
            .args([
                OsString::from("find-generic-password"),
                OsString::from("-a"),
                OsString::from(ACCOUNT),
                OsString::from("-s"),
                OsString::from(SERVICE),
                OsString::from("-w"),
                keychain.as_os_str().to_owned(),
            ])
            .output()
            .map_err(|error| {
                HarnessError::IsolationUnavailable(format!(
                    "could not verify the macOS Keychain deny rule: {error}"
                ))
            });
        let cleanup = self
            .host_security_command()
            .arg("delete-keychain")
            .arg(&keychain)
            .output()
            .map_err(|error| {
                HarnessError::IsolationUnavailable(format!(
                    "could not remove the macOS Keychain isolation probe: {error}"
                ))
            })?;
        if !cleanup.status.success() {
            return Err(HarnessError::IsolationUnavailable(format!(
                "could not remove the macOS Keychain isolation probe: {}",
                String::from_utf8_lossy(&cleanup.stderr).trim()
            )));
        }
        if probe?.status.success() {
            return Err(HarnessError::IsolationUnavailable(
                "macOS Seatbelt did not enforce the Keychain deny rule".into(),
            ));
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn host_security_command(&self) -> StdCommand {
        let mut command = StdCommand::new("/usr/bin/security");
        command
            .env_clear()
            .envs(self.safe_environment())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        command
    }

    #[cfg(target_os = "macos")]
    fn probe_command(&self, program: &str) -> StdCommand {
        let mut command = StdCommand::new(SANDBOX_EXEC);
        command
            .arg("-f")
            .arg(&self.profile)
            .arg(program)
            .current_dir(&self.working_directory)
            .env_clear()
            .envs(self.safe_environment())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        command
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ExecutableRead {
    Literal(PathBuf),
    Subpath(PathBuf),
}

#[cfg(target_os = "macos")]
fn reject_executable_write_overlap(
    executable_reads: &[ExecutableRead],
    writable_roots: &[PathBuf],
) -> Result<(), HarnessError> {
    let overlaps = executable_reads.iter().any(|rule| {
        let readable = match rule {
            ExecutableRead::Literal(path) | ExecutableRead::Subpath(path) => path,
        };
        writable_roots
            .iter()
            .any(|writable| readable.starts_with(writable) || writable.starts_with(readable))
    });
    if overlaps {
        return Err(HarnessError::InvalidCommand(
            "harness executable and support files must be outside writable sandbox storage".into(),
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn resolve_executable(program: &std::ffi::OsStr) -> Result<PathBuf, HarnessError> {
    let requested = Path::new(program);
    let candidate = if requested.is_absolute() || requested.components().count() > 1 {
        requested.to_path_buf()
    } else {
        std::env::var_os("PATH")
            .and_then(|path| {
                std::env::split_paths(&path)
                    .map(|directory| directory.join(requested))
                    .find(|candidate| is_executable(candidate))
            })
            .ok_or_else(|| HarnessError::Missing(program.to_string_lossy().into_owned()))?
    };
    let canonical = fs::canonicalize(&candidate).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            HarnessError::Missing(program.to_string_lossy().into_owned())
        } else {
            HarnessError::Io(error)
        }
    })?;
    if !is_executable(&canonical) {
        return Err(HarnessError::InvalidCommand(format!(
            "harness executable is not a regular executable: {}",
            canonical.display()
        )));
    }
    Ok(canonical)
}

#[cfg(target_os = "macos")]
fn stage_shebang_interpreter(
    executable: &Path,
    tools: &Path,
    reads: &mut Vec<ExecutableRead>,
) -> Result<(), HarnessError> {
    let Some(interpreter_name) = shebang_environment_program(executable)? else {
        return Ok(());
    };
    let interpreter = resolve_executable(interpreter_name.as_ref())?;
    let staged = tools.join(&interpreter_name);
    std::os::unix::fs::symlink(&interpreter, &staged)?;
    reads.push(ExecutableRead::Literal(interpreter.clone()));
    for root in executable_support_roots(&interpreter) {
        reads.push(ExecutableRead::Subpath(root));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn shebang_environment_program(executable: &Path) -> Result<Option<OsString>, HarnessError> {
    let mut file = File::open(executable)?;
    let mut bytes = [0_u8; 512];
    let count = file.read(&mut bytes)?;
    let Some(line) = std::str::from_utf8(&bytes[..count])
        .ok()
        .and_then(|contents| contents.lines().next())
    else {
        return Ok(None);
    };
    let Some(shebang) = line.strip_prefix("#!") else {
        return Ok(None);
    };
    let words = shebang.split_whitespace().collect::<Vec<_>>();
    if words.first().copied() != Some("/usr/bin/env") {
        return Ok(None);
    }
    let mut index = 1;
    if words.get(index).copied() == Some("-S") {
        index += 1;
    }
    let Some(program) = words.get(index) else {
        return Err(HarnessError::InvalidCommand(
            "harness shebang does not name an interpreter".into(),
        ));
    };
    if program.starts_with('-') || program.contains('/') {
        return Err(HarnessError::InvalidCommand(
            "harness shebang uses an unsupported environment interpreter".into(),
        ));
    }
    Ok(Some((*program).into()))
}

#[cfg(target_os = "macos")]
fn executable_support_roots(executable: &Path) -> Vec<PathBuf> {
    let components = executable.components().collect::<Vec<_>>();
    let mut roots = Vec::new();
    for (index, component) in components.iter().enumerate() {
        let value = component.as_os_str().to_string_lossy();
        if value == "node_modules" {
            let Some(package) = components.get(index + 1) else {
                continue;
            };
            let package_value = package.as_os_str().to_string_lossy();
            let package_end = if package_value.starts_with('@') {
                index + 2
            } else {
                index + 1
            };
            if package_end < components.len() {
                let mut root = PathBuf::new();
                for component in &components[..=package_end] {
                    root.push(component.as_os_str());
                }
                roots.push(root);
            }
            break;
        }
        if value.ends_with(".app") {
            let mut root = PathBuf::new();
            for component in &components[..=index] {
                root.push(component.as_os_str());
            }
            roots.push(root);
            break;
        }
    }
    roots
}

#[cfg(target_os = "macos")]
fn seatbelt_profile(
    home: &Path,
    temporary: &Path,
    tools: &Path,
    shot: Option<&Path>,
    executable_reads: &[ExecutableRead],
) -> Result<String, HarnessError> {
    let mut metadata_paths = vec![
        home.to_path_buf(),
        temporary.to_path_buf(),
        tools.to_path_buf(),
    ];
    let mut read_rules = vec![
        ExecutableRead::Subpath(PathBuf::from("/bin")),
        ExecutableRead::Subpath(PathBuf::from("/sbin")),
        ExecutableRead::Subpath(PathBuf::from("/usr/bin")),
        ExecutableRead::Subpath(PathBuf::from("/usr/sbin")),
        ExecutableRead::Subpath(PathBuf::from("/Library/Developer")),
        ExecutableRead::Subpath(PathBuf::from("/private/var/select")),
        ExecutableRead::Subpath(PathBuf::from("/private/etc/ssl")),
        ExecutableRead::Literal(PathBuf::from("/private/var/db/xcode_select_link")),
        ExecutableRead::Literal(PathBuf::from("/private/etc/hosts")),
        ExecutableRead::Literal(PathBuf::from("/private/etc/resolv.conf")),
        ExecutableRead::Literal(PathBuf::from("/private/var/run/resolv.conf")),
        ExecutableRead::Subpath(home.to_path_buf()),
        ExecutableRead::Subpath(temporary.to_path_buf()),
        ExecutableRead::Subpath(tools.to_path_buf()),
    ];
    if let Some(developer_root) = selected_developer_root() {
        metadata_paths.push(developer_root.clone());
        read_rules.push(ExecutableRead::Subpath(developer_root));
    }
    if let Some(shot) = shot {
        metadata_paths.push(shot.to_path_buf());
        read_rules.push(ExecutableRead::Subpath(shot.to_path_buf()));
    }
    for rule in executable_reads {
        let path = match rule {
            ExecutableRead::Literal(path) | ExecutableRead::Subpath(path) => path,
        };
        metadata_paths.push(path.clone());
        read_rules.push(rule.clone());
    }
    metadata_paths.sort();
    metadata_paths.dedup();
    read_rules.sort();
    read_rules.dedup();

    let mut profile = String::from(
        "(version 1)\n\
         (deny default)\n\
         (import \"system.sb\")\n\n\
         ;; Imported system rules admit a few local services. Override them so\n\
         ;; the harness can neither reach Keychain nor use local IPC as an\n\
         ;; escape hatch. DNS is the sole Unix-domain socket exception below.\n\
         (deny mach-lookup\n\
           (global-name \"com.apple.SecurityServer\")\n\
           (global-name \"com.apple.securityd\")\n\
           (global-name-prefix \"com.apple.securityd.\")\n\
           (local-name \"com.apple.SecurityServer\")\n\
           (local-name \"com.apple.securityd\")\n\
           (local-name \"com.apple.cfprefsd.agent\")\n\
           (global-name \"com.apple.cfprefsd.agent\")\n\
           (global-name \"com.apple.cfprefsd.daemon\")\n\
           (xpc-service-name \"com.apple.SecurityServer\")\n\
           (xpc-service-name \"com.apple.securityd\")\n\
           (xpc-service-name \"com.apple.securityd.xpc\")\n\
           (xpc-service-name-prefix \"com.apple.securityd.\")\n\
           (xpc-service-name-prefix \"\"))\n\
         (deny ipc-posix-shm*)\n\
         (deny network-inbound)\n\
         (deny network-outbound\n\
           (remote ip \"localhost:*\")\n\
           (literal \"/private/var/run/syslog\")\n\
           (literal \"/private/var/run/asl_input\")\n\
           (literal \"/private/var/run/systemkeychaincheck.socket\")\n\
           (literal \"/private/var/run/usbmuxd\")\n\
           (literal \"/private/var/run/cupsd\"))\n\
         (deny file-read* file-write*\n\
           (subpath \"/Library/Keychains\")\n\
           (subpath \"/private/var/root/Library/Keychains\"))\n\n\
         (allow process-fork process-exec)\n\
         (allow network-outbound\n\
           (require-all\n\
             (require-any (remote tcp \"*:443\") (remote udp \"*:443\"))\n\
             (require-not (remote ip \"localhost:*\"))))\n\
         (allow network-outbound (literal \"/private/var/run/mDNSResponder\"))\n\n\
         (allow file-read-metadata file-test-existence\n",
    );
    for path in &metadata_paths {
        profile.push_str(&format!("  (path-ancestors {})\n", seatbelt_string(path)?));
    }
    profile.push_str(")\n\n(allow file-read* file-test-existence file-map-executable\n");
    for rule in &read_rules {
        match rule {
            ExecutableRead::Literal(path) => {
                profile.push_str(&format!("  (literal {})\n", seatbelt_string(path)?));
            }
            ExecutableRead::Subpath(path) => {
                profile.push_str(&format!("  (subpath {})\n", seatbelt_string(path)?));
            }
        }
    }
    profile.push_str(")\n\n(allow file-write*\n");
    for path in [home, temporary] {
        profile.push_str(&format!("  (subpath {})\n", seatbelt_string(path)?));
    }
    if let Some(shot) = shot {
        profile.push_str(&format!(
            "  (subpath {})\n",
            seatbelt_string(&shot.join("src"))?
        ));
    }
    profile.push_str(")\n");
    Ok(profile)
}

#[cfg(target_os = "macos")]
fn selected_developer_root() -> Option<PathBuf> {
    let selected = Path::new("/private/var/db/xcode_select_link");
    let canonical = fs::canonicalize(selected).ok()?;
    let components = canonical.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        if component.as_os_str().to_string_lossy().ends_with(".app") {
            let mut app = PathBuf::new();
            for component in &components[..=index] {
                app.push(component.as_os_str());
            }
            return Some(app);
        }
    }
    Some(canonical)
}

#[cfg(target_os = "macos")]
fn seatbelt_string(path: &Path) -> Result<String, HarnessError> {
    let value = path.to_str().ok_or_else(|| {
        HarnessError::IsolationUnavailable(format!(
            "sandbox path is not valid UTF-8: {}",
            path.display()
        ))
    })?;
    if value.chars().any(char::is_control) {
        return Err(HarnessError::IsolationUnavailable(
            "sandbox path contains a control character".into(),
        ));
    }
    Ok(format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

pub fn discover_harnesses(selected: &HarnessConfig) -> Vec<HarnessOption> {
    let selected_id = split_command(&selected.command)
        .ok()
        .and_then(|words| words.first().cloned())
        .and_then(|program| known_harness_for_program(&program).map(|known| known.id));
    KNOWN_HARNESSES
        .iter()
        .map(|known| {
            let executable = find_executable(known);
            HarnessOption {
                id: known.id.into(),
                label: known.label.into(),
                command: executable
                    .as_ref()
                    .map(|path| command_for_path(path))
                    .unwrap_or_else(|| known.binaries[0].into()),
                installed: executable.is_some(),
                selected: selected_id == Some(known.id),
            }
        })
        .collect()
}

pub fn selected_harness(id: &str) -> Result<HarnessConfig, HarnessError> {
    let known = KNOWN_HARNESSES
        .iter()
        .find(|known| known.id.eq_ignore_ascii_case(id))
        .ok_or_else(|| HarnessError::Unsupported(id.into()))?;
    let path =
        find_executable(known).ok_or_else(|| HarnessError::Missing(known.binaries[0].into()))?;
    let command = command_for_path(&path);
    Ok(HarnessConfig { command })
}

pub(crate) fn command_for_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if value
        .chars()
        .any(|character| character.is_whitespace() || matches!(character, '\\' | '"'))
    {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.into_owned()
    }
}

fn find_executable(known: &KnownHarness) -> Option<PathBuf> {
    let from_path = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .flat_map(|directory| {
                known
                    .binaries
                    .iter()
                    .map(move |binary| directory.join(binary))
            })
            .find(|candidate| is_executable(candidate))
    });
    if from_path.is_some() {
        return from_path;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    known
        .home_paths
        .iter()
        .map(|relative| home.join(relative))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn known_harness_for_program(program: &std::ffi::OsStr) -> Option<&'static KnownHarness> {
    let name = Path::new(program).file_name()?.to_string_lossy();
    KNOWN_HARNESSES.iter().find(|known| {
        known
            .binaries
            .iter()
            .any(|binary| name.eq_ignore_ascii_case(binary))
    })
}

#[derive(Clone, Debug)]
pub struct Harness {
    config: HarnessConfig,
    events: EventBus,
}

impl Harness {
    pub fn new(config: HarnessConfig, events: EventBus) -> Self {
        Self { config, events }
    }

    pub async fn wait_until_available(&self) -> Result<HarnessMode, HarnessError> {
        let mut announced = false;
        loop {
            match self.detect_mode().await {
                Ok(mode) => return Ok(mode),
                Err(HarnessError::Missing(_)) => {
                    if !announced {
                        self.events
                            .emit(Event::handoff(install_handoff(&self.config.command)));
                        announced = true;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub async fn detect_mode(&self) -> Result<HarnessMode, HarnessError> {
        let words = split_command(&self.config.command)?;
        let (program, configured_args) = words.split_first().ok_or(HarnessError::EmptyCommand)?;
        validate_configured_args(configured_args)?;
        let help = command_help(program, configured_args, &[]).await?;
        let known = known_harness_for_program(program);
        match known.map(|known| known.id) {
            Some("claude")
                if help.contains("--print")
                    && help.contains("stream-json")
                    && help.contains("--permission-mode")
                    && help.contains("--allowedTools")
                    && help.contains("--no-session-persistence") =>
            {
                Ok(HarnessMode::ClaudeStreamJson)
            }
            Some("codex") => {
                let exec_help = command_help(program, configured_args, &["exec"]).await?;
                if [
                    "--json",
                    "--sandbox",
                    "--skip-git-repo-check",
                    "--ephemeral",
                    "--ignore-user-config",
                    "--ignore-rules",
                ]
                .iter()
                .all(|flag| exec_help.contains(flag))
                {
                    Ok(HarnessMode::CodexJson)
                } else {
                    Err(HarnessError::Incompatible("Codex".into()))
                }
            }
            Some("grok")
                if ["--single", "--output-format", "--permission-mode"]
                    .iter()
                    .all(|flag| help.contains(flag)) =>
            {
                Ok(HarnessMode::GrokPlain)
            }
            Some("hermes") => {
                let chat_help = command_help(program, configured_args, &["chat"]).await?;
                if chat_help.contains("-q") || chat_help.contains("--query") {
                    Ok(HarnessMode::HermesPlain)
                } else {
                    Err(HarnessError::Incompatible("Hermes Agent".into()))
                }
            }
            Some("opencode") => {
                let run_help = command_help(program, configured_args, &["run"]).await?;
                if help.contains("run") && !run_help.is_empty() {
                    Ok(HarnessMode::OpenCodePlain)
                } else {
                    Err(HarnessError::Incompatible("OpenCode".into()))
                }
            }
            Some(known) => Err(HarnessError::Incompatible(known.into())),
            None if help.contains("--print")
                && help.contains("stream-json")
                && help.contains("--permission-mode") =>
            {
                Ok(HarnessMode::ClaudeStreamJson)
            }
            None => Ok(HarnessMode::Generic),
        }
    }

    pub async fn run(
        &self,
        ledger: &Ledger,
        shot: &Shot,
        mode: HarnessMode,
        instruction: &str,
    ) -> Result<(), HarnessError> {
        let words = split_command(&self.config.command)?;
        let (program, configured_args) = words.split_first().ok_or(HarnessError::EmptyCommand)?;
        validate_configured_args(configured_args)?;
        let sandbox = HarnessSandbox::new(program, Some(&shot.path))?;
        let mut command = sandbox.command();
        command
            .args(configured_args)
            .args(harness_arguments(mode, instruction))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command.spawn().map_err(|error| {
            HarnessError::IsolationUnavailable(format!(
                "could not start the macOS Seatbelt launcher: {error}"
            ))
        })?;
        let stdout = child.stdout.take().ok_or(HarnessError::PipeMissing)?;
        let stderr = child.stderr.take().ok_or(HarnessError::PipeMissing)?;
        let (sender, mut receiver) = mpsc::channel::<(bool, String)>(128);
        let stdout_sender = sender.clone();
        let stdout_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Some(line) = lines.next_line().await? {
                if stdout_sender.send((false, line)).await.is_err() {
                    break;
                }
            }
            Ok::<(), std::io::Error>(())
        });
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Some(line) = lines.next_line().await? {
                if sender.send((true, line)).await.is_err() {
                    break;
                }
            }
            Ok::<(), std::io::Error>(())
        });

        while let Some((is_stderr, line)) = receiver.recv().await {
            ledger
                .append_shot_log(
                    shot,
                    "harness.log",
                    format!("{}{line}\n", if is_stderr { "[stderr] " } else { "" }).as_bytes(),
                )
                .map_err(HarnessError::Ledger)?;
            if let Some(display) = display_line(mode, &line, is_stderr) {
                for physical_line in display.lines() {
                    if !physical_line.trim().is_empty() {
                        self.events.emit(Event::harness_line(physical_line));
                    }
                }
            }
        }
        stdout_task.await.map_err(HarnessError::Join)??;
        stderr_task.await.map_err(HarnessError::Join)??;
        let status = child.wait().await.map_err(HarnessError::Io)?;
        if status.success() {
            Ok(())
        } else {
            Err(HarnessError::Exit(status.code()))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HarnessMode {
    ClaudeStreamJson,
    CodexJson,
    GrokPlain,
    HermesPlain,
    OpenCodePlain,
    Generic,
}

fn harness_arguments(mode: HarnessMode, instruction: &str) -> Vec<OsString> {
    let arguments: Vec<&str> = match mode {
        HarnessMode::ClaudeStreamJson => vec![
            "--print",
            "--output-format",
            "stream-json",
            "--verbose",
            "--permission-mode",
            "acceptEdits",
            "--tools",
            "Bash,Edit,Read,Write,Glob,Grep",
            "--allowedTools",
            "Bash,Edit,Read,Write,Glob,Grep",
            "--strict-mcp-config",
            "--mcp-config",
            r#"{"mcpServers":{}}"#,
            "--setting-sources",
            "",
            "--no-session-persistence",
            instruction,
        ],
        HarnessMode::CodexJson => vec![
            "exec",
            "--json",
            "--sandbox",
            "workspace-write",
            "--skip-git-repo-check",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
            instruction,
        ],
        HarnessMode::GrokPlain => vec![
            "--single",
            instruction,
            "--output-format",
            "plain",
            "--permission-mode",
            "acceptEdits",
            "--no-memory",
            "--no-subagents",
            "--verbatim",
        ],
        HarnessMode::HermesPlain => vec!["chat", "-q", instruction],
        HarnessMode::OpenCodePlain => vec!["run", instruction],
        HarnessMode::Generic => vec![instruction],
    };
    arguments.into_iter().map(OsString::from).collect()
}

fn display_line(mode: HarnessMode, line: &str, is_stderr: bool) -> Option<String> {
    if is_stderr
        || matches!(
            mode,
            HarnessMode::Generic
                | HarnessMode::GrokPlain
                | HarnessMode::HermesPlain
                | HarnessMode::OpenCodePlain
        )
    {
        return Some(line.to_owned());
    }
    let value: Value = serde_json::from_str(line).ok()?;
    if mode == HarnessMode::CodexJson {
        return display_codex_line(&value);
    }
    match value.get("type").and_then(Value::as_str) {
        Some("assistant") => {
            let content = value
                .pointer("/message/content")
                .and_then(Value::as_array)?;
            let output = content
                .iter()
                .filter_map(|block| match block.get("type").and_then(Value::as_str) {
                    Some("text") => block.get("text").and_then(Value::as_str).map(str::to_owned),
                    Some("tool_use") => block
                        .get("name")
                        .and_then(Value::as_str)
                        .map(|name| format!("using {name}…")),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            (!output.is_empty()).then_some(output)
        }
        Some("content_block_delta") => value
            .pointer("/delta/text")
            .and_then(Value::as_str)
            .map(str::to_owned),
        Some("result") => value
            .get("result")
            .and_then(Value::as_str)
            .filter(|result| !result.is_empty())
            .map(str::to_owned),
        _ => None,
    }
}

fn display_codex_line(value: &Value) -> Option<String> {
    match value.get("type").and_then(Value::as_str) {
        Some("item.completed") | Some("item.started") => {
            let item = value.get("item")?;
            match item.get("type").and_then(Value::as_str) {
                Some("agent_message") => item.get("text")?.as_str().map(str::to_owned),
                Some("command_execution") => item
                    .get("command")
                    .and_then(Value::as_str)
                    .map(|command| format!("running {command}…")),
                Some("mcp_tool_call") => {
                    let server = item.get("server").and_then(Value::as_str).unwrap_or("tool");
                    let tool = item.get("tool").and_then(Value::as_str).unwrap_or("call");
                    Some(format!("using {server}.{tool}…"))
                }
                Some("file_change") => Some("writing source…".into()),
                _ => None,
            }
        }
        Some("error") => value
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_owned),
        _ => None,
    }
}

async fn command_help(
    program: &std::ffi::OsStr,
    configured_args: &[OsString],
    subcommand: &[&str],
) -> Result<String, HarnessError> {
    validate_configured_args(configured_args)?;
    let sandbox = HarnessSandbox::new(program, None)?;
    let output = sandbox
        .command()
        .args(configured_args)
        .args(subcommand)
        .arg("--help")
        .output()
        .await
        .map_err(HarnessError::Io)?;
    Ok(format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn install_handoff(command: &str) -> String {
    let program = split_command(command)
        .ok()
        .and_then(|words| words.first().cloned());
    match program
        .as_deref()
        .and_then(known_harness_for_program)
        .map(|known| known.id)
    {
        Some("claude") => {
            "Run `curl -fsSL https://claude.ai/install.sh | bash`, then return here.".into()
        }
        Some("codex") => "Install Codex, then return here.".into(),
        Some("grok") => "Install Grok Build, then return here.".into(),
        Some("hermes") => "Install Hermes Agent, then return here.".into(),
        Some("opencode") => "Install OpenCode, then return here.".into(),
        _ => "Install the configured coding agent, then return here.".into(),
    }
}

fn split_command(command: &str) -> Result<Vec<OsString>, HarnessError> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in command.chars() {
        if escaped {
            current.push(character);
            escaped = false;
        } else if character == '\\' && quote != Some('\'') {
            escaped = true;
        } else if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            } else {
                current.push(character);
            }
        } else if character == '\'' || character == '"' {
            quote = Some(character);
        } else if character.is_whitespace() {
            if !current.is_empty() {
                words.push(OsString::from(std::mem::take(&mut current)));
            }
        } else {
            current.push(character);
        }
    }
    if escaped {
        current.push('\\');
    }
    if quote.is_some() {
        return Err(HarnessError::InvalidCommand(
            "unterminated quote in harness.command".into(),
        ));
    }
    if !current.is_empty() {
        words.push(OsString::from(current));
    }
    Ok(words)
}

fn validate_configured_args(arguments: &[OsString]) -> Result<(), HarnessError> {
    for (index, argument) in arguments.iter().enumerate() {
        let value = argument.to_string_lossy();
        let normalized = value.to_ascii_lowercase();
        let option = normalized.split('=').next().unwrap_or(&normalized);
        let option_loads_external_state = option == "-c"
            || matches!(
                option,
                "--tools"
                    | "--allowedtools"
                    | "--disallowedtools"
                    | "--enable"
                    | "--feature"
                    | "--env"
                    | "--env-file"
            )
            || [
                "config",
                "setting",
                "profile",
                "plugin",
                "extension",
                "mcp",
                "connector",
                "integration",
            ]
            .iter()
            .any(|keyword| option.contains(keyword));

        if normalized.contains("dangerously")
            || normalized.contains("bypasspermissions")
            || normalized.contains("danger-full-access")
            || matches!(
                option,
                "--always-approve" | "--auto" | "--full-auto" | "--allow-all"
            )
        {
            return Err(HarnessError::InvalidCommand(
                "harness.command may not disable permissions or sandboxing".into(),
            ));
        }
        if matches!(
            option,
            "--api-key"
                | "--apikey"
                | "--token"
                | "--auth-token"
                | "--access-token"
                | "--password"
                | "--secret"
                | "--client-secret"
                | "--credential"
                | "--credentials"
                | "--authorization"
                | "--header"
                | "--cookie"
                | "--private-key"
                | "--ssh-key"
        ) || ["sk-", "xoxb-", "xoxp-", "ghp_", "github_pat_", "bearer "]
            .iter()
            .any(|prefix| normalized.starts_with(prefix))
        {
            return Err(HarnessError::InvalidCommand(
                "harness.command may not contain credential-bearing arguments".into(),
            ));
        }
        if option_loads_external_state {
            return Err(HarnessError::InvalidCommand(
                "harness.command may not load settings, plugins, tools, or integrations".into(),
            ));
        }
        if let Some((name, _)) = value.split_once('=') {
            if !name.starts_with('-')
                && !name.is_empty()
                && name
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
            {
                return Err(HarnessError::InvalidCommand(
                    "harness.command may not inject environment assignments".into(),
                ));
            }
        }
        let path_value = value
            .split_once('=')
            .map_or(value.as_ref(), |(_, value)| value);
        let path_value_lower = path_value.to_ascii_lowercase();
        if Path::new(path_value).is_absolute()
            || path_value.starts_with("~/")
            || path_value.starts_with("./")
            || path_value.starts_with("../")
            || path_value.split(['/', '\\']).any(|part| part == "..")
            || path_value.starts_with('@')
            || path_value_lower.starts_with("file:")
        {
            return Err(HarnessError::InvalidCommand(
                "harness.command arguments may not reference host files".into(),
            ));
        }

        let next = arguments
            .get(index + 1)
            .map(|next| next.to_string_lossy().to_ascii_lowercase());
        let paired_bypass = (matches!(option, "--permission-mode")
            && next.as_deref().is_some_and(|mode| mode.contains("bypass")))
            || (matches!(option, "--sandbox") && next.as_deref() == Some("danger-full-access"))
            || (matches!(option, "--ask-for-approval" | "-a") && next.as_deref() == Some("never"));
        if paired_bypass {
            return Err(HarnessError::InvalidCommand(
                "harness.command may not disable permissions or sandboxing".into(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum HarnessError {
    Io(std::io::Error),
    Ledger(LedgerError),
    Missing(String),
    Unsupported(String),
    Incompatible(String),
    IsolationUnavailable(String),
    EmptyCommand,
    InvalidCommand(String),
    PipeMissing,
    Exit(Option<i32>),
    Join(tokio::task::JoinError),
}

impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Ledger(error) => write!(f, "{error}"),
            Self::Missing(command) => write!(f, "harness not found: {command}"),
            Self::Unsupported(harness) => write!(f, "unsupported coding agent: {harness}"),
            Self::Incompatible(harness) => {
                write!(
                    f,
                    "{harness} does not expose the required non-interactive flags"
                )
            }
            Self::IsolationUnavailable(message) => {
                write!(f, "coding harness isolation unavailable: {message}")
            }
            Self::EmptyCommand => write!(f, "harness.command is empty"),
            Self::InvalidCommand(message) => write!(f, "{message}"),
            Self::PipeMissing => write!(f, "harness output pipe is unavailable"),
            Self::Exit(code) => write!(
                f,
                "the coding agent exited unsuccessfully ({code:?}). Inside TOHSENO's \
                 isolated sandbox an agent has no provider sign-in yet; pass \
                 --harness /absolute/path/to/your-agent to bring your own, or see \
                 docs/adr/0002-harness-credential-broker.md"
            ),
            Self::Join(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for HarnessError {}

impl From<std::io::Error> for HarnessError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn generic_commands_may_contain_quoted_arguments() {
        assert_eq!(
            split_command("agent --profile 'ios press'").unwrap(),
            ["agent", "--profile", "ios press"]
                .into_iter()
                .map(OsString::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn configured_arguments_reject_credentials_and_permission_bypasses() {
        for command in [
            "agent --api-key sk-example",
            "agent OPENAI_API_KEY=sk-example",
            "agent --dangerously-skip-permissions",
            "agent --always-approve",
            "agent --sandbox danger-full-access",
            "agent --permission-mode bypassPermissions",
            "agent --ask-for-approval never",
            "agent --config /Users/example/.agent/config.toml",
            "agent --settings={}",
            "agent --profile ios-builder",
            "agent --plugin source-control",
            "agent --mcp-config mcp.json",
            "agent --tools Read,Bash",
            "agent --feature remote-integrations",
            "agent --model @/Users/example/model",
            "agent --model ../../host-file",
        ] {
            let words = split_command(command).unwrap();
            assert!(validate_configured_args(&words[1..]).is_err(), "{command}");
        }
        let safe = split_command("agent --model sonnet --verbose").unwrap();
        validate_configured_args(&safe[1..]).unwrap();
    }

    #[test]
    fn executable_paths_with_spaces_remain_one_command_word() {
        let command = command_for_path(Path::new("/Users/App Maker/bin/codex"));
        assert_eq!(
            split_command(&command).unwrap(),
            [OsString::from("/Users/App Maker/bin/codex")]
        );
    }

    #[test]
    fn parses_claude_text_without_showing_json_protocol() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Writing the app."},{"type":"tool_use","name":"Write"}]}}"#;
        assert_eq!(
            display_line(HarnessMode::ClaudeStreamJson, line, false).unwrap(),
            "Writing the app.\nusing Write…"
        );
    }

    #[test]
    fn supported_agents_have_stable_public_ids() {
        assert_eq!(
            KNOWN_HARNESSES
                .iter()
                .map(|known| known.id)
                .collect::<Vec<_>>(),
            ["hermes", "codex", "claude", "grok", "opencode"]
        );
    }

    #[test]
    fn parses_codex_events_without_showing_json_protocol() {
        let line = r#"{"type":"item.completed","item":{"type":"agent_message","text":"The project builds."}}"#;
        assert_eq!(
            display_line(HarnessMode::CodexJson, line, false).unwrap(),
            "The project builds."
        );
    }

    #[test]
    fn noninteractive_arguments_never_disable_permissions_or_sandboxing() {
        for mode in [
            HarnessMode::ClaudeStreamJson,
            HarnessMode::CodexJson,
            HarnessMode::GrokPlain,
            HarnessMode::HermesPlain,
            HarnessMode::OpenCodePlain,
            HarnessMode::Generic,
        ] {
            let arguments = harness_arguments(mode, "build");
            let rendered = arguments
                .iter()
                .map(|argument| argument.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            for forbidden in [
                "dangerously",
                "bypassPermissions",
                "always-approve",
                "--auto",
            ] {
                assert!(!rendered.contains(forbidden), "{mode:?}: {rendered}");
            }
        }

        let codex = harness_arguments(HarnessMode::CodexJson, "build");
        assert!(codex
            .windows(2)
            .any(|pair| pair == ["--sandbox", "workspace-write"]));
        let claude = harness_arguments(HarnessMode::ClaudeStreamJson, "build");
        assert!(claude
            .windows(2)
            .any(|pair| pair == ["--permission-mode", "acceptEdits"]));
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn seatbelt_exposes_only_staged_shot_input_and_source_output() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(directory.path().join("ledger"));
        ledger
            .create_app("press", "org.tohseno.genesis.test.press")
            .unwrap();
        let shot = ledger.reserve_shot("press", None).unwrap();
        ledger
            .write_shot_file(&shot, "TASK.md", b"staged input\n")
            .unwrap();
        let outside = directory.path().join("outside-secret");
        fs::write(&outside, b"host secret\n").unwrap();
        let agent = directory.path().join("fake-agent.sh");
        fs::write(
            &agent,
            format!(
                r#"#!/bin/sh
set -eu
if /bin/cat {} >/dev/null 2>&1; then
  exit 41
fi
if printf 'changed\n' > TASK.md 2>/dev/null; then
  exit 42
fi
printf '%s\n' "$1" > src/result.txt
printf '%s\n' "isolated write complete"
"#,
                command_for_path(&outside)
            ),
        )
        .unwrap();
        fs::set_permissions(&agent, fs::Permissions::from_mode(0o700)).unwrap();
        let config = HarnessConfig {
            command: command_for_path(&agent),
        };
        Harness::new(config, EventBus::default())
            .run(&ledger, &shot, HarnessMode::Generic, "expected output")
            .await
            .unwrap();

        assert_eq!(
            fs::read_to_string(shot.source_path().join("result.txt")).unwrap(),
            "expected output\n"
        );
        assert_eq!(
            fs::read_to_string(shot.path.join("TASK.md")).unwrap(),
            "staged input\n"
        );
        assert_eq!(fs::read_to_string(outside).unwrap(), "host secret\n");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn writable_shot_source_cannot_supply_the_harness_executable() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(directory.path().join("ledger"));
        ledger
            .create_app("press", "org.tohseno.genesis.test.press")
            .unwrap();
        let shot = ledger.reserve_shot("press", None).unwrap();
        let agent = shot.source_path().join("agent");
        fs::write(&agent, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&agent, fs::Permissions::from_mode(0o700)).unwrap();

        assert!(matches!(
            HarnessSandbox::new(agent.as_os_str(), Some(&shot.path)),
            Err(HarnessError::InvalidCommand(_))
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn sandbox_environment_is_an_explicit_nonsecret_allowlist() {
        let sandbox = HarnessSandbox::new("/usr/bin/true".as_ref(), None).unwrap();
        let environment = sandbox
            .safe_environment()
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>();
        for forbidden in [
            OsString::from("SSH_AUTH_SOCK"),
            OsString::from("AWS_SECRET_ACCESS_KEY"),
            OsString::from("OPENAI_API_KEY"),
            OsString::from("ANTHROPIC_API_KEY"),
            OsString::from("TOHSENO_DATA_ROOT"),
        ] {
            assert!(!environment.contains(&forbidden));
        }
        let profile = fs::read_to_string(&sandbox.profile).unwrap();
        assert!(profile.contains(r#"(remote tcp "*:443")"#));
        assert!(profile.contains(r#"(remote udp "*:443")"#));
        assert!(profile.contains(r#"(require-not (remote ip "localhost:*"))"#));
        assert!(!profile.contains(r#"(remote tcp "*:*")"#));
        assert!(!profile.contains(r#"(remote udp "*:*")"#));
        assert!(profile.contains(r#"(global-name "com.apple.SecurityServer")"#));
        assert!(profile.contains(r#"(global-name-prefix "com.apple.securityd.")"#));
        assert!(profile.contains(r#"(xpc-service-name-prefix "")"#));
        assert!(profile.contains("(deny ipc-posix-shm*)"));
        assert!(profile.contains("(deny network-inbound)"));
        assert!(profile.contains(r#"(literal "/private/var/run/syslog")"#));
        assert!(profile.contains(r#"(literal "/private/var/run/systemkeychaincheck.socket")"#));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn unsupported_hosts_fail_before_starting_a_harness() {
        assert!(matches!(
            HarnessSandbox::new("agent".as_ref(), None),
            Err(HarnessError::IsolationUnavailable(_))
        ));
    }

    #[tokio::test]
    async fn installed_known_agents_advertise_supported_contracts() {
        for option in discover_harnesses(&HarnessConfig::default())
            .into_iter()
            .filter(|option| option.installed)
        {
            Harness::new(
                HarnessConfig {
                    command: option.command,
                },
                EventBus::default(),
            )
            .detect_mode()
            .await
            .unwrap_or_else(|error| panic!("{}: {error}", option.label));
        }
    }
}
