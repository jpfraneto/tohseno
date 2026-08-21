use super::{run_checked, CommandError};
use crate::gates::device::Device;
use crate::ledger::sanitize_component;
use crate::safe_file::read_bounded_utf8;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_DEVICECTL_JSON_BYTES: u64 = 16 * 1024 * 1024;

pub const CANDIDATE_BUNDLE_PREFIX: &str = "org.tohseno.genesis.";
pub const FREE_PROVISIONING_APP_LIMIT: usize = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledCandidateApp {
    pub bundle_id: String,
    pub name: Option<String>,
}

pub fn require_candidate_namespace(bundle_id: &str) -> Result<(), InstallError> {
    let Some(remainder) = bundle_id.strip_prefix(CANDIDATE_BUNDLE_PREFIX) else {
        return Err(InstallError::BundleNamespace(bundle_id.into()));
    };
    let components = remainder.split('.').collect::<Vec<_>>();
    if components.len() != 2
        || components.iter().any(|component| {
            component.is_empty()
                || sanitize_component(component) != *component
                || !component
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_alphanumeric())
        })
    {
        return Err(InstallError::BundleNamespace(bundle_id.into()));
    }
    Ok(())
}

pub fn install(device: &Device, app: &Path, bundle_id: &str) -> Result<(), InstallError> {
    require_candidate_namespace(bundle_id)?;
    run_checked(
        "xcrun",
        [
            "devicectl".as_ref(),
            "device".as_ref(),
            "install".as_ref(),
            "app".as_ref(),
            "--device".as_ref(),
            device.identifier.as_ref(),
            app.as_os_str(),
        ],
        None,
    )?;
    Ok(())
}

/// Reads the connected phone's actual app inventory through devicectl's
/// stable file-based JSON interface. Local Shot history is deliberately not
/// evidence that an app consumes a provisioning slot on this device.
pub fn installed_candidate_apps(
    device: &Device,
) -> Result<Vec<InstalledCandidateApp>, InstallError> {
    let json_path = temporary_json_path("installed-apps");
    let result = run_checked(
        "xcrun",
        [
            "devicectl",
            "device",
            "info",
            "apps",
            "--device",
            &device.identifier,
            "--json-output",
            json_path.to_string_lossy().as_ref(),
        ],
        None,
    );
    let json = read_bounded_utf8(&json_path, MAX_DEVICECTL_JSON_BYTES);
    let _ = fs::remove_file(&json_path);
    result?;
    parse_installed_candidate_apps(&json?)
}

fn parse_installed_candidate_apps(json: &str) -> Result<Vec<InstalledCandidateApp>, InstallError> {
    let response: InstalledAppsResponse = serde_json::from_str(json)?;
    let mut apps = response
        .result
        .apps
        .into_iter()
        .filter(|app| require_candidate_namespace(&app.bundle_identifier).is_ok())
        .map(|app| InstalledCandidateApp {
            bundle_id: app.bundle_identifier,
            name: app.name,
        })
        .collect::<Vec<_>>();
    apps.sort_by(|left, right| left.bundle_id.cmp(&right.bundle_id));
    apps.dedup_by(|left, right| left.bundle_id == right.bundle_id);
    Ok(apps)
}

pub fn free_team_slot_blocker<'a>(
    installed: &'a [InstalledCandidateApp],
    target_bundle_id: &str,
) -> Option<&'a InstalledCandidateApp> {
    let target_already_installed = installed
        .iter()
        .any(|app| app.bundle_id == target_bundle_id);
    (!target_already_installed && installed.len() >= FREE_PROVISIONING_APP_LIMIT)
        .then(|| &installed[0])
}

fn temporary_json_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "tohseno-{label}-{}-{nonce}.json",
        std::process::id()
    ))
}

#[derive(Debug, Deserialize)]
struct InstalledAppsResponse {
    result: InstalledAppsResult,
}

#[derive(Debug, Deserialize)]
struct InstalledAppsResult {
    #[serde(default)]
    apps: Vec<InstalledAppEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledAppEntry {
    bundle_identifier: String,
    name: Option<String>,
}

pub fn launch(device: &Device, bundle_id: &str) -> Result<(), InstallError> {
    require_candidate_namespace(bundle_id)?;
    run_checked(
        "xcrun",
        [
            "devicectl",
            "device",
            "process",
            "launch",
            "--device",
            &device.identifier,
            "--terminate-existing",
            bundle_id,
        ],
        None,
    )?;
    Ok(())
}

pub fn retire(device: &Device, bundle_id: &str) -> Result<(), InstallError> {
    require_candidate_namespace(bundle_id)?;
    run_checked(
        "xcrun",
        [
            "devicectl",
            "device",
            "uninstall",
            "app",
            "--device",
            &device.identifier,
            bundle_id,
        ],
        None,
    )?;
    Ok(())
}

#[derive(Debug)]
pub enum InstallError {
    Command(CommandError),
    Io(std::io::Error),
    Json(serde_json::Error),
    BundleNamespace(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Command(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::BundleNamespace(bundle_id) => write!(
                formatter,
                "refusing candidate device mutation for unnamespaced bundle identifier: {bundle_id}"
            ),
        }
    }
}

impl std::error::Error for InstallError {}

impl InstallError {
    /// A locked phone is an external readiness condition, not an app defect.
    /// Keep this deliberately narrow so every other devicectl failure still
    /// fails closed at the delivery gate.
    pub fn is_device_locked(&self) -> bool {
        let Self::Command(error) = self else {
            return false;
        };
        let stderr = String::from_utf8_lossy(&error.output.stderr);
        stderr.contains("reason: Locked")
            && (stderr.contains("device was not, or could not be, unlocked")
                || stderr.contains("FBSOpenApplicationErrorDomain error 7"))
    }
}

impl From<CommandError> for InstallError {
    fn from(error: CommandError) -> Self {
        Self::Command(error)
    }
}

impl From<std::io::Error> for InstallError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for InstallError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_namespace_is_structurally_distinct_from_stable_ids() {
        assert!(require_candidate_namespace("org.tohseno.genesis.alice.press").is_ok());
        for unsafe_id in [
            "com.tohseno.alice.press",
            "com.tohseno.genesis.press",
            "org.tohseno.genesis.press",
            "org.tohseno.genesis.alice.press.extra",
            "org.tohseno.genesis.Alice.press",
            "org.tohseno.genesis.alice.-press",
        ] {
            assert!(
                matches!(
                    require_candidate_namespace(unsafe_id),
                    Err(InstallError::BundleNamespace(_))
                ),
                "{unsafe_id}"
            );
        }
    }

    #[test]
    fn every_device_mutation_refuses_a_stable_identifier_before_xcrun() {
        let device = Device {
            identifier: "must-not-be-used".into(),
            udid: None,
            name: "fixture".into(),
            product_type: None,
            marketing_name: None,
            os_version: None,
            os_build: None,
            physical: true,
            transport: "fixture".into(),
        };
        for result in [
            install(
                &device,
                Path::new("/definitely/missing.app"),
                "com.tohseno.alice.press",
            ),
            launch(&device, "com.tohseno.alice.press"),
            retire(&device, "com.tohseno.alice.press"),
        ] {
            assert!(matches!(result, Err(InstallError::BundleNamespace(_))));
        }
    }

    #[test]
    fn generated_apps_use_a_candidate_only_keychain_service() {
        let source = include_str!("../../../fascia/apple/swift/InstallationIdentity.swift");
        assert!(source
            .contains(r#"private let service = "org.tohseno.genesis.installation-identity.v1""#));
        assert!(!source.contains(r#"private let service = "org.tohseno.installation-identity.v1""#));
    }

    #[test]
    fn installed_inventory_counts_only_exact_candidate_namespace_bundles() {
        let json = r#"{
          "result": {
            "apps": [
              {"bundleIdentifier":"org.tohseno.genesis.alice.press","name":"Press","builtByDeveloper":true},
              {"bundleIdentifier":"org.tohseno.genesis.alice.press","name":"Press duplicate"},
              {"bundleIdentifier":"org.tohseno.stable","name":"Stable"},
              {"bundleIdentifier":"com.example.docs","name":"org.tohseno.genesis prose"}
            ]
          }
        }"#;
        assert_eq!(
            parse_installed_candidate_apps(json).unwrap(),
            [InstalledCandidateApp {
                bundle_id: "org.tohseno.genesis.alice.press".into(),
                name: Some("Press".into()),
            }]
        );
    }

    #[test]
    fn free_team_wall_uses_device_truth_and_allows_replacing_same_bundle() {
        let installed = ["one", "two", "three"].map(|name| InstalledCandidateApp {
            bundle_id: format!("org.tohseno.genesis.alice.{name}"),
            name: Some(name.into()),
        });
        assert!(free_team_slot_blocker(&installed, "org.tohseno.genesis.alice.four").is_some());
        assert!(free_team_slot_blocker(&installed, "org.tohseno.genesis.alice.two").is_none());
        assert!(
            free_team_slot_blocker(&installed[..2], "org.tohseno.genesis.alice.four").is_none()
        );
    }

    #[cfg(unix)]
    #[test]
    fn only_the_explicit_locked_device_launch_error_is_retryable() {
        use std::os::unix::process::ExitStatusExt;

        let command_error = |stderr: &str| {
            InstallError::Command(CommandError {
                program: "xcrun devicectl device process launch".into(),
                output: std::process::Output {
                    status: std::process::ExitStatus::from_raw(1 << 8),
                    stdout: Vec::new(),
                    stderr: stderr.as_bytes().to_vec(),
                },
            })
        };

        assert!(command_error(
            "The request was denied for reason: Locked (Unable to launch because the device was not, or could not be, unlocked). FBSOpenApplicationErrorDomain error 7"
        )
        .is_device_locked());
        assert!(!command_error(
            "The application failed to launch because its signature is invalid"
        )
        .is_device_locked());
        assert!(!InstallError::BundleNamespace("com.example.app".into()).is_device_locked());
    }
}
