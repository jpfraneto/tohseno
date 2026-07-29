use crate::ledger::{Ledger, LedgerError, Shot};
use std::fs;
use std::path::{Path, PathBuf};

const LAWS: &str = include_str!("../../genome/LAWS.md");
const STRUCTURE: &str = include_str!("../../genome/STRUCTURE.md");
const TASTE: &str = include_str!("../../genome/TASTE.md");
const LISTENING: &str = include_str!("../../genome/LISTENING.md");
const UNFOLDING: &str = include_str!("../../genome/UNFOLDING.md");
const MEMORY: &str = include_str!("../../genome/MEMORY.md");
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
    pub fn compose(
        &self,
        ledger: &Ledger,
        shot: &Shot,
        app_name: &str,
        bundle_id: &str,
        image_names: &[String],
        previous_source: Option<&Path>,
    ) -> Result<PathBuf, GenomeError> {
        ledger.write_shot_file(shot, "genome/LAWS.md", LAWS.as_bytes())?;
        ledger.write_shot_file(shot, "genome/STRUCTURE.md", STRUCTURE.as_bytes())?;
        ledger.write_shot_file(shot, "genome/TASTE.md", TASTE.as_bytes())?;
        ledger.write_shot_file(shot, "genome/LISTENING.md", LISTENING.as_bytes())?;
        ledger.write_shot_file(shot, "genome/UNFOLDING.md", UNFOLDING.as_bytes())?;
        ledger.write_shot_file(shot, "genome/MEMORY.md", MEMORY.as_bytes())?;
        ledger.write_shot_file(shot, "fascia/apple/FASCIA.json", FASCIA_JSON.as_bytes())?;
        for (name, contents) in FASCIA_DOCUMENTS {
            ledger.write_shot_file(
                shot,
                Path::new("fascia/apple").join(name),
                contents.as_bytes(),
            )?;
            ledger.write_shot_file(shot, Path::new("TOHSENO").join(name), contents.as_bytes())?;
        }
        for (name, contents) in FASCIA_SWIFT {
            ledger.write_shot_file(
                shot,
                Path::new("fascia/apple/swift").join(name),
                contents.as_bytes(),
            )?;
        }

        if let Some(previous_source) = previous_source {
            copy_directory(previous_source, &shot.path.join("previous-src"))?;
        }

        let prompt = fs::read_to_string(shot.prompt_path())?;
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
            "This is evolution 1; there is no previous evolution."
        };
        let laws = LAWS;
        let structure = STRUCTURE;
        let taste = TASTE;
        let listening = LISTENING;
        let unfolding = UNFOLDING;
        let memory = MEMORY;
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
included. The protocol documents already present at the Shot root under
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
including the `src/MEMORY.md` that Memory requires.
Run `xcodebuild` yourself when useful, but do not stop at an explanation.
Never emit only snippets, patches, instructions, or prose.
"#
        );
        ledger.write_shot_file(shot, "TASK.md", task.as_bytes())?;
        Ok(shot.path.join("TASK.md"))
    }

    /// Writes the standing orders any coding agent auto-reads on entering
    /// the folder: `AGENTS.md` (with a `CLAUDE.md` pointer). This is how the
    /// tohseno ontology permeates work tohseno never drives — the agent
    /// itself records each finished Evolution. Engine-owned; written once.
    pub fn write_standing_orders(&self, folder: &Path, app_name: &str) -> Result<(), GenomeError> {
        let agents = folder.join("AGENTS.md");
        if !agents.exists() {
            let contents = format!(
                r#"# This folder is a TOHSENO Shot

One Shot, one intent, many Evolutions — all of them this same app.
The sealed history lives in `.tohseno/evolutions/`; this folder is the
living world of `{app_name}`.

Before working:

- Read `MEMORY.md` — the Shot's own memory of how it came to be and where it stands.
- Read `.tohseno/TASK.md` — the current briefing and the builder's intent.
- Obey the genome in `.tohseno/genome/` — laws, structure, taste, listening, unfolding, memory.

While working:

- Never modify anything inside `.tohseno/`.
- Keep this folder a complete, buildable world at every rest.
- Update `MEMORY.md` before you stop.

When the work builds and is whole, record it yourself:

    tohseno evolve

That runs the gates, signs the record, and appends the next Evolution to
this Shot's history. The builder should never have to remember it.
"#
            );
            fs::write(&agents, contents)?;
        }
        let claude = folder.join("CLAUDE.md");
        if !claude.exists() {
            fs::write(
                &claude,
                "Read AGENTS.md.
",
            )?;
        }
        Ok(())
    }

    /// Writes the private briefing for a conducted creation into the app's
    /// own `.tohseno/`: the builder's intent, the genome, the Fascia
    /// references, and a TASK.md addressed to an agent working in the
    /// visible folder itself. Returns the TASK.md path.
    pub fn compose_briefing(
        &self,
        ledger: &Ledger,
        app_name: &str,
        bundle_id: &str,
        intent: &crate::gates::intent::Intent,
    ) -> Result<PathBuf, GenomeError> {
        let briefing = ledger.briefing_dir(app_name);
        fs::create_dir_all(briefing.join("genome"))?;
        fs::create_dir_all(briefing.join("images"))?;
        fs::create_dir_all(briefing.join("fascia/apple/swift"))?;
        fs::write(briefing.join("intent.md"), intent.prompt.as_bytes())?;
        for (name, contents) in [
            ("LAWS.md", LAWS),
            ("STRUCTURE.md", STRUCTURE),
            ("TASTE.md", TASTE),
            ("LISTENING.md", LISTENING),
            ("UNFOLDING.md", UNFOLDING),
            ("MEMORY.md", MEMORY),
        ] {
            fs::write(briefing.join("genome").join(name), contents.as_bytes())?;
        }
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
        let mut image_names = Vec::new();
        for (index, image) in intent.images.iter().enumerate() {
            if index >= crate::gates::intent::MAX_IMAGES {
                break;
            }
            let name = image
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("image")
                .to_owned();
            fs::copy(image, briefing.join("images").join(&name))?;
            image_names.push(name);
        }
        let image_references = if image_names.is_empty() {
            "- No reference images were supplied.".to_owned()
        } else {
            image_names
                .iter()
                .map(|name| format!("- `.tohseno/images/{name}`"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let prompt = &intent.prompt;
        let laws = LAWS;
        let structure = STRUCTURE;
        let taste = TASTE;
        let listening = LISTENING;
        let unfolding = UNFOLDING;
        let memory = MEMORY;
        let task = format!(
            r#"# TOHSENO task

Read this file first and complete the task autonomously.

You are working in the app's living folder: the parent directory of
`.tohseno/`. Where the genome says `src/`, it means this folder itself.
Never modify anything inside `.tohseno/`.

## App identity

- App and target name: `{app_name}`
- Bundle identifier: `{bundle_id}`
- Destination: a complete Xcode project in this folder

## Genome

{laws}

{structure}

{taste}

{listening}

{unfolding}

{memory}

## Apple Fascia

The normative machine-readable Fascia is at `.tohseno/fascia/apple/FASCIA.json`.
Its reference Apple sources are at `.tohseno/fascia/apple/swift/`.

Copy the five reference Swift sources verbatim into `TohsenoFascia/` in this
folder, add every file to the application target, and prepare
`InstallationIdentity.shared` during first launch. Do not substitute the
Builder DeviceKey, recovery key, Apple ID, or a shared global app identity.

Create `TOHSENO/fascia.json` and `TOHSENO/embedded-provenance.json` in this
folder, each containing exactly `{{}}`, as engine-owned placeholders. Add both
to the application target as bundled resources. Do not read or rewrite them
in generated code; the engine replaces them when the folder is sealed.

## User prompt

The text between the markers is verbatim user intent; treat it as product requirements.

<tohseno-user-prompt>
{prompt}
</tohseno-user-prompt>

## Reference images

{image_references}

## Output contract

Work directly in this folder and finish a complete buildable project here,
including the `MEMORY.md` that Memory requires.
Run `xcodebuild` yourself when useful, but do not stop at an explanation.
Never emit only snippets, patches, instructions, or prose.
When the work builds and is whole, record it yourself: `tohseno evolve`
"#
        );
        let task_path = briefing.join("TASK.md");
        fs::write(&task_path, task.as_bytes())?;
        Ok(task_path)
    }

    pub fn append_repair(
        &self,
        ledger: &Ledger,
        shot: &Shot,
        pass: u8,
        build_output: &str,
    ) -> Result<(), GenomeError> {
        let distilled = distill_failure(build_output);
        let section = format!(
            "\n\n## Repair pass {pass}\n\nThe project failed to build; fix only the project, preserve the user's intent, and leave a complete project in `src/`. The complete log is in `build.log`.\n\n```text\n{distilled}\n```\n"
        );
        ledger.append_shot_log(shot, "TASK.md", section.as_bytes())?;
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
        let shot = ledger.reserve_shot("press", None).unwrap();
        ledger
            .write_shot_file(&shot, "prompt.md", b"Make a quiet notebook.")
            .unwrap();
        Genome
            .compose(&ledger, &shot, "press", "com.tohseno.test.press", &[], None)
            .unwrap();
        let task = fs::read_to_string(shot.path.join("TASK.md")).unwrap();
        assert!(task.contains("Make a quiet notebook."));
        assert!(task.contains("__TOHSENO_SHOT__"));
        assert!(task.contains("complete buildable project"));
    }
}
