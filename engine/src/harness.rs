//! Native coding-harness adapters.
//!
//! TOHSENO prepares and observes work, but the selected harness remains an
//! ordinary interactive child process with inherited terminal input/output.
//! Adapter commands therefore contain no permission-bypass or non-interactive
//! flags.

use crate::config::HarnessConfig;
use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationStatus {
    Authenticated,
    NotDetected,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentBehavior {
    NativeImageArguments,
    LocalPathsInIntent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessModel {
    pub id: String,
    pub label: String,
    pub is_default: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessRoute {
    pub id: String,
    pub label: String,
    pub billing: String,
    pub available: bool,
    pub estimated_additional_cost_usd: Option<f64>,
    pub cost_estimation: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessOption {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing)]
    pub command: String,
    pub installed: bool,
    pub selected: bool,
    pub authentication: AuthenticationStatus,
    pub models: Vec<HarnessModel>,
    pub routes: Vec<HarnessRoute>,
    pub attachment_behavior: AttachmentBehavior,
    pub completion_detection: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarnessSelection {
    pub harness: String,
    pub model: String,
    pub route: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarnessCommand {
    pub program: PathBuf,
    pub arguments: Vec<OsString>,
    pub environment: Vec<(OsString, OsString)>,
    pub removed_environment: Vec<OsString>,
}

#[derive(Clone, Copy)]
struct KnownHarness {
    id: &'static str,
    aliases: &'static [&'static str],
    label: &'static str,
    binaries: &'static [&'static str],
    home_paths: &'static [&'static str],
    models: &'static [(&'static str, &'static str)],
    default_route: &'static str,
    attachment_behavior: AttachmentBehavior,
    /// Flags that disable the harness's own permission prompts. Shots run
    /// unattended inside the repository sandbox, so the harness must never
    /// stall waiting for an approval nobody is present to grant.
    bypass_arguments: &'static [&'static str],
}

static KNOWN_HARNESSES: [KnownHarness; 5] = [
    KnownHarness {
        id: "codex",
        aliases: &[],
        label: "Codex",
        binaries: &["codex"],
        home_paths: &[".local/bin/codex"],
        models: &[("default", "Configured default")],
        default_route: "chatgpt-subscription",
        attachment_behavior: AttachmentBehavior::NativeImageArguments,
        bypass_arguments: &["--yolo"],
    },
    KnownHarness {
        id: "claude-code",
        aliases: &["claude"],
        label: "Claude Code",
        binaries: &["claude"],
        home_paths: &[".local/bin/claude", ".bun/bin/claude"],
        models: &[
            ("default", "Configured default"),
            ("sonnet", "Claude Sonnet"),
            ("opus", "Claude Opus"),
        ],
        default_route: "claude-subscription",
        attachment_behavior: AttachmentBehavior::LocalPathsInIntent,
        bypass_arguments: &["--dangerously-skip-permissions"],
    },
    KnownHarness {
        id: "opencode",
        aliases: &[],
        label: "OpenCode",
        binaries: &["opencode"],
        home_paths: &[".opencode/bin/opencode", ".local/bin/opencode"],
        models: &[("default", "Configured default")],
        default_route: "configured",
        attachment_behavior: AttachmentBehavior::LocalPathsInIntent,
        bypass_arguments: &[],
    },
    KnownHarness {
        id: "grok-build",
        aliases: &["grok"],
        label: "Grok Build",
        binaries: &["grok", "grok-build", "grokbuild"],
        home_paths: &[".grok/bin/grok", ".local/bin/grok"],
        models: &[("default", "Configured default")],
        default_route: "configured",
        attachment_behavior: AttachmentBehavior::LocalPathsInIntent,
        bypass_arguments: &[],
    },
    KnownHarness {
        id: "hermes",
        aliases: &[],
        label: "Hermes Agent",
        binaries: &["hermes"],
        home_paths: &[".local/bin/hermes", ".hermes/bin/hermes"],
        models: &[("default", "Configured default")],
        default_route: "configured",
        attachment_behavior: AttachmentBehavior::LocalPathsInIntent,
        bypass_arguments: &[],
    },
];

pub fn discover_harnesses(selected: &HarnessConfig) -> Vec<HarnessOption> {
    let selected_id = split_command(&selected.command)
        .and_then(|words| words.first().cloned())
        .and_then(|program| known_harness_for_program(&program).map(|known| known.id));
    KNOWN_HARNESSES
        .iter()
        .map(|known| describe_harness(known, selected_id == Some(known.id)))
        .collect()
}

pub fn default_selection(selected: &HarnessConfig) -> Option<HarnessSelection> {
    let harnesses = discover_harnesses(selected);
    let harness = harnesses
        .iter()
        .find(|option| option.selected && option.installed)
        .or_else(|| harnesses.iter().find(|option| option.installed))?;
    Some(HarnessSelection {
        harness: harness.id.clone(),
        model: "default".into(),
        route: harness
            .routes
            .iter()
            .find(|route| route.available)
            .map(|route| route.id.clone())
            .unwrap_or_else(|| "configured".into()),
    })
}

pub fn resolve_selection(
    selection: &HarnessSelection,
) -> Result<(HarnessOption, HarnessCommand), String> {
    validate_token("harness", &selection.harness)?;
    validate_token("model", &selection.model)?;
    validate_token("route", &selection.route)?;
    let known = known_harness(&selection.harness)
        .ok_or_else(|| format!("unsupported coding harness `{}`", selection.harness))?;
    let option = describe_harness(known, false);
    if !option.installed {
        return Err(format!(
            "{} is unavailable; install `{}` before preparing the Shot",
            option.label, known.binaries[0]
        ));
    }
    if !option
        .models
        .iter()
        .any(|model| model.id == selection.model)
    {
        return Err(format!(
            "{} does not advertise model `{}` on this machine",
            option.label, selection.model
        ));
    }
    let route = option
        .routes
        .iter()
        .find(|route| route.id == selection.route)
        .ok_or_else(|| {
            format!(
                "{} does not support inference route `{}`",
                option.label, selection.route
            )
        })?;
    if !route.available {
        return Err(format!(
            "{} route `{}` is not authenticated on this machine",
            option.label, route.label
        ));
    }
    let executable =
        find_executable(known).ok_or_else(|| format!("{} became unavailable", option.label))?;
    let removed_environment = removed_environment_for_route(&route.id);
    Ok((
        option,
        HarnessCommand {
            program: executable,
            arguments: known
                .bypass_arguments
                .iter()
                .map(OsString::from)
                .collect(),
            environment: Vec::new(),
            removed_environment,
        },
    ))
}

pub fn build_interactive_command(
    selection: &HarnessSelection,
    intent_path: &Path,
    image_paths: &[PathBuf],
) -> Result<HarnessCommand, String> {
    let (option, mut command) = resolve_selection(selection)?;
    if selection.model != "default" {
        command.arguments.push("--model".into());
        command.arguments.push(selection.model.clone().into());
    }
    if option.attachment_behavior == AttachmentBehavior::NativeImageArguments {
        for image in image_paths {
            command.arguments.push("--image".into());
            command.arguments.push(image.as_os_str().to_owned());
        }
    }
    command.arguments.push(
        format!(
            "Read `{}` and follow it as the authoritative TOHSENO intention package. Inspect every labeled reference image before implementing. Work through your normal interactive interface, including its native questions and plans; permissions are pre-granted for this run, so execute without pausing for approval.",
            intent_path.display()
        )
        .into(),
    );
    Ok(command)
}

pub fn estimated_cost(selection: &HarnessSelection) -> Result<Option<f64>, String> {
    let (option, _) = resolve_selection(selection)?;
    Ok(option
        .routes
        .iter()
        .find(|route| route.id == selection.route)
        .and_then(|route| route.estimated_additional_cost_usd))
}

fn describe_harness(known: &KnownHarness, selected: bool) -> HarnessOption {
    let executable = find_executable(known);
    let (authentication, routes) = authentication_and_routes(known);
    HarnessOption {
        id: known.id.into(),
        label: known.label.into(),
        command: executable
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| known.binaries[0].into()),
        installed: executable.is_some(),
        selected,
        authentication,
        models: {
            let mut models = known
                .models
                .iter()
                .enumerate()
                .map(|(index, (id, label))| HarnessModel {
                    id: (*id).into(),
                    label: (*label).into(),
                    is_default: index == 0,
                })
                .collect::<Vec<_>>();
            if known.id == "codex" {
                if let Some(configured) = configured_codex_model() {
                    if !models.iter().any(|model| model.id == configured) {
                        models.push(HarnessModel {
                            label: format!("Configured: {configured}"),
                            id: configured,
                            is_default: false,
                        });
                    }
                }
            }
            models
        },
        routes,
        attachment_behavior: known.attachment_behavior,
        completion_detection: "native process exit plus independent workspace state".into(),
    }
}

fn authentication_and_routes(known: &KnownHarness) -> (AuthenticationStatus, Vec<HarnessRoute>) {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match known.id {
        "codex" => {
            let subscription = home
                .as_ref()
                .is_some_and(|home| home.join(".codex/auth.json").is_file());
            let api = std::env::var_os("OPENAI_API_KEY").is_some();
            (
                if subscription || api {
                    AuthenticationStatus::Authenticated
                } else {
                    AuthenticationStatus::NotDetected
                },
                vec![
                    zero_cost_route("chatgpt-subscription", "ChatGPT subscription", subscription),
                    usage_route("openai-api", "OpenAI API", api),
                ],
            )
        }
        "claude-code" => {
            let subscription = home.as_ref().is_some_and(|home| {
                home.join(".claude").is_dir()
                    || home.join(".config/claude-code").is_dir()
                    || home.join(".claude.json").is_file()
            });
            let api = std::env::var_os("ANTHROPIC_API_KEY").is_some();
            (
                if subscription || api {
                    AuthenticationStatus::Authenticated
                } else {
                    AuthenticationStatus::Unknown
                },
                vec![
                    zero_cost_route("claude-subscription", "Claude subscription", subscription),
                    usage_route("anthropic-api", "Anthropic API", api),
                ],
            )
        }
        _ => (
            AuthenticationStatus::Unknown,
            vec![HarnessRoute {
                id: known.default_route.into(),
                label: "Configured local route".into(),
                billing: "configured".into(),
                available: true,
                estimated_additional_cost_usd: None,
                cost_estimation: false,
            }],
        ),
    }
}

fn zero_cost_route(id: &str, label: &str, available: bool) -> HarnessRoute {
    HarnessRoute {
        id: id.into(),
        label: label.into(),
        billing: "subscription".into(),
        available,
        estimated_additional_cost_usd: Some(0.0),
        cost_estimation: true,
    }
}

fn usage_route(id: &str, label: &str, available: bool) -> HarnessRoute {
    HarnessRoute {
        id: id.into(),
        label: label.into(),
        billing: "api".into(),
        available,
        estimated_additional_cost_usd: None,
        cost_estimation: false,
    }
}

fn removed_environment_for_route(route: &str) -> Vec<OsString> {
    match route {
        "chatgpt-subscription" => vec![OsString::from("OPENAI_API_KEY")],
        "claude-subscription" => vec![OsString::from("ANTHROPIC_API_KEY")],
        _ => Vec::new(),
    }
}

fn configured_codex_model() -> Option<String> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let body = std::fs::read_to_string(home.join(".codex/config.toml")).ok()?;
    let value = body.parse::<toml::Value>().ok()?;
    let model = value.get("model")?.as_str()?.to_owned();
    validate_token("model", &model).ok()?;
    Some(model)
}

fn known_harness(id: &str) -> Option<&'static KnownHarness> {
    KNOWN_HARNESSES
        .iter()
        .find(|known| known.id == id || known.aliases.contains(&id))
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

fn known_harness_for_program(program: &OsStr) -> Option<&'static KnownHarness> {
    let name = Path::new(program).file_name()?.to_string_lossy();
    KNOWN_HARNESSES.iter().find(|known| {
        known
            .binaries
            .iter()
            .any(|binary| name.eq_ignore_ascii_case(binary))
    })
}

fn validate_token(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(format!(
            "{label} must be one nonempty token of at most 128 bytes"
        ));
    }
    Ok(())
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
    fn first_class_adapters_are_stable_and_safe() {
        assert_eq!(KNOWN_HARNESSES[0].id, "codex");
        assert_eq!(KNOWN_HARNESSES[1].id, "claude-code");
        for known in KNOWN_HARNESSES {
            assert!(!known
                .models
                .iter()
                .any(|(model, _)| model.contains("dangerously")));
        }
    }

    #[test]
    fn first_class_adapters_always_bypass_permission_prompts() {
        let codex = known_harness("codex").unwrap();
        assert_eq!(codex.bypass_arguments, &["--yolo"]);
        let claude = known_harness("claude-code").unwrap();
        assert_eq!(claude.bypass_arguments, &["--dangerously-skip-permissions"]);
    }

    #[test]
    fn selection_tokens_reject_shell_material() {
        assert!(validate_token("model", "opus").is_ok());
        assert!(validate_token("model", "opus --print").is_err());
        assert!(validate_token("route", "api\nTOKEN=x").is_err());
    }

    #[test]
    fn subscription_routes_remove_competing_api_credentials() {
        assert_eq!(
            removed_environment_for_route("chatgpt-subscription"),
            vec![OsString::from("OPENAI_API_KEY")]
        );
        assert_eq!(
            removed_environment_for_route("claude-subscription"),
            vec![OsString::from("ANTHROPIC_API_KEY")]
        );
        assert!(removed_environment_for_route("openai-api").is_empty());
    }

    #[test]
    fn split_configured_command_preserves_quoted_executable() {
        assert_eq!(
            split_command("\"/Users/App Maker/bin/codex\""),
            Some(vec![OsString::from("/Users/App Maker/bin/codex")])
        );
    }
}
