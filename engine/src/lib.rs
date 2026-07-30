//! The local TOHSENO engine.
//!
//! This crate owns state and orchestration. It deliberately contains no terminal
//! rendering or HTTP code; frontends subscribe to [`events::Event`] values.

mod app_metadata_policy;
pub mod apple_identity;
pub mod builder_identity;
pub mod config;
pub mod contract_generation;
pub mod enclave;
pub mod events;
pub mod gates;
pub mod genome;
pub mod harness;
pub mod ledger;
pub mod machine;
pub mod page;
pub mod protocol_lifecycle;
pub mod recovery;
pub mod shot_execution;
pub mod shot_layout;
pub mod verifier;

pub use config::Config;
pub use events::{Event, EventBus};
pub use harness::{
    AttachmentBehavior, AuthenticationStatus, HarnessCommand, HarnessModel, HarnessOption,
    HarnessRoute, HarnessSelection,
};
pub use ledger::{AppRecord, Evolution, Ledger, LedgerError};
pub use machine::{
    AcceptedGenomeRevision, ConductedCreation, DevicePipeline, Engine, EngineError,
    InitialExpressionPlan, InitialOrganPlan, ShotRequest, TokenAssociationReceipt,
};
pub use shot_execution::{
    ChangedFile, CompletionRecord, ExecutionOutcome, ExecutionPhase, ExecutionPreparation,
    ExecutionReference, PreparedExecution, ShotExecutionError, ShotExecutionEvent,
    ValidationObservation,
};
pub use shot_layout::{
    describe_feedback_attachment, hash_expression_working_tree, render_genome_document,
    AcceptedMaterialization, DerivedExpressionHead, DerivedShotSnapshot, ImportedShot,
    PortableFile, PortableShotManifest, PortableVisibility, PreparedIntentPackage,
    PreparedIntentReference, ShotBodyVerification, ShotLayout, ShotLayoutError, StoredFeedback,
};
