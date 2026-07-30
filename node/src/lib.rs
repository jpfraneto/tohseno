//! A protocol-driven TOHSENO lineage node.
//!
//! Nodes keep an append-only, content-addressed subset of public signed
//! lineage. They deterministically validate what they possess, rebuild all
//! indexes from canonical actions, and synchronize only when explicitly
//! asked. They do not elect a universal head or gain ownership by storing an
//! action. A public middle segment may be preserved with unresolved authority;
//! only a complete neutral reduction plus pinned candidate BuilderAccount
//! prediction is reported as candidate-authorized before any ownership
//! transfer. Ownership actions and their descendants remain neutrally valid
//! but candidate-authority unresolved until a transfer proof is defined.

#![forbid(unsafe_code)]

mod candidate;
mod error;
mod fs;
mod http;
mod model;
mod protocol_adapter;
mod store;
mod sync;

pub use candidate::{candidate_contract_configuration, predict_candidate_builder_id};
pub use error::{NodeError, Result};
pub use http::{router, serve};
pub use model::{
    ActionReference, ActionValidation, AuthorityStatus, CandidateContractConfiguration, Health,
    IngestOutcome, IntegrityIssue, IntegrityReport, MissingArtifact, MissingParentReference,
    NodeIdentity, NodeInfo, PeerDescription, PeerSyncResult, PlannedContract, SegmentStatus,
    ShotSummary, ShotView, SignedRecordStatus, SyncReport, SyncState, ValidationCounts,
};
pub use store::{NodeStore, MAX_ACTIONS_PER_NODE, MAX_ACTION_BYTES};
pub use sync::{Node, Peer, MAX_PEERS};
pub use tohseno_protocol::lineage::{LINEAGE_PROTOCOL, LINEAGE_PROTOCOL_VERSION};

pub const NODE_PROTOCOL: &str = "tohseno.node/2";
