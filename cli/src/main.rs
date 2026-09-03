mod billing;
mod cable_genesis;
mod companion_service;
mod companion_simulator;
mod device_readiness;
mod identity_commands;
mod installation_commands;
mod intent_commands;
mod living_project;
mod local_openai_harness;
mod managed_compute;
mod native_client;
mod native_install;
mod native_session;
mod network_commands;
mod onboarding;
mod protocol_commands;
mod renderer;
mod service_client;
mod service_commands;
mod shot_commands;
mod simulator;
mod workspace_identity;
mod workspace_service;

use clap::{Parser, Subcommand, ValueEnum};
use renderer::Renderer;
use serde_json::{json, Value};
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use tohseno_application::ReferenceInput;
use tohseno_engine::{Config, Engine, Event, EventBus, Ledger};
use tohseno_protocol::digest::Bytes32;
use uuid::Uuid;
use workspace_identity::SecretStore;

const MAX_TEXT_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_FEEDBACK_FILE_BYTES: u64 = 100_000;

#[derive(Debug, Parser)]
#[command(
    name = "tohseno",
    version,
    about = "Keep iPhone apps connected to the Mac that builds and evolves them",
    disable_help_subcommand = true
)]
struct Cli {
    /// Emit structured JSON for supported commands.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ClaimEditionArgument {
    Open,
    Limited,
    Timed,
}

impl ClaimEditionArgument {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Limited => "limited",
            Self::Timed => "timed",
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Secondary generated-app path: describe an app for the local factory.
    Create {
        /// Optional technical name. When omitted, the implementation model names the app from its purpose.
        app_name: Option<String>,
        /// Supply the exact creation intention inline.
        #[arg(long, value_name = "TEXT")]
        prompt: Option<String>,
        /// Read the exact creation intention from a bounded UTF-8 file.
        #[arg(long, value_name = "PATH")]
        prompt_file: Option<PathBuf>,
        /// Attach one reference image. Repeat up to eight times.
        #[arg(long = "image", value_name = "PATH")]
        images: Vec<PathBuf>,
        /// Wait until deterministic acceptance or failure.
        #[arg(long)]
        wait: bool,
    },
    /// Describe what should change. TOHSENO evolves the app and installs it.
    Evolve {
        app_name: Option<String>,
        /// Supply the exact evolutionary intention inline.
        #[arg(long, value_name = "TEXT")]
        prompt: Option<String>,
        /// Read the exact evolutionary intention from a bounded UTF-8 file.
        #[arg(long, value_name = "PATH")]
        prompt_file: Option<PathBuf>,
        /// Select one exact signed Feedback action commitment.
        #[arg(long = "feedback-action", value_name = "COMMITMENT")]
        feedback_actions: Vec<String>,
        /// Attach one reference image. Repeat up to eight times.
        #[arg(long = "image", value_name = "PATH")]
        images: Vec<PathBuf>,
        /// Wait until deterministic acceptance or failure.
        #[arg(long)]
        wait: bool,
    },
    /// Connect an existing Xcode app to Tohseno without restructuring it.
    Init {
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,
        /// Choose one exact Xcode scheme when the project has several app targets.
        #[arg(long, value_name = "SCHEME")]
        scheme: Option<String>,
    },
    /// Prepare, approve on Companion, and publish the current app.
    Deploy {
        /// Inspect and package without requesting approval or publishing.
        #[arg(long)]
        dry_run: bool,
        #[arg(long, hide = true)]
        project_id: Option<String>,
        /// Pin the immutable first-Ship Claim Edition policy.
        #[arg(long, value_enum)]
        claim_edition: Option<ClaimEditionArgument>,
        /// Maximum identities for a limited first-Ship Claim Edition.
        #[arg(long, value_name = "COUNT")]
        max_claims: Option<u64>,
        /// RFC 3339 closing time for a timed first-Ship Claim Edition.
        #[arg(long, value_name = "TIMESTAMP")]
        closes_at: Option<String>,
        /// Human app slug signed into this release (for example, anky).
        #[arg(long, value_name = "SLUG")]
        app_slug: Option<String>,
    },
    /// Show this project's local, Companion, and public network readiness.
    Status,
    /// Verify, locally sign, and install one exact public Shot release.
    Install {
        /// Canonical Shot link, tohseno:// install link, or 32-byte ShotID.
        shot: String,
        /// Pin one immutable release instead of resolving the current release.
        #[arg(long, value_name = "DIGEST")]
        release: Option<String>,
        /// Materialize the verified owner-visible source in this new folder.
        #[arg(long, value_name = "DIRECTORY")]
        into: Option<PathBuf>,
        /// Confirm that the named non-Green build reasons were reviewed locally.
        #[arg(long)]
        approve_mac_review: bool,
    },
    /// Materialize one exact public release as a new local Shot.
    Fork {
        /// Canonical Shot link, tohseno:// fork link, or 32-byte ShotID.
        shot: String,
        /// Pin one immutable release instead of resolving the current release.
        #[arg(long, value_name = "DIGEST")]
        release: Option<String>,
        /// Materialize the mutable fork in this new folder.
        #[arg(long, value_name = "DIRECTORY")]
        into: Option<PathBuf>,
        /// Confirm that the named non-Green build reasons were reviewed locally.
        #[arg(long)]
        approve_mac_review: bool,
    },
    /// Explicit compatibility namespace for ADR 0014 recording tools.
    Recording {
        #[command(subcommand)]
        command: RecordingCommand,
    },
    /// Ensure the persistent Local Workspace Service is healthy, open Studio, and return.
    Studio {
        /// Development-only foreground port override.
        #[arg(long, hide = true)]
        foreground_port: Option<u16>,
    },
    /// Issue a bounded session to a verified native Tohseno.app parent.
    #[command(hide = true)]
    NativeSession,
    /// Inspect or enable Terminal integration for a verified native Tohseno.app parent.
    #[command(hide = true)]
    NativeCli {
        #[command(subcommand)]
        command: NativeCliCommand,
    },
    /// Run the bounded built-in adapter for one configured loopback model.
    #[command(hide = true)]
    LocalOpenAiHarness {
        #[arg(long)]
        base_url: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        privacy: String,
        #[arg(long)]
        credential_reference: Option<String>,
        instruction: String,
    },
    /// Run the bundled adapter through one admitted managed-compute route.
    #[command(hide = true)]
    ManagedOpenAiHarness {
        #[arg(long)]
        proxy_origin: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        privacy: String,
        #[arg(long)]
        command_id: String,
        #[arg(long)]
        execution_id: String,
        #[arg(long)]
        maximum_microusd: u64,
        #[arg(long)]
        pricing_snapshot_at: String,
        #[arg(long)]
        input_microusd_per_million: u64,
        #[arg(long)]
        output_microusd_per_million: u64,
        instruction: String,
    },
    /// Store one local-model bearer credential from stdin in macOS Keychain.
    #[command(hide = true)]
    LocalModelCredential {
        #[arg(long)]
        reference: String,
    },
    /// Import an encrypted browser handoff or a private intent package.
    #[command(hide = true)]
    Intent {
        #[command(subcommand)]
        command: IntentCommand,
    },
    /// Install, inspect, or control the persistent Local Workspace Service.
    Service {
        #[command(subcommand)]
        command: ServiceCommand,
    },
    /// Pair, inspect, revoke, simulate, or vendor the private Companion channel.
    Companion {
        #[command(subcommand)]
        command: CompanionAdminCommand,
    },
    /// List local apps and versions.
    List,
    /// Check local prerequisites.
    Doctor {
        #[arg(long, hide = true)]
        background: bool,
    },
    /// Install the latest stable TOHSENO release.
    #[command(alias = "upgrade")]
    Update,
    /// Remove TOHSENO program files while preserving every app and identity.
    Uninstall,
    /// Access recovery, inspection, sharing, and protocol tools.
    #[command(
        after_help = "Available commands: verify, inspect, feedback, share, try, export, import, migrate, migrate-legacy, genome, identity, protocol, page, network, registry, token\n\nRun `tohseno advanced <command> --help` for details."
    )]
    Advanced {
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "COMMAND"
        )]
        command: Vec<String>,
    },
    /// Verify the current local app, or an explicit local name/path, without an LLM.
    #[command(hide = true)]
    Verify {
        target: Option<String>,
        /// Require an activated public witness; fails closed before RPC while none exists.
        #[arg(long)]
        public: bool,
    },
    /// Show exact local protocol facts for one app or Shot path.
    #[command(hide = true)]
    Inspect { target: String },
    /// Record owned feedback or exchange exact-version workshop feedback.
    #[command(hide = true)]
    Feedback {
        app_name: Option<String>,
        /// Exact accepted version ordinal, such as 1 for version 0001.
        #[arg(long, value_name = "N")]
        version: Option<u64>,
        /// Feedback text. Use --file for longer material.
        #[arg(long, value_name = "TEXT", conflicts_with = "file")]
        text: Option<String>,
        /// Read feedback from a bounded UTF-8 regular file.
        #[arg(long, value_name = "PATH", conflicts_with = "text")]
        file: Option<PathBuf>,
        /// Copy one private attachment by digest. Repeat for multiple files.
        #[arg(long = "attachment", value_name = "PATH")]
        attachments: Vec<PathBuf>,
        /// Create a feedback packet for a materialized workshop instead of an owned Shot.
        #[arg(long, value_name = "DIRECTORY")]
        workshop: Option<PathBuf>,
        /// Admit one received workshop feedback packet to an owned Shot after review.
        #[arg(long, value_name = "FILE")]
        packet: Option<PathBuf>,
        /// Self-declared display name stored in a workshop feedback packet.
        #[arg(long, value_name = "NAME")]
        author: Option<String>,
        /// Destination for a newly created workshop feedback packet.
        #[arg(long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Package one accepted open-source Version for source-first community testing.
    #[command(hide = true)]
    Share {
        app_name: Option<String>,
        #[arg(long, value_name = "FILE")]
        output: PathBuf,
    },
    /// Verify workshop source, build it locally, and run it in Simulator.
    #[command(hide = true)]
    Try {
        capsule: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        output: PathBuf,
        /// Verify and materialize source without building or launching it.
        #[arg(long)]
        no_launch: bool,
    },
    /// Export verified Shot records as a portable bundle, not a source archive.
    #[command(hide = true)]
    Export {
        app_name: Option<String>,
        #[arg(long, value_name = "DIRECTORY")]
        output: PathBuf,
        /// Include exact private intention and feedback bytes.
        #[arg(long)]
        include_private: bool,
    },
    /// Verify and receive a portable Shot record bundle without taking ownership.
    #[command(hide = true)]
    Import {
        bundle: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        output: PathBuf,
    },
    /// Project frozen v1 Evolutions into the neutral model without rewriting them.
    #[command(hide = true)]
    Migrate { app_name: Option<String> },
    /// Copy preserved v0.6 apps into visible folders, then project their
    /// frozen signed history without changing the old ledger.
    #[command(hide = true)]
    MigrateLegacy { app_name: Option<String> },
    /// Inspect or explicitly accept a post-birth Shot Genome mutation.
    #[command(hide = true)]
    Genome {
        #[command(subcommand)]
        command: GenomeCommand,
    },
    /// Inspect a frozen v0.7 local identity and DeviceKey; never public authority.
    #[command(hide = true)]
    Identity {
        #[command(subcommand)]
        command: IdentityCommand,
    },
    /// Inspect protocol law and independently verify record files.
    #[command(hide = true)]
    Protocol {
        #[command(subcommand)]
        command: ProtocolCommand,
    },
    /// Build a deterministic static page for a local Shot.
    #[command(hide = true)]
    Page {
        #[command(subcommand)]
        command: PageCommand,
    },
    /// Inspect the committed contract definition and activation state offline.
    #[command(hide = true)]
    Network {
        #[command(subcommand)]
        command: NetworkCommand,
    },
    /// Inspect a verified local Shot head and public-witness availability.
    #[command(hide = true)]
    Registry {
        #[command(subcommand)]
        command: RegistryCommand,
    },
    /// Record optional chain-specific Token Associations in canonical Shot lineage.
    ///
    /// This neutral protocol action never changes Shot identity or ownership.
    /// The legacy `--public` flag fails closed until an ancestry-free public
    /// Token Association record is defined.
    #[command(hide = true)]
    Token {
        #[command(subcommand)]
        command: TokenCommand,
    },
    /// Internal durable execution administration.
    #[command(hide = true)]
    Shot {
        #[command(subcommand)]
        command: ShotExecutionCommand,
    },
}

#[derive(Debug, Subcommand)]
enum RecordingCommand {
    /// Initialize the historical app-local recording layer.
    Init { app_name: String },
    /// Record the current app tree through the historical recording layer.
    Record {
        app_name: Option<String>,
        #[arg(long, value_name = "TEXT")]
        note: Option<String>,
        #[arg(long, value_name = "PATH", conflicts_with = "note")]
        note_file: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum NativeCliCommand {
    Status,
    Enable,
}

#[derive(Debug, Subcommand)]
enum ServiceCommand {
    /// Install the user LaunchAgent and start the service.
    Install,
    /// Start the installed service.
    Start,
    /// Stop the installed service cleanly.
    Stop,
    /// Restart the installed service.
    Restart,
    /// Show installed, launchd, and verified health state.
    Status,
    /// Show the bounded operational log tail.
    Logs,
    /// Internal foreground process used by launchd.
    Run {
        #[arg(long, hide = true)]
        port: Option<u16>,
    },
    /// Remove only the installer-owned LaunchAgent; private state is preserved.
    Uninstall,
}

#[derive(Debug, Subcommand)]
enum IntentCommand {
    /// Claim one encrypted pending relay intention and open it in Studio.
    Claim {
        #[arg(
            value_name = "TOKEN",
            required_unless_present = "stdin",
            conflicts_with = "stdin"
        )]
        token: Option<String>,
        /// Read the one-use claim token from standard input.
        #[arg(long)]
        stdin: bool,
        /// Import durably without opening Studio.
        #[arg(long)]
        no_open: bool,
    },
    /// Import a private .tohseno-intent file and open it in Studio.
    Open {
        path: PathBuf,
        /// Import durably without opening Studio.
        #[arg(long)]
        no_open: bool,
    },
}

#[derive(Debug, Subcommand)]
enum CompanionAdminCommand {
    /// Show pairing, device, and relay state.
    Status,
    /// Open Studio directly into the pairing surface.
    Pair,
    /// Build, install, and launch the current Companion on the connected iPhone.
    Install,
    /// List paired and revoked companion devices.
    Devices,
    /// Immediately revoke one paired device.
    Revoke { device_id: String },
    /// Show the content-blind relay configuration state.
    RelayStatus,
    /// Run the deterministic local companion simulator.
    Simulate {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        arguments: Vec<String>,
    },
    /// Manage the immutable Apple CompanionKit source.
    Sdk {
        #[command(subcommand)]
        command: CompanionSdkCommand,
    },
}

#[derive(Debug, Subcommand)]
enum CompanionSdkCommand {
    /// Vendor the exact released CompanionKit and license into a Shot repository.
    Vendor {
        #[arg(long, value_name = "SHOT_PATH")]
        into: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum ShotExecutionCommand {
    Harnesses,
    Run {
        #[arg(long)]
        app: String,
        #[arg(long)]
        execution: String,
    },
    Follow {
        #[arg(long)]
        app: String,
        #[arg(long)]
        execution: String,
    },
    Result {
        #[arg(long)]
        app: String,
        #[arg(long)]
        execution: String,
    },
    Cancel {
        #[arg(long)]
        app: String,
        #[arg(long)]
        execution: String,
    },
}

#[derive(Debug, Subcommand)]
enum IdentityCommand {
    /// Show the local legacy BuilderID prediction and local recovery-backup status.
    Show,
    /// Reveal or create a local backup for the frozen v0.7 identity.
    ///
    /// This does not activate account recovery or create public authority.
    Backup {
        #[arg(long)]
        confirm: bool,
        #[arg(long, value_name = "ABSOLUTE_PATH")]
        passphrase_file: Option<PathBuf>,
    },
    /// Import recovery words as a local backup for the stored legacy BuilderID.
    ///
    /// This does not recover or rotate an account or create public authority.
    ImportBackup {
        /// Confirm this local-only backup import for the stored legacy BuilderID.
        #[arg(long)]
        confirm: bool,
        /// Read the 24 secret backup words from a private local file.
        #[arg(long, value_name = "ABSOLUTE_PATH")]
        mnemonic_file: Option<PathBuf>,
        /// Read the new vault-encryption passphrase from a private local file.
        #[arg(long, value_name = "ABSOLUTE_PATH")]
        passphrase_file: Option<PathBuf>,
    },
    /// Show the local-only DeviceKey used for frozen v0.7 offline verification.
    Devices,
}

#[derive(Debug, Subcommand)]
enum ProtocolCommand {
    /// Show protocol identifiers, the stable product, and conditional contract coordinates.
    Info,
    /// Print the frozen cross-language protocol vectors.
    Vectors,
    /// Verify a shot.json and its sibling signature.json.
    VerifyRecord { path: PathBuf },
}

#[derive(Debug, Subcommand)]
enum GenomeCommand {
    /// Show the current accepted machine Genome after drift verification.
    Show { app_name: Option<String> },
    /// Sign a reviewed mutation of an already accepted app-specific Genome.
    Accept {
        app_name: Option<String>,
        #[arg(long, value_name = "PATH")]
        file: PathBuf,
        #[arg(
            long,
            default_value = "Owner reviewed and explicitly accepted this Genome revision."
        )]
        rationale: String,
        /// Explain one intentional mutation. Required for revisions after 1.
        #[arg(long = "mutation", value_name = "DESCRIPTION")]
        mutations: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum PageCommand {
    /// Build a self-contained static directory for an app.
    Build { app_name: String },
}

#[derive(Debug, Subcommand)]
enum NetworkCommand {
    /// Show the contract definition and activation state without contacting an RPC.
    Status,
}

#[derive(Debug, Subcommand)]
enum RegistryCommand {
    /// Show the local head and why no public witness is checked while inactive.
    Show { app_name: String },
}

#[derive(Debug, Subcommand)]
enum TokenCommand {
    /// Associate a token contract without making it the Shot or its owner.
    Associate {
        app_name: String,
        chain_id: u64,
        token_address: String,
        /// Optional human-facing symbol; it is descriptive, not authoritative.
        #[arg(long, value_name = "SYMBOL")]
        symbol: Option<String>,
        /// Retired compatibility flag. Public export currently fails closed.
        ///
        /// This does not contact a node, submit a transaction, or prove that
        /// the token contract exists.
        #[arg(long)]
        public: bool,
    },
    /// Remove the current exact token relation while retaining its history.
    Remove {
        app_name: String,
        chain_id: u64,
        token_address: String,
        /// Retired compatibility flag. Public export currently fails closed.
        #[arg(long)]
        public: bool,
    },
}

#[tokio::main]
async fn main() {
    let arguments = product_arguments(std::env::args_os().collect());
    let cli = Cli::parse_from(arguments);
    run_main(cli).await;
}

fn product_arguments(mut arguments: Vec<std::ffi::OsString>) -> Vec<std::ffi::OsString> {
    if arguments.len() == 1 {
        arguments.push("studio".into());
    }
    arguments
}

async fn run_main(cli: Cli) {
    let redact_service_error = matches!(
        &cli.command,
        Command::Service {
            command: ServiceCommand::Run { .. }
        }
    );
    if let Err(error) = run(cli).await {
        if error
            .downcast_ref::<tohseno_engine::EngineError>()
            .is_some_and(|error| matches!(error, tohseno_engine::EngineError::SlotLimit))
        {
            std::process::exit(1);
        }
        if redact_service_error {
            eprintln!("tohseno: Local Workspace Service exited with an operational error.");
        } else {
            eprintln!("tohseno: {error}");
        }
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    if !cli.json
        && matches!(&cli.command, Command::Init { .. })
        && io::stdin().is_terminal()
        && io::stdout().is_terminal()
    {
        onboarding::run_init(&mut io::stdin().lock(), &mut io::stdout().lock())?;
    }
    let bus = EventBus::default();
    // The LaunchAgent runs `--json service run`, and that long-lived process
    // never prints a command result for `--json` to keep clean. Suppressing
    // the renderer there leaves the installed service unable to write a single
    // line to the operational log it is configured to own, so the run command
    // always renders its bounded event stream.
    let service_run = matches!(
        &cli.command,
        Command::Service {
            command: ServiceCommand::Run { .. }
        }
    );
    let render_task = if cli.json && !service_run {
        None
    } else {
        let renderer = Renderer::new(io::stdout(), io::stdout().is_terminal());
        Some(tokio::spawn(renderer.follow(bus.subscribe())))
    };
    if !cli.json
        && io::stdout().is_terminal()
        && !matches!(&cli.command, Command::Update | Command::Uninstall)
    {
        installation_commands::maybe_emit_update_notice(&bus).await;
    }
    let outcome = dispatch(cli.command, &bus, cli.json).await;
    drop(bus);
    if let Some(render_task) = render_task {
        render_task.await??;
    }
    outcome
}

async fn dispatch(
    command: Command,
    bus: &EventBus,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Command::Update => installation_commands::update(bus).await?,
        Command::Uninstall => installation_commands::uninstall(bus)?,
        Command::List => list(bus)?,
        Command::Create {
            app_name,
            prompt,
            prompt_file,
            images,
            wait,
        } => factory_create(app_name, prompt, prompt_file, images, wait, json, bus).await?,
        Command::Evolve {
            app_name,
            prompt,
            prompt_file,
            feedback_actions,
            images,
            wait,
        } => {
            factory_evolve(
                app_name,
                prompt,
                prompt_file,
                feedback_actions,
                images,
                wait,
                json,
                bus,
            )
            .await?;
        }
        Command::Init { path, scheme } => network_commands::init(path, scheme, json, bus).await?,
        Command::Deploy {
            dry_run,
            project_id,
            claim_edition,
            max_claims,
            closes_at,
            app_slug,
        } => {
            network_commands::deploy(
                network_commands::DeployOptions {
                    dry_run,
                    project_id: project_id.as_deref(),
                    claim_edition: claim_edition.map(ClaimEditionArgument::as_str),
                    max_claims,
                    closes_at: closes_at.as_deref(),
                    app_slug: app_slug.as_deref(),
                },
                json,
                bus,
            )
            .await?
        }
        Command::Status => network_commands::status(json, bus).await?,
        Command::Install {
            shot,
            release,
            into,
            approve_mac_review,
        } => {
            network_commands::receive(
                &shot,
                release.as_deref(),
                into,
                network_commands::ReceiveKind::Install,
                approve_mac_review,
                json,
                bus,
            )
            .await?
        }
        Command::Fork {
            shot,
            release,
            into,
            approve_mac_review,
        } => {
            network_commands::receive(
                &shot,
                release.as_deref(),
                into,
                network_commands::ReceiveKind::Fork,
                approve_mac_review,
                json,
                bus,
            )
            .await?
        }
        Command::Recording { command } => match command {
            RecordingCommand::Init { app_name } => {
                let app_name = normalize_cli_app_name(&app_name)?;
                let engine = Engine::discover(bus.clone())?;
                engine.initialize_app(&app_name)?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string(&json!({
                            "schema": "tohseno.recording-initialization-receipt/1",
                            "name": app_name,
                            "kind": "recording_only",
                        }))?
                    );
                }
            }
            RecordingCommand::Record {
                app_name,
                note,
                note_file,
            } => {
                recording_record(app_name, note, note_file, json, bus)?;
            }
        },
        Command::Advanced { command } => {
            if command.is_empty() {
                return Err(
                    "Choose an advanced command, for example `tohseno advanced verify`.".into(),
                );
            }
            let mut arguments = vec!["tohseno".to_owned()];
            arguments.extend(command);
            let parsed = match Cli::try_parse_from(arguments) {
                Ok(parsed) => parsed,
                Err(error)
                    if matches!(
                        error.kind(),
                        clap::error::ErrorKind::DisplayHelp
                            | clap::error::ErrorKind::DisplayVersion
                    ) =>
                {
                    error.print()?;
                    return Ok(());
                }
                Err(error) => return Err(error.into()),
            };
            if matches!(parsed.command, Command::Advanced { .. }) {
                return Err("nested `advanced` commands are not supported".into());
            }
            Box::pin(dispatch(parsed.command, bus, json)).await?;
        }
        Command::Studio { foreground_port } => {
            if let Some(port) = foreground_port {
                workspace_service::run(Some(port), bus.clone())
                    .await
                    .map_err(|error| error.to_string())?;
            } else {
                let service = service_client::ServiceClient::ensure_running()
                    .await
                    .map_err(|error| error.to_string())?;
                let opened = std::process::Command::new("/usr/bin/open")
                    .args(["-a", "Tohseno"])
                    .status()?;
                if !opened.success() {
                    return Err("macOS could not open Tohseno.app".into());
                }
                if json {
                    println!(
                        "{}",
                        serde_json::to_string(&json!({
                            "schema": "tohseno.native-app-opened/1",
                            "application": "Tohseno.app",
                            "workspace_id": service.runtime().workspace_id,
                            "service_version": service.runtime().service_version,
                        }))?
                    );
                } else {
                    bus.emit(Event::result("Tohseno is open. The Local Workspace Service remains available after this Terminal closes."));
                }
            }
        }
        Command::NativeSession => {
            let credential = native_client::issue_session()
                .await
                .map_err(|error| error.to_string())?;
            println!("{}", serde_json::to_string(&credential)?);
        }
        Command::NativeCli { command } => {
            native_client::verify_native_parent().map_err(|error| error.to_string())?;
            let status = match command {
                NativeCliCommand::Status => installation_commands::cli_integration_status(),
                NativeCliCommand::Enable => installation_commands::enable_cli_integration(),
            }?;
            println!("{}", serde_json::to_string(&status)?);
        }
        Command::LocalOpenAiHarness {
            base_url,
            model,
            privacy,
            credential_reference,
            instruction,
        } => {
            local_openai_harness::run(
                &base_url,
                &model,
                &privacy,
                credential_reference.as_deref(),
                &instruction,
            )
            .await?;
        }
        Command::ManagedOpenAiHarness {
            proxy_origin,
            model,
            privacy,
            command_id,
            execution_id,
            maximum_microusd,
            pricing_snapshot_at,
            input_microusd_per_million,
            output_microusd_per_million,
            instruction,
        } => {
            local_openai_harness::run_managed(local_openai_harness::ManagedRunRequest {
                proxy_origin: &proxy_origin,
                reservation: managed_compute::ManagedReservationRequest {
                    command_id: &command_id,
                    execution_id: &execution_id,
                    model: &model,
                    privacy: &privacy,
                    maximum_microusd,
                    pricing_snapshot_at: &pricing_snapshot_at,
                    input_microusd_per_million,
                    output_microusd_per_million,
                },
                instruction: &instruction,
            })
            .await?;
        }
        Command::LocalModelCredential { reference } => {
            if reference.is_empty()
                || reference.len() > 128
                || reference
                    .bytes()
                    .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
            {
                return Err("local model credential reference is invalid".into());
            }
            let mut secret = zeroize::Zeroizing::new(Vec::new());
            std::io::Read::by_ref(&mut std::io::stdin())
                .take(16 * 1024 + 1)
                .read_to_end(&mut secret)?;
            while secret
                .last()
                .is_some_and(|byte| matches!(byte, b'\n' | b'\r'))
            {
                secret.pop();
            }
            if secret.is_empty() || secret.len() > 16 * 1024 {
                return Err("local model credential is empty or oversized".into());
            }
            workspace_identity::KeychainSecretStore
                .put(&reference, &secret)
                .map_err(|error| error.to_string())?;
        }
        Command::Service { command } => service_admin(command, json, bus).await?,
        Command::Companion { command } => companion_admin(command, json, bus).await?,
        Command::Intent { command } => match command {
            IntentCommand::Claim {
                token,
                stdin,
                no_open,
            } => {
                let token = if stdin {
                    intent_commands::read_claim_token_from_stdin()?
                } else {
                    token.expect("clap requires a token unless --stdin is used")
                };
                intent_commands::claim(token, no_open, bus).await?;
            }
            IntentCommand::Open { path, no_open } => {
                intent_commands::open_package(&path, no_open, bus).await?;
            }
        },
        Command::Shot { command } => match command {
            ShotExecutionCommand::Harnesses => {
                let engine = Engine::discover(bus.clone())?;
                if json {
                    println!("{}", serde_json::to_string(&engine.harnesses())?);
                } else {
                    for harness in engine.harnesses() {
                        bus.emit(Event::status(format!(
                            "{} · {}",
                            harness.label,
                            if harness.installed {
                                "available"
                            } else {
                                "unavailable"
                            }
                        )));
                    }
                }
            }
            ShotExecutionCommand::Run { app, execution } => {
                tohseno_application::execution_manager::run(&app, &execution, json, bus).await?;
            }
            ShotExecutionCommand::Follow { app, execution } => {
                tohseno_application::execution_manager::follow(&app, &execution, json, bus).await?;
            }
            ShotExecutionCommand::Result { app, execution } => {
                tohseno_application::execution_manager::result(&app, &execution, json, bus)?;
            }
            ShotExecutionCommand::Cancel { app, execution } => {
                tohseno_application::execution_manager::cancel(&app, &execution, json, bus)?;
            }
        },
        Command::Doctor { background } => {
            if !background {
                bus.emit(Event::status("checking this Mac…"));
            }
            product_doctor(json, bus).await?;
        }
        Command::Identity { command } => match command {
            IdentityCommand::Show => identity_commands::show(bus, json)?,
            IdentityCommand::Devices => identity_commands::devices(bus, json)?,
            IdentityCommand::Backup {
                confirm,
                passphrase_file,
            } => identity_commands::backup(bus, json, confirm, passphrase_file.as_deref())?,
            IdentityCommand::ImportBackup {
                confirm,
                mnemonic_file,
                passphrase_file,
            } => identity_commands::import_backup(
                bus,
                json,
                confirm,
                mnemonic_file.as_deref(),
                passphrase_file.as_deref(),
            )?,
        },
        Command::Verify { target, public } => {
            let target = match target {
                Some(target) => target,
                None => std::env::current_dir()?.display().to_string(),
            };
            protocol_commands::verify_target(&target, public, json, bus)?;
        }
        Command::Inspect { target } => protocol_commands::inspect_target(&target, json, bus)?,
        Command::Feedback {
            app_name,
            version,
            text,
            file,
            attachments,
            workshop,
            packet,
            author,
            output,
        } => {
            if let Some(packet) = packet {
                if version.is_some()
                    || text.is_some()
                    || file.is_some()
                    || !attachments.is_empty()
                    || workshop.is_some()
                    || author.is_some()
                    || output.is_some()
                {
                    return Err("--packet accepts only an owned app name".into());
                }
                let (engine, name) = engine_for(app_name, bus)?;
                shot_commands::import_workshop_feedback(&engine, &name, &packet, json, bus)?;
            } else {
                let feedback = match (text, file) {
                    (Some(text), None) => text,
                    (None, Some(path)) => strip_one_terminal_line_ending(read_bounded_utf8(
                        &path,
                        MAX_FEEDBACK_FILE_BYTES,
                        "feedback file",
                    )?),
                    _ => return Err("provide exactly one of --text or --file".into()),
                };
                if let Some(workshop) = workshop {
                    if app_name.is_some() || version.is_some() || !attachments.is_empty() {
                        return Err(
                            "--workshop feedback does not take an app name, --version, or attachments"
                                .into(),
                        );
                    }
                    let output = output.ok_or("--workshop feedback requires --output")?;
                    shot_commands::write_workshop_feedback(
                        &workshop,
                        &feedback,
                        author.as_deref(),
                        &output,
                        json,
                        bus,
                    )?;
                } else {
                    if author.is_some() || output.is_some() {
                        return Err("--author and --output are only for --workshop feedback".into());
                    }
                    let version = version.ok_or("owned feedback requires --version")?;
                    let (engine, name) = engine_for(app_name, bus)?;
                    shot_commands::record_feedback(
                        &engine,
                        &name,
                        version,
                        &feedback,
                        &attachments,
                        json,
                        bus,
                    )?;
                }
            }
        }
        Command::Share { app_name, output } => {
            let (engine, name) = engine_for(app_name, bus)?;
            shot_commands::share_for_workshop(&engine, &name, &output, json, bus)?;
        }
        Command::Try {
            capsule,
            output,
            no_launch,
        } => {
            shot_commands::try_workshop(&capsule, &output, no_launch, json, bus).await?;
        }
        Command::Export {
            app_name,
            output,
            include_private,
        } => {
            let (engine, name) = engine_for(app_name, bus)?;
            shot_commands::export_shot(&engine, &name, &output, include_private, json, bus)?;
        }
        Command::Import { bundle, output } => {
            shot_commands::import_shot(&bundle, &output, json, bus)?;
        }
        Command::Migrate { app_name } => {
            let (engine, name) = engine_for(app_name, bus)?;
            shot_commands::migrate_v1(&engine, &name, json, bus)?;
        }
        Command::MigrateLegacy { app_name } => {
            let engine = Engine::discover(bus.clone())?;
            shot_commands::migrate_legacy_v0_6(&engine, app_name.as_deref(), json, bus)?;
        }
        Command::Genome { command } => match command {
            GenomeCommand::Show { app_name } => {
                let (engine, name) = engine_for(app_name, bus)?;
                shot_commands::show_genome(&engine, &name, json, bus)?;
            }
            GenomeCommand::Accept {
                app_name,
                file,
                rationale,
                mutations,
            } => {
                let (engine, name) = engine_for(app_name, bus)?;
                let genome = read_genome_file(&file)?;
                shot_commands::accept_genome(
                    &engine, &name, &genome, &rationale, &mutations, json, bus,
                )?;
            }
        },
        Command::Protocol { command } => protocol_commands::protocol_command(command, json, bus)?,
        Command::Page { command } => match command {
            PageCommand::Build { app_name } => protocol_commands::build_page(&app_name, json, bus)?,
        },
        Command::Network { command } => match command {
            NetworkCommand::Status => protocol_commands::network_status(json, bus)?,
        },
        Command::Registry { command } => match command {
            RegistryCommand::Show { app_name } => {
                protocol_commands::registry_show(&app_name, json, bus)?;
            }
        },
        Command::Token { command } => match command {
            TokenCommand::Associate {
                app_name,
                chain_id,
                token_address,
                symbol,
                public,
            } => {
                let engine = Engine::discover(bus.clone())?;
                shot_commands::associate_token(
                    &engine,
                    &app_name,
                    chain_id,
                    &token_address,
                    symbol.as_deref(),
                    public,
                    json,
                    bus,
                )?;
            }
            TokenCommand::Remove {
                app_name,
                chain_id,
                token_address,
                public,
            } => {
                let engine = Engine::discover(bus.clone())?;
                shot_commands::remove_token(
                    &engine,
                    &app_name,
                    chain_id,
                    &token_address,
                    public,
                    json,
                    bus,
                )?;
            }
        },
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn factory_create(
    app_name: Option<String>,
    prompt: Option<String>,
    prompt_file: Option<PathBuf>,
    images: Vec<PathBuf>,
    wait: bool,
    json_output: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = app_name
        .as_deref()
        .map(normalize_cli_app_name)
        .transpose()?;
    if images.len() > 8 {
        return Err("at most eight reference images are accepted".into());
    }
    let intention = resolve_create_intention(prompt, prompt_file, json_output)?;
    let service = service_client::ServiceClient::ensure_running()
        .await
        .map_err(|error| error.to_string())?;
    let intention = match intention {
        CreationIntention::Exact(intention) => intention,
        CreationIntention::Composer { prefill } => {
            let pending = prefill.as_deref().map(stage_local_intention).transpose()?;
            let route = match (name.as_deref(), pending.flatten()) {
                (Some(name), Some(pending_id)) => {
                    format!("/create?name={name}&pending={pending_id}")
                }
                (None, Some(pending_id)) => format!("/create?pending={pending_id}"),
                (Some(name), None) => format!("/create?name={name}"),
                (None, None) => "/create".into(),
            };
            service
                .open_studio(&route)
                .map_err(|error| error.to_string())?;
            bus.emit(Event::result(match name.as_deref() {
                Some(name) => format!("Describe {name}, then press Create App."),
                None => "Describe your app, then press Create App. TOHSENO will name it.".into(),
            }));
            return Ok(());
        }
    };
    if intention.trim().is_empty() {
        return Err(
            "creation intention must contain non-whitespace UTF-8 text; supply --prompt, --prompt-file, or bounded stdin"
                .into(),
        );
    }
    let references = read_reference_inputs(&images)?;
    let payload_commitment = json!({
        "name": name,
        "intention": intention,
        "references": references.iter().map(|reference| json!({
            "filename": reference.display_filename,
            "media_type": reference.media_type,
            "origin": reference.origin,
            "sha256": tohseno_protocol::digest::sha256(&reference.bytes),
        })).collect::<Vec<_>>(),
    });
    let command_id = stable_command_id("shot.create", &payload_commitment)?;
    let body = json!({
        "command_id": command_id,
        "origin": "cli",
        "name": name,
        "intention": intention,
        "references": api_references(&references),
    });
    let receipt: Value = service
        .post_durable("/api/v1/shots", &body)
        .await
        .map_err(|error| error.to_string())?;
    let execution_id = receipt
        .get("execution_id")
        .and_then(Value::as_str)
        .ok_or("Local Workspace Service returned no execution ID")?;
    let shot_id = receipt
        .get("shot_id")
        .and_then(Value::as_str)
        .ok_or("Local Workspace Service returned no Shot ID")?;
    let completion = if wait {
        if !json_output {
            bus.emit(Event::result(admission_message(name.as_deref(), false)));
        }
        let mut previous_state = None;
        Some(
            service
                .wait_for_execution_with_updates(execution_id, |status| {
                    if json_output {
                        return;
                    }
                    let state = status.get("state").and_then(Value::as_str);
                    if state != previous_state.as_deref() {
                        if let Some(message) = progress_message(status) {
                            bus.emit(Event::status(message));
                        }
                        previous_state = state.map(str::to_owned);
                    }
                })
                .await
                .map_err(|error| error.to_string())?,
        )
    } else {
        if !json_output {
            let queued = service
                .execution_status(execution_id)
                .await
                .ok()
                .and_then(|status| {
                    status
                        .get("state")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .as_deref()
                == Some("queued");
            bus.emit(Event::result(admission_message(name.as_deref(), queued)));
        }
        None
    };
    if json_output {
        let output = if let Some(completion) = completion {
            json!({
                "schema": "tohseno.create-command-result/1",
                "receipt": receipt,
                "execution": completion,
            })
        } else {
            receipt
        };
        println!("{}", serde_json::to_string(&output)?);
    } else {
        if completion.is_some() {
            bus.emit(Event::result(match name.as_deref() {
                Some(name) => format!("{name} is on your iPhone."),
                None => "Your app is on your iPhone.".into(),
            }));
        }
        if io::stdout().is_terminal() {
            let _ = service.open_studio(&format!("/shots/{shot_id}"));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn factory_evolve(
    app_name: Option<String>,
    prompt: Option<String>,
    prompt_file: Option<PathBuf>,
    feedback_actions: Vec<String>,
    images: Vec<PathBuf>,
    wait: bool,
    json_output: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    if images.len() > 8 {
        return Err("at most eight reference images are accepted".into());
    }
    let name = match app_name {
        Some(name) => normalize_cli_app_name(&name)?,
        None => engine_for(None, bus)?.1,
    };
    let explicit = resolve_intention(prompt, prompt_file)?;
    let piped = if explicit.is_none() && !io::stdin().is_terminal() {
        Some(read_stdin_bounded(MAX_TEXT_FILE_BYTES as usize)?)
    } else {
        None
    };
    let intention = explicit.or(piped).unwrap_or_default();
    let compose = intention.is_empty() && feedback_actions.is_empty();
    if compose && !interactive(json_output) {
        return Err(
            "evolve requires --prompt, --prompt-file, piped UTF-8 intention, or --feedback-action"
                .into(),
        );
    }
    let service = service_client::ServiceClient::ensure_running()
        .await
        .map_err(|error| error.to_string())?;
    let workspace: Value = service
        .get("/api/v1/workspace")
        .await
        .map_err(|error| error.to_string())?;
    let shot = workspace
        .get("shots")
        .and_then(Value::as_array)
        .and_then(|shots| {
            shots.iter().find(|shot| {
                shot.get("display_name").and_then(Value::as_str) == Some(name.as_str())
                    && shot.get("kind").and_then(Value::as_str) == Some("factory_shot")
            })
        })
        .ok_or("evolution target is not a factory Shot")?;
    let shot_id = required_string(shot, "shot_id")?;
    if compose {
        // No intention was written, so open the one place where it is written.
        // Studio binds the exact accepted base when Evolve App is pressed.
        service
            .open_studio(&format!("/shots/{shot_id}"))
            .map_err(|error| error.to_string())?;
        bus.emit(Event::result(format!(
            "Describe what should change about {name}, then press Evolve App."
        )));
        return Ok(());
    }
    let expression_id = required_string(shot, "expression_id")?;
    let version_id = required_string(shot, "latest_version_id")?;
    let version_ordinal = shot
        .get("latest_version_ordinal")
        .and_then(Value::as_u64)
        .ok_or("Shot has no accepted base Version")?;
    for action in &feedback_actions {
        Bytes32::from_hex("Feedback action commitment", action)?;
    }
    let references = read_reference_inputs(&images)?;
    let payload_commitment = json!({
        "shot_id": shot_id,
        "base_expression_id": expression_id,
        "base_version_id": version_id,
        "base_version_ordinal": version_ordinal,
        "intention": intention,
        "selected_feedback_actions": feedback_actions,
        "references": references.iter().map(|reference| json!({
            "filename": reference.display_filename,
            "media_type": reference.media_type,
            "origin": reference.origin,
            "sha256": tohseno_protocol::digest::sha256(&reference.bytes),
        })).collect::<Vec<_>>(),
    });
    let command_id = stable_command_id("shot.evolve", &payload_commitment)?;
    let body = json!({
        "command_id": command_id,
        "origin": "cli",
        "base_expression_id": expression_id,
        "base_version_id": version_id,
        "base_version_ordinal": version_ordinal,
        "intention": intention,
        "selected_feedback_actions": feedback_actions,
        "references": api_references(&references),
    });
    let receipt: Value = service
        .post_durable(&format!("/api/v1/shots/{shot_id}/evolutions"), &body)
        .await
        .map_err(|error| error.to_string())?;
    let execution_id = receipt
        .get("execution_id")
        .and_then(Value::as_str)
        .ok_or("Local Workspace Service returned no execution ID")?;
    let completion = if wait {
        if !json_output {
            bus.emit(Event::result(admission_message(Some(&name), false)));
        }
        let mut previous_state = None;
        Some(
            service
                .wait_for_execution_with_updates(execution_id, |status| {
                    if json_output {
                        return;
                    }
                    let state = status.get("state").and_then(Value::as_str);
                    if state != previous_state.as_deref() {
                        if let Some(message) = progress_message(status) {
                            bus.emit(Event::status(message));
                        }
                        previous_state = state.map(str::to_owned);
                    }
                })
                .await
                .map_err(|error| error.to_string())?,
        )
    } else {
        if !json_output {
            let queued = service
                .execution_status(execution_id)
                .await
                .ok()
                .and_then(|status| {
                    status
                        .get("state")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .as_deref()
                == Some("queued");
            bus.emit(Event::result(admission_message(Some(&name), queued)));
        }
        None
    };
    if json_output {
        let output = if let Some(completion) = completion {
            json!({
                "schema": "tohseno.evolve-command-result/1",
                "receipt": receipt,
                "execution": completion,
            })
        } else {
            receipt
        };
        println!("{}", serde_json::to_string(&output)?);
    } else {
        if completion.is_some() {
            bus.emit(Event::result(format!("{name} is on your iPhone.")));
        }
        if io::stdout().is_terminal() {
            let _ = service.open_studio(&format!("/shots/{shot_id}"));
        }
    }
    Ok(())
}

fn admission_message(name: Option<&str>, queued: bool) -> String {
    let app = name.unwrap_or("your app");
    let state = if queued {
        "is waiting for this Mac to finish another app"
    } else {
        "was received and will start automatically"
    };
    format!("Got it — {app} {state}. You can close this Terminal; TOHSENO keeps working.")
}

fn progress_message(status: &Value) -> Option<String> {
    let state = status.get("state")?.as_str()?;
    let label = match state {
        "queued" => "Waiting to start",
        "planning" | "materializing" => "Making the app",
        "building" | "testing" | "verifying" | "repairing" => "Checking the app",
        "installing" | "launching" => "Putting it on your iPhone",
        "waiting_for_device" => "Ready — connect and unlock your iPhone",
        "accepted" => "Installed",
        "failed" => "Stopped before installation",
        "cancelled" => "Cancelled",
        _ => return None,
    };
    let elapsed = status
        .get("elapsed_seconds")
        .and_then(Value::as_u64)
        .map(format_elapsed)
        .unwrap_or_else(|| "just now".into());
    Some(format!("{label} · {elapsed} elapsed"))
}

fn format_elapsed(seconds: u64) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    if minutes == 0 {
        format!("{seconds}s")
    } else {
        format!("{minutes}m {seconds:02}s")
    }
}

fn recording_record(
    app_name: Option<String>,
    note: Option<String>,
    note_file: Option<PathBuf>,
    json_output: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let (engine, name) = engine_for(app_name, bus)?;
    let file_note = note_file.as_deref().map(read_note_file).transpose()?;
    let piped_note = if note.is_none() && file_note.is_none() && !io::stdin().is_terminal() {
        Some(read_stdin_bounded(MAX_TEXT_FILE_BYTES as usize)?)
    } else {
        None
    };
    let note = note
        .as_deref()
        .or(file_note.as_deref())
        .or(piped_note.as_deref());
    let version = engine.record_version(&name, note)?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "schema": "tohseno.recording-version-receipt/1",
                "name": name,
                "kind": "recording_only",
                "version_ordinal": version.number,
            }))?
        );
    }
    Ok(())
}

async fn service_admin(
    command: ServiceCommand,
    json_output: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    use service_commands::{ServicePaths, SystemLaunchctl};
    let paths = ServicePaths::discover()?;
    match command {
        ServiceCommand::Run { port } => {
            workspace_service::run(port, bus.clone())
                .await
                .map_err(|error| error.to_string())?;
        }
        ServiceCommand::Install => {
            let receipt = service_commands::install(&paths, &SystemLaunchctl)?;
            let client = service_client::ServiceClient::ensure_running()
                .await
                .map_err(|error| error.to_string())?;
            present_json_or_status(
                json_output,
                json!({
                    "schema": "tohseno.service-status/1",
                    "operation": receipt.operation,
                    "installed": true,
                    "healthy": true,
                    "origin": client.runtime().origin,
                    "workspace_id": client.runtime().workspace_id,
                    "service_version": client.runtime().service_version,
                    "state_preserved": true,
                }),
                "Local Workspace Service is installed and healthy.",
                bus,
            )?;
        }
        ServiceCommand::Start => {
            let receipt = service_commands::start(&paths, &SystemLaunchctl)?;
            let client = service_client::ServiceClient::ensure_running()
                .await
                .map_err(|error| error.to_string())?;
            present_json_or_status(
                json_output,
                json!({
                    "schema": "tohseno.service-status/1",
                    "operation": receipt.operation,
                    "installed": true,
                    "healthy": true,
                    "origin": client.runtime().origin,
                    "workspace_id": client.runtime().workspace_id,
                    "service_version": client.runtime().service_version,
                }),
                "Local Workspace Service is healthy.",
                bus,
            )?;
        }
        ServiceCommand::Stop => {
            let receipt = service_commands::stop(&paths, &SystemLaunchctl)?;
            present_json_or_status(
                json_output,
                json!(receipt),
                "Local Workspace Service stopped cleanly.",
                bus,
            )?;
        }
        ServiceCommand::Restart => {
            let previous_instance = service_client::ServiceClient::connect()
                .await
                .ok()
                .map(|client| client.runtime().instance_id.clone());
            let receipt = service_commands::restart(&paths, &SystemLaunchctl)?;
            let client = service_client::ServiceClient::ensure_running()
                .await
                .map_err(|error| error.to_string())?;
            if previous_instance.as_deref() == Some(client.runtime().instance_id.as_str()) {
                return Err(
                    "launchd did not replace the previous Local Workspace Service process".into(),
                );
            }
            present_json_or_status(
                json_output,
                json!({
                    "schema": "tohseno.service-status/1",
                    "operation": receipt.operation,
                    "installed": true,
                    "healthy": true,
                    "origin": client.runtime().origin,
                    "service_version": client.runtime().service_version,
                }),
                "Local Workspace Service restarted and is healthy.",
                bus,
            )?;
        }
        ServiceCommand::Status => {
            let installed = paths.launch_agent.is_file();
            let loaded =
                service_commands::launchd_loaded(&paths, &SystemLaunchctl).unwrap_or(false);
            let healthy = service_client::ServiceClient::connect().await.ok();
            let value = json!({
                "schema": "tohseno.service-status/1",
                "installed": installed,
                "launchd_loaded": loaded,
                "healthy": healthy.is_some(),
                "origin": healthy.as_ref().map(|client| client.runtime().origin.as_str()),
                "workspace_id": healthy.as_ref().map(|client| client.runtime().workspace_id.as_str()),
                "service_version": healthy.as_ref().map(|client| client.runtime().service_version.as_str()),
            });
            present_json_or_status(
                json_output,
                value,
                if healthy.is_some() {
                    "Local Workspace Service is healthy."
                } else {
                    "Local Workspace Service is not healthy."
                },
                bus,
            )?;
            if healthy.is_none() {
                return Err(
                    "Local Workspace Service is not healthy; run `tohseno service start`".into(),
                );
            }
        }
        ServiceCommand::Logs => {
            let lines = service_commands::bounded_logs(&paths)?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "schema": "tohseno.service-log-tail/1",
                        "lines": lines,
                    }))?
                );
            } else {
                for line in lines {
                    bus.emit(Event::status(line));
                }
            }
        }
        ServiceCommand::Uninstall => {
            let receipt = service_commands::uninstall(&paths, &SystemLaunchctl)?;
            present_json_or_status(json_output, json!(receipt), "Installer-owned LaunchAgent removed; private workspace and pairing state were preserved.", bus)?;
        }
    }
    Ok(())
}

async fn product_doctor(
    json_output: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    use tohseno_engine::gates::{apple_signing, device, toolchain};

    let command_text = |program: &str, arguments: &[&str]| {
        std::process::Command::new(program)
            .args(arguments)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    let macos =
        command_text("/usr/bin/sw_vers", &["-productVersion"]).unwrap_or_else(|| "unknown".into());
    let node = command_text("node", &["--version"]);
    let xcode_tools = command_text("xcode-select", &["-p"]).is_some();
    let xcode = toolchain::check() == toolchain::ToolchainState::Ready;
    let (apple_signing_ready, provisioning) = match apple_signing::check() {
        apple_signing::AppleSigningState::Ready { provisioning, .. } => {
            (true, provisioning.as_str())
        }
        apple_signing::AppleSigningState::Missing => (false, "unknown"),
    };
    let device_state = if xcode {
        match device::check() {
            Ok(device::DeviceState::Ready(_)) => "ready",
            Ok(device::DeviceState::DeviceUnreachable) => "device_unreachable",
            Ok(device::DeviceState::TrustRequired) => "trust_required",
            Ok(device::DeviceState::DeveloperModeRequired) => "developer_mode_required",
            Err(_) => "unknown",
        }
    } else if device::cable_visible() {
        "xcode_required"
    } else {
        "device_unreachable"
    };
    let paths = service_commands::ServicePaths::discover()?;
    let service_installed = paths.launch_agent.is_file();
    let client = service_client::ServiceClient::connect().await.ok();
    let service_healthy = client.is_some();
    let entitlement = match &client {
        Some(client) => client.get::<Value>("/api/v1/entitlement").await.ok(),
        None => None,
    };
    let companion = match &client {
        Some(client) => client.get::<Value>("/api/v1/companion/status").await.ok(),
        None => None,
    };
    let release_manifest_compatible = paths
        .install_root
        .join("current/RELEASE.json")
        .is_file()
        .then(|| {
            fs::read(paths.install_root.join("current/RELEASE.json"))
                .ok()
                .filter(|bytes| bytes.len() <= 64 * 1024)
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .is_some_and(|value| {
                    value["schema"] == "tohseno.release/1"
                        && value["version"] == env!("CARGO_PKG_VERSION")
                        && value["channel"] == "stable"
                        && value["prerelease"] == false
                })
        });
    let report = json!({
        "schema": "tohseno.doctor/1",
        "macos_version": macos,
        "architecture": std::env::consts::ARCH,
        "node_version": node,
        "native_version": env!("CARGO_PKG_VERSION"),
        "release_manifest_compatible": release_manifest_compatible,
        "service_installed": service_installed,
        "service_healthy": service_healthy,
        "xcode_installed": xcode,
        "xcode_command_line_tools": xcode_tools,
        "apple_signing_ready": apple_signing_ready,
        "provisioning_category": provisioning,
        "iphone_state": device_state,
        "companion_paired": companion.as_ref().and_then(|value| value["paired_devices"].as_u64()).unwrap_or(0) > 0,
        "companion_relay": companion.as_ref().and_then(|value| value["relay_connection"].as_str()),
        "entitlement_phase": entitlement.as_ref().and_then(|value| value["phase"].as_str()),
        "successful_days": entitlement.as_ref().and_then(|value| value["successful_days"].as_u64()),
    });
    if json_output {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        for (label, value) in [
            (
                "macOS",
                report["macos_version"].as_str().unwrap_or("unknown"),
            ),
            (
                "architecture",
                report["architecture"].as_str().unwrap_or("unknown"),
            ),
            (
                "Node",
                report["node_version"].as_str().unwrap_or("not installed"),
            ),
            (
                "native TOHSENO",
                report["native_version"].as_str().unwrap_or("unknown"),
            ),
            ("iPhone", device_state),
            (
                "Apple signing",
                if apple_signing_ready {
                    provisioning
                } else {
                    "not ready"
                },
            ),
            (
                "entitlement",
                report["entitlement_phase"]
                    .as_str()
                    .unwrap_or("service unavailable"),
            ),
        ] {
            bus.emit(Event::status(format!("{label}: {value}")));
        }
        bus.emit(Event::result(if service_healthy {
            "Local Workspace Service is healthy."
        } else if service_installed {
            "Local Workspace Service is installed but not healthy."
        } else {
            "Local Workspace Service is not installed."
        }));
    }
    Ok(())
}

async fn companion_admin(
    command: CompanionAdminCommand,
    json_output: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        CompanionAdminCommand::Status | CompanionAdminCommand::RelayStatus => {
            let service = service_client::ServiceClient::ensure_running()
                .await
                .map_err(|error| error.to_string())?;
            let status: Value = service
                .get("/api/v1/companion/status")
                .await
                .map_err(|error| error.to_string())?;
            present_json_or_status(
                json_output,
                status,
                "Companion status loaded from your Local Workspace Service.",
                bus,
            )?;
        }
        CompanionAdminCommand::Pair => {
            let service = service_client::ServiceClient::ensure_running()
                .await
                .map_err(|error| error.to_string())?;
            let session: Value = service
                .post("/api/v1/companion/pairing-sessions", &json!({}))
                .await
                .map_err(|error| error.to_string())?;
            let session_id = required_string(&session, "session_id")?;
            if !json_output {
                service
                    .open_studio(&format!("/?pair={session_id}"))
                    .map_err(|error| error.to_string())?;
            }
            present_json_or_status(
                json_output,
                session,
                "Studio is waiting for the iPhone.",
                bus,
            )?;
        }
        CompanionAdminCommand::Install => {
            use tohseno_engine::gates::{apple_signing, device};
            let device = match device::check()? {
                device::DeviceState::Ready(device) => device,
                _ => return Err("the connected iPhone is not ready for installation".into()),
            };
            let team_id = match apple_signing::check() {
                apple_signing::AppleSigningState::Ready { team_id, .. } => team_id,
                apple_signing::AppleSigningState::Missing => {
                    return Err(
                        "add your Apple Account in Xcode before installing the Companion".into(),
                    )
                }
            };
            let paths = service_commands::ServicePaths::discover()?;
            let installed_project = paths.install_root.join(
                "current/share/companion/apple/TohsenoCompanion/App/TohsenoCompanion.xcodeproj",
            );
            #[cfg(debug_assertions)]
            let project = {
                let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .ok_or("CLI source path has no repository root")?
                    .join("companion/apple/TohsenoCompanion/App/TohsenoCompanion.xcodeproj");
                if installed_project.is_file() {
                    installed_project
                } else {
                    source
                }
            };
            #[cfg(not(debug_assertions))]
            let project = installed_project;
            cable_genesis::build_and_install_companion(
                &project,
                &paths.service_state,
                &device,
                &team_id,
            )
            .map_err(|error| error.to_string())?;
            cable_genesis::launch_companion(&paths.service_state, &device)
                .map_err(|error| error.to_string())?;
            present_json_or_status(
                json_output,
                json!({
                    "schema": "tohseno.companion-install-receipt/1",
                    "bundle_identifier": "com.tohseno.companion",
                    "version": env!("CARGO_PKG_VERSION"),
                    "launched": true,
                }),
                "The current TOHSENO Companion was installed and launched.",
                bus,
            )?;
        }
        CompanionAdminCommand::Devices => {
            let service = service_client::ServiceClient::ensure_running()
                .await
                .map_err(|error| error.to_string())?;
            let devices: Value = service
                .get("/api/v1/companion/devices")
                .await
                .map_err(|error| error.to_string())?;
            present_json_or_status(json_output, devices, "Paired-device state loaded.", bus)?;
        }
        CompanionAdminCommand::Revoke { device_id } => {
            let service = service_client::ServiceClient::ensure_running()
                .await
                .map_err(|error| error.to_string())?;
            let receipt: Value = service
                .delete(&format!("/api/v1/companion/devices/{device_id}"))
                .await
                .map_err(|error| error.to_string())?;
            present_json_or_status(
                json_output,
                receipt,
                "Companion device revoked immediately.",
                bus,
            )?;
        }
        CompanionAdminCommand::Sdk {
            command: CompanionSdkCommand::Vendor { into },
        } => {
            let receipt = vendor_companion_sdk(&into)?;
            present_json_or_status(
                json_output,
                receipt,
                "The immutable CompanionKit source and license were vendored into the Shot.",
                bus,
            )?;
        }
        CompanionAdminCommand::Simulate { arguments } => {
            return companion_simulate(arguments, json_output, bus).await;
        }
    }
    Ok(())
}

fn present_json_or_status(
    json_output: bool,
    value: Value,
    status: &str,
    bus: &EventBus,
) -> Result<(), serde_json::Error> {
    if json_output {
        println!("{}", serde_json::to_string(&value)?);
    } else {
        bus.emit(Event::result(status));
    }
    Ok(())
}

fn required_string<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Local Workspace Service response is missing {field}").into())
}

/// How `tohseno create` obtained its intention, or that it must be written.
enum CreationIntention {
    /// Exact bytes from `--prompt`, `--prompt-file`, or bounded piped stdin.
    Exact(String),
    /// Nothing was supplied interactively: open the simple composer instead.
    /// `prefill` is an exact regular `./MASTER_PROMPT.md`, when one is present.
    Composer { prefill: Option<PathBuf> },
}

fn resolve_create_intention(
    prompt: Option<String>,
    prompt_file: Option<PathBuf>,
    json_output: bool,
) -> Result<CreationIntention, Box<dyn std::error::Error>> {
    if let Some(intention) = resolve_intention(prompt, prompt_file)? {
        return Ok(CreationIntention::Exact(intention));
    }
    if !io::stdin().is_terminal() {
        return Ok(CreationIntention::Exact(read_stdin_bounded(
            MAX_TEXT_FILE_BYTES as usize,
        )?));
    }
    if !interactive(json_output) {
        return Err(
            "no creation intention was supplied; use --prompt, --prompt-file, or bounded UTF-8 stdin"
                .into(),
        );
    }
    Ok(CreationIntention::Composer {
        prefill: master_prompt_prefill(Path::new("."))?,
    })
}

/// An exact regular `MASTER_PROMPT.md` beside the caller, if there is one.
///
/// Its presence may prefill the composer, but it never starts a build on its
/// own: one explicit Create App is always required.
fn master_prompt_prefill(directory: &Path) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    let path = directory.join("MASTER_PROMPT.md");
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("./MASTER_PROMPT.md exists but is not a regular file".into())
        }
        Ok(_) => Ok(Some(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn interactive(json_output: bool) -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal() && !json_output
}

/// Import an exact local intention file into the durable pending-intention
/// store so the composer can show it and submit it unchanged.
fn stage_local_intention(path: &Path) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let ledger = Ledger::discover()?;
    ledger.initialize()?;
    stage_intention_in(&ledger, path)
}

/// Returns `None` when those exact bytes were already imported and consumed by
/// an earlier creation; the composer then opens empty rather than replaying a
/// spent record.
fn stage_intention_in(
    ledger: &Ledger,
    path: &Path,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let intention = read_bounded_utf8(path, MAX_TEXT_FILE_BYTES, "MASTER_PROMPT.md")?;
    // The intention was written when the file was written. Using its own
    // timestamp — not the clock — keeps repeated `tohseno create` invocations
    // byte-identical, so the store recognizes them as the same record instead
    // of accumulating a new one on every open.
    let package =
        tohseno_engine::build_intent_package(&intention_timestamp(path)?, &intention, &[])?;
    let pending = tohseno_engine::PendingIntentionStore::for_ledger(ledger).import_bytes(
        &package,
        tohseno_engine::PendingIntentionSource::PortableFile,
    )?;
    Ok((pending.state == tohseno_engine::PendingIntentionState::Ready).then_some(pending.id))
}

fn intention_timestamp(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let modified = fs::symlink_metadata(path)
        .and_then(|metadata| metadata.modified())
        .map(time::OffsetDateTime::from)
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    Ok(modified
        .to_offset(time::UtcOffset::UTC)
        .replace_nanosecond(0)?
        .format(&time::format_description::well_known::Rfc3339)?)
}

fn resolve_intention(
    prompt: Option<String>,
    prompt_file: Option<PathBuf>,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if let Some(prompt) = prompt {
        return Ok(Some(prompt));
    }
    prompt_file
        .as_deref()
        .map(|path| read_bounded_utf8(path, MAX_TEXT_FILE_BYTES, "intention file"))
        .transpose()
}

fn read_reference_inputs(
    paths: &[PathBuf],
) -> Result<Vec<ReferenceInput>, Box<dyn std::error::Error>> {
    paths
        .iter()
        .map(|path| ReferenceInput::from_path(path).map_err(|error| error.into()))
        .collect()
}

fn api_references(references: &[ReferenceInput]) -> Vec<Value> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    references
        .iter()
        .map(|reference| {
            json!({
                "filename": reference.display_filename,
                "media_type": reference.media_type,
                "origin": reference.origin,
                "bytes_base64url": URL_SAFE_NO_PAD.encode(&reference.bytes),
            })
        })
        .collect()
}

fn stable_command_id(kind: &str, payload: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let mut material = Vec::new();
    material.extend_from_slice(b"TOHSENO-LOCAL-COMMAND-ID-V1\0");
    material.extend_from_slice(kind.as_bytes());
    material.push(0);
    material.extend_from_slice(&tohseno_protocol::canonical::to_vec(payload)?);
    let digest = tohseno_protocol::digest::sha256(&material).to_string();
    Ok(format!(
        "command_{}",
        digest
            .trim_start_matches("0x")
            .chars()
            .take(40)
            .collect::<String>()
    ))
}

fn read_stdin_bounded(maximum: usize) -> io::Result<String> {
    let mut bytes = Vec::new();
    io::stdin()
        .take(u64::try_from(maximum).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "piped intention exceeds the UTF-8 byte limit",
        ));
    }
    String::from_utf8(bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "piped intention must be valid UTF-8",
        )
    })
}

fn vendor_companion_sdk(into: &std::path::Path) -> Result<Value, Box<dyn std::error::Error>> {
    use std::collections::BTreeMap;
    use std::fs::{File, FileTimes, OpenOptions};
    use std::io::Read as _;
    use std::time::{Duration as StdDuration, UNIX_EPOCH};

    let metadata = fs::symlink_metadata(into)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("--into must name an existing non-symlink Shot directory".into());
    }
    let absolute = into.canonicalize()?;
    if absolute != absolute.canonicalize()? {
        return Err("Shot directory resolution is unsafe".into());
    }
    let (source, vector) = companion_sdk_sources()?;
    let vendor_root = absolute.join("Vendor");
    match fs::symlink_metadata(&vendor_root) {
        Ok(value) if value.file_type().is_symlink() || !value.is_dir() => {
            return Err("Shot Vendor path is unsafe".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(&vendor_root)?,
        Err(error) => return Err(error.into()),
    }
    let destination = vendor_root.join("TohsenoCompanionKit");
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(format!(
            "{} already exists; refusing to overwrite vendored source",
            destination.display()
        )
        .into());
    }
    let staging = vendor_root.join(format!(
        ".TohsenoCompanionKit.{}.staging",
        Uuid::new_v4().simple()
    ));
    fs::create_dir(&staging)?;
    let result = (|| -> Result<Value, Box<dyn std::error::Error>> {
        let files = collect_sdk_files(&source)?;
        let mut total = 0_u64;
        for relative in files {
            let from = source.join(&relative);
            let to = staging.join(&relative);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            let source_metadata = fs::symlink_metadata(&from)?;
            total = total.saturating_add(source_metadata.len());
            if total > 100 * 1024 * 1024 {
                return Err("CompanionKit source exceeds the release byte bound".into());
            }
            let mut read_options = OpenOptions::new();
            read_options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                read_options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
            }
            let input = read_options.open(&from)?;
            let opened = input.metadata()?;
            if !opened.is_file() || opened.len() != source_metadata.len() {
                return Err("CompanionKit source changed while vendoring".into());
            }
            let mut write_options = OpenOptions::new();
            write_options.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                write_options
                    .mode(0o644)
                    .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
            }
            let mut output = write_options.open(&to)?;
            io::copy(&mut input.take(opened.len() + 1), &mut output)?;
            output.sync_all()?;
        }
        let vector_target =
            staging.join("Tests/TohsenoCompanionKitTests/TestVectors/companion-v1.json");
        fs::create_dir_all(vector_target.parent().ok_or("vector has no parent")?)?;
        let vector_bytes = read_bounded_bytes(&vector, 16 * 1024 * 1024)?;
        let vector_value: Value = serde_json::from_slice(&vector_bytes)?;
        if vector_value.get("schema").and_then(Value::as_str)
            != Some("tohseno.companion-test-vectors/1")
            || vector_value.get("test_only").and_then(Value::as_bool) != Some(true)
        {
            return Err("shared Companion vectors do not match the released schema".into());
        }
        fs::write(&vector_target, vector_bytes)?;

        let mut entries = BTreeMap::new();
        for relative in collect_sdk_files(&staging)? {
            if relative == PathBuf::from("VENDORED-MANIFEST.sha256") {
                continue;
            }
            let bytes = read_bounded_bytes(&staging.join(&relative), 32 * 1024 * 1024)?;
            let digest = tohseno_protocol::digest::sha256(&bytes)
                .to_string()
                .trim_start_matches("0x")
                .to_owned();
            entries.insert(relative, digest);
        }
        let mut manifest = String::new();
        for (relative, digest) in entries {
            manifest.push_str(&format!("{digest}  {}\n", relative.to_string_lossy()));
        }
        let manifest_path = staging.join("VENDORED-MANIFEST.sha256");
        fs::write(&manifest_path, manifest.as_bytes())?;
        let fixed = UNIX_EPOCH + StdDuration::from_secs(946_684_800);
        let times = FileTimes::new().set_accessed(fixed).set_modified(fixed);
        for path in collect_sdk_paths(&staging)? {
            File::open(&path)?.set_times(times)?;
        }
        File::open(&staging)?.sync_all()?;
        fs::rename(&staging, &destination)?;
        File::open(&vendor_root)?.sync_all()?;
        let version = read_bounded_utf8(&destination.join("VERSION"), 64, "CompanionKit VERSION")?
            .trim()
            .to_owned();
        let manifest_bytes = read_bounded_bytes(
            &destination.join("VENDORED-MANIFEST.sha256"),
            16 * 1024 * 1024,
        )?;
        Ok(json!({
            "schema": "tohseno.companion-sdk-vendored/1",
            "version": version,
            "destination": destination,
            "manifest_sha256": tohseno_protocol::digest::sha256(&manifest_bytes)
                .to_string()
                .trim_start_matches("0x"),
        }))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn companion_sdk_sources() -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    let executable = std::env::current_exe()?;
    let executable_root = executable.parent().and_then(|value| value.parent());
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("CLI source directory has no repository parent")?
        .to_path_buf();
    let source_candidates = [
        std::env::var_os("TOHSENO_COMPANION_SDK_SOURCE").map(PathBuf::from),
        executable_root.map(|root| root.join("share/sdk/apple/TohsenoCompanionKit")),
        Some(repository.join("sdk/apple/TohsenoCompanionKit")),
    ];
    let vector_candidates = [
        executable_root.map(|root| root.join("share/companion/test-vectors/companion-v1.json")),
        Some(repository.join("companion/test-vectors/companion-v1.json")),
    ];
    let source = source_candidates
        .into_iter()
        .flatten()
        .find(|path| path.join("Package.swift").is_file())
        .ok_or("released CompanionKit source is unavailable")?;
    let vector = vector_candidates
        .into_iter()
        .flatten()
        .find(|path| path.is_file())
        .ok_or("released shared companion vectors are unavailable")?;
    Ok((source, vector))
}

fn collect_sdk_files(root: &std::path::Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    fn visit(
        root: &std::path::Path,
        directory: &std::path::Path,
        output: &mut Vec<PathBuf>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if output.len() > 20_000 {
            return Err("CompanionKit contains too many files".into());
        }
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(root)?.to_path_buf();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "CompanionKit source contains a symbolic link: {}",
                    relative.display()
                )
                .into());
            }
            if relative.components().any(|part| {
                matches!(
                    part.as_os_str().to_str(),
                    Some(".build" | ".swiftpm" | "VENDORED-MANIFEST.sha256")
                )
            }) {
                continue;
            }
            if metadata.is_dir() {
                visit(root, &path, output)?;
            } else if metadata.is_file() {
                output.push(relative);
            } else {
                return Err("CompanionKit source contains a special file".into());
            }
        }
        Ok(())
    }
    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("CompanionKit source root is unsafe".into());
    }
    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    files.sort_by(|left, right| {
        left.as_os_str()
            .as_encoded_bytes()
            .cmp(right.as_os_str().as_encoded_bytes())
    });
    Ok(files)
}

fn collect_sdk_paths(root: &std::path::Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut values = collect_sdk_files(root)?
        .into_iter()
        .map(|relative| root.join(relative))
        .collect::<Vec<_>>();
    values.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    Ok(values)
}

fn read_bounded_bytes(
    path: &std::path::Path,
    maximum: u64,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err("file must be a bounded regular file".into());
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() != metadata.len() {
        return Err("file changed while opening".into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err("file exceeds its bound".into());
    }
    Ok(bytes)
}

async fn companion_simulate(
    arguments: Vec<String>,
    json_output: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let result = companion_simulator::run(arguments).await?;
    present_json_or_status(
        json_output,
        result,
        "The companion simulator completed the real encrypted relay flow.",
        bus,
    )?;
    Ok(())
}

fn read_note_file(path: &std::path::Path) -> Result<String, Box<dyn std::error::Error>> {
    read_bounded_utf8(path, MAX_TEXT_FILE_BYTES, "note file")
}

fn strip_one_terminal_line_ending(mut text: String) -> String {
    if text.ends_with("\r\n") {
        text.truncate(text.len() - 2);
    } else if text.ends_with('\n') {
        text.pop();
    }
    text
}

fn read_bounded_utf8(
    path: &std::path::Path,
    maximum_bytes: u64,
    label: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum_bytes {
        return Err(format!(
            "{label} must be a regular UTF-8 file no larger than {maximum_bytes} bytes"
        )
        .into());
    }
    let bytes = read_bounded_bytes(path, maximum_bytes)?;
    Ok(String::from_utf8(bytes).map_err(|_| format!("{label} must contain valid UTF-8 text"))?)
}

fn read_genome_file(
    path: &std::path::Path,
) -> Result<tohseno_protocol::Genome, Box<dyn std::error::Error>> {
    let text = read_bounded_utf8(path, MAX_TEXT_FILE_BYTES, "genome file")?;
    let genome = serde_json::from_str::<tohseno_protocol::Genome>(&text)?;
    genome.validate()?;
    Ok(genome)
}

/// Resolves the engine and app name, git-style: an explicit name uses the
/// configured homes; no name walks up from the current directory to the
/// nearest folder carrying a `.tohseno` ledger.
fn engine_for(
    app_name: Option<String>,
    bus: &EventBus,
) -> Result<(Engine, String), Box<dyn std::error::Error>> {
    if let Some(name) = app_name {
        return Ok((
            Engine::discover(bus.clone())?,
            normalize_cli_app_name(&name)?,
        ));
    }
    let mut directory = std::env::current_dir()?;
    loop {
        if directory.join(".tohseno").join("app.toml").is_file() {
            let (ledger, name) = Ledger::for_app_folder(&directory)?;
            ledger.initialize()?;
            let config = Config::load_or_default(ledger.machine_root())?;
            return Ok((Engine::at(ledger, bus.clone(), config), name));
        }
        if !directory.pop() {
            break;
        }
    }
    Err("run this inside an app folder, or pass the app name".into())
}

fn normalize_cli_app_name(name: &str) -> Result<String, tohseno_engine::ledger::LedgerError> {
    let normalized = name.to_ascii_lowercase();
    tohseno_engine::ledger::validate_app_name(&normalized)?;
    Ok(normalized)
}

fn list(bus: &EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = Ledger::discover()?;
    let apps = ledger.list_apps()?;
    if apps.is_empty() {
        bus.emit(Event::status("no apps yet."));
    } else {
        for app in apps {
            let detail = if let Some(number) = app.latest_evolution {
                format!("Versions 1–{number}")
            } else {
                "no Versions recorded".into()
            };
            bus.emit(Event::status(format!("{} · {detail}", app.name)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_cli_admission_and_progress_are_truthful_and_plain() {
        assert!(admission_message(Some("pocket-sip"), true)
            .contains("waiting for this Mac to finish another app"));
        assert!(admission_message(None, false).contains("You can close this Terminal"));
        assert_eq!(
            progress_message(&json!({"state":"queued","elapsed_seconds":4})).unwrap(),
            "Waiting to start · 4s elapsed"
        );
        assert_eq!(
            progress_message(&json!({"state":"materializing","elapsed_seconds":65})).unwrap(),
            "Making the app · 1m 05s elapsed"
        );
        assert_eq!(
            progress_message(&json!({"state":"waiting_for_device","elapsed_seconds":125})).unwrap(),
            "Ready — connect and unlock your iPhone · 2m 05s elapsed"
        );
        for message in [
            admission_message(None, false),
            progress_message(&json!({"state":"building","elapsed_seconds":65})).unwrap(),
        ] {
            for internal in ["Shot", "Expression", "Version", "execution", "harness"] {
                assert!(!message.contains(internal), "{message}");
            }
        }
    }

    #[test]
    fn cli_app_names_are_case_insensitive_without_sanitizing_unsafe_names() {
        assert_eq!(normalize_cli_app_name("THYSY").unwrap(), "thysy");
        assert!(normalize_cli_app_name("../THYSY").is_err());
        assert!(normalize_cli_app_name("My App").is_err());
    }

    #[test]
    fn update_is_canonical_upgrade_is_an_alias_and_uninstall_is_explicit() {
        assert!(matches!(
            Cli::try_parse_from(["tohseno", "update"]).unwrap().command,
            Command::Update
        ));
        assert!(matches!(
            Cli::try_parse_from(["tohseno", "upgrade"]).unwrap().command,
            Command::Update
        ));
        assert!(matches!(
            Cli::try_parse_from(["tohseno", "uninstall"])
                .unwrap()
                .command,
            Command::Uninstall
        ));
        let help = Cli::try_parse_from(["tohseno", "--help"])
            .unwrap_err()
            .to_string();
        assert!(help.contains("update"));
        assert!(help.contains("uninstall"));
        assert!(!help.contains("upgrade"));
    }

    #[test]
    fn studio_uses_the_persistent_service_by_default() {
        let parsed = Cli::try_parse_from(["tohseno", "studio"]).unwrap();
        assert!(matches!(
            parsed.command,
            Command::Studio {
                foreground_port: None,
            }
        ));

        let foreground =
            Cli::try_parse_from(["tohseno", "studio", "--foreground-port", "0"]).unwrap();
        assert!(matches!(
            foreground.command,
            Command::Studio {
                foreground_port: Some(0)
            }
        ));
    }

    #[test]
    fn no_arguments_enter_the_same_studio_product_door() {
        let arguments = product_arguments(vec!["tohseno".into()]);
        let parsed = Cli::try_parse_from(arguments).unwrap();
        assert!(matches!(
            parsed.command,
            Command::Studio {
                foreground_port: None,
            }
        ));
    }

    #[test]
    fn verify_defaults_to_the_current_local_app_without_repeating_its_name() {
        let local = Cli::try_parse_from(["tohseno", "verify"]).unwrap();
        assert!(matches!(
            local.command,
            Command::Verify {
                target: None,
                public: false,
            }
        ));

        let explicit = Cli::try_parse_from(["tohseno", "verify", "field-notebook"]).unwrap();
        assert!(matches!(
            explicit.command,
            Command::Verify {
                target: Some(target),
                public: false,
            } if target == "field-notebook"
        ));
    }

    #[test]
    fn backup_import_has_an_explicitly_local_command_name() {
        let parsed =
            Cli::try_parse_from(["tohseno", "identity", "import-backup", "--confirm"]).unwrap();
        assert!(matches!(
            parsed.command,
            Command::Identity {
                command: IdentityCommand::ImportBackup {
                    confirm: true,
                    mnemonic_file: None,
                    passphrase_file: None,
                },
            }
        ));
        assert!(Cli::try_parse_from(["tohseno", "identity", "recover", "--confirm"]).is_err());
    }

    #[test]
    fn backup_import_help_disclaims_account_recovery() {
        let help = Cli::try_parse_from(["tohseno", "identity", "import-backup", "--help"])
            .unwrap_err()
            .to_string();
        assert!(help.contains("local backup for the stored legacy BuilderID"));
        assert!(help.contains("does not recover or rotate an account or create public authority"));
        assert!(!help.contains("current BuilderID"));
        assert!(!help.contains("public recovery status"));
    }

    #[test]
    fn identity_help_marks_the_frozen_generation_local_and_non_authoritative() {
        let root_help = Cli::try_parse_from(["tohseno", "--help"])
            .unwrap_err()
            .to_string();
        assert!(!root_help.contains("frozen v0.7 local identity"));
        assert!(!root_help.contains("never public authority"));
        assert!(root_help.contains("advanced"));
        assert!(!root_help.contains("durable BuilderID"));

        let identity_help = Cli::try_parse_from(["tohseno", "identity", "--help"])
            .unwrap_err()
            .to_string();
        assert!(identity_help.contains("local legacy BuilderID prediction"));
        assert!(identity_help.contains("local-only DeviceKey"));
        assert!(!identity_help.contains("current BuilderID"));
        assert!(!identity_help.contains("public recovery status"));
    }

    #[test]
    fn incomplete_device_replacement_commands_are_not_exposed() {
        assert!(Cli::try_parse_from([
            "tohseno",
            "identity",
            "authorize",
            "pairing-request.json",
            "--device-nonce",
            "0",
        ])
        .is_err());
        assert!(Cli::try_parse_from([
            "tohseno",
            "identity",
            "revoke",
            "0x1111111111111111111111111111111111111111111111111111111111111111",
            "--device-nonce",
            "1",
            "--deadline",
            "2000000000",
        ])
        .is_err());
    }

    #[test]
    fn retired_v07_public_mutations_are_not_exposed() {
        for retired in [
            vec![
                "tohseno",
                "publish",
                "field-notebook",
                "--rpc-url",
                "https://rpc.mainnet.chain.robinhood.com",
                "--deadline",
                "2000000000",
            ],
            vec![
                "tohseno",
                "handle",
                "claim",
                "field-notebook",
                "my-shot",
                "--rpc-url",
                "https://rpc.mainnet.chain.robinhood.com",
                "--deadline",
                "2000000000",
            ],
            vec![
                "tohseno",
                "appcoin",
                "associate",
                "field-notebook",
                "8453",
                "0x1111111111111111111111111111111111111111",
                "--rpc-url",
                "https://rpc.mainnet.chain.robinhood.com",
                "--deadline",
                "2000000000",
            ],
        ] {
            assert!(Cli::try_parse_from(retired).is_err());
        }

        let help = Cli::try_parse_from(["tohseno", "--help"])
            .unwrap_err()
            .to_string();
        assert!(!help.contains("\n  publish "));
        assert!(!help.contains("\n  handle "));
        assert!(!help.contains("\n  appcoin "));
        assert!(!help.contains("public-action commands"));
    }

    #[test]
    fn an_interactive_creation_without_an_intention_opens_the_composer() {
        // `tohseno create paper` in a Terminal must not build anything. It
        // resolves to the composer, optionally prefilled, and the person
        // presses Create App once.
        let root = tempfile::tempdir().unwrap();
        assert!(master_prompt_prefill(root.path()).unwrap().is_none());

        let master = root.path().join("MASTER_PROMPT.md");
        fs::write(&master, "Make a paper app.\n").unwrap();
        assert_eq!(master_prompt_prefill(root.path()).unwrap(), Some(master));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_master_prompt_is_refused_rather_than_followed() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let elsewhere = root.path().join("elsewhere.md");
        fs::write(&elsewhere, "Not mine.\n").unwrap();
        symlink(&elsewhere, root.path().join("MASTER_PROMPT.md")).unwrap();
        assert!(master_prompt_prefill(root.path()).is_err());
    }

    #[test]
    fn a_prefilled_intention_is_staged_once_and_never_replayed_after_use() {
        let root = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(root.path().join("data"));
        ledger.initialize().unwrap();
        let master = root.path().join("MASTER_PROMPT.md");
        fs::write(&master, "Make a paper app.\n").unwrap();

        let first = stage_intention_in(&ledger, &master).unwrap().unwrap();
        // Re-running the same command reuses the same durable record instead
        // of accumulating duplicates of identical bytes.
        assert_eq!(
            stage_intention_in(&ledger, &master).unwrap(),
            Some(first.clone())
        );

        let store = tohseno_engine::PendingIntentionStore::for_ledger(&ledger);
        let pending = store.load(&first).unwrap();
        assert_eq!(pending.prompt, "Make a paper app.\n");
        store.consume_loaded(&pending).unwrap();
        assert_eq!(stage_intention_in(&ledger, &master).unwrap(), None);
    }

    #[test]
    fn retired_v07_mainnet_lifecycle_fails_closed() {
        let lifecycle = include_str!("../../scripts/lifecycle-mainnet.sh");
        assert!(lifecycle.contains("v0.7 Robinhood mainnet lifecycle is retired"));
        assert!(lifecycle.contains("exit 1"));
        assert!(!lifecycle.contains("cargo run"));
        assert!(!lifecycle.contains("deploy-candidate.sh"));
    }

    #[test]
    fn init_and_deploy_are_the_network_path_while_recording_is_explicit() {
        let create_help = Cli::try_parse_from(["tohseno", "create", "--help"])
            .unwrap_err()
            .to_string();
        assert!(create_help.contains("Secondary generated-app path"));
        assert!(create_help.contains("--prompt"));
        assert!(create_help.contains("--prompt-file"));
        assert!(create_help.contains("--image"));
        assert!(create_help.contains("--wait"));
        let create = Cli::try_parse_from([
            "tohseno",
            "create",
            "field-notebook",
            "--prompt",
            "Make a field notebook.",
            "--image",
            "/tmp/reference.png",
        ])
        .unwrap();
        assert!(matches!(
            create.command,
            Command::Create {
                app_name: Some(app_name),
                prompt: Some(prompt),
                images,
                ..
            } if app_name == "field-notebook"
                && prompt == "Make a field notebook."
                && images == [PathBuf::from("/tmp/reference.png")]
        ));
        let unnamed = Cli::try_parse_from([
            "tohseno",
            "create",
            "--prompt",
            "Keep a private log of the trails I hike.",
        ])
        .unwrap();
        assert!(matches!(
            unnamed.command,
            Command::Create {
                app_name: None,
                prompt: Some(_),
                ..
            }
        ));

        let evolve = Cli::try_parse_from([
            "tohseno",
            "evolve",
            "field-notebook",
            "--prompt-file",
            "/tmp/evolution.md",
            "--feedback-action",
            "0x1111111111111111111111111111111111111111111111111111111111111111",
        ])
        .unwrap();
        assert!(matches!(
            evolve.command,
            Command::Evolve {
                app_name: Some(app_name),
                prompt_file: Some(path),
                feedback_actions,
                ..
            } if app_name == "field-notebook"
                && path == PathBuf::from("/tmp/evolution.md")
                && feedback_actions.len() == 1
        ));
        let evolve_help = Cli::try_parse_from(["tohseno", "evolve", "--help"])
            .unwrap_err()
            .to_string();
        assert!(evolve_help.contains("--prompt-file"));
        assert!(evolve_help.contains("--feedback-action"));
        assert!(Cli::try_parse_from(["tohseno", "init", "/tmp/App.xcodeproj"]).is_ok());
        assert!(Cli::try_parse_from(["tohseno", "deploy", "--dry-run"]).is_ok());
        assert!(Cli::try_parse_from([
            "tohseno",
            "deploy",
            "--claim-edition",
            "limited",
            "--max-claims",
            "888",
            "--closes-at",
            "2099-09-08T18:00:00Z",
            "--app-slug",
            "field-notebook",
        ])
        .is_ok());
        assert!(Cli::try_parse_from(["tohseno", "deploy", "--claim-edition", "auction",]).is_err());
        assert!(Cli::try_parse_from([
            "tohseno",
            "recording",
            "record",
            "recording-app",
            "--note-file",
            "/tmp/note.md",
        ])
        .is_ok());
        assert!(Cli::try_parse_from(["tohseno", "create", "app", "--harness", "codex"]).is_err());
        assert!(Cli::try_parse_from(["tohseno", "evolve", "app", "--image", "ref.png"]).is_ok());
    }
    #[test]
    fn ordinary_help_contains_only_the_product_loop() {
        let help = Cli::try_parse_from(["tohseno", "--help"])
            .unwrap_err()
            .to_string();
        for command in [
            "create",
            "evolve",
            "init",
            "deploy",
            "status",
            "install",
            "fork",
            "recording",
            "studio",
            "service",
            "companion",
            "list",
            "doctor",
            "update",
            "uninstall",
            "advanced",
        ] {
            assert!(help.contains(&format!("  {command}")), "missing {command}");
        }
        for hidden in [
            "intent", "verify", "feedback", "genome", "identity", "protocol", "token", "shot",
        ] {
            assert!(!help.contains(&format!("  {hidden}")), "exposed {hidden}");
        }
        for removed in ["refresh", "retire", "adopt"] {
            assert!(
                Cli::try_parse_from(["tohseno", removed]).is_err(),
                "obsolete command remains parseable: {removed}"
            );
        }
        assert!(Cli::try_parse_from(["tohseno", "intent", "claim", "--stdin"]).is_ok());
    }

    #[test]
    fn advanced_help_names_the_retained_compatibility_tools() {
        let help = Cli::try_parse_from(["tohseno", "advanced", "--help"])
            .unwrap_err()
            .to_string();
        assert!(help.contains("Available commands: verify, inspect, feedback"));
        assert!(help.contains("tohseno advanced <command> --help"));
    }

    #[test]
    fn neutral_token_commands_are_distinct_from_genesis_appcoin_mutations() {
        let associated = Cli::try_parse_from([
            "tohseno",
            "token",
            "associate",
            "anky",
            "8453",
            "0xa7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7",
            "--symbol",
            "ANKY",
            "--public",
        ])
        .unwrap();
        assert!(matches!(
            associated.command,
            Command::Token {
                command: TokenCommand::Associate {
                    app_name,
                    chain_id: 8453,
                    symbol: Some(symbol),
                    public: true,
                    ..
                }
            } if app_name == "anky" && symbol == "ANKY"
        ));

        let removed = Cli::try_parse_from([
            "tohseno",
            "token",
            "remove",
            "anky",
            "8453",
            "0xa7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7",
        ])
        .unwrap();
        assert!(matches!(
            removed.command,
            Command::Token {
                command: TokenCommand::Remove {
                    chain_id: 8453,
                    public: false,
                    ..
                }
            }
        ));

        // The neutral command never accepts GENESIS relay/deadline switches.
        assert!(Cli::try_parse_from([
            "tohseno",
            "token",
            "associate",
            "anky",
            "8453",
            "0xa7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7",
            "--rpc-url",
            "https://example.invalid",
            "--deadline",
            "2000000000",
        ])
        .is_err());
    }

    #[test]
    fn note_file_reader_rejects_symlinks_and_non_utf8() {
        let temporary = tempfile::tempdir().unwrap();
        let note = temporary.path().join("version-note.md");
        fs::write(&note, "preserve these exact words\n").unwrap();
        assert_eq!(
            read_note_file(&note).unwrap(),
            "preserve these exact words\n"
        );

        let binary = temporary.path().join("binary.md");
        fs::write(&binary, [0xff, 0xfe]).unwrap();
        assert!(read_note_file(&binary).is_err());

        #[cfg(unix)]
        {
            let link = temporary.path().join("linked.md");
            std::os::unix::fs::symlink(&note, &link).unwrap();
            assert!(read_note_file(&link).is_err());
        }
    }

    #[test]
    fn feedback_files_accept_one_conventional_terminal_line_ending() {
        assert_eq!(
            strip_one_terminal_line_ending("one observation\n".into()),
            "one observation"
        );
        assert_eq!(
            strip_one_terminal_line_ending("one observation\r\n".into()),
            "one observation"
        );
        assert_eq!(
            strip_one_terminal_line_ending("two endings\n\n".into()),
            "two endings\n"
        );
    }

    #[test]
    fn portable_feedback_and_migration_commands_are_explicit() {
        let feedback = Cli::try_parse_from([
            "tohseno",
            "feedback",
            "field-notebook",
            "--version",
            "2",
            "--text",
            "The save state was unclear.",
            "--attachment",
            "/tmp/screenshot.png",
        ])
        .unwrap();
        assert!(matches!(
            feedback.command,
            Command::Feedback {
                app_name: Some(app_name),
                version: Some(2),
                text: Some(text),
                file: None,
                attachments,
                ..
            } if app_name == "field-notebook"
                && text == "The save state was unclear."
                && attachments == [PathBuf::from("/tmp/screenshot.png")]
        ));
        assert!(Cli::try_parse_from([
            "tohseno",
            "feedback",
            "--version",
            "1",
            "--text",
            "one",
            "--file",
            "/tmp/two",
        ])
        .is_err());

        let share = Cli::try_parse_from([
            "tohseno",
            "share",
            "field-notebook",
            "--output",
            "/tmp/field-notebook.tohseno-workshop",
        ])
        .unwrap();
        assert!(matches!(
            share.command,
            Command::Share {
                app_name: Some(app_name),
                output,
            } if app_name == "field-notebook"
                && output == PathBuf::from("/tmp/field-notebook.tohseno-workshop")
        ));

        let try_workshop = Cli::try_parse_from([
            "tohseno",
            "try",
            "/tmp/field-notebook.tohseno-workshop",
            "--output",
            "/tmp/field-notebook-workshop",
            "--no-launch",
        ])
        .unwrap();
        assert!(matches!(
            try_workshop.command,
            Command::Try {
                capsule,
                output,
                no_launch: true,
            } if capsule == PathBuf::from("/tmp/field-notebook.tohseno-workshop")
                && output == PathBuf::from("/tmp/field-notebook-workshop")
        ));

        let workshop_feedback = Cli::try_parse_from([
            "tohseno",
            "feedback",
            "--workshop",
            "/tmp/field-notebook-workshop",
            "--text",
            "The save gesture was clear.",
            "--author",
            "Maya",
            "--output",
            "/tmp/maya.tohseno-feedback",
        ])
        .unwrap();
        assert!(matches!(
            workshop_feedback.command,
            Command::Feedback {
                app_name: None,
                version: None,
                text: Some(text),
                workshop: Some(workshop),
                author: Some(author),
                output: Some(output),
                ..
            } if text == "The save gesture was clear."
                && workshop == PathBuf::from("/tmp/field-notebook-workshop")
                && author == "Maya"
                && output == PathBuf::from("/tmp/maya.tohseno-feedback")
        ));

        let imported_feedback = Cli::try_parse_from([
            "tohseno",
            "feedback",
            "field-notebook",
            "--packet",
            "/tmp/maya.tohseno-feedback",
        ])
        .unwrap();
        assert!(matches!(
            imported_feedback.command,
            Command::Feedback {
                app_name: Some(app_name),
                packet: Some(packet),
                ..
            } if app_name == "field-notebook"
                && packet == PathBuf::from("/tmp/maya.tohseno-feedback")
        ));

        let export = Cli::try_parse_from([
            "tohseno",
            "export",
            "field-notebook",
            "--output",
            "/tmp/field-notebook.shot",
            "--include-private",
        ])
        .unwrap();
        assert!(matches!(
            export.command,
            Command::Export {
                app_name: Some(app_name),
                output,
                include_private: true,
            } if app_name == "field-notebook"
                && output == PathBuf::from("/tmp/field-notebook.shot")
        ));

        let import = Cli::try_parse_from([
            "tohseno",
            "import",
            "/tmp/field-notebook.shot",
            "--output",
            "/tmp/received-shot",
        ])
        .unwrap();
        assert!(matches!(
            import.command,
            Command::Import { bundle, output }
                if bundle == PathBuf::from("/tmp/field-notebook.shot")
                    && output == PathBuf::from("/tmp/received-shot")
        ));
        assert!(matches!(
            Cli::try_parse_from(["tohseno", "migrate", "field-notebook"])
                .unwrap()
                .command,
            Command::Migrate {
                app_name: Some(app_name)
            } if app_name == "field-notebook"
        ));
        assert!(matches!(
            Cli::try_parse_from(["tohseno", "migrate-legacy"])
                .unwrap()
                .command,
            Command::MigrateLegacy { app_name: None }
        ));
    }
}
