mod identity_commands;
mod installation_commands;
mod protocol_commands;
mod renderer;
mod shot_commands;
mod simulator;
mod studio_server;

use clap::{Parser, Subcommand};
use renderer::Renderer;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use tohseno_engine::{Config, Engine, Event, EventBus, Ledger};

const MAX_TEXT_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_FEEDBACK_FILE_BYTES: u64 = 100_000;
const DEFAULT_STUDIO_PORT: u16 = 8888;

#[derive(Debug, Parser)]
#[command(
    name = "tohseno",
    version,
    about = "Record versions of an app beside its filesystem",
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
    /// Initialize an app folder with an embedded .tohseno history.
    Create { app_name: String },
    /// Record the app folder as its next Version.
    Evolve {
        app_name: Option<String>,
        /// Optional note stored with the Version.
        #[arg(long, value_name = "TEXT")]
        note: Option<String>,
        /// Read the Version note from a bounded UTF-8 text file.
        #[arg(
            long,
            value_name = "PATH",
            conflicts_with = "note",
            alias = "prompt-file"
        )]
        note_file: Option<PathBuf>,
    },
    /// Open local Studio.
    Studio {
        /// Loopback port. Use 0 to ask macOS for any available port.
        #[arg(long, default_value_t = DEFAULT_STUDIO_PORT)]
        port: u16,
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
        Command::Create { app_name } => {
            let app_name = normalize_cli_app_name(&app_name)?;
            let engine = Engine::discover(bus.clone())?;
            engine.initialize_app(&app_name)?;
        }
        Command::Evolve {
            app_name,
            note,
            note_file,
        } => {
            let (engine, name) = engine_for(app_name, bus)?;
            let file_note = note_file.as_deref().map(read_note_file).transpose()?;
            let piped_note = if note.is_none() && file_note.is_none() && !io::stdin().is_terminal()
            {
                Some(read_stdin_note()?)
            } else {
                None
            };
            let note = note
                .as_deref()
                .or(file_note.as_deref())
                .or(piped_note.as_deref());
            engine.record_version(&name, note)?;
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
        Command::Studio { port } => studio_server::serve(port, bus.clone()).await?,
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

fn read_note_file(path: &std::path::Path) -> Result<String, Box<dyn std::error::Error>> {
    read_bounded_utf8(path, MAX_TEXT_FILE_BYTES, "note file")
}

fn read_stdin_note() -> io::Result<String> {
    let mut note = String::new();
    io::stdin().read_to_string(&mut note)?;
    Ok(note)
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
    let bytes = fs::read(path)?;
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
    fn studio_uses_a_predictable_unprivileged_port_by_default() {
        let parsed = Cli::try_parse_from(["tohseno", "studio"]).unwrap();
        assert!(matches!(
            parsed.command,
            Command::Studio {
                port: DEFAULT_STUDIO_PORT,
            }
        ));

        let ephemeral = Cli::try_parse_from(["tohseno", "studio", "--port", "0"]).unwrap();
        assert!(matches!(ephemeral.command, Command::Studio { port: 0 }));
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
    fn create_and_evolve_are_filesystem_recording_commands() {
        let create_help = Cli::try_parse_from(["tohseno", "create", "--help"])
            .unwrap_err()
            .to_string();
        assert!(create_help.contains("embedded .tohseno history"));
        for removed in ["prompt", "image", "harness", "model", "route", "launch"] {
            assert!(!create_help.contains(removed), "create exposes {removed}");
        }
        let create = Cli::try_parse_from(["tohseno", "create", "field-notebook"]).unwrap();
        assert!(matches!(
            create.command,
            Command::Create { app_name } if app_name == "field-notebook"
        ));

        let evolve = Cli::try_parse_from([
            "tohseno",
            "evolve",
            "field-notebook",
            "--note-file",
            "/tmp/version-note.md",
        ])
        .unwrap();
        assert!(matches!(
            evolve.command,
            Command::Evolve {
                app_name: Some(app_name),
                note: None,
                note_file: Some(path),
            } if app_name == "field-notebook"
                && path == PathBuf::from("/tmp/version-note.md")
        ));
        let evolve_help = Cli::try_parse_from(["tohseno", "evolve", "--help"])
            .unwrap_err()
            .to_string();
        assert!(evolve_help.contains("--note-file"));
        assert!(!evolve_help.contains("prompt"));
        assert!(Cli::try_parse_from([
            "tohseno",
            "evolve",
            "field-notebook",
            "--prompt-file",
            "/tmp/version-note.md",
        ])
        .is_ok());
        assert!(Cli::try_parse_from(["tohseno", "create", "app", "--harness", "codex"]).is_err());
        assert!(Cli::try_parse_from(["tohseno", "evolve", "app", "--image", "ref.png"]).is_err());
    }
    #[test]
    fn ordinary_help_contains_only_the_product_loop() {
        let help = Cli::try_parse_from(["tohseno", "--help"])
            .unwrap_err()
            .to_string();
        for command in [
            "create",
            "evolve",
            "studio",
            "list",
            "doctor",
            "update",
            "uninstall",
            "advanced",
        ] {
            assert!(help.contains(&format!("  {command}")), "missing {command}");
        }
        for hidden in [
            "verify", "feedback", "genome", "identity", "protocol", "token", "shot",
        ] {
            assert!(!help.contains(&format!("  {hidden}")), "exposed {hidden}");
        }
        assert!(!help.contains("  install"));
        for removed in ["install", "refresh", "retire", "intent", "adopt", "shot"] {
            assert!(
                Cli::try_parse_from(["tohseno", removed]).is_err(),
                "obsolete command remains parseable: {removed}"
            );
        }
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
