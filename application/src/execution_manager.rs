use crate::factory_lease::FactoryLease;
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
    load_completion, load_execution, prepare_execution, prepare_execution_with_id,
    privacy_safe_workspace_progress, read_events, update_phase,
};
use tohseno_engine::{
    CompletionRecord, ConductedCreation, ConductionPhase, DevicePipeline, Engine, EngineError,
    Event, EventBus, ExecutionMode, ExecutionOutcome, ExecutionPhase, ExecutionPreparation,
    ExecutionReference, FactoryStage, PreparedExecution, ShotLayout,
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
        && harness.is_none()
        && model.is_none()
        && route.is_none()
        && tohseno_engine::harness::test_factory_harness_program()?.is_some()
    {
        return Ok(HarnessSelection {
            harness: "tohseno-test-factory".into(),
            model: "fixture".into(),
            route: "no-inference".into(),
        });
    }
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

/// Re-proves a previously selected harness immediately before the application
/// service accepts a command. This keeps an unavailable or unauthenticated
/// route from leaving a half-born Shot merely because selection was resolved
/// earlier by a frontend.
pub fn validate_selection(selection: &HarnessSelection) -> Result<(), Box<dyn std::error::Error>> {
    tohseno_engine::harness::resolve_selection(selection)
        .map(|_| ())
        .map_err(Into::into)
}

pub fn prepare(
    engine: &Engine,
    creation: &ConductedCreation,
    app_name: &str,
    selection: &HarnessSelection,
    start_runner: bool,
    events: &EventBus,
) -> Result<PreparedExecution, Box<dyn std::error::Error>> {
    prepare_inner(
        engine,
        creation,
        app_name,
        selection,
        start_runner,
        events,
        None,
    )
}

/// Prepares an execution whose identity is stable for one durable command.
pub fn prepare_for_command(
    engine: &Engine,
    creation: &ConductedCreation,
    app_name: &str,
    selection: &HarnessSelection,
    start_runner: bool,
    events: &EventBus,
    command_id: &str,
) -> Result<PreparedExecution, Box<dyn std::error::Error>> {
    let execution_id = command_execution_id(command_id);
    prepare_inner(
        engine,
        creation,
        app_name,
        selection,
        start_runner,
        events,
        Some(&execution_id),
    )
}

/// Stable private execution identity reserved by one durable command.
pub fn command_execution_id(command_id: &str) -> String {
    let material = format!("TOHSENO-COMMAND-EXECUTION-V1\0{command_id}");
    tohseno_protocol::digest::sha256(material.as_bytes())
        .to_string()
        .trim_start_matches("0x")
        .chars()
        .take(32)
        .collect()
}

pub fn load_command_execution(
    repository: &Path,
    command_id: &str,
) -> Result<Option<PreparedExecution>, Box<dyn std::error::Error>> {
    let execution_id = command_execution_id(command_id);
    match load_execution(repository, &execution_id) {
        Ok(execution) => Ok(Some(execution)),
        Err(tohseno_engine::ShotExecutionError::Io(error))
            if error.kind() == std::io::ErrorKind::NotFound =>
        {
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_inner(
    engine: &Engine,
    creation: &ConductedCreation,
    app_name: &str,
    selection: &HarnessSelection,
    start_runner: bool,
    events: &EventBus,
    execution_id: Option<&str>,
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
    let preparation = ExecutionPreparation {
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
            ConductionPhase::EvolutionMaterialization => ExecutionMode::EvolutionMaterialization,
        },
    };
    let mut execution = match execution_id {
        Some(execution_id) => {
            prepare_execution_with_id(&creation.folder, preparation, execution_id)?
        }
        None => prepare_execution(&creation.folder, preparation)?,
    };
    if start_runner && execution.phase == ExecutionPhase::Prepared {
        // The durable execution exists before its detached runner. The exact
        // recovery command is carried by any spawn error, while callers still
        // receive failure and therefore do not consume imported intention
        // state as though the one-shot run had begun.
        execution =
            ensure_background_runner(&execution.repository, &execution.execution_id, events)?;
    } else if !start_runner && execution_id.is_none() {
        events.emit(Event::handoff(format!(
            "SHOT PREPARED · run `tohseno shot run --app {} --execution {}` from {}.",
            app_name,
            execution.execution_id,
            creation.folder.display()
        )));
    }
    Ok(execution)
}

/// Starts a durably prepared execution exactly once.
///
/// Application commands publish their stable receipt before calling this
/// function. A crash in that small window is recovered by invoking this on a
/// receipt retry; the execution-scoped lock prevents duplicate frontend
/// requests from launching two runners.
pub fn ensure_background_runner(
    repository: &Path,
    execution_id: &str,
    events: &EventBus,
) -> Result<PreparedExecution, Box<dyn std::error::Error>> {
    // Validate every user-controlled ancestor before creating the lock. The
    // engine read also binds the directory to this repository and execution.
    let initial = load_execution(repository, execution_id)?;
    let directory = checked_runner_directory(&initial)?;
    let mut lock_options = OpenOptions::new();
    lock_options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        lock_options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let lock = lock_options.open(directory.join("runner.lock"))?;
    if !lock.metadata()?.is_file() {
        return Err("execution runner lock is not a regular file".into());
    }
    let _guard = RunnerLock::acquire(lock)?;
    let mut execution = load_execution(repository, execution_id)?;
    let runner_alive = recorded_runner_is_alive(&execution)?;
    let process_tree_claimed = runner_claim_is_held(&execution)?;
    match runner_recovery_disposition(execution.phase, runner_alive || process_tree_claimed) {
        RunnerRecoveryDisposition::Start => {
            remove_stale_runner_pid(&execution)?;
            start_background_runner(&mut execution)?;
            events.emit(Event::handoff(format!(
                "SHOT IN FLIGHT · {} · {} · conception, materialization, verification, and phone delivery are running unattended.",
                execution.harness_display_name, execution.model
            )));
        }
        RunnerRecoveryDisposition::ResumeDelivery => {
            // Waiting is the only post-harness checkpoint that is safe to
            // resume: source work has ended and no accepted Version exists.
            remove_stale_runner_pid(&execution)?;
            spawn_background_runner(&execution)?;
            events.emit(Event::handoff(
                "SHOT RESUMED · verified source is waiting for the configured iPhone.",
            ));
        }
        RunnerRecoveryDisposition::FinalizeAbandoned => {
            finalize_stalled_execution(
                &execution.app_name,
                &execution.execution_id,
                "the exact unattended runner stopped before publishing a terminal record",
            )?;
            execution = load_execution(repository, execution_id)?;
        }
        RunnerRecoveryDisposition::Observe => {}
    }
    Ok(execution)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RunnerRecoveryDisposition {
    Start,
    ResumeDelivery,
    FinalizeAbandoned,
    Observe,
}

fn runner_recovery_disposition(
    phase: ExecutionPhase,
    runner_alive: bool,
) -> RunnerRecoveryDisposition {
    if runner_alive {
        return RunnerRecoveryDisposition::Observe;
    }
    match phase {
        ExecutionPhase::Prepared | ExecutionPhase::RunnerStarted => {
            RunnerRecoveryDisposition::Start
        }
        ExecutionPhase::WaitingForDevice => RunnerRecoveryDisposition::ResumeDelivery,
        ExecutionPhase::TerminalOpened
        | ExecutionPhase::ExecutionStarted
        | ExecutionPhase::ContextLoaded
        | ExecutionPhase::Conception
        | ExecutionPhase::Materializing
        | ExecutionPhase::HarnessRunning
        | ExecutionPhase::WorkspaceChanged
        | ExecutionPhase::Building
        | ExecutionPhase::Testing
        | ExecutionPhase::Verifying
        | ExecutionPhase::Repairing
        | ExecutionPhase::Installing
        | ExecutionPhase::Launching
        | ExecutionPhase::ValidationStarted
        | ExecutionPhase::ValidationCompleted => RunnerRecoveryDisposition::FinalizeAbandoned,
        ExecutionPhase::ExecutionCompleted
        | ExecutionPhase::ExecutionFailed
        | ExecutionPhase::ExecutionCancelled => RunnerRecoveryDisposition::Observe,
    }
}

fn recorded_runner_is_alive(
    execution: &PreparedExecution,
) -> Result<bool, Box<dyn std::error::Error>> {
    let Some(pid) = read_runner_pid(execution)? else {
        return Ok(false);
    };
    #[cfg(unix)]
    {
        Ok(verify_runner_process(pid, &execution.app_name, &execution.execution_id).is_ok())
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        Ok(true)
    }
}

fn remove_stale_runner_pid(
    execution: &PreparedExecution,
) -> Result<(), Box<dyn std::error::Error>> {
    checked_runner_directory(execution)?;
    let path = runner_pid_path(execution);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("execution runner PID path is unsafe".into())
        }
        Ok(_) => {
            fs::remove_file(path)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

struct RunnerLock(std::fs::File);

struct ProcessTreeClaim {
    _file: std::fs::File,
}

impl RunnerLock {
    fn acquire(file: std::fs::File) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        Ok(Self(file))
    }

    fn try_acquire(file: std::fs::File) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                let error = std::io::Error::last_os_error();
                let code = error.raw_os_error();
                if code == Some(libc::EWOULDBLOCK) || code == Some(libc::EAGAIN) {
                    return Err(Box::new(RunnerAlreadyClaimed));
                }
                return Err(error.into());
            }
        }
        Ok(Self(file))
    }
}

impl Drop for RunnerLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let _ = unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

#[derive(Debug)]
struct RunnerAlreadyClaimed;

impl std::fmt::Display for RunnerAlreadyClaimed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("this exact execution already has an active runner")
    }
}

impl std::error::Error for RunnerAlreadyClaimed {}

fn claim_execution_runner(
    execution: &PreparedExecution,
) -> Result<ProcessTreeClaim, Box<dyn std::error::Error>> {
    let directory = checked_runner_directory(execution)?;
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(directory.join("run.claim"))?;
    if !file.metadata()?.is_file() {
        return Err("execution runner claim is not a regular file".into());
    }
    #[cfg(unix)]
    {
        // The exact harness/build subprocess tree inherits this harmless lock
        // descriptor. If its supervisor crashes, recovery observes ownership
        // until the last descendant exits, avoiding both PID-reuse hangs and
        // a second execution mutating the same Shot concurrently.
        use std::os::fd::AsRawFd;
        let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) };
        if flags < 0
            || unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, flags & !libc::FD_CLOEXEC) }
                != 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = std::io::Error::last_os_error();
            let code = error.raw_os_error();
            if code == Some(libc::EWOULDBLOCK) || code == Some(libc::EAGAIN) {
                return Err(Box::new(RunnerAlreadyClaimed));
            }
            return Err(error.into());
        }
    }
    // Intentionally no explicit LOCK_UN on drop. A spawned descendant shares
    // this open-file description, so closing the supervisor's copy keeps the
    // claim held until the final inherited descriptor is closed.
    Ok(ProcessTreeClaim { _file: file })
}

fn runner_claim_is_held(execution: &PreparedExecution) -> Result<bool, Box<dyn std::error::Error>> {
    let directory = checked_runner_directory(execution)?;
    let path = directory.join("run.claim");
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !file.metadata()?.is_file() {
        return Err("execution runner claim is not a regular file".into());
    }
    match RunnerLock::try_acquire(file) {
        Ok(lock) => {
            drop(lock);
            Ok(false)
        }
        Err(error) if error.downcast_ref::<RunnerAlreadyClaimed>().is_some() => Ok(true),
        Err(error) => Err(error),
    }
}

fn start_background_runner(
    execution: &mut PreparedExecution,
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = execution_directory(&execution.repository, &execution.execution_id);
    let log_path = directory.join("harness.log");
    update_phase(
        execution,
        ExecutionPhase::RunnerStarted,
        format!(
            "The unattended Shot runner started; private harness output is retained at {}.",
            log_path.display()
        ),
    )?;
    if let Err(error) = spawn_background_runner(execution) {
        update_phase(
            execution,
            ExecutionPhase::Prepared,
            format!("The unattended runner did not start: {error}"),
        )?;
        return Err(error);
    }
    Ok(())
}

fn spawn_background_runner(
    execution: &PreparedExecution,
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = checked_runner_directory(execution)?;
    let log_path = directory.join("harness.log");
    let mut log_options = OpenOptions::new();
    log_options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        log_options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let stdout = log_options.open(&log_path)?;
    if !stdout.metadata()?.is_file() {
        return Err("private harness log is not a regular file".into());
    }
    let stderr = stdout.try_clone()?;
    let executable = std::env::current_exe()?;
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
        format!(
            "{error}. Run `tohseno shot run --app {} --execution {}` from {}",
            execution.app_name,
            execution.execution_id,
            execution.repository.display()
        )
    })?;
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

fn remove_runner_pid(execution: &PreparedExecution) -> std::io::Result<()> {
    checked_runner_directory(execution)?;
    match fs::remove_file(runner_pid_path(execution)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn checked_runner_directory(execution: &PreparedExecution) -> std::io::Result<PathBuf> {
    let private = execution.repository.join(".tohseno");
    let executions = private.join("executions");
    let directory = executions.join(&execution.execution_id);
    for path in [&execution.repository, &private, &executions, &directory] {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "execution runner storage has an unsafe directory component",
            ));
        }
    }
    Ok(directory)
}

fn write_runner_pid(execution: &PreparedExecution, runner_pid: u32) -> std::io::Result<()> {
    use std::io::Write;
    checked_runner_directory(execution)?;
    let destination = runner_pid_path(execution);
    if let Some(existing) = read_runner_pid(execution)? {
        return if existing == runner_pid {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "another exact runner already owns this execution",
            ))
        };
    }
    let directory = destination.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "runner PID destination has no parent",
        )
    })?;
    let temporary = directory.join(format!(".runner-pid-{}", uuid::Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(&temporary)?;
    writeln!(file, "{runner_pid}")?;
    file.sync_all()?;
    match fs::hard_link(&temporary, &destination) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = read_runner_pid(execution)?;
            if existing != Some(runner_pid) {
                let _ = fs::remove_file(&temporary);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "another exact runner already owns this execution",
                ));
            }
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    }
    fs::remove_file(temporary)?;
    std::fs::File::open(directory)?.sync_all()
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
        Err(error) if error.downcast_ref::<RunnerAlreadyClaimed>().is_some() => {
            // A launch-recovery race may briefly start two identical child
            // processes. Only the process holding the execution claim may do
            // work; the loser exits quietly and must never poison the valid
            // runner's durable outcome.
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
    let mut execution = load_execution(&repository, execution_id)?;
    if let Some(completion) = load_completion(&repository, execution_id)? {
        let landed_evidence_valid = if completion.landed {
            let (accepted, validation_passed, _) =
                accepted_validation(&ledger, app_name, execution.version_ordinal)?;
            accepted && validation_passed
        } else {
            true
        };
        reconcile_published_completion(&mut execution, &completion, landed_evidence_valid)?;
        let _ = remove_runner_pid(&execution);
        return Ok(Some(completion));
    }
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
    let _ = remove_runner_pid(&execution);
    Ok(Some(completion))
}

fn reconcile_published_completion(
    execution: &mut PreparedExecution,
    completion: &CompletionRecord,
    landed_evidence_valid: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if completion.schema != tohseno_engine::shot_execution::COMPLETION_RECORD_SCHEMA
        || completion.execution_id != execution.execution_id
        || completion.shot_id != execution.shot_id
        || completion.version_ordinal != execution.version_ordinal
        || completion.mode != Some(execution.mode)
        || completion.harness != execution.harness
        || completion.model != execution.model
        || completion.route != execution.route
        || completion.intention_digest != execution.intention_digest
        || completion.reference_digests
            != execution
                .references
                .iter()
                .map(|reference| reference.digest)
                .collect::<Vec<_>>()
    {
        return Err("execution completion does not match its durable preparation".into());
    }
    if completion.landed && !landed_evidence_valid {
        return Err(
            "execution completion claims acceptance without an independently verified Version"
                .into(),
        );
    }
    let (phase, report) = if completion.landed {
        (
            ExecutionPhase::ExecutionCompleted,
            "Recovered the independently verified accepted execution outcome.",
        )
    } else if completion.outcome == ExecutionOutcome::Cancelled {
        (
            ExecutionPhase::ExecutionCancelled,
            "Recovered the cancelled execution outcome.",
        )
    } else {
        (
            ExecutionPhase::ExecutionFailed,
            "Recovered the unsealed execution outcome; no incomplete Version was accepted.",
        )
    };
    let current_is_terminal = terminal_execution_phase(execution.phase);
    if current_is_terminal && execution.phase != phase {
        return Err("execution terminal phase contradicts its durable completion".into());
    }
    let has_terminal_event = terminal_event_present(execution, phase)?;
    if !current_is_terminal && has_terminal_event {
        return Err("execution terminal event precedes its durable terminal phase".into());
    }
    if !has_terminal_event {
        // `update_phase` publishes execution.json before appending the event.
        // Repeating it with the same terminal phase repairs precisely that
        // crash window; the validated scan above prevents a duplicate event.
        update_phase(execution, phase, report)?;
        if !terminal_event_present(execution, phase)? {
            return Err("execution terminal event was not durably published".into());
        }
    }
    Ok(())
}

fn terminal_execution_phase(phase: ExecutionPhase) -> bool {
    matches!(
        phase,
        ExecutionPhase::ExecutionCompleted
            | ExecutionPhase::ExecutionFailed
            | ExecutionPhase::ExecutionCancelled
    )
}

fn terminal_event_present(
    execution: &PreparedExecution,
    expected: ExecutionPhase,
) -> Result<bool, Box<dyn std::error::Error>> {
    let events = match read_events(&execution.repository, &execution.execution_id) {
        Ok(events) => events,
        Err(tohseno_engine::ShotExecutionError::Io(error))
            if error.kind() == std::io::ErrorKind::NotFound =>
        {
            Vec::new()
        }
        Err(error) => return Err(error.into()),
    };
    let terminal = events
        .iter()
        .filter(|event| terminal_execution_phase(event.phase))
        .collect::<Vec<_>>();
    if terminal.len() > 1 {
        return Err("execution event journal contains duplicate terminal events".into());
    }
    let Some(event) = terminal.first() else {
        return Ok(false);
    };
    if event.phase != expected || events.last().map(|event| event.sequence) != Some(event.sequence)
    {
        return Err("execution event journal contains a conflicting terminal event".into());
    }
    Ok(true)
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
    // The child, not its launcher, owns a nonblocking process-lifetime claim.
    // A recovery race can spawn two identical children after a crash, but
    // only one can publish its PID or touch the Shot.
    let _runner_claim = claim_execution_runner(&execution)?;
    write_runner_pid(&execution, std::process::id())?;
    if matches!(
        execution.phase,
        ExecutionPhase::ExecutionStarted
            | ExecutionPhase::ContextLoaded
            | ExecutionPhase::Conception
            | ExecutionPhase::Materializing
            | ExecutionPhase::HarnessRunning
            | ExecutionPhase::WorkspaceChanged
            | ExecutionPhase::Building
            | ExecutionPhase::Testing
            | ExecutionPhase::Verifying
            | ExecutionPhase::Repairing
            | ExecutionPhase::Installing
            | ExecutionPhase::Launching
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

    if execution.phase == ExecutionPhase::WaitingForDevice {
        return resume_waiting_delivery(&engine, app_name, &mut execution, json, events).await;
    }

    // Expensive local work is serialized by one advisory lease. A durably
    // admitted command that arrives while the factory is busy simply stays in
    // its `queued` phase — presented everywhere as "Waiting to build…" — and
    // starts by itself the moment the lease frees.
    let mut lease = Some(take_factory_lease(&engine, events).await?);

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
    let first_phase = if execution.mode == ExecutionMode::Conception {
        ExecutionPhase::Conception
    } else {
        ExecutionPhase::Materializing
    };
    let (mut status, mut cancelled) = execute_harness(
        first_command,
        &repository,
        &mut execution,
        &mut changed_reported,
        first_phase,
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
                        ExecutionPhase::Repairing,
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
                ExecutionPhase::Materializing,
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
                ExecutionPhase::Materializing,
                "The engine is beginning its deterministic materialization, build, test, verification, and delivery gates.",
            )?;
            match record_and_deliver_with_wait(
                &engine,
                app_name,
                &mut execution,
                &mut lease,
                events,
            )
            .await
            {
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
                        ExecutionPhase::Repairing,
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
    let _ = remove_runner_pid(&execution);
    // Release the local factory before the terminal record is presented so a
    // waiting execution starts as early as possible.
    drop(lease.take());
    present_completion(&completion, json, events)?;
    Ok(completion)
}

/// Take the exclusive local-factory lease, reporting once if another execution
/// is currently using the coding harness or Xcode.
///
/// No execution event is appended while waiting: the durable phase is already
/// the honest answer, and every surface presents it as "Waiting to build…".
/// Writing a `HarnessRunning` heartbeat here would claim work that has not
/// started.
async fn take_factory_lease(
    engine: &Engine,
    events: &EventBus,
) -> Result<FactoryLease, Box<dyn std::error::Error>> {
    let machine_root = engine.ledger().machine_root().to_path_buf();
    FactoryLease::acquire(&machine_root, || {
        events.emit(Event::status(
            "WAITING · this Mac is building another app; this one starts automatically.",
        ));
        Ok(())
    })
    .await
}

/// Resume the only safe mid-flight checkpoint without invoking the coding
/// harness again. The source candidate already reached the device boundary;
/// the Mac remains authoritative and repeats deterministic verification before
/// it can accept the exact Version.
async fn resume_waiting_delivery(
    engine: &Engine,
    app_name: &str,
    execution: &mut PreparedExecution,
    json: bool,
    events: &EventBus,
) -> Result<CompletionRecord, Box<dyn std::error::Error>> {
    let repository = engine.ledger().working_tree(app_name);
    let started_at = read_events(&repository, &execution.execution_id)?
        .into_iter()
        .find(|event| event.phase == ExecutionPhase::ExecutionStarted)
        .map(|event| event.timestamp)
        .unwrap_or_else(|| execution.prepared_at.clone());
    DevicePipeline::new(events.clone())
        .wait_for_device()
        .await?;
    // Waiting for a cable costs nothing, so the lease is taken only once the
    // iPhone is actually here and deterministic delivery is about to run.
    let mut lease = Some(take_factory_lease(engine, events).await?);
    update_phase(
        execution,
        ExecutionPhase::Materializing,
        "The configured iPhone is available; the exact candidate resumed at the deterministic delivery pipeline.",
    )?;
    let diagnostic = record_and_deliver_with_wait(engine, app_name, execution, &mut lease, events)
        .await
        .err()
        .map(|error| error.to_string());
    let (accepted, validation_passed, accepted_evidence) =
        accepted_validation(engine.ledger(), app_name, execution.version_ordinal)?;
    let completion = complete_execution(
        execution,
        started_at,
        0,
        Some(if diagnostic.is_none() { 0 } else { 1 }),
        false,
        accepted,
        validation_passed,
        accepted_evidence.or(diagnostic),
    )?;
    let _ = remove_runner_pid(execution);
    drop(lease.take());
    present_completion(&completion, json, events)?;
    Ok(completion)
}

/// Let the engine perform every device-independent build and verification
/// step. If the final device gate is unavailable, the engine returns while
/// releasing its Shot lock; this function durably publishes the waiting phase,
/// waits outside that lock, and retries the same exact execution.
async fn record_and_deliver_with_wait(
    engine: &Engine,
    app_name: &str,
    execution: &mut PreparedExecution,
    lease: &mut Option<FactoryLease>,
    events: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let result = {
            let mut stage_events = events.subscribe();
            let operation = async {
                if cfg!(debug_assertions)
                    && std::env::var("TOHSENO_TEST_FACTORY_NO_DEVICE").as_deref() == Ok("1")
                {
                    engine.record(app_name, None).await
                } else {
                    engine.record_and_deliver(app_name, None).await
                }
            };
            tokio::pin!(operation);
            loop {
                tokio::select! {
                    result = &mut operation => break result,
                    event = stage_events.recv() => {
                        if let Ok(Event::FactoryStage(stage)) = event {
                            let phase = execution_phase_for_factory_stage(stage);
                            if execution.phase != phase {
                                update_phase(
                                    execution,
                                    phase,
                                    format!("{} is in progress on this Mac.", stage.label()),
                                )?;
                            }
                        }
                    }
                }
            }
        };
        match result {
            Ok(_) => return Ok(()),
            Err(EngineError::DeviceUnavailable(_)) => {
                update_phase(
                    execution,
                    ExecutionPhase::WaitingForDevice,
                    "The verified candidate is waiting for the configured iPhone before installation, launch, and acceptance.",
                )?;
                // Source and build work already finished. Holding the local
                // factory while a cable is missing would block unrelated apps
                // for no reason, so the lease goes back until the phone is here.
                drop(lease.take());
                DevicePipeline::new(events.clone())
                    .wait_for_device()
                    .await?;
                *lease = Some(take_factory_lease(engine, events).await?);
                update_phase(
                    execution,
                    ExecutionPhase::ValidationStarted,
                    "The configured iPhone is available; deterministic verification and final delivery resumed.",
                )?;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn execution_phase_for_factory_stage(stage: FactoryStage) -> ExecutionPhase {
    match stage {
        FactoryStage::Planning => ExecutionPhase::ContextLoaded,
        FactoryStage::Conception => ExecutionPhase::Conception,
        FactoryStage::Materializing => ExecutionPhase::Materializing,
        FactoryStage::Building => ExecutionPhase::Building,
        FactoryStage::Testing => ExecutionPhase::Testing,
        FactoryStage::Verifying => ExecutionPhase::Verifying,
        FactoryStage::Repairing => ExecutionPhase::Repairing,
        FactoryStage::Installing => ExecutionPhase::Installing,
        FactoryStage::Launching => ExecutionPhase::Launching,
    }
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
    active_phase: ExecutionPhase,
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
        active_phase,
        match active_phase {
            ExecutionPhase::Conception => {
                "The harness is deriving the app-specific Birth Plan and Genome."
            }
            ExecutionPhase::Repairing => {
                "The harness is repairing only the independently diagnosed acceptance gap."
            }
            _ => "The harness is materializing the preserved intention and references.",
        },
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
                        // A file change is useful activity, but it is not a
                        // factory lifecycle phase. Preserve the typed
                        // Conception/Materializing/Repairing state while the
                        // harness is still doing that exact work.
                        events.emit(Event::status(
                            "The Shot repository changed after the prepared boundary.",
                        ));
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
            let _ = remove_runner_pid(&execution);
            return present_completion(&completion, json, events);
        }
        let _ = remove_runner_pid(&execution);
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
            | ExecutionPhase::Conception
            | ExecutionPhase::Materializing
            | ExecutionPhase::HarnessRunning
            | ExecutionPhase::WorkspaceChanged
            | ExecutionPhase::Building
            | ExecutionPhase::Testing
            | ExecutionPhase::Verifying
            | ExecutionPhase::Repairing
            | ExecutionPhase::Installing
            | ExecutionPhase::Launching
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
    checked_runner_directory(execution)?;
    let path = runner_pid_path(execution);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 32 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the unattended runner PID record is unsafe",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() != metadata.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the unattended runner PID record changed while opening",
        ));
    }
    use std::io::Read as _;
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    file.take(33).read_to_end(&mut bytes)?;
    if bytes.len() > 32 || bytes.len() as u64 != opened.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the unattended runner PID record changed while reading",
        ));
    }
    let body = String::from_utf8(bytes).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the unattended runner PID record is not UTF-8",
        )
    })?;
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
    let ps = if Path::new("/bin/ps").is_file() {
        "/bin/ps"
    } else {
        "/usr/bin/ps"
    };
    let output = Command::new(ps)
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

    fn prepared_fixture(repository: PathBuf) -> PreparedExecution {
        PreparedExecution {
            schema: "fixture".into(),
            execution_id: "0123456789abcdef0123456789abcdef".into(),
            shot_id: tohseno_protocol::digest::ShotId::from_bytes([0x42; 32]),
            version_ordinal: 1,
            app_name: "field-notebook".into(),
            repository,
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
        }
    }

    #[test]
    fn background_runner_names_only_the_durable_execution() {
        let execution = prepared_fixture(PathBuf::from("/tmp/Shot Root"));
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

    #[cfg(unix)]
    #[test]
    fn process_tree_inherits_the_execution_claim_after_supervisor_exit() {
        let root = tempfile::tempdir().unwrap();
        let repository = root.path().join("shot");
        let directory = repository
            .join(".tohseno/executions")
            .join("0123456789abcdef0123456789abcdef");
        fs::create_dir_all(&directory).unwrap();
        let execution = prepared_fixture(repository);
        let claim = claim_execution_runner(&execution).unwrap();
        let mut child = Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        drop(claim);
        assert!(runner_claim_is_held(&execution).unwrap());
        drop(child.stdin.take());
        assert!(child.wait().unwrap().success());
        assert!(!runner_claim_is_held(&execution).unwrap());
    }

    #[test]
    fn recovery_restarts_only_safe_runner_boundaries() {
        assert_eq!(
            runner_recovery_disposition(ExecutionPhase::RunnerStarted, false),
            RunnerRecoveryDisposition::Start
        );
        assert_eq!(
            runner_recovery_disposition(ExecutionPhase::WaitingForDevice, false),
            RunnerRecoveryDisposition::ResumeDelivery
        );
        assert_eq!(
            runner_recovery_disposition(ExecutionPhase::HarnessRunning, false),
            RunnerRecoveryDisposition::FinalizeAbandoned
        );
        assert_eq!(
            runner_recovery_disposition(ExecutionPhase::ValidationStarted, false),
            RunnerRecoveryDisposition::FinalizeAbandoned
        );
        assert_eq!(
            runner_recovery_disposition(ExecutionPhase::HarnessRunning, true),
            RunnerRecoveryDisposition::Observe
        );
        assert_eq!(
            runner_recovery_disposition(ExecutionPhase::ExecutionCompleted, false),
            RunnerRecoveryDisposition::Observe
        );
    }

    #[test]
    fn structured_factory_stages_project_without_parsing_status_text() {
        for (stage, phase) in [
            (FactoryStage::Planning, ExecutionPhase::ContextLoaded),
            (FactoryStage::Conception, ExecutionPhase::Conception),
            (FactoryStage::Materializing, ExecutionPhase::Materializing),
            (FactoryStage::Building, ExecutionPhase::Building),
            (FactoryStage::Testing, ExecutionPhase::Testing),
            (FactoryStage::Verifying, ExecutionPhase::Verifying),
            (FactoryStage::Repairing, ExecutionPhase::Repairing),
            (FactoryStage::Installing, ExecutionPhase::Installing),
            (FactoryStage::Launching, ExecutionPhase::Launching),
        ] {
            assert_eq!(execution_phase_for_factory_stage(stage), phase);
        }
    }

    #[test]
    fn published_completion_recovers_a_missing_terminal_phase() {
        let root = tempfile::tempdir().unwrap();
        let repository = root.path().join("shot");
        let directory = repository
            .join(".tohseno/executions")
            .join("0123456789abcdef0123456789abcdef");
        fs::create_dir_all(&directory).unwrap();
        let mut execution = prepared_fixture(repository.clone());
        execution.schema = tohseno_engine::shot_execution::EXECUTION_RECORD_SCHEMA.into();
        execution.mode = ExecutionMode::BirthMaterialization;
        execution.phase = ExecutionPhase::ValidationCompleted;
        fs::write(
            directory.join("execution.json"),
            serde_json::to_vec(&execution).unwrap(),
        )
        .unwrap();
        let mut completion = completion_fixture(
            ExecutionMode::BirthMaterialization,
            ExecutionOutcome::Completed,
            true,
            "Experience the accepted app.",
        );
        completion.schema = tohseno_engine::shot_execution::COMPLETION_RECORD_SCHEMA.into();
        completion.harness = execution.harness.clone();
        completion.model = execution.model.clone();
        completion.route = execution.route.clone();
        reconcile_published_completion(&mut execution, &completion, true).unwrap();
        reconcile_published_completion(&mut execution, &completion, true).unwrap();
        assert_eq!(execution.phase, ExecutionPhase::ExecutionCompleted);
        assert_eq!(
            load_execution(&repository, &execution.execution_id)
                .unwrap()
                .phase,
            ExecutionPhase::ExecutionCompleted
        );
        assert_eq!(
            read_events(&repository, &execution.execution_id)
                .unwrap()
                .last()
                .unwrap()
                .phase,
            ExecutionPhase::ExecutionCompleted
        );
        assert_eq!(
            read_events(&repository, &execution.execution_id)
                .unwrap()
                .into_iter()
                .filter(|event| terminal_execution_phase(event.phase))
                .count(),
            1
        );
    }

    #[test]
    fn terminal_phase_without_its_event_is_repaired_exactly_once() {
        let root = tempfile::tempdir().unwrap();
        let repository = root.path().join("shot");
        let directory = repository
            .join(".tohseno/executions")
            .join("0123456789abcdef0123456789abcdef");
        fs::create_dir_all(&directory).unwrap();
        let mut execution = prepared_fixture(repository.clone());
        execution.schema = tohseno_engine::shot_execution::EXECUTION_RECORD_SCHEMA.into();
        execution.mode = ExecutionMode::BirthMaterialization;
        update_phase(
            &mut execution,
            ExecutionPhase::ValidationCompleted,
            "The accepted Version passed independent verification.",
        )
        .unwrap();

        // Simulate the precise `update_phase` crash boundary: execution.json
        // reached its final phase, but the corresponding events.jsonl append
        // did not. Recovery must append that terminal event once, and a
        // repeated recovery pass must remain byte-sequence idempotent.
        execution.phase = ExecutionPhase::ExecutionCompleted;
        fs::write(
            directory.join("execution.json"),
            serde_json::to_vec(&execution).unwrap(),
        )
        .unwrap();
        let mut completion = completion_fixture(
            ExecutionMode::BirthMaterialization,
            ExecutionOutcome::Completed,
            true,
            "Experience the accepted app.",
        );
        completion.schema = tohseno_engine::shot_execution::COMPLETION_RECORD_SCHEMA.into();
        completion.harness = execution.harness.clone();
        completion.model = execution.model.clone();
        completion.route = execution.route.clone();

        reconcile_published_completion(&mut execution, &completion, true).unwrap();
        reconcile_published_completion(&mut execution, &completion, true).unwrap();
        let events = read_events(&repository, &execution.execution_id).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| terminal_execution_phase(event.phase))
                .count(),
            1
        );
        assert_eq!(
            events.last().map(|event| event.phase),
            Some(ExecutionPhase::ExecutionCompleted)
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
