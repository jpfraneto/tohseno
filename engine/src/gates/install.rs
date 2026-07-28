use super::{run_checked, CommandError};
use crate::gates::device::Device;
use std::path::Path;

pub fn install(device: &Device, app: &Path) -> Result<(), CommandError> {
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

pub fn launch(device: &Device, bundle_id: &str) -> Result<(), CommandError> {
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

pub fn retire(device: &Device, bundle_id: &str) -> Result<(), CommandError> {
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
