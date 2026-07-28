use crossterm::cursor::{Hide, MoveToColumn, MoveUp, Show};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;
use tohseno_engine::{Event as EngineEvent, EventBus, HarnessOption};

pub fn choose_harness(
    options: &[HarnessOption],
    requested: Option<&str>,
    events: &EventBus,
) -> io::Result<Option<String>> {
    let installed = options
        .iter()
        .filter(|option| option.installed)
        .collect::<Vec<_>>();

    if let Some(requested) = requested {
        let option = options
            .iter()
            .find(|option| option.id.eq_ignore_ascii_case(requested))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported coding agent: {requested}"),
                )
            })?;
        if !option.installed {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is not installed", option.label),
            ));
        }
        events.emit(EngineEvent::status(format!("using {}.", option.label)));
        return Ok(Some(option.id.clone()));
    }

    if installed.is_empty() {
        events.emit(EngineEvent::status(
            "no supported coding agents were detected.",
        ));
        return Ok(None);
    }

    let default = installed
        .iter()
        .position(|option| option.selected)
        .unwrap_or(0);
    let chosen = if installed.len() > 1 && io::stdin().is_terminal() && io::stdout().is_terminal() {
        events.emit(EngineEvent::handoff(
            "Choose a coding agent with ↑ or ↓, then press Enter.",
        ));
        HarnessMenu::new(io::stdout(), &installed, default).read()?
    } else {
        default
    };
    let option = installed[chosen];
    events.emit(EngineEvent::status(format!("using {}.", option.label)));
    Ok(Some(option.id.clone()))
}

pub fn collect(prompt_file: Option<&Path>, events: &EventBus) -> io::Result<String> {
    if let Some(path) = prompt_file {
        return fs::read_to_string(path);
    }

    let automatic = std::env::current_dir()?.join("MASTER_PROMPT.md");
    if automatic.is_file() {
        events.emit(EngineEvent::handoff(
            "Press y to use MASTER_PROMPT.md or n to type this shot.",
        ));
        if confirm()? {
            return fs::read_to_string(automatic);
        }
    }

    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        MultilineBox::new(io::stdout()).read()
    } else {
        let mut prompt = String::new();
        io::stdin().read_to_string(&mut prompt)?;
        Ok(prompt)
    }
}

fn confirm() -> io::Result<bool> {
    if !io::stdin().is_terminal() {
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        return Ok(answer.trim().eq_ignore_ascii_case("y"));
    }
    terminal::enable_raw_mode()?;
    let answer = loop {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => break true,
                KeyCode::Char('n') | KeyCode::Char('N') => break false,
                _ => {}
            }
        }
    };
    terminal::disable_raw_mode()?;
    writeln!(io::stdout())?;
    Ok(answer)
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

struct HarnessMenu<'a, W> {
    writer: W,
    options: &'a [&'a HarnessOption],
    selected: usize,
    rendered_height: u16,
}

impl<'a, W: Write> HarnessMenu<'a, W> {
    fn new(writer: W, options: &'a [&'a HarnessOption], selected: usize) -> Self {
        Self {
            writer,
            options,
            selected,
            rendered_height: 0,
        }
    }

    fn read(mut self) -> io::Result<usize> {
        let _guard = TerminalGuard::enter(&mut self.writer)?;
        self.draw()?;
        loop {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Up => {
                        self.selected = self
                            .selected
                            .checked_sub(1)
                            .unwrap_or(self.options.len() - 1);
                        self.draw()?;
                    }
                    KeyCode::Down => {
                        self.selected = (self.selected + 1) % self.options.len();
                        self.draw()?;
                    }
                    KeyCode::Enter => {
                        writeln!(self.writer)?;
                        return Ok(self.selected);
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Err(io::Error::new(
                            io::ErrorKind::Interrupted,
                            "harness selection interrupted",
                        ));
                    }
                    _ => {}
                }
            }
        }
    }

    fn draw(&mut self) -> io::Result<()> {
        if self.rendered_height > 0 {
            queue!(self.writer, MoveUp(self.rendered_height))?;
        }
        for (index, option) in self.options.iter().enumerate() {
            queue!(self.writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
            let marker = if index == self.selected { "›" } else { " " };
            let current = if option.selected { " · current" } else { "" };
            writeln!(self.writer, "  {marker} {}{current}", option.label)?;
        }
        self.writer.flush()?;
        self.rendered_height = self.options.len() as u16;
        Ok(())
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
        let long_footer = " Enter submits · Shift+Enter or Option+Enter adds a line ";
        let footer = if long_footer.chars().count() > width - 2 {
            " Enter submits · modified Enter adds a line "
        } else {
            long_footer
        };
        let bottom = format!(
            "└{footer}{}┘",
            "─".repeat(width.saturating_sub(footer.chars().count() + 2))
        );

        queue!(self.writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
        writeln!(self.writer, "{top}")?;
        for line in &visual_lines {
            queue!(self.writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
            writeln!(self.writer, "│ {line:<inner$} │")?;
        }
        queue!(self.writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
        writeln!(self.writer, "{bottom}")?;
        self.writer.flush()?;
        self.rendered_height = (visual_lines.len() + 2) as u16;
        Ok(())
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

    #[test]
    fn wrapping_preserves_explicit_newlines_and_unicode() {
        assert_eq!(wrap("hello\nenso ◯", 5), ["hello", "enso ", "◯"]);
    }

    #[test]
    fn prompt_files_are_verbatim() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("PROMPT.md");
        fs::write(&path, "line one\nline two\n").unwrap();
        let prompt = collect(Some(&path), &EventBus::default()).unwrap();
        assert_eq!(prompt, "line one\nline two\n");
    }

    #[test]
    fn an_explicit_installed_harness_skips_the_picker() {
        let options = vec![HarnessOption {
            id: "codex".into(),
            label: "Codex".into(),
            command: "/usr/local/bin/codex".into(),
            installed: true,
            selected: false,
        }];
        assert_eq!(
            choose_harness(&options, Some("codex"), &EventBus::default()).unwrap(),
            Some("codex".into())
        );
    }

    #[test]
    fn an_explicit_missing_harness_is_rejected() {
        let options = vec![HarnessOption {
            id: "opencode".into(),
            label: "OpenCode".into(),
            command: "opencode".into(),
            installed: false,
            selected: false,
        }];
        let error = choose_harness(&options, Some("opencode"), &EventBus::default()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
