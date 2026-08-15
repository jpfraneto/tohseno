use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tohseno_engine::harness::{
    build_conception_command, build_evolution_command, build_materialization_command,
    HarnessSelection,
};
use tohseno_engine::protocol_lifecycle::verify_completed_evolution;
use tohseno_engine::shot_execution::{
    append_harness_heartbeat, complete_execution, execution_directory, has_workspace_changed,
    load_completion, load_execution, prepare_execution, privacy_safe_workspace_progress,
    read_events, update_phase,
};
use tohseno_engine::{
    CompletionRecord, ConductedCreation, ConductionPhase, Engine, Event, EventBus, ExecutionMode,
    ExecutionOutcome, ExecutionPhase, ExecutionPreparation, ExecutionReference, PreparedExecution,
    ShotLayout,
};

const DEFAULT_REPAIR_PASSES: u8 = 5;
const MAXIMUM_REPAIR_PASSES: u8 = 8;

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
    start_runner: bool,
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
        },
    )?;
    if start_runner {
        // The durable execution exists before its detached runner. The exact
        // recovery command is carried by any spawn error, while callers still
        // receive failure and therefore do not consume imported intention
        // state as though the one-shot run had begun.
        start_background_runner(&mut execution)?;
        events.emit(Event::handoff(format!(
            "SHOT IN FLIGHT · {} · {} · conception, materialization, verification, and phone delivery are running unattended.",
            execution.harness_display_name, execution.model
        )));
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

fn start_background_runner(
    execution: &mut PreparedExecution,
) -> Result<(), Box<dyn std::error::Error>> {
    let executable = std::env::current_exe()?;
    let directory = execution_directory(&execution.repository, &execution.execution_id);
    let log_path = directory.join("harness.log");
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let stderr = stdout.try_clone()?;
    update_phase(
        execution,
        ExecutionPhase::RunnerStarted,
        format!(
            "The unattended Shot runner started; private harness output is retained at {}.",
            log_path.display()
        ),
    )?;
    let mut command = if Path::new("/usr/bin/nohup").is_file() {
        let mut command = Command::new("/usr/bin/nohup");
        command.arg(&executable);
        command
    } else {
        Command::new(&executable)
    };
    command
        .args(runner_arguments(execution))
        .current_dir(&execution.repository)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|error| {
        let _ = update_phase(
            execution,
            ExecutionPhase::Prepared,
            format!("The unattended runner did not start: {error}"),
        );
        format!(
            "{error}. Run `tohseno shot run --app {} --execution {}` from {}",
            execution.app_name,
            execution.execution_id,
            execution.repository.display()
        )
    })?;
    let runner_pid = child.id();
    if let Err(error) = fs::write(runner_pid_path(execution), format!("{runner_pid}\n")) {
        #[cfg(unix)]
        unsafe {
            libc::kill(-(runner_pid as i32), libc::SIGTERM);
        }
        #[cfg(not(unix))]
        let _ = child.kill();
        let _ = child.wait();
        update_phase(
            execution,
            ExecutionPhase::Prepared,
            format!("The unattended runner was stopped because its PID record failed: {error}"),
        )?;
        return Err(format!("the unattended runner PID could not be persisted: {error}").into());
    }
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn runner_arguments(execution: &PreparedExecution) -> Vec<String> {
    vec![
        "shot".into(),
        "run".into(),
        "--app".into(),
        execution.app_name.clone(),
        "--execution".into(),
        execution.execution_id.clone(),
    ]
}

fn runner_pid_path(execution: &PreparedExecution) -> PathBuf {
    execution_directory(&execution.repository, &execution.execution_id).join("runner.pid")
}

pub async fn run(
    app_name: &str,
    execution_id: &str,
    json: bool,
    events: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    // The builder sent the one Shot and walked away; the ending is announced
    // whether the run sealed, stayed unsealed, or stalled before completion.
    match run_shot(app_name, execution_id, json, events).await {
        Ok(completion) => {
            let (headline, message) = completion_notification(app_name, &completion);
            notify_builder(&headline, &message);
            Ok(())
        }
        Err(error) => {
            let diagnostic = error.to_string();
            let _ = finalize_stalled_execution(app_name, execution_id, &diagnostic);
            notify_builder(
                &format!("SHOT STALLED · {app_name}"),
                &format!("{diagnostic}. Inspect the execution events, then start a new execution for the preserved exact intention; final execution records are immutable."),
            );
            Err(error)
        }
    }
}

fn finalize_stalled_execution(
    app_name: &str,
    execution_id: &str,
    diagnostic: &str,
) -> Result<Option<CompletionRecord>, Box<dyn std::error::Error>> {
    let ledger = tohseno_engine::Ledger::discover()?;
    let repository = ledger.working_tree(app_name);
    if let Some(completion) = load_completion(&repository, execution_id)? {
        return Ok(Some(completion));
    }
    let mut execution = load_execution(&repository, execution_id)?;
    if matches!(
        execution.phase,
        ExecutionPhase::ExecutionCompleted
            | ExecutionPhase::ExecutionFailed
            | ExecutionPhase::ExecutionCancelled
    ) {
        return Ok(None);
    }
    let started_at = read_events(&repository, execution_id)?
        .into_iter()
        .find(|event| event.phase == ExecutionPhase::ExecutionStarted)
        .map(|event| event.timestamp)
        .unwrap_or_else(|| execution.prepared_at.clone());
    let (accepted, validation_passed, evidence) =
        accepted_validation(&ledger, app_name, execution.version_ordinal)
            .unwrap_or((false, false, None));
    let evidence = Some(match evidence {
        Some(evidence) => format!("{diagnostic}; {evidence}"),
        None => diagnostic.to_owned(),
    });
    let completion = complete_execution(
        &mut execution,
        started_at,
        0,
        Some(1),
        false,
        accepted,
        validation_passed,
        evidence,
    )?;
    let _ = fs::remove_file(runner_pid_path(&execution));
    Ok(Some(completion))
}

async fn run_shot(
    app_name: &str,
    execution_id: &str,
    json: bool,
    events: &EventBus,
) -> Result<CompletionRecord, Box<dyn std::error::Error>> {
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
        return Err("this execution already has a final outcome".into());
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
    // have changed between durable preparation and runner launch.
    engine.verify_builder_binding(app_name)?;

    update_phase(
        &mut execution,
        ExecutionPhase::ExecutionStarted,
        "The unattended runner claimed the prepared Shot.",
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
            None,
        ),
        ExecutionMode::BirthMaterialization => build_materialization_command(
            &selection,
            Path::new(".tohseno/TASK.md"),
            &relative_images,
            None,
        ),
        ExecutionMode::EvolutionMaterialization => build_evolution_command(
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
        let maximum_conception_repairs = std::env::var("TOHSENO_MAX_CONCEPTION_REPAIR_PASSES")
            .or_else(|_| std::env::var("TOHSENO_MAX_REPAIR_PASSES"))
            .ok()
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(DEFAULT_REPAIR_PASSES)
            .min(MAXIMUM_REPAIR_PASSES);
        let mut validated_conception = None;
        for pass in 0..=maximum_conception_repairs {
            match engine.pending_conception(app_name) {
                Ok(value) => {
                    validated_conception = Some(value);
                    validation_diagnostic = None;
                    break;
                }
                Err(error) => {
                    let diagnostic = error.to_string();
                    validation_diagnostic = Some(diagnostic.clone());
                    if pass == maximum_conception_repairs {
                        break;
                    }
                    events.emit(Event::status(format!(
                        "CONCEPTION REPAIR {}/{} · {diagnostic}",
                        pass + 1,
                        maximum_conception_repairs
                    )));
                    let command = build_conception_command(
                        &selection,
                        Path::new(".tohseno/CONCEPTION.md"),
                        &relative_images,
                        Some(&diagnostic),
                    )
                    .map_err(|adapter| {
                        format!("harness adapter rejected Conception repair: {adapter}")
                    })?;
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
        if let Some((proposal, expression)) = validated_conception {
            present_conception_summary(&proposal, &expression, events);
            engine.accept_pending_conception(app_name)?;
            execution.mode = ExecutionMode::BirthMaterialization;
            update_phase(
                &mut execution,
                ExecutionPhase::ContextLoaded,
                "The app-specific Genome, Birth Plan, organs, and Experience Contract passed deterministic validation and were accepted internally for materialization.",
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
            .unwrap_or(DEFAULT_REPAIR_PASSES)
            .min(MAXIMUM_REPAIR_PASSES);
        for pass in 0..=maximum_repairs {
            update_phase(
                &mut execution,
                ExecutionPhase::ValidationStarted,
                "The engine is independently evaluating protocol conformance, intent fidelity, and target-user experience evidence.",
            )?;
            match engine.record_and_deliver(app_name, None).await {
                Ok(_) => {
                    validation_diagnostic = None;
                    break;
                }
                Err(error) => {
                    let diagnostic = error.to_string();
                    validation_diagnostic = Some(diagnostic.clone());
                    let externally_blocked = is_external_validation_block(&diagnostic);
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
    let _ = fs::remove_file(runner_pid_path(&execution));
    present_completion(&completion, json, events)?;
    Ok(completion)
}

fn is_external_validation_block(diagnostic: &str) -> bool {
    diagnostic.contains("external_environment_constraint")
        || diagnostic.contains("acceptance_pending_physical_experience")
        || diagnostic.contains("acceptance_pending_simulator_environment")
        || diagnostic.contains("factory identity changed")
        || diagnostic.contains("Developer Mode")
        || diagnostic.contains("Trust This Computer")
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
        .stdin(Stdio::null())
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
            "{} is running {:?} as an unattended one-shot process.",
            execution.harness_display_name, execution.mode
        ),
    )?;
    events.emit(Event::status("SHOT IN FLIGHT"));
    let mut cancelled = false;
    let harness_started = Instant::now();
    let mut last_heartbeat = Instant::now();
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
                    if last_heartbeat.elapsed() >= Duration::from_secs(60) {
                        let elapsed_minutes = harness_started.elapsed().as_secs() / 60;
                        let activity = privacy_safe_workspace_progress(execution)
                            .unwrap_or_else(|_| {
                                if *changed_reported {
                                    "workspace changes are present; artifact classification is temporarily unavailable".into()
                                } else {
                                    "no workspace change is visible yet; artifact classification is temporarily unavailable".into()
                                }
                            });
                        append_harness_heartbeat(
                            execution,
                            format!(
                                "{} is still running after {elapsed_minutes} minute(s); {activity}.",
                                execution.harness_display_name
                            ),
                        )?;
                        last_heartbeat = Instant::now();
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

fn present_conception_summary(
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
        return Err("the execution already has a final completion record".into());
    }
    #[cfg(unix)]
    if let Some(runner_pid) = read_runner_pid(&execution)? {
        let running = unsafe { libc::kill(runner_pid as i32, 0) } == 0;
        if running {
            verify_runner_process(runner_pid, app_name, execution_id)?;
            let process_group = unsafe { libc::getpgid(runner_pid as i32) };
            let target = if process_group == runner_pid as i32 {
                -(runner_pid as i32)
            } else {
                runner_pid as i32
            };
            if unsafe { libc::kill(target, libc::SIGTERM) } != 0 {
                return Err("the unattended runner stopped before cancellation reached it".into());
            }
            for _ in 0..40 {
                if unsafe { libc::kill(runner_pid as i32, 0) } != 0 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            if unsafe { libc::kill(runner_pid as i32, 0) } == 0 {
                return Err("the unattended runner did not stop after cancellation".into());
            }
            execution = load_execution(&repository, execution_id)?;
            if let Some(completion) = load_completion(&repository, execution_id)? {
                return present_completion(&completion, json, events);
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
            let _ = fs::remove_file(runner_pid_path(&execution));
            return present_completion(&completion, json, events);
        }
        let _ = fs::remove_file(runner_pid_path(&execution));
    }
    let mut dead_recorded_child = false;
    #[cfg(unix)]
    if let Some(pid) = execution.process_id {
        let running = unsafe { libc::kill(pid as i32, 0) } == 0;
        if running {
            return Err(
                "a legacy/manual harness process is still running; stop that process before finalizing cancellation"
                    .into(),
            );
        }
        dead_recorded_child = true;
        execution.process_id = None;
    }
    if matches!(
        execution.phase,
        ExecutionPhase::RunnerStarted
            | ExecutionPhase::ExecutionStarted
            | ExecutionPhase::ContextLoaded
            | ExecutionPhase::HarnessRunning
            | ExecutionPhase::WorkspaceChanged
            | ExecutionPhase::ValidationStarted
    ) && !dead_recorded_child
    {
        return Err(
            "the unattended runner is between cancellable harness phases; wait for the next event and retry"
                .into(),
        );
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

fn read_runner_pid(execution: &PreparedExecution) -> Result<Option<u32>, std::io::Error> {
    let path = runner_pid_path(execution);
    let body = match fs::read_to_string(path) {
        Ok(body) => body,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let pid = body.trim().parse::<u32>().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the unattended runner PID record is malformed",
        )
    })?;
    if pid <= 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the unattended runner PID is unsafe",
        ));
    }
    Ok(Some(pid))
}

#[cfg(unix)]
fn verify_runner_process(
    pid: u32,
    app_name: &str,
    execution_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()?;
    let command = String::from_utf8_lossy(&output.stdout);
    let expected = format!("shot run --app {app_name} --execution {execution_id}");
    if !output.status.success() || !command.contains(&expected) {
        return Err("the stored runner PID no longer identifies this exact Shot execution".into());
    }
    Ok(())
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

fn completion_notification(app_name: &str, completion: &CompletionRecord) -> (String, String) {
    let headline = if completion.landed {
        if completion.mode == Some(ExecutionMode::BirthMaterialization) {
            format!("BIRTH ACCEPTED · {app_name}")
        } else {
            format!("VERSION RECORDED · {app_name}")
        }
    } else if completion.outcome == ExecutionOutcome::Cancelled {
        format!("SHOT CANCELLED · {app_name}")
    } else {
        format!("CANDIDATE UNSEALED · {app_name}")
    };
    (headline, completion.authoritative_next_action.clone())
}

/// Announce the ending of unattended work on the builder's own machine. The
/// notification is a courtesy signal, never part of the record: it must not
/// alter the Shot outcome, so every failure to display it is absorbed.
fn notify_builder(headline: &str, message: &str) {
    if std::env::consts::OS != "macos" {
        return;
    }
    let script = format!(
        "display notification \"{}\" with title \"TOHSENO\" subtitle \"{}\" sound name \"Glass\"",
        applescript_text(message),
        applescript_text(headline),
    );
    let _ = Command::new("osascript")
        .args(["-e", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn applescript_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tohseno_engine::shot_execution::GitBoundary;

    #[test]
    fn background_runner_names_only_the_durable_execution() {
        let execution = PreparedExecution {
            schema: "fixture".into(),
            execution_id: "0123456789abcdef0123456789abcdef".into(),
            shot_id: tohseno_protocol::digest::ShotId::from_bytes([0x42; 32]),
            version_ordinal: 1,
            app_name: "field-notebook".into(),
            repository: PathBuf::from("/tmp/Shot Root"),
            harness: "codex".into(),
            harness_display_name: "Codex".into(),
            model: "default".into(),
            route: "chatgpt-subscription".into(),
            route_billing: "subscription".into(),
            estimated_additional_cost_usd: Some(0.0),
            intention_digest: tohseno_protocol::digest::sha256(b"fixture"),
            intent_path: ".tohseno/EVOLUTION_INTENT.md".into(),
            references: Vec::new(),
            mode: ExecutionMode::Conception,
            auto_accept_genome: false,
            baseline: GitBoundary {
                tree: "0".repeat(40),
                head: None,
                pre_existing_status: Vec::new(),
            },
            prepared_at: "2026-08-05T00:00:00Z".into(),
            process_id: None,
            phase: ExecutionPhase::Prepared,
        };
        assert_eq!(
            runner_arguments(&execution),
            [
                "shot",
                "run",
                "--app",
                "field-notebook",
                "--execution",
                "0123456789abcdef0123456789abcdef"
            ]
        );
    }

    fn completion_fixture(
        mode: ExecutionMode,
        outcome: ExecutionOutcome,
        landed: bool,
        next_action: &str,
    ) -> CompletionRecord {
        CompletionRecord {
            schema: "fixture".into(),
            execution_id: "0123456789abcdef0123456789abcdef".into(),
            shot_id: tohseno_protocol::digest::ShotId::from_bytes([0x42; 32]),
            version_ordinal: 1,
            mode: Some(mode),
            outcome,
            landed,
            harness: "claude-code".into(),
            model: "default".into(),
            route: "claude-subscription".into(),
            intention_digest: tohseno_protocol::digest::sha256(b"fixture"),
            reference_digests: Vec::new(),
            started_at: "2026-08-05T00:00:00Z".into(),
            ended_at: "2026-08-05T01:00:00Z".into(),
            duration_seconds: 3600,
            exit_code: Some(0),
            files_changed: Vec::new(),
            git_diff_summary: String::new(),
            baseline_tree: "0".repeat(40),
            final_tree: "0".repeat(40),
            pre_existing_worktree_status: Vec::new(),
            validation_commands_executed: Vec::new(),
            validation_results: Vec::new(),
            harness_provided_final_summary: None,
            independently_computed_repository_state: String::new(),
            estimated_additional_cost_usd: Some(0.0),
            actual_additional_cost_usd: None,
            authoritative_next_action: next_action.into(),
        }
    }

    #[test]
    fn completion_notification_announces_every_final_outcome() {
        let (headline, message) = completion_notification(
            "field-notebook",
            &completion_fixture(
                ExecutionMode::BirthMaterialization,
                ExecutionOutcome::Completed,
                true,
                "Experience the accepted app now running on the paired iPhone.",
            ),
        );
        assert_eq!(headline, "BIRTH ACCEPTED · field-notebook");
        assert_eq!(
            message,
            "Experience the accepted app now running on the paired iPhone."
        );

        let (headline, _) = completion_notification(
            "field-notebook",
            &completion_fixture(
                ExecutionMode::EvolutionMaterialization,
                ExecutionOutcome::Completed,
                true,
                "Experience the accepted app.",
            ),
        );
        assert_eq!(headline, "VERSION RECORDED · field-notebook");

        let (headline, _) = completion_notification(
            "field-notebook",
            &completion_fixture(
                ExecutionMode::BirthMaterialization,
                ExecutionOutcome::Failed,
                false,
                "Inspect the execution events and retry the prepared Shot.",
            ),
        );
        assert_eq!(headline, "CANDIDATE UNSEALED · field-notebook");

        let (headline, _) = completion_notification(
            "field-notebook",
            &completion_fixture(
                ExecutionMode::BirthMaterialization,
                ExecutionOutcome::Cancelled,
                false,
                "Inspect the execution events and retry the prepared Shot.",
            ),
        );
        assert_eq!(headline, "SHOT CANCELLED · field-notebook");
    }

    #[test]
    fn notification_text_stays_inside_one_applescript_string() {
        assert_eq!(
            applescript_text("engine said \"no\"\nacross\ttwo lines"),
            "engine said \\\"no\\\" across two lines"
        );
        assert_eq!(applescript_text("back\\slash"), "back\\\\slash");
    }

    #[test]
    fn typed_external_constraints_stop_automatic_repair() {
        assert!(is_external_validation_block(
            "incompleteness.backend [external_environment_constraint]: DNS is unavailable"
        ));
        assert!(!is_external_validation_block(
            "incompleteness.upload [product_gap]: retry is broken"
        ));
    }
}
