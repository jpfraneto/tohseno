use super::{run_checked, CommandError};
use crate::gates::device::Device;
use crate::ledger::sanitize_component;
use std::path::Path;

pub const CANDIDATE_BUNDLE_PREFIX: &str = "org.tohseno.genesis.";

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
    BundleNamespace(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Command(error) => write!(formatter, "{error}"),
            Self::BundleNamespace(bundle_id) => write!(
                formatter,
                "refusing candidate device mutation for unnamespaced bundle identifier: {bundle_id}"
            ),
        }
    }
}

impl std::error::Error for InstallError {}

impl From<CommandError> for InstallError {
    fn from(error: CommandError) -> Self {
        Self::Command(error)
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
}
