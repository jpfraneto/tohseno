use super::sign;
use crate::ledger::{Ledger, LedgerError, Shot};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SHOT_TOKEN: &str = "__TOHSENO_SHOT__";

#[derive(Debug)]
pub struct BuildFailure {
    pub output: String,
}

pub fn substitute_shot_number(source: &Path, shot_number: u32) -> Result<usize, BuildError> {
    let mut substitutions = 0;
    visit_files(source, &mut |path| {
        let bytes = fs::read(path)?;
        let Ok(text) = String::from_utf8(bytes) else {
            return Ok(());
        };
        if text.contains(SHOT_TOKEN) {
            substitutions += text.matches(SHOT_TOKEN).count();
            fs::write(path, text.replace(SHOT_TOKEN, &shot_number.to_string()))?;
        }
        Ok(())
    })?;
    Ok(substitutions)
}

pub fn compile(
    ledger: &Ledger,
    shot: &Shot,
    app_name: &str,
) -> Result<Result<(), BuildFailure>, BuildError> {
    substitute_shot_number(&shot.source_path(), shot.number)?;
    let project = sign::find_project(&shot.source_path()).ok_or(BuildError::ProjectMissing)?;
    let derived_data = temporary_path("build");
    // The repair gate precedes the human device gate, so Apple's generic iOS
    // device destination proves arm64/iOS compilation here; the signed gate
    // rebuilds against the exact cabled device immediately afterward.
    let output = Command::new("xcodebuild")
        .current_dir(shot.source_path())
        .args([
            OsString::from("-project"),
            project.into_os_string(),
            OsString::from("-scheme"),
            OsString::from(app_name),
            OsString::from("-configuration"),
            OsString::from("Release"),
            OsString::from("-sdk"),
            OsString::from("iphoneos"),
            OsString::from("-destination"),
            OsString::from("generic/platform=iOS"),
            OsString::from("-derivedDataPath"),
            derived_data.into_os_string(),
            OsString::from("CODE_SIGNING_ALLOWED=NO"),
            OsString::from(format!("CURRENT_PROJECT_VERSION={}", shot.number)),
            OsString::from("build"),
        ])
        .output()?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    ledger.append_shot_log(
        shot,
        "build.log",
        format!("\n===== build pass =====\n{combined}\n===== end build pass =====\n").as_bytes(),
    )?;
    if output.status.success() {
        Ok(Ok(()))
    } else {
        Ok(Err(BuildFailure { output: combined }))
    }
}

pub fn validate_complete_source(source: &Path) -> Result<(), BuildError> {
    if sign::find_project(source).is_none() {
        return Err(BuildError::ProjectMissing);
    }
    let has_swift = contains_extension(source, "swift")?;
    if !has_swift {
        return Err(BuildError::SwiftMissing);
    }
    Ok(())
}

fn contains_extension(directory: &Path, extension: &str) -> std::io::Result<bool> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if contains_extension(&path, extension)? {
                return Ok(true);
            }
        } else if path
            .extension()
            .is_some_and(|candidate| candidate == extension)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn visit_files(
    directory: &Path,
    callback: &mut impl FnMut(&Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            visit_files(&entry.path(), callback)?;
        } else {
            callback(&entry.path())?;
        }
    }
    Ok(())
}

fn temporary_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("tohseno-{label}-{}-{nonce}", std::process::id()))
}

#[derive(Debug)]
pub enum BuildError {
    Io(std::io::Error),
    Ledger(LedgerError),
    ProjectMissing,
    SwiftMissing,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Ledger(error) => write!(f, "{error}"),
            Self::ProjectMissing => write!(f, "the harness did not produce an Xcode project"),
            Self::SwiftMissing => write!(f, "the harness did not produce Swift source"),
        }
    }
}

impl std::error::Error for BuildError {}

impl From<std::io::Error> for BuildError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<LedgerError> for BuildError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_the_engine_owned_shot_token() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("App.xcodeproj");
        fs::create_dir(&project).unwrap();
        let file = project.join("project.pbxproj");
        fs::write(&file, "CURRENT_PROJECT_VERSION = __TOHSENO_SHOT__;").unwrap();
        assert_eq!(substitute_shot_number(directory.path(), 7).unwrap(), 1);
        assert_eq!(
            fs::read_to_string(file).unwrap(),
            "CURRENT_PROJECT_VERSION = 7;"
        );
    }
}
