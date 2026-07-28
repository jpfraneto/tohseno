use crate::config::HarnessConfig;
use crate::events::{Event, EventBus};
use crate::ledger::{Ledger, LedgerError, Shot};
use serde_json::Value;
use std::ffi::OsString;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub struct Harness {
    config: HarnessConfig,
    events: EventBus,
}

impl Harness {
    pub fn new(config: HarnessConfig, events: EventBus) -> Self {
        Self { config, events }
    }

    pub async fn wait_until_available(&self) -> Result<HarnessMode, HarnessError> {
        let mut announced = false;
        loop {
            match self.detect_mode().await {
                Ok(mode) => return Ok(mode),
                Err(HarnessError::Missing(_)) => {
                    if !announced {
                        self.events.emit(Event::handoff(
                            "Run `curl -fsSL https://claude.ai/install.sh | bash`, then return here.",
                        ));
                        announced = true;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub async fn detect_mode(&self) -> Result<HarnessMode, HarnessError> {
        let words = split_command(&self.config.command)?;
        let (program, configured_args) = words.split_first().ok_or(HarnessError::EmptyCommand)?;
        let output = Command::new(program)
            .args(configured_args)
            .arg("--help")
            .output()
            .await
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    HarnessError::Missing(program.to_string_lossy().into_owned())
                } else {
                    HarnessError::Io(error)
                }
            })?;
        let help = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if help.contains("--print")
            && help.contains("stream-json")
            && help.contains("--dangerously-skip-permissions")
        {
            Ok(HarnessMode::ClaudeStreamJson)
        } else {
            Ok(HarnessMode::Generic)
        }
    }

    pub async fn run(
        &self,
        ledger: &Ledger,
        shot: &Shot,
        mode: HarnessMode,
        instruction: &str,
    ) -> Result<(), HarnessError> {
        let words = split_command(&self.config.command)?;
        let (program, configured_args) = words.split_first().ok_or(HarnessError::EmptyCommand)?;
        let mut command = Command::new(program);
        command
            .args(configured_args)
            .current_dir(&shot.path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        match mode {
            HarnessMode::ClaudeStreamJson => {
                // These flags are selected only after this installed binary's
                // `--help` advertises them; Xcode/agent CLIs are not assumed.
                command.args([
                    OsString::from("--print"),
                    OsString::from("--output-format"),
                    OsString::from("stream-json"),
                    OsString::from("--verbose"),
                    OsString::from("--dangerously-skip-permissions"),
                    OsString::from("--no-session-persistence"),
                    OsString::from(instruction),
                ]);
            }
            HarnessMode::Generic => {
                command.arg(instruction);
            }
        }

        let mut child = command.spawn().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                HarnessError::Missing(program.to_string_lossy().into_owned())
            } else {
                HarnessError::Io(error)
            }
        })?;
        let stdout = child.stdout.take().ok_or(HarnessError::PipeMissing)?;
        let stderr = child.stderr.take().ok_or(HarnessError::PipeMissing)?;
        let (sender, mut receiver) = mpsc::channel::<(bool, String)>(128);
        let stdout_sender = sender.clone();
        let stdout_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Some(line) = lines.next_line().await? {
                if stdout_sender.send((false, line)).await.is_err() {
                    break;
                }
            }
            Ok::<(), std::io::Error>(())
        });
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Some(line) = lines.next_line().await? {
                if sender.send((true, line)).await.is_err() {
                    break;
                }
            }
            Ok::<(), std::io::Error>(())
        });

        while let Some((is_stderr, line)) = receiver.recv().await {
            ledger
                .append_shot_log(
                    shot,
                    "harness.log",
                    format!("{}{line}\n", if is_stderr { "[stderr] " } else { "" }).as_bytes(),
                )
                .map_err(HarnessError::Ledger)?;
            if let Some(display) = display_line(mode, &line, is_stderr) {
                for physical_line in display.lines() {
                    if !physical_line.trim().is_empty() {
                        self.events.emit(Event::harness_line(physical_line));
                    }
                }
            }
        }
        stdout_task.await.map_err(HarnessError::Join)??;
        stderr_task.await.map_err(HarnessError::Join)??;
        let status = child.wait().await.map_err(HarnessError::Io)?;
        if status.success() {
            Ok(())
        } else {
            Err(HarnessError::Exit(status.code()))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HarnessMode {
    ClaudeStreamJson,
    Generic,
}

fn display_line(mode: HarnessMode, line: &str, is_stderr: bool) -> Option<String> {
    if is_stderr || mode == HarnessMode::Generic {
        return Some(line.to_owned());
    }
    let value: Value = serde_json::from_str(line).ok()?;
    match value.get("type").and_then(Value::as_str) {
        Some("assistant") => {
            let content = value
                .pointer("/message/content")
                .and_then(Value::as_array)?;
            let output = content
                .iter()
                .filter_map(|block| match block.get("type").and_then(Value::as_str) {
                    Some("text") => block.get("text").and_then(Value::as_str).map(str::to_owned),
                    Some("tool_use") => block
                        .get("name")
                        .and_then(Value::as_str)
                        .map(|name| format!("using {name}…")),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            (!output.is_empty()).then_some(output)
        }
        Some("content_block_delta") => value
            .pointer("/delta/text")
            .and_then(Value::as_str)
            .map(str::to_owned),
        Some("result") => value
            .get("result")
            .and_then(Value::as_str)
            .filter(|result| !result.is_empty())
            .map(str::to_owned),
        _ => None,
    }
}

fn split_command(command: &str) -> Result<Vec<OsString>, HarnessError> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in command.chars() {
        if escaped {
            current.push(character);
            escaped = false;
        } else if character == '\\' && quote != Some('\'') {
            escaped = true;
        } else if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            } else {
                current.push(character);
            }
        } else if character == '\'' || character == '"' {
            quote = Some(character);
        } else if character.is_whitespace() {
            if !current.is_empty() {
                words.push(OsString::from(std::mem::take(&mut current)));
            }
        } else {
            current.push(character);
        }
    }
    if escaped {
        current.push('\\');
    }
    if quote.is_some() {
        return Err(HarnessError::InvalidCommand(
            "unterminated quote in harness.command".into(),
        ));
    }
    if !current.is_empty() {
        words.push(OsString::from(current));
    }
    Ok(words)
}

#[derive(Debug)]
pub enum HarnessError {
    Io(std::io::Error),
    Ledger(LedgerError),
    Missing(String),
    EmptyCommand,
    InvalidCommand(String),
    PipeMissing,
    Exit(Option<i32>),
    Join(tokio::task::JoinError),
}

impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Ledger(error) => write!(f, "{error}"),
            Self::Missing(command) => write!(f, "harness not found: {command}"),
            Self::EmptyCommand => write!(f, "harness.command is empty"),
            Self::InvalidCommand(message) => write!(f, "{message}"),
            Self::PipeMissing => write!(f, "harness output pipe is unavailable"),
            Self::Exit(code) => write!(f, "harness exited unsuccessfully ({code:?})"),
            Self::Join(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for HarnessError {}

impl From<std::io::Error> for HarnessError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_commands_may_contain_quoted_arguments() {
        assert_eq!(
            split_command("agent --profile 'ios press'").unwrap(),
            ["agent", "--profile", "ios press"]
                .into_iter()
                .map(OsString::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn parses_claude_text_without_showing_json_protocol() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Writing the app."},{"type":"tool_use","name":"Write"}]}}"#;
        assert_eq!(
            display_line(HarnessMode::ClaudeStreamJson, line, false).unwrap(),
            "Writing the app.\nusing Write…"
        );
    }
}
