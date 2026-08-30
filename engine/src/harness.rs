//! Native coding-harness adapters.
//!
//! A Shot is an unattended factory run. The selected harness keeps its own
//! authentication and inference route, while every phase uses that harness's
//! supported one-shot command so the engine always regains control for
//! validation, repair, sealing, and device installation. Each first-class
//! harness additionally carries its own permission-bypass mode so the run
//! never stalls on an approval nobody is present to grant.

use crate::config::{Config, CustomHarnessConfig, LocalEndpointConfig};
use crate::safe_file::read_bounded_utf8;
use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

const MAX_HARNESS_CONFIG_BYTES: u64 = 1024 * 1024;

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
    #[serde(skip_serializing)]
    pub adapter: Option<HarnessAdapter>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessSelection {
    pub harness: String,
    pub model: String,
    pub route: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<HarnessAdapter>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HarnessAdapter {
    CustomExecutable {
        executable: String,
        arguments: Vec<String>,
    },
    LocalOpenAi {
        base_url: String,
        privacy_mode: String,
        credential_reference: Option<String>,
    },
    ManagedOpenAi {
        proxy_origin: String,
        command_id: String,
        execution_id: String,
        privacy_mode: String,
        maximum_microusd: u64,
        pricing_snapshot_at: String,
        input_microusd_per_million: u64,
        output_microusd_per_million: u64,
        estimate_low_microusd: u64,
        estimate_high_microusd: u64,
    },
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
    /// Arguments placed before model and attachment options to select the
    /// harness's supported non-interactive agent loop.
    unattended_arguments: &'static [&'static str],
    /// Arguments placed immediately before the prompt. Some CLIs model their
    /// one-shot prompt as an option value rather than a positional argument.
    unattended_prompt_arguments: &'static [&'static str],
}

static KNOWN_HARNESSES: [KnownHarness; 5] = [
    KnownHarness {
        id: "codex",
        aliases: &[],
        label: "Codex",
        binaries: &["codex"],
        home_paths: &[
            ".local/bin/codex",
            ".volta/bin/codex",
            ".npm-global/bin/codex",
            ".bun/bin/codex",
        ],
        models: &[("default", "Configured default")],
        default_route: "chatgpt-subscription",
        attachment_behavior: AttachmentBehavior::NativeImageArguments,
        bypass_arguments: &["--yolo"],
        unattended_arguments: &["exec"],
        // Codex models `--image` as a variadic option (`<FILE>...`). Without
        // an option terminator, the final image occurrence consumes the
        // positional instruction and Codex falls back to reading null stdin.
        unattended_prompt_arguments: &["--"],
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
        unattended_arguments: &["--print"],
        unattended_prompt_arguments: &[],
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
        unattended_arguments: &["run", "--auto"],
        unattended_prompt_arguments: &[],
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
        bypass_arguments: &["--always-approve"],
        unattended_arguments: &[],
        unattended_prompt_arguments: &["--single"],
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
        bypass_arguments: &["--yolo"],
        unattended_arguments: &["chat"],
        unattended_prompt_arguments: &["--quiet", "--query"],
    },
];

pub fn discover_harnesses(config: &Config) -> Vec<HarnessOption> {
    let selected_id = config
        .intelligence
        .preferred_harness
        .as_deref()
        .or_else(|| {
            split_command(&config.harness.command)
                .and_then(|words| words.first().cloned())
                .and_then(|program| known_harness_for_program(&program).map(|known| known.id))
        });
    let mut options = KNOWN_HARNESSES
        .iter()
        .map(|known| describe_harness(known, selected_id == Some(known.id)))
        .collect::<Vec<_>>();
    options.extend(
        config
            .intelligence
            .custom_harnesses
            .iter()
            .map(|custom| custom_harness_option(custom, selected_id == Some(custom.id.as_str()))),
    );
    options.extend(config.intelligence.local_endpoints.iter().map(|endpoint| {
        local_endpoint_option(endpoint, selected_id == Some(endpoint.id.as_str()))
    }));
    options
}

pub fn default_selection(config: &Config) -> Option<HarnessSelection> {
    let harnesses = discover_harnesses(config);
    let harness = harnesses
        .iter()
        .find(|option| option.selected && usable(option))
        .or_else(|| harnesses.iter().find(|option| usable(option)))?;
    let model = harness
        .models
        .iter()
        .find(|model| model.is_default)
        .or_else(|| harness.models.first())?;
    Some(HarnessSelection {
        harness: harness.id.clone(),
        model: model.id.clone(),
        route: harness
            .routes
            .iter()
            .find(|route| route.available)
            .map(|route| route.id.clone())
            .unwrap_or_else(|| "configured".into()),
        adapter: harness.adapter.clone(),
    })
}

fn usable(option: &HarnessOption) -> bool {
    option.installed
        && option.authentication == AuthenticationStatus::Authenticated
        && option.routes.iter().any(|route| route.available)
}

pub fn resolve_selection(
    selection: &HarnessSelection,
) -> Result<(HarnessOption, HarnessCommand), String> {
    validate_token("harness", &selection.harness)?;
    validate_token("model", &selection.model)?;
    validate_token("route", &selection.route)?;
    if let Some(adapter) = &selection.adapter {
        return resolve_configured_adapter(selection, adapter);
    }
    if cfg!(debug_assertions)
        && selection.harness == "tohseno-test-factory"
        && selection.model == "fixture"
        && selection.route == "no-inference"
    {
        let program = test_factory_harness_program()?
            .ok_or("TOHSENO_TEST_FACTORY_HARNESS is not configured")?;
        return Ok((
            HarnessOption {
                id: selection.harness.clone(),
                label: "Deterministic factory fixture".into(),
                command: program.display().to_string(),
                installed: true,
                selected: true,
                authentication: AuthenticationStatus::Authenticated,
                models: vec![HarnessModel {
                    id: selection.model.clone(),
                    label: "Fixture".into(),
                    is_default: true,
                }],
                routes: vec![HarnessRoute {
                    id: selection.route.clone(),
                    label: "No inference".into(),
                    billing: "none".into(),
                    available: true,
                    estimated_additional_cost_usd: Some(0.0),
                    cost_estimation: true,
                }],
                attachment_behavior: AttachmentBehavior::LocalPathsInIntent,
                completion_detection: "fixture process exit plus deterministic engine acceptance"
                    .into(),
                adapter: None,
            },
            HarnessCommand {
                program,
                arguments: Vec::new(),
                environment: Vec::new(),
                removed_environment: Vec::new(),
            },
        ));
    }
    if cfg!(debug_assertions)
        && std::env::var("TOHSENO_TEST_NONLAUNCHING_HARNESS").as_deref() == Ok("1")
        && selection.harness == "tohseno-test-nonlaunching"
        && selection.model == "fixture"
        && selection.route == "no-inference"
    {
        return Ok((
            HarnessOption {
                id: selection.harness.clone(),
                label: "Nonlaunching test harness".into(),
                command: "/usr/bin/false".into(),
                installed: true,
                selected: false,
                authentication: AuthenticationStatus::Authenticated,
                models: vec![HarnessModel {
                    id: selection.model.clone(),
                    label: "Fixture".into(),
                    is_default: true,
                }],
                routes: vec![HarnessRoute {
                    id: selection.route.clone(),
                    label: "No inference".into(),
                    billing: "none".into(),
                    available: true,
                    estimated_additional_cost_usd: Some(0.0),
                    cost_estimation: true,
                }],
                attachment_behavior: AttachmentBehavior::LocalPathsInIntent,
                completion_detection: "never launched".into(),
                adapter: None,
            },
            HarnessCommand {
                program: PathBuf::from("/usr/bin/false"),
                arguments: Vec::new(),
                environment: Vec::new(),
                removed_environment: Vec::new(),
            },
        ));
    }
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
    let environment = executable_path_environment(&executable)?;
    Ok((
        option,
        HarnessCommand {
            program: executable,
            arguments: known.bypass_arguments.iter().map(OsString::from).collect(),
            environment,
            removed_environment,
        },
    ))
}

/// Debug-build-only deterministic harness used by the repository's vertical
/// factory lifecycle test. Production binaries ignore this environment key.
pub fn test_factory_harness_program() -> Result<Option<PathBuf>, String> {
    if !cfg!(debug_assertions) {
        return Ok(None);
    }
    let Some(value) = std::env::var_os("TOHSENO_TEST_FACTORY_HARNESS") else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err("TOHSENO_TEST_FACTORY_HARNESS must be an absolute path".into());
    }
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|error| format!("test factory harness is unavailable: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("test factory harness must be a regular non-symlink file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err("test factory harness must be executable".into());
        }
    }
    Ok(Some(path))
}

pub fn build_evolution_command(
    selection: &HarnessSelection,
    intent_path: &Path,
    image_paths: &[PathBuf],
) -> Result<HarnessCommand, String> {
    build_command(
        selection,
        image_paths,
        format!(
            "Read `{}`, complete the requested app, verify it, and exit.",
            intent_path.display()
        ),
    )
}

pub fn build_materialization_command(
    selection: &HarnessSelection,
    task_path: &Path,
    image_paths: &[PathBuf],
    repair_diagnostic: Option<&str>,
) -> Result<HarnessCommand, String> {
    build_command(
        selection,
        image_paths,
        materialization_instruction(task_path, repair_diagnostic),
    )
}

fn materialization_instruction(task_path: &Path, repair_diagnostic: Option<&str>) -> String {
    match repair_diagnostic {
        Some(diagnostic) => format!(
            "Read `{}`. Repair only this independently diagnosed criterion: {diagnostic}. Do not redo the implementation or run xcodebuild; TOHSENO immediately reruns its deterministic gates. Do not replace the existing state-transition draft. Then exit.",
            task_path.display()
        ),
        None => format!(
            "Read `{}`, complete the requested app, verify it, and exit.",
            task_path.display()
        ),
    }
}

fn build_command(
    selection: &HarnessSelection,
    image_paths: &[PathBuf],
    instruction: String,
) -> Result<HarnessCommand, String> {
    let (option, mut command) = resolve_selection(selection)?;
    if let Some(known) = known_harness(&option.id) {
        command
            .arguments
            .extend(known.unattended_arguments.iter().map(OsString::from));
    }
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
    if let Some(known) = known_harness(&option.id) {
        command
            .arguments
            .extend(known.unattended_prompt_arguments.iter().map(OsString::from));
    }
    command.arguments.push(instruction.into());
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
        completion_detection: "unattended process exit plus independent workspace state".into(),
        adapter: None,
    }
}

fn custom_harness_option(config: &CustomHarnessConfig, selected: bool) -> HarnessOption {
    let valid = validate_configured_id(&config.id).is_ok()
        && validate_custom_executable(&config.executable, &config.arguments).is_ok()
        && !config.models.is_empty()
        && config.models.len() <= 32
        && config
            .models
            .iter()
            .all(|model| validate_token("model", model).is_ok());
    HarnessOption {
        id: config.id.clone(),
        label: config.label.chars().take(80).collect(),
        command: config.executable.clone(),
        installed: valid,
        selected,
        authentication: if valid {
            AuthenticationStatus::Authenticated
        } else {
            AuthenticationStatus::NotDetected
        },
        models: config
            .models
            .iter()
            .take(32)
            .enumerate()
            .map(|(index, model)| HarnessModel {
                id: model.clone(),
                label: model.clone(),
                is_default: index == 0,
            })
            .collect(),
        routes: vec![HarnessRoute {
            id: "custom-local".into(),
            label: "Custom executable".into(),
            billing: "configured".into(),
            available: valid,
            estimated_additional_cost_usd: None,
            cost_estimation: false,
        }],
        attachment_behavior: AttachmentBehavior::LocalPathsInIntent,
        completion_detection: "declared executable exit plus independent workspace state".into(),
        adapter: Some(HarnessAdapter::CustomExecutable {
            executable: config.executable.clone(),
            arguments: config.arguments.clone(),
        }),
    }
}

fn local_endpoint_option(config: &LocalEndpointConfig, selected: bool) -> HarnessOption {
    let valid = validate_configured_id(&config.id).is_ok()
        && validate_local_endpoint(&config.base_url).is_ok()
        && config.consent_to_send_source
        && !config.models.is_empty()
        && config.models.len() <= 32
        && config
            .models
            .iter()
            .all(|model| validate_token("model", model).is_ok())
        && config
            .credential_reference
            .as_deref()
            .is_none_or(|reference| validate_token("credential reference", reference).is_ok())
        && matches!(
            config.privacy_mode.as_str(),
            "local" | "standard" | "zdr" | "private"
        );
    HarnessOption {
        id: config.id.clone(),
        label: config.label.chars().take(80).collect(),
        command: "bundled local OpenAI-compatible adapter".into(),
        installed: true,
        selected,
        authentication: if valid {
            AuthenticationStatus::Authenticated
        } else {
            AuthenticationStatus::NotDetected
        },
        models: config
            .models
            .iter()
            .take(32)
            .enumerate()
            .map(|(index, model)| HarnessModel {
                id: model.clone(),
                label: model.clone(),
                is_default: index == 0,
            })
            .collect(),
        routes: vec![HarnessRoute {
            id: "local-openai".into(),
            label: "Local model endpoint".into(),
            billing: "local".into(),
            available: valid,
            estimated_additional_cost_usd: Some(0.0),
            cost_estimation: true,
        }],
        attachment_behavior: AttachmentBehavior::LocalPathsInIntent,
        completion_detection: "bounded local adapter exit plus independent workspace state".into(),
        adapter: Some(HarnessAdapter::LocalOpenAi {
            base_url: config.base_url.clone(),
            privacy_mode: config.privacy_mode.clone(),
            credential_reference: config.credential_reference.clone(),
        }),
    }
}

fn resolve_configured_adapter(
    selection: &HarnessSelection,
    adapter: &HarnessAdapter,
) -> Result<(HarnessOption, HarnessCommand), String> {
    match adapter {
        HarnessAdapter::CustomExecutable {
            executable,
            arguments,
        } => {
            let program = validate_custom_executable(executable, arguments)?;
            Ok((
                HarnessOption {
                    id: selection.harness.clone(),
                    label: "Configured custom harness".into(),
                    command: executable.clone(),
                    installed: true,
                    selected: true,
                    authentication: AuthenticationStatus::Authenticated,
                    models: vec![HarnessModel {
                        id: selection.model.clone(),
                        label: selection.model.clone(),
                        is_default: true,
                    }],
                    routes: vec![HarnessRoute {
                        id: selection.route.clone(),
                        label: "Custom executable".into(),
                        billing: "configured".into(),
                        available: true,
                        estimated_additional_cost_usd: None,
                        cost_estimation: false,
                    }],
                    attachment_behavior: AttachmentBehavior::LocalPathsInIntent,
                    completion_detection:
                        "declared executable exit plus independent workspace state".into(),
                    adapter: Some(adapter.clone()),
                },
                HarnessCommand {
                    program,
                    arguments: arguments.iter().map(OsString::from).collect(),
                    environment: Vec::new(),
                    removed_environment: sensitive_environment(),
                },
            ))
        }
        HarnessAdapter::LocalOpenAi {
            base_url,
            privacy_mode,
            credential_reference,
        } => {
            validate_local_endpoint(base_url)?;
            if !matches!(
                privacy_mode.as_str(),
                "local" | "standard" | "zdr" | "private"
            ) {
                return Err("local endpoint privacy mode is invalid".into());
            }
            let program = std::env::current_exe()
                .map_err(|error| format!("bundled adapter is unavailable: {error}"))?;
            let mut arguments = [
                "local-openai-harness",
                "--base-url",
                base_url,
                "--model",
                &selection.model,
                "--privacy",
                privacy_mode,
            ]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
            if let Some(reference) = credential_reference {
                validate_token("credential reference", reference)?;
                arguments.push("--credential-reference".into());
                arguments.push(reference.into());
            }
            Ok((
                HarnessOption {
                    id: selection.harness.clone(),
                    label: "Local OpenAI-compatible model".into(),
                    command: program.display().to_string(),
                    installed: true,
                    selected: true,
                    authentication: AuthenticationStatus::Authenticated,
                    models: vec![HarnessModel {
                        id: selection.model.clone(),
                        label: selection.model.clone(),
                        is_default: true,
                    }],
                    routes: vec![HarnessRoute {
                        id: selection.route.clone(),
                        label: "Local model endpoint".into(),
                        billing: "local".into(),
                        available: true,
                        estimated_additional_cost_usd: Some(0.0),
                        cost_estimation: true,
                    }],
                    attachment_behavior: AttachmentBehavior::LocalPathsInIntent,
                    completion_detection:
                        "bounded local adapter exit plus independent workspace state".into(),
                    adapter: Some(adapter.clone()),
                },
                HarnessCommand {
                    program,
                    arguments,
                    environment: Vec::new(),
                    removed_environment: sensitive_environment(),
                },
            ))
        }
        HarnessAdapter::ManagedOpenAi {
            proxy_origin,
            command_id,
            execution_id,
            privacy_mode,
            maximum_microusd,
            pricing_snapshot_at,
            input_microusd_per_million,
            output_microusd_per_million,
            estimate_low_microusd,
            estimate_high_microusd,
        } => {
            validate_managed_origin(proxy_origin)?;
            validate_token("command identifier", command_id)?;
            validate_token("execution identifier", execution_id)?;
            if !matches!(privacy_mode.as_str(), "standard" | "zdr" | "private") {
                return Err("managed privacy mode is invalid".into());
            }
            if *maximum_microusd == 0 || *maximum_microusd > 100_000_000 {
                return Err("managed maximum is invalid".into());
            }
            if pricing_snapshot_at.is_empty()
                || pricing_snapshot_at.len() > 64
                || pricing_snapshot_at.chars().any(char::is_control)
                || *input_microusd_per_million == 0
                || *output_microusd_per_million == 0
                || *estimate_low_microusd == 0
                || estimate_low_microusd > estimate_high_microusd
                || estimate_high_microusd > maximum_microusd
            {
                return Err("managed pricing snapshot or estimate is invalid".into());
            }
            let program = std::env::current_exe()
                .map_err(|error| format!("bundled managed adapter is unavailable: {error}"))?;
            let arguments = vec![
                "managed-open-ai-harness".into(),
                "--proxy-origin".into(),
                proxy_origin.into(),
                "--model".into(),
                selection.model.clone().into(),
                "--privacy".into(),
                privacy_mode.into(),
                "--command-id".into(),
                command_id.into(),
                "--execution-id".into(),
                execution_id.into(),
                "--maximum-microusd".into(),
                maximum_microusd.to_string().into(),
                "--pricing-snapshot-at".into(),
                pricing_snapshot_at.into(),
                "--input-microusd-per-million".into(),
                input_microusd_per_million.to_string().into(),
                "--output-microusd-per-million".into(),
                output_microusd_per_million.to_string().into(),
            ];
            Ok((
                HarnessOption {
                    id: selection.harness.clone(),
                    label: "TOHSENO managed intelligence".into(),
                    command: program.display().to_string(),
                    installed: true,
                    selected: true,
                    authentication: AuthenticationStatus::Authenticated,
                    models: vec![HarnessModel {
                        id: selection.model.clone(),
                        label: selection.model.clone(),
                        is_default: true,
                    }],
                    routes: vec![HarnessRoute {
                        id: selection.route.clone(),
                        label: "TOHSENO managed intelligence".into(),
                        billing: "managed_balance".into(),
                        available: true,
                        estimated_additional_cost_usd: None,
                        cost_estimation: true,
                    }],
                    attachment_behavior: AttachmentBehavior::LocalPathsInIntent,
                    completion_detection:
                        "bounded managed adapter exit plus independent workspace state".into(),
                    adapter: Some(adapter.clone()),
                },
                HarnessCommand {
                    program,
                    arguments,
                    environment: Vec::new(),
                    removed_environment: sensitive_environment(),
                },
            ))
        }
    }
}

fn validate_managed_origin(origin: &str) -> Result<(), String> {
    if origin == "https://tohseno.com" {
        return Ok(());
    }
    #[cfg(debug_assertions)]
    if std::env::var("TOHSENO_MANAGED_ORIGIN").as_deref() == Ok(origin)
        && (origin.starts_with("http://127.0.0.1:") || origin.starts_with("http://localhost:"))
        && !origin.contains(['?', '#', '@'])
        && !origin.ends_with('/')
    {
        return Ok(());
    }
    Err("managed service origin is not an approved release origin".into())
}

fn validate_configured_id(id: &str) -> Result<(), String> {
    validate_token("configured harness", id)?;
    if KNOWN_HARNESSES.iter().any(|known| known.id == id) {
        return Err("configured harness ID is reserved".into());
    }
    Ok(())
}

fn validate_custom_executable(executable: &str, arguments: &[String]) -> Result<PathBuf, String> {
    if arguments.len() > 32
        || arguments
            .iter()
            .any(|argument| argument.is_empty() || argument.len() > 512 || argument.contains('\0'))
    {
        return Err("custom harness arguments are invalid".into());
    }
    let path = PathBuf::from(executable);
    if !path.is_absolute() {
        return Err("custom harness executable must be absolute".into());
    }
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|error| format!("custom harness executable is unavailable: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("custom harness executable must be a regular non-symlink file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err("custom harness executable is not executable".into());
        }
    }
    Ok(path)
}

fn validate_local_endpoint(base_url: &str) -> Result<(), String> {
    if base_url.len() > 512 || base_url.contains(['?', '#', '@']) || base_url.ends_with('/') {
        return Err("local endpoint URL is invalid".into());
    }
    let authority = base_url
        .strip_prefix("http://127.0.0.1:")
        .or_else(|| base_url.strip_prefix("http://localhost:"))
        .ok_or("local endpoint must use explicit loopback HTTP")?;
    let port = authority.split('/').next().unwrap_or_default();
    if port.parse::<u16>().ok().filter(|port| *port > 0).is_none() {
        return Err("local endpoint port is invalid".into());
    }
    Ok(())
}

fn sensitive_environment() -> Vec<OsString> {
    [
        "BANKR_API_KEY",
        "STRIPE_SECRET_KEY",
        "STRIPE_WEBHOOK_SECRET",
        "TOHSENO_OPERATOR_TOKEN",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
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
    let body =
        read_bounded_utf8(&home.join(".codex/config.toml"), MAX_HARNESS_CONFIG_BYTES).ok()?;
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
    let from_home = known
        .home_paths
        .iter()
        .map(|relative| home.join(relative))
        .find(|candidate| is_executable(candidate));
    let from_global = ["/opt/homebrew/bin", "/usr/local/bin"]
        .into_iter()
        .flat_map(|directory| {
            known
                .binaries
                .iter()
                .map(move |binary| Path::new(directory).join(binary))
        })
        .find(|candidate| is_executable(candidate));
    from_home
        .or(from_global)
        .or_else(|| find_nvm_executable(&home, known))
}

/// NVM does not expose its selected Node installation to launchd. Resolve the
/// same default alias an interactive shell uses so the persistent factory sees
/// an npm-installed harness without inheriting the user's entire shell PATH.
fn find_nvm_executable(home: &Path, known: &KnownHarness) -> Option<PathBuf> {
    let nvm = home.join(".nvm");
    let mut versions = std::fs::read_dir(nvm.join("versions/node"))
        .ok()?
        .take(128)
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|candidate| candidate.is_dir())
        .collect::<Vec<_>>();
    versions.sort();

    let configured = read_bounded_utf8(&nvm.join("alias/default"), 256)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|alias| {
            !alias.is_empty()
                && alias.len() <= 64
                && alias
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b'.')
        });
    let matches_default = |version: &&PathBuf| {
        let Some(alias) = configured.as_deref() else {
            return false;
        };
        let prefix = format!("v{alias}");
        version
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name == prefix
                    || name
                        .strip_prefix(&prefix)
                        .is_some_and(|suffix| suffix.starts_with('.'))
            })
    };
    let executable_in = |version: &Path| {
        known
            .binaries
            .iter()
            .map(|binary| version.join("bin").join(binary))
            .find(|candidate| is_executable(candidate))
    };
    versions
        .iter()
        .rev()
        .find(matches_default)
        .and_then(|version| executable_in(version))
        .or_else(|| {
            versions
                .into_iter()
                .rev()
                .find_map(|version| executable_in(&version))
        })
}

/// npm launchers commonly use `#!/usr/bin/env node`. Prepending the selected
/// executable's own directory lets that stable launcher find its sibling Node
/// binary even when launchd supplies only the macOS system PATH.
fn executable_path_environment(executable: &Path) -> Result<Vec<(OsString, OsString)>, String> {
    let parent = executable
        .parent()
        .ok_or("the harness executable has no parent directory")?;
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let value = std::env::join_paths(
        std::iter::once(parent.to_path_buf()).chain(std::env::split_paths(&inherited)),
    )
    .map_err(|error| format!("the harness PATH could not be constructed: {error}"))?;
    Ok(vec![(OsString::from("PATH"), value)])
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
        assert_eq!(codex.unattended_arguments, &["exec"]);
        assert_eq!(codex.unattended_prompt_arguments, &["--"]);
        let claude = known_harness("claude-code").unwrap();
        assert_eq!(claude.bypass_arguments, &["--dangerously-skip-permissions"]);
        assert_eq!(claude.unattended_arguments, &["--print"]);
        let opencode = known_harness("opencode").unwrap();
        assert_eq!(opencode.unattended_arguments, &["run", "--auto"]);
        let grok = known_harness("grok-build").unwrap();
        assert_eq!(grok.bypass_arguments, &["--always-approve"]);
        assert_eq!(grok.unattended_prompt_arguments, &["--single"]);
        let hermes = known_harness("hermes").unwrap();
        assert_eq!(hermes.bypass_arguments, &["--yolo"]);
        assert_eq!(hermes.unattended_arguments, &["chat"]);
        assert_eq!(hermes.unattended_prompt_arguments, &["--quiet", "--query"]);
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
    fn repair_instruction_carries_the_engine_diagnostic() {
        let instruction = materialization_instruction(
            Path::new(".tohseno/TASK.md"),
            Some("organ dependency has not been declared"),
        );
        assert!(instruction.contains("Repair only this independently diagnosed criterion"));
        assert!(instruction.contains("organ dependency has not been declared"));
        assert!(instruction.contains("Do not redo the implementation or run xcodebuild"));
        assert!(instruction.contains("Do not replace the existing state-transition draft"));
        assert!(!instruction.contains("sibling Shot repositories"));
    }

    #[test]
    fn materialization_requires_real_visual_reference_inspection() {
        let instruction = materialization_instruction(Path::new(".tohseno/TASK.md"), None);
        assert_eq!(
            instruction,
            "Read `.tohseno/TASK.md`, complete the requested app, verify it, and exit."
        );

        let repair = materialization_instruction(
            Path::new(".tohseno/TASK.md"),
            Some("one trial field is invalid"),
        );
        assert!(repair.contains("Repair only this independently diagnosed criterion"));
    }

    #[test]
    fn split_configured_command_preserves_quoted_executable() {
        assert_eq!(
            split_command("\"/Users/App Maker/bin/codex\""),
            Some(vec![OsString::from("/Users/App Maker/bin/codex")])
        );
    }

    #[cfg(unix)]
    #[test]
    fn custom_adapter_preserves_argument_boundaries_without_a_shell() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("custom harness");
        std::fs::write(&executable, b"#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&executable, permissions).unwrap();
        let selection = HarnessSelection {
            harness: "my-harness".into(),
            model: "local-model".into(),
            route: "custom-local".into(),
            adapter: Some(HarnessAdapter::CustomExecutable {
                executable: executable.display().to_string(),
                arguments: vec!["literal; touch /tmp/never".into(), "$(false)".into()],
            }),
        };
        let (_, command) = resolve_selection(&selection).unwrap();
        assert_eq!(command.program, executable);
        assert_eq!(
            command.arguments,
            ["literal; touch /tmp/never", "$(false)"]
                .into_iter()
                .map(OsString::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn local_adapter_requires_explicit_consent_and_loopback() {
        let mut config = Config::default();
        config
            .intelligence
            .local_endpoints
            .push(LocalEndpointConfig {
                id: "ollama".into(),
                label: "Ollama".into(),
                base_url: "http://127.0.0.1:11434/v1".into(),
                models: vec!["qwen3-coder".into()],
                credential_reference: None,
                consent_to_send_source: false,
                privacy_mode: "local".into(),
            });
        let unavailable = discover_harnesses(&config)
            .into_iter()
            .find(|option| option.id == "ollama")
            .unwrap();
        assert!(!unavailable.routes[0].available);
        config.intelligence.local_endpoints[0].consent_to_send_source = true;
        let available = discover_harnesses(&config)
            .into_iter()
            .find(|option| option.id == "ollama")
            .unwrap();
        assert!(available.routes[0].available);
        assert_eq!(available.routes[0].billing, "local");
        assert!(validate_local_endpoint("http://example.com:11434/v1").is_err());
    }

    #[test]
    fn automatic_selection_honors_preference_consent_and_advertised_model() {
        let mut config = Config::default();
        config.intelligence.preferred_harness = Some("ollama".into());
        config
            .intelligence
            .local_endpoints
            .push(LocalEndpointConfig {
                id: "ollama".into(),
                label: "Ollama".into(),
                base_url: "http://127.0.0.1:11434/v1".into(),
                models: vec!["qwen3-coder".into()],
                credential_reference: None,
                consent_to_send_source: false,
                privacy_mode: "local".into(),
            });
        assert_ne!(
            default_selection(&config).map(|value| value.harness),
            Some("ollama".into())
        );
        config.intelligence.local_endpoints[0].consent_to_send_source = true;
        let selection = default_selection(&config).unwrap();
        assert_eq!(selection.harness, "ollama");
        assert_eq!(selection.model, "qwen3-coder");
        assert_eq!(selection.route, "local-openai");
    }

    #[test]
    fn managed_selection_keeps_pricing_and_cap_out_of_arguments_except_the_cap() {
        let selection = HarnessSelection {
            harness: "tohseno-managed".into(),
            model: "qwen3-coder".into(),
            route: "managed-zdr".into(),
            adapter: Some(HarnessAdapter::ManagedOpenAi {
                proxy_origin: "https://tohseno.com".into(),
                command_id: "command_fixture".into(),
                execution_id: "execution_fixture".into(),
                privacy_mode: "zdr".into(),
                maximum_microusd: 500_000,
                pricing_snapshot_at: "2026-08-27T00:00:00Z".into(),
                input_microusd_per_million: 120_000,
                output_microusd_per_million: 360_000,
                estimate_low_microusd: 100_000,
                estimate_high_microusd: 400_000,
            }),
        };
        let (_, command) = resolve_selection(&selection).unwrap();
        let arguments = command
            .arguments
            .iter()
            .map(|value| value.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(arguments.contains("--maximum-microusd 500000"));
        assert!(arguments.contains("--pricing-snapshot-at 2026-08-27T00:00:00Z"));
        assert!(command.environment.is_empty());
        assert!(command
            .removed_environment
            .iter()
            .any(|name| name == "BANKR_API_KEY"));
    }

    #[cfg(unix)]
    #[test]
    fn launchd_discovers_an_nvm_default_harness_and_its_sibling_node() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let nvm = root.path().join(".nvm");
        std::fs::create_dir_all(nvm.join("alias")).unwrap();
        std::fs::write(nvm.join("alias/default"), b"22.12\n").unwrap();
        let bin = nvm.join("versions/node/v22.12.0/bin");
        std::fs::create_dir_all(&bin).unwrap();
        for name in ["codex", "node"] {
            let executable = bin.join(name);
            std::fs::write(&executable, b"#!/bin/sh\n").unwrap();
            let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&executable, permissions).unwrap();
        }

        let codex = known_harness("codex").unwrap();
        let executable = find_nvm_executable(root.path(), codex).unwrap();
        assert_eq!(executable, bin.join("codex"));

        let environment = executable_path_environment(&executable).unwrap();
        let configured_path = environment
            .iter()
            .find(|(name, _)| name == "PATH")
            .map(|(_, value)| value)
            .unwrap();
        assert_eq!(std::env::split_paths(configured_path).next(), Some(bin));
    }

    #[cfg(unix)]
    #[test]
    fn launchd_discovers_an_nvm_harness_without_a_default_alias() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join(".nvm/versions/node/v24.1.0/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let executable = bin.join("codex");
        std::fs::write(&executable, b"#!/bin/sh\n").unwrap();
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions).unwrap();

        let codex = known_harness("codex").unwrap();
        assert_eq!(find_nvm_executable(root.path(), codex), Some(executable));
    }
}
