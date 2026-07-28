use crate::config::HarnessConfig;
use crate::events::{Event, EventBus};
use crate::ledger::{Ledger, LedgerError, Shot};
use serde::Serialize;
use serde_json::Value;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Clone, Debug, Serialize)]
pub struct HarnessOption {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing)]
    pub command: String,
    pub installed: bool,
    pub selected: bool,
}

#[derive(Clone, Copy)]
struct KnownHarness {
    id: &'static str,
    label: &'static str,
    binaries: &'static [&'static str],
    home_paths: &'static [&'static str],
}

static KNOWN_HARNESSES: [KnownHarness; 5] = [
    KnownHarness {
        id: "hermes",
        label: "Hermes Agent",
        binaries: &["hermes"],
        home_paths: &[".local/bin/hermes", ".hermes/bin/hermes"],
    },
    KnownHarness {
        id: "codex",
        label: "Codex",
        binaries: &["codex"],
        home_paths: &[".local/bin/codex"],
    },
    KnownHarness {
        id: "claude",
        label: "Claude Code",
        binaries: &["claude"],
        home_paths: &[".local/bin/claude"],
    },
    KnownHarness {
        id: "grok",
        label: "Grok Build",
        binaries: &["grok", "grok-build", "grokbuild"],
        home_paths: &[".grok/bin/grok", ".local/bin/grok"],
    },
    KnownHarness {
        id: "opencode",
        label: "OpenCode",
        binaries: &["opencode"],
        home_paths: &[".opencode/bin/opencode", ".local/bin/opencode"],
    },
];

pub fn discover_harnesses(selected: &HarnessConfig) -> Vec<HarnessOption> {
    let selected_id = split_command(&selected.command)
        .ok()
        .and_then(|words| words.first().cloned())
        .and_then(|program| known_harness_for_program(&program).map(|known| known.id));
    KNOWN_HARNESSES
        .iter()
        .map(|known| {
            let executable = find_executable(known);
            HarnessOption {
                id: known.id.into(),
                label: known.label.into(),
                command: executable
                    .as_ref()
                    .map(|path| command_for_path(path))
                    .unwrap_or_else(|| known.binaries[0].into()),
                installed: executable.is_some(),
                selected: selected_id == Some(known.id),
            }
        })
        .collect()
}

pub fn selected_harness(id: &str) -> Result<HarnessConfig, HarnessError> {
    let known = KNOWN_HARNESSES
        .iter()
        .find(|known| known.id.eq_ignore_ascii_case(id))
        .ok_or_else(|| HarnessError::Unsupported(id.into()))?;
    let path =
        find_executable(known).ok_or_else(|| HarnessError::Missing(known.binaries[0].into()))?;
    let command = command_for_path(&path);
    Ok(HarnessConfig { command })
}

fn command_for_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if value
        .chars()
        .any(|character| character.is_whitespace() || matches!(character, '\\' | '"'))
    {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.into_owned()
    }
}

fn find_executable(known: &KnownHarness) -> Option<PathBuf> {
    let from_path = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .flat_map(|directory| {
                known
                    .binaries
                    .iter()
                    .map(move |binary| directory.join(binary))
            })
            .find(|candidate| is_executable(candidate))
    });
    if from_path.is_some() {
        return from_path;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    known
        .home_paths
        .iter()
        .map(|relative| home.join(relative))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn known_harness_for_program(program: &std::ffi::OsStr) -> Option<&'static KnownHarness> {
    let name = Path::new(program).file_name()?.to_string_lossy();
    KNOWN_HARNESSES.iter().find(|known| {
        known
            .binaries
            .iter()
            .any(|binary| name.eq_ignore_ascii_case(binary))
    })
}

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
                        self.events
                            .emit(Event::handoff(install_handoff(&self.config.command)));
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
        let help = command_help(program, configured_args, &[]).await?;
        let known = known_harness_for_program(program);
        match known.map(|known| known.id) {
            Some("claude")
                if help.contains("--print")
                    && help.contains("stream-json")
                    && help.contains("--dangerously-skip-permissions") =>
            {
                Ok(HarnessMode::ClaudeStreamJson)
            }
            Some("codex") => {
                let exec_help = command_help(program, configured_args, &["exec"]).await?;
                if [
                    "--json",
                    "--skip-git-repo-check",
                    "--ephemeral",
                    "--dangerously-bypass-approvals-and-sandbox",
                ]
                .iter()
                .all(|flag| exec_help.contains(flag))
                {
                    Ok(HarnessMode::CodexJson)
                } else {
                    Err(HarnessError::Incompatible("Codex".into()))
                }
            }
            Some("grok")
                if ["--single", "--output-format", "--always-approve"]
                    .iter()
                    .all(|flag| help.contains(flag)) =>
            {
                Ok(HarnessMode::GrokPlain)
            }
            Some("hermes") => {
                let chat_help = command_help(program, configured_args, &["chat"]).await?;
                if chat_help.contains("-q") || chat_help.contains("--query") {
                    Ok(HarnessMode::HermesPlain)
                } else {
                    Err(HarnessError::Incompatible("Hermes Agent".into()))
                }
            }
            Some("opencode") => {
                let run_help = command_help(program, configured_args, &["run"]).await?;
                if help.contains("run") && (help.contains("--auto") || run_help.contains("--auto"))
                {
                    Ok(HarnessMode::OpenCodePlain)
                } else {
                    Err(HarnessError::Incompatible("OpenCode".into()))
                }
            }
            Some(known) => Err(HarnessError::Incompatible(known.into())),
            None if help.contains("--print")
                && help.contains("stream-json")
                && help.contains("--dangerously-skip-permissions") =>
            {
                Ok(HarnessMode::ClaudeStreamJson)
            }
            None => Ok(HarnessMode::Generic),
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
            HarnessMode::CodexJson => {
                command.args([
                    OsString::from("exec"),
                    OsString::from("--json"),
                    OsString::from("--dangerously-bypass-approvals-and-sandbox"),
                    OsString::from("--skip-git-repo-check"),
                    OsString::from("--ephemeral"),
                    OsString::from(instruction),
                ]);
            }
            HarnessMode::GrokPlain => {
                command.args([
                    OsString::from("--single"),
                    OsString::from(instruction),
                    OsString::from("--output-format"),
                    OsString::from("plain"),
                    OsString::from("--always-approve"),
                    OsString::from("--no-memory"),
                    OsString::from("--verbatim"),
                ]);
            }
            HarnessMode::HermesPlain => {
                command.args([
                    OsString::from("chat"),
                    OsString::from("-q"),
                    OsString::from(instruction),
                ]);
            }
            HarnessMode::OpenCodePlain => {
                command.args([
                    OsString::from("run"),
                    OsString::from("--auto"),
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
    CodexJson,
    GrokPlain,
    HermesPlain,
    OpenCodePlain,
    Generic,
}

fn display_line(mode: HarnessMode, line: &str, is_stderr: bool) -> Option<String> {
    if is_stderr
        || matches!(
            mode,
            HarnessMode::Generic
                | HarnessMode::GrokPlain
                | HarnessMode::HermesPlain
                | HarnessMode::OpenCodePlain
        )
    {
        return Some(line.to_owned());
    }
    let value: Value = serde_json::from_str(line).ok()?;
    if mode == HarnessMode::CodexJson {
        return display_codex_line(&value);
    }
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

fn display_codex_line(value: &Value) -> Option<String> {
    match value.get("type").and_then(Value::as_str) {
        Some("item.completed") | Some("item.started") => {
            let item = value.get("item")?;
            match item.get("type").and_then(Value::as_str) {
                Some("agent_message") => item.get("text")?.as_str().map(str::to_owned),
                Some("command_execution") => item
                    .get("command")
                    .and_then(Value::as_str)
                    .map(|command| format!("running {command}…")),
                Some("mcp_tool_call") => {
                    let server = item.get("server").and_then(Value::as_str).unwrap_or("tool");
                    let tool = item.get("tool").and_then(Value::as_str).unwrap_or("call");
                    Some(format!("using {server}.{tool}…"))
                }
                Some("file_change") => Some("writing source…".into()),
                _ => None,
            }
        }
        Some("error") => value
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_owned),
        _ => None,
    }
}

async fn command_help(
    program: &std::ffi::OsStr,
    configured_args: &[OsString],
    subcommand: &[&str],
) -> Result<String, HarnessError> {
    let output = Command::new(program)
        .args(configured_args)
        .args(subcommand)
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
    Ok(format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn install_handoff(command: &str) -> String {
    let program = split_command(command)
        .ok()
        .and_then(|words| words.first().cloned());
    match program
        .as_deref()
        .and_then(known_harness_for_program)
        .map(|known| known.id)
    {
        Some("claude") => {
            "Run `curl -fsSL https://claude.ai/install.sh | bash`, then return here.".into()
        }
        Some("codex") => "Install Codex, then return here.".into(),
        Some("grok") => "Install Grok Build, then return here.".into(),
        Some("hermes") => "Install Hermes Agent, then return here.".into(),
        Some("opencode") => "Install OpenCode, then return here.".into(),
        _ => "Install the configured coding agent, then return here.".into(),
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
    Unsupported(String),
    Incompatible(String),
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
            Self::Unsupported(harness) => write!(f, "unsupported coding agent: {harness}"),
            Self::Incompatible(harness) => {
                write!(
                    f,
                    "{harness} does not expose the required non-interactive flags"
                )
            }
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
    fn executable_paths_with_spaces_remain_one_command_word() {
        let command = command_for_path(Path::new("/Users/App Maker/bin/codex"));
        assert_eq!(
            split_command(&command).unwrap(),
            [OsString::from("/Users/App Maker/bin/codex")]
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

    #[test]
    fn supported_agents_have_stable_public_ids() {
        assert_eq!(
            KNOWN_HARNESSES
                .iter()
                .map(|known| known.id)
                .collect::<Vec<_>>(),
            ["hermes", "codex", "claude", "grok", "opencode"]
        );
    }

    #[test]
    fn parses_codex_events_without_showing_json_protocol() {
        let line = r#"{"type":"item.completed","item":{"type":"agent_message","text":"The project builds."}}"#;
        assert_eq!(
            display_line(HarnessMode::CodexJson, line, false).unwrap(),
            "The project builds."
        );
    }

    #[tokio::test]
    async fn installed_known_agents_advertise_supported_contracts() {
        for option in discover_harnesses(&HarnessConfig::default())
            .into_iter()
            .filter(|option| option.installed)
        {
            Harness::new(
                HarnessConfig {
                    command: option.command,
                },
                EventBus::default(),
            )
            .detect_mode()
            .await
            .unwrap_or_else(|error| panic!("{}: {error}", option.label));
        }
    }
}
