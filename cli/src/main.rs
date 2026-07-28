mod intake;
mod renderer;
mod studio_server;

use clap::{Parser, Subcommand};
use renderer::Renderer;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use tohseno_engine::gates::intent::Intent;
use tohseno_engine::{Engine, Event, EventBus, Ledger, ShotRequest};

#[derive(Debug, Parser)]
#[command(
    name = "tohseno",
    version,
    about = "A printing press for iOS apps",
    disable_help_subcommand = true
)]
struct Cli {
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
    },
    /// Create a new complete shot using the previous shot as context.
    Evolve {
        app_name: String,
        #[arg(long, value_name = "PATH")]
        prompt_file: Option<PathBuf>,
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
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Cli::parse()).await {
        eprintln!("tohseno: {error}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let bus = EventBus::default();
    let renderer = Renderer::new(io::stdout(), io::stdout().is_terminal());
    let render_task = tokio::spawn(renderer.follow(bus.subscribe()));
    let outcome = dispatch(cli.command, &bus).await;
    drop(bus);
    render_task.await??;
    outcome
}

async fn dispatch(command: Command, bus: &EventBus) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Command::List => list(bus)?,
        Command::Create {
            app_name,
            prompt_file,
        } => {
            let prompt = intake::collect(prompt_file.as_deref(), bus)?;
            Engine::discover(bus.clone())?
                .create(ShotRequest {
                    app_name,
                    intent: Intent::parse(&prompt),
                })
                .await?;
        }
        Command::Evolve {
            app_name,
            prompt_file,
        } => {
            let prompt = intake::collect(prompt_file.as_deref(), bus)?;
            Engine::discover(bus.clone())?
                .evolve(ShotRequest {
                    app_name,
                    intent: Intent::parse(&prompt),
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
    }
    Ok(())
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
