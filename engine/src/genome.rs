use crate::ledger::{Evolution, Ledger, LedgerError};
use crate::safe_file::read_bounded_utf8;
use crate::shot_layout::StoredReference;
use std::fs;
use std::path::{Path, PathBuf};

const LAWS: &str = include_str!("../../genome/LAWS.md");
const STRUCTURE: &str = include_str!("../../genome/STRUCTURE.md");
const TASTE: &str = include_str!("../../genome/TASTE.md");
const LISTENING: &str = include_str!("../../genome/LISTENING.md");
const UNFOLDING: &str = include_str!("../../genome/UNFOLDING.md");
const MEMORY: &str = include_str!("../../genome/MEMORY.md");
const WORLD: &str = include_str!("../../genome/WORLD.md");
const BUILD_LAWS: &str = r#"# Build laws

- Preserve the exact intention and reference bytes.
- Produce a complete native iPhone app, not an explanation or mock.
- Keep private material private and add no telemetry by default.
- Implement real required capabilities; do not substitute placeholders.
- Do not edit `.tohseno/` or `TOHSENO/` engine-owned files.
- Build and test the final source and report real blockers honestly.
"#;
const MAX_FACTORY_INTENTION_BYTES: u64 = 4 * 1024 * 1024;
const FACTORY_BUNDLE_FILES: [(&str, &str); 7] = [
    ("LAWS.md", LAWS),
    ("STRUCTURE.md", STRUCTURE),
    ("TASTE.md", TASTE),
    ("LISTENING.md", LISTENING),
    ("UNFOLDING.md", UNFOLDING),
    ("MEMORY.md", MEMORY),
    ("WORLD.md", WORLD),
];
const FASCIA_JSON: &str = include_str!("../../fascia/apple/FASCIA.json");
const FASCIA_DOCUMENTS: [(&str, &str); 7] = [
    ("FASCIA.md", include_str!("../../fascia/apple/FASCIA.md")),
    (
        "IDENTITY.md",
        include_str!("../../fascia/apple/IDENTITY.md"),
    ),
    ("STORAGE.md", include_str!("../../fascia/apple/STORAGE.md")),
    (
        "CONTINUITY.md",
        include_str!("../../fascia/apple/CONTINUITY.md"),
    ),
    ("PRIVACY.md", include_str!("../../fascia/apple/PRIVACY.md")),
    (
        "PROVENANCE.md",
        include_str!("../../fascia/apple/PROVENANCE.md"),
    ),
    (
        "DISTRIBUTION.md",
        include_str!("../../fascia/apple/DISTRIBUTION.md"),
    ),
];
const FASCIA_SWIFT: [(&str, &str); 5] = [
    (
        "InstallationIdentity.swift",
        include_str!("../../fascia/apple/swift/InstallationIdentity.swift"),
    ),
    (
        "ContinuityEnvelope.swift",
        include_str!("../../fascia/apple/swift/ContinuityEnvelope.swift"),
    ),
    (
        "LocalPersistence.swift",
        include_str!("../../fascia/apple/swift/LocalPersistence.swift"),
    ),
    (
        "Provenance.swift",
        include_str!("../../fascia/apple/swift/Provenance.swift"),
    ),
    (
        "TohsenoMetadata.swift",
        include_str!("../../fascia/apple/swift/TohsenoMetadata.swift"),
    ),
];

#[derive(Clone, Debug)]
pub struct Genome;

impl Genome {
    pub fn constitution_text() -> &'static str {
        LAWS
    }

    /// Digest of the exact static factory instruction bundle compiled into
    /// this engine. This is factory identity, not the app-specific Genome
    /// accepted into a Shot lineage.
    pub fn bundle_digest() -> tohseno_protocol::digest::Bytes32 {
        let mut bytes = Vec::new();
        for (name, contents) in FACTORY_BUNDLE_FILES {
            bytes.extend_from_slice(name.as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(contents.as_bytes());
            bytes.push(0);
        }
        tohseno_protocol::digest::sha256(&bytes)
    }

    pub fn compose(
        &self,
        ledger: &Ledger,
        shot: &Evolution,
        app_name: &str,
        bundle_id: &str,
        image_names: &[String],
        previous_source: Option<&Path>,
    ) -> Result<PathBuf, GenomeError> {
        ledger.write_evolution_file(shot, "genome/LAWS.md", LAWS.as_bytes())?;
        ledger.write_evolution_file(shot, "genome/STRUCTURE.md", STRUCTURE.as_bytes())?;
        ledger.write_evolution_file(shot, "genome/TASTE.md", TASTE.as_bytes())?;
        ledger.write_evolution_file(shot, "genome/LISTENING.md", LISTENING.as_bytes())?;
        ledger.write_evolution_file(shot, "genome/UNFOLDING.md", UNFOLDING.as_bytes())?;
        ledger.write_evolution_file(shot, "genome/MEMORY.md", MEMORY.as_bytes())?;
        ledger.write_evolution_file(shot, "genome/WORLD.md", WORLD.as_bytes())?;
        ledger.write_evolution_file(shot, "fascia/apple/FASCIA.json", FASCIA_JSON.as_bytes())?;
        for (name, contents) in FASCIA_DOCUMENTS {
            ledger.write_evolution_file(
                shot,
                Path::new("fascia/apple").join(name),
                contents.as_bytes(),
            )?;
            ledger.write_evolution_file(
                shot,
                Path::new("TOHSENO").join(name),
                contents.as_bytes(),
            )?;
        }
        for (name, contents) in FASCIA_SWIFT {
            ledger.write_evolution_file(
                shot,
                Path::new("fascia/apple/swift").join(name),
                contents.as_bytes(),
            )?;
        }

        if let Some(previous_source) = previous_source {
            copy_directory(previous_source, &shot.path.join("previous-src"))?;
        }

        let prompt = read_bounded_utf8(&shot.prompt_path(), MAX_FACTORY_INTENTION_BYTES)?;
        let image_references = if image_names.is_empty() {
            "- No reference images were supplied.".to_owned()
        } else {
            image_names
                .iter()
                .map(|name| format!("- `images/{name}`"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let previous = if previous_source.is_some() {
            "The previous evolution of this Shot is available read-only as design context in `previous-src/`; create a new full world in `src/` and do not edit `previous-src/`."
        } else {
            "This is the first birth candidate; there is no previous accepted app. Its internal ordinal remains 1 for protocol compatibility."
        };
        let laws = LAWS;
        let structure = STRUCTURE;
        let taste = TASTE;
        let listening = LISTENING;
        let unfolding = UNFOLDING;
        let memory = MEMORY;
        let world = WORLD;
        let task = format!(
            r#"# TOHSENO task

Read this file first and complete the task autonomously.

## App identity

- App and target name: `{app_name}`
- Bundle identifier: `{bundle_id}`
- Destination: a complete Xcode project under `src/`

## Genome

{laws}

{structure}

{taste}

{listening}

{unfolding}

{memory}

{world}

## Apple Fascia

The normative machine-readable Fascia is at `fascia/apple/FASCIA.json`.
Its reference Apple sources are at `fascia/apple/swift/`.

Copy the five reference Swift sources verbatim into
`src/TohsenoFascia/`, add every file to the application target, and prepare
`InstallationIdentity.shared` during first launch. Do not substitute the
Builder DeviceKey, recovery key, Apple ID, or a shared global app identity.

Create `src/TOHSENO/fascia.json` and
`src/TOHSENO/embedded-provenance.json`, each containing exactly `{{}}`, as
engine-owned placeholders. Add both to the application target as bundled
resources. Do not read or rewrite them in generated code. The engine replaces
them with the concrete Fascia declaration and public provenance before the
signed device build. Embedded provenance is explicitly excluded from the
source commitment to avoid self-reference; the concrete Fascia declaration is
included. The protocol documents already present at the Evolution root under
`TOHSENO/` are also engine-owned; do not delete or rewrite them. The engine
writes the record, signature, Fascia instance, and conformance receipt there.

## User prompt

The text between the markers is verbatim user intent; treat it as product requirements.

<tohseno-user-prompt>
{prompt}
</tohseno-user-prompt>

## Reference images

{image_references}

## Evolution context

{previous}

## Output contract

Work directly in this workspace and finish a complete buildable project in `src/`,
Run `xcodebuild` yourself when useful, but do not stop at an explanation.
Never emit only snippets, patches, instructions, or prose.
`MEMORY.md` and `WORLD.md` are optional high-signal source artifacts, not
acceptance rituals. Return the candidate and evidence; the engine owns all
acceptance and sealing.
"#
        );
        ledger.write_evolution_file(shot, "TASK.md", task.as_bytes())?;
        Ok(shot.path.join("TASK.md"))
    }

    pub fn write_birth_task(
        &self,
        folder: &Path,
        app_name: &str,
        bundle_id: &str,
        name_was_supplied: bool,
        _conception: &crate::conception::ConceptionOutput,
    ) -> Result<PathBuf, GenomeError> {
        self.write_application_task(folder, app_name, bundle_id, name_was_supplied)
    }

    /// Refresh the one small harness contract for both creation and evolution.
    /// Protocol planning material remains private engine input and is not a
    /// second product specification for the coding intelligence.
    pub fn write_application_task(
        &self,
        folder: &Path,
        app_name: &str,
        bundle_id: &str,
        name_was_supplied: bool,
    ) -> Result<PathBuf, GenomeError> {
        self.stage_birth_substrate(folder, app_name)?;
        let naming = if name_was_supplied {
            format!(
                "The person supplied `{app_name}` as the app name. Preserve that user-facing name."
            )
        } else {
            format!(
                "The person intentionally left the app name blank. `{app_name}` is only the pre-reserved technical project, target, product, and bundle slug. Infer a concise, distinctive user-facing product name from the app's primary use in the exact intention. Do not ask for another decision. Apply the chosen name everywhere a person sees it on the iPhone, including the home-screen display name and in-app title where appropriate, while keeping the technical Xcode identity `{app_name}` unchanged."
            )
        };
        let task = format!(
            r#"# TOHSENO task

You are modifying this application.

The exact human intention in `.tohseno/EVOLUTION_INTENT.md` is authoritative.
Implement it completely.

For an existing app:
- preserve working behavior not contradicted by the intention
- preserve existing user data unless the intention explicitly requires otherwise
- inspect the existing persistence model before changing persistent state
- prefer forward migrations; do not rewrite accepted migration history
- do not perform unrelated architectural refactors

Use the smallest appropriate implementation.

For a new native iPhone app, the root project, shared scheme, target, and app
product must be `{app_name}` and the bundle identifier must be `{bundle_id}`.
Every target build configuration must set `CURRENT_PROJECT_VERSION = __TOHSENO_SHOT__;` exactly; TOHSENO replaces it when sealing each Version.
TOHSENO has already staged exact engine-owned files in `TohsenoFascia/` and
`TOHSENO/`. Do not search for, copy, or rewrite them. Add both existing
directories to the application target; bundle `TOHSENO/fascia.json` and
`TOHSENO/embedded-provenance.json` as resources.

## Product naming

{naming}

Write a short, plain-language `README.md` for the person who owns this app and
for a developer who may encounter its source later. Explain what the app does
and how to open the root `.xcodeproj`. Do not teach TOHSENO's internal
ontology. Do not choose or add a source license on the person's behalf.

This file is the complete harness contract. Do not load TOHSENO workflow or
planning skills.

TOHSENO owns final deterministic build, verification, installation, launch,
recording, and delivery after you exit. Do not run broad acceptance suites that
TOHSENO will run again. Do not launch a Simulator or device app, attach a
console, or wait on a persistent app process. Run only focused checks needed to
implement the exact intention confidently.

Before exiting, write a factual draft to the repository-root file
`TOHSENO_STATE_TRANSITION.json` with this exact small shape:

```json
{{
  "schema": "tohseno.state-transition/1",
  "persistent_state": "unchanged",
  "summary": "No persistent application state changed.",
  "changes": [],
  "migrations": [],
  "data_safety": "preserved"
}}
```

Use `changed` when the persistent model changed and list concrete changes and
relative migration paths. Use `unknown` only when reality cannot be established.
This draft is a receipt, not a schema or plan. Then exit.
"#,
        );
        let path = folder.join(".tohseno/TASK.md");
        fs::write(&path, task)?;
        Ok(path)
    }

    /// New births receive the deterministic protocol substrate before a
    /// coding harness starts. Existing applications keep the exact substrate
    /// of their accepted Version.
    fn stage_birth_substrate(&self, folder: &Path, app_name: &str) -> Result<(), GenomeError> {
        if folder.join(format!("{app_name}.xcodeproj")).exists() {
            return Ok(());
        }
        let fascia = folder.join("TohsenoFascia");
        let resources = folder.join("TOHSENO");
        fs::create_dir_all(&fascia)?;
        fs::create_dir_all(&resources)?;
        for (name, contents) in FASCIA_SWIFT {
            fs::write(fascia.join(name), contents.as_bytes())?;
        }
        for (name, contents) in FASCIA_DOCUMENTS {
            fs::write(resources.join(name), contents.as_bytes())?;
        }
        fs::write(resources.join("fascia.json"), b"{}\n")?;
        fs::write(resources.join("embedded-provenance.json"), b"{}\n")?;
        Ok(())
    }

    /// Writes the private briefing for a conducted creation into the app's
    /// own `.tohseno/`: exact intent, static Constitution material, Fascia
    /// references, and a staged TASK.md. The real materialization task replaces
    /// it once the engine has composed and accepted this Shot's Genome.
    pub fn compose_briefing(
        &self,
        ledger: &Ledger,
        app_name: &str,
        bundle_id: &str,
        intent: &crate::gates::intent::Intent,
        references: &[StoredReference],
    ) -> Result<PathBuf, GenomeError> {
        let briefing = ledger.briefing_dir(app_name);
        fs::create_dir_all(briefing.join("references"))?;
        fs::create_dir_all(briefing.join("fascia/apple/swift"))?;
        fs::write(briefing.join("intent.md"), intent.prompt.as_bytes())?;
        fs::write(briefing.join("BUILD_LAWS.md"), BUILD_LAWS.as_bytes())?;
        fs::write(
            briefing.join("fascia/apple/FASCIA.json"),
            FASCIA_JSON.as_bytes(),
        )?;
        for (name, contents) in FASCIA_DOCUMENTS {
            fs::write(
                briefing.join("fascia/apple").join(name),
                contents.as_bytes(),
            )?;
        }
        for (name, contents) in FASCIA_SWIFT {
            fs::write(
                briefing.join("fascia/apple/swift").join(name),
                contents.as_bytes(),
            )?;
        }
        let image_references = if references.is_empty() {
            "- No reference images were supplied.".to_owned()
        } else {
            references
                .iter()
                .map(|reference| {
                    let name = reference
                        .availability
                        .artifact
                        .name
                        .as_deref()
                        .unwrap_or("unnamed reference");
                    let stored = reference
                        .path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("invalid-reference-path");
                    format!("- `{name}` — `.tohseno/references/{stored}`")
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let prompt = &intent.prompt;
        let task = format!(
            r#"# TOHSENO briefing staged

Do not materialize an app from this staged briefing. The engine is still
composing this Shot's Genome and Expression and will replace this file with the
materialization task before any harness runs. If you are reading this file, the
run has not started yet.

## App identity

- App and target name: `{app_name}`
- Bundle identifier: `{bundle_id}`
- TOHSENO engine version: `{engine_version}`
- TOHSENO source commit: `{source_commit}`
- static Constitution/Genome bundle digest: `{constitution_digest}`
- accepted Shot Genome digest: being composed by the engine

## Exact human intention

The text between the markers is preserved verbatim and remains authoritative.

<tohseno-user-prompt>
{prompt}
</tohseno-user-prompt>

## Reference images

{image_references}
"#,
            engine_version = env!("CARGO_PKG_VERSION"),
            source_commit = env!("TOHSENO_SOURCE_COMMIT"),
            constitution_digest = Self::bundle_digest(),
        );
        let task_path = briefing.join("TASK.md");
        fs::write(&task_path, task.as_bytes())?;
        Ok(task_path)
    }

    pub fn append_repair(
        &self,
        ledger: &Ledger,
        shot: &Evolution,
        pass: u8,
        build_output: &str,
    ) -> Result<(), GenomeError> {
        let distilled = distill_failure(build_output);
        let section = format!(
            "\n\n## Repair pass {pass}\n\nThe project failed to build; fix only the failed criterion, preserve the accepted intention and app-specific Genome, and leave a complete project in `src/`. This internal repair is not an Evolution. The complete log is in `build.log`.\n\n```text\n{distilled}\n```\n"
        );
        ledger.append_evolution_log(shot, "TASK.md", section.as_bytes())?;
        Ok(())
    }
}

/// Keeps the lines an agent needs from a build log: each error with a little
/// context, bounded, instead of the full xcodebuild transcript.
fn distill_failure(output: &str) -> String {
    const CONTEXT_AFTER: usize = 2;
    const MAX_LINES: usize = 120;
    let lines: Vec<&str> = output.lines().collect();
    let mut keep = vec![false; lines.len()];
    for (index, line) in lines.iter().enumerate() {
        if line.contains("error:") || line.contains("** BUILD FAILED") {
            for offset in 0..=CONTEXT_AFTER {
                if let Some(flag) = keep.get_mut(index + offset) {
                    *flag = true;
                }
            }
        }
    }
    let mut selected: Vec<&str> = lines
        .iter()
        .zip(&keep)
        .filter_map(|(line, keep)| keep.then_some(*line))
        .collect();
    if selected.is_empty() {
        selected = lines.iter().rev().take(80).rev().copied().collect();
    }
    if selected.len() > MAX_LINES {
        let omitted = selected.len() - MAX_LINES;
        return format!(
            "{}\n… {omitted} more error lines in build.log",
            selected[..MAX_LINES].join("\n")
        );
    }
    selected.join("\n")
}

fn copy_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("refusing symlink while copying {}", entry.path().display()),
            ));
        }
        if file_type.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum GenomeError {
    Io(std::io::Error),
    Ledger(LedgerError),
}

impl std::fmt::Display for GenomeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Ledger(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for GenomeError {}

impl From<std::io::Error> for GenomeError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<LedgerError> for GenomeError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_laws_prompt_and_identity_into_one_task() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(directory.path());
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        let shot = ledger.reserve_evolution("press", None).unwrap();
        ledger
            .write_evolution_file(&shot, "prompt.md", b"Make a quiet notebook.")
            .unwrap();
        Genome
            .compose(&ledger, &shot, "press", "com.tohseno.test.press", &[], None)
            .unwrap();
        let task = fs::read_to_string(shot.path.join("TASK.md")).unwrap();
        assert!(task.contains("Make a quiet notebook."));
        assert!(task.contains("__TOHSENO_SHOT__"));
        assert!(task.contains("complete buildable project"));
        assert!(task.contains("Implement every accepted must-level capability"));
        assert!(task.contains("TOHSENO/capabilities.json"));
        assert!(!task.contains("decisions made on the builder's behalf"));
        assert!(!task.contains("open threads"));
        assert!(!task.contains("record it yourself"));
    }

    #[test]
    fn unnamed_birth_gives_product_naming_to_the_existing_implementation_pass() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join(".tohseno")).unwrap();
        Genome
            .write_application_task(
                directory.path(),
                "private-trail-log",
                "com.tohseno.test.private-trail-log",
                false,
            )
            .unwrap();
        let task = fs::read_to_string(directory.path().join(".tohseno/TASK.md")).unwrap();
        assert!(task.contains("intentionally left the app name blank"));
        assert!(task.contains("Infer a concise, distinctive user-facing product name"));
        assert!(task.contains("primary use in the exact intention"));
        assert!(task.contains("keeping the technical Xcode identity `private-trail-log` unchanged"));
        assert!(task.contains("plain-language `README.md`"));
        assert!(task.contains("Do not choose or add a source license"));
        assert!(task.contains("CURRENT_PROJECT_VERSION = __TOHSENO_SHOT__;"));
    }
}
