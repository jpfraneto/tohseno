use super::{run_checked, CommandError};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Device {
    /// CoreDevice identifier accepted by `devicectl --device`.
    pub identifier: String,
    pub udid: Option<String>,
    pub name: String,
    pub product_type: Option<String>,
    pub marketing_name: Option<String>,
    pub os_version: Option<String>,
    pub os_build: Option<String>,
    pub physical: bool,
    pub transport: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceState {
    Ready(Device),
    CableMissing,
    TrustRequired,
    DeveloperModeRequired,
}

#[derive(Debug)]
pub enum DeviceError {
    Command(CommandError),
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Command(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for DeviceError {}

pub fn check() -> Result<DeviceState, DeviceError> {
    let json_path = temporary_json_path("devices");
    // Xcode 26 explicitly says file-based JSON is its only stable scripting API.
    let output = run_checked(
        "xcrun",
        [
            "devicectl",
            "list",
            "devices",
            "--json-output",
            json_path.to_string_lossy().as_ref(),
        ],
        None,
    )
    .map_err(DeviceError::Command);
    let json = fs::read_to_string(&json_path).map_err(DeviceError::Io);
    let _ = fs::remove_file(&json_path);
    output?;
    let usb_registry = std::process::Command::new("ioreg")
        .args(["-p", "IOUSB", "-l", "-w", "0"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default();
    parse_with_usb_registry(&json?, &usb_registry)
}

#[cfg(test)]
fn parse(json: &str) -> Result<DeviceState, DeviceError> {
    parse_with_usb_registry(json, "")
}

fn parse_with_usb_registry(json: &str, usb_registry: &str) -> Result<DeviceState, DeviceError> {
    let response: Response = serde_json::from_str(json).map_err(DeviceError::Json)?;
    let mut saw_reachable_untrusted = false;
    let mut saw_reachable_without_developer_mode = false;

    for entry in response.result.devices {
        if entry.hardware_properties.reality.as_deref() != Some("physical")
            || entry.hardware_properties.platform.as_deref() != Some("iOS")
        {
            continue;
        }
        let registry_has_udid = entry
            .hardware_properties
            .udid
            .as_deref()
            .is_some_and(|udid| usb_registry.contains(udid));
        if !is_reachable(
            entry.connection_properties.transport_type.as_deref(),
            entry.connection_properties.tunnel_state.as_deref(),
        ) && !registry_has_udid
        {
            continue;
        }
        if entry.connection_properties.pairing_state.as_deref() != Some("paired") {
            saw_reachable_untrusted = true;
            continue;
        }
        if entry.device_properties.developer_mode_status.as_deref() != Some("enabled") {
            saw_reachable_without_developer_mode = true;
            continue;
        }
        return Ok(DeviceState::Ready(Device {
            identifier: entry.identifier,
            udid: entry.hardware_properties.udid,
            name: entry
                .device_properties
                .name
                .unwrap_or_else(|| "iPhone".into()),
            product_type: entry.hardware_properties.product_type,
            marketing_name: entry.hardware_properties.marketing_name,
            os_version: entry.device_properties.os_version_number,
            os_build: entry.device_properties.os_build_update,
            physical: true,
            transport: entry
                .connection_properties
                .transport_type
                .unwrap_or_else(|| "unknown".into()),
        }));
    }

    if saw_reachable_untrusted {
        Ok(DeviceState::TrustRequired)
    } else if saw_reachable_without_developer_mode {
        Ok(DeviceState::DeveloperModeRequired)
    } else if usb_registry.contains("iPhone") || usb_registry.contains("Apple Mobile Device") {
        // USB sees the phone but CoreDevice does not yet know it, which is the
        // observable pre-Trust state.
        Ok(DeviceState::TrustRequired)
    } else {
        Ok(DeviceState::CableMissing)
    }
}

fn is_reachable(transport: Option<&str>, tunnel_state: Option<&str>) -> bool {
    let active_tunnel = tunnel_state.is_some_and(|value| value.eq_ignore_ascii_case("connected"));
    transport.is_some_and(|value| {
        let normalized = value.to_ascii_lowercase();
        if ["localnetwork", "network"]
            .iter()
            .any(|candidate| normalized.contains(candidate))
        {
            active_tunnel
        } else {
            ["usb", "wired", "cable", "direct"]
                .iter()
                .any(|candidate| normalized.contains(candidate))
        }
    })
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
struct Response {
    result: ResultBody,
}

#[derive(Debug, Deserialize)]
struct ResultBody {
    #[serde(default)]
    devices: Vec<DeviceEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceEntry {
    identifier: String,
    #[serde(default)]
    connection_properties: ConnectionProperties,
    #[serde(default)]
    device_properties: DeviceProperties,
    #[serde(default)]
    hardware_properties: HardwareProperties,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionProperties {
    transport_type: Option<String>,
    pairing_state: Option<String>,
    tunnel_state: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceProperties {
    name: Option<String>,
    developer_mode_status: Option<String>,
    os_version_number: Option<String>,
    os_build_update: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HardwareProperties {
    platform: Option<String>,
    reality: Option<String>,
    udid: Option<String>,
    product_type: Option<String>,
    marketing_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(
        transport: &str,
        tunnel_state: &str,
        pairing: &str,
        developer_mode: &str,
    ) -> String {
        format!(
            r#"{{
              "result": {{
                "devices": [{{
                  "identifier": "core-device-id",
                  "connectionProperties": {{
                    "transportType": "{transport}",
                    "pairingState": "{pairing}",
                    "tunnelState": "{tunnel_state}"
                  }},
                  "deviceProperties": {{
                    "name": "Test iPhone",
                    "developerModeStatus": "{developer_mode}",
                    "osVersionNumber": "26.5",
                    "osBuildUpdate": "23F84"
                  }},
                  "hardwareProperties": {{
                    "platform": "iOS",
                    "reality": "physical",
                    "udid": "device-udid",
                    "productType": "iPhone15,4",
                    "marketingName": "iPhone 15"
                  }}
                }}]
              }}
            }}"#
        )
    }

    #[test]
    fn paired_network_device_is_a_real_connected_verification_target() {
        let DeviceState::Ready(device) =
            parse(&response("localNetwork", "connected", "paired", "enabled")).unwrap()
        else {
            panic!("paired local-network iPhone was not ready");
        };
        assert_eq!(device.product_type.as_deref(), Some("iPhone15,4"));
        assert_eq!(device.os_version.as_deref(), Some("26.5"));
        assert_eq!(device.os_build.as_deref(), Some("23F84"));
        assert_eq!(device.transport, "localNetwork");
    }

    #[test]
    fn paired_but_disconnected_network_device_is_not_a_live_target() {
        assert_eq!(
            parse(&response(
                "localNetwork",
                "disconnected",
                "paired",
                "enabled"
            ))
            .unwrap(),
            DeviceState::CableMissing
        );
    }

    #[test]
    fn wired_device_advances_one_handoff_at_a_time() {
        assert_eq!(
            parse(&response("usb", "disconnected", "unpaired", "enabled")).unwrap(),
            DeviceState::TrustRequired
        );
        assert_eq!(
            parse(&response("wired", "disconnected", "paired", "disabled")).unwrap(),
            DeviceState::DeveloperModeRequired
        );
        assert!(matches!(
            parse(&response("usb", "disconnected", "paired", "enabled")).unwrap(),
            DeviceState::Ready(_)
        ));
    }
}
