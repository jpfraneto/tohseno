//! The local TOHSENO engine.
//!
//! This crate owns state and orchestration. It deliberately contains no terminal
//! rendering or HTTP code; frontends subscribe to [`events::Event`] values.

pub mod events;
pub mod gates;
pub mod ledger;
pub mod machine;

pub use events::{Event, EventBus};
pub use ledger::{AppRecord, Ledger, LedgerError, Shot};
pub use machine::{DevicePipeline, EngineError};
