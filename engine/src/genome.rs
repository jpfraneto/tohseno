use crate::ledger::{Ledger, LedgerError, Shot};
use std::fs;
use std::path::{Path, PathBuf};

const LAWS: &str = include_str!("../../genome/LAWS.md");
const STRUCTURE: &str = include_str!("../../genome/STRUCTURE.md");
const TASTE: &str = include_str!("../../genome/TASTE.md");

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
            "A complete prior shot is available read-only as design context in `previous-src/`; create a new full world in `src/` and do not edit `previous-src/`."
        } else {
            "There is no prior shot."
        };
        let laws = LAWS;
        let structure = STRUCTURE;
        let taste = TASTE;
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

Work directly in this workspace and finish a complete buildable project in `src/`.
Run `xcodebuild` yourself when useful, but do not stop at an explanation.
Never emit only snippets, patches, instructions, or prose.
"#
        );
        ledger.write_shot_file(shot, "TASK.md", task.as_bytes())?;
        Ok(shot.path.join("TASK.md"))
    }

    pub fn append_repair(
        &self,
        ledger: &Ledger,
        shot: &Shot,
        pass: u8,
        build_output: &str,
    ) -> Result<(), GenomeError> {
        let section = format!(
            "\n\n## Repair pass {pass}\n\nThe project failed to build; fix only the project, preserve the user's intent, and leave a complete project in `src/`.\n\n```text\n{build_output}\n```\n"
        );
        ledger.append_shot_log(shot, "TASK.md", section.as_bytes())?;
        Ok(())
    }
}

fn copy_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
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
