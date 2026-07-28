use super::{run_checked, CommandError};
use crate::gates::device::Device;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct SignRequest<'a> {
    pub source: &'a Path,
    pub artifact_directory: &'a Path,
    pub app_name: &'a str,
    pub bundle_id: &'a str,
    pub shot_number: u32,
    pub device: &'a Device,
}

#[derive(Debug)]
pub enum SignError {
    Command(CommandError),
    Io(std::io::Error),
    IdentityMissing,
    ProjectMissing,
    ArtifactMissing,
}

impl std::fmt::Display for SignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Command(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::IdentityMissing => write!(f, "no Apple Development signing identity was found"),
            Self::ProjectMissing => write!(f, "the shot contains no Xcode project"),
            Self::ArtifactMissing => write!(f, "the signed app was not produced"),
        }
    }
}

impl std::error::Error for SignError {}

pub fn development_team() -> Result<String, SignError> {
    let output = run_checked(
        "security",
        ["find-identity", "-v", "-p", "codesigning"],
        None,
    )
    .map_err(SignError::Command)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let certificate_teams = stdout
        .lines()
        .filter(|line| line.contains("\"Apple Development:"))
        .filter_map(|line| line.rsplit_once('('))
        .filter_map(|(_, suffix)| suffix.strip_suffix(")\""))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let defaults = run_checked(
        "defaults",
        [
            "read",
            "com.apple.dt.Xcode",
            "IDEProvisioningTeamByIdentifier",
        ],
        None,
    )
    .map_err(|_| SignError::IdentityMissing)?;
    let xcode_teams = parse_xcode_team_ids(&String::from_utf8_lossy(&defaults.stdout));
    certificate_teams
        .into_iter()
        .find(|team| xcode_teams.contains(team))
        .ok_or(SignError::IdentityMissing)
}

pub fn build_signed(request: SignRequest<'_>) -> Result<PathBuf, SignError> {
    let team = development_team()?;
    let project = find_project(request.source).ok_or(SignError::ProjectMissing)?;
    let derived_data = request.artifact_directory.join("DerivedData");
    fs::create_dir_all(request.artifact_directory).map_err(SignError::Io)?;
    let destination = request
        .device
        .udid
        .as_ref()
        .unwrap_or(&request.device.identifier);
    let args: Vec<OsString> = vec![
        "-project".into(),
        project.into_os_string(),
        "-scheme".into(),
        request.app_name.into(),
        "-configuration".into(),
        "Release".into(),
        "-destination".into(),
        format!("id={destination}").into(),
        "-derivedDataPath".into(),
        derived_data.clone().into_os_string(),
        "-allowProvisioningUpdates".into(),
        "CODE_SIGN_STYLE=Automatic".into(),
        format!("DEVELOPMENT_TEAM={team}").into(),
        format!("PRODUCT_BUNDLE_IDENTIFIER={}", request.bundle_id).into(),
        format!("CURRENT_PROJECT_VERSION={}", request.shot_number).into(),
        "MARKETING_VERSION=1.0".into(),
        "build".into(),
    ];
    run_checked("xcodebuild", args, Some(request.source)).map_err(SignError::Command)?;

    let built_app = derived_data
        .join("Build")
        .join("Products")
        .join("Release-iphoneos")
        .join(format!("{}.app", request.app_name));
    if !built_app.exists() {
        return Err(SignError::ArtifactMissing);
    }
    let artifact = request
        .artifact_directory
        .join(format!("{}.app", request.app_name));
    if artifact.exists() {
        fs::remove_dir_all(&artifact).map_err(SignError::Io)?;
    }
    copy_directory(&built_app, &artifact).map_err(SignError::Io)?;
    Ok(artifact)
}

pub fn days_until_expiry(app: &Path) -> Option<i64> {
    let profile = app.join("embedded.mobileprovision");
    let output = Command::new("security")
        .args(["cms", "-D", "-i"])
        .arg(profile)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let plist = String::from_utf8(output.stdout).ok()?;
    let key = "<key>ExpirationDate</key>";
    let remainder = plist.split_once(key)?.1;
    let raw_date = remainder
        .split_once("<date>")?
        .1
        .split_once("</date>")?
        .0
        .trim();
    let parsed = Command::new("/bin/date")
        .args(["-j", "-f", "%Y-%m-%dT%H:%M:%SZ", raw_date, "+%s"])
        .output()
        .ok()?;
    if !parsed.status.success() {
        return None;
    }
    let expiry = String::from_utf8(parsed.stdout)
        .ok()?
        .trim()
        .parse::<i64>()
        .ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    Some((expiry - now).div_euclid(86_400))
}

pub(crate) fn find_project(source: &Path) -> Option<PathBuf> {
    fs::read_dir(source)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "xcodeproj")
        })
}

fn copy_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn parse_xcode_team_ids(defaults: &str) -> Vec<String> {
    defaults
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("teamID = ")
                .and_then(|value| value.strip_suffix(';'))
                .map(str::to_owned)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_discovery_is_shallow_and_deterministic() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("Press.xcodeproj")).unwrap();
        assert_eq!(
            find_project(directory.path()).unwrap(),
            directory.path().join("Press.xcodeproj")
        );
    }

    #[test]
    fn reads_team_ids_from_xcode_account_defaults() {
        let defaults = r#"{
            teamID = R8G2NH6ZA9;
            teamName = "Personal Team";
        }"#;
        assert_eq!(parse_xcode_team_ids(defaults), ["R8G2NH6ZA9"]);
    }
}
