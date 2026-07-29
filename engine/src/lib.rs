//! The local TOHSENO engine.
//!
//! This crate owns state and orchestration. It deliberately contains no terminal
//! rendering or HTTP code; frontends subscribe to [`events::Event`] values.

pub mod apple_identity;
pub mod builder_identity;
pub mod config;
pub mod events;
pub mod gates;
pub mod genome;
pub mod harness;
pub mod ledger;
pub mod machine;
pub mod page;
pub mod protocol_lifecycle;
pub mod public_actions;
pub mod public_network;
pub mod public_submission;
pub mod recovery;
pub mod verifier;

pub use config::Config;
pub use events::{Event, EventBus};
pub use harness::HarnessOption;
pub use ledger::{AppRecord, Ledger, LedgerError, Shot};
pub use machine::{ConductedCreation, DevicePipeline, Engine, EngineError, ShotRequest};
