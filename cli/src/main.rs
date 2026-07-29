mod identity_commands;
mod intake;
mod protocol_commands;
mod renderer;
mod simulator;
mod studio_server;

use clap::{Args, Parser, Subcommand, ValueEnum};
use renderer::Renderer;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use tohseno_engine::gates::intent::Intent;
use tohseno_engine::public_submission::EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION;
use tohseno_engine::{
    ConductedCreation, Config, Engine, Event, EventBus, HarnessOption, Ledger, ShotRequest,
};

#[derive(Debug, Parser)]
#[command(
    name = "tohseno",
    version,
    about = "A printing press for iOS apps",
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
    /// Create the first complete shot of an app.
    Create {
        app_name: String,
        #[arg(long, value_name = "PATH")]
        prompt_file: Option<PathBuf>,
        /// Use a detected coding agent by id (hermes, codex, claude, grok, opencode)
        /// or an absolute path to your own TASK.md-compatible agent executable.
        #[arg(long, value_name = "AGENT")]
        harness: Option<String>,
    },
    /// Seal the app folder's current state as the next Evolution.
    ///
    /// However the folder got there — your own agent, Xcode, an editor —
    /// `shot` snapshots it, runs every gate, signs the record, and appends
    /// an immutable Evolution. Run it inside the folder or pass the name.
    Shot {
        app_name: Option<String>,
        /// One line recorded as this Evolution's intention.
        #[arg(long, value_name = "TEXT")]
        note: Option<String>,
    },
    /// Create a new complete shot using the previous shot as context.
    Evolve {
        app_name: String,
        #[arg(long, value_name = "PATH")]
        prompt_file: Option<PathBuf>,
        /// Use a detected coding agent by id (hermes, codex, claude, grok, opencode)
        /// or an absolute path to your own TASK.md-compatible agent executable.
        #[arg(long, value_name = "AGENT")]
        harness: Option<String>,
    },
    /// Re-sign and install the latest shot of one app or every app.
    Refresh { app_name: Option<String> },
    /// List local apps and their shots.
    List,
    /// Remove an app from the phone without touching its ledger.
    Retire { app_name: String },
    /// Open the local Studio intake.
    Studio {
        #[arg(long, default_value_t = 0)]
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
    /// Create the first honest signed Evolution for a legacy local app.
    Adopt {
        app_name: String,
        #[arg(long, value_name = "AGENT")]
        harness: Option<String>,
    },
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
    /// Manage optional appcoin relations.
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
        Command::List => list(bus)?,
        Command::Create {
            app_name,
            prompt_file,
            harness,
        } => {
            let engine = Engine::discover(bus.clone())?;
            let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
            if harness.is_none() && interactive {
                // Conducted: the builder's own agent, in their own session,
                // working in the visible folder. `tohseno shot` seals it.
                let options = engine.harnesses();
                let agent_id = intake::choose_harness(&options, None, bus)?;
                engine.prime_toolchain();
                let prompt = intake::collect(prompt_file.as_deref(), bus)?;
                let creation = engine.create_folder(&ShotRequest {
                    app_name: app_name.clone(),
                    intent: Intent::parse(&prompt),
                    harness: None,
                })?;
                let agent =
                    agent_id.and_then(|id| options.into_iter().find(|option| option.id == id));
                conduct_agent(&creation, agent.as_ref(), &app_name, bus);
            } else {
                let harness = intake::choose_harness(&engine.harnesses(), harness.as_deref(), bus)?;
                engine.prime_toolchain();
                let prompt = intake::collect(prompt_file.as_deref(), bus)?;
                engine
                    .create(ShotRequest {
                        app_name,
                        intent: Intent::parse(&prompt),
                        harness,
                    })
                    .await?;
            }
        }
        Command::Shot { app_name, note } => {
            let (engine, name) = engine_for(app_name, bus)?;
            engine.seal(&name, note.as_deref()).await?;
        }
        Command::Evolve {
            app_name,
            prompt_file,
            harness,
        } => {
            let engine = Engine::discover(bus.clone())?;
            let harness = intake::choose_harness(&engine.harnesses(), harness.as_deref(), bus)?;
            engine.prime_toolchain();
            let prompt = intake::collect(prompt_file.as_deref(), bus)?;
            engine
                .evolve(ShotRequest {
                    app_name,
                    intent: Intent::parse(&prompt),
                    harness,
                })
                .await?;
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
        Command::Doctor { background } => {
            let engine = Engine::discover(bus.clone())?;
            if !background {
                bus.emit(Event::status("checking this Mac…"));
            }
            engine.doctor_once()?;
        }
        Command::Adopt { app_name, harness } => {
            let engine = Engine::discover(bus.clone())?;
            let harness = intake::choose_harness(&engine.harnesses(), harness.as_deref(), bus)?;
            engine.prime_toolchain();
            engine.adopt(&app_name, harness.as_deref()).await?;
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

/// Opens the builder's own agent in a new Terminal window on the folder.
fn conduct_agent(
    creation: &ConductedCreation,
    agent: Option<&HarnessOption>,
    app_name: &str,
    bus: &EventBus,
) {
    let folder = creation.folder.display();
    let Some(agent) = agent else {
        bus.emit(Event::handoff(format!(
            "open a terminal in {folder}, run your coding agent, then: tohseno shot {app_name}"
        )));
        return;
    };
    let instruction = format!(
        "Read .tohseno/TASK.md first, then build the complete app in this folder. When it builds, tell me to run: tohseno shot {app_name}"
    );
    let shell = format!(
        "cd {} && {} {}",
        shell_quote(&creation.folder.to_string_lossy()),
        agent.command,
        shell_quote(&instruction)
    );
    let script = format!(
        "tell application \"Terminal\"\n\tactivate\n\tdo script \"{}\"\nend tell",
        shell.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let opened = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if opened {
        bus.emit(Event::handoff(format!(
            "{} is working in {folder} — when it's done: tohseno shot {app_name}",
            agent.label
        )));
    } else {
        bus.emit(Event::handoff(format!(
            "open a terminal in {folder}, run {}, then: tohseno shot {app_name}",
            agent.label
        )));
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

fn list(bus: &EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = Ledger::discover()?;
    let apps = ledger.list_apps()?;
    if apps.is_empty() {
        bus.emit(Event::status("no apps yet."));
    } else {
        for app in apps {
            let detail = if let Some(number) = app.latest_shot {
                let shot = ledger.shot(&app.name, number)?;
                let artifact = shot.artifact_path().join(format!("{}.app", app.name));
                let expiry = tohseno_engine::gates::sign::days_until_expiry(&artifact)
                    .map(|days| format!("{days} days until expiry"))
                    .unwrap_or_else(|| "signing profile unavailable".into());
                format!("shots 1–{number} · {expiry}")
            } else {
                "no complete shots".into()
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
}
