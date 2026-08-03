use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;
use tohseno_engine::{Event, EventBus};

const INSTALL_MARKER: &str = "tohseno-stable-install-v2";
const INSTALLER_URL: &str = "https://tohseno.com/oneshot.sh";
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/jpfraneto/tohseno/releases/latest";
const UPDATE_CACHE_SCHEMA: &str = "tohseno.update-check/1";
const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_RELEASE_RESPONSE: usize = 64 * 1024;
const MAX_INSTALLER_BYTES: usize = 256 * 1024;
const PATH_LINE: &str = r#"export PATH="$HOME/.tohseno/bin:$PATH""#;

#[derive(Debug, Deserialize)]
struct LatestRelease {
    tag_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct UpdateCache {
    schema: String,
    checked_at_unix_seconds: u64,
    latest_version: String,
}

pub async fn maybe_emit_update_notice(bus: &EventBus) {
    let Ok(root) = validated_install_root() else {
        return;
    };
    let latest = match fresh_cached_version(&root) {
        Some(version) => Some(version),
        None => match fetch_latest_version().await {
            Ok(version) => {
                let _ = write_update_cache(&root, &version);
                Some(version)
            }
            Err(_) => None,
        },
    };
    if latest
        .as_deref()
        .is_some_and(|version| version_is_newer(version, env!("CARGO_PKG_VERSION")))
    {
        let latest = latest.expect("checked as present");
        bus.emit(Event::handoff(format!(
            "TOHSENO {latest} is available. Run `tohseno update`."
        )));
    }
}

pub async fn update(bus: &EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let root = validated_install_root()?;
    let current = env!("CARGO_PKG_VERSION");
    let latest = fetch_latest_version().await?;
    write_update_cache(&root, &latest)?;
    if !version_is_newer(&latest, current) {
        bus.emit(Event::result(format!("TOHSENO {current} is current.")));
        return Ok(());
    }

    bus.emit(Event::status(format!(
        "updating TOHSENO {current} → {latest}…"
    )));
    let installer = download_installer(&latest).await?;
    let status = tokio::process::Command::new("/bin/sh")
        .arg(installer.path())
        .env("TOHSENO_START_STUDIO", "0")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;
    if !status.success() {
        return Err(format!(
            "the verified installer exited with {}",
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "a signal".into())
        )
        .into());
    }
    installer.close()?;
    verify_installed_version(&root, &latest).await?;
    write_update_cache(&root, &latest)?;
    bus.emit(Event::result(format!(
        "Updated TOHSENO to {latest}. Studio was left closed."
    )));
    Ok(())
}

pub fn uninstall(bus: &EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let root = validated_install_root()?;
    let home = root
        .parent()
        .ok_or("TOHSENO installation root has no home directory")?;
    let profiles = [
        home.join(".zshrc"),
        home.join(".bashrc"),
        home.join(".profile"),
    ];
    let warnings = uninstall_at(&root, &profiles)?;
    bus.emit(Event::result(
        "TOHSENO program files were removed. Shots, identities, feedback, and app data remain.",
    ));
    for warning in warnings {
        bus.emit(Event::status(warning));
    }
    bus.emit(Event::handoff(
        "Open a new terminal to clear the old PATH entry from this shell.",
    ));
    Ok(())
}

fn install_root() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    let home = PathBuf::from(home);
    if !home.is_absolute() {
        return Err("HOME must be absolute".into());
    }
    Ok(home.join(".tohseno"))
}

fn validated_install_root() -> Result<PathBuf, String> {
    let root = install_root()?;
    require_real_directory(&root, "TOHSENO installation root")?;
    let marker = root.join(".tohseno-install-root");
    let metadata = fs::symlink_metadata(&marker)
        .map_err(|_| "no stable TOHSENO installation was found".to_owned())?;
    if !metadata.file_type().is_file() || metadata.len() > 128 {
        return Err("the TOHSENO installation marker is unsafe".into());
    }
    let value = fs::read_to_string(&marker)
        .map_err(|error| format!("the TOHSENO installation marker could not be read: {error}"))?;
    if value.trim_end() != INSTALL_MARKER {
        return Err("the TOHSENO installation marker is unrecognized".into());
    }
    Ok(root)
}

fn require_real_directory(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| format!("{label} does not exist"))?;
    if !metadata.file_type().is_dir() {
        return Err(format!("{label} must be a real directory"));
    }
    Ok(())
}

fn fresh_cached_version(root: &Path) -> Option<String> {
    let path = root.join(".update-check.json");
    let metadata = fs::symlink_metadata(&path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > 4096 {
        return None;
    }
    let cache = serde_json::from_slice::<UpdateCache>(&fs::read(path).ok()?).ok()?;
    if cache.schema != UPDATE_CACHE_SCHEMA || parse_version(&cache.latest_version).is_none() {
        return None;
    }
    let now = unix_seconds().ok()?;
    if cache.checked_at_unix_seconds > now
        || now - cache.checked_at_unix_seconds > UPDATE_CHECK_INTERVAL.as_secs()
    {
        return None;
    }
    Some(cache.latest_version)
}

fn write_update_cache(root: &Path, latest: &str) -> Result<(), String> {
    if parse_version(latest).is_none() {
        return Err("latest release returned an invalid stable version".into());
    }
    let cache = UpdateCache {
        schema: UPDATE_CACHE_SCHEMA.into(),
        checked_at_unix_seconds: unix_seconds()?,
        latest_version: latest.into(),
    };
    let encoded = serde_json::to_vec(&cache).map_err(|error| error.to_string())?;
    let path = root.join(".update-check.json");
    if fs::symlink_metadata(&path).is_ok_and(|metadata| !metadata.file_type().is_file()) {
        return Err("update cache path is unsafe".into());
    }
    let mut file = tempfile::Builder::new()
        .prefix(".update-check.")
        .tempfile_in(root)
        .map_err(|error| format!("update cache could not be staged: {error}"))?;
    file.write_all(&encoded)
        .and_then(|()| file.as_file().sync_all())
        .map_err(|error| format!("update cache could not be written: {error}"))?;
    file.persist(&path)
        .map_err(|error| format!("update cache could not be committed: {error}"))?;
    Ok(())
}

async fn fetch_latest_version() -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(1))
        .timeout(Duration::from_secs(2))
        .user_agent(format!("tohseno-update/{}", env!("CARGO_PKG_VERSION")))
        .build()?;
    let response = client
        .get(LATEST_RELEASE_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(format!("GitHub release check returned {}", response.status()).into());
    }
    let body = bounded_body(response, MAX_RELEASE_RESPONSE).await?;
    let release = serde_json::from_slice::<LatestRelease>(&body)?;
    let version = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name)
        .to_owned();
    if parse_version(&version).is_none() {
        return Err("GitHub returned an invalid stable release version".into());
    }
    Ok(version)
}

async fn download_installer(
    expected_version: &str,
) -> Result<NamedTempFile, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .user_agent(format!("tohseno-update/{}", env!("CARGO_PKG_VERSION")))
        .build()?;
    let response = client.get(INSTALLER_URL).send().await?;
    if !response.status().is_success() || response.url().as_str() != INSTALLER_URL {
        return Err(format!("installer download returned {}", response.status()).into());
    }
    let body = bounded_body(response, MAX_INSTALLER_BYTES).await?;
    validate_installer_pin(&body, expected_version)?;
    let mut file = NamedTempFile::new()?;
    file.write_all(&body)?;
    file.as_file().sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o700))?;
    }
    Ok(file)
}

fn validate_installer_pin(body: &[u8], expected_version: &str) -> Result<(), String> {
    if !body.starts_with(b"#!/bin/sh\n") {
        return Err("downloaded installer has an unexpected format".into());
    }
    let source =
        std::str::from_utf8(body).map_err(|_| "downloaded installer is not UTF-8".to_owned())?;
    let expected_pin = format!("version=\"v{expected_version}\"");
    let version_pins = source
        .lines()
        .filter(|line| line.starts_with("version="))
        .collect::<Vec<_>>();
    if version_pins.as_slice() != [expected_pin.as_str()] {
        return Err(format!(
            "tohseno.com is not serving the TOHSENO {expected_version} installer yet; try `tohseno update` again shortly"
        ));
    }
    Ok(())
}

async fn verify_installed_version(
    root: &Path,
    expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = tokio::process::Command::new(root.join("bin/tohseno"))
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
    let observed = String::from_utf8(output.stdout)?;
    let expected_output = format!("tohseno {expected}");
    if !output.status.success() || observed.trim() != expected_output {
        return Err(format!(
            "the installer did not activate TOHSENO {expected}; try `tohseno update` again shortly"
        )
        .into());
    }
    Ok(())
}

async fn bounded_body(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err("response exceeded its size limit".into());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > limit {
            return Err("response exceeded its size limit".into());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn uninstall_at(root: &Path, profiles: &[PathBuf]) -> Result<Vec<String>, String> {
    require_real_directory(root, "TOHSENO installation root")?;
    let marker = root.join(".tohseno-install-root");
    let marker_metadata =
        fs::symlink_metadata(&marker).map_err(|_| "installation marker is missing")?;
    if !marker_metadata.file_type().is_file()
        || fs::read_to_string(&marker)
            .map_err(|error| error.to_string())?
            .trim_end()
            != INSTALL_MARKER
    {
        return Err("installation marker is unsafe or unrecognized".into());
    }

    validate_current_link(root)?;
    validate_optional_regular_file(&root.join("bin/tohseno"))?;
    validate_optional_regular_file(&root.join("bin/tohseno-apple-identity"))?;
    validate_optional_real_directory(&root.join("bin"))?;
    validate_managed_genesis_link(root)?;
    validate_optional_real_directory(&root.join("share"))?;
    validate_optional_real_directory(&root.join("releases"))?;
    validate_optional_regular_file(&root.join(".update-check.json"))?;

    remove_current_link(root)?;
    remove_managed_file(&root.join("bin/tohseno"))?;
    remove_managed_file(&root.join("bin/tohseno-apple-identity"))?;
    remove_directory_if_empty(&root.join("bin"))?;
    remove_managed_genesis_link(root)?;
    remove_directory_if_empty(&root.join("share"))?;
    remove_managed_tree(&root.join("releases"), "release directory")?;
    remove_optional_regular_file(&root.join(".update-check.json"))?;

    let mut warnings = Vec::new();
    for profile in profiles {
        if let Err(error) = remove_path_line(profile) {
            warnings.push(format!(
                "PATH cleanup skipped for {}: {error}",
                profile.display()
            ));
        }
    }
    fs::remove_file(&marker).map_err(|error| format!("installation marker: {error}"))?;
    remove_directory_if_empty(root)?;
    Ok(warnings)
}

fn remove_current_link(root: &Path) -> Result<(), String> {
    let path = root.join("current");
    validate_current_link(root)?;
    if !path.exists() && fs::symlink_metadata(&path).is_err() {
        return Ok(());
    }
    fs::remove_file(path).map_err(|error| error.to_string())
}

fn validate_current_link(root: &Path) -> Result<(), String> {
    let path = root.join("current");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.file_type().is_symlink() {
        return Err("current release pointer is not an installer-managed symlink".into());
    }
    let target = fs::read_link(&path).map_err(|error| error.to_string())?;
    let components = target.components().collect::<Vec<_>>();
    if !matches!(
        components.as_slice(),
        [Component::Normal(parent), Component::Normal(release)]
            if *parent == "releases" && !release.is_empty()
    ) {
        return Err("current release pointer escapes its installer boundary".into());
    }
    Ok(())
}

fn remove_managed_genesis_link(root: &Path) -> Result<(), String> {
    let path = root.join("share/genesis");
    validate_managed_genesis_link(root)?;
    if !path.exists() && fs::symlink_metadata(&path).is_err() {
        return Ok(());
    }
    fs::remove_file(path).map_err(|error| error.to_string())
}

fn validate_managed_genesis_link(root: &Path) -> Result<(), String> {
    let path = root.join("share/genesis");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.file_type().is_symlink()
        || fs::read_link(&path).map_err(|error| error.to_string())?
            != Path::new("../current/share/genesis")
    {
        return Err("Genesis materials path is not installer-managed".into());
    }
    Ok(())
}

fn remove_managed_file(path: &Path) -> Result<(), String> {
    validate_optional_regular_file(path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    debug_assert!(metadata.file_type().is_file());
    fs::remove_file(path).map_err(|error| error.to_string())
}

fn remove_optional_regular_file(path: &Path) -> Result<(), String> {
    validate_optional_regular_file(path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    debug_assert!(metadata.file_type().is_file());
    fs::remove_file(path).map_err(|error| error.to_string())
}

fn remove_managed_tree(path: &Path, label: &str) -> Result<(), String> {
    validate_optional_real_directory(path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    debug_assert!(metadata.file_type().is_dir());
    fs::remove_dir_all(path).map_err(|error| format!("{label}: {error}"))
}

fn remove_directory_if_empty(path: &Path) -> Result<(), String> {
    validate_optional_real_directory(path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    debug_assert!(metadata.file_type().is_dir());
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn validate_optional_regular_file(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) => Err(format!("{} is not a managed regular file", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn validate_optional_real_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Ok(_) => Err(format!("{} is not a real directory", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn remove_path_line(path: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.file_type().is_file() || metadata.len() > 4 * 1024 * 1024 {
        return Err("shell profile is not a bounded regular file".into());
    }
    let source = fs::read_to_string(path).map_err(|_| "shell profile is not UTF-8")?;
    let filtered = source
        .split_inclusive('\n')
        .filter(|line| line.trim_end_matches(['\r', '\n']) != PATH_LINE)
        .collect::<String>();
    if filtered == source {
        return Ok(());
    }
    let parent = path.parent().ok_or("shell profile has no parent")?;
    let mut file = tempfile::Builder::new()
        .prefix(".tohseno-profile.")
        .tempfile_in(parent)
        .map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        file.as_file()
            .set_permissions(metadata.permissions())
            .map_err(|error| error.to_string())?;
    }
    file.write_all(filtered.as_bytes())
        .and_then(|()| file.as_file().sync_all())
        .map_err(|error| error.to_string())?;
    file.persist(path)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn unix_seconds() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| "system clock is before the Unix epoch".into())
}

fn parse_version(value: &str) -> Option<[u64; 3]> {
    fn component(value: &str) -> Option<u64> {
        if value.is_empty() || (value.len() > 1 && value.starts_with('0')) {
            return None;
        }
        value.parse().ok()
    }

    let mut parts = value.split('.');
    let parsed = [
        component(parts.next()?)?,
        component(parts.next()?)?,
        component(parts.next()?)?,
    ];
    if parts.next().is_some() {
        return None;
    }
    Some(parsed)
}

fn version_is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn stable_versions_compare_without_accepting_ambiguous_tags() {
        assert!(version_is_newer("0.7.1", "0.7.0"));
        assert!(version_is_newer("0.8.0", "0.7.9"));
        assert!(!version_is_newer("0.7.0", "0.7.0"));
        assert!(!version_is_newer("v0.7.1", "0.7.0"));
        assert!(!version_is_newer("0.7.1-rc.1", "0.7.0"));
        assert!(!version_is_newer("0.07.1", "0.7.0"));
    }

    #[test]
    fn update_runs_only_the_installer_pinned_to_the_discovered_release() {
        assert!(
            validate_installer_pin(b"#!/bin/sh\nversion=\"v0.7.1\"\nprintf ready\n", "0.7.1")
                .is_ok()
        );
        assert!(
            validate_installer_pin(b"#!/bin/sh\nversion=\"v0.7.0\"\nprintf ready\n", "0.7.1")
                .is_err()
        );
        assert!(validate_installer_pin(
            b"#!/bin/sh\nversion=\"v0.7.1\"\nversion=\"v0.7.2\"\n",
            "0.7.1"
        )
        .is_err());
    }

    #[test]
    fn uninstall_removes_only_installer_owned_files_and_exact_path_lines() {
        let temporary = TempDir::new().unwrap();
        let home = temporary.path();
        let root = home.join(".tohseno");
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("releases/0.7.1-test/bin")).unwrap();
        fs::create_dir_all(root.join("share")).unwrap();
        fs::create_dir_all(root.join("identity")).unwrap();
        fs::create_dir_all(root.join("pending-intentions/records/local-id")).unwrap();
        fs::write(
            root.join(".tohseno-install-root"),
            format!("{INSTALL_MARKER}\n"),
        )
        .unwrap();
        fs::write(root.join("bin/tohseno"), b"launcher").unwrap();
        fs::write(root.join("bin/tohseno-apple-identity"), b"helper").unwrap();
        fs::write(root.join("releases/0.7.1-test/bin/tohseno"), b"binary").unwrap();
        fs::write(root.join("identity/recovery.vault"), b"preserve").unwrap();
        fs::write(root.join("config.toml"), b"preserve = true\n").unwrap();
        fs::write(
            root.join("pending-intentions/records/local-id/record.json"),
            b"preserve pending intention",
        )
        .unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("releases/0.7.1-test", root.join("current")).unwrap();
            std::os::unix::fs::symlink("../current/share/genesis", root.join("share/genesis"))
                .unwrap();
        }
        let profile = home.join(".zshrc");
        fs::write(
            &profile,
            format!("export PATH=\"/custom:$PATH\"\n{PATH_LINE}\nalias t=tohseno\n"),
        )
        .unwrap();

        let warnings = uninstall_at(&root, std::slice::from_ref(&profile)).unwrap();
        assert!(warnings.is_empty());
        assert!(!root.join("bin/tohseno").exists());
        assert!(!root.join("releases").exists());
        assert!(!root.join("current").exists());
        assert!(!root.join(".tohseno-install-root").exists());
        assert_eq!(
            fs::read(root.join("identity/recovery.vault")).unwrap(),
            b"preserve"
        );
        assert!(root.join("config.toml").is_file());
        assert_eq!(
            fs::read(root.join("pending-intentions/records/local-id/record.json")).unwrap(),
            b"preserve pending intention"
        );
        let profile = fs::read_to_string(profile).unwrap();
        assert_eq!(profile, "export PATH=\"/custom:$PATH\"\nalias t=tohseno\n");
    }

    #[test]
    fn uninstall_refuses_an_unmarked_tree() {
        let temporary = TempDir::new().unwrap();
        let root = temporary.path().join(".tohseno");
        fs::create_dir_all(root.join("releases/keep")).unwrap();
        assert!(uninstall_at(&root, &[]).is_err());
        assert!(root.join("releases/keep").is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn uninstall_refuses_a_current_pointer_outside_the_managed_releases() {
        let temporary = TempDir::new().unwrap();
        let root = temporary.path().join(".tohseno");
        fs::create_dir_all(root.join("releases/keep")).unwrap();
        fs::write(root.join(".tohseno-install-root"), INSTALL_MARKER).unwrap();
        std::os::unix::fs::symlink("../../outside", root.join("current")).unwrap();

        assert!(uninstall_at(&root, &[]).is_err());
        assert!(root.join("releases/keep").is_dir());
        assert!(root.join(".tohseno-install-root").is_file());
    }
}
