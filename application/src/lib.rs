//! Shared TOHSENO product application service.
//!
//! This crate is the sole product orchestration boundary used by the CLI,
//! loopback Studio server, companion command processor, and conformance
//! clients. It deliberately does not contain terminal or HTTP rendering.

pub mod application_service;
pub mod command;
pub mod execution_manager;
pub mod journal;
pub mod snapshot;

pub use application_service::{
    ApplicationError, CommandRecoverySummary, CreateShotCommand, CreateShotReceipt,
    EvolveShotCommand, EvolveShotReceipt, FactoryDefaults, FeedbackActionSummary, FeedbackReceipt,
    MarketingNoteReceipt, ReferenceInput, ShotApplicationService, SubmitFeedbackCommand,
    SubmitMarketingNoteCommand,
};
pub use command::{
    CommandKind, CommandOrigin, CommandRecord, CommandResult, CommandState, CommandStatus,
};
pub use journal::{Admission, CommandJournal, JournalError};
pub use snapshot::{
    ExecutionSummary, IconDescriptor, ShotKind, ShotSummary, SupportedCompanionAction,
    WorkspaceSnapshot,
};
