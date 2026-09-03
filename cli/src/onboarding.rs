use std::io::{self, BufRead, Write};

const NEXT_STEP: &str = "Press Enter for next step";

const INIT_STEPS: [&str; 6] = [
    "Run this command from the folder that contains the Xcode project you want to publish.",
    "Before connecting the project, Tohseno checks the intended iPhone's real installed-app list for the exact Companion bundle and checks its private pairing. If either is missing, run `tohseno companion install` first.",
    "Tohseno will ask Xcode to build the iOS app for Simulator. This checks the real project before anything can be published.",
    "Init keeps the source where it is, does not rewrite Git, and reserves one stable candidate ShotID for this app.",
    "When init succeeds, run `tohseno deploy`. Tohseno will inspect and package the current source, then ask your paired Companion to approve the exact release.",
    "The first approved publication is this app's one Ship. Every later approved publication is an Update.",
];

pub fn run_init<R: BufRead, W: Write>(reader: &mut R, writer: &mut W) -> io::Result<()> {
    writeln!(writer, "TOHSENO · CONNECT AN XCODE APP")?;
    writeln!(writer)?;

    for (index, step) in INIT_STEPS.iter().enumerate() {
        writeln!(writer, "[{}/{}] {step}", index + 1, INIT_STEPS.len())?;
        writeln!(writer, "{NEXT_STEP}")?;
        writer.flush()?;

        let mut response = String::new();
        if reader.read_line(&mut response)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "interactive init onboarding ended before the next step",
            ));
        }
        writeln!(writer)?;
    }

    writeln!(writer, "Connecting this Xcode app now…")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_walkthrough_advances_once_per_enter() {
        let mut input = io::Cursor::new("\n\n\n\n\n\n");
        let mut output = Vec::new();

        run_init(&mut input, &mut output).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert_eq!(output.matches(NEXT_STEP).count(), INIT_STEPS.len());
        assert!(output.contains("run `tohseno deploy`"));
        assert!(output.contains("real installed-app list"));
        assert!(output.contains("`tohseno companion install`"));
        assert!(output.contains("one Ship"));
        assert!(output.ends_with("Connecting this Xcode app now…\n"));
    }

    #[test]
    fn init_walkthrough_does_not_continue_without_human_input() {
        let mut input = io::Cursor::new("");
        let mut output = Vec::new();

        let error = run_init(&mut input, &mut output).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(
            String::from_utf8(output)
                .unwrap()
                .matches(NEXT_STEP)
                .count(),
            1
        );
    }
}
