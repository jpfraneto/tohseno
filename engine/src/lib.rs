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
pub mod intent_envelope;
pub mod intent_package;
pub mod ledger;
pub mod machine;
pub mod page;
pub mod pending_intention;
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
pub use intent_envelope::{decrypt_intent_envelope, IntentEnvelopeError, INTENT_ENVELOPE_AAD};
pub use intent_package::{
    build_intent_package, parse_intent_package, IntentPackage, IntentPackageError,
    IntentPackageReference, INTENT_PACKAGE_SCHEMA,
};
pub use ledger::{AppRecord, Evolution, Ledger, LedgerError};
pub use machine::{
    AcceptedGenomeRevision, ConductedCreation, DevicePipeline, Engine, EngineError,
    InitialExpressionPlan, InitialOrganPlan, ShotRequest, TokenAssociationReceipt,
};
pub use pending_intention::{
    LocalPendingIntention, LocalPendingReference, PendingIntentionError, PendingIntentionSource,
    PendingIntentionState, PendingIntentionStore,
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
