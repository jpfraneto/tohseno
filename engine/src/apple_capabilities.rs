use crate::gates::sign;
use crate::ledger::Ledger;
use crate::safe_file::read_bounded_regular_file;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tohseno_protocol::canonical;
use tohseno_protocol::digest::Bytes32;

pub const APPLE_CAPABILITY_CATALOG_SCHEMA: &str = "tohseno.apple-capability-catalog/1";
pub const APPLE_CAPABILITY_PROFILE_SCHEMA: &str = "tohseno.apple-capability-profile/1";
const DEVICE_HISTORY_SCHEMA: &str = "tohseno.local-apple-device-history/1";
const MAX_DEVICECTL_JSON_BYTES: u64 = 16 * 1024 * 1024;
const MAX_DEVICE_HISTORY_BYTES: u64 = 4 * 1024 * 1024;
const CATALOG_JSON: &str = include_str!("../data/apple-capabilities.json");

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulatorSupport {
    Supported,
    Partial,
    FixtureOnly,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppleCapabilityDefinition {
    pub identifier: String,
    pub frameworks: Vec<String>,
    pub minimum_ios: String,
    pub hardware_predicate: String,
    pub permission_requirements: Vec<String>,
    pub usage_description_keys: Vec<String>,
    pub entitlement_requirements: Vec<String>,
    pub simulator_support: SimulatorSupport,
    pub physical_device_verification: bool,
    pub privacy_implications: Vec<String>,
    pub known_fallback_classes: Vec<String>,
    pub fascia_capabilities: Vec<String>,
    pub factory_supported: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppleCapabilityCatalog {
    pub schema: String,
    pub capabilities: Vec<AppleCapabilityDefinition>,
}

impl AppleCapabilityCatalog {
    pub fn embedded() -> Result<Self, CapabilityProfileError> {
        let catalog: Self = serde_json::from_str(CATALOG_JSON)?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), CapabilityProfileError> {
        check(
            self.schema == APPLE_CAPABILITY_CATALOG_SCHEMA,
            "unsupported Apple capability catalog schema",
        )?;
        check(
            !self.capabilities.is_empty(),
            "Apple capability catalog is empty",
        )?;
        let mut ids = BTreeSet::new();
        for capability in &self.capabilities {
            token("capability", &capability.identifier)?;
            check(
                ids.insert(capability.identifier.as_str()),
                format!(
                    "Apple capability catalog repeats `{}`",
                    capability.identifier
                ),
            )?;
            check(
                !capability.frameworks.is_empty()
                    && !capability.minimum_ios.is_empty()
                    && !capability.hardware_predicate.is_empty(),
                format!("capability `{}` is incomplete", capability.identifier),
            )?;
            for fascia in &capability.fascia_capabilities {
                check(
                    matches!(
                        fascia.as_str(),
                        "local_storage"
                            | "network_access"
                            | "private_cloudkit_sync"
                            | "storekit"
                            | "notifications"
                            | "camera"
                            | "microphone"
                            | "location"
                            | "contacts"
                            | "health"
                            | "bluetooth"
                            | "other_apple_entitlement"
                    ),
                    format!(
                        "capability `{}` names unknown Fascia capability `{fascia}`",
                        capability.identifier
                    ),
                )?;
            }
        }
        Ok(())
    }

    pub fn get(&self, identifier: &str) -> Option<&AppleCapabilityDefinition> {
        self.capabilities
            .iter()
            .find(|capability| capability.identifier == identifier)
    }

    pub fn digest(&self) -> Result<Bytes32, CapabilityProfileError> {
        self.validate()?;
        let bytes = canonical::to_vec(self)
            .map_err(|error| CapabilityProfileError(format!("catalog encoding failed: {error}")))?;
        Ok(tohseno_protocol::digest::sha256(&bytes))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Supported,
    SupportedWithPermission,
    SupportedWithEntitlement,
    HardwareSpecific,
    SimulatorUnavailable,
    UnknownUntilPhysicalDevice,
    UnsupportedByCurrentSdk,
    UnsupportedByFactory,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimulatorRuntimeProfile {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_version: Option<String>,
    pub identifier: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppleDeviceProfile {
    pub product_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketing_name: Option<String>,
    pub os_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_build: Option<String>,
    pub physical: bool,
    pub connection_transport: String,
    pub available_capability_classes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceCapabilityResolution {
    pub product_type: String,
    pub state: CapabilityState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityResolution {
    pub identifier: String,
    pub state: CapabilityState,
    pub simulator_state: CapabilityState,
    pub device_states: Vec<DeviceCapabilityResolution>,
    pub physical_device_verification: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppleSigningProfile {
    pub team_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_name: Option<String>,
    pub provisioning: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppleCapabilityProfile {
    pub schema: String,
    pub catalog_digest: Bytes32,
    pub xcode_version: String,
    pub xcode_build: String,
    pub iphoneos_sdk_version: String,
    pub simulator_runtimes: Vec<SimulatorRuntimeProfile>,
    pub connected_devices: Vec<AppleDeviceProfile>,
    pub last_known_devices: Vec<AppleDeviceProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_team: Option<AppleSigningProfile>,
    pub resolutions: Vec<CapabilityResolution>,
    pub observed_at_unix: u64,
}

impl AppleCapabilityProfile {
    pub fn discover(ledger: &Ledger) -> Result<Self, CapabilityProfileError> {
        let catalog = AppleCapabilityCatalog::embedded()?;
        let (xcode_version, xcode_build) = xcode_version();
        let sdk_version = command_line("xcrun", &["--sdk", "iphoneos", "--show-sdk-version"])
            .unwrap_or_else(|| "unknown".into());
        // The deterministic factory lifecycle deliberately has no live Apple
        // device environment. CoreSimulator and CoreDevice can each wait
        // indefinitely for their daemon on a fresh headless macOS runner, so
        // the existing debug-only no-device boundary must cover dynamic
        // capability discovery as well as final physical delivery. The
        // runner's independent build and test gates are not bypassed.
        let deterministic_no_device = crate::machine::test_factory_no_device();
        let simulator_runtimes = if deterministic_no_device {
            Vec::new()
        } else {
            simulator_runtimes()
        };
        let connected_devices = if deterministic_no_device {
            Vec::new()
        } else {
            connected_device_profiles()
        };
        let signing_team = sign::development_team_profile()
            .ok()
            .map(|team| AppleSigningProfile {
                team_id: team.team_id,
                team_name: team.team_name,
                provisioning: team.provisioning.as_str().into(),
            });
        let history_path = ledger
            .machine_root()
            .join("device-history")
            .join("apple-devices.json");
        let previous = read_device_history(&history_path).unwrap_or_default();
        let last_known_devices = if connected_devices.is_empty() {
            previous
        } else {
            if let Err(error) = write_device_history(&history_path, &connected_devices) {
                // Capability discovery must remain useful when the optional
                // local cache cannot be refreshed.
                let _ = error;
            }
            Vec::new()
        };
        let devices_for_resolution = if connected_devices.is_empty() {
            &last_known_devices
        } else {
            &connected_devices
        };
        let resolutions = catalog
            .capabilities
            .iter()
            .map(|capability| resolve(capability, &sdk_version, devices_for_resolution))
            .collect();
        let profile = Self {
            schema: APPLE_CAPABILITY_PROFILE_SCHEMA.into(),
            catalog_digest: catalog.digest()?,
            xcode_version,
            xcode_build,
            iphoneos_sdk_version: sdk_version,
            simulator_runtimes,
            connected_devices,
            last_known_devices,
            signing_team,
            resolutions,
            observed_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        profile.validate(&catalog)?;
        Ok(profile)
    }

    pub fn validate(&self, catalog: &AppleCapabilityCatalog) -> Result<(), CapabilityProfileError> {
        check(
            self.schema == APPLE_CAPABILITY_PROFILE_SCHEMA,
            "unsupported Apple capability profile schema",
        )?;
        check(
            self.catalog_digest == catalog.digest()?,
            "Apple capability profile catalog digest is stale",
        )?;
        check(
            !self.xcode_version.is_empty()
                && !self.xcode_build.is_empty()
                && !self.iphoneos_sdk_version.is_empty(),
            "Apple capability profile omits toolchain facts",
        )?;
        if let Some(signing) = &self.signing_team {
            check(
                signing.team_id.len() == 10
                    && signing
                        .team_id
                        .bytes()
                        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
                    && matches!(signing.provisioning.as_str(), "paid" | "free" | "unknown")
                    && signing
                        .team_name
                        .as_deref()
                        .is_none_or(|name| !name.trim().is_empty()),
                "Apple capability profile contains an invalid signing team",
            )?;
        }
        check(
            self.resolutions.len() == catalog.capabilities.len(),
            "Apple capability profile does not resolve the whole catalog",
        )?;
        let mut ids = BTreeSet::new();
        for resolution in &self.resolutions {
            check(
                catalog.get(&resolution.identifier).is_some(),
                format!(
                    "profile resolves unknown capability `{}`",
                    resolution.identifier
                ),
            )?;
            check(
                ids.insert(resolution.identifier.as_str()),
                "profile repeats a capability resolution",
            )?;
        }
        for device in self
            .connected_devices
            .iter()
            .chain(self.last_known_devices.iter())
        {
            check(
                device.physical && !device.product_type.is_empty() && !device.os_version.is_empty(),
                "device profile must contain only sanitized physical-device facts",
            )?;
            check(
                device.connection_transport.len() <= 32
                    && device.connection_transport.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
                    }),
                "device profile has an unsafe connection transport",
            )?;
            let mut classes = BTreeSet::new();
            for class in &device.available_capability_classes {
                token("device capability class", class)?;
                check(
                    classes.insert(class.as_str()),
                    "device profile repeats a capability class",
                )?;
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<Bytes32, CapabilityProfileError> {
        let catalog = AppleCapabilityCatalog::embedded()?;
        self.validate(&catalog)?;
        let bytes = canonical::to_vec(self)
            .map_err(|error| CapabilityProfileError(format!("profile encoding failed: {error}")))?;
        Ok(tohseno_protocol::digest::sha256(&bytes))
    }

    pub fn resolution(&self, identifier: &str) -> Option<&CapabilityResolution> {
        self.resolutions
            .iter()
            .find(|resolution| resolution.identifier == identifier)
    }

    pub fn validate_required_capabilities(
        &self,
        identifiers: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<(), CapabilityProfileError> {
        for identifier in identifiers {
            let identifier = identifier.as_ref();
            let resolution = self.resolution(identifier).ok_or_else(|| {
                CapabilityProfileError(format!(
                    "factory_capability_gap: planning capability `{identifier}` is absent from the catalog"
                ))
            })?;
            match resolution.state {
                CapabilityState::UnsupportedByCurrentSdk => {
                    return Err(CapabilityProfileError(format!(
                        "factory_capability_gap: `{identifier}` is unsupported by the current Apple SDK"
                    )))
                }
                CapabilityState::UnsupportedByFactory => {
                    return Err(CapabilityProfileError(format!(
                        "factory_capability_gap: `{identifier}` is unsupported by this factory"
                    )))
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn resolve(
    capability: &AppleCapabilityDefinition,
    sdk_version: &str,
    devices: &[AppleDeviceProfile],
) -> CapabilityResolution {
    let sdk_supported = major_version(sdk_version)
        .zip(major_version(&capability.minimum_ios))
        .is_none_or(|(sdk, minimum)| sdk >= minimum);
    let device_states = devices
        .iter()
        .map(|device| DeviceCapabilityResolution {
            product_type: device.product_type.clone(),
            state: device_state(capability, device),
        })
        .collect::<Vec<_>>();
    let state = if !capability.factory_supported {
        CapabilityState::UnsupportedByFactory
    } else if !sdk_supported {
        CapabilityState::UnsupportedByCurrentSdk
    } else if capability.physical_device_verification
        && capability.hardware_predicate != "none"
        && devices.is_empty()
    {
        CapabilityState::UnknownUntilPhysicalDevice
    } else if capability.hardware_predicate != "none"
        && !device_states.is_empty()
        && device_states
            .iter()
            .all(|resolution| resolution.state == CapabilityState::HardwareSpecific)
    {
        CapabilityState::HardwareSpecific
    } else {
        supported_state(capability)
    };
    let simulator_state = match capability.simulator_support {
        SimulatorSupport::Unavailable | SimulatorSupport::FixtureOnly => {
            CapabilityState::SimulatorUnavailable
        }
        SimulatorSupport::Supported | SimulatorSupport::Partial => supported_state(capability),
    };
    CapabilityResolution {
        identifier: capability.identifier.clone(),
        state,
        simulator_state,
        device_states,
        physical_device_verification: capability.physical_device_verification,
    }
}

fn supported_state(capability: &AppleCapabilityDefinition) -> CapabilityState {
    if !capability.entitlement_requirements.is_empty() {
        CapabilityState::SupportedWithEntitlement
    } else if !capability.permission_requirements.is_empty() {
        CapabilityState::SupportedWithPermission
    } else {
        CapabilityState::Supported
    }
}

fn device_state(
    capability: &AppleCapabilityDefinition,
    device: &AppleDeviceProfile,
) -> CapabilityState {
    if capability.hardware_predicate == "none"
        || device
            .available_capability_classes
            .iter()
            .any(|class| class == &capability.hardware_predicate)
    {
        supported_state(capability)
    } else {
        CapabilityState::HardwareSpecific
    }
}

fn baseline_capability_classes(product_type: &str) -> Vec<String> {
    if product_type.starts_with("iPhone") {
        [
            "rear_camera",
            "microphone",
            "audio_output",
            "arkit_world_tracking",
            "depth_sensor_or_estimation",
            "metal",
            "motion_sensors",
            "haptic_engine",
            "neural_or_cpu_compute",
            "local_storage",
            "notification_service",
            "wifi_or_bluetooth",
            "location_services",
            "healthkit_device",
            "bluetooth",
            "network_interface",
            "secure_enclave_when_available",
            "app_store_services",
            "nfc_reader",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    } else {
        Vec::new()
    }
}

fn connected_device_profiles() -> Vec<AppleDeviceProfile> {
    let path = temporary_json_path("capability-devices");
    let status = Command::new("xcrun")
        .args([
            "devicectl",
            "list",
            "devices",
            "--json-output",
            path.to_string_lossy().as_ref(),
        ])
        .output();
    let bytes = status
        .ok()
        .filter(|output| output.status.success())
        .and_then(|_| read_bounded_regular_file(&path, MAX_DEVICECTL_JSON_BYTES).ok());
    let _ = fs::remove_file(&path);
    let Some(bytes) = bytes else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return Vec::new();
    };
    let usb_registry = Command::new("ioreg")
        .args(["-p", "IOUSB", "-l", "-w", "0"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default();
    device_profiles_from_devicectl(&value, &usb_registry)
}

fn device_profiles_from_devicectl(value: &Value, usb_registry: &str) -> Vec<AppleDeviceProfile> {
    value
        .pointer("/result/devices")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let hardware = entry.get("hardwareProperties")?;
            let device = entry.get("deviceProperties")?;
            let connection = entry.get("connectionProperties")?;
            let transport = connection
                .get("transportType")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let active_tunnel = connection
                .get("tunnelState")
                .and_then(Value::as_str)
                .is_some_and(|state| state.eq_ignore_ascii_case("connected"));
            let direct_transport = {
                let normalized = transport.to_ascii_lowercase();
                ["usb", "wired", "cable", "direct"]
                    .iter()
                    .any(|candidate| normalized.contains(candidate))
            };
            let usb_connected = hardware
                .get("udid")
                .and_then(Value::as_str)
                .is_some_and(|udid| usb_registry_contains_identifier(usb_registry, udid));
            if hardware.get("reality")?.as_str()? != "physical"
                || hardware.get("platform")?.as_str()? != "iOS"
                || connection.get("pairingState").and_then(Value::as_str) != Some("paired")
                || (!active_tunnel && !direct_transport && !usb_connected)
            {
                return None;
            }
            let product_type = hardware.get("productType")?.as_str()?.to_owned();
            Some(AppleDeviceProfile {
                available_capability_classes: baseline_capability_classes(&product_type),
                product_type,
                marketing_name: hardware
                    .get("marketingName")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                os_version: device
                    .get("osVersionNumber")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_owned(),
                os_build: device
                    .get("osBuildUpdate")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                physical: true,
                connection_transport: transport.to_owned(),
            })
        })
        .collect()
}

fn usb_registry_contains_identifier(usb_registry: &str, identifier: &str) -> bool {
    let normalized_identifier: String = identifier
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    !normalized_identifier.is_empty()
        && usb_registry.lines().any(|line| {
            line.chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
                .contains(&normalized_identifier)
        })
}

fn xcode_version() -> (String, String) {
    let text =
        command_line("xcodebuild", &["-version"]).unwrap_or_else(|| "unknown\nunknown".into());
    let mut lines = text.lines();
    let version = lines
        .next()
        .and_then(|line| line.strip_prefix("Xcode "))
        .unwrap_or("unknown")
        .to_owned();
    let build = lines
        .next()
        .and_then(|line| line.strip_prefix("Build version "))
        .unwrap_or("unknown")
        .to_owned();
    (version, build)
}

fn simulator_runtimes() -> Vec<SimulatorRuntimeProfile> {
    let Some(text) = command_line("xcrun", &["simctl", "list", "runtimes", "-j"]) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    value
        .get("runtimes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|runtime| runtime.get("isAvailable").and_then(Value::as_bool) == Some(true))
        .filter(|runtime| {
            runtime
                .get("identifier")
                .and_then(Value::as_str)
                .is_some_and(|identifier| identifier.contains("SimRuntime.iOS"))
        })
        .filter_map(|runtime| {
            Some(SimulatorRuntimeProfile {
                name: runtime.get("name")?.as_str()?.to_owned(),
                version: runtime.get("version")?.as_str()?.to_owned(),
                build_version: runtime
                    .get("buildversion")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                identifier: runtime.get("identifier")?.as_str()?.to_owned(),
            })
        })
        .collect()
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceHistory {
    schema: String,
    devices: Vec<AppleDeviceProfile>,
}

fn read_device_history(path: &Path) -> Option<Vec<AppleDeviceProfile>> {
    let bytes = read_bounded_regular_file(path, MAX_DEVICE_HISTORY_BYTES).ok()?;
    let history: DeviceHistory = serde_json::from_slice(&bytes).ok()?;
    (history.schema == DEVICE_HISTORY_SCHEMA).then_some(history.devices)
}

fn write_device_history(
    path: &Path,
    devices: &[AppleDeviceProfile],
) -> Result<(), CapabilityProfileError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let history = DeviceHistory {
        schema: DEVICE_HISTORY_SCHEMA.into(),
        devices: devices.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&history)?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, path)?;
    Ok(())
}

fn command_line(program: &str, arguments: &[&str]) -> Option<String> {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn major_version(value: &str) -> Option<u64> {
    value.split('.').next()?.parse().ok()
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

fn token(field: &str, value: &str) -> Result<(), CapabilityProfileError> {
    check(
        !value.is_empty()
            && value.len() <= 128
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-' | b'.')
            }),
        format!("{field} `{value}` must be a bounded lower-case token"),
    )
}

fn check(condition: bool, message: impl Into<String>) -> Result<(), CapabilityProfileError> {
    if condition {
        Ok(())
    } else {
        Err(CapabilityProfileError(message.into()))
    }
}

#[derive(Debug)]
pub struct CapabilityProfileError(pub String);

impl fmt::Display for CapabilityProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CapabilityProfileError {}

impl From<std::io::Error> for CapabilityProfileError {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<serde_json::Error> for CapabilityProfileError {
    fn from(value: serde_json::Error) -> Self {
        Self(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_is_data_driven_and_contains_rich_apple_materials() {
        let catalog = AppleCapabilityCatalog::embedded().unwrap();
        for identifier in [
            "camera_capture",
            "microphone_input",
            "ar_world_tracking",
            "realitykit_rendering",
            "scene_reconstruction",
            "motion_orientation",
            "haptics",
            "healthkit",
            "nearby_interaction",
        ] {
            assert!(catalog.get(identifier).is_some(), "missing {identifier}");
        }
        assert_eq!(
            catalog
                .get("ar_world_tracking")
                .unwrap()
                .fascia_capabilities,
            ["camera"]
        );
    }

    #[test]
    fn signing_profile_uses_one_valid_engine_selected_team() {
        let catalog = AppleCapabilityCatalog::embedded().unwrap();
        let mut profile = crate::anky_fixture::profile();
        profile.signing_team = Some(AppleSigningProfile {
            team_id: "AB12CD34EF".into(),
            team_name: Some("Example Team".into()),
            provisioning: "paid".into(),
        });
        profile.validate(&catalog).unwrap();

        profile.signing_team.as_mut().unwrap().team_id = "certificate-label".into();
        assert!(profile
            .validate(&catalog)
            .unwrap_err()
            .to_string()
            .contains("invalid signing team"));
    }

    #[test]
    fn simulator_absence_is_not_product_prohibition() {
        let capability = AppleCapabilityCatalog::embedded()
            .unwrap()
            .get("ar_world_tracking")
            .unwrap()
            .clone();
        let device = AppleDeviceProfile {
            product_type: "iPhone15,4".into(),
            marketing_name: Some("iPhone 15".into()),
            os_version: "26.0".into(),
            os_build: None,
            physical: true,
            connection_transport: "usb".into(),
            available_capability_classes: baseline_capability_classes("iPhone15,4"),
        };
        let resolution = resolve(&capability, "26.0", &[device]);
        assert_eq!(resolution.state, CapabilityState::SupportedWithPermission);
        assert_eq!(
            resolution.simulator_state,
            CapabilityState::SimulatorUnavailable
        );
    }

    #[test]
    fn paired_wired_device_is_present_without_a_coredevice_tunnel() {
        let value = serde_json::json!({
            "result": {
                "devices": [{
                    "connectionProperties": {
                        "pairingState": "paired",
                        "transportType": "wired",
                        "tunnelState": "disconnected"
                    },
                    "deviceProperties": {
                        "osVersionNumber": "26.5.2",
                        "osBuildUpdate": "23F84"
                    },
                    "hardwareProperties": {
                        "marketingName": "iPhone 15",
                        "platform": "iOS",
                        "productType": "iPhone15,4",
                        "reality": "physical",
                        "udid": "00008120-00062D311442601E"
                    }
                }]
            }
        });

        let profiles = device_profiles_from_devicectl(&value, "");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].product_type, "iPhone15,4");
        assert_eq!(profiles[0].connection_transport, "wired");
    }

    #[test]
    fn usb_registry_matches_hyphenated_coredevice_udid() {
        assert!(usb_registry_contains_identifier(
            r#"\"USB Serial Number\" = \"0000812000062D311442601E\""#,
            "00008120-00062D311442601E"
        ));
    }

    #[test]
    fn non_hardware_critical_capability_does_not_need_a_connected_phone() {
        let capability = AppleCapabilityCatalog::embedded()
            .unwrap()
            .get("local_persistence")
            .unwrap()
            .clone();
        let resolution = resolve(&capability, "26.0", &[]);
        assert_eq!(resolution.state, CapabilityState::Supported);
        assert_eq!(resolution.simulator_state, CapabilityState::Supported);
    }

    #[test]
    fn unverified_extension_capability_is_a_visible_factory_gap() {
        let catalog = AppleCapabilityCatalog::embedded().unwrap();
        let capability = catalog.get("widgets_live_activities").unwrap();
        let resolution = resolve(capability, "26.0", &[]);
        assert_eq!(resolution.state, CapabilityState::UnsupportedByFactory);
        let mut profile = crate::anky_fixture::profile();
        profile
            .resolutions
            .iter_mut()
            .find(|item| item.identifier == "widgets_live_activities")
            .unwrap()
            .state = CapabilityState::UnsupportedByFactory;
        let error = profile
            .validate_required_capabilities(["widgets_live_activities"])
            .unwrap_err()
            .to_string();
        assert!(error.contains("factory_capability_gap"));
        assert!(error.contains("unsupported by this factory"));
    }
}
