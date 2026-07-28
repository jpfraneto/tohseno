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
    parse(&json?)
}

fn parse(json: &str) -> Result<DeviceState, DeviceError> {
    let response: Response = serde_json::from_str(json).map_err(DeviceError::Json)?;
    let mut saw_wired_untrusted = false;
    let mut saw_wired_without_developer_mode = false;

    for entry in response.result.devices {
        if entry.hardware_properties.reality.as_deref() != Some("physical")
            || entry.hardware_properties.platform.as_deref() != Some("iOS")
        {
            continue;
        }
        if !is_wired(entry.connection_properties.transport_type.as_deref()) {
            // A paired Wi-Fi device must never satisfy the cable-only invariant.
            continue;
        }
        if entry.connection_properties.pairing_state.as_deref() != Some("paired") {
            saw_wired_untrusted = true;
            continue;
        }
        if entry.device_properties.developer_mode_status.as_deref() != Some("enabled") {
            saw_wired_without_developer_mode = true;
            continue;
        }
        return Ok(DeviceState::Ready(Device {
            identifier: entry.identifier,
            udid: entry.hardware_properties.udid,
            name: entry
                .device_properties
                .name
                .unwrap_or_else(|| "iPhone".into()),
        }));
    }

    if saw_wired_untrusted {
        Ok(DeviceState::TrustRequired)
    } else if saw_wired_without_developer_mode {
        Ok(DeviceState::DeveloperModeRequired)
    } else {
        Ok(DeviceState::CableMissing)
    }
}

fn is_wired(transport: Option<&str>) -> bool {
    transport.is_some_and(|value| {
        let normalized = value.to_ascii_lowercase();
        ["usb", "wired", "cable", "direct"]
            .iter()
            .any(|candidate| normalized.contains(candidate))
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
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceProperties {
    name: Option<String>,
    developer_mode_status: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HardwareProperties {
    platform: Option<String>,
    reality: Option<String>,
    udid: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(transport: &str, pairing: &str, developer_mode: &str) -> String {
        format!(
            r#"{{
              "result": {{
                "devices": [{{
                  "identifier": "core-device-id",
                  "connectionProperties": {{
                    "transportType": "{transport}",
                    "pairingState": "{pairing}"
                  }},
                  "deviceProperties": {{
                    "name": "Test iPhone",
                    "developerModeStatus": "{developer_mode}"
                  }},
                  "hardwareProperties": {{
                    "platform": "iOS",
                    "reality": "physical",
                    "udid": "device-udid"
                  }}
                }}]
              }}
            }}"#
        )
    }

    #[test]
    fn wifi_never_satisfies_the_cable_gate() {
        assert_eq!(
            parse(&response("localNetwork", "paired", "enabled")).unwrap(),
            DeviceState::CableMissing
        );
    }

    #[test]
    fn wired_device_advances_one_handoff_at_a_time() {
        assert_eq!(
            parse(&response("usb", "unpaired", "enabled")).unwrap(),
            DeviceState::TrustRequired
        );
        assert_eq!(
            parse(&response("wired", "paired", "disabled")).unwrap(),
            DeviceState::DeveloperModeRequired
        );
        assert!(matches!(
            parse(&response("usb", "paired", "enabled")).unwrap(),
            DeviceState::Ready(_)
        ));
    }
}
