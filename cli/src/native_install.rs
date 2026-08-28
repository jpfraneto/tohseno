//! First-open installation of the bundled core from a verified Tohseno.app.
//! Program files move through the existing installer-owned release boundary;
//! workspace, app, command, identity, entitlement, and Companion state are
//! never copied, rewritten, or removed here.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{symlink, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use crate::service_commands::{self, ServicePaths, SystemLaunchctl};

const MANIFEST: &str = "FILES.sha256";
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_RELEASE_FILES: usize = 20_000;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub fn install_bundled_core_if_present() -> Result<(), BoxError> {
    let Some(source) = bundled_release_root()? else {
        return Ok(());
    };
    let expected = verify_release(&source)?;
    let manifest_digest = digest_file(&source.join(MANIFEST))?;
    let release_name = format!(
        "native-{}-{}",
        env!("CARGO_PKG_VERSION"),
        &manifest_digest[..16]
    );
    let paths = ServicePaths::discover().map_err(|error| error.to_string())?;
    ensure_private_directory(&paths.install_root)?;
    ensure_private_directory(&paths.install_root.join("releases"))?;
    ensure_private_directory(&paths.install_root.join("bin"))?;
    let _activation_lock = acquire_activation_lock(&paths.install_root)?;
    let release = paths.install_root.join("releases").join(&release_name);
    if release.exists() {
        verify_release(&release)?;
    } else {
        stage_release(&source, &release, &release_name, &expected)?;
        verify_release(&release)?;
    }

    let old_current = current_release_target(&paths.install_root)?;
    let old_launcher = old_current
        .as_deref()
        .map(|target| paths.install_root.join(target).join("bin/tohseno"))
        .filter(|path| path.is_file());
    let old_identity_helper = old_current
        .as_deref()
        .map(|target| {
            paths
                .install_root
                .join(target)
                .join("bin/tohseno-apple-identity")
        })
        .filter(|path| path.is_file());
    if old_current.is_some() && (old_launcher.is_none() || old_identity_helper.is_none()) {
        return Err(
            "the selected installed release is incomplete; no program files were changed".into(),
        );
    }
    let stable_identity = paths.install_root.join("bin/tohseno-apple-identity");
    let launcher_existed = regular_destination_exists(&paths.launcher)?;
    let identity_existed = regular_destination_exists(&stable_identity)?;
    if old_current.is_none() && (launcher_existed || identity_existed) {
        return Err("stable factory programs exist without an installer-owned current release; no files were changed".into());
    }
    let desired_current = format!("releases/{release_name}");
    let new_launcher = release.join("bin/tohseno");
    let new_identity_helper = release.join("bin/tohseno-apple-identity");
    if old_current.as_deref() == Some(desired_current.as_str())
        && launcher_existed
        && identity_existed
        && paths.launch_agent.exists()
        && digest_file(&paths.launcher)? == digest_file(&new_launcher)?
        && digest_file(&stable_identity)? == digest_file(&new_identity_helper)?
    {
        return Ok(());
    }
    let had_launch_agent = paths.launch_agent.exists();
    if had_launch_agent {
        // An unrecognized or symlinked agent fails closed inside stop().
        service_commands::stop(&paths, &SystemLaunchctl).map_err(|error| error.to_string())?;
    }
    let activation = (|| -> Result<(), BoxError> {
        publish_regular(&new_launcher, &paths.launcher)?;
        publish_regular(&new_identity_helper, &stable_identity)?;
        publish_current(&paths.install_root, &desired_current)?;
        service_commands::install(&paths, &SystemLaunchctl).map_err(|error| error.to_string())?;
        Ok(())
    })();
    if let Err(error) = activation {
        restore_program_selection(
            &paths,
            old_current.as_deref(),
            old_launcher.as_deref(),
            old_identity_helper.as_deref(),
            launcher_existed,
            identity_existed,
        )?;
        if had_launch_agent {
            service_commands::start(&paths, &SystemLaunchctl)
                .map_err(|restart| format!("activation failed ({error}); the prior programs were restored but its service could not be restarted: {restart}"))?;
        }
        return Err(format!("the bundled Local Workspace Service could not be activated; the prior program selection was restored: {error}").into());
    }
    Ok(())
}

fn acquire_activation_lock(install_root: &Path) -> Result<File, BoxError> {
    let lock_path = install_root.join(".native-activation.lock");
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let file = options.open(&lock_path)?;
    if !file.metadata()?.is_file() {
        return Err("native activation lock is not a regular file".into());
    }
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    loop {
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
            return Ok(file);
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(error.into());
        }
    }
}

fn regular_destination_exists(path: &Path) -> Result<bool, BoxError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(format!("{} is not an installer-owned regular file", path.display()).into())
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn restore_program_selection(
    paths: &ServicePaths,
    old_current: Option<&str>,
    old_launcher: Option<&Path>,
    old_identity: Option<&Path>,
    launcher_existed: bool,
    identity_existed: bool,
) -> Result<(), BoxError> {
    if let Some(source) = old_launcher {
        publish_regular(source, &paths.launcher)?;
    } else if !launcher_existed {
        remove_regular_if_present(&paths.launcher)?;
    }
    let stable_identity = paths.install_root.join("bin/tohseno-apple-identity");
    if let Some(source) = old_identity {
        publish_regular(source, &stable_identity)?;
    } else if !identity_existed {
        remove_regular_if_present(&stable_identity)?;
    }
    if let Some(target) = old_current {
        publish_current(&paths.install_root, target)?;
    } else {
        remove_current_if_present(&paths.install_root)?;
    }
    Ok(())
}

fn remove_regular_if_present(path: &Path) -> Result<(), BoxError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(format!(
            "refusing to remove an unsafe rollback path: {}",
            path.display()
        )
        .into()),
        Ok(_) => {
            fs::remove_file(path)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn remove_current_if_present(install_root: &Path) -> Result<(), BoxError> {
    let current = install_root.join("current");
    match fs::symlink_metadata(&current) {
        Ok(metadata) if !metadata.file_type().is_symlink() => {
            Err("refusing to remove a non-symlink current release during rollback".into())
        }
        Ok(_) => {
            fs::remove_file(current)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn bundled_release_root() -> Result<Option<PathBuf>, BoxError> {
    let executable = std::env::current_exe()?;
    let Some(helpers) = executable.parent() else {
        return Ok(None);
    };
    let Some(contents) = helpers.parent() else {
        return Ok(None);
    };
    if helpers.file_name().and_then(|name| name.to_str()) != Some("Helpers")
        || contents.file_name().and_then(|name| name.to_str()) != Some("Contents")
    {
        #[cfg(debug_assertions)]
        return Ok(None);
        #[cfg(not(debug_assertions))]
        return Err("native helper is not inside the signed application bundle".into());
    }
    let root = contents.join("Resources/FactoryRelease");
    if !root.join(MANIFEST).is_file() {
        return Err("Tohseno.app is missing its bundled factory release manifest".into());
    }
    Ok(Some(root))
}

fn verify_release(root: &Path) -> Result<BTreeMap<String, String>, BoxError> {
    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("bundled factory release root is unsafe".into());
    }
    let manifest = read_regular(&root.join(MANIFEST), MAX_MANIFEST_BYTES)?;
    if !manifest.ends_with(b"\n") || !manifest.is_ascii() {
        return Err("factory release manifest is not canonical ASCII".into());
    }
    let body = std::str::from_utf8(&manifest)?;
    let mut expected = BTreeMap::new();
    let mut previous: Option<&str> = None;
    for line in body.lines() {
        if line.len() < 67 || &line[64..66] != "  " {
            return Err("factory release manifest contains a malformed line".into());
        }
        let digest = &line[..64];
        let relative = &line[66..];
        if !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || relative == MANIFEST
            || !safe_relative(relative)
            || previous.is_some_and(|value| value.as_bytes() >= relative.as_bytes())
            || expected.insert(relative.into(), digest.into()).is_some()
        {
            return Err("factory release manifest is unsafe or not sorted".into());
        }
        previous = Some(relative);
    }
    if expected.is_empty() || expected.len() > MAX_RELEASE_FILES {
        return Err("factory release manifest has an invalid file count".into());
    }
    let observed = collect_files(root)?;
    let expected_names = expected.keys().cloned().collect::<BTreeSet<_>>();
    if observed != expected_names {
        return Err("factory release manifest does not cover exactly the bundled files".into());
    }
    for (relative, digest) in &expected {
        if digest_file(&root.join(relative))? != *digest {
            return Err(format!("factory release checksum mismatch: {relative}").into());
        }
    }
    let launcher = root.join("bin/tohseno");
    let launcher_metadata = fs::symlink_metadata(&launcher)?;
    if !launcher_metadata.is_file()
        || launcher_metadata.file_type().is_symlink()
        || launcher_metadata.permissions().mode() & 0o111 == 0
    {
        return Err("factory release launcher is missing or not executable".into());
    }
    let identity_helper = root.join("bin/tohseno-apple-identity");
    let identity_metadata = fs::symlink_metadata(&identity_helper)?;
    if !identity_metadata.is_file()
        || identity_metadata.file_type().is_symlink()
        || identity_metadata.permissions().mode() & 0o111 == 0
    {
        return Err("factory release Apple identity helper is missing or not executable".into());
    }
    Ok(expected)
}

fn collect_files(root: &Path) -> Result<BTreeSet<String>, BoxError> {
    fn visit(root: &Path, directory: &Path, output: &mut BTreeSet<String>) -> Result<(), BoxError> {
        let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err("factory release contains a symlink".into());
            }
            if metadata.is_dir() {
                visit(root, &path, output)?;
            } else if metadata.is_file() {
                let relative = path.strip_prefix(root)?.to_string_lossy().into_owned();
                if relative != MANIFEST
                    && (!safe_relative(&relative)
                        || !output.insert(relative)
                        || output.len() > MAX_RELEASE_FILES)
                {
                    return Err("factory release contains an unsafe path".into());
                }
            } else {
                return Err("factory release contains an unsupported file".into());
            }
        }
        Ok(())
    }
    let mut files = BTreeSet::new();
    visit(root, root, &mut files)?;
    Ok(files)
}

fn stage_release(
    source: &Path,
    destination: &Path,
    release_name: &str,
    expected: &BTreeMap<String, String>,
) -> Result<(), BoxError> {
    let parent = destination
        .parent()
        .ok_or("release destination has no parent")?;
    let staging = parent.join(format!(
        ".native-stage-{release_name}-{}",
        std::process::id()
    ));
    match fs::symlink_metadata(&staging) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err("native release staging path is already occupied".into()),
    }
    fs::create_dir(&staging)?;
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))?;
    let result = (|| -> Result<(), BoxError> {
        for relative in expected
            .keys()
            .map(String::as_str)
            .chain(std::iter::once(MANIFEST))
        {
            let from = source.join(relative);
            let to = staging.join(relative);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            let metadata = fs::symlink_metadata(&from)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("factory release changed during staging".into());
            }
            copy_regular(&from, &to, metadata.permissions().mode())?;
            fs::set_permissions(&to, metadata.permissions())?;
        }
        verify_release(&staging)?;
        File::open(&staging)?.sync_all()?;
        fs::rename(&staging, destination)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() && staging.is_dir() && !staging.is_symlink() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn publish_regular(source: &Path, destination: &Path) -> Result<(), BoxError> {
    let parent = destination
        .parent()
        .ok_or("launcher destination has no parent")?;
    ensure_private_directory(parent)?;
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err("refusing to replace an unsafe stable launcher".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let temporary = parent.join(format!(".tohseno.native.{}.tmp", std::process::id()));
    let result = (|| -> Result<(), BoxError> {
        copy_regular(source, &temporary, 0o755)?;
        fs::rename(&temporary, destination)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = remove_regular_if_present(&temporary);
    }
    result?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn open_regular_nofollow(path: &Path) -> Result<File, BoxError> {
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(libc::O_NOFOLLOW);
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err("factory release path is not a regular file".into());
    }
    Ok(file)
}

fn copy_regular(source: &Path, destination: &Path, mode: u32) -> Result<(), BoxError> {
    let mut input = open_regular_nofollow(source)?;
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .mode(mode & 0o777)
        .custom_flags(libc::O_NOFOLLOW);
    let mut output = options.open(destination)?;
    std::io::copy(&mut input, &mut output)?;
    output.flush()?;
    output.sync_all()?;
    fs::set_permissions(destination, fs::Permissions::from_mode(mode & 0o777))?;
    Ok(())
}

fn publish_current(install_root: &Path, target: &str) -> Result<(), BoxError> {
    if !target.starts_with("releases/") || !safe_relative(target) {
        return Err("current release target is unsafe".into());
    }
    let current = install_root.join("current");
    match fs::symlink_metadata(&current) {
        Ok(metadata) if !metadata.file_type().is_symlink() => {
            return Err("refusing to replace a non-symlink current release".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let temporary = install_root.join(format!(".current.native.{}", std::process::id()));
    symlink(target, &temporary)?;
    fs::rename(&temporary, current)?;
    File::open(install_root)?.sync_all()?;
    Ok(())
}

fn current_release_target(install_root: &Path) -> Result<Option<String>, BoxError> {
    let current = install_root.join("current");
    match fs::symlink_metadata(&current) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = fs::read_link(&current)?;
            let value = target
                .to_str()
                .ok_or("current release target is not UTF-8")?;
            if !value.starts_with("releases/") || !safe_relative(value) {
                return Err("current release target is unsafe".into());
            }
            Ok(Some(value.into()))
        }
        Ok(_) => Err("current release is not an installer-owned symlink".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn safe_relative(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains(['\\', '\0'])
        && value.split('/').all(|component| !component.is_empty())
        && Path::new(value)
            .components()
            .all(|component| match component {
                Component::Normal(part) => part.to_str().is_some_and(|part| {
                    !part.is_empty()
                        && part.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
                        })
                }),
                _ => false,
            })
}

fn digest_file(path: &Path) -> Result<String, BoxError> {
    let mut file = open_regular_nofollow(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > 2 * 1024 * 1024 * 1024 {
        return Err("factory release file is unsafe or oversized".into());
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let length = file.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        digest.update(&buffer[..length]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn read_regular(path: &Path, maximum: u64) -> Result<Vec<u8>, BoxError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err("factory release file is unsafe or oversized".into());
    }
    let mut bytes = Vec::new();
    open_regular_nofollow(path)?
        .take(maximum + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err("factory release file is oversized".into());
    }
    Ok(bytes)
}

fn ensure_private_directory(path: &Path) -> Result<(), BoxError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(format!("{} is not a real directory", path.display()).into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir_all(path)?,
        Err(error) => return Err(error.into()),
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    fn release(root: &Path, launcher: &[u8], identity: &[u8]) {
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(root.join("bin/tohseno"), launcher).unwrap();
        fs::write(root.join("bin/tohseno-apple-identity"), identity).unwrap();
        for path in [
            root.join("bin/tohseno"),
            root.join("bin/tohseno-apple-identity"),
        ] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let lines = ["bin/tohseno", "bin/tohseno-apple-identity"]
            .into_iter()
            .map(|name| format!("{}  {name}\n", digest_file(&root.join(name)).unwrap()))
            .collect::<Vec<_>>();
        let mut sorted = lines;
        sorted.sort_by(|left, right| left[66..].cmp(&right[66..]));
        fs::write(root.join(MANIFEST), sorted.concat()).unwrap();
    }

    #[test]
    fn exact_release_manifest_accepts_only_regular_covered_files() {
        let fixture = tempfile::tempdir().unwrap();
        release(fixture.path(), b"factory", b"identity");
        assert_eq!(verify_release(fixture.path()).unwrap().len(), 2);
        fs::write(fixture.path().join("uncovered"), b"extra").unwrap();
        assert!(verify_release(fixture.path()).is_err());
        fs::remove_file(fixture.path().join("uncovered")).unwrap();
        symlink("bin/tohseno", fixture.path().join("alias")).unwrap();
        assert!(verify_release(fixture.path()).is_err());
    }

    #[test]
    fn rollback_restores_all_program_selection_paths() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("install");
        let old = root.join("releases/old");
        let new = root.join("releases/new");
        release(&old, b"old-factory", b"old-identity");
        release(&new, b"new-factory", b"new-identity");
        fs::create_dir_all(root.join("bin")).unwrap();
        let paths = ServicePaths {
            service_label: "com.tohseno.fixture".into(),
            install_root: root.clone(),
            launcher: root.join("bin/tohseno"),
            logs: root.join("logs"),
            service_state: root.join("service"),
            launch_agent: fixture.path().join("fixture.plist"),
        };
        publish_regular(&old.join("bin/tohseno"), &paths.launcher).unwrap();
        publish_regular(
            &old.join("bin/tohseno-apple-identity"),
            &root.join("bin/tohseno-apple-identity"),
        )
        .unwrap();
        publish_current(&root, "releases/old").unwrap();
        publish_regular(&new.join("bin/tohseno"), &paths.launcher).unwrap();
        publish_regular(
            &new.join("bin/tohseno-apple-identity"),
            &root.join("bin/tohseno-apple-identity"),
        )
        .unwrap();
        publish_current(&root, "releases/new").unwrap();
        restore_program_selection(
            &paths,
            Some("releases/old"),
            Some(&old.join("bin/tohseno")),
            Some(&old.join("bin/tohseno-apple-identity")),
            true,
            true,
        )
        .unwrap();
        assert_eq!(fs::read(&paths.launcher).unwrap(), b"old-factory");
        assert_eq!(
            fs::read(root.join("bin/tohseno-apple-identity")).unwrap(),
            b"old-identity"
        );
        assert_eq!(
            fs::read_link(root.join("current")).unwrap(),
            Path::new("releases/old")
        );
    }

    #[test]
    fn path_rules_reject_traversal_and_unsafe_current_targets() {
        assert!(safe_relative("share/studio/app.js"));
        for value in [
            "",
            "/tmp/file",
            "../escape",
            "share//file",
            "share/file name",
            "share\\file",
        ] {
            assert!(!safe_relative(value), "unexpectedly accepted {value:?}");
        }
        let fixture = tempfile::tempdir().unwrap();
        assert!(publish_current(fixture.path(), "../release").is_err());
    }

    #[test]
    fn activation_lock_serializes_installers() {
        let fixture = tempfile::tempdir().unwrap();
        let first = acquire_activation_lock(fixture.path()).unwrap();
        let root = fixture.path().to_path_buf();
        let (started_tx, started_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let contender = thread::spawn(move || {
            started_tx.send(()).unwrap();
            let _second = acquire_activation_lock(&root).unwrap();
            acquired_tx.send(()).unwrap();
        });
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(acquired_rx.recv_timeout(Duration::from_millis(50)).is_err());
        drop(first);
        acquired_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        contender.join().unwrap();
    }
}
