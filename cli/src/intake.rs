use crossterm::cursor::{Hide, MoveToColumn, MoveUp, Show};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use tohseno_engine::gates::intent::{Intent, MAX_IMAGES};
use tohseno_engine::{HarnessOption, HarnessSelection};

pub struct CreateIntake {
    pub prompt: String,
    pub images: Vec<PathBuf>,
    pub harness: String,
    pub model: String,
}

/// Presents the single-screen Shot composer used by interactive `create`.
/// The selected harness was already preflighted by the caller, so entering
/// raw mode never hides an installation or authentication error.
pub fn collect_create(
    harnesses: &[HarnessOption],
    selected: &HarnessSelection,
    images: Vec<PathBuf>,
) -> io::Result<CreateIntake> {
    let choices = harnesses
        .iter()
        .filter(|harness| {
            harness.installed
                && !harness.models.is_empty()
                && harness.routes.iter().any(|route| route.available)
        })
        .cloned()
        .collect::<Vec<_>>();
    Composer::new(io::stdout(), choices, selected, images)?.read()
}

/// Plain text intake remains available for piped create/evolve requests and
/// for evolve's existing interactive instruction box.
pub fn collect() -> io::Result<String> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        MultilineBox::new(io::stdout()).read()
    } else {
        let mut prompt = String::new();
        io::stdin().read_to_string(&mut prompt)?;
        Ok(prompt)
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter(writer: &mut impl Write) -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(writer, EnableBracketedPaste, Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, DisableBracketedPaste, Show);
        let _ = terminal::disable_raw_mode();
    }
}

struct Composer<W> {
    writer: W,
    prompt: String,
    images: Vec<PathBuf>,
    harnesses: Vec<HarnessOption>,
    harness_index: usize,
    model_index: usize,
    notice: Option<String>,
    rendered_height: u16,
}

impl<W: Write> Composer<W> {
    fn new(
        writer: W,
        harnesses: Vec<HarnessOption>,
        selected: &HarnessSelection,
        images: Vec<PathBuf>,
    ) -> io::Result<Self> {
        if harnesses.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no supported coding harness is ready",
            ));
        }
        if images.len() > MAX_IMAGES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a Shot accepts at most eight image references",
            ));
        }
        let harness_index = harnesses
            .iter()
            .position(|harness| harness.id == selected.harness)
            .unwrap_or(0);
        let model_index = harnesses[harness_index]
            .models
            .iter()
            .position(|model| model.id == selected.model)
            .or_else(|| {
                harnesses[harness_index]
                    .models
                    .iter()
                    .position(|model| model.is_default)
            })
            .unwrap_or(0);
        Ok(Self {
            writer,
            prompt: String::new(),
            images,
            harnesses,
            harness_index,
            model_index,
            notice: None,
            rendered_height: 0,
        })
    }

    fn read(mut self) -> io::Result<CreateIntake> {
        let _guard = TerminalGuard::enter(&mut self.writer)?;
        self.draw()?;
        loop {
            match event::read()? {
                Event::Paste(value) => {
                    self.absorb_paste(&value);
                    self.draw()?;
                }
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Enter
                        if key
                            .modifiers
                            .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT) =>
                    {
                        self.prompt.push('\n');
                        self.notice = None;
                        self.draw()?;
                    }
                    KeyCode::Enter => {
                        if self.prompt.trim().is_empty() {
                            self.notice =
                                Some("Describe the app before running the Shot preview.".into());
                            self.draw()?;
                            continue;
                        }
                        let harness = &self.harnesses[self.harness_index];
                        let model = &harness.models[self.model_index];
                        writeln!(self.writer)?;
                        return Ok(CreateIntake {
                            prompt: self.prompt,
                            images: self.images,
                            harness: harness.id.clone(),
                            model: model.id.clone(),
                        });
                    }
                    KeyCode::Backspace => {
                        self.prompt.pop();
                        self.notice = None;
                        self.draw()?;
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Err(io::Error::new(
                            io::ErrorKind::Interrupted,
                            "intake interrupted",
                        ));
                    }
                    KeyCode::Char(character) => {
                        self.prompt.push(character);
                        self.notice = None;
                        self.draw()?;
                    }
                    KeyCode::Tab => {
                        self.prompt.push_str("    ");
                        self.notice = None;
                        self.draw()?;
                    }
                    KeyCode::Up => {
                        self.harness_index = previous(self.harness_index, self.harnesses.len());
                        self.select_default_model();
                        self.notice = None;
                        self.draw()?;
                    }
                    KeyCode::Down => {
                        self.harness_index = next(self.harness_index, self.harnesses.len());
                        self.select_default_model();
                        self.notice = None;
                        self.draw()?;
                    }
                    KeyCode::Left => {
                        let models = self.harnesses[self.harness_index].models.len();
                        self.model_index = previous(self.model_index, models);
                        self.notice = None;
                        self.draw()?;
                    }
                    KeyCode::Right => {
                        let models = self.harnesses[self.harness_index].models.len();
                        self.model_index = next(self.model_index, models);
                        self.notice = None;
                        self.draw()?;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn absorb_paste(&mut self, value: &str) {
        let parsed = Intent::parse(value);
        let pasted_images = !parsed.images.is_empty();
        let replaced_by_document = !pasted_images && parsed.prompt != value;
        let mut new_images = parsed
            .images
            .into_iter()
            .filter(|image| !self.images.contains(image))
            .collect::<Vec<_>>();
        new_images.dedup();
        let available = MAX_IMAGES.saturating_sub(self.images.len());
        let image_count = new_images.len();
        for image in new_images.into_iter().take(available) {
            self.images.push(image);
        }
        if image_count > available {
            self.notice = Some("Eight image references are already attached.".into());
        } else {
            self.notice = None;
        }

        if replaced_by_document && !self.prompt.trim().is_empty() {
            self.prompt.push_str(value);
        } else {
            self.prompt.push_str(&parsed.prompt);
        }
    }

    fn select_default_model(&mut self) {
        self.model_index = self.harnesses[self.harness_index]
            .models
            .iter()
            .position(|model| model.is_default)
            .unwrap_or(0);
    }

    fn draw(&mut self) -> io::Result<()> {
        if self.rendered_height > 0 {
            queue!(self.writer, MoveUp(self.rendered_height))?;
        }
        let terminal_width = terminal::size().map(|size| size.0).unwrap_or(80);
        let lines = self.render_lines(terminal_width.clamp(32, 100) as usize);
        for line in &lines {
            queue!(self.writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
            writeln!(self.writer, "{line}")?;
        }
        self.writer.flush()?;
        self.rendered_height = lines.len() as u16;
        Ok(())
    }

    fn render_lines(&self, width: usize) -> Vec<String> {
        let inner = width - 4;
        let mut lines = vec![truncate(
            "Describe your app (or drop the MASTER_PROMPT.md)",
            width,
        )];
        lines.push(format!("┌{}┐", "─".repeat(width - 2)));
        let mut prompt_lines = wrap(&self.prompt, inner);
        while prompt_lines.len() < 3 {
            prompt_lines.push(String::new());
        }
        lines.extend(
            prompt_lines
                .into_iter()
                .map(|line| format!("│ {line:<inner$} │")),
        );
        lines.push(format!("├{}┤", "─".repeat(width - 2)));
        lines.push(framed("Drag up to 8 image references", inner));
        lines.extend(self.render_image_slots(inner));
        if let Some(notice) = &self.notice {
            lines.push(framed(&truncate(notice, inner), inner));
        }
        lines.push(format!("├{}┤", "─".repeat(width - 2)));
        let harness = &self.harnesses[self.harness_index];
        let model = &harness.models[self.model_index];
        let controls = format!("↑↓ {} · ←→ {}", harness.label, model.label);
        let action = "Enter runs Shot preview";
        if action.chars().count() + controls.chars().count() + 1 <= inner {
            lines.push(framed(&left_right(action, &controls, inner), inner));
        } else {
            lines.push(framed(action, inner));
            lines.push(framed(&right(&truncate(&controls, inner), inner), inner));
        }
        lines.push(format!("└{}┘", "─".repeat(width - 2)));
        lines
    }

    fn render_image_slots(&self, inner: usize) -> Vec<String> {
        let per_row = if inner >= 71 {
            8
        } else if inner >= 39 {
            4
        } else {
            2
        };
        (0..MAX_IMAGES)
            .collect::<Vec<_>>()
            .chunks(per_row)
            .map(|indexes| {
                let cell_width = (inner - indexes.len().saturating_sub(1)) / indexes.len();
                let cells = indexes
                    .iter()
                    .map(|index| {
                        let name = self
                            .images
                            .get(*index)
                            .and_then(|path| path.file_name())
                            .and_then(|name| name.to_str())
                            .unwrap_or("+");
                        let inside = cell_width.saturating_sub(2);
                        let label = truncate(&format!("{} {name}", index + 1), inside);
                        format!("[{label:<inside$}]")
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                framed(&cells, inner)
            })
            .collect()
    }
}

struct MultilineBox<W> {
    writer: W,
    prompt: String,
    rendered_height: u16,
}

impl<W: Write> MultilineBox<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            prompt: String::new(),
            rendered_height: 0,
        }
    }

    fn read(mut self) -> io::Result<String> {
        let _guard = TerminalGuard::enter(&mut self.writer)?;
        self.draw()?;
        loop {
            match event::read()? {
                Event::Paste(value) => {
                    self.prompt.push_str(&value);
                    self.draw()?;
                }
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Enter
                        if key
                            .modifiers
                            .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT) =>
                    {
                        self.prompt.push('\n');
                        self.draw()?;
                    }
                    KeyCode::Enter => {
                        writeln!(self.writer)?;
                        return Ok(self.prompt);
                    }
                    KeyCode::Backspace => {
                        self.prompt.pop();
                        self.draw()?;
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Err(io::Error::new(
                            io::ErrorKind::Interrupted,
                            "intake interrupted",
                        ));
                    }
                    KeyCode::Char(character) => {
                        self.prompt.push(character);
                        self.draw()?;
                    }
                    KeyCode::Tab => {
                        self.prompt.push_str("    ");
                        self.draw()?;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn draw(&mut self) -> io::Result<()> {
        if self.rendered_height > 0 {
            queue!(self.writer, MoveUp(self.rendered_height))?;
        }
        let terminal_width = terminal::size().map(|size| size.0).unwrap_or(80);
        let width = terminal_width.clamp(32, 100) as usize;
        let inner = width - 4;
        let visual_lines = wrap(&self.prompt, inner);
        let top_label = " describe the app ";
        let top = format!(
            "┌{top_label}{}┐",
            "─".repeat(width.saturating_sub(top_label.chars().count() + 2))
        );
        let footer = " Enter submits · modified Enter adds a line ";
        let bottom = format!(
            "└{}{}┘",
            truncate(footer, width - 2),
            "─".repeat(width.saturating_sub(footer.chars().count() + 2))
        );

        let mut lines = vec![top];
        lines.extend(
            visual_lines
                .iter()
                .map(|line| format!("│ {line:<inner$} │")),
        );
        lines.push(bottom);
        for line in &lines {
            queue!(self.writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
            writeln!(self.writer, "{line}")?;
        }
        self.writer.flush()?;
        self.rendered_height = lines.len() as u16;
        Ok(())
    }
}

fn framed(value: &str, inner: usize) -> String {
    let value = truncate(value, inner);
    format!("│ {value:<inner$} │")
}

fn left_right(left: &str, right: &str, width: usize) -> String {
    format!(
        "{left}{}{right}",
        " ".repeat(width - left.chars().count() - right.chars().count())
    )
}

fn right(value: &str, width: usize) -> String {
    format!("{}{value}", " ".repeat(width - value.chars().count()))
}

fn truncate(value: &str, width: usize) -> String {
    let characters = value.chars().collect::<Vec<_>>();
    if characters.len() <= width {
        return value.into();
    }
    if width <= 1 {
        return "…".chars().take(width).collect();
    }
    characters[..width - 1]
        .iter()
        .chain(std::iter::once(&'…'))
        .collect()
}

fn next(index: usize, length: usize) -> usize {
    if length == 0 {
        0
    } else {
        (index + 1) % length
    }
}

fn previous(index: usize, length: usize) -> usize {
    if length == 0 {
        0
    } else {
        (index + length - 1) % length
    }
}

fn wrap(value: &str, width: usize) -> Vec<String> {
    let mut output = Vec::new();
    for logical_line in value.split('\n') {
        let characters: Vec<char> = logical_line.chars().collect();
        if characters.is_empty() {
            output.push(String::new());
        } else {
            for chunk in characters.chunks(width) {
                output.push(chunk.iter().collect());
            }
        }
    }
    if output.is_empty() {
        output.push(String::new());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use tohseno_engine::harness::{
        AttachmentBehavior, AuthenticationStatus, HarnessModel, HarnessRoute,
    };

    fn harness(id: &str, label: &str, models: &[(&str, &str, bool)]) -> HarnessOption {
        HarnessOption {
            id: id.into(),
            label: label.into(),
            command: id.into(),
            installed: true,
            selected: id == "codex",
            authentication: AuthenticationStatus::Authenticated,
            models: models
                .iter()
                .map(|(id, label, is_default)| HarnessModel {
                    id: (*id).into(),
                    label: (*label).into(),
                    is_default: *is_default,
                })
                .collect(),
            routes: vec![HarnessRoute {
                id: "subscription".into(),
                label: "Subscription".into(),
                billing: "existing".into(),
                available: true,
                estimated_additional_cost_usd: Some(0.0),
                cost_estimation: true,
            }],
            attachment_behavior: AttachmentBehavior::NativeImageArguments,
            completion_detection: "test".into(),
        }
    }

    fn composer() -> Composer<Vec<u8>> {
        let harnesses = vec![
            harness("codex", "Codex", &[("default", "Configured default", true)]),
            harness(
                "claude-code",
                "Claude Code",
                &[("sonnet", "Sonnet", true), ("opus", "Opus", false)],
            ),
        ];
        Composer::new(
            Vec::new(),
            harnesses,
            &HarnessSelection {
                harness: "codex".into(),
                model: "default".into(),
                route: "subscription".into(),
            },
            Vec::new(),
        )
        .unwrap()
    }

    #[test]
    fn wrapping_preserves_explicit_newlines_and_unicode() {
        assert_eq!(wrap("hello\nenso ◯", 5), ["hello", "enso ", "◯"]);
    }

    #[test]
    fn composer_shows_eight_slots_and_direct_controls() {
        let lines = composer().render_lines(100);
        let rendered = lines.join("\n");
        assert!(rendered.contains("Describe your app (or drop the MASTER_PROMPT.md)"));
        assert!(rendered.contains("Drag up to 8 image references"));
        for number in 1..=8 {
            assert!(rendered.contains(&format!("[{number} +")));
        }
        assert!(rendered.contains("Enter runs Shot preview"));
        assert!(rendered.contains("↑↓ Codex · ←→ Configured default"));
    }

    #[test]
    fn composer_stays_inside_narrow_and_wide_terminals() {
        for width in [32, 56, 80, 100] {
            for line in composer().render_lines(width) {
                assert!(
                    line.chars().count() <= width,
                    "{width}-column render overflowed: {line}"
                );
            }
        }
    }

    #[test]
    fn arrows_wrap_harnesses_and_models() {
        let mut composer = composer();
        composer.harness_index = next(composer.harness_index, composer.harnesses.len());
        composer.select_default_model();
        assert_eq!(composer.harnesses[composer.harness_index].id, "claude-code");
        assert_eq!(
            composer.harnesses[composer.harness_index].models[composer.model_index].id,
            "sonnet"
        );
        composer.model_index = previous(
            composer.model_index,
            composer.harnesses[composer.harness_index].models.len(),
        );
        assert_eq!(
            composer.harnesses[composer.harness_index].models[composer.model_index].id,
            "opus"
        );
    }

    #[test]
    fn pasted_images_fill_slots_without_becoming_prompt_text() {
        let mut composer = composer();
        composer.absorb_paste("/tmp/home\\ screen.png /tmp/detail.jpeg");
        assert_eq!(
            composer.images,
            [
                PathBuf::from("/tmp/home screen.png"),
                PathBuf::from("/tmp/detail.jpeg")
            ]
        );
        assert!(composer.prompt.trim().is_empty());
        let rendered = composer.render_lines(100).join("\n");
        assert!(rendered.contains("home"));
        assert!(rendered.contains("detail"));
    }

    #[test]
    fn dropped_master_prompt_becomes_the_description() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("MASTER_PROMPT.md");
        std::fs::write(&path, "Build one calm thing.").unwrap();
        let mut composer = composer();
        composer.absorb_paste(&format!("'{}'", path.display()));
        assert_eq!(composer.prompt, "Build one calm thing.");
        assert!(composer.images.is_empty());
    }
}
