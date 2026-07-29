use crossterm::cursor::{Hide, MoveToColumn, MoveUp, Show};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use std::io::{self, IsTerminal, Read, Write};
use tohseno_engine::{Event as EngineEvent, EventBus};

pub fn collect(events: &EventBus) -> io::Result<String> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        events.emit(EngineEvent::status("what you write stays on this Mac."));
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
}
