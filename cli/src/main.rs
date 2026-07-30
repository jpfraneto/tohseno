mod bankr_launch;
mod identity_commands;
mod installation_commands;
mod intake;
mod protocol_commands;
mod renderer;
mod shot_commands;
mod shot_execution_commands;
mod simulator;
mod studio_server;

use clap::{Args, Parser, Subcommand, ValueEnum};
use renderer::Renderer;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use tohseno_engine::gates::intent::Intent;
use tohseno_engine::machine::Evolved;
use tohseno_engine::public_submission::EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION;
use tohseno_engine::{ConductedCreation, Config, Engine, Event, EventBus, Ledger, ShotRequest};

const MAX_PROMPT_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_FEEDBACK_FILE_BYTES: u64 = 100_000;
const DEFAULT_STUDIO_PORT: u16 = 8888;

#[derive(Debug, Parser)]
#[command(
    name = "tohseno",
    version,
    about = "Give one coherent intention persistent identity and a native Apple expression",
    disable_help_subcommand = true
)]
struct Cli {
    /// Emit structured JSON for supported inspection and public-action commands.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Install the latest stable TOHSENO release.
    #[command(alias = "upgrade")]
    Update,
    /// Remove TOHSENO program files while preserving every Shot and identity.
    Uninstall,
    /// Prepare a visible Shot folder, intention package, and native harness
    /// command. The harness starts only after Enter in the opened terminal.
    Create {
        app_name: String,
        /// Read the exact intention from a bounded UTF-8 text file.
        #[arg(long, value_name = "PATH")]
        prompt_file: Option<PathBuf>,
        /// Attach one reference image. Repeat up to eight times.
        #[arg(long = "image", value_name = "PATH")]
        images: Vec<PathBuf>,
        /// Native coding harness adapter, such as codex or claude-code.
        #[arg(long, value_name = "HARNESS")]
        harness: Option<String>,
        /// Harness model identifier or alias. Defaults to the harness configuration.
        #[arg(long, value_name = "MODEL")]
        model: Option<String>,
        /// Subscription, API, or configured inference route.
        #[arg(long, value_name = "ROUTE")]
        route: Option<String>,
        /// Explicitly accept the reviewed initial Genome and Apple expression plan.
        #[arg(long)]
        accept_genome: bool,
        /// Accept this schema-validated Genome instead of the deterministic proposal.
        #[arg(long, value_name = "PATH", requires = "accept_genome")]
        genome_file: Option<PathBuf>,
        /// Prepare the Shot folder without opening a terminal execution.
        #[arg(long)]
        no_launch: bool,
    },
    /// Record the folder's current state as this Shot's next Evolution.
    ///
    /// However the folder got there — your own agent, Xcode, an editor —
    /// `evolve` snapshots it, runs every gate, signs the record, and appends
    /// it to the Shot's history. Run it inside the folder or pass the name.
    /// Piped text hands your agent a new intent after recording.
    Evolve {
        app_name: Option<String>,
        /// One line recorded as this Evolution's intention.
        #[arg(long, value_name = "TEXT")]
        note: Option<String>,
        /// Read the evolutionary intention from a bounded UTF-8 text file.
        #[arg(long, value_name = "PATH", conflicts_with = "note")]
        prompt_file: Option<PathBuf>,
        /// Select one signed Feedback action for this evolutionary intent.
        ///
        /// Use the `action_commitment` returned by `tohseno --json feedback`.
        /// Repeat this option to select more than one exact-version observation.
        #[arg(long = "feedback-action", value_name = "0xBYTES32")]
        feedback_actions: Vec<String>,
        /// Attach one reference image. Repeat up to eight times.
        #[arg(long = "image", value_name = "PATH")]
        images: Vec<PathBuf>,
        /// Native coding harness adapter, such as codex or claude-code.
        #[arg(long, value_name = "HARNESS")]
        harness: Option<String>,
        /// Harness model identifier or alias.
        #[arg(long, value_name = "MODEL")]
        model: Option<String>,
        /// Subscription, API, or configured inference route.
        #[arg(long, value_name = "ROUTE")]
        route: Option<String>,
        /// Stage the intention without opening a new terminal execution.
        #[arg(long)]
        no_launch: bool,
    },
    /// Re-sign and install the latest shot of one app or every app.
    Refresh { app_name: Option<String> },
    /// List local apps and their shots.
    List,
    /// Remove an app from the phone without touching its ledger.
    Retire { app_name: String },
    /// Open the local Studio intake.
    Studio {
        /// Loopback port. Use 0 to ask macOS for any available port.
        #[arg(long, default_value_t = DEFAULT_STUDIO_PORT)]
        port: u16,
    },
    /// Check local prerequisites.
    Doctor {
        #[arg(long, hide = true)]
        background: bool,
    },
    /// Verify one local Shot or app deterministically without an LLM.
    Verify {
        target: String,
        /// Also verify the configured public registry through read-only RPC.
        #[arg(long)]
        public: bool,
    },
    /// Show exact local protocol facts for one app or Shot path.
    Inspect { target: String },
    /// Attach private experience to one exact accepted expression version.
    Feedback {
        app_name: Option<String>,
        /// Exact accepted version ordinal, such as 1 for version 0001.
        #[arg(long, value_name = "N")]
        version: u64,
        /// Feedback text. Use --file for longer material.
        #[arg(
            long,
            value_name = "TEXT",
            conflicts_with = "file",
            required_unless_present = "file"
        )]
        text: Option<String>,
        /// Read feedback from a bounded UTF-8 regular file.
        #[arg(
            long,
            value_name = "PATH",
            conflicts_with = "text",
            required_unless_present = "text"
        )]
        file: Option<PathBuf>,
        /// Copy one private attachment by digest. Repeat for multiple files.
        #[arg(long = "attachment", value_name = "PATH")]
        attachments: Vec<PathBuf>,
    },
    /// Export verified Shot records as a portable bundle, not a source archive.
    Export {
        app_name: Option<String>,
        #[arg(long, value_name = "DIRECTORY")]
        output: PathBuf,
        /// Include exact private intention and feedback bytes.
        #[arg(long)]
        include_private: bool,
    },
    /// Verify and receive a portable Shot record bundle without taking ownership.
    Import {
        bundle: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        output: PathBuf,
    },
    /// Project frozen v1 Evolutions into the neutral model without rewriting them.
    Migrate { app_name: Option<String> },
    /// Copy preserved v0.6 apps into visible folders, then project their
    /// frozen signed history without changing the old ledger.
    MigrateLegacy { app_name: Option<String> },
    /// Inspect or explicitly accept a Shot Genome revision.
    Genome {
        #[command(subcommand)]
        command: GenomeCommand,
    },
    /// Turn the current folder into a Shot: it gains its ledger and its
    /// first recorded Evolution, without changing the app itself.
    Adopt,
    /// Inspect the durable BuilderID, fixed initial DeviceKey, and local backup.
    Identity {
        #[command(subcommand)]
        command: IdentityCommand,
    },
    /// Inspect protocol law and independently verify record files.
    Protocol {
        #[command(subcommand)]
        command: ProtocolCommand,
    },
    /// Build a deterministic static page for a local Shot.
    Page {
        #[command(subcommand)]
        command: PageCommand,
    },
    /// Publish a locally verified Shot through the experimental registry.
    Publish {
        app_name: String,
        #[command(flatten)]
        public: PublicMutationArgs,
    },
    /// Inspect configured chain and candidate deployment evidence.
    Network {
        #[command(subcommand)]
        command: NetworkCommand,
    },
    /// Inspect public-registry state for a Shot.
    Registry {
        #[command(subcommand)]
        command: RegistryCommand,
    },
    /// Manage optional human-readable Shot handles.
    Handle {
        #[command(subcommand)]
        command: HandleCommand,
    },
    /// Record optional chain-specific Token Associations in canonical Shot lineage.
    ///
    /// This neutral protocol action never changes Shot identity or ownership.
    /// `--public` writes an explicit node-ingestible outbox record but does not
    /// relay it or perform an on-chain transaction.
    Token {
        #[command(subcommand)]
        command: TokenCommand,
    },
    /// Prepare, run, follow, and inspect authentic local harness executions.
    Shot {
        #[command(subcommand)]
        command: ShotExecutionCommand,
    },
    /// Manage optional appcoin relations.
    ///
    /// This is the frozen GENESIS ShotRelations compatibility surface on
    /// Robinhood Chain, not the neutral v2 Token Association lifecycle.
    Appcoin {
        #[command(subcommand)]
        command: AppcoinCommand,
    },
}

#[derive(Debug, Subcommand)]
enum IdentityCommand {
    /// Show the current BuilderID and public recovery status.
    Show,
    /// Reveal or create a local recovery-authority backup; this does not activate recovery.
    Backup {
        #[arg(long)]
        confirm: bool,
        #[arg(long, value_name = "ABSOLUTE_PATH")]
        passphrase_file: Option<PathBuf>,
    },
    /// Import recovery words as an encrypted local backup for the current BuilderID.
    ///
    /// This does not recover or rotate an account.
    ImportBackup {
        /// Confirm this local-only backup import for the current BuilderID.
        #[arg(long)]
        confirm: bool,
        /// Read the 24 secret backup words from a private local file.
        #[arg(long, value_name = "ABSOLUTE_PATH")]
        mnemonic_file: Option<PathBuf>,
        /// Read the new vault-encryption passphrase from a private local file.
        #[arg(long, value_name = "ABSOLUTE_PATH")]
        passphrase_file: Option<PathBuf>,
    },
    /// Show the original local DeviceKey accepted by this candidate.
    Devices,
}

#[derive(Debug, Subcommand)]
enum ProtocolCommand {
    /// Show protocol identifiers, versions, and candidate network coordinates.
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
    /// Sign a reviewed initial Genome or explicit mutation.
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
    /// Show candidate chain, precompile, and deployment evidence.
    Status,
}

#[derive(Debug, Subcommand)]
enum RegistryCommand {
    /// Show the local head and any evidence-backed public witness.
    Show { app_name: String },
}

#[derive(Debug, Subcommand)]
enum HandleCommand {
    /// Claim a handle through the normal signed relation path.
    Claim {
        handle: String,
        app_name: String,
        #[command(flatten)]
        public: PublicMutationArgs,
    },
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
        /// Mark the signed action public and write it to the local public-action outbox.
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
        /// Mark the signed removal public and write it to the local public-action outbox.
        #[arg(long)]
        public: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ShotExecutionCommand {
    /// List installed adapters, models, routes, authentication, and cost facts.
    Harnesses,
    /// Run one prepared execution in this authentic terminal session.
    Run {
        #[arg(long)]
        app: String,
        #[arg(long)]
        execution: String,
    },
    /// Follow durable structured events without requiring Studio.
    Follow {
        #[arg(long)]
        app: String,
        #[arg(long)]
        execution: String,
    },
    /// Inspect the durable completion record.
    Result {
        #[arg(long)]
        app: String,
        #[arg(long)]
        execution: String,
    },
    /// Mark a prepared or abandoned local execution cancelled.
    Cancel {
        #[arg(long)]
        app: String,
        #[arg(long)]
        execution: String,
    },
}

#[derive(Debug, Subcommand)]
enum AppcoinCommand {
    /// Associate an optional token with an already-published Shot.
    Associate {
        app_name: String,
        chain_id: u64,
        token_address: String,
        #[command(flatten)]
        public: PublicMutationArgs,
    },
}

#[derive(Clone, Debug, Args)]
struct PublicMutationArgs {
    /// Explicit read/relay RPC endpoint. Embedded credentials are rejected.
    #[arg(long, value_name = "URL")]
    rpc_url: String,
    /// Unix deadline signed into the exact public action.
    #[arg(long)]
    deadline: u64,
    /// Relay after preparing and persisting the signed action.
    #[arg(long)]
    submit: bool,
    /// Exact experimental-mainnet confirmation sentence.
    #[arg(long, requires = "submit", value_name = "SENTENCE")]
    confirm_experimental_mainnet: Option<String>,
    /// Required only when --submit would irreversibly deploy the missing BuilderAccount.
    ///
    /// The exact sentence is:
    /// "I UNDERSTAND THIS WILL IRREVERSIBLY DEPLOY MY BUILDERACCOUNT TO ROBINHOOD CHAIN MAINNET 4663"
    #[arg(long, requires = "submit", value_name = "SENTENCE")]
    confirm_builder_account_deployment: Option<String>,
    /// Named Foundry keystore account; raw secrets are never accepted.
    #[arg(
        long,
        requires = "submit",
        conflicts_with = "hardware_wallet",
        value_name = "NAME"
    )]
    foundry_account: Option<String>,
    /// Attached hardware wallet used only to pay relay gas.
    #[arg(
        long,
        requires = "submit",
        conflicts_with = "foundry_account",
        value_enum
    )]
    hardware_wallet: Option<HardwareWallet>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum HardwareWallet {
    Ledger,
    Trezor,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Cli::parse()).await {
        if error
            .downcast_ref::<tohseno_engine::EngineError>()
            .is_some_and(|error| matches!(error, tohseno_engine::EngineError::SlotLimit))
        {
            std::process::exit(1);
        }
        eprintln!("tohseno: {error}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let bus = EventBus::default();
    let renderer = Renderer::new(io::stdout(), io::stdout().is_terminal());
    let render_task = tokio::spawn(renderer.follow(bus.subscribe()));
    if !cli.json
        && io::stdout().is_terminal()
        && !matches!(&cli.command, Command::Update | Command::Uninstall)
    {
        installation_commands::maybe_emit_update_notice(&bus).await;
    }
    let outcome = dispatch(cli.command, &bus, cli.json).await;
    drop(bus);
    render_task.await??;
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
            prompt_file,
            images,
            harness,
            model,
            route,
            accept_genome,
            genome_file,
            no_launch,
        } => {
            let engine = Engine::discover(bus.clone())?;
            engine.prime_toolchain();
            let prompt = collect_prompt(prompt_file.as_deref(), bus)?;
            let request = ShotRequest {
                app_name,
                intent: Intent::parse(&prompt).with_images(images),
                selected_feedback_actions: Vec::new(),
            };
            let default_genome = Engine::propose_initial_genome(&request)?;
            let proposed_genome = match genome_file.as_deref() {
                Some(path) => read_genome_file(path)?,
                None => default_genome,
            };
            let expression_plan =
                Engine::propose_initial_expression_plan(&request, &proposed_genome)?;
            let creation = engine.create(&request)?;
            preserve_initial_review(&creation, &proposed_genome, &expression_plan)?;
            present_initial_review(&proposed_genome, &expression_plan, bus)?;
            let accepted = accept_genome
                || (io::stdin().is_terminal()
                    && io::stdout().is_terminal()
                    && confirm_initial_review()?);
            if !accepted {
                bus.emit(Event::handoff(format!(
                    "Review the exact proposal under {}/.tohseno/private/planning, then rerun `tohseno create {} --accept-genome{}`.",
                    creation.folder.display(),
                    request.app_name,
                    if no_launch { " --no-launch" } else { "" }
                )));
                return Ok(());
            }
            engine.accept_genome(
                &request.app_name,
                &proposed_genome,
                "Owner reviewed and accepted the initial operational Genome.",
                &[],
            )?;
            engine.declare_initial_expression(&request.app_name, &expression_plan)?;
            let creation = engine.conduct_accepted_creation(&request.app_name)?;
            if no_launch {
                handoff_without_launch(&creation, bus);
            } else {
                let selection = shot_execution_commands::selection(
                    &engine,
                    harness.as_deref(),
                    model.as_deref(),
                    route.as_deref(),
                )?;
                shot_execution_commands::prepare(
                    &engine,
                    &creation,
                    &request.app_name,
                    &selection,
                    true,
                    bus,
                )?;
            }
        }
        Command::Evolve {
            app_name,
            note,
            prompt_file,
            feedback_actions,
            images,
            harness,
            model,
            route,
            no_launch,
        } => {
            let (engine, name) = engine_for(app_name, bus)?;
            let selected_feedback_actions = parse_feedback_actions(&feedback_actions)?;
            let prompt = if let Some(path) = prompt_file.as_deref() {
                read_prompt_file(path)?
            } else if io::stdin().is_terminal() {
                String::new()
            } else {
                intake::collect(bus)?
            };
            if prompt.trim().is_empty() && note.is_some() {
                if !selected_feedback_actions.is_empty() || !images.is_empty() {
                    return Err(
                        "--feedback-action and --image require an evolutionary instruction, not --note".into(),
                    );
                }
                engine.record(&name, note.as_deref()).await?;
            } else {
                match engine
                    .evolve(&ShotRequest {
                        app_name: name.clone(),
                        intent: Intent::parse(&prompt).with_images(images),
                        selected_feedback_actions,
                    })
                    .await?
                {
                    Evolved::Conducted(creation) if no_launch => {
                        handoff_without_launch(&creation, bus)
                    }
                    Evolved::Conducted(creation) => {
                        let selection = shot_execution_commands::selection(
                            &engine,
                            harness.as_deref(),
                            model.as_deref(),
                            route.as_deref(),
                        )?;
                        shot_execution_commands::prepare(
                            &engine, &creation, &name, &selection, true, bus,
                        )?;
                    }
                    Evolved::Recorded(_) | Evolved::NothingNew(_) => {}
                }
            }
        }
        Command::Refresh { app_name } => {
            Engine::discover(bus.clone())?
                .refresh(app_name.as_deref())
                .await?;
        }
        Command::Retire { app_name } => {
            Engine::discover(bus.clone())?.retire(&app_name).await?;
        }
        Command::Studio { port } => {
            studio_server::serve(port, bus.clone()).await?;
        }
        Command::Shot { command } => match command {
            ShotExecutionCommand::Harnesses => {
                let engine = Engine::discover(bus.clone())?;
                let harnesses = engine.harnesses();
                if json {
                    println!("{}", serde_json::to_string_pretty(&harnesses)?);
                } else {
                    for harness in harnesses {
                        let state = if harness.installed {
                            "available"
                        } else {
                            "unavailable"
                        };
                        let routes = harness
                            .routes
                            .iter()
                            .filter(|route| route.available)
                            .map(|route| {
                                let cost = route
                                    .estimated_additional_cost_usd
                                    .map(|cost| format!("${cost:.2}"))
                                    .unwrap_or_else(|| "usage-based".into());
                                format!("{} ({cost})", route.label)
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        bus.emit(Event::status(format!(
                            "{} · {} · {}",
                            harness.label,
                            state,
                            if routes.is_empty() {
                                "no authenticated route".into()
                            } else {
                                routes
                            }
                        )));
                    }
                }
            }
            ShotExecutionCommand::Run { app, execution } => {
                shot_execution_commands::run(&app, &execution, json, bus).await?;
            }
            ShotExecutionCommand::Follow { app, execution } => {
                shot_execution_commands::follow(&app, &execution, json, bus).await?;
            }
            ShotExecutionCommand::Result { app, execution } => {
                shot_execution_commands::result(&app, &execution, json, bus)?;
            }
            ShotExecutionCommand::Cancel { app, execution } => {
                shot_execution_commands::cancel(&app, &execution, json, bus)?;
            }
        },
        Command::Doctor { background } => {
            let engine = Engine::discover(bus.clone())?;
            if !background {
                bus.emit(Event::status("checking this Mac…"));
            }
            engine.doctor_once()?;
        }
        Command::Adopt => {
            let folder = std::env::current_dir()?;
            let (ledger, name) = Ledger::for_app_folder(&folder)?;
            ledger.initialize()?;
            let config = Config::load_or_create(ledger.machine_root())?;
            let engine = Engine::at(ledger, bus.clone(), config);
            engine.adopt(&name).await?;
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
            protocol_commands::verify_target(&target, public, json, bus)?;
        }
        Command::Inspect { target } => protocol_commands::inspect_target(&target, json, bus)?,
        Command::Feedback {
            app_name,
            version,
            text,
            file,
            attachments,
        } => {
            let (engine, name) = engine_for(app_name, bus)?;
            let feedback = match (text, file) {
                (Some(text), None) => text,
                (None, Some(path)) => strip_one_terminal_line_ending(read_bounded_utf8(
                    &path,
                    MAX_FEEDBACK_FILE_BYTES,
                    "feedback file",
                )?),
                _ => unreachable!("clap requires exactly one feedback text source"),
            };
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
        Command::Publish { app_name, public } => {
            protocol_commands::publish(&app_name, &public, json, bus)?;
        }
        Command::Network { command } => match command {
            NetworkCommand::Status => protocol_commands::network_status(json, bus)?,
        },
        Command::Registry { command } => match command {
            RegistryCommand::Show { app_name } => {
                protocol_commands::registry_show(&app_name, json, bus)?;
            }
        },
        Command::Handle { command } => match command {
            HandleCommand::Claim {
                handle,
                app_name,
                public,
            } => {
                protocol_commands::claim_handle(&handle, &app_name, &public, json, bus)?;
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
        Command::Appcoin { command } => match command {
            AppcoinCommand::Associate {
                app_name,
                chain_id,
                token_address,
                public,
            } => protocol_commands::associate_appcoin(
                &app_name,
                chain_id,
                &token_address,
                &public,
                json,
                bus,
            )?,
        },
    }
    Ok(())
}

fn collect_prompt(
    prompt_file: Option<&std::path::Path>,
    bus: &EventBus,
) -> Result<String, Box<dyn std::error::Error>> {
    match prompt_file {
        Some(path) => read_prompt_file(path),
        None => Ok(intake::collect(bus)?),
    }
}

fn read_prompt_file(path: &std::path::Path) -> Result<String, Box<dyn std::error::Error>> {
    read_bounded_utf8(path, MAX_PROMPT_FILE_BYTES, "prompt file")
}

fn strip_one_terminal_line_ending(mut text: String) -> String {
    if text.ends_with("\r\n") {
        text.truncate(text.len() - 2);
    } else if text.ends_with('\n') {
        text.pop();
    }
    text
}

fn parse_feedback_actions(
    values: &[String],
) -> Result<Vec<tohseno_protocol::digest::Bytes32>, Box<dyn std::error::Error>> {
    let mut actions = values
        .iter()
        .map(|value| {
            tohseno_protocol::digest::Bytes32::from_hex("feedback action commitment", value)
        })
        .collect::<Result<Vec<_>, _>>()?;
    actions.sort_unstable();
    if actions.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("feedback action commitments must not repeat".into());
    }
    Ok(actions)
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
    let bytes = fs::read(path)?;
    Ok(String::from_utf8(bytes).map_err(|_| format!("{label} must contain valid UTF-8 text"))?)
}

fn read_genome_file(
    path: &std::path::Path,
) -> Result<tohseno_protocol::Genome, Box<dyn std::error::Error>> {
    let text = read_bounded_utf8(path, MAX_PROMPT_FILE_BYTES, "genome file")?;
    let genome = serde_json::from_str::<tohseno_protocol::Genome>(&text)?;
    genome.validate()?;
    Ok(genome)
}

fn preserve_initial_review(
    creation: &ConductedCreation,
    genome: &tohseno_protocol::Genome,
    plan: &tohseno_engine::InitialExpressionPlan,
) -> Result<(), Box<dyn std::error::Error>> {
    let layout = tohseno_engine::ShotLayout::at(&creation.folder);
    let genome_json = tohseno_protocol::canonical::to_vec(genome)?;
    let genome_markdown = tohseno_engine::render_genome_document(genome)?;
    let expression_json = tohseno_protocol::canonical::to_vec(plan)?;
    layout.preserve_private_planning_file("genome-proposal.json", &genome_json)?;
    layout.preserve_private_planning_file("GENOME.proposed.md", genome_markdown.as_bytes())?;
    layout.preserve_private_planning_file("expression-plan.proposed.json", &expression_json)?;
    Ok(())
}

fn present_initial_review(
    genome: &tohseno_protocol::Genome,
    plan: &tohseno_engine::InitialExpressionPlan,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    bus.emit(Event::status(
        "review the proposed Shot Genome and first expression:",
    ));
    bus.emit(Event::status(tohseno_engine::render_genome_document(
        genome,
    )?));
    bus.emit(Event::status(format!(
        "Expression · {} · {} · {} declared organs",
        plan.name,
        plan.platforms.join(", "),
        plan.organs.len()
    )));
    Ok(())
}

fn confirm_initial_review() -> Result<bool, std::io::Error> {
    print!("Accept this Genome and native Apple expression plan? [y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}

/// Resolves the engine and app name, git-style: an explicit name uses the
/// configured homes; no name walks up from the current directory to the
/// nearest folder carrying a `.tohseno` ledger.
fn engine_for(
    app_name: Option<String>,
    bus: &EventBus,
) -> Result<(Engine, String), Box<dyn std::error::Error>> {
    if let Some(name) = app_name {
        return Ok((Engine::discover(bus.clone())?, name));
    }
    let mut directory = std::env::current_dir()?;
    loop {
        if directory.join(".tohseno").join("app.toml").is_file() {
            let (ledger, name) = Ledger::for_app_folder(&directory)?;
            ledger.initialize()?;
            let config = Config::load_or_create(ledger.machine_root())?;
            return Ok((Engine::at(ledger, bus.clone(), config), name));
        }
        if !directory.pop() {
            break;
        }
    }
    Err("run this inside an app folder, or pass the app name".into())
}

fn handoff_without_launch(creation: &ConductedCreation, bus: &EventBus) {
    bus.emit(Event::handoff(format!(
        "Shot body is ready at {}. Next: open that folder with your agent; AGENTS.md carries the exact recording handoff.",
        creation.folder.display()
    )));
}

fn list(bus: &EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = Ledger::discover()?;
    let apps = ledger.list_apps()?;
    if apps.is_empty() {
        bus.emit(Event::status("no apps yet."));
    } else {
        for app in apps {
            let detail = if let Some(number) = app.latest_evolution {
                let shot = ledger.shot(&app.name, number)?;
                let artifact = shot.artifact_path().join(format!("{}.app", app.name));
                let expiry = tohseno_engine::gates::sign::days_until_expiry(&artifact)
                    .map(|days| format!("{days} days until expiry"))
                    .unwrap_or_else(|| "signing profile unavailable".into());
                format!("evolutions 1–{number} · {expiry}")
            } else {
                "no complete evolutions".into()
            };
            let retired = if app.retired { " · retired" } else { "" };
            bus.emit(Event::status(format!("{} · {detail}{retired}", app.name)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn studio_uses_a_predictable_unprivileged_port_by_default() {
        let parsed = Cli::try_parse_from(["tohseno", "studio"]).unwrap();
        assert!(matches!(
            parsed.command,
            Command::Studio {
                port: DEFAULT_STUDIO_PORT
            }
        ));

        let ephemeral = Cli::try_parse_from(["tohseno", "studio", "--port", "0"]).unwrap();
        assert!(matches!(ephemeral.command, Command::Studio { port: 0 }));
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
        assert!(help.contains("encrypted local backup for the current BuilderID"));
        assert!(help.contains("does not recover or rotate an account"));
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
    fn public_mutations_require_explicit_rpc_and_deadline() {
        assert!(Cli::try_parse_from(["tohseno", "publish", "field-notebook"]).is_err());
        let parsed = Cli::try_parse_from([
            "tohseno",
            "publish",
            "field-notebook",
            "--rpc-url",
            "https://rpc.mainnet.chain.robinhood.com",
            "--deadline",
            "2000000000",
        ])
        .unwrap();
        assert!(matches!(
            parsed.command,
            Command::Publish {
                public: PublicMutationArgs {
                    deadline: 2_000_000_000,
                    submit: false,
                    ..
                },
                ..
            }
        ));
        assert!(Cli::try_parse_from([
            "tohseno",
            "publish",
            "field-notebook",
            "--rpc-url",
            "https://rpc.mainnet.chain.robinhood.com",
            "--deadline",
            "2000000000",
            "--content-commitment",
            "0x00",
        ])
        .is_err());
    }

    #[test]
    fn submit_surface_has_no_raw_secret_option() {
        let base = [
            "tohseno",
            "publish",
            "field-notebook",
            "--rpc-url",
            "https://rpc.mainnet.chain.robinhood.com",
            "--deadline",
            "2000000000",
            "--submit",
            "--confirm-experimental-mainnet",
            tohseno_engine::public_submission::EXACT_MAINNET_CONFIRMATION,
        ];
        let mut named = base.to_vec();
        named.extend(["--foundry-account", "relay-mainnet"]);
        assert!(Cli::try_parse_from(named).is_ok());
        let mut hardware = base.to_vec();
        hardware.extend(["--hardware-wallet", "ledger"]);
        assert!(Cli::try_parse_from(hardware).is_ok());
        let mut raw = base.to_vec();
        raw.extend(["--private-key", "0xdeadbeef"]);
        assert!(Cli::try_parse_from(raw).is_err());
    }

    #[test]
    fn builder_account_deployment_confirmation_is_explicit_in_help() {
        let help = Cli::try_parse_from(["tohseno", "publish", "--help"])
            .unwrap_err()
            .to_string();
        assert!(help.contains("--confirm-builder-account-deployment <SENTENCE>"));
        assert!(help.contains("irreversibly deploy the missing BuilderAccount"));
        assert!(help.contains("IRREVERSIBLY DEPLOY MY BUILDERACCOUNT"));
        assert!(help.contains("ROBINHOOD CHAIN MAINNET 4663"));
    }

    #[test]
    fn builder_account_deployment_confirmation_remains_runtime_conditional() {
        assert!(Cli::try_parse_from([
            "tohseno",
            "publish",
            "field-notebook",
            "--rpc-url",
            "https://rpc.mainnet.chain.robinhood.com",
            "--deadline",
            "2000000000",
            "--confirm-builder-account-deployment",
            EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION,
        ])
        .is_err());

        let parsed = Cli::try_parse_from([
            "tohseno",
            "publish",
            "field-notebook",
            "--rpc-url",
            "https://rpc.mainnet.chain.robinhood.com",
            "--deadline",
            "2000000000",
            "--submit",
            "--confirm-experimental-mainnet",
            tohseno_engine::public_submission::EXACT_MAINNET_CONFIRMATION,
            "--foundry-account",
            "relay-mainnet",
        ])
        .unwrap();
        assert!(matches!(
            parsed.command,
            Command::Publish {
                public: PublicMutationArgs {
                    confirm_builder_account_deployment: None,
                    ..
                },
                ..
            }
        ));

        let parsed = Cli::try_parse_from([
            "tohseno",
            "publish",
            "field-notebook",
            "--rpc-url",
            "https://rpc.mainnet.chain.robinhood.com",
            "--deadline",
            "2000000000",
            "--submit",
            "--confirm-experimental-mainnet",
            tohseno_engine::public_submission::EXACT_MAINNET_CONFIRMATION,
            "--confirm-builder-account-deployment",
            EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION,
            "--foundry-account",
            "relay-mainnet",
        ])
        .unwrap();
        assert!(matches!(
            parsed.command,
            Command::Publish {
                public: PublicMutationArgs {
                    confirm_builder_account_deployment: Some(value),
                    ..
                },
                ..
            } if value == EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION
        ));
    }

    #[test]
    fn lifecycle_forwards_separate_builder_account_deployment_authorization() {
        let lifecycle = include_str!("../../scripts/lifecycle-mainnet.sh");
        assert!(lifecycle.contains("--confirm-builder-account-deployment"));
        assert!(lifecycle.contains("TOHSENO_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION"));
        assert!(!lifecycle.contains(EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION));

        let readme = include_str!("../../README.md");
        assert!(readme.contains("--confirm-builder-account-deployment"));
        assert!(readme.contains(EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION));
    }

    #[test]
    fn create_and_evolve_accept_automation_safe_prompt_files() {
        let create = Cli::try_parse_from([
            "tohseno",
            "create",
            "field-notebook",
            "--prompt-file",
            "/tmp/intention.md",
            "--no-launch",
        ])
        .unwrap();
        assert!(matches!(
            create.command,
            Command::Create {
                app_name,
                prompt_file: Some(path),
                accept_genome: false,
                genome_file: None,
                no_launch: true,
                ..
            } if app_name == "field-notebook" && path == PathBuf::from("/tmp/intention.md")
        ));

        let evolve = Cli::try_parse_from([
            "tohseno",
            "evolve",
            "field-notebook",
            "--prompt-file",
            "/tmp/evolution.md",
            "--no-launch",
        ])
        .unwrap();
        assert!(matches!(
            evolve.command,
            Command::Evolve {
                app_name: Some(app_name),
                note: None,
                prompt_file: Some(path),
                feedback_actions,
                no_launch: true,
                ..
            } if app_name == "field-notebook"
                && path == PathBuf::from("/tmp/evolution.md")
                && feedback_actions.is_empty()
        ));

        let accepted = Cli::try_parse_from([
            "tohseno",
            "create",
            "field-notebook",
            "--prompt-file",
            "/tmp/intention.md",
            "--accept-genome",
            "--genome-file",
            "/tmp/reviewed-genome.json",
            "--no-launch",
        ])
        .unwrap();
        assert!(matches!(
            accepted.command,
            Command::Create {
                accept_genome: true,
                genome_file: Some(path),
                ..
            } if path == PathBuf::from("/tmp/reviewed-genome.json")
        ));
        assert!(Cli::try_parse_from([
            "tohseno",
            "create",
            "field-notebook",
            "--genome-file",
            "/tmp/unaccepted.json",
        ])
        .is_err());
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
    fn prompt_file_reader_rejects_symlinks_and_non_utf8() {
        let temporary = tempfile::tempdir().unwrap();
        let prompt = temporary.path().join("intention.md");
        fs::write(&prompt, "preserve these exact words\n").unwrap();
        assert_eq!(
            read_prompt_file(&prompt).unwrap(),
            "preserve these exact words\n"
        );

        let binary = temporary.path().join("binary.md");
        fs::write(&binary, [0xff, 0xfe]).unwrap();
        assert!(read_prompt_file(&binary).is_err());

        #[cfg(unix)]
        {
            let link = temporary.path().join("linked.md");
            std::os::unix::fs::symlink(&prompt, &link).unwrap();
            assert!(read_prompt_file(&link).is_err());
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
    fn evolve_selects_exact_signed_feedback_action_commitments() {
        let first = format!("0x{}", "11".repeat(32));
        let second = format!("0x{}", "22".repeat(32));
        let parsed = Cli::try_parse_from([
            "tohseno",
            "evolve",
            "field-notebook",
            "--prompt-file",
            "/tmp/evolution.md",
            "--feedback-action",
            &second,
            "--feedback-action",
            &first,
            "--no-launch",
        ])
        .unwrap();
        let Command::Evolve {
            feedback_actions, ..
        } = parsed.command
        else {
            panic!("expected evolve");
        };
        assert_eq!(feedback_actions, [second, first]);
        let commitments = parse_feedback_actions(&feedback_actions).unwrap();
        assert_eq!(commitments[0].to_string(), format!("0x{}", "11".repeat(32)));
        assert_eq!(commitments[1].to_string(), format!("0x{}", "22".repeat(32)));

        assert!(parse_feedback_actions(&[
            format!("0x{}", "11".repeat(32)),
            format!("0x{}", "11".repeat(32)),
        ])
        .is_err());
        assert!(parse_feedback_actions(&["0x11".into()]).is_err());
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
                version: 2,
                text: Some(text),
                file: None,
                attachments,
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
