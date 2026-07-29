use super::sign;
use crate::ledger::{Ledger, LedgerError, Shot};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SHOT_TOKEN: &str = "__TOHSENO_SHOT__";
const REQUIRED_FASCIA: [(&str, &[u8]); 5] = [
    (
        "InstallationIdentity.swift",
        include_bytes!("../../../fascia/apple/swift/InstallationIdentity.swift"),
    ),
    (
        "ContinuityEnvelope.swift",
        include_bytes!("../../../fascia/apple/swift/ContinuityEnvelope.swift"),
    ),
    (
        "LocalPersistence.swift",
        include_bytes!("../../../fascia/apple/swift/LocalPersistence.swift"),
    ),
    (
        "Provenance.swift",
        include_bytes!("../../../fascia/apple/swift/Provenance.swift"),
    ),
    (
        "TohsenoMetadata.swift",
        include_bytes!("../../../fascia/apple/swift/TohsenoMetadata.swift"),
    ),
];

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
    let project = sign::find_project(&shot.source_path())?.ok_or(BuildError::ProjectMissing)?;
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
            OsString::from("-disableAutomaticPackageResolution"),
            OsString::from("-onlyUsePackageVersionsFromResolvedFile"),
            OsString::from("CODE_SIGNING_ALLOWED=NO"),
            OsString::from("ENABLE_USER_SCRIPT_SANDBOXING=YES"),
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

/// Builds the retained conformance artifact without a device or signing.
///
/// A Shot completes on the Mac; the phone is a destination, not a birth
/// requirement. The Simulator-SDK bundle carries the same Info.plist,
/// provenance resource, and runtime boundary the conformance gate inspects.
pub fn materialize_artifact(
    ledger: &Ledger,
    shot: &Shot,
    app_name: &str,
) -> Result<Result<PathBuf, BuildFailure>, BuildError> {
    let project = sign::find_project(&shot.source_path())?.ok_or(BuildError::ProjectMissing)?;
    let derived_data = temporary_path("artifact");
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
            OsString::from("iphonesimulator"),
            OsString::from("-destination"),
            OsString::from("generic/platform=iOS Simulator"),
            OsString::from("-derivedDataPath"),
            derived_data.clone().into_os_string(),
            OsString::from("-disableAutomaticPackageResolution"),
            OsString::from("-onlyUsePackageVersionsFromResolvedFile"),
            OsString::from("CODE_SIGNING_ALLOWED=NO"),
            OsString::from("ENABLE_USER_SCRIPT_SANDBOXING=YES"),
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
        format!("\n===== artifact pass =====\n{combined}\n===== end artifact pass =====\n")
            .as_bytes(),
    )?;
    if !output.status.success() {
        return Ok(Err(BuildFailure { output: combined }));
    }
    let built = derived_data
        .join("Build")
        .join("Products")
        .join("Release-iphonesimulator")
        .join(format!("{app_name}.app"));
    if !built.is_dir() {
        return Ok(Err(BuildFailure {
            output: "the Simulator artifact was not produced".into(),
        }));
    }
    let destination = shot.artifact_path().join(format!("{app_name}.app"));
    if destination.exists() {
        fs::remove_dir_all(&destination)?;
    }
    copy_bundle(&built, &destination)?;
    Ok(Ok(destination))
}

fn copy_bundle(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "refusing symlink in built bundle: {}",
                    entry.path().display()
                ),
            ));
        }
        if file_type.is_dir() {
            copy_bundle(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub fn validate_complete_source(source: &Path) -> Result<(), BuildError> {
    validate_fascia_source_inventory(source)?;
    let project = sign::find_project(source)?.ok_or(BuildError::ProjectMissing)?;
    let project_file = project.join("project.pbxproj");
    if !project_file.is_file() {
        return Err(BuildError::ProjectMissing);
    }
    let has_swift = contains_extension(source, "swift")?;
    if !has_swift {
        return Err(BuildError::SwiftMissing);
    }
    for (required, normative_bytes) in REQUIRED_FASCIA {
        let expected_path = source.join("TohsenoFascia").join(required);
        let observed = find_named_file(source, required)?
            .ok_or_else(|| BuildError::FasciaMissing(required.into()))?;
        if observed != expected_path || fs::read(&observed)? != normative_bytes {
            return Err(BuildError::FasciaModified(required.into()));
        }
    }

    let project_text = fs::read_to_string(project_file)?;
    if [
        "PBXShellScriptBuildPhase",
        "PBXLegacyTarget",
        "PBXAggregateTarget",
        "PBXExternalBuildToolExecution",
        "shellScript =",
        "preActions =",
        "postActions =",
    ]
    .iter()
    .any(|marker| project_text.contains(marker))
    {
        return Err(BuildError::ExecutableBuildPhase);
    }
    reject_executable_schemes(source)?;
    let synchronized_fascia_group = project_text.contains("PBXFileSystemSynchronizedRootGroup")
        && project_text.contains("TohsenoFascia");
    if !synchronized_fascia_group
        && REQUIRED_FASCIA
            .iter()
            .any(|(required, _)| !project_text.contains(required))
    {
        return Err(BuildError::FasciaTargetMembership);
    }
    if project_text.contains("XCRemoteSwiftPackageReference") {
        return Err(BuildError::ThirdPartyDependency);
    }
    let metadata_files = ["fascia.json", "embedded-provenance.json"];
    if metadata_files
        .iter()
        .any(|name| !source.join("TOHSENO").join(name).is_file())
    {
        return Err(BuildError::EmbeddedProvenanceMissing);
    }
    let synchronized_metadata_group = project_text.contains("PBXFileSystemSynchronizedRootGroup")
        && project_text.contains("TOHSENO");
    if !synchronized_metadata_group
        && metadata_files
            .iter()
            .any(|name| !project_text.contains(name))
    {
        return Err(BuildError::EmbeddedProvenanceTargetMembership);
    }

    let mut installation_bootstrap = false;
    visit_files(source, &mut |path| {
        if path
            .extension()
            .is_some_and(|extension| extension == "swift")
        {
            let text = swift_code_without_comments_or_literals(&fs::read_to_string(path)?);
            if text.contains("InstallationIdentity.shared.prepare(")
                || text.contains("InstallationIdentity.shared.descriptor(")
            {
                installation_bootstrap = true;
            }
        }
        Ok(())
    })?;
    if !installation_bootstrap {
        return Err(BuildError::FasciaBootstrapMissing);
    }
    Ok(())
}

/// Enforces the closed generated subtree claimed by the Fascia manifest.
///
/// The five normative files are the entire `TohsenoFascia` target subtree:
/// additional files, nested directories, symlinks, and special files fail.
pub(crate) fn validate_fascia_source_inventory(source: &Path) -> Result<(), BuildError> {
    let root = source.join("TohsenoFascia");
    let metadata = fs::symlink_metadata(&root)
        .map_err(|error| BuildError::FasciaInventory(format!("{}: {error}", root.display())))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(BuildError::FasciaInventory(format!(
            "{} must be a non-symlink directory",
            root.display()
        )));
    }
    let expected = REQUIRED_FASCIA
        .iter()
        .map(|(name, _)| *name)
        .collect::<BTreeSet<_>>();
    let mut observed = BTreeSet::new();
    for item in fs::read_dir(&root)? {
        let item = item?;
        let name = item
            .file_name()
            .into_string()
            .map_err(|_| BuildError::FasciaInventory("contains a non-UTF-8 filename".into()))?;
        let file_type = item.file_type()?;
        if file_type.is_symlink() || !file_type.is_file() || !expected.contains(name.as_str()) {
            return Err(BuildError::FasciaInventory(format!(
                "unclaimed entry {}",
                item.path().display()
            )));
        }
        if !observed.insert(name) {
            return Err(BuildError::FasciaInventory(
                "contains a duplicate normative filename".into(),
            ));
        }
    }
    let missing = expected
        .iter()
        .filter(|name| !observed.contains(**name))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(BuildError::FasciaInventory(format!(
            "missing {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

fn find_named_file(directory: &Path, name: &str) -> std::io::Result<Option<PathBuf>> {
    let mut found = None;
    find_named_file_inner(directory, name, &mut found)?;
    Ok(found)
}

fn find_named_file_inner(
    directory: &Path,
    name: &str,
    found: &mut Option<PathBuf>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "refusing symlink in generated source: {}",
                    entry.path().display()
                ),
            ));
        }
        if file_type.is_dir() {
            find_named_file_inner(&entry.path(), name, found)?;
        } else if file_type.is_file()
            && entry.file_name() == name
            && found.replace(entry.path()).is_some()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("generated source contains more than one {name}"),
            ));
        }
    }
    Ok(())
}

fn swift_code_without_comments_or_literals(source: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        LineComment,
        BlockComment(u32),
        String { escaped: bool },
        MultilineString,
    }

    let bytes = source.as_bytes();
    let mut output = String::with_capacity(bytes.len());
    let mut state = State::Code;
    let mut index = 0;
    while index < bytes.len() {
        let remaining = &bytes[index..];
        match state {
            State::Code if remaining.starts_with(b"//") => {
                output.push_str("  ");
                state = State::LineComment;
                index += 2;
            }
            State::Code if remaining.starts_with(b"/*") => {
                output.push_str("  ");
                state = State::BlockComment(1);
                index += 2;
            }
            State::Code if remaining.starts_with(b"\"\"\"") => {
                output.push_str("   ");
                state = State::MultilineString;
                index += 3;
            }
            State::Code if bytes[index] == b'"' => {
                output.push(' ');
                state = State::String { escaped: false };
                index += 1;
            }
            State::Code => {
                output.push(char::from(bytes[index]));
                index += 1;
            }
            State::LineComment if bytes[index] == b'\n' => {
                output.push('\n');
                state = State::Code;
                index += 1;
            }
            State::LineComment => {
                output.push(' ');
                index += 1;
            }
            State::BlockComment(depth) if remaining.starts_with(b"/*") => {
                output.push_str("  ");
                state = State::BlockComment(depth.saturating_add(1));
                index += 2;
            }
            State::BlockComment(depth) if remaining.starts_with(b"*/") => {
                output.push_str("  ");
                state = if depth == 1 {
                    State::Code
                } else {
                    State::BlockComment(depth - 1)
                };
                index += 2;
            }
            State::BlockComment(depth) => {
                output.push(if bytes[index] == b'\n' { '\n' } else { ' ' });
                state = State::BlockComment(depth);
                index += 1;
            }
            State::String { escaped: false } if bytes[index] == b'\\' => {
                output.push(' ');
                state = State::String { escaped: true };
                index += 1;
            }
            State::String { escaped: false } if bytes[index] == b'"' => {
                output.push(' ');
                state = State::Code;
                index += 1;
            }
            State::String { .. } => {
                output.push(if bytes[index] == b'\n' { '\n' } else { ' ' });
                state = State::String { escaped: false };
                index += 1;
            }
            State::MultilineString if remaining.starts_with(b"\"\"\"") => {
                output.push_str("   ");
                state = State::Code;
                index += 3;
            }
            State::MultilineString => {
                output.push(if bytes[index] == b'\n' { '\n' } else { ' ' });
                index += 1;
            }
        }
    }
    output
}

fn contains_extension(directory: &Path, extension: &str) -> std::io::Result<bool> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("refusing symlink in generated source: {}", path.display()),
            ));
        }
        if file_type.is_dir() {
            if contains_extension(&path, extension)? {
                return Ok(true);
            }
        } else if file_type.is_file()
            && path
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
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "refusing symlink in generated source: {}",
                    entry.path().display()
                ),
            ));
        }
        if file_type.is_dir() {
            visit_files(&entry.path(), callback)?;
        } else if file_type.is_file() {
            callback(&entry.path())?;
        }
    }
    Ok(())
}

fn reject_executable_schemes(source: &Path) -> Result<(), BuildError> {
    let mut executable = false;
    visit_files(source, &mut |path| {
        if path
            .extension()
            .is_some_and(|extension| extension == "xcscheme")
        {
            let metadata = fs::symlink_metadata(path)?;
            if metadata.len() > 1024 * 1024 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Xcode scheme exceeds the 1 MiB inspection limit",
                ));
            }
            let text = fs::read_to_string(path)?;
            executable |= [
                "<PreActions>",
                "<PostActions>",
                "<ExecutionAction",
                "scriptText=",
                "shellScript",
            ]
            .iter()
            .any(|marker| text.contains(marker));
        }
        Ok(())
    })?;
    if executable {
        Err(BuildError::ExecutableBuildPhase)
    } else {
        Ok(())
    }
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
    FasciaMissing(String),
    FasciaModified(String),
    FasciaInventory(String),
    FasciaTargetMembership,
    FasciaBootstrapMissing,
    EmbeddedProvenanceMissing,
    EmbeddedProvenanceTargetMembership,
    ThirdPartyDependency,
    ExecutableBuildPhase,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Ledger(error) => write!(f, "{error}"),
            Self::ProjectMissing => write!(f, "the harness did not produce an Xcode project"),
            Self::SwiftMissing => write!(f, "the harness did not produce Swift source"),
            Self::FasciaMissing(file) => {
                write!(f, "the generated app is missing Apple Fascia source {file}")
            }
            Self::FasciaModified(file) => {
                write!(
                    f,
                    "the generated app modified normative Apple Fascia source {file}"
                )
            }
            Self::FasciaInventory(detail) => {
                write!(
                    f,
                    "the generated Apple Fascia subtree is not closed: {detail}"
                )
            }
            Self::FasciaTargetMembership => {
                write!(
                    f,
                    "the Apple Fascia sources are not in the application target"
                )
            }
            Self::FasciaBootstrapMissing => {
                write!(
                    f,
                    "the generated app does not prepare its InstallationIdentity"
                )
            }
            Self::EmbeddedProvenanceMissing => {
                write!(
                    f,
                    "the generated app is missing its Fascia or provenance resource placeholder"
                )
            }
            Self::EmbeddedProvenanceTargetMembership => {
                write!(
                    f,
                    "the Fascia or provenance resource is not in the application target"
                )
            }
            Self::ThirdPartyDependency => {
                write!(f, "the generated app declares a third-party Swift package")
            }
            Self::ExecutableBuildPhase => {
                write!(
                    f,
                    "the generated project contains an executable build phase"
                )
            }
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

    #[test]
    fn rejects_executable_scheme_actions() {
        let directory = tempfile::tempdir().unwrap();
        let schemes = directory
            .path()
            .join("App.xcodeproj/xcshareddata/xcschemes");
        fs::create_dir_all(&schemes).unwrap();
        fs::write(
            schemes.join("App.xcscheme"),
            r#"<Scheme><BuildAction><PreActions><ExecutionAction scriptText="curl bad"/></PreActions></BuildAction></Scheme>"#,
        )
        .unwrap();
        assert!(matches!(
            reject_executable_schemes(directory.path()),
            Err(BuildError::ExecutableBuildPhase)
        ));
    }

    #[test]
    fn ignores_bootstrap_text_in_comments_and_literals() {
        let source = r#"
// InstallationIdentity.shared.prepare()
let decoy = "InstallationIdentity.shared.descriptor()"
/* InstallationIdentity.shared.prepare() */
try await InstallationIdentity.shared.prepare()
"#;
        let code = swift_code_without_comments_or_literals(source);
        assert_eq!(
            code.matches("InstallationIdentity.shared.prepare(").count(),
            1
        );
        assert!(!code.contains("InstallationIdentity.shared.descriptor("));
    }

    #[test]
    fn fascia_source_inventory_rejects_unclaimed_entries() {
        let directory = tempfile::tempdir().unwrap();
        let fascia = directory.path().join("TohsenoFascia");
        fs::create_dir(&fascia).unwrap();
        for (name, _) in REQUIRED_FASCIA {
            fs::write(fascia.join(name), b"").unwrap();
        }
        validate_fascia_source_inventory(directory.path()).unwrap();

        fs::write(fascia.join("Unclaimed.swift"), b"let bypass = true\n").unwrap();
        assert!(matches!(
            validate_fascia_source_inventory(directory.path()),
            Err(BuildError::FasciaInventory(_))
        ));
    }
}
