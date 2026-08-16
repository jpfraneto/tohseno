use crate::events::{Event, EventBus};
use crate::ledger::{Evolution, Ledger, LedgerError};
use crate::safe_file::{read_bounded_regular_file, read_bounded_utf8};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tohseno_protocol::digest::{sha256, Bytes32};

pub const MAX_IMAGES: usize = 8;
const MAX_DROPPED_TEXT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "heic", "webp"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Intent {
    pub prompt: String,
    pub images: Vec<PathBuf>,
}

#[derive(Debug)]
pub enum IntentError {
    Io(std::io::Error),
    Ledger(LedgerError),
    Invalid(String),
}

impl std::fmt::Display for IntentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Ledger(error) => write!(f, "{error}"),
            Self::Invalid(reason) => write!(f, "invalid intention reference: {reason}"),
        }
    }
}

impl std::error::Error for IntentError {}

impl From<std::io::Error> for IntentError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<LedgerError> for IntentError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

impl Intent {
    /// Finds macOS terminal drag/drop paths anywhere in a submitted prompt and
    /// removes only those path spans from the prompt body.
    pub fn parse(submission: &str) -> Self {
        let tokens = path_tokens(submission);
        let mut ranges = Vec::new();
        let mut images = Vec::new();
        let mut dropped_texts = Vec::new();
        for token in tokens {
            let path = PathBuf::from(token.value);
            if path.is_absolute() && is_supported_image(&path) {
                ranges.push(token.start..token.end);
                images.push(path);
            } else if path.is_absolute() && is_supported_text(&path) {
                // A dropped MASTER_PROMPT.md (or any text file) IS the
                // intention; its contents replace the path in place.
                if let Ok(contents) = read_bounded_utf8(&path, MAX_DROPPED_TEXT_BYTES) {
                    ranges.push(token.start..token.end);
                    dropped_texts.push(contents);
                }
            }
        }

        ranges.sort_by_key(|range| range.start);
        let mut prompt = submission.to_owned();
        for range in ranges.into_iter().rev() {
            prompt.replace_range(range, "");
        }
        // A dropped prompt file replaces the intention only when the box held
        // nothing but dropped files; prose that merely mentions a path is
        // never silently rewritten.
        if !dropped_texts.is_empty() && prompt.trim().is_empty() {
            prompt = dropped_texts.join("\n\n");
        }
        Self { prompt, images }
    }

    /// Interprets a terminal file-drop payload. Unlike [`Self::parse`], text
    /// documents are expanded wherever they appear because the interactive
    /// composer knows this input came from a file insertion rather than prose
    /// that merely mentions a path. Images must exist before they can become
    /// a visible composer slot.
    pub fn parse_dropped(submission: &str) -> Self {
        let tokens = path_tokens(submission);
        let mut replacements = Vec::new();
        let mut images = Vec::new();
        for token in tokens {
            let path = PathBuf::from(token.value);
            if path.is_absolute() && path.is_file() && is_supported_image(&path) {
                replacements.push((token.start..token.end, String::new()));
                images.push(path);
            } else if path.is_absolute() && is_supported_text(&path) {
                if let Ok(contents) = read_bounded_utf8(&path, MAX_DROPPED_TEXT_BYTES) {
                    replacements.push((token.start..token.end, contents));
                }
            }
        }

        replacements.sort_by_key(|(range, _)| range.start);
        let mut prompt = submission.to_owned();
        for (range, replacement) in replacements.into_iter().rev() {
            prompt.replace_range(range, &replacement);
        }
        Self { prompt, images }
    }

    pub fn with_images(mut self, additional_images: impl IntoIterator<Item = PathBuf>) -> Self {
        self.images.extend(additional_images);
        self
    }

    pub fn write_to_shot(
        &self,
        ledger: &Ledger,
        shot: &Evolution,
        events: &EventBus,
    ) -> Result<Vec<String>, IntentError> {
        if self.images.len() > MAX_IMAGES {
            return Err(IntentError::Invalid(
                "a Shot accepts at most eight reference images; no attachment was written".into(),
            ));
        }
        let mut prepared = Vec::new();
        let mut used_names = BTreeSet::new();
        let mut used_digests = BTreeSet::<Bytes32>::new();
        for path in &self.images {
            let original_name =
                path.file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        IntentError::Invalid(format!("{} has no UTF-8 filename", path.display()))
                    })?;
            validate_image_name(original_name)?;
            if !used_names.insert(original_name.to_ascii_lowercase()) {
                return Err(IntentError::Invalid(
                    "image names collide on Apple filesystems".into(),
                ));
            }
            let contents = read_bounded_image(path)?;
            if !used_digests.insert(sha256(&contents)) {
                return Err(IntentError::Invalid(
                    "reference images must not repeat content".into(),
                ));
            }
            prepared.push((original_name.to_owned(), contents));
        }

        ledger.write_evolution_file(shot, "prompt.md", self.prompt.as_bytes())?;
        let mut copied_names = Vec::with_capacity(prepared.len());
        for (target_name, contents) in prepared {
            ledger.write_evolution_file(shot, Path::new("images").join(&target_name), &contents)?;
            copied_names.push(target_name.clone());
            events.emit(Event::status(format!(
                "attached {target_name} · {} of 8",
                copied_names.len()
            )));
        }
        Ok(copied_names)
    }
}

fn validate_image_name(name: &str) -> Result<(), IntentError> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 255
        || !name.is_ascii()
        || name.starts_with('.')
        || name.ends_with('.')
        || name.ends_with(' ')
        || bytes
            .iter()
            .any(|byte| byte.is_ascii_control() || matches!(*byte, b'/' | b'\\' | b':'))
        || !matches!(
            Path::new(name).components().collect::<Vec<_>>().as_slice(),
            [std::path::Component::Normal(_)]
        )
    {
        return Err(IntentError::Invalid(
            "image name is not one safe portable component".into(),
        ));
    }
    Ok(())
}

fn read_bounded_image(path: &Path) -> Result<Vec<u8>, IntentError> {
    let path_metadata = fs::symlink_metadata(path)?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.is_file()
        || path_metadata.len() > MAX_IMAGE_BYTES
    {
        return Err(IntentError::Invalid(format!(
            "{} is not a regular file of at most 64 MiB",
            path.display()
        )));
    }
    read_bounded_regular_file(path, MAX_IMAGE_BYTES).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            IntentError::Invalid(format!("{} changed while it was read", path.display()))
        } else {
            IntentError::Io(error)
        }
    })
}

fn is_supported_text(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["md", "markdown", "txt"]
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
        && path.is_file()
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            IMAGE_EXTENSIONS
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

#[derive(Debug)]
struct Token {
    start: usize,
    end: usize,
    value: String,
}

fn path_tokens(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut start = None;
    let mut value = String::new();
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in input.char_indices() {
        if start.is_none() {
            if character.is_whitespace() {
                continue;
            }
            start = Some(index);
        }

        if escaped {
            value.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' && quote != Some('\'') {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            } else {
                value.push(character);
            }
            continue;
        }
        if character == '\'' || character == '"' {
            quote = Some(character);
            continue;
        }
        if character.is_whitespace() {
            tokens.push(Token {
                start: start.take().unwrap(),
                end: index,
                value: std::mem::take(&mut value),
            });
        } else {
            value.push(character);
        }
    }
    if let Some(start) = start {
        if escaped {
            value.push('\\');
        }
        tokens.push(Token {
            start,
            end: input.len(),
            value,
        });
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_quoted_and_drag_escaped_paths_without_rewriting_the_prompt() {
        let intent = Intent::parse(
            "Build this\n'/tmp/first mockup.PNG' with /tmp/detail\\ view.jpeg\nand keep this.",
        );
        assert_eq!(
            intent.images,
            [
                PathBuf::from("/tmp/first mockup.PNG"),
                PathBuf::from("/tmp/detail view.jpeg")
            ]
        );
        assert_eq!(intent.prompt, "Build this\n with \nand keep this.");
    }

    #[test]
    fn unrelated_absolute_paths_remain_prompt_text() {
        let intent = Intent::parse("  Read /tmp/notes.md and build an app.\n");
        assert!(intent.images.is_empty());
        assert_eq!(intent.prompt, "  Read /tmp/notes.md and build an app.\n");
    }

    #[test]
    fn dropped_markdown_is_expanded_even_beside_existing_prose() {
        let temporary = tempfile::tempdir().unwrap();
        let document = temporary.path().join("MASTER_PROMPT.md");
        fs::write(&document, "Make the quiet idea tangible.").unwrap();
        let intent = Intent::parse_dropped(&format!(
            "Keep this preface. '{}' Keep this ending.",
            document.display()
        ));
        assert_eq!(
            intent.prompt,
            "Keep this preface. Make the quiet idea tangible. Keep this ending."
        );
        assert!(intent.images.is_empty());
    }

    #[test]
    fn rejects_a_ninth_image_before_writing_any_intention_input() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("ledger"));
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        let shot = ledger.reserve_evolution("press", None).unwrap();
        let mut paths = Vec::new();
        for number in 1..=9 {
            let path = temporary.path().join(format!("{number}.png"));
            fs::write(&path, [number]).unwrap();
            paths.push(path);
        }
        let intent = Intent {
            prompt: "Make it.".into(),
            images: paths,
        };
        let bus = EventBus::default();
        let error = intent.write_to_shot(&ledger, &shot, &bus).unwrap_err();
        assert!(error.to_string().contains("at most eight"));
        assert!(!shot.prompt_path().exists());
        assert_eq!(fs::read_dir(shot.images_path()).unwrap().count(), 0);
    }
}
