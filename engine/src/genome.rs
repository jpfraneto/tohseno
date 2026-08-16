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

    /// Writes the standing orders any coding agent auto-reads on entering
    /// the folder: `AGENTS.md` (with a `CLAUDE.md` pointer). This is how the
    /// tohseno ontology permeates work tohseno never drives — the agent
    /// itself records each finished Evolution. Engine-owned; written once.
    pub fn write_standing_orders(&self, folder: &Path, app_name: &str) -> Result<(), GenomeError> {
        let agents = folder.join("AGENTS.md");
        if !agents.exists() {
            let contents = format!(
                "# {app_name}\n\nRead `.tohseno/TASK.md`, follow `.tohseno/BUILD_LAWS.md`, and complete the app. Never edit engine-owned files under `.tohseno/` or `TOHSENO/`.\n"
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

    pub fn write_birth_task(
        &self,
        folder: &Path,
        app_name: &str,
        bundle_id: &str,
        conception: &crate::conception::ConceptionOutput,
        _expression: &crate::birth_plan::BirthExpressionPlan,
        factory: &crate::factory_identity::FactoryIdentity,
    ) -> Result<PathBuf, GenomeError> {
        let experience_digest = conception
            .experience_contract
            .digest()
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let task = format!(
            r#"# TOHSENO birth materialization

Return a complete production-quality native iPhone candidate and its evidence.
The engine—not this harness—owns final acceptance and sealing.

## Factory identity

- TOHSENO engine version: `{engine_version}`
- TOHSENO source commit: `{source_commit}`
- static Constitution/Genome bundle digest: `{constitution_digest}`
- accepted Shot Genome digest: `{genome_digest}`
- Apple capability profile digest: `{profile_digest}`

## Authority and product truth

- The exact human intention in `.tohseno/EVOLUTION_INTENT.md` is authoritative.
- The accepted Genome in `.tohseno/genome.json` is its app-specific interpretation.
- The Apple profile at `.tohseno/private/planning/apple-capability-profile.json`
  describes available material and evidence constraints, not a denylist.
- When that profile contains `signing_team`, its exact `team_id` is the
  engine-selected development team for physical evidence. Do not guess a team
  from certificate display names or parenthetical labels; the engine still
  owns the final signed delivery gate.
- Explicit native capabilities must be implemented in the Release product.
- `simulator_unavailable` or unknown hardware never means product absence.
- A fallback is allowed only under its accepted runtime condition and cannot
  become primary merely because Simulator sensor input is absent.
- No primary interaction may be a mock, inert surface, placeholder, or promise
  of later work. “MVP” means the smallest complete production-quality promise.
- Release dependency construction must fail visibly when required durable
  recovery state or the Fascia InstallationIdentity cannot initialize. Never
  silently replace accepted persistence with an in-memory store or replace an
  installation identity with a sentinel string merely to keep the UI moving.
- When the exact intention requires a named live service or API contract,
  successful contract retrieval and real-service journey evidence are part of
  acceptance. Fixtures may test deterministic UI and failure states, but they
  never satisfy the required live integration. An unavailable required service
  remains a blocking product gap in the Experience Trial.
- Internal repair passes are part of this birth, not Evolutions.
- Protocol conformance is necessary and insufficient: the product promise and
  target-user experience must independently pass.

## App identity

- Product/target: `{app_name}`
- Bundle identifier: `{bundle_id}`
- The Shot repository root is the canonical source root. Put exactly one real
  Xcode project at `./{app_name}.xcodeproj`; do not leave the only project
  under `src/`, and do not create a second nested `.xcodeproj`. Application
  source and resources may live in subordinate folders referenced by that
  root project.
- The shared Xcode scheme and built `.app` basename must both be exactly
  `{app_name}`; use `CFBundleDisplayName` for differently cased user-facing
  branding. The engine invokes `xcodebuild -scheme {app_name}` and looks for
  `{app_name}.app`.
- Do not disable signing in project settings. The engine disables signing for
  generic compile/Simulator gates and explicitly enables automatic signing for
  the paired-device delivery gate.

Copy the five Apple Fascia reference sources from
`.tohseno/fascia/apple/swift/` into the repository-root `TohsenoFascia/`
directory and add them to the app target. Keep repository-root
`TOHSENO/fascia.json` and `TOHSENO/embedded-provenance.json` as engine-owned
`{{}}` placeholders in source; do not put the only copies under another source
folder. The engine reconciles observed source/artifact evidence into final
Fascia facts.

## App plan

Read the structured plan at `.tohseno/private/planning/birth-plan.json`, the
accepted app rules at `.tohseno/genome.json`, and the implementation plan at
`.tohseno/private/planning/birth-expression-plan.json`. These files are authoritative;
do not reproduce them in this task.

Protocol-substrate organs preserve identity and provenance. They do not fulfill
product requirements. App-specific organs must drive the implementation.

## Experience Contract

- Authoritative RFC 8785 canonical JSON SHA-256 digest:
  `{experience_digest}`

Read the structured contract at
`.tohseno/private/planning/experience-contract.json`; do not reproduce it here.

Build the Release implementation, run deterministic XCTest and XCUITest where
appropriate, launch in Simulator, traverse each required target-user journey,
capture meaningful multi-state evidence, inspect it from that actor's
perspective, repair mismatches, and repeat until the contract passes. Use
launch arguments or injected sensor fixtures only in test configurations;
prove the real framework path remains in Release source. When the profile has
a compatible connected iPhone and a required scenario is hardware-critical,
also build/install/launch and exercise that scenario on the physical device.

Every claimed final verification must exercise the exact final source tree.
After the last source, test, fixture, or project-definition edit, regenerate
the Xcode project when applicable, rebuild the affected products, and rerun the
relevant suites. An in-flight run or `test-without-building` result from an
older product is not evidence for newer files. Preserve the real exit status
of `xcodebuild`; when filtering output, use `set -o pipefail` or capture and
check the producer status so `grep`, `head`, or `tail` cannot turn a failing
suite into shell success.

Write strict `{trial_schema}` JSON to
`.tohseno/private/planning/experience-trial.json` using the closed schema at
`.tohseno/private/planning/{trial_schema_file}`. Give every organ criterion
its own result and evidence. Do not infer all organ results from a build or
from overall conformance. A product gap, failed must-level journey, missing
required physical trial, or forbidden substitution must remain failed.
Set `experience_contract_digest` to the authoritative digest printed above
exactly; do not substitute a hash of the pretty-printed contract file. Set
`birth_plan_digest` to the accepted Birth Plan's RFC 8785 canonical digest.
Every evidence `relative_path` is relative to the Shot repository root, not to
the trial file. Evidence kept beside the trial must therefore be named like
`.tohseno/private/planning/evidence/...`, and the file, byte length, and raw
SHA-256 digest must match that exact repository-root-relative declaration.
A scenario's `passed` flag covers its complete environment, gestures, expected
states, and completion condition. A DEBUG fixture may prove individual
mechanics inside a failed scenario, but it never makes a named live-service or
physical-device scenario pass; keep that scenario false and record the typed
blocking constraint.

Do not call `tohseno evolve`. Exit after returning the candidate and evidence;
the engine will evaluate, issue a focused repair pass when needed, and seal
only if all three acceptance dimensions pass.
"#,
            engine_version = factory.engine_version,
            source_commit = factory.source_commit,
            constitution_digest = factory.static_constitution_digest,
            genome_digest = factory
                .accepted_shot_genome_digest
                .expect("materialization identity requires an accepted Genome"),
            profile_digest = factory.apple_capability_profile_digest,
            experience_digest = experience_digest,
            trial_schema = crate::experience::EXPERIENCE_TRIAL_SCHEMA,
            trial_schema_file = crate::conception::EXPERIENCE_TRIAL_SCHEMA_FILE,
        );
        let path = folder.join(".tohseno/TASK.md");
        fs::write(&path, task)?;
        Ok(path)
    }

    /// Writes the private briefing for a conducted creation into the app's
    /// own `.tohseno/`: exact intent, static Constitution material, Fascia
    /// references, and a holding TASK.md. The actual app-specific task is
    /// written only after intelligent conception and Genome acceptance.
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
            r#"# TOHSENO conception pending

Do not materialize an app from this holding task. Read
`.tohseno/CONCEPTION.md`; the selected intelligence must first interpret the
exact intention and Apple capability context into a strict app-specific Birth
Plan, Genome, organs, and Experience Contract. The engine validates and accepts
that proposal before replacing this file with the materialization task.

## App identity

- App and target name: `{app_name}`
- Bundle identifier: `{bundle_id}`
- TOHSENO engine version: `{engine_version}`
- TOHSENO source commit: `{source_commit}`
- static Constitution/Genome bundle digest: `{constitution_digest}`
- accepted Shot Genome digest: not yet accepted
- Apple capability profile digest: see `.tohseno/CONCEPTION.md` after discovery

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
}
