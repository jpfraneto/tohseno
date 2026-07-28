use super::{run_checked, CommandError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolchainState {
    Ready,
    Missing,
}

pub fn check() -> ToolchainState {
    let selected = run_checked("xcode-select", ["-p"], None).is_ok();
    let xcode = run_checked("xcodebuild", ["-version"], None).is_ok();
    if selected && xcode {
        ToolchainState::Ready
    } else {
        ToolchainState::Missing
    }
}

pub fn trigger_install() -> Result<(), CommandError> {
    // This is harmless when the installer is already open; the state machine
    // still polls the authoritative `xcodebuild -version` check.
    match run_checked("xcode-select", ["--install"], None) {
        Ok(_) => Ok(()),
        Err(error)
            if String::from_utf8_lossy(&error.output.stderr)
                .to_ascii_lowercase()
                .contains("already installed") =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}
