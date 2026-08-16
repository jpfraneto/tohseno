mod companion_service;
mod companion_simulator;
mod identity_commands;
mod installation_commands;
mod intent_commands;
mod protocol_commands;
mod renderer;
mod service_client;
mod service_commands;
mod shot_commands;
mod simulator;
mod workspace_identity;
mod workspace_service;

use clap::{Parser, Subcommand};
use renderer::Renderer;
use serde_json::{json, Value};
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use tohseno_application::ReferenceInput;
use tohseno_engine::{Config, Engine, Event, EventBus, Ledger};
use tohseno_protocol::digest::Bytes32;
use uuid::Uuid;

const MAX_TEXT_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_FEEDBACK_FILE_BYTES: u64 = 100_000;

#[derive(Debug, Parser)]
#[command(
    name = "tohseno",
    version,
    about = "Create and evolve native apps in your private local factory",
    disable_help_subcommand = true
)]
struct Cli {
    /// Emit structured JSON for supported commands.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Begin an intention-led app birth in the local factory.
    Create {
        app_name: String,
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
    /// Begin an exact-base evolutionary transaction in the local factory.
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
    /// Initialize the explicit app-local recording layer (ADR 0014 compatibility).
    Init { app_name: String },
    /// Record the current app tree through the explicit recording layer.
    Record {
        app_name: Option<String>,
        #[arg(long, value_name = "TEXT")]
        note: Option<String>,
        #[arg(long, value_name = "PATH", conflicts_with = "note")]
        note_file: Option<PathBuf>,
    },
    /// Ensure the persistent Local Workspace Service is healthy, open Studio, and return.
    Studio {
        /// Development-only foreground port override.
        #[arg(long, hide = true)]
        foreground_port: Option<u16>,
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
    let cli = Cli::parse();
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
    let bus = EventBus::default();
    let render_task = if cli.json {
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
        } => factory_create(&app_name, prompt, prompt_file, images, wait, json, bus).await?,
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
        Command::Init { app_name } => {
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
        Command::Record {
            app_name,
            note,
            note_file,
        } => {
            recording_record(app_name, note, note_file, json, bus)?;
        }
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
                service
                    .open_studio("/")
                    .map_err(|error| error.to_string())?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string(&json!({
                            "schema": "tohseno.studio-opened/1",
                            "origin": service.runtime().origin,
                            "workspace_id": service.runtime().workspace_id,
                            "service_version": service.runtime().service_version,
                        }))?
                    );
                } else {
                    bus.emit(Event::result("Studio is open. The Local Workspace Service remains available after this Terminal closes."));
                }
            }
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
            let engine = Engine::discover(bus.clone())?;
            if !background {
                bus.emit(Event::status("checking this Mac…"));
            }
            engine.doctor_once()?;
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
    app_name: &str,
    prompt: Option<String>,
    prompt_file: Option<PathBuf>,
    images: Vec<PathBuf>,
    wait: bool,
    json_output: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = normalize_cli_app_name(app_name)?;
    if images.len() > 8 {
        return Err("at most eight reference images are accepted".into());
    }
    let intention = resolve_create_intention(prompt, prompt_file, json_output)?;
    let service = service_client::ServiceClient::ensure_running()
        .await
        .map_err(|error| error.to_string())?;
    let Some(intention) = intention else {
        service
            .open_studio(&format!("/create?name={name}"))
            .map_err(|error| error.to_string())?;
        bus.emit(Event::result(format!(
            "Studio is ready to create {name}. Add the exact intention, then TAKE THE SHOT."
        )));
        return Ok(());
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
        .post("/api/v1/shots", &body)
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
        Some(
            service
                .wait_for_execution(execution_id)
                .await
                .map_err(|error| error.to_string())?,
        )
    } else {
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
        bus.emit(Event::result(format!(
            "SHOT IN FLIGHT · command {command_id} · Shot {shot_id} · execution {execution_id}."
        )));
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
    if intention.is_empty() && feedback_actions.is_empty() {
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
        .post(&format!("/api/v1/shots/{shot_id}/evolutions"), &body)
        .await
        .map_err(|error| error.to_string())?;
    let execution_id = receipt
        .get("execution_id")
        .and_then(Value::as_str)
        .ok_or("Local Workspace Service returned no execution ID")?;
    let completion = if wait {
        Some(
            service
                .wait_for_execution(execution_id)
                .await
                .map_err(|error| error.to_string())?,
        )
    } else {
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
        bus.emit(Event::result(format!(
            "EVOLUTION IN FLIGHT · command {command_id} · execution {execution_id} · exact base Version {version_id}."
        )));
        if io::stdout().is_terminal() {
            let _ = service.open_studio(&format!("/shots/{shot_id}"));
        }
    }
    Ok(())
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
            let receipt = service_commands::restart(&paths, &SystemLaunchctl)?;
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

fn resolve_create_intention(
    prompt: Option<String>,
    prompt_file: Option<PathBuf>,
    json_output: bool,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if let Some(intention) = resolve_intention(prompt, prompt_file)? {
        return Ok(Some(intention));
    }
    if !io::stdin().is_terminal() {
        return Ok(Some(read_stdin_bounded(MAX_TEXT_FILE_BYTES as usize)?));
    }
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal() && !json_output;
    if interactive {
        let path = PathBuf::from("./MASTER_PROMPT.md");
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err("./MASTER_PROMPT.md exists but is not a regular file".into());
            }
            Ok(_) => {
                let intention = read_bounded_utf8(&path, MAX_TEXT_FILE_BYTES, "MASTER_PROMPT.md")?;
                let digest = tohseno_protocol::digest::sha256(intention.as_bytes());
                eprintln!("Using ./MASTER_PROMPT.md ({digest})");
                return Ok(Some(intention));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        }
    }
    Err(
        "no creation intention was supplied; use --prompt, --prompt-file, or bounded UTF-8 stdin"
            .into(),
    )
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
    fn retired_v07_mainnet_lifecycle_fails_closed() {
        let lifecycle = include_str!("../../scripts/lifecycle-mainnet.sh");
        assert!(lifecycle.contains("v0.7 Robinhood mainnet lifecycle is retired"));
        assert!(lifecycle.contains("exit 1"));
        assert!(!lifecycle.contains("cargo run"));
        assert!(!lifecycle.contains("deploy-candidate.sh"));
    }

    #[test]
    fn create_and_evolve_are_factory_commands_while_init_and_record_preserve_recording() {
        let create_help = Cli::try_parse_from(["tohseno", "create", "--help"])
            .unwrap_err()
            .to_string();
        assert!(create_help.contains("intention-led app birth"));
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
                app_name,
                prompt: Some(prompt),
                images,
                ..
            } if app_name == "field-notebook"
                && prompt == "Make a field notebook."
                && images == [PathBuf::from("/tmp/reference.png")]
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
        assert!(Cli::try_parse_from(["tohseno", "init", "recording-app"]).is_ok());
        assert!(Cli::try_parse_from([
            "tohseno",
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
            "record",
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
        assert!(!help.contains("  install"));
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
