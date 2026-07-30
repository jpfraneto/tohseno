use clap::{Parser, Subcommand};
use serde::Serialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use tohseno_node::{serve, Node, NodeError, NodeStore, Peer, Result, SyncState};
use tohseno_protocol::digest::ShotId;
use tokio::net::TcpListener;

#[derive(Debug, Parser)]
#[command(
    name = "tohseno-node",
    version,
    about = "Preserve and revalidate bounded neutral or legacy TOHSENO lineage evidence"
)]
struct Cli {
    /// Persistent node storage. Defaults to ~/.tohseno-node.
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    /// Static peer origin. Repeat for more than one peer.
    #[arg(long, global = true, value_name = "HTTP_ORIGIN")]
    peer: Vec<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serve the node's bounded HTTP API.
    Serve {
        #[arg(long, default_value = "127.0.0.1:8788")]
        listen: SocketAddr,
    },
    /// Show node identity, holdings, peers, and current sync state.
    Status,
    /// Pull bounded neutral or legacy evidence records from configured static peers.
    Sync,
    /// Validate and append one bounded signed lineage evidence record.
    Ingest {
        #[arg(value_name = "SIGNED_ACTION_JSON")]
        action: PathBuf,
    },
    /// Inspect all locally held evidence records and observed heads for one Shot.
    Inspect { shot_id: String },
    /// Revalidate append-only storage and derived indexes.
    Integrity {
        /// Replace the in-memory derived index with a fresh disk rebuild.
        #[arg(long)]
        rebuild: bool,
    },
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Cli::parse()).await {
        eprintln!("tohseno-node: {error}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let root = match cli.root {
        Some(root) => root,
        None => default_root()?,
    };
    let peers = cli
        .peer
        .iter()
        .map(|peer| Peer::parse(peer))
        .collect::<Result<Vec<_>>>()?;
    let node = Node::new(NodeStore::open(root)?, peers)?;
    match cli.command {
        Command::Serve { listen } => {
            let listener = TcpListener::bind(listen).await?;
            println!(
                "tohseno-node {} listening on http://{}",
                node.store().identity().node_id,
                listener.local_addr()?
            );
            serve(node, listener).await
        }
        Command::Status => {
            let status = StatusOutput {
                node: node.store().info()?,
                health: node.store().health()?,
                peers: node.peers(),
                sync: node.sync_status().await,
            };
            print_json(&status)
        }
        Command::Sync => {
            let report = node.sync().await?;
            print_json(&report)?;
            if report.state == SyncState::Failed {
                return Err(NodeError::PeerResponse(
                    "one or more configured peers failed".into(),
                ));
            }
            Ok(())
        }
        Command::Ingest { action } => print_json(&node.store().ingest_file(action)?),
        Command::Inspect { shot_id } => {
            let shot_id: ShotId = serde_json::from_str(&format!("\"{shot_id}\""))?;
            print_json(&node.store().shot(shot_id)?)
        }
        Command::Integrity { rebuild } => {
            let report = if rebuild {
                node.store().rebuild()?
            } else {
                node.store().integrity()?
            };
            let ok = report.ok;
            print_json(&report)?;
            if !ok {
                return Err(NodeError::Protocol(
                    "storage integrity verification failed".into(),
                ));
            }
            Ok(())
        }
    }
}

#[derive(Serialize)]
struct StatusOutput {
    node: tohseno_node::NodeInfo,
    health: tohseno_node::Health,
    peers: Vec<tohseno_node::PeerDescription>,
    sync: tohseno_node::SyncReport,
}

fn default_root() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        NodeError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "HOME is not set; pass --root",
        ))
    })?;
    Ok(PathBuf::from(home).join(".tohseno-node"))
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_exposes_bounded_evidence_ingestion_without_claiming_public_authority() {
        use clap::CommandFactory;

        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("ingest"));
        assert!(help.contains("bounded signed lineage evidence record"));
        assert!(help.contains("bounded neutral or legacy TOHSENO lineage evidence"));
        assert!(!help.contains("public action"));
        assert!(!help.contains("public authority"));
        assert!(!help.contains("publish"));
    }

    #[test]
    fn ingest_requires_exactly_one_action_path() {
        use clap::Parser;

        let parsed = Cli::try_parse_from([
            "tohseno-node",
            "--root",
            "/tmp/tohseno-node-test",
            "ingest",
            "action.json",
        ])
        .unwrap();
        assert!(matches!(
            parsed.command,
            Command::Ingest { action } if action == PathBuf::from("action.json")
        ));
    }
}
