//! The local TOHSENO engine.
//!
//! This crate owns state and orchestration. It deliberately contains no terminal
//! rendering or HTTP code; frontends subscribe to [`events::Event`] values.

#[cfg(test)]
mod anky_fixture;
mod app_metadata_policy;
pub mod apple_capabilities;
pub mod apple_identity;
pub mod birth_plan;
pub mod builder_identity;
pub mod claims_activation;
pub mod conception;
pub mod config;
pub mod contract_generation;
pub mod enclave;
pub mod events;
pub mod experience;
pub mod factory_identity;
pub mod gates;
pub mod genome;
pub mod harness;
pub mod harness_usage;
pub mod intent_envelope;
pub mod intent_package;
pub mod ledger;
pub mod machine;
pub mod page;
pub mod pending_intention;
pub mod protocol_lifecycle;
pub mod recovery;
#[doc(hidden)]
pub mod safe_file;
pub mod shot_execution;
pub mod shot_layout;
mod swift_source;
pub mod verifier;
pub mod workshop;

pub use apple_capabilities::{
    AppleCapabilityCatalog, AppleCapabilityProfile, AppleDeviceProfile, CapabilityProfileError,
    CapabilityState, APPLE_CAPABILITY_CATALOG_SCHEMA, APPLE_CAPABILITY_PROFILE_SCHEMA,
};
pub use birth_plan::{
    protocol_substrate_organs, BirthExpressionPlan, BirthOrganPlan, BirthPlan, BirthPlanError,
    OrganKind, BIRTH_EXPRESSION_PLAN_SCHEMA, BIRTH_PLAN_SCHEMA,
};
pub use conception::{
    synthesize, ConceptionError, ConceptionInput, ConceptionOutput, CONCEPTION_INPUT_SCHEMA,
    CONCEPTION_OUTPUT_SCHEMA,
};
pub use config::{
    Config, CustomHarnessConfig, HarnessConfig, IntelligenceConfig, LocalEndpointConfig,
};
pub use events::{Event, EventBus, FactoryStage};
pub use experience::{
    evaluate_birth, BirthReceipt, ExperienceContract, ExperienceError, ExperienceTrial,
    IncompletenessCategory, BIRTH_RECEIPT_SCHEMA, EXPERIENCE_CONTRACT_SCHEMA,
    EXPERIENCE_TRIAL_SCHEMA,
};
pub use factory_identity::{FactoryIdentity, FACTORY_IDENTITY_SCHEMA};
pub use harness::{
    AttachmentBehavior, AuthenticationStatus, HarnessAdapter, HarnessCommand, HarnessModel,
    HarnessOption, HarnessRoute, HarnessSelection,
};
pub use harness_usage::{read_harness_usage, HarnessUsage, HARNESS_USAGE_SCHEMA};
pub use intent_envelope::{decrypt_intent_envelope, IntentEnvelopeError, INTENT_ENVELOPE_AAD};
pub use intent_package::{
    build_intent_package, parse_intent_package, IntentPackage, IntentPackageError,
    IntentPackageReference, INTENT_PACKAGE_SCHEMA,
};
pub use ledger::{AppRecord, Evolution, Ledger, LedgerError};
pub use machine::{
    AcceptedGenomeRevision, AcceptedVersionBase, AppKind, ConductedCreation, ConductionPhase,
    DevicePipeline, Engine, EngineError, InitialExpressionPlan, InitialOrganPlan, ShotRequest,
    TokenAssociationReceipt,
};
pub use pending_intention::{
    LocalPendingIntention, LocalPendingReference, PendingIntentionError, PendingIntentionSource,
    PendingIntentionState, PendingIntentionStore,
};
pub use shot_execution::{
    ChangedFile, CompletionRecord, ExecutionMode, ExecutionOutcome, ExecutionPhase,
    ExecutionPreparation, ExecutionReference, PreparedExecution, ShotExecutionError,
    ShotExecutionEvent, ValidationObservation,
};
pub use shot_layout::{
    describe_feedback_attachment, hash_expression_working_tree, render_genome_document,
    AcceptedMaterialization, DerivedExpressionHead, DerivedShotSnapshot, ImportedShot,
    PortableFile, PortableShotManifest, PortableVisibility, PreparedIntentPackage,
    PreparedIntentReference, ShotBodyVerification, ShotLayout, ShotLayoutError, StoredFeedback,
};
pub use workshop::{
    create_workshop_feedback, materialize_workshop, read_workshop_feedback, read_workshop_receipt,
    share_workshop, WorkshopFeedbackPacket, WorkshopMaterialization, WorkshopReceipt,
    WorkshopShare, WORKSHOP_CAPSULE_EXTENSION, WORKSHOP_FEEDBACK_EXTENSION,
};
