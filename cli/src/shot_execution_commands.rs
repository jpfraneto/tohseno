use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tohseno_engine::harness::{build_interactive_command, HarnessSelection};
use tohseno_engine::protocol_lifecycle::verify_completed_evolution;
use tohseno_engine::shot_execution::{
    complete_execution, execution_directory, has_workspace_changed, load_completion,
    load_execution, prepare_execution, read_events, update_phase,
};
use tohseno_engine::{
    CompletionRecord, ConductedCreation, Engine, Event, EventBus, ExecutionPhase,
    ExecutionPreparation, ExecutionReference, PreparedExecution, ShotLayout,
};

pub fn selection(
    engine: &Engine,
    harness: Option<&str>,
    model: Option<&str>,
    route: Option<&str>,
) -> Result<HarnessSelection, Box<dyn std::error::Error>> {
    let harnesses = engine.harnesses();
    let selected = match harness {
        Some(id) => harnesses.iter().find(|candidate| {
            candidate.id == id || (id == "claude" && candidate.id == "claude-code")
        }),
        None => harnesses
            .iter()
            .find(|candidate| candidate.selected && candidate.installed)
            .or_else(|| harnesses.iter().find(|candidate| candidate.installed)),
    }
    .ok_or("no supported coding harness is installed")?;
    if !selected.installed {
        return Err(format!("{} is not installed", selected.label).into());
    }
    let selected_model = model.unwrap_or("default");
    if !selected
        .models
        .iter()
        .any(|candidate| candidate.id == selected_model)
    {
        return Err(format!(
            "{} does not advertise model `{selected_model}` on this machine",
            selected.label
        )
        .into());
    }
    let selected_route = match route {
        Some(id) => selected.routes.iter().find(|candidate| candidate.id == id),
        None => selected.routes.iter().find(|candidate| candidate.available),
    }
    .ok_or_else(|| {
        format!(
            "{} has no available authenticated inference route",
            selected.label
        )
    })?;
    Ok(HarnessSelection {
        harness: selected.id.clone(),
        model: selected_model.into(),
        route: selected_route.id.clone(),
    })
}

pub fn prepare(
    engine: &Engine,
    creation: &ConductedCreation,
    app_name: &str,
    selection: &HarnessSelection,
    open_terminal_window: bool,
    events: &EventBus,
) -> Result<PreparedExecution, Box<dyn std::error::Error>> {
    // No anonymous Shots: the execution must land attributed to the local
    // Builder identity, so the binding is proven before anything is prepared.
    engine.verify_builder_binding(app_name)?;
    let app = engine.ledger().load_app(app_name)?;
    let shot_id = app
        .shot_id
        .ok_or("the Shot has no canonical identity to bind the execution")?;
    let version_ordinal = u64::from(app.latest_evolution.unwrap_or(0)) + 1;
    let layout = ShotLayout::at(&creation.folder);
    let package = layout.prepared_intent_package()?;
    let references = package
        .references
        .iter()
        .map(|reference| ExecutionReference {
            label: reference.label.clone(),
            relative_path: reference.relative_path.clone(),
            digest: reference.availability.artifact.digest,
            byte_length: reference.availability.artifact.byte_length,
            media_type: reference.availability.artifact.media_type.clone(),
        })
        .collect();
    let mut execution = prepare_execution(
        &creation.folder,
        ExecutionPreparation {
            app_name: app_name.into(),
            shot_id,
            version_ordinal,
            selection: selection.clone(),
            intent_path: package.document_relative_path,
            intention_digest: package.intention_digest,
            references,
        },
    )?;
    if open_terminal_window {
        // The Shot and its execution are durably prepared either way; a
        // Terminal that cannot open must not be reported as an unprepared
        // Shot. The error text already carries the exact manual command.
        match open_terminal(&mut execution) {
            Ok(()) => events.emit(Event::handoff(format!(
                "SHOT PREPARED · {} · {} · waiting for confirmation in Terminal…",
                execution.harness_display_name, execution.model
            ))),
            Err(error) => events.emit(Event::handoff(format!(
                "SHOT PREPARED · {} · {} · the Terminal window could not open — {error}",
                execution.harness_display_name, execution.model
            ))),
        }
    } else {
        events.emit(Event::handoff(format!(
            "SHOT PREPARED · run `tohseno shot run --app {} --execution {}` from {}.",
            app_name,
            execution.execution_id,
            creation.folder.display()
        )));
    }
    Ok(execution)
}

pub fn open_terminal(execution: &mut PreparedExecution) -> Result<(), Box<dyn std::error::Error>> {
    if std::env::consts::OS != "macos" {
        return Err("native terminal preparation currently supports macOS only".into());
    }
    let executable = std::env::current_exe()?;
    let terminal_environment = [
        "TOHSENO_DATA_ROOT",
        "TOHSENO_HOME",
        "TOHSENO_IDENTITY_BACKEND",
        "TOHSENO_APPLE_IDENTITY_HELPER",
    ]
    .into_iter()
    .filter_map(|name| std::env::var_os(name).map(|value| (name, value)))
    .collect::<Vec<_>>();
    let preloaded = build_preloaded_command(
        &executable,
        &execution.app_name,
        &execution.execution_id,
        &terminal_environment,
    );
    let terminal_directory =
        execution_directory(&execution.repository, &execution.execution_id).join("terminal");
    fs::create_dir_all(&terminal_directory)?;
    let user_zdotdir = std::env::var_os("ZDOTDIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .ok_or("HOME is unavailable; cannot prepare the user's zsh session")?;
    let user_rc = user_zdotdir.join(".zshrc");
    let rc = format!(
        "typeset -g TOHSENO_PENDING_LINE=\"$TOHSENO_PRELOAD\"\nunset TOHSENO_PRELOAD\nif [[ -r {} ]]; then\n  source {}\nfi\nprint -z -- \"$TOHSENO_PENDING_LINE\"\nunset TOHSENO_PENDING_LINE\n",
        shell_quote(&user_rc.to_string_lossy()),
        shell_quote(&user_rc.to_string_lossy())
    );
    fs::write(terminal_directory.join(".zshrc"), rc)?;
    let bootstrap = format!(
        "cd {} && TOHSENO_PRELOAD={} ZDOTDIR={} exec /bin/zsh -l",
        shell_quote(&execution.repository.to_string_lossy()),
        shell_quote(&preloaded),
        shell_quote(&terminal_directory.to_string_lossy())
    );
    let (application, script) = match std::env::var("TERM_PROGRAM").as_deref() {
        Ok("iTerm.app") | Ok("iTerm2") => (
            "iTerm",
            format!(
                "tell application \"iTerm\"\nactivate\ncreate window with default profile command \"{}\"\nend tell",
                applescript_string(&bootstrap)
            ),
        ),
        _ => (
            "Terminal",
            format!(
                "tell application \"Terminal\"\nactivate\ndo script \"{}\"\nend tell",
                applescript_string(&bootstrap)
            ),
        ),
    };
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "Terminal could not be prepared: {}. Run this command manually in {}: {}",
            String::from_utf8_lossy(&output.stderr).trim(),
            execution.repository.display(),
            preloaded
        )
        .into());
    }
    update_phase(
        execution,
        ExecutionPhase::TerminalOpened,
        format!(
            "A native {application} window opened with the launch command preloaded and unexecuted."
        ),
    )?;
    Ok(())
}

pub async fn run(
    app_name: &str,
    execution_id: &str,
    json: bool,
    events: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::discover(events.clone())?;
    let repository = engine.ledger().working_tree(app_name);
    let mut execution = load_execution(&repository, execution_id)?;
    if execution.app_name != app_name {
        return Err("execution identity is not bound to the requested app".into());
    }
    if matches!(
        execution.phase,
        ExecutionPhase::ExecutionStarted
            | ExecutionPhase::ContextLoaded
            | ExecutionPhase::HarnessRunning
            | ExecutionPhase::WorkspaceChanged
            | ExecutionPhase::ValidationStarted
    ) {
        return Err("this execution is already in flight".into());
    }
    if matches!(
        execution.phase,
        ExecutionPhase::ExecutionCompleted
            | ExecutionPhase::ExecutionFailed
            | ExecutionPhase::ExecutionCancelled
    ) {
        return Err("this execution already has a terminal outcome".into());
    }

    let intent_path = repository.join(&execution.intent_path);
    let intent_metadata = fs::symlink_metadata(&intent_path)?;
    if intent_metadata.file_type().is_symlink() || !intent_metadata.is_file() {
        return Err("prepared intent is not a regular file".into());
    }
    let images = execution
        .references
        .iter()
        .map(|reference| repository.join(&reference.relative_path))
        .collect::<Vec<_>>();
    for image in &images {
        let metadata = fs::symlink_metadata(image)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!("prepared reference is unavailable: {}", image.display()).into());
        }
    }
    // Re-proven here because `run` is a separate process: identity state may
    // have changed between the prepared Terminal window and this launch.
    engine.verify_builder_binding(app_name)?;

    update_phase(
        &mut execution,
        ExecutionPhase::ExecutionStarted,
        "The user confirmed the prepared Shot in Terminal.",
    )?;
    let started_at = read_events(&repository, execution_id)?
        .last()
        .map(|event| event.timestamp.clone())
        .ok_or("execution start event was not persisted")?;
    update_phase(
        &mut execution,
        ExecutionPhase::ContextLoaded,
        "Reading the current Shot and its prepared intention package.",
    )?;
    let selection = HarnessSelection {
        harness: execution.harness.clone(),
        model: execution.model.clone(),
        route: execution.route.clone(),
    };
    let harness = build_interactive_command(
        &selection,
        Path::new(&execution.intent_path),
        &images
            .iter()
            .map(|path| path.strip_prefix(&repository).unwrap_or(path).to_path_buf())
            .collect::<Vec<_>>(),
    )
    .map_err(|error| format!("harness adapter rejected execution: {error}"))?;
    let mut command = tokio::process::Command::new(&harness.program);
    command
        .args(&harness.arguments)
        .envs(harness.environment)
        .current_dir(&repository)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(false);
    for name in harness.removed_environment {
        command.env_remove(name);
    }
    let mut child = command.spawn()?;
    let child_id = child.id();
    execution.process_id = child_id;
    let harness_report = format!(
        "{} is running in its native interactive terminal interface.",
        execution.harness_display_name
    );
    update_phase(
        &mut execution,
        ExecutionPhase::HarnessRunning,
        harness_report,
    )?;
    events.emit(Event::status("SHOT IN FLIGHT"));
    let started = Instant::now();
    let mut changed_reported = false;
    let mut cancelled = false;
    let status;
    {
        let wait = child.wait();
        tokio::pin!(wait);
        let mut interval = tokio::time::interval(Duration::from_secs(3));
        loop {
            tokio::select! {
                result = &mut wait => {
                    status = result?;
                    break;
                }
                _ = interval.tick() => {
                    if !changed_reported && has_workspace_changed(&execution)? {
                        update_phase(
                            &mut execution,
                            ExecutionPhase::WorkspaceChanged,
                            "The Shot repository changed after the prepared boundary.",
                        )?;
                        changed_reported = true;
                    }
                }
                signal = tokio::signal::ctrl_c() => {
                    signal?;
                    cancelled = true;
                    #[cfg(unix)]
                    if let Some(pid) = child_id {
                        // Forward the terminal cancellation to the authentic
                        // harness without taking over its input/output.
                        unsafe { libc::kill(pid as i32, libc::SIGINT); }
                    }
                }
            }
        }
    }

    let (accepted, validation_passed, validation_evidence) =
        accepted_validation(engine.ledger(), app_name, execution.version_ordinal)?;
    let completion = complete_execution(
        &mut execution,
        started_at,
        started.elapsed().as_secs(),
        status.code(),
        cancelled,
        accepted,
        validation_passed,
        validation_evidence,
    )?;
    present_completion(&completion, json, events)?;
    Ok(())
}

pub async fn follow(
    app_name: &str,
    execution_id: &str,
    json: bool,
    events: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = tohseno_engine::Ledger::discover()?;
    let repository = ledger.working_tree(app_name);
    let mut seen = 0;
    loop {
        let observed = read_events(&repository, execution_id)?;
        for event in observed.iter().skip(seen) {
            if json {
                println!("{}", serde_json::to_string(event)?);
            } else {
                events.emit(Event::status(format!("{} · {}", event.event, event.report)));
            }
        }
        seen = observed.len();
        if let Some(completion) = load_completion(&repository, execution_id)? {
            present_completion(&completion, json, events)?;
            return Ok(());
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(500)) => {}
            signal = tokio::signal::ctrl_c() => {
                signal?;
                return Ok(());
            }
        }
    }
}

pub fn result(
    app_name: &str,
    execution_id: &str,
    json: bool,
    events: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = tohseno_engine::Ledger::discover()?;
    let repository = ledger.working_tree(app_name);
    let completion = load_completion(&repository, execution_id)?
        .ok_or("the execution has no completion record yet")?;
    present_completion(&completion, json, events)
}

pub fn cancel(
    app_name: &str,
    execution_id: &str,
    json: bool,
    events: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = tohseno_engine::Ledger::discover()?;
    let repository = ledger.working_tree(app_name);
    let mut execution = load_execution(&repository, execution_id)?;
    if load_completion(&repository, execution_id)?.is_some() {
        return Err("the execution already has a terminal completion record".into());
    }
    #[cfg(unix)]
    if let Some(pid) = execution.process_id {
        let running = unsafe { libc::kill(pid as i32, 0) } == 0;
        if running {
            return Err(
                "the harness process is still running; cancel it in its authentic terminal with Control-C"
                    .into(),
            );
        }
    }
    let (accepted, validation_passed, evidence) =
        accepted_validation(&ledger, app_name, execution.version_ordinal)?;
    let prepared_at = execution.prepared_at.clone();
    let completion = complete_execution(
        &mut execution,
        prepared_at,
        0,
        None,
        true,
        accepted,
        validation_passed,
        evidence,
    )?;
    present_completion(&completion, json, events)
}

fn present_completion(
    completion: &CompletionRecord,
    json: bool,
    events: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        println!("{}", serde_json::to_string_pretty(completion)?);
        return Ok(());
    }
    if completion.landed {
        events.emit(Event::result(format!(
            "SHOT LANDED · {} file(s) changed · validation passed.",
            completion.files_changed.len()
        )));
    } else {
        events.emit(Event::result(format!(
            "SHOT NOT LANDED · {:?} · {} file(s) changed.",
            completion.outcome,
            completion.files_changed.len()
        )));
    }
    events.emit(Event::handoff(completion.authoritative_next_action.clone()));
    Ok(())
}

fn accepted_validation(
    ledger: &tohseno_engine::Ledger,
    app_name: &str,
    version_ordinal: u64,
) -> Result<(bool, bool, Option<String>), Box<dyn std::error::Error>> {
    let latest = ledger.latest_evolution(app_name)?;
    let accepted = latest
        .as_ref()
        .is_some_and(|version| u64::from(version.number) >= version_ordinal);
    if !accepted {
        return Ok((false, false, None));
    }
    let number = u32::try_from(version_ordinal)
        .map_err(|_| "execution version ordinal exceeds the Apple Evolution range")?;
    let version = ledger.shot(app_name, number)?;
    let passed = verify_completed_evolution(&version).is_ok();
    Ok((
        true,
        passed,
        Some(
            version
                .path
                .join("TOHSENO/conformance.json")
                .display()
                .to_string(),
        ),
    ))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

fn applescript_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "")
        .replace('\n', "\\n")
}

fn build_preloaded_command(
    executable: &Path,
    app_name: &str,
    execution_id: &str,
    environment: &[(&str, std::ffi::OsString)],
) -> String {
    let environment = environment
        .iter()
        .map(|(name, value)| shell_quote(&format!("{name}={}", value.to_string_lossy())))
        .collect::<Vec<_>>();
    let prefix = if environment.is_empty() {
        String::new()
    } else {
        format!("/usr/bin/env {} ", environment.join(" "))
    };
    format!(
        "{prefix}{} shot run --app {} --execution {}",
        shell_quote(&executable.to_string_lossy()),
        shell_quote(app_name),
        shell_quote(execution_id)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_and_applescript_quoting_keep_metacharacters_literal() {
        assert_eq!(shell_quote("a b'c"), "'a b'\\''c'");
        assert_eq!(applescript_string("a\"b\\c"), "a\\\"b\\\\c");
    }

    #[test]
    fn preloaded_command_names_the_durable_execution_only() {
        let executable = Path::new("/Applications/TOHSENO App/tohseno");
        let command = build_preloaded_command(
            executable,
            "field-notebook",
            "0123456789abcdef0123456789abcdef",
            &[(
                "TOHSENO_DATA_ROOT",
                std::ffi::OsString::from("/tmp/Shot Root"),
            )],
        );
        assert!(command.starts_with(
            "/usr/bin/env 'TOHSENO_DATA_ROOT=/tmp/Shot Root' '/Applications/TOHSENO App/tohseno'"
        ));
        assert!(!command.contains("EVOLUTION_INTENT"));
        assert!(!command.contains("API_KEY"));
    }
}
