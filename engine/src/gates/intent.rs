use crate::events::{Event, EventBus};
use crate::ledger::{Ledger, LedgerError, Shot};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_IMAGES: usize = 8;
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
}

impl std::fmt::Display for IntentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Ledger(error) => write!(f, "{error}"),
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
        for token in tokens {
            let path = PathBuf::from(token.value);
            if path.is_absolute() && is_supported_image(&path) {
                ranges.push(token.start..token.end);
                images.push(path);
            }
        }

        let mut prompt = submission.to_owned();
        for range in ranges.into_iter().rev() {
            prompt.replace_range(range, "");
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
        shot: &Shot,
        events: &EventBus,
    ) -> Result<Vec<String>, IntentError> {
        ledger.write_shot_file(shot, "prompt.md", self.prompt.as_bytes())?;
        let mut copied_names = Vec::new();
        let mut used_names = HashSet::new();

        for (index, path) in self.images.iter().enumerate() {
            let original_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("image");
            if index >= MAX_IMAGES {
                events.emit(Event::status(format!("ignored {original_name} · 8 of 8")));
                continue;
            }
            let target_name = unique_name(original_name, &mut used_names);
            let contents = fs::read(path)?;
            ledger.write_shot_file(shot, Path::new("images").join(&target_name), &contents)?;
            copied_names.push(target_name.clone());
            events.emit(Event::status(format!(
                "attached {target_name} · {} of 8",
                copied_names.len()
            )));
        }
        Ok(copied_names)
    }
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

fn unique_name(original: &str, used: &mut HashSet<String>) -> String {
    if used.insert(original.to_owned()) {
        return original.to_owned();
    }
    let path = Path::new(original);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("image");
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    for ordinal in 2.. {
        let candidate = if extension.is_empty() {
            format!("{stem}-{ordinal}")
        } else {
            format!("{stem}-{ordinal}.{extension}")
        };
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!()
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
    fn writes_eight_images_and_ignores_the_ninth() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("ledger"));
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        let shot = ledger.reserve_shot("press", None).unwrap();
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
        let copied = intent.write_to_shot(&ledger, &shot, &bus).unwrap();
        assert_eq!(copied.len(), 8);
        assert_eq!(
            fs::read_dir(shot.images_path()).unwrap().count(),
            MAX_IMAGES
        );
        assert_eq!(fs::read_to_string(shot.prompt_path()).unwrap(), "Make it.");
    }
}
