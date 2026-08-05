//! Best-effort preview capture. Birth acceptance uses the structured
//! Experience Contract and trial evidence; this decorative first-frame
//! capture is never allowed to masquerade as product verification.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimulatorDevice {
    udid: String,
    state: String,
    device_type_identifier: String,
}

#[derive(Debug, Deserialize)]
struct DeviceListing {
    devices: BTreeMap<String, Vec<SimulatorDevice>>,
}

/// Boots (or reuses) an iPhone Simulator headlessly, installs the built
/// bundle, launches it, and captures one PNG of its first screen.
/// Best-effort by design: any failure returns a short reason and the
/// Evolution completes without a face.
pub fn capture(artifact_app: &Path, bundle_id: &str, output: &Path) -> Result<(), String> {
    let udid = ensure_iphone_simulator()?;
    checked(
        "xcrun",
        &["simctl", "install", &udid, &artifact_app.to_string_lossy()],
    )?;
    checked(
        "xcrun",
        &[
            "simctl",
            "launch",
            "--terminate-running-process",
            &udid,
            bundle_id,
        ],
    )?;
    std::thread::sleep(Duration::from_millis(2500));
    checked(
        "xcrun",
        &[
            "simctl",
            "io",
            &udid,
            "screenshot",
            "--type=png",
            &output.to_string_lossy(),
        ],
    )?;
    let _ = Command::new("xcrun")
        .args(["simctl", "terminate", &udid, bundle_id])
        .output();
    Ok(())
}

/// Returns a booted iPhone Simulator without treating its capabilities as the
/// definition of the Release product. Birth acceptance uses this destination
/// to rerun deterministic tests and controlled target-user adapters.
pub(crate) fn ensure_iphone_simulator() -> Result<String, String> {
    let listing = checked("xcrun", &["simctl", "list", "devices", "available", "-j"])?;
    let listing: DeviceListing =
        serde_json::from_slice(&listing).map_err(|error| error.to_string())?;
    let mut phones: Vec<SimulatorDevice> = listing
        .devices
        .into_iter()
        .rev()
        .flat_map(|(_, devices)| devices)
        .filter(|device| device.device_type_identifier.contains("iPhone"))
        .collect();
    let device = phones
        .iter()
        .find(|device| device.state == "Booted")
        .cloned()
        .or_else(|| phones.drain(..).next())
        .ok_or_else(|| "no iPhone Simulator available".to_owned())?;
    if device.state != "Booted" {
        checked("xcrun", &["simctl", "boot", &device.udid])?;
    }
    checked("xcrun", &["simctl", "bootstatus", &device.udid, "-b"])?;
    Ok(device.udid)
}

pub fn failure_diagnostic(reason: &str) -> String {
    format!("preview.capture · external_environment_constraint · non_product_gap · {reason}")
}

fn checked(program: &str, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr)
            .lines()
            .next()
            .unwrap_or("simulator command failed")
            .to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_failure_is_not_classified_as_product_incompleteness() {
        let diagnostic = failure_diagnostic("Simulator service unavailable");
        assert!(diagnostic.contains("external_environment_constraint"));
        assert!(diagnostic.contains("non_product_gap"));
        assert!(!diagnostic.contains("· product_gap ·"));
    }
}
