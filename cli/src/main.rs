mod intake;
mod renderer;

use clap::{Parser, Subcommand};
use renderer::Renderer;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use tohseno_engine::{Event, EventBus, Ledger};

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

    match cli.command {
        Command::List => list(&bus)?,
        Command::Create {
            app_name,
            prompt_file,
        } => {
            let prompt = intake::collect(prompt_file.as_deref(), &bus)?;
            let intent = tohseno_engine::gates::intent::Intent::parse(&prompt);
            bus.emit(Event::status(format!("preparing shot 1 of {app_name}…")));
            bus.emit(Event::status(format!(
                "captured {} characters and {} images.",
                intent.prompt.chars().count(),
                intent.images.len().min(8)
            )));
        }
        Command::Evolve {
            app_name,
            prompt_file,
        } => {
            let prompt = intake::collect(prompt_file.as_deref(), &bus)?;
            let intent = tohseno_engine::gates::intent::Intent::parse(&prompt);
            bus.emit(Event::status(format!(
                "preparing the next shot of {app_name}…"
            )));
            bus.emit(Event::status(format!(
                "captured {} characters and {} images.",
                intent.prompt.chars().count(),
                intent.images.len().min(8)
            )));
        }
        Command::Refresh { app_name } => {
            let subject = app_name.as_deref().unwrap_or("every app");
            bus.emit(Event::status(format!("refreshing {subject}…")));
        }
        Command::Retire { app_name } => {
            bus.emit(Event::status(format!("retiring {app_name}…")));
        }
        Command::Studio { port } => {
            bus.emit(Event::status(format!(
                "opening studio on port {}…",
                if port == 0 {
                    "auto".into()
                } else {
                    port.to_string()
                }
            )));
        }
        Command::Doctor { background } => {
            if !background {
                bus.emit(Event::status("checking this Mac…"));
            }
        }
    }

    drop(bus);
    render_task.await??;
    Ok(())
}

fn list(bus: &EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = Ledger::discover()?;
    let apps = ledger.list_apps()?;
    if apps.is_empty() {
        bus.emit(Event::status("no apps yet."));
    } else {
        for app in apps {
            let shots = app
                .latest_shot
                .map(|shot| format!("shots 1–{shot}"))
                .unwrap_or_else(|| "no complete shots".into());
            bus.emit(Event::status(format!("{} · {shots}", app.name)));
        }
    }
    Ok(())
}
