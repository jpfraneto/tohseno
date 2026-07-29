//! Coding-agent discovery and launch for conducted work.
//!
//! TOHSENO no longer drives agents. It finds the builder's own agent,
//! opens it in the builder's own session on the app folder, and lets the
//! standing orders (`AGENTS.md`) do the rest. The agent records each
//! finished Evolution itself with `tohseno evolve`.

use crate::config::HarnessConfig;
use serde::Serialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

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
    /// Arguments that let the agent work uninterrupted in the builder's own
    /// visible session. The builder chose this agent; the terminal window is
    /// theirs; permission prompts would only be noise.
    launch_arguments: &'static [&'static str],
}

static KNOWN_HARNESSES: [KnownHarness; 5] = [
    KnownHarness {
        id: "hermes",
        label: "Hermes Agent",
        binaries: &["hermes"],
        home_paths: &[".local/bin/hermes", ".hermes/bin/hermes"],
        launch_arguments: &[],
    },
    KnownHarness {
        id: "codex",
        label: "Codex",
        binaries: &["codex"],
        home_paths: &[".local/bin/codex"],
        launch_arguments: &["--yolo"],
    },
    KnownHarness {
        id: "claude",
        label: "Claude Code",
        binaries: &["claude"],
        home_paths: &[".local/bin/claude"],
        launch_arguments: &["--dangerously-skip-permissions"],
    },
    KnownHarness {
        id: "grok",
        label: "Grok Build",
        binaries: &["grok", "grok-build", "grokbuild"],
        home_paths: &[".grok/bin/grok", ".local/bin/grok"],
        launch_arguments: &[],
    },
    KnownHarness {
        id: "opencode",
        label: "OpenCode",
        binaries: &["opencode"],
        home_paths: &[".opencode/bin/opencode", ".local/bin/opencode"],
        launch_arguments: &[],
    },
];

pub fn discover_harnesses(selected: &HarnessConfig) -> Vec<HarnessOption> {
    let selected_id = split_command(&selected.command)
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

/// The full launch line for one agent: executable plus its uninterrupted-work
/// arguments, ready for the builder's terminal.
pub fn launch_command(option: &HarnessOption) -> String {
    let arguments = KNOWN_HARNESSES
        .iter()
        .find(|known| known.id == option.id)
        .map(|known| known.launch_arguments)
        .unwrap_or(&[]);
    if arguments.is_empty() {
        option.command.clone()
    } else {
        format!("{} {}", option.command, arguments.join(" "))
    }
}

pub(crate) fn command_for_path(path: &Path) -> String {
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

fn split_command(command: &str) -> Option<Vec<OsString>> {
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
    if quote.is_some() {
        return None;
    }
    if !current.is_empty() {
        words.push(OsString::from(current));
    }
    Some(words)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn launch_lines_let_the_builders_own_agent_work_uninterrupted() {
        let claude = HarnessOption {
            id: "claude".into(),
            label: "Claude Code".into(),
            command: "/usr/local/bin/claude".into(),
            installed: true,
            selected: false,
        };
        assert_eq!(
            launch_command(&claude),
            "/usr/local/bin/claude --dangerously-skip-permissions"
        );
        let codex = HarnessOption {
            id: "codex".into(),
            label: "Codex".into(),
            command: "codex".into(),
            installed: true,
            selected: false,
        };
        assert_eq!(launch_command(&codex), "codex --yolo");
    }

    #[test]
    fn executable_paths_with_spaces_remain_one_command_word() {
        let command = command_for_path(Path::new("/Users/App Maker/bin/codex"));
        assert_eq!(
            split_command(&command).unwrap(),
            [OsString::from("/Users/App Maker/bin/codex")]
        );
    }
}
