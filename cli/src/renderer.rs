use std::io::{self, Write};
use tohseno_engine::events::Event;
use tokio::sync::broadcast;

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const ACCENT: &str = "\x1b[1;36m";

pub struct Renderer<W> {
    writer: W,
    color: bool,
}

impl<W: Write> Renderer<W> {
    pub fn new(writer: W, color: bool) -> Self {
        Self { writer, color }
    }

    pub async fn follow(
        mut self,
        mut receiver: broadcast::Receiver<Event>,
    ) -> Result<(), io::Error> {
        loop {
            match receiver.recv().await {
                Ok(event) => self.render(&event)?,
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    self.render(&Event::status(format!(
                        "the display skipped {skipped} earlier lines."
                    )))?;
                }
                Err(broadcast::error::RecvError::Closed) => return Ok(()),
            }
        }
    }

    pub fn render(&mut self, event: &Event) -> Result<(), io::Error> {
        let (prefix, suffix, indent, message) = match event {
            Event::Status(message) => (DIM, RESET, "", message.as_str()),
            Event::Handoff(message) => (BOLD, RESET, "", message.as_str()),
            Event::Result(message) => (ACCENT, RESET, "", message.as_str()),
            Event::HarnessLine(message) => (DIM, RESET, "  ", message.as_str()),
        };

        // A harness line can contain embedded newlines. Rendering every physical
        // line separately preserves the engine's one-event/one-voice discipline.
        for line in message.lines().chain(message.is_empty().then_some("")) {
            if self.color {
                writeln!(self.writer, "{prefix}{indent}{line}{suffix}")?;
            } else {
                writeln!(self.writer, "{indent}{line}")?;
            }
        }
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_voices_have_distinct_plain_shapes() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, false);
        renderer.render(&Event::status("building shot 3…")).unwrap();
        renderer
            .render(&Event::handoff("Plug in your iPhone with a cable."))
            .unwrap();
        renderer
            .render(&Event::result("shot 3 of press is on your phone."))
            .unwrap();
        renderer
            .render(&Event::harness_line("Writing project\nBuilding target"))
            .unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            concat!(
                "building shot 3…\n",
                "Plug in your iPhone with a cable.\n",
                "shot 3 of press is on your phone.\n",
                "  Writing project\n",
                "  Building target\n"
            )
        );
    }

    #[test]
    fn color_maps_to_status_handoff_result_and_theater() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, true);
        renderer.render(&Event::status("status")).unwrap();
        renderer.render(&Event::handoff("handoff.")).unwrap();
        renderer.render(&Event::result("result.")).unwrap();
        renderer.render(&Event::harness_line("theater")).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("\x1b[2mstatus\x1b[0m"));
        assert!(output.contains("\x1b[1mhandoff.\x1b[0m"));
        assert!(output.contains("\x1b[1;36mresult.\x1b[0m"));
        assert!(output.contains("\x1b[2m  theater\x1b[0m"));
    }
}
