use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tohseno_engine::harness::{
    build_conception_command, build_interactive_command, build_materialization_command,
    HarnessSelection,
};
use tohseno_engine::protocol_lifecycle::verify_completed_evolution;
use tohseno_engine::shot_execution::{
    complete_execution, execution_directory, has_workspace_changed, load_completion,
    load_execution, prepare_execution, read_events, update_phase,
};
use tohseno_engine::{
    CompletionRecord, ConductedCreation, ConductionPhase, Engine, Event, EventBus, ExecutionMode,
    ExecutionPhase, ExecutionPreparation, ExecutionReference, PreparedExecution, ShotLayout,
};

pub fn selection(
    engine: &Engine,
    harness: Option<&str>,
    model: Option<&str>,
    route: Option<&str>,
) -> Result<HarnessSelection, Box<dyn std::error::Error>> {
    if cfg!(debug_assertions)
        && std::env::var("TOHSENO_TEST_NONLAUNCHING_HARNESS").as_deref() == Ok("1")
        && harness == Some("tohseno-test-nonlaunching")
        && model == Some("fixture")
        && route == Some("no-inference")
    {
        return Ok(HarnessSelection {
            harness: "tohseno-test-nonlaunching".into(),
            model: "fixture".into(),
            route: "no-inference".into(),
        });
    }
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
    auto_accept_genome: bool,
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
            mode: match creation.phase {
                ConductionPhase::Conception => ExecutionMode::Conception,
                ConductionPhase::BirthMaterialization => ExecutionMode::BirthMaterialization,
                ConductionPhase::EvolutionMaterialization => {
                    ExecutionMode::EvolutionMaterialization
                }
            },
            auto_accept_genome,
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
    let started = Instant::now();
    let mut changed_reported = false;
    let relative_images = images
        .iter()
        .map(|path| path.strip_prefix(&repository).unwrap_or(path).to_path_buf())
        .collect::<Vec<_>>();
    let first_command = match execution.mode {
        ExecutionMode::Conception => build_conception_command(
            &selection,
            Path::new(".tohseno/CONCEPTION.md"),
            &relative_images,
        ),
        ExecutionMode::BirthMaterialization => build_materialization_command(
            &selection,
            Path::new(".tohseno/TASK.md"),
            &relative_images,
            None,
        ),
        ExecutionMode::EvolutionMaterialization => build_interactive_command(
            &selection,
            Path::new(&execution.intent_path),
            &relative_images,
        ),
    }
    .map_err(|error| format!("harness adapter rejected execution: {error}"))?;
    let (mut status, mut cancelled) = execute_harness(
        first_command,
        &repository,
        &mut execution,
        &mut changed_reported,
        events,
    )
    .await?;

    let mut validation_diagnostic = None;
    if status.success() && !cancelled && execution.mode == ExecutionMode::Conception {
        let (proposal, expression) = engine.pending_conception(app_name)?;
        present_conception_review(&proposal, &expression, events);
        let accepted = execution.auto_accept_genome
            || (io::stdin().is_terminal()
                && io::stdout().is_terminal()
                && confirm_conception_review()?);
        if accepted {
            engine.accept_pending_conception(app_name)?;
            execution.mode = ExecutionMode::BirthMaterialization;
            update_phase(
                &mut execution,
                ExecutionPhase::ContextLoaded,
                "The app-specific Genome, Birth Plan, organs, and Experience Contract passed deterministic validation and were accepted for materialization.",
            )?;
            let command = build_materialization_command(
                &selection,
                Path::new(".tohseno/TASK.md"),
                &relative_images,
                None,
            )
            .map_err(|error| format!("harness adapter rejected materialization: {error}"))?;
            (status, cancelled) = execute_harness(
                command,
                &repository,
                &mut execution,
                &mut changed_reported,
                events,
            )
            .await?;
        } else {
            validation_diagnostic = Some(format!(
                "The app-specific proposal remains unaccepted. Review it under {}/.tohseno/private/planning and rerun create with --accept-genome.",
                repository.display()
            ));
        }
    }

    if status.success()
        && !cancelled
        && validation_diagnostic.is_none()
        && matches!(
            execution.mode,
            ExecutionMode::BirthMaterialization | ExecutionMode::EvolutionMaterialization
        )
    {
        let maximum_repairs = std::env::var("TOHSENO_MAX_REPAIR_PASSES")
            .or_else(|_| std::env::var("TOHSENO_MAX_BIRTH_REPAIR_PASSES"))
            .ok()
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(3)
            .min(8);
        for pass in 0..=maximum_repairs {
            update_phase(
                &mut execution,
                ExecutionPhase::ValidationStarted,
                "The engine is independently evaluating protocol conformance, intent fidelity, and target-user experience evidence.",
            )?;
            match engine.record(app_name, None).await {
                Ok(_) => {
                    validation_diagnostic = None;
                    break;
                }
                Err(error) => {
                    let diagnostic = error.to_string();
                    validation_diagnostic = Some(diagnostic.clone());
                    let externally_blocked = diagnostic
                        .contains("acceptance_pending_physical_experience")
                        || diagnostic.contains("acceptance_pending_simulator_environment")
                        || diagnostic.contains("factory identity changed")
                        || diagnostic.contains("Developer Mode")
                        || diagnostic.contains("Trust This Computer");
                    if pass == maximum_repairs || externally_blocked {
                        break;
                    }
                    let repair_kind = match execution.mode {
                        ExecutionMode::BirthMaterialization => "BIRTH REPAIR",
                        ExecutionMode::EvolutionMaterialization => "EVOLUTION CANDIDATE REPAIR",
                        ExecutionMode::Conception => {
                            unreachable!("conception is not materialization")
                        }
                    };
                    events.emit(Event::status(format!(
                        "{repair_kind} {}/{} · {diagnostic}",
                        pass + 1,
                        maximum_repairs
                    )));
                    let command = build_materialization_command(
                        &selection,
                        Path::new(".tohseno/TASK.md"),
                        &relative_images,
                        Some(&diagnostic),
                    )
                    .map_err(|adapter| format!("harness adapter rejected repair: {adapter}"))?;
                    (status, cancelled) = execute_harness(
                        command,
                        &repository,
                        &mut execution,
                        &mut changed_reported,
                        events,
                    )
                    .await?;
                    if !status.success() || cancelled {
                        break;
                    }
                }
            }
        }
    }

    let (accepted, validation_passed, accepted_evidence) =
        accepted_validation(engine.ledger(), app_name, execution.version_ordinal)?;
    let validation_evidence = accepted_evidence.or(validation_diagnostic);
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

async fn execute_harness(
    harness: tohseno_engine::HarnessCommand,
    repository: &Path,
    execution: &mut PreparedExecution,
    changed_reported: &mut bool,
    events: &EventBus,
) -> Result<(std::process::ExitStatus, bool), Box<dyn std::error::Error>> {
    let mut command = tokio::process::Command::new(&harness.program);
    command
        .args(&harness.arguments)
        .envs(harness.environment)
        .current_dir(repository)
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
    update_phase(
        execution,
        ExecutionPhase::HarnessRunning,
        format!(
            "{} is running {:?} in its native interactive terminal interface.",
            execution.harness_display_name, execution.mode
        ),
    )?;
    events.emit(Event::status("SHOT IN FLIGHT"));
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
                    if !*changed_reported && has_workspace_changed(execution)? {
                        update_phase(
                            execution,
                            ExecutionPhase::WorkspaceChanged,
                            "The Shot repository changed after the prepared boundary.",
                        )?;
                        *changed_reported = true;
                    }
                }
                signal = tokio::signal::ctrl_c() => {
                    signal?;
                    cancelled = true;
                    #[cfg(unix)]
                    if let Some(pid) = child_id {
                        unsafe { libc::kill(pid as i32, libc::SIGINT); }
                    }
                }
            }
        }
    }
    execution.process_id = None;
    Ok((status, cancelled))
}

fn present_conception_review(
    output: &tohseno_engine::ConceptionOutput,
    expression: &tohseno_engine::BirthExpressionPlan,
    events: &EventBus,
) {
    let actors = output
        .birth_plan
        .target_users
        .iter()
        .map(|actor| actor.role.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let capabilities = output
        .birth_plan
        .capabilities
        .iter()
        .map(|capability| capability.identifier.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let product_organs = expression
        .organs
        .iter()
        .filter(|organ| organ.kind == tohseno_engine::OrganKind::AppSpecific)
        .map(|organ| organ.organ_id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    events.emit(Event::status(format!(
        "APP-SPECIFIC CONCEPTION\nPromise: {}\nTarget users: {actors}\nApple capabilities: {capabilities}\nProduct organs: {product_organs}\nRequired journeys: {}",
        output.birth_plan.promise,
        output.birth_plan.completion_contract.required_scenario_ids.join(", ")
    )));
}

fn confirm_conception_review() -> Result<bool, std::io::Error> {
    print!("Accept this app-specific Genome and materialize the birth? [y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
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
        let (label, acceptance) = if completion.mode == Some(ExecutionMode::BirthMaterialization) {
            (
                "BIRTH ACCEPTED",
                "protocol conformance, intent fidelity, and experience verification passed",
            )
        } else {
            ("VERSION RECORDED", "all declared Version gates passed")
        };
        events.emit(Event::result(format!(
            "{label} · {} file(s) changed · {acceptance}.",
            completion.files_changed.len(),
        )));
    } else {
        events.emit(Event::result(format!(
            "CANDIDATE UNSEALED · {:?} · {} file(s) changed.",
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
