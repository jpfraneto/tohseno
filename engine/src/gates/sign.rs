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
    pub team_id: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisioningKind {
    Paid,
    Free,
    Unknown,
}

impl ProvisioningKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paid => "paid",
            Self::Free => "free",
            Self::Unknown => "unknown",
        }
    }

    fn selection_priority(self) -> u8 {
        match self {
            Self::Paid => 0,
            Self::Unknown => 1,
            Self::Free => 2,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentTeam {
    pub team_id: String,
    pub team_name: Option<String>,
    pub provisioning: ProvisioningKind,
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
    development_team_profile().map(|team| team.team_id)
}

pub fn development_team_profile() -> Result<DevelopmentTeam, SignError> {
    let output = run_checked(
        "security",
        ["find-identity", "-v", "-p", "codesigning"],
        None,
    )
    .map_err(SignError::Command)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("\"Apple Development:") {
        return Err(SignError::IdentityMissing);
    }
    // The authoritative team is the certificate's OU. The CN's parenthetical
    // suffix is an Xcode-managed certificate label, not a Team ID, so it is
    // only a last-resort candidate.
    let mut certificate_teams = certificate_team_ids().unwrap_or_default();
    certificate_teams.extend(
        stdout
            .lines()
            .filter(|line| line.contains("\"Apple Development:"))
            .filter_map(|line| line.rsplit_once('('))
            .filter_map(|(_, suffix)| suffix.strip_suffix(")\""))
            .map(ToOwned::to_owned),
    );
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
    certificate_teams.sort();
    certificate_teams.dedup();
    let xcode_teams = parse_xcode_teams(&String::from_utf8_lossy(&defaults.stdout));
    select_development_team(&certificate_teams, &xcode_teams).ok_or(SignError::IdentityMissing)
}

/// Team IDs (subject OU) of every local Apple Development signing certificate.
fn certificate_team_ids() -> Result<Vec<String>, SignError> {
    let output = run_checked(
        "security",
        ["find-certificate", "-a", "-c", "Apple Development", "-p"],
        None,
    )
    .map_err(SignError::Command)?;
    let pem = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut teams = Vec::new();
    for block in pem.split_inclusive("-----END CERTIFICATE-----") {
        let Some(start) = block.find("-----BEGIN CERTIFICATE-----") else {
            continue;
        };
        let certificate = tempfile::NamedTempFile::new().map_err(SignError::Io)?;
        fs::write(certificate.path(), &block[start..]).map_err(SignError::Io)?;
        let Ok(subject) = run_checked(
            "openssl",
            [
                OsString::from("x509"),
                OsString::from("-noout"),
                OsString::from("-subject"),
                OsString::from("-in"),
                certificate.path().as_os_str().to_owned(),
            ],
            None,
        ) else {
            continue;
        };
        let subject = String::from_utf8_lossy(&subject.stdout).into_owned();
        for field in subject.split(&[',', '/'][..]) {
            if let Some(value) = field.trim().strip_prefix("OU=") {
                let value = value.trim();
                if !value.is_empty() {
                    teams.push(value.to_owned());
                }
            }
        }
    }
    Ok(teams)
}

pub fn build_signed(request: SignRequest<'_>) -> Result<PathBuf, SignError> {
    let project = find_project(request.source)
        .map_err(SignError::Io)?
        .ok_or(SignError::ProjectMissing)?;
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
        "-disableAutomaticPackageResolution".into(),
        "-onlyUsePackageVersionsFromResolvedFile".into(),
        "-allowProvisioningUpdates".into(),
        "ENABLE_USER_SCRIPT_SANDBOXING=YES".into(),
        "CODE_SIGNING_ALLOWED=YES".into(),
        "CODE_SIGNING_REQUIRED=YES".into(),
        "CODE_SIGN_STYLE=Automatic".into(),
        format!("DEVELOPMENT_TEAM={}", request.team_id).into(),
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
    run_checked(
        "/usr/bin/codesign",
        [
            OsString::from("--verify"),
            OsString::from("--deep"),
            OsString::from("--strict"),
            OsString::from("--verbose=2"),
            artifact.clone().into_os_string(),
        ],
        None,
    )
    .map_err(SignError::Command)?;
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

pub(crate) fn find_project(source: &Path) -> std::io::Result<Option<PathBuf>> {
    let mut projects = Vec::new();
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "xcodeproj")
        {
            let file_type = entry.file_type()?;
            if file_type.is_symlink() || !file_type.is_dir() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Xcode project is not a real directory: {}", path.display()),
                ));
            }
            projects.push(path);
        }
    }
    projects.sort();
    match projects.len() {
        0 => Ok(None),
        1 => Ok(projects.pop()),
        count => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("expected exactly one Xcode project, observed {count}"),
        )),
    }
}

fn copy_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("refusing symlink while copying {}", entry.path().display()),
            ));
        }
        if file_type.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Every team Xcode can automatically provision for, including its declared
/// free/paid class. That class is needed both to select the strongest usable
/// team and to avoid inventing free-account restrictions for paid builders.
fn parse_xcode_teams(defaults: &str) -> Vec<DevelopmentTeam> {
    defaults
        .split('{')
        .filter_map(|section| {
            let record = section
                .split_once('}')
                .map_or(section, |(record, _)| record);
            let team_id = openstep_field(record, "teamID")?;
            let team_name = openstep_field(record, "teamName");
            let provisioning = match openstep_field(record, "isFreeProvisioningTeam").as_deref() {
                Some("0") => ProvisioningKind::Paid,
                Some("1") => ProvisioningKind::Free,
                _ => ProvisioningKind::Unknown,
            };
            Some(DevelopmentTeam {
                team_id,
                team_name,
                provisioning,
            })
        })
        .collect()
}

fn openstep_field(record: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} = ");
    record.lines().find_map(|line| {
        line.trim()
            .strip_prefix(&prefix)
            .and_then(|value| value.strip_suffix(';'))
            .map(|value| value.trim().trim_matches('"').to_owned())
    })
}

fn select_development_team(
    certificate_teams: &[String],
    xcode_teams: &[DevelopmentTeam],
) -> Option<DevelopmentTeam> {
    xcode_teams
        .iter()
        .filter(|team| certificate_teams.contains(&team.team_id))
        .min_by_key(|team| (team.provisioning.selection_priority(), &team.team_id))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_discovery_is_shallow_and_deterministic() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("Press.xcodeproj")).unwrap();
        assert_eq!(
            find_project(directory.path()).unwrap().unwrap(),
            directory.path().join("Press.xcodeproj")
        );
    }

    #[test]
    fn project_discovery_rejects_ambiguity() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("One.xcodeproj")).unwrap();
        fs::create_dir(directory.path().join("Two.xcodeproj")).unwrap();
        assert!(find_project(directory.path()).is_err());
    }

    #[test]
    fn reads_personal_and_company_teams_from_xcode_account_defaults() {
        let defaults = r#"{
            teamID = R8G2NH6ZA9;
            teamName = "Personal Team";
            isFreeProvisioningTeam = 1;
        },
        {
            teamID = PAIDTEAM01;
            teamName = "Company Team";
            isFreeProvisioningTeam = 0;
        }"#;
        assert_eq!(
            parse_xcode_teams(defaults),
            [
                DevelopmentTeam {
                    team_id: "R8G2NH6ZA9".into(),
                    team_name: Some("Personal Team".into()),
                    provisioning: ProvisioningKind::Free,
                },
                DevelopmentTeam {
                    team_id: "PAIDTEAM01".into(),
                    team_name: Some("Company Team".into()),
                    provisioning: ProvisioningKind::Paid,
                },
            ]
        );
    }

    #[test]
    fn paid_team_is_preferred_regardless_of_account_or_certificate_order() {
        let certificates = vec!["FREETEAM01".into(), "PAIDTEAM01".into()];
        let teams = vec![
            DevelopmentTeam {
                team_id: "FREETEAM01".into(),
                team_name: Some("Personal Team".into()),
                provisioning: ProvisioningKind::Free,
            },
            DevelopmentTeam {
                team_id: "PAIDTEAM01".into(),
                team_name: Some("Company".into()),
                provisioning: ProvisioningKind::Paid,
            },
        ];
        let selected = select_development_team(&certificates, &teams).unwrap();
        assert_eq!(selected.team_id, "PAIDTEAM01");
        assert_eq!(selected.provisioning, ProvisioningKind::Paid);
    }

    #[test]
    fn free_team_is_used_when_it_is_the_only_certified_xcode_team() {
        let certificates = vec!["FREETEAM01".into()];
        let teams = vec![
            DevelopmentTeam {
                team_id: "PAIDTEAM01".into(),
                team_name: Some("Company".into()),
                provisioning: ProvisioningKind::Paid,
            },
            DevelopmentTeam {
                team_id: "FREETEAM01".into(),
                team_name: Some("Personal Team".into()),
                provisioning: ProvisioningKind::Free,
            },
        ];
        assert_eq!(
            select_development_team(&certificates, &teams)
                .unwrap()
                .provisioning,
            ProvisioningKind::Free
        );
    }

    #[test]
    fn missing_xcode_tier_is_preserved_as_unknown() {
        let parsed = parse_xcode_teams(
            r#"{
                teamID = TEAMUNKNOWN;
                teamName = Example;
            }"#,
        );
        assert_eq!(parsed[0].provisioning, ProvisioningKind::Unknown);
    }
}
